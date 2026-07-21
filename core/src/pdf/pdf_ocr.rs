// Ported from kkdoc (MIT): reference/kkdoc/src/ocr/pdf-ocr.ts
//
//! PDF OCR pipeline — rasterizes triage-selected pages, runs the built-in OCR
//! engine, and maps recognized text lines back into PDF coordinate space.
//!
//! Pipeline: `pdf::triage::classify_document` picks pages to OCR — Scanned
//! pages are rasterized whole-page, Mixed pages only at their `ocr_regions`.
//! OCR runs in raster-pixel space (top-left origin); [`line_to_block`]
//! converts back to PDF points (bottom-left origin), the same conversion as
//! the reference `ocrItemsToBlocks`.
//!
//! Rasterization (and, for tests, recognition) is injected so the selection
//! and coordinate-math logic is unit-testable without pdfium or ONNX models
//! — see [`run_pipeline`]. [`ocr_pdf_with_rasterizer`] pins recognition to
//! [`ocr::ocr_rgba`]; [`ocr_pdf`] additionally wires rasterization to
//! `pdfium-render` behind the optional `ocr-pdf` feature.
//!
//! **Off-by-one**: triage page numbers are 1-based; the `rasterize` callback
//! takes a 0-based page index — callers must pass `page - 1` (the reference
//! hit this exact trap, see pdf-ocr.ts:62-63).

use std::io;

use lopdf::Document;

use super::triage::{self, BoundingBox, PageTriage, PdfCategory, TriageConfig};
use crate::ocr;

/// Options controlling the OCR PDF pipeline.
#[derive(Debug, Clone)]
pub struct OcrPdfOptions {
    /// Render scale relative to 72dpi (e.g. `3.0` = 216dpi). Must be > 0.
    pub render_scale: f32,
    /// Restrict OCR to these 1-based page numbers (intersected with the
    /// triage-selected targets). `None` = every triage-selected page.
    pub pages: Option<Vec<usize>>,
}

impl Default for OcrPdfOptions {
    fn default() -> Self {
        Self { render_scale: 3.0, pages: None }
    }
}

/// A rasterized page: RGBA pixels, top-left origin, row-major.
#[derive(Debug, Clone)]
pub struct RasterPage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// One recognized text block, in PDF point space (bottom-left origin).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OcrTextBlock {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub confidence: f32,
}

/// OCR result for a single page.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageOcr {
    pub page: usize,
    pub category: PdfCategory,
    pub blocks: Vec<OcrTextBlock>,
    pub text: String,
}

/// A page selected for OCR by triage — full page or specific regions.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// Whole page needs rasterizing + OCR (Scanned).
    FullPage(usize),
    /// Only these PDF-point regions need OCR (Mixed).
    Regions(usize, Vec<BoundingBox>),
}

impl Target {
    fn page(&self) -> usize {
        match self {
            Target::FullPage(p) | Target::Regions(p, _) => *p,
        }
    }
}

/// Pick OCR targets from triage results: Scanned → full page, Mixed → its
/// `ocr_regions`, TextNative/Unknown → skipped. `opts.pages`, when set,
/// intersects the selection.
fn select_ocr_targets(triage: &[PageTriage], opts: &OcrPdfOptions) -> Vec<Target> {
    triage
        .iter()
        .filter(|t| opts.pages.as_ref().map_or(true, |pages| pages.contains(&t.page)))
        .filter_map(|t| match t.category {
            PdfCategory::Scanned => Some(Target::FullPage(t.page)),
            PdfCategory::Mixed => Some(Target::Regions(t.page, t.ocr_regions.clone())),
            PdfCategory::TextNative | PdfCategory::Unknown => None,
        })
        .collect()
}

/// Convert an OCR line (raster-pixel space, top-left origin) into a PDF-point
/// text block (bottom-left origin). Mirrors the reference's
/// `ocrItemsToBlocks` — `sx`/`sy` are this page's raster/PDF-point scale
/// factors (`raster_w / pdf_w`, `raster_h / pdf_h`).
fn line_to_block(line: &ocr::OcrLine, pdf_h: f64, sx: f64, sy: f64) -> OcrTextBlock {
    let x = line.x as f64 / sx;
    let y = pdf_h - (line.y as f64 + line.h as f64) / sy;
    let width = line.w as f64 / sx;
    let height = line.h as f64 / sy;
    OcrTextBlock {
        text: line.text.clone(),
        x: x as f32,
        y: y as f32,
        width: width as f32,
        height: height as f32,
        confidence: line.confidence,
    }
}

/// Convert a region's PDF-point bbox into a raster-pixel crop rect, clamped
/// to the raster's actual bounds. `None` if the region is degenerate or
/// falls entirely outside the raster (e.g. from scale rounding).
fn region_to_px_rect(
    region: &BoundingBox,
    pdf_h: f64,
    sx: f64,
    sy: f64,
    raster_w: u32,
    raster_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    if region.width <= 0.0 || region.height <= 0.0 || sx <= 0.0 || sy <= 0.0 {
        return None;
    }
    // Round/clamp both endpoints of each axis *before* deriving width/height.
    // Rounding x0 and (x1 - x0) independently can overshoot the raster bound
    // by a pixel when x0 lands on a half-pixel boundary — e.g. x0=0.5,
    // x1=2.0 (raster_w=2) would give x=round(0.5)=1, w=round(1.5)=2, so
    // x+w=3 > raster_w=2 and a perfectly valid region gets rejected.
    let x0 = (region.x * sx).max(0.0).min(raster_w as f64).round();
    let y0 = ((pdf_h - (region.y + region.height)) * sy).max(0.0).min(raster_h as f64).round();
    let x1 = ((region.x + region.width) * sx).max(0.0).min(raster_w as f64).round();
    let y1 = ((pdf_h - region.y) * sy).max(0.0).min(raster_h as f64).round();
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    if w == 0 || h == 0 {
        None
    } else {
        Some((x0 as u32, y0 as u32, w, h))
    }
}

/// Crop an RGBA buffer to `(x, y, w, h)`. `None` on any size/bounds
/// mismatch — callers skip the region rather than panic.
fn crop_rgba(rgba: &[u8], src_w: u32, src_h: u32, x: u32, y: u32, w: u32, h: u32) -> Option<Vec<u8>> {
    if rgba.len() != src_w as usize * src_h as usize * 4 {
        return None;
    }
    if w == 0 || h == 0 || x.checked_add(w)? > src_w || y.checked_add(h)? > src_h {
        return None;
    }
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for row in y..y + h {
        let start = (row as usize * src_w as usize + x as usize) * 4;
        out.extend_from_slice(&rgba[start..start + w as usize * 4]);
    }
    Some(out)
}

/// Shared pipeline: triage → select targets → rasterize → recognize →
/// convert coordinates. `rasterize`/`recognize` are injected so this stays
/// testable without pdfium or ONNX models — see [`ocr_pdf_with_rasterizer`]
/// and [`ocr_pdf`] for the real wiring.
fn run_pipeline<R, G>(
    bytes: &[u8],
    mut rasterize: R,
    mut recognize: G,
    opts: &OcrPdfOptions,
) -> io::Result<Vec<PageOcr>>
where
    R: FnMut(usize, f32) -> io::Result<RasterPage>,
    G: FnMut(&[u8], u32, u32) -> ocr::Result<Vec<ocr::OcrLine>>,
{
    if opts.render_scale <= 0.0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "render_scale must be > 0"));
    }

    let doc = Document::load_mem(bytes)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("PDF open failed: {e}")))?;
    let triage = triage::classify_document(&doc, &TriageConfig::default());
    let page_ids = doc.get_pages();
    let dpi = 72.0 * opts.render_scale;

    let mut out = Vec::new();
    for target in select_ocr_targets(&triage, opts) {
        let page_num = target.page();
        let page_id = match page_ids.get(&(page_num as u32)) {
            Some(id) => *id,
            None => continue,
        };
        // MediaBox width/height <= 0 would divide-by-zero in the scale
        // factors below — skip rather than propagate NaN/inf blocks.
        let (pdf_w, pdf_h) = match triage::page_media_box_wh(&doc, page_id) {
            Some((w, h)) if w > 0.0 && h > 0.0 => (w, h),
            _ => continue,
        };
        // rasterize is 0-based (see module docs); triage pages are 1-based.
        let raster = match rasterize(page_num - 1, dpi) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if raster.rgba.len() != raster.width as usize * raster.height as usize * 4 {
            continue;
        }
        let sx = raster.width as f64 / pdf_w;
        let sy = raster.height as f64 / pdf_h;
        let category = triage
            .iter()
            .find(|t| t.page == page_num)
            .map(|t| t.category)
            .unwrap_or(PdfCategory::Unknown);

        let mut blocks = Vec::new();
        match &target {
            Target::FullPage(_) => {
                if let Ok(lines) = recognize(&raster.rgba, raster.width, raster.height) {
                    blocks.extend(lines.iter().map(|l| line_to_block(l, pdf_h, sx, sy)));
                }
            }
            Target::Regions(_, regions) => {
                for region in regions {
                    let Some((rx, ry, rw, rh)) =
                        region_to_px_rect(region, pdf_h, sx, sy, raster.width, raster.height)
                    else {
                        continue;
                    };
                    let Some(cropped) = crop_rgba(&raster.rgba, raster.width, raster.height, rx, ry, rw, rh) else {
                        continue;
                    };
                    if let Ok(lines) = recognize(&cropped, rw, rh) {
                        // Lines come back relative to the crop — shift by the
                        // crop's offset before converting to page-level PDF pt.
                        blocks.extend(lines.iter().map(|l| {
                            let shifted = ocr::OcrLine { x: l.x + rx, y: l.y + ry, ..l.clone() };
                            line_to_block(&shifted, pdf_h, sx, sy)
                        }));
                    }
                }
            }
        }

        // Reading order: top-to-bottom (descending PDF y), then left-to-right.
        blocks.sort_by(|a, b| {
            b.y.partial_cmp(&a.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        let text = blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n");
        out.push(PageOcr { page: page_num, category, blocks, text });
    }
    Ok(out)
}

/// Run OCR over a PDF's triage-selected pages using an injected rasterizer.
/// Recognition is pinned to [`ocr::ocr_rgba`] (the built-in engine).
///
/// Fails fast if [`ocr::ocr_available`] is false — without the `ocr` feature
/// and downloaded models there is nothing productive this pipeline can do.
pub fn ocr_pdf_with_rasterizer<R>(bytes: &[u8], rasterize: R, opts: &OcrPdfOptions) -> io::Result<Vec<PageOcr>>
where
    R: FnMut(usize, f32) -> io::Result<RasterPage>,
{
    if !ocr::ocr_available() {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "OCR not available — build with `--features ocr` and run scripts/download-ocr-models.sh",
        ));
    }
    run_pipeline(bytes, rasterize, ocr::ocr_rgba, opts)
}

/// Rasterize with `pdfium-render` and OCR the result. Requires the
/// `ocr-pdf` feature (pulls in `pdfium-render`, which needs a system
/// `libpdfium` at runtime — the reference uses `@hyzyla/pdfium` for the
/// same job).
#[cfg(feature = "ocr-pdf")]
pub fn ocr_pdf(bytes: &[u8], opts: &OcrPdfOptions) -> io::Result<Vec<PageOcr>> {
    use pdfium_render::prelude::*;

    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./"))
            .or_else(|_| Pdfium::bind_to_system_library())
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("pdfium bind failed: {e}")))?,
    );
    let document = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("pdfium load failed: {e}")))?;

    // pdfium renders at 72dpi by default (1 point == 1 pixel), so
    // scale_page_by_factor(dpi / 72) gets us the target DPI.
    // as_rgba_bytes() already normalizes pdfium's native bitmap format into
    // RGBA — no manual BGRA swap needed here (cf. reference's bgraToRgba).
    ocr_pdf_with_rasterizer(
        bytes,
        move |page_idx, dpi| {
            let page = document
                .pages()
                .get(page_idx as PdfPageIndex)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("pdfium page {page_idx}: {e}")))?;
            let config = PdfRenderConfig::new().scale_page_by_factor(dpi / 72.0);
            let bitmap = page
                .render_with_config(&config)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("pdfium render page {page_idx}: {e}")))?;
            let width = bitmap.width() as u32;
            let height = bitmap.height() as u32;
            Ok(RasterPage { rgba: bitmap.as_rgba_bytes(), width, height })
        },
        opts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{content::Content, content::Operation, dictionary, Object, Stream};
    use std::cell::Cell;

    fn dummy_triage(page: usize, category: PdfCategory, regions: Vec<BoundingBox>) -> PageTriage {
        PageTriage {
            page,
            category,
            confidence: 0.9,
            text_coverage: 0.0,
            image_coverage: 0.0,
            image_count: 0,
            font_reliability: None,
            has_invisible_text: None,
            contains_cjk: None,
            ocr_underlay_ratio: None,
            ocr_iou: None,
            garbled_ratio: None,
            ocr_regions: regions,
        }
    }

    // --- 1) coordinate conversion -----------------------------------------

    #[test]
    fn line_to_block_converts_raster_px_to_pdf_pt() {
        let line = ocr::OcrLine { text: "hi".into(), x: 300, y: 150, w: 600, h: 60, confidence: 0.9 };
        let block = line_to_block(&line, 792.0, 3.0, 3.0);
        assert_eq!(block.x, 100.0);
        assert_eq!(block.y, 722.0);
        assert_eq!(block.width, 200.0);
        assert_eq!(block.height, 20.0);
    }

    // --- 2) select_ocr_targets ----------------------------------------------

    #[test]
    fn select_targets_routes_by_category() {
        let region = BoundingBox { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let triage = vec![
            dummy_triage(1, PdfCategory::TextNative, vec![]),
            dummy_triage(2, PdfCategory::Scanned, vec![]),
            dummy_triage(3, PdfCategory::Mixed, vec![region]),
            dummy_triage(4, PdfCategory::Unknown, vec![]),
        ];
        let targets = select_ocr_targets(&triage, &OcrPdfOptions::default());
        assert_eq!(targets, vec![Target::FullPage(2), Target::Regions(3, vec![region])]);
    }

    #[test]
    fn select_targets_intersects_opts_pages() {
        let triage = vec![
            dummy_triage(1, PdfCategory::Scanned, vec![]),
            dummy_triage(2, PdfCategory::Scanned, vec![]),
        ];
        let opts = OcrPdfOptions { pages: Some(vec![2]), ..Default::default() };
        let targets = select_ocr_targets(&triage, &opts);
        assert_eq!(targets, vec![Target::FullPage(2)]);
    }

    // --- 3) region px rect clamp --------------------------------------------

    #[test]
    fn region_px_rect_converts_and_clamps() {
        // region touches the page's right/bottom edge; raster is 1px short
        // of the theoretical size (rounding) — the rect must clamp, not
        // overflow past the actual raster bounds.
        let region = BoundingBox { x: 400.0, y: 0.0, width: 200.0, height: 100.0 };
        let rect = region_to_px_rect(&region, 200.0, 2.0, 2.0, 1199, 399).unwrap();
        assert_eq!(rect, (800, 200, 399, 199));
    }

    #[test]
    fn region_px_rect_out_of_bounds_is_none() {
        let region = BoundingBox { x: 5000.0, y: 5000.0, width: 10.0, height: 10.0 };
        assert!(region_to_px_rect(&region, 792.0, 2.0, 2.0, 1000, 1000).is_none());
    }

    #[test]
    fn region_px_rect_zero_size_is_none() {
        let region = BoundingBox { x: 0.0, y: 0.0, width: 0.0, height: 10.0 };
        assert!(region_to_px_rect(&region, 792.0, 2.0, 2.0, 1000, 1000).is_none());
    }

    #[test]
    fn region_px_rect_edge_alignment_stays_croppable() {
        // x0 lands on a half-pixel (0.5) and x1 lands exactly on the
        // raster's right edge (2.0, raster_w=2). Rounding x0 and (x1 - x0)
        // independently used to give x=round(0.5)=1, w=round(1.5)=2, so
        // x+w=3 > raster_w=2 — a perfectly valid region was rejected and its
        // OCR results silently dropped. Rounding both endpoints first must
        // keep the rect inside the raster.
        let region = BoundingBox { x: 0.5, y: 0.0, width: 1.5, height: 10.0 };
        let (x, y, w, h) = region_to_px_rect(&region, 10.0, 1.0, 1.0, 2, 10)
            .expect("edge-aligned region must still be croppable");
        assert!(x + w <= 2, "x+w={} must not exceed raster_w=2", x + w);
        assert!(y + h <= 10);

        let rgba = vec![0u8; 2 * 10 * 4];
        assert!(crop_rgba(&rgba, 2, 10, x, y, w, h).is_some());
    }

    // --- 4) crop_rgba ---------------------------------------------------------

    #[test]
    fn crop_rgba_extracts_subregion() {
        let (w, h) = (4u32, 4u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            for col in 0..w {
                let idx = ((row * w + col) * 4) as usize;
                rgba[idx] = (row * w + col) as u8;
                rgba[idx + 3] = 255;
            }
        }
        let cropped = crop_rgba(&rgba, w, h, 1, 1, 2, 2).unwrap();
        assert_eq!(cropped.len(), 2 * 2 * 4);
        assert_eq!(cropped[0], 5); // original (1,1)
        assert_eq!(cropped[4], 6); // original (2,1)
        assert_eq!(cropped[8], 9); // original (1,2)
    }

    #[test]
    fn crop_rgba_size_mismatch_is_none() {
        let rgba = vec![0u8; 10];
        assert!(crop_rgba(&rgba, 4, 4, 0, 0, 2, 2).is_none());
    }

    #[test]
    fn crop_rgba_out_of_bounds_is_none() {
        let rgba = vec![0u8; 4 * 4 * 4];
        assert!(crop_rgba(&rgba, 4, 4, 3, 3, 2, 2).is_none());
        assert!(crop_rgba(&rgba, 4, 4, 0, 0, 0, 2).is_none());
    }

    // --- 5/6) run_pipeline end-to-end with mock rasterize + recognize ------

    /// Minimal single-page PDF, `page_w x page_h` points, no content.
    /// Triages to TextNative/Unknown (no text, no images) — used for the
    /// "nothing to OCR" edge cases.
    ///
    /// MediaBox is set on the Page object itself, not just Pages — triage's
    /// `page_media_box_wh` reads the page dict directly and does not walk
    /// the Parent inheritance chain.
    fn blank_pdf_bytes(page_w: f64, page_h: f64) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save test pdf");
        bytes
    }

    /// Single-page PDF whose whole page is covered by an image XObject and
    /// no text — triages to Scanned (see `triage::classify_stage1`).
    fn scanned_pdf_bytes(page_w: f64, page_h: f64) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let image_id = doc.add_object(Stream::new(
            dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1 },
            vec![0u8],
        ));
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Im0" => image_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new("cm", vec![page_w.into(), 0.into(), 0.into(), page_h.into(), 0.into(), 0.into()]),
                Operation::new("Do", vec!["Im0".into()]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save test pdf");
        bytes
    }

    /// Same as `scanned_pdf_bytes`, except the Page object has no `MediaBox`
    /// of its own — it only exists on the `Pages` ancestor, exactly as many
    /// real-world PDFs are authored (MediaBox is an inheritable attribute
    /// per PDF 32000-1 §7.7.3.4). Regression fixture for the Parent-walk in
    /// `triage::page_media_box_wh`.
    fn scanned_pdf_bytes_inherited_mediabox(page_w: f64, page_h: f64) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let image_id = doc.add_object(Stream::new(
            dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1, "Height" => 1 },
            vec![0u8],
        ));
        let resources_id = doc.add_object(dictionary! {
            "XObject" => dictionary! { "Im0" => image_id },
        });
        let content = Content {
            operations: vec![
                Operation::new("q", vec![]),
                Operation::new("cm", vec![page_w.into(), 0.into(), 0.into(), page_h.into(), 0.into(), 0.into()]),
                Operation::new("Do", vec!["Im0".into()]),
                Operation::new("Q", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            // deliberately no "MediaBox" here
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).expect("save test pdf");
        bytes
    }

    #[test]
    fn run_pipeline_ocrs_page_with_inherited_mediabox() {
        // Before the Parent-walk fix, page_media_box_wh returned None for a
        // leaf page with no MediaBox of its own, so run_pipeline silently
        // skipped the page (`_ => continue` on the media-box match) even
        // though triage correctly routed it to Scanned — the page produced
        // zero OCR results.
        let bytes = scanned_pdf_bytes_inherited_mediabox(600.0, 800.0);
        let raster_calls = Cell::new(0usize);
        let result = run_pipeline(
            &bytes,
            |_page_idx, _dpi| {
                raster_calls.set(raster_calls.get() + 1);
                Ok(RasterPage { rgba: vec![255u8; 600 * 800 * 4], width: 600, height: 800 })
            },
            |_rgba, _w, _h| Ok(vec![ocr::OcrLine { text: "hello".into(), x: 10, y: 10, w: 50, h: 20, confidence: 0.9 }]),
            &OcrPdfOptions::default(),
        )
        .expect("pipeline ok");

        assert_eq!(raster_calls.get(), 1, "page must not be skipped for lack of an own MediaBox");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].blocks.len(), 1);
    }

    #[test]
    fn run_pipeline_full_scanned_page() {
        let bytes = scanned_pdf_bytes(600.0, 800.0);
        let raster_calls = Cell::new(0usize);
        let recognize_calls = Cell::new(0usize);
        let result = run_pipeline(
            &bytes,
            |page_idx, dpi| {
                raster_calls.set(raster_calls.get() + 1);
                assert_eq!(page_idx, 0); // 1-based triage page 1 -> 0-based callback
                assert_eq!(dpi, 216.0); // default render_scale 3.0 * 72
                Ok(RasterPage { rgba: vec![255u8; 600 * 800 * 4], width: 600, height: 800 })
            },
            |_rgba, _w, _h| {
                recognize_calls.set(recognize_calls.get() + 1);
                Ok(vec![ocr::OcrLine { text: "hello".into(), x: 10, y: 10, w: 50, h: 20, confidence: 0.9 }])
            },
            &OcrPdfOptions::default(),
        )
        .expect("pipeline ok");

        assert_eq!(raster_calls.get(), 1);
        assert_eq!(recognize_calls.get(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].page, 1);
        assert_eq!(result[0].category, PdfCategory::Scanned);
        assert_eq!(result[0].blocks.len(), 1);
        assert_eq!(result[0].text, "hello");
    }

    #[test]
    fn run_pipeline_empty_targets_never_rasterizes() {
        let bytes = blank_pdf_bytes(600.0, 800.0);
        let raster_calls = Cell::new(0usize);
        let result = run_pipeline(
            &bytes,
            |_page_idx, _dpi| {
                raster_calls.set(raster_calls.get() + 1);
                Ok(RasterPage { rgba: vec![], width: 0, height: 0 })
            },
            |_rgba, _w, _h| Ok(Vec::new()),
            &OcrPdfOptions::default(),
        )
        .expect("pipeline ok");

        assert!(result.is_empty());
        assert_eq!(raster_calls.get(), 0);
    }

    #[test]
    fn run_pipeline_skips_page_on_rgba_len_mismatch() {
        let bytes = scanned_pdf_bytes(600.0, 800.0);
        let recognize_calls = Cell::new(0usize);
        let result = run_pipeline(
            &bytes,
            // rgba buffer deliberately too short for 600x800.
            |_page_idx, _dpi| Ok(RasterPage { rgba: vec![0u8; 10], width: 600, height: 800 }),
            |_rgba, _w, _h| {
                recognize_calls.set(recognize_calls.get() + 1);
                Ok(Vec::new())
            },
            &OcrPdfOptions::default(),
        )
        .expect("pipeline ok");

        assert!(result.is_empty());
        assert_eq!(recognize_calls.get(), 0);
    }

    #[test]
    fn run_pipeline_rejects_non_positive_render_scale() {
        let bytes = blank_pdf_bytes(600.0, 800.0);
        let opts = OcrPdfOptions { render_scale: 0.0, ..Default::default() };
        let err = run_pipeline(&bytes, |_p, _d| unreachable!("should fail before rasterizing"), |_r, _w, _h| Ok(Vec::new()), &opts)
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
