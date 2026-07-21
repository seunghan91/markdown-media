//! Per-page PDF triage: decide whether a page is text-native, scanned, or mixed.
//!
//! Runs before the main parser. The result drives OCR routing: only Scanned and
//! Mixed pages are handed off to the external OCR bridge; TextNative pages go
//! straight to the Rust text extractor.
//!
//! Design: `plan/pdf-triage.md`. Thresholds live in `core/config/triage.toml`
//! and are injected via [`TriageConfig`] — do not hardcode.

use lopdf::content::Content;
use lopdf::{Document, Object, ObjectId};
use serde::Serialize;

/// Page classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfCategory {
    /// High-quality text layer; extract directly.
    TextNative,
    /// Image-only or OCR-underlay garbage; rasterize + OCR.
    Scanned,
    /// Text layer present but image regions need OCR.
    Mixed,
    /// Signals insufficient; escalate to next stage.
    Unknown,
}

/// Bounding box in PDF user-space (points, origin bottom-left).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl BoundingBox {
    pub fn area(&self) -> f64 {
        self.width.max(0.0) * self.height.max(0.0)
    }
}

/// Per-page triage output.
#[derive(Debug, Clone, Serialize)]
pub struct PageTriage {
    pub page: usize,
    pub category: PdfCategory,
    pub confidence: f32,

    // Stage 1 (always computed)
    pub text_coverage: f32,
    pub image_coverage: f32,
    pub image_count: usize,

    // Stage 2 (computed when Stage 1 is borderline)
    pub font_reliability: Option<f32>,
    pub has_invisible_text: Option<bool>,
    pub contains_cjk: Option<bool>,
    /// Fraction of text bboxes geometrically contained within an image bbox.
    /// High values (>0.5) strongly indicate an OCR underlay on a scanned page.
    pub ocr_underlay_ratio: Option<f32>,

    // Stage 3 (computed when Stage 2 still borderline)
    pub ocr_iou: Option<f32>,
    /// Proportion of decoded text chars that are likely garbled — Unicode
    /// replacement (U+FFFD), control codes outside whitespace, or from
    /// Private Use Area (bad font mapping). Elevated values mean the text
    /// layer exists but can't be read reliably → prefer OCR.
    pub garbled_ratio: Option<f32>,

    /// For Mixed pages: regions that need OCR handoff.
    pub ocr_regions: Vec<BoundingBox>,
}

/// Thresholds loaded from `triage.toml`. Defaults match the committed config.
#[derive(Debug, Clone)]
pub struct TriageConfig {
    pub text_native_min_text_cov: f32,
    pub text_native_max_image_cov: f32,
    pub scanned_max_text_cov: f32,
    pub scanned_min_image_cov: f32,
    pub scanned_page_image_ratio: f32,
    pub font_reliability_good: f32,
    pub font_reliability_bad: f32,
    pub invisible_text_threshold: f32,
    pub ocr_underlay_threshold: f32,
    pub garbled_threshold: f32,
    pub conf_high: f32,
    pub conf_medium: f32,
    pub conf_low: f32,
}

impl Default for TriageConfig {
    fn default() -> Self {
        Self {
            text_native_min_text_cov: 0.20,
            text_native_max_image_cov: 0.10,
            scanned_max_text_cov: 0.05,
            scanned_min_image_cov: 0.60,
            scanned_page_image_ratio: 1.0,
            font_reliability_good: 0.90,
            font_reliability_bad: 0.50,
            invisible_text_threshold: 0.50,
            ocr_underlay_threshold: 0.50,
            garbled_threshold: 0.30,
            conf_high: 0.85,
            conf_medium: 0.65,
            conf_low: 0.50,
        }
    }
}

/// Affine matrix stored as PDF row form (a b c d e f) representing
///   [ a c e ]
///   [ b d f ]
///   [ 0 0 1 ]
#[derive(Debug, Clone, Copy)]
struct Ctm(f64, f64, f64, f64, f64, f64);

impl Ctm {
    const IDENT: Ctm = Ctm(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);

    /// Left-multiply: new = other × self (PDF `cm` semantics).
    fn premul(self, o: Ctm) -> Ctm {
        let (a1, b1, c1, d1, e1, f1) = (o.0, o.1, o.2, o.3, o.4, o.5);
        let (a2, b2, c2, d2, e2, f2) = (self.0, self.1, self.2, self.3, self.4, self.5);
        Ctm(
            a1 * a2 + b1 * c2,
            a1 * b2 + b1 * d2,
            c1 * a2 + d1 * c2,
            c1 * b2 + d1 * d2,
            e1 * a2 + f1 * c2 + e2,
            e1 * b2 + f1 * d2 + f2,
        )
    }

    fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (self.0 * x + self.2 * y + self.4, self.1 * x + self.3 * y + self.5)
    }
}

/// Raw signals collected from a single content-stream walk. One pass —
/// text bboxes, image placements, invisible-text presence, and font usage
/// are all harvested together so Stage 1 and the cheap parts of Stage 2
/// share work.
#[derive(Debug, Default)]
struct PageSignals {
    text_bboxes: Vec<BoundingBox>,
    image_bboxes: Vec<BoundingBox>,
    /// Proportion of text-show operators with rendering mode 3 (invisible).
    invisible_text_ratio: f32,
    text_show_count: usize,
    /// Distinct font resource names referenced on this page (Tf operands).
    fonts_used: Vec<String>,
    /// Raw show-operator byte payloads (for CJK + underlay pattern analysis).
    /// We keep bytes, not decoded text, because reliable decoding requires
    /// the ToUnicode CMap for each font — that's part of the font-reliability
    /// check we're about to run, not a precondition for it.
    show_payloads: Vec<Vec<u8>>,
}

fn read_num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(n) => Some(*n as f64),
        Object::Real(n) => Some(*n as f64),
        _ => None,
    }
}

/// Concatenate the string payloads out of a Tj/TJ operand.
/// TJ takes an array of mixed strings and kerning numbers; we keep only strings.
fn collect_show_bytes(o: &Object) -> Option<Vec<u8>> {
    match o {
        Object::String(bytes, _) => Some(bytes.clone()),
        Object::Array(items) => {
            let mut out = Vec::new();
            for x in items {
                if let Object::String(b, _) = x {
                    out.extend_from_slice(b);
                }
            }
            if out.is_empty() { None } else { Some(out) }
        }
        _ => None,
    }
}

/// Count glyphs in a Tj/TJ operand for text-bbox width estimation.
fn count_glyphs(o: &Object) -> usize {
    match o {
        Object::String(bytes, _) => bytes.len().max(1),
        Object::Array(items) => items
            .iter()
            .filter_map(|x| match x {
                Object::String(b, _) => Some(b.len()),
                _ => None,
            })
            .sum::<usize>()
            .max(1),
        _ => 1,
    }
}

/// Walk the page content stream once, collecting signals used by Stages 1–2.
fn walk_page(doc: &Document, page_id: ObjectId) -> PageSignals {
    let mut sig = PageSignals::default();
    let content_bytes = match doc.get_page_content(page_id) {
        Ok(c) => c,
        Err(_) => return sig,
    };
    let content = match Content::decode(&content_bytes) {
        Ok(c) => c,
        Err(_) => return sig,
    };

    let mut ctm = Ctm::IDENT;
    let mut ctm_stack: Vec<Ctm> = Vec::new();
    let mut tm = Ctm::IDENT;
    let mut tx = 0.0;
    let mut ty = 0.0;
    let mut leading = 0.0;
    let mut font_size: f64 = 12.0;
    let mut text_render_mode: i64 = 0;
    let mut invisible_shows: usize = 0;
    let mut in_text = false;

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(prev) = ctm_stack.pop() {
                    ctm = prev;
                }
            }
            "cm" => {
                if op.operands.len() >= 6 {
                    let v: Vec<f64> = op.operands.iter().take(6).map(|o| read_num(o).unwrap_or(0.0)).collect();
                    ctm = ctm.premul(Ctm(v[0], v[1], v[2], v[3], v[4], v[5]));
                }
            }
            "BT" => {
                in_text = true;
                tm = Ctm::IDENT;
                tx = 0.0;
                ty = 0.0;
            }
            "ET" => {
                in_text = false;
            }
            "Tr" => {
                if let Some(n) = op.operands.first().and_then(read_num) {
                    text_render_mode = n as i64;
                }
            }
            "Tf" => {
                if let Some(Object::Name(n)) = op.operands.first() {
                    let name = String::from_utf8_lossy(n).to_string();
                    if !sig.fonts_used.contains(&name) {
                        sig.fonts_used.push(name);
                    }
                }
                if let Some(sz) = op.operands.get(1).and_then(read_num) {
                    font_size = sz;
                }
            }
            "TL" => {
                if let Some(l) = op.operands.first().and_then(read_num) {
                    leading = l;
                }
            }
            "Tm" if in_text => {
                if op.operands.len() >= 6 {
                    let v: Vec<f64> = op.operands.iter().take(6).map(|o| read_num(o).unwrap_or(0.0)).collect();
                    tm = Ctm(v[0], v[1], v[2], v[3], v[4], v[5]);
                    tx = 0.0;
                    ty = 0.0;
                }
            }
            "Td" if in_text => {
                if let (Some(dx), Some(dy)) = (
                    op.operands.first().and_then(read_num),
                    op.operands.get(1).and_then(read_num),
                ) {
                    tx += dx;
                    ty += dy;
                }
            }
            "TD" if in_text => {
                if let (Some(dx), Some(dy)) = (
                    op.operands.first().and_then(read_num),
                    op.operands.get(1).and_then(read_num),
                ) {
                    tx += dx;
                    ty += dy;
                    leading = -dy;
                }
            }
            "T*" if in_text => {
                ty -= leading;
            }
            op_name @ ("Tj" | "TJ" | "'" | "\"") if in_text => {
                let operand_idx = if op_name == "\"" { 2 } else { 0 };
                if op_name == "'" || op_name == "\"" {
                    ty -= leading;
                }
                let operand = match op.operands.get(operand_idx) {
                    Some(o) => o,
                    None => continue,
                };
                let glyphs = count_glyphs(operand);
                if glyphs == 0 {
                    continue;
                }
                if let Some(bytes) = collect_show_bytes(operand) {
                    if !bytes.is_empty() {
                        sig.show_payloads.push(bytes);
                    }
                }
                sig.text_show_count += 1;
                if text_render_mode == 3 {
                    invisible_shows += 1;
                }

                // Width estimate: glyph count × 0.5em (rough average advance).
                let width_em = (glyphs as f64) * 0.5;
                let combined = tm.premul(ctm);
                let (x0, y0) = combined.apply(tx, ty);
                let (x1, y1) = combined.apply(tx + width_em * font_size, ty + font_size);
                let bx = x0.min(x1);
                let by = y0.min(y1);
                let bw = (x1 - x0).abs();
                let bh = (y1 - y0).abs().max(font_size.abs());
                sig.text_bboxes.push(BoundingBox { x: bx, y: by, width: bw, height: bh });
            }
            "Do" => {
                // Image or form XObject invocation. The CTM at this point
                // encodes the placement — the XObject is drawn into the unit
                // square [(0,0)–(1,1)] transformed by CTM. We only count this
                // as an image if the /Resources dict marks it as Image.
                let name = match op.operands.first() {
                    Some(Object::Name(n)) => String::from_utf8_lossy(n).to_string(),
                    _ => continue,
                };
                if !is_image_xobject(doc, page_id, &name) {
                    continue;
                }
                let (x0, y0) = ctm.apply(0.0, 0.0);
                let (x1, y1) = ctm.apply(1.0, 1.0);
                let bx = x0.min(x1);
                let by = y0.min(y1);
                let bw = (x1 - x0).abs();
                let bh = (y1 - y0).abs();
                sig.image_bboxes.push(BoundingBox { x: bx, y: by, width: bw, height: bh });
            }
            _ => {}
        }
    }

    sig.invisible_text_ratio = if sig.text_show_count > 0 {
        invisible_shows as f32 / sig.text_show_count as f32
    } else {
        0.0
    };
    sig
}

fn is_image_xobject(doc: &Document, page_id: ObjectId, name: &str) -> bool {
    let page = match doc.get_object(page_id).and_then(|o| o.as_dict()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let resources = match page.get(b"Resources") {
        Ok(Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_dict()).ok(),
        Ok(o) => o.as_dict().ok(),
        Err(_) => None,
    };
    let resources = match resources {
        Some(r) => r,
        None => return false,
    };
    let xobjects = match resources.get(b"XObject") {
        Ok(Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_dict()).ok(),
        Ok(o) => o.as_dict().ok(),
        Err(_) => None,
    };
    let xobjects = match xobjects {
        Some(x) => x,
        None => return false,
    };
    let xref = xobjects.get(name.as_bytes()).ok();
    let stream_obj = match xref {
        Some(Object::Reference(id)) => doc.get_object(*id).ok(),
        Some(o) => Some(o),
        None => None,
    };
    match stream_obj.and_then(|o| o.as_stream().ok()) {
        Some(s) => s
            .dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| n == b"Image")
            .unwrap_or(false),
        None => false,
    }
}

/// Union area of a set of bboxes (overlap-corrected via sweep/grid).
/// Uses a simple 20×20 grid approximation — adequate for coverage ratios;
/// the real goal is "does text occupy most of the page" not exact geometry.
fn union_area_ratio(bboxes: &[BoundingBox], page_w: f64, page_h: f64) -> f32 {
    if page_w <= 0.0 || page_h <= 0.0 || bboxes.is_empty() {
        return 0.0;
    }
    const GRID: usize = 20;
    let cell_w = page_w / GRID as f64;
    let cell_h = page_h / GRID as f64;
    let mut hits = [[false; GRID]; GRID];
    for b in bboxes {
        let x0 = (b.x / cell_w).floor().clamp(0.0, GRID as f64 - 1.0) as usize;
        let y0 = (b.y / cell_h).floor().clamp(0.0, GRID as f64 - 1.0) as usize;
        let x1 = ((b.x + b.width) / cell_w).ceil().clamp(0.0, GRID as f64) as usize;
        let y1 = ((b.y + b.height) / cell_h).ceil().clamp(0.0, GRID as f64) as usize;
        for r in y0..y1 {
            for c in x0..x1 {
                hits[r][c] = true;
            }
        }
    }
    let count: usize = hits.iter().flatten().filter(|x| **x).count();
    count as f32 / (GRID * GRID) as f32
}

/// Fraction of `text_bboxes` whose center lies inside any image bbox.
/// High → OCR underlay (invisible text positioned on top of a page-size scan).
fn ocr_underlay_ratio(texts: &[BoundingBox], images: &[BoundingBox]) -> f32 {
    if texts.is_empty() || images.is_empty() {
        return 0.0;
    }
    let contained = texts
        .iter()
        .filter(|t| {
            let cx = t.x + t.width * 0.5;
            let cy = t.y + t.height * 0.5;
            images.iter().any(|im| {
                cx >= im.x && cx <= im.x + im.width && cy >= im.y && cy <= im.y + im.height
            })
        })
        .count();
    contained as f32 / texts.len() as f32
}

/// Font reliability = (fonts with /ToUnicode CMap) / (distinct fonts used on page).
/// Missing ToUnicode + CID-encoded fonts → glyphs can't be reliably mapped to
/// Unicode → extraction produces garbage even if the content stream is intact.
fn font_reliability(doc: &Document, page_id: ObjectId, fonts_used: &[String]) -> Option<f32> {
    if fonts_used.is_empty() {
        return None;
    }
    let page = doc.get_object(page_id).ok()?.as_dict().ok()?;
    let resources = match page.get(b"Resources").ok()? {
        Object::Reference(id) => doc.get_object(*id).and_then(|o| o.as_dict()).ok()?,
        o => o.as_dict().ok()?,
    };
    let fonts_dict = match resources.get(b"Font") {
        Ok(Object::Reference(id)) => doc.get_object(*id).and_then(|o| o.as_dict()).ok()?,
        Ok(o) => o.as_dict().ok()?,
        Err(_) => return Some(0.0),
    };
    let mut reliable = 0usize;
    for name in fonts_used {
        let key = name.trim_start_matches('/').as_bytes();
        let font_obj = match fonts_dict.get(key) {
            Ok(Object::Reference(id)) => doc.get_object(*id).ok(),
            Ok(o) => Some(o),
            Err(_) => None,
        };
        let font_dict = match font_obj.and_then(|o| o.as_dict().ok()) {
            Some(d) => d,
            None => continue,
        };
        if font_dict.get(b"ToUnicode").is_ok() {
            reliable += 1;
        }
    }
    Some(reliable as f32 / fonts_used.len() as f32)
}

/// Stage 3: text-layer garble ratio. Looks at a decoded sample of the page
/// content for characters that indicate the glyph→unicode map is broken:
///   - U+FFFD replacement characters (emitted when decoding fails)
///   - control codes outside standard whitespace (\t \n \r space)
///   - Private Use Area code points (U+E000..U+F8FF) — common when a
///     custom font maps glyphs to PUA without a ToUnicode CMap
///
/// Returns the fraction of characters that match one of the above.
fn garbled_text_ratio(payloads: &[Vec<u8>]) -> f32 {
    let mut total: usize = 0;
    let mut bad: usize = 0;
    for p in payloads {
        // Decode as UTF-8 losslessly; invalid sequences become U+FFFD which
        // we already count as garbled.
        let s = String::from_utf8_lossy(p);
        for ch in s.chars() {
            total += 1;
            let code = ch as u32;
            let is_control = (code < 0x20 && !matches!(ch, '\t' | '\n' | '\r'))
                || code == 0x7F;
            let is_pua = (0xE000..=0xF8FF).contains(&code);
            let is_replacement = ch == '\u{FFFD}';
            if is_control || is_pua || is_replacement {
                bad += 1;
            }
        }
    }
    if total == 0 {
        return 0.0;
    }
    bad as f32 / total as f32
}

/// CJK detection: scan raw show-payload bytes for high-bit patterns typical
/// of CJK CID encodings. We don't decode (no CMap access here); we treat
/// payloads with ≥30% bytes in the 0x80–0xFF range as "likely CJK".
///
/// This is a heuristic — a page using a 1-byte Latin font will almost never
/// exceed 10% high bytes, while CJK CID-encoded text is typically 2-byte with
/// at least one high byte per glyph (50%+ ratio). The 30% cutoff leaves
/// headroom for mixed-language pages.
fn detect_cjk(payloads: &[Vec<u8>]) -> bool {
    let mut total = 0usize;
    let mut high = 0usize;
    for p in payloads {
        total += p.len();
        high += p.iter().filter(|b| **b >= 0x80).count();
    }
    if total < 16 {
        return false;
    }
    (high as f32 / total as f32) >= 0.30
}

/// Stage 2: escalated signals, invoked only when Stage 1 returned Unknown.
/// Updates `triage` in place; reuses the cached `sig` from the single walk.
fn apply_stage2(
    triage: &mut PageTriage,
    sig: &PageSignals,
    doc: &Document,
    page_id: ObjectId,
    cfg: &TriageConfig,
) {
    let font_rel = font_reliability(doc, page_id, &sig.fonts_used);
    let has_invis = if sig.text_show_count > 0 {
        Some(sig.invisible_text_ratio >= cfg.invisible_text_threshold)
    } else {
        None
    };
    let cjk = Some(detect_cjk(&sig.show_payloads));
    let underlay = Some(ocr_underlay_ratio(&sig.text_bboxes, &sig.image_bboxes));
    let garbled = Some(garbled_text_ratio(&sig.show_payloads));

    triage.font_reliability = font_rel;
    triage.has_invisible_text = has_invis;
    triage.contains_cjk = cjk;
    triage.ocr_underlay_ratio = underlay;
    triage.garbled_ratio = garbled;

    // OCR underlay overrides Stage 1: if most text is pasted on top of a scan,
    // the page is scanned no matter what text_coverage said.
    if let Some(r) = underlay {
        if r >= cfg.ocr_underlay_threshold && triage.image_coverage >= 0.30 {
            triage.category = PdfCategory::Scanned;
            triage.confidence = 0.87;
            triage.ocr_regions = sig.image_bboxes.clone();
            return;
        }
    }

    // Invisible text (Tr=3) on an image-heavy page is another OCR-underlay
    // signature (less common, but seen in some scanner software).
    if has_invis == Some(true) && triage.image_coverage >= 0.30 {
        triage.category = PdfCategory::Scanned;
        triage.confidence = 0.82;
        triage.ocr_regions = sig.image_bboxes.clone();
        return;
    }

    // Unreliable fonts + image content → OCR underlay pattern → Scanned.
    // Unreliable fonts WITHOUT images → probably text-native with missing
    // ToUnicode (common in synthetic/benchmark PDFs); rasterizing won't help
    // since there is nothing to OCR, so we keep the Stage 1 verdict.
    if let Some(fr) = font_rel {
        if fr <= cfg.font_reliability_bad
            && triage.text_coverage >= 0.05
            && triage.image_coverage >= 0.10
        {
            triage.category = PdfCategory::Scanned;
            triage.confidence = 0.75;
            triage.ocr_regions = sig.image_bboxes.clone();
            return;
        }
    }

    // Stage 3: garbled text layer. Even when fonts report ToUnicode and the
    // page has no image backdrop, the decoded characters can be unusable
    // (PUA glyph codes, replacement chars, control bytes). In that case
    // the text-native extractor will emit garbage, so prefer OCR.
    if let Some(g) = garbled {
        if g >= cfg.garbled_threshold && sig.text_show_count >= 20 {
            triage.category = PdfCategory::Scanned;
            triage.confidence = 0.72;
            triage.ocr_regions = if sig.image_bboxes.is_empty() {
                Vec::new()
            } else {
                sig.image_bboxes.clone()
            };
            return;
        }
    }

    // Otherwise: Stage 1 was borderline but Stage 2 finds nothing alarming —
    // promote to TextNative with reduced confidence.
    if triage.category == PdfCategory::Unknown {
        triage.category = PdfCategory::TextNative;
        triage.confidence = cfg.conf_medium;
    }
}

/// Run Stage 1, then escalate to Stage 2 when Stage 1 returned Unknown.
/// Uses a single content-stream walk for both stages.
pub fn classify_page(
    doc: &Document,
    page_num: usize,
    page_id: ObjectId,
    cfg: &TriageConfig,
) -> PageTriage {
    let (page_w, page_h) = page_media_box_wh(doc, page_id).unwrap_or((612.0, 792.0));
    let sig = walk_page(doc, page_id);

    let text_coverage = union_area_ratio(&sig.text_bboxes, page_w, page_h);
    let image_coverage = union_area_ratio(&sig.image_bboxes, page_w, page_h);
    let image_count = sig.image_bboxes.len();
    let (category, confidence) = classify_stage1(text_coverage, image_coverage, cfg);

    let mut triage = PageTriage {
        page: page_num,
        category,
        confidence,
        text_coverage,
        image_coverage,
        image_count,
        font_reliability: None,
        has_invisible_text: None,
        contains_cjk: None,
        ocr_underlay_ratio: None,
        ocr_iou: None,
        garbled_ratio: None,
        ocr_regions: if category == PdfCategory::Mixed {
            sig.image_bboxes.clone()
        } else {
            Vec::new()
        },
    };

    // Always run Stage 2 when we have image presence + any text — even if
    // Stage 1 committed to TextNative or Scanned — because OCR-underlay can
    // masquerade as either. Skip Stage 2 only when signals are boring
    // (pure text, no images) to keep the hot path fast.
    let needs_stage2 = triage.category == PdfCategory::Unknown
        || (!sig.image_bboxes.is_empty() && sig.text_show_count > 0);
    if needs_stage2 {
        apply_stage2(&mut triage, &sig, doc, page_id, cfg);
    }
    triage
}

/// Stage 1 only — kept for callers that want the raw fast-path signal.
pub fn stage1(
    doc: &Document,
    page_num: usize,
    page_id: ObjectId,
    cfg: &TriageConfig,
) -> PageTriage {
    let (page_w, page_h) = page_media_box_wh(doc, page_id).unwrap_or((612.0, 792.0));
    let sig = walk_page(doc, page_id);
    let text_coverage = union_area_ratio(&sig.text_bboxes, page_w, page_h);
    let image_coverage = union_area_ratio(&sig.image_bboxes, page_w, page_h);
    let image_count = sig.image_bboxes.len();
    let (category, confidence) = classify_stage1(text_coverage, image_coverage, cfg);
    PageTriage {
        page: page_num,
        category,
        confidence,
        text_coverage,
        image_coverage,
        image_count,
        font_reliability: None,
        has_invisible_text: None,
        contains_cjk: None,
        ocr_underlay_ratio: None,
        ocr_iou: None,
        garbled_ratio: None,
        ocr_regions: if category == PdfCategory::Mixed {
            sig.image_bboxes
        } else {
            Vec::new()
        },
    }
}

/// `MediaBox` is an inheritable page attribute (PDF 32000-1 §7.7.3.4) — many
/// valid documents set it once on a `Pages` node and never repeat it on leaf
/// pages. Walk `Parent` until a `MediaBox` is found; a cycle guard covers
/// malformed documents with a `Parent` loop.
pub(crate) fn page_media_box_wh(doc: &Document, page_id: ObjectId) -> Option<(f64, f64)> {
    let mut current = page_id;
    let mut visited = std::collections::HashSet::new();
    loop {
        if !visited.insert(current) {
            return None;
        }
        let dict = doc.get_object(current).ok()?.as_dict().ok()?;
        if let Ok(mb) = dict.get(b"MediaBox").and_then(Object::as_array) {
            if mb.len() >= 4 {
                let llx = read_num(&mb[0])?;
                let lly = read_num(&mb[1])?;
                let urx = read_num(&mb[2])?;
                let ury = read_num(&mb[3])?;
                return Some(((urx - llx).abs(), (ury - lly).abs()));
            }
        }
        match dict.get(b"Parent") {
            Ok(Object::Reference(parent_id)) => current = *parent_id,
            _ => return None,
        }
    }
}

fn classify_stage1(text_cov: f32, image_cov: f32, cfg: &TriageConfig) -> (PdfCategory, f32) {
    if text_cov >= cfg.text_native_min_text_cov && image_cov <= cfg.text_native_max_image_cov {
        return (PdfCategory::TextNative, 0.90);
    }
    if text_cov <= cfg.scanned_max_text_cov && image_cov >= cfg.scanned_min_image_cov {
        return (PdfCategory::Scanned, 0.88);
    }
    if text_cov > cfg.scanned_max_text_cov && image_cov > cfg.text_native_max_image_cov {
        return (PdfCategory::Mixed, 0.70);
    }
    (PdfCategory::Unknown, 0.40)
}

/// Triage every page in a document. Runs Stage 1 + conditional Stage 2.
pub fn classify_document(doc: &Document, cfg: &TriageConfig) -> Vec<PageTriage> {
    doc.get_pages()
        .into_iter()
        .map(|(page_num, page_id)| classify_page(doc, page_num as usize, page_id, cfg))
        .collect()
}

/// OCR routing manifest — consumed by the Python bridge.
/// See `plan/pdf-triage.md` §OCR Routing Contract.
#[derive(Debug, Serialize)]
pub struct OcrRoutingManifest {
    /// Original document path (caller-supplied, not validated here).
    pub document: String,
    /// Total page count for quick scanning.
    pub page_count: usize,
    /// Per-page routing decision.
    pub pages: Vec<OcrRoutingEntry>,
}

#[derive(Debug, Serialize)]
pub struct OcrRoutingEntry {
    pub page: usize,
    pub category: PdfCategory,
    pub confidence: f32,
    /// Full-page rasterization required (Scanned pages).
    pub needs_full_page_ocr: bool,
    /// Region-level OCR targets (Mixed pages).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ocr_regions: Vec<BoundingBox>,
    /// True when CJK content was detected — lets the bridge pick a CJK-aware
    /// OCR engine (tesseract -l kor+jpn+chi_sim, or a VLM with CJK prompt).
    pub cjk_hint: bool,
}

/// Build an OCR routing manifest from triage results.
pub fn build_manifest(document: &str, triage: &[PageTriage]) -> OcrRoutingManifest {
    let pages = triage
        .iter()
        .map(|t| OcrRoutingEntry {
            page: t.page,
            category: t.category,
            confidence: t.confidence,
            needs_full_page_ocr: matches!(t.category, PdfCategory::Scanned),
            ocr_regions: t.ocr_regions.clone(),
            cjk_hint: t.contains_cjk.unwrap_or(false),
        })
        .collect::<Vec<_>>();
    OcrRoutingManifest {
        document: document.to_string(),
        page_count: pages.len(),
        pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    #[test]
    fn page_media_box_wh_walks_parent_inheritance() {
        // MediaBox is an inheritable attribute (PDF 32000-1 §7.7.3.4) — set
        // only on the Pages ancestor, never repeated on the leaf Page. Many
        // real-world PDFs are authored this way.
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "MediaBox" => vec![0.into(), 0.into(), 600.into(), 800.into()],
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let wh = page_media_box_wh(&doc, page_id);
        assert_eq!(wh, Some((600.0, 800.0)));
    }

    #[test]
    fn page_media_box_wh_parent_cycle_returns_none_not_infinite_loop() {
        let mut doc = Document::with_version("1.5");
        let page_id = doc.new_object_id();
        // Parent points back to itself — malformed, must terminate via the
        // visited-set guard instead of looping forever.
        doc.objects.insert(page_id, Object::Dictionary(dictionary! { "Type" => "Page", "Parent" => page_id }));
        assert_eq!(page_media_box_wh(&doc, page_id), None);
    }

    #[test]
    fn default_config_matches_toml() {
        let c = TriageConfig::default();
        assert_eq!(c.text_native_min_text_cov, 0.20);
        assert_eq!(c.scanned_min_image_cov, 0.60);
    }

    #[test]
    fn classify_text_native() {
        let c = TriageConfig::default();
        let (cat, conf) = classify_stage1(0.30, 0.05, &c);
        assert_eq!(cat, PdfCategory::TextNative);
        assert!(conf >= 0.85);
    }

    #[test]
    fn classify_scanned() {
        let c = TriageConfig::default();
        let (cat, _) = classify_stage1(0.01, 0.85, &c);
        assert_eq!(cat, PdfCategory::Scanned);
    }

    #[test]
    fn classify_mixed() {
        let c = TriageConfig::default();
        let (cat, _) = classify_stage1(0.15, 0.40, &c);
        assert_eq!(cat, PdfCategory::Mixed);
    }

    #[test]
    fn classify_unknown_when_borderline() {
        let c = TriageConfig::default();
        let (cat, _) = classify_stage1(0.08, 0.05, &c);
        assert_eq!(cat, PdfCategory::Unknown);
    }

    #[test]
    fn ctm_premul_identity() {
        let m = Ctm(2.0, 0.0, 0.0, 3.0, 10.0, 20.0);
        let r = m.premul(Ctm::IDENT);
        assert_eq!(r.0, 2.0);
        assert_eq!(r.3, 3.0);
        assert_eq!(r.4, 10.0);
        assert_eq!(r.5, 20.0);
    }

    #[test]
    fn ctm_apply_translate_scale() {
        let m = Ctm(2.0, 0.0, 0.0, 3.0, 5.0, 7.0);
        let (x, y) = m.apply(1.0, 1.0);
        assert_eq!(x, 2.0 * 1.0 + 5.0);
        assert_eq!(y, 3.0 * 1.0 + 7.0);
    }

    #[test]
    fn union_area_empty() {
        assert_eq!(union_area_ratio(&[], 612.0, 792.0), 0.0);
    }

    #[test]
    fn union_area_full_page() {
        let b = BoundingBox { x: 0.0, y: 0.0, width: 612.0, height: 792.0 };
        assert!(union_area_ratio(&[b], 612.0, 792.0) > 0.99);
    }

    #[test]
    fn union_area_half_page() {
        let b = BoundingBox { x: 0.0, y: 0.0, width: 612.0, height: 396.0 };
        let r = union_area_ratio(&[b], 612.0, 792.0);
        assert!((r - 0.5).abs() < 0.05);
    }

    #[test]
    fn ocr_underlay_detects_contained_text() {
        let img = BoundingBox { x: 0.0, y: 0.0, width: 500.0, height: 700.0 };
        let texts = vec![
            BoundingBox { x: 100.0, y: 100.0, width: 200.0, height: 20.0 },
            BoundingBox { x: 100.0, y: 300.0, width: 200.0, height: 20.0 },
            BoundingBox { x: 600.0, y: 100.0, width: 100.0, height: 20.0 }, // outside
        ];
        let r = ocr_underlay_ratio(&texts, &[img]);
        assert!((r - 2.0 / 3.0).abs() < 1e-3);
    }

    #[test]
    fn ocr_underlay_empty_inputs() {
        assert_eq!(ocr_underlay_ratio(&[], &[]), 0.0);
        let img = BoundingBox { x: 0.0, y: 0.0, width: 500.0, height: 700.0 };
        assert_eq!(ocr_underlay_ratio(&[], &[img]), 0.0);
    }

    #[test]
    fn cjk_detection_high_bytes() {
        // Korean CID encoding: at least one high byte per 2-byte glyph cluster.
        // Min payload size is 16 bytes — pad with realistic CID pattern.
        let payload: Vec<u8> = (0..20).map(|i| if i % 2 == 0 { 0xAB } else { 0x40 }).collect();
        assert!(detect_cjk(&[payload]));
    }

    #[test]
    fn cjk_detection_latin() {
        let payload = b"Hello, this is Latin text.".to_vec();
        assert!(!detect_cjk(&[payload]));
    }

    #[test]
    fn cjk_detection_too_short_payload() {
        let payload = vec![0xFF, 0xFF];
        assert!(!detect_cjk(&[payload]));
    }

    #[test]
    fn garbled_ratio_detects_replacement_chars() {
        // U+FFFD in UTF-8 is EF BF BD
        let payload = b"Hello\xEF\xBF\xBD\xEF\xBF\xBD\xEF\xBF\xBDWorld".to_vec();
        let r = garbled_text_ratio(&[payload]);
        // 3 replacement chars out of 13 total chars (Hello + 3*replacement + World)
        assert!(r > 0.2 && r < 0.3, "garbled ratio {}", r);
    }

    #[test]
    fn garbled_ratio_detects_pua() {
        // U+E000 in UTF-8 is EE 80 80 — Private Use Area start
        let mut payload = b"text".to_vec();
        for _ in 0..6 {
            payload.extend_from_slice(&[0xEE, 0x80, 0x80]);
        }
        let r = garbled_text_ratio(&[payload]);
        // 6 PUA chars out of 10 total (t,e,x,t + 6 PUA)
        assert!(r > 0.5 && r <= 0.7, "pua ratio {}", r);
    }

    #[test]
    fn garbled_ratio_clean_text() {
        let payload = b"Hello world, normal text.".to_vec();
        let r = garbled_text_ratio(&[payload]);
        assert_eq!(r, 0.0);
    }

    #[test]
    fn garbled_ratio_empty() {
        assert_eq!(garbled_text_ratio(&[]), 0.0);
    }
}
