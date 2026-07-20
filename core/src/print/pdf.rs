//! Best-effort `IRBlock[]` → PDF renderer, via `printpdf`'s `Op`-based
//! drawing API. Feature-gated behind `print-pdf` (see `core/Cargo.toml`).
//!
//! kkdoc's PDF stage (`reference/kkdoc/src/print/renderer.ts::htmlToPdf`)
//! shells out to `puppeteer-core` — a full headless Chromium — to
//! rasterize `renderHtml()`'s output. Bundling a browser engine into this
//! crate is out of scope (see `mod.rs`'s module doc comment), so this is
//! not a port of `htmlToPdf`; it's an independent renderer that walks
//! `IRBlock`s directly and lays them out with `printpdf` primitives —
//! headings/paragraphs as text runs, tables as plain-text rows, lists with
//! bullet/number prefixes, `Separator` as a ruled line.
//!
//! Sibling to `crate::gen_pdf` (Markdown-line → PDF, feature `pdf-out`),
//! which this reuses the general approach of (builtin fonts, fixed line
//! budget, no wrapping) but operates on structured `IRBlock`s instead of
//! raw Markdown lines, so headings/tables/lists get distinct treatment.
//!
//! # Known limitations (best-effort, see the porting brief's "한계는
//! 보고서에 명시" requirement)
//!
//! - **No Korean/CJK glyph support.** `printpdf::BuiltinFont` covers only
//!   the 14 standard PDF fonts (Helvetica/Times/Courier — Latin-1). Since
//!   HWP/HWPX source documents are overwhelmingly Korean, this PDF path is
//!   only useful for Latin-script content or as a rough layout preview
//!   until an embedded Korean TTF is wired in via `printpdf`'s
//!   `PdfDocument::add_font`. [`super::render_ir_to_html`] has no such
//!   limitation.
//! - **No text wrapping.** A long paragraph/cell runs past the right
//!   margin instead of wrapping — matches `crate::gen_pdf`'s existing
//!   fixed-budget approach rather than introducing a new line-breaking
//!   algorithm here.
//! - **Tables render as plain-text rows** (`"a | b | c"`, monospaced), not
//!   bordered grids: `col_span`/`row_span` are not visually merged, only
//!   the origin cell's text is shown per column.
//! - `RenderOptions::header` / `footer` print once per page (a single line
//!   at the top/bottom margin) rather than kordoc's puppeteer header/footer
//!   *templates* (which support `.pageNumber`/`.totalPages` HTML spans);
//!   `page_numbers` appends `"N / total"` to the footer line instead.
//! - `watermark` renders as flat gray text rotated -30° via
//!   `TextMatrix::TranslateRotate` — `printpdf::Color::Rgb` has no alpha
//!   channel, so this approximates kordoc's `rgba(0,0,0,0.08)` with a
//!   light solid gray instead of true transparency.

use std::io;

use printpdf::{
    BuiltinFont, Color, Line, LinePoint, Mm, Op, PdfDocument, PdfFontHandle, PdfPage,
    PdfSaveOptions, Point, Pt, Rgb, TextItem, TextMatrix,
};

use crate::ir::IRBlock;

use super::renderer::{default_margin, Orientation, PageSize, RenderOptions};

const GRAY: (f32, f32, f32) = (0.55, 0.55, 0.55);
const BLACK: (f32, f32, f32) = (0.05, 0.05, 0.05);

fn rgb(c: (f32, f32, f32)) -> Color {
    Color::Rgb(Rgb {
        r: c.0,
        g: c.1,
        b: c.2,
        icc_profile: None,
    })
}

fn pt_to_mm(pt: f32) -> f32 {
    pt * 25.4 / 72.0
}

/// Parses a CSS length string (`"20mm"`, `"1in"`, `"72pt"`, or a bare
/// number treated as millimeters) into millimeters. Falls back to `20.0`
/// (this renderer's own default) on anything unparseable — margins are
/// cosmetic, not worth failing the whole render over.
fn parse_mm(s: &str) -> f32 {
    let s = s.trim();
    let split_at = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(split_at);
    let value: f32 = num.parse().unwrap_or(20.0);
    match unit.trim() {
        "" | "mm" => value,
        "cm" => value * 10.0,
        "in" => value * 25.4,
        "pt" => pt_to_mm(value),
        "px" => value * 25.4 / 96.0,
        _ => value,
    }
}

fn page_dims_mm(options: &RenderOptions) -> (f32, f32) {
    let (w, h) = match options.page_size {
        PageSize::A4 => (210.0, 297.0),
        PageSize::Letter => (215.9, 279.4),
    };
    match options.orientation {
        Orientation::Portrait => (w, h),
        Orientation::Landscape => (h, w),
    }
}

/// Font sizes (pt) per block kind, mirroring the values `renderer.rs`'s CSS
/// presets use for the same preset — kept in sync manually since this path
/// has no CSS cascade to read them from.
fn preset_sizes(preset: super::renderer::PrintPreset) -> [f32; 5] {
    use super::renderer::PrintPreset::*;
    // [h1, h2, h3, h4-6, body]
    match preset {
        Default => [20.0, 16.0, 13.0, 11.0, 11.0],
        GovFormal => [18.0, 14.0, 12.0, 11.0, 11.0],
        Compact => [14.0, 12.0, 10.0, 9.0, 9.0],
    }
}

struct PageBuilder {
    ops: Vec<Op>,
    y: f32,
}

/// Lays out `blocks` into one or more pages worth of `Op`s, returning the
/// finished per-page op lists (body content only — headers/footers/page
/// numbers are appended afterward once the total page count is known).
struct Layout {
    content_left: f32,
    content_right: f32,
    content_top: f32,
    content_bottom: f32,
    sizes: [f32; 5],
}

impl Layout {
    fn line_height(size_pt: f32) -> f32 {
        pt_to_mm(size_pt * 1.5)
    }

    fn new_page(&self) -> PageBuilder {
        PageBuilder {
            ops: Vec::new(),
            y: self.content_top,
        }
    }

    fn ensure_room(&self, page: &mut PageBuilder, pages: &mut Vec<Vec<Op>>, needed: f32) {
        if page.y - needed < self.content_bottom {
            pages.push(std::mem::take(&mut page.ops));
            page.y = self.content_top;
        }
    }

    fn text_line(
        &self,
        page: &mut PageBuilder,
        pages: &mut Vec<Vec<Op>>,
        text: &str,
        size_pt: f32,
        font: BuiltinFont,
        color: (f32, f32, f32),
    ) {
        let lh = Self::line_height(size_pt);
        self.ensure_room(page, pages, lh);
        page.ops.extend([
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(self.content_left), Mm(page.y)),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(font),
                size: Pt(size_pt),
            },
            Op::SetFillColor { col: rgb(color) },
            Op::ShowText {
                items: vec![TextItem::Text(text.to_string())],
            },
            Op::EndTextSection,
        ]);
        page.y -= lh;
    }

    fn rule(&self, page: &mut PageBuilder, pages: &mut Vec<Vec<Op>>) {
        let gap = 3.0;
        self.ensure_room(page, pages, gap * 2.0);
        page.y -= gap;
        page.ops.extend([
            Op::SaveGraphicsState,
            Op::SetOutlineColor { col: rgb(GRAY) },
            Op::SetOutlineThickness { pt: Pt(0.5) },
            Op::DrawLine {
                line: Line {
                    points: vec![
                        LinePoint {
                            p: Point::new(Mm(self.content_left), Mm(page.y)),
                            bezier: false,
                        },
                        LinePoint {
                            p: Point::new(Mm(self.content_right), Mm(page.y)),
                            bezier: false,
                        },
                    ],
                    is_closed: false,
                },
            },
            Op::RestoreGraphicsState,
        ]);
        page.y -= gap;
    }

    fn render_block(&self, block: &IRBlock, page: &mut PageBuilder, pages: &mut Vec<Vec<Op>>) {
        let [h1, h2, h3, h_rest, body] = self.sizes;
        match block {
            IRBlock::Paragraph { text, footnote, href } => {
                for line in text.lines() {
                    self.text_line(page, pages, line, body, BuiltinFont::Helvetica, BLACK);
                }
                if let Some(url) = href {
                    self.text_line(page, pages, url, body * 0.85, BuiltinFont::Helvetica, GRAY);
                }
                if let Some(note) = footnote {
                    self.text_line(page, pages, note, body * 0.85, BuiltinFont::HelveticaOblique, GRAY);
                }
            }
            IRBlock::Heading { level, text } => {
                let size = match level {
                    1 => h1,
                    2 => h2,
                    3 => h3,
                    _ => h_rest,
                };
                self.text_line(page, pages, text, size, BuiltinFont::HelveticaBold, BLACK);
            }
            IRBlock::Table(table) => {
                for row in &table.cells {
                    let line = row
                        .iter()
                        .map(|c| c.text.replace('\n', " "))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    self.text_line(page, pages, &line, body * 0.85, BuiltinFont::Courier, BLACK);
                }
            }
            IRBlock::List { ordered, items } => {
                // Per-depth ordinal numbering shared with the Markdown
                // renderer (see `crate::ir::ordered_list_ordinals`) so a
                // nested ordered list numbers 1./2. per level instead of a
                // running global count.
                let ordinals = crate::ir::ordered_list_ordinals(items);
                for (idx, item) in items.iter().enumerate() {
                    let indent = "  ".repeat(item.depth as usize);
                    let prefix = if *ordered {
                        format!("{}. ", ordinals[idx])
                    } else {
                        "\u{2022} ".to_string()
                    };
                    self.text_line(
                        page,
                        pages,
                        &format!("{indent}{prefix}{}", item.text),
                        body,
                        BuiltinFont::Helvetica,
                        BLACK,
                    );
                }
            }
            IRBlock::Image { alt } => {
                self.text_line(
                    page,
                    pages,
                    &format!("[이미지: {alt}]"),
                    body * 0.85,
                    BuiltinFont::HelveticaOblique,
                    GRAY,
                );
            }
            IRBlock::Separator => {
                self.rule(page, pages);
            }
        }
    }
}

/// Renders `blocks` to PDF bytes. Layout only — see the module doc comment
/// for the (significant, documented) fidelity limitations of this path
/// versus [`super::render_ir_to_html`].
pub fn render_ir_to_pdf(blocks: &[IRBlock], options: &RenderOptions) -> io::Result<Vec<u8>> {
    let (width_mm, height_mm) = page_dims_mm(options);
    let margin = options
        .margin
        .clone()
        .unwrap_or_else(|| default_margin(options.preset));
    let margin_top = parse_mm(&margin.top);
    let margin_bottom = parse_mm(&margin.bottom);
    let margin_left = parse_mm(&margin.left);
    let margin_right = parse_mm(&margin.right);

    let layout = Layout {
        content_left: margin_left,
        content_right: width_mm - margin_right,
        content_top: height_mm - margin_top,
        content_bottom: margin_bottom,
        sizes: preset_sizes(options.preset),
    };

    let mut pages: Vec<Vec<Op>> = Vec::new();
    let mut page = layout.new_page();

    for block in blocks {
        layout.render_block(block, &mut page, &mut pages);
    }
    // Always flush the last (possibly empty) page so an empty `blocks`
    // slice still produces a single-page PDF rather than zero pages.
    pages.push(page.ops);

    let total = pages.len();
    let mut doc = PdfDocument::new("MDM Print Document");
    let mut pdf_pages = Vec::with_capacity(total);

    for (idx, mut ops) in pages.into_iter().enumerate() {
        add_chrome(&layout, &mut ops, options, idx + 1, total, height_mm, width_mm);
        pdf_pages.push(PdfPage::new(Mm(width_mm), Mm(height_mm), ops));
    }

    let mut warnings = Vec::new();
    let bytes = doc
        .with_pages(pdf_pages)
        .save(&PdfSaveOptions::default(), &mut warnings);
    Ok(bytes)
}

/// Appends header/footer/page-number/watermark ops to a single page's op
/// list, once the page's body content is already laid out.
fn add_chrome(
    layout: &Layout,
    ops: &mut Vec<Op>,
    options: &RenderOptions,
    page_no: usize,
    total: usize,
    height_mm: f32,
    width_mm: f32,
) {
    if let Some(header) = &options.header {
        ops.extend([
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(layout.content_left), Mm(height_mm - layout.content_top / 4.0)),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(9.0),
            },
            Op::SetFillColor { col: rgb(GRAY) },
            Op::ShowText {
                items: vec![TextItem::Text(header.clone())],
            },
            Op::EndTextSection,
        ]);
    }

    let footer_text = match (&options.footer, options.page_numbers) {
        (Some(f), true) => Some(format!("{f}    {page_no} / {total}")),
        (Some(f), false) => Some(f.clone()),
        (None, true) => Some(format!("{page_no} / {total}")),
        (None, false) => None,
    };
    if let Some(text) = footer_text {
        ops.extend([
            Op::StartTextSection,
            Op::SetTextCursor {
                pos: Point::new(Mm(layout.content_left), Mm(layout.content_bottom / 2.0)),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(8.0),
            },
            Op::SetFillColor { col: rgb(GRAY) },
            Op::ShowText {
                items: vec![TextItem::Text(text)],
            },
            Op::EndTextSection,
        ]);
    }

    if let Some(watermark) = &options.watermark {
        ops.extend([
            Op::SaveGraphicsState,
            Op::StartTextSection,
            Op::SetTextMatrix {
                matrix: TextMatrix::TranslateRotate(
                    Pt(width_mm * 72.0 / 25.4 / 4.0),
                    Pt(height_mm * 72.0 / 25.4 / 2.0),
                    -30.0,
                ),
            },
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
                size: Pt(48.0),
            },
            Op::SetFillColor {
                col: rgb((0.85, 0.85, 0.85)),
            },
            Op::ShowText {
                items: vec![TextItem::Text(watermark.clone())],
            },
            Op::EndTextSection,
            Op::RestoreGraphicsState,
        ]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{IRCell, IRTable};
    use crate::print::RenderOptions;

    fn pdf_bytes(blocks: &[IRBlock], options: &RenderOptions) -> Vec<u8> {
        render_ir_to_pdf(blocks, options).expect("pdf render should not fail")
    }

    #[test]
    fn empty_blocks_produce_a_valid_single_page_pdf() {
        let bytes = pdf_bytes(&[], &RenderOptions::default());
        assert!(bytes.starts_with(b"%PDF"));
        assert!(!bytes.is_empty());
    }

    #[test]
    fn each_block_kind_renders_without_panicking() {
        let blocks = vec![
            IRBlock::heading(1, "Heading"),
            IRBlock::paragraph("Body paragraph text."),
            IRBlock::Table(IRTable::new(vec![
                vec![IRCell::new("A"), IRCell::new("B")],
                vec![IRCell::new("1"), IRCell::new("2")],
            ])),
            IRBlock::List {
                ordered: false,
                items: vec!["one".into(), "two".into()],
            },
            IRBlock::Image { alt: "img1".into() },
            IRBlock::Separator,
        ];
        let bytes = pdf_bytes(&blocks, &RenderOptions::default());
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn huge_table_spans_multiple_pages_without_panicking() {
        let cells: Vec<Vec<IRCell>> = (0..500)
            .map(|r| vec![IRCell::new(format!("row {r}")), IRCell::new("value")])
            .collect();
        let blocks = vec![IRBlock::Table(IRTable::new(cells))];
        let bytes = pdf_bytes(&blocks, &RenderOptions::default());
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn header_footer_watermark_page_numbers_do_not_panic() {
        let mut options = RenderOptions::default();
        options.header = Some("Report Header".into());
        options.footer = Some("Confidential".into());
        options.watermark = Some("DRAFT".into());
        options.page_numbers = true;
        let blocks = vec![IRBlock::paragraph("content")];
        let bytes = pdf_bytes(&blocks, &options);
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn parse_mm_handles_common_units() {
        assert_eq!(parse_mm("20mm"), 20.0);
        assert!((parse_mm("1in") - 25.4).abs() < 0.01);
        assert_eq!(parse_mm("20"), 20.0);
    }
}
