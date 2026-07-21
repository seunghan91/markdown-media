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
//! # Korean/CJK fonts
//!
//! Korean text renders when a CJK-capable TrueType/OpenType font is
//! available at render time: it is parsed with [`printpdf::ParsedFont`],
//! registered via [`PdfDocument::add_font`], and selected with
//! `PdfFontHandle::External` so `printpdf` subsets and embeds it (see
//! [`resolve_font_bytes`] for the search strategy). When no font resolves,
//! this path degrades gracefully to the 14 Latin-1 [`BuiltinFont`]s — Latin
//! text still renders, Hangul comes out as `?` — without panicking.
//!
//! Font resolution order (first hit wins):
//!   1. `MDM_PDF_FONT` env var — an explicit font file path, authoritative
//!      (no system search; used as-is or falls straight through to Latin).
//!   2. System search over an **open-license allowlist** ([`OPEN_CJK_FONTS`]:
//!      Nanum / Noto CJK-KR / Source Han, all SIL OFL 1.1) across
//!      `$CARGO_MANIFEST_DIR/assets/fonts` (gitignored), `$MDM_PDF_FONT_DIR`,
//!      user font dirs, and the standard macOS/Linux font directories.
//!   3. With `MDM_PDF_ALLOW_SYSTEM_FONTS` set, the allowlist is dropped and
//!      the first Hangul-covering system font (e.g. Apple SD Gothic Neo) is
//!      embedded — **opt-in** because embedding proprietary system fonts into
//!      a redistributed PDF is a licensing decision the caller must own; a
//!      warning naming the font is logged.
//!
//! We deliberately do **not** auto-embed proprietary system fonts by default,
//! and never commit font binaries to the repo (`assets/` is gitignored); an
//! optional `scripts/download-fonts.sh` fetches Nanum Gothic (OFL) locally.
//!
//! # Known limitations (best-effort, see the porting brief's "한계는
//! 보고서에 명시" requirement)
//!
//! - **Bold/italic/monospace collapse under an embedded font.** When a single
//!   CJK font is embedded, all four logical roles (regular/bold/italic/mono)
//!   map to it — headings lose their bold weight and table cells lose the
//!   Courier monospace, because `BuiltinFont::Courier` cannot render Hangul.
//!   The Latin fallback keeps the four distinct builtins.
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

use std::path::{Path, PathBuf};
use std::{env, fs, io};

use printpdf::{
    BuiltinFont, Color, FontId, Line, LinePoint, Mm, Op, ParsedFont, PdfDocument, PdfFontHandle,
    PdfPage, PdfSaveOptions, Point, Pt, Rgb, TextItem, TextMatrix,
};

use crate::ir::IRBlock;

use super::renderer::{default_margin, Orientation, PageSize, RenderOptions};

const GRAY: (f32, f32, f32) = (0.55, 0.55, 0.55);
const BLACK: (f32, f32, f32) = (0.05, 0.05, 0.05);

/// Filename fragments of the open-license CJK fonts we are willing to embed
/// automatically. All are SIL Open Font License 1.1: Nanum (Naver),
/// Noto Sans/Serif CJK-KR (Google), Source Han Sans/Serif (Adobe). Matching
/// is by substring so weight/style variants (`NanumGothicBold`, …) also hit.
/// Proprietary system fonts are intentionally excluded here (see the module
/// doc comment's licensing note); `MDM_PDF_ALLOW_SYSTEM_FONTS` overrides.
const OPEN_CJK_FONTS: &[&str] = &[
    "NanumGothic",
    "NanumMyeongjo",
    "NanumBarunGothic",
    "NanumSquare",
    "NotoSansKR",
    "NotoSansCJK",
    "NotoSerifKR",
    "NotoSerifCJK",
    "SourceHanSans",
    "SourceHanSerif",
];

/// A resolved font for one logical text role: either one of the 14 builtin
/// Latin-1 fonts, or an embedded font registered on the [`PdfDocument`].
#[derive(Clone)]
enum FontRef {
    Builtin(BuiltinFont),
    Embedded(FontId),
}

impl FontRef {
    fn handle(&self) -> PdfFontHandle {
        match self {
            FontRef::Builtin(f) => PdfFontHandle::Builtin(*f),
            FontRef::Embedded(id) => PdfFontHandle::External(id.clone()),
        }
    }
}

/// The four logical text roles this renderer draws with. When a CJK font is
/// embedded, every role points at the same embedded handle (bold/italic/mono
/// distinctions are lost — see the module doc comment); otherwise each role
/// keeps its distinct [`BuiltinFont`].
struct FontSet {
    regular: FontRef,
    bold: FontRef,
    italic: FontRef,
    mono: FontRef,
}

impl FontSet {
    /// The Latin-1 builtin fallback: no embedding, four distinct fonts.
    fn latin_fallback() -> Self {
        FontSet {
            regular: FontRef::Builtin(BuiltinFont::Helvetica),
            bold: FontRef::Builtin(BuiltinFont::HelveticaBold),
            italic: FontRef::Builtin(BuiltinFont::HelveticaOblique),
            mono: FontRef::Builtin(BuiltinFont::Courier),
        }
    }
}

/// Directories searched for CJK fonts, in priority order. Bundled/downloaded
/// fonts under the crate's (gitignored) `assets/fonts` win over system ones.
fn font_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts")];
    if let Some(dir) = env::var_os("MDM_PDF_FONT_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(home) = env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join("Library/Fonts")); // macOS user
        dirs.push(home.join(".fonts")); // Linux user (legacy)
        dirs.push(home.join(".local/share/fonts")); // Linux user (XDG)
    }
    for p in [
        "/Library/Fonts",
        "/System/Library/Fonts",
        "/System/Library/Fonts/Supplemental",
        "/usr/share/fonts",
        "/usr/local/share/fonts",
    ] {
        dirs.push(PathBuf::from(p));
    }
    dirs
}

/// Collects `*.ttf|otf|ttc` files under `dir` up to `max_depth` levels deep
/// (Linux packagers nest fonts under `truetype/nanum/…`). Bounded so a huge
/// system font tree can't turn resolution into a full-disk walk.
fn collect_font_files(dir: &Path, max_depth: u32, out: &mut Vec<PathBuf>) {
    if max_depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_font_files(&path, max_depth - 1, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc") {
                out.push(path);
            }
        }
    }
}

/// Resolves the bytes of a CJK-capable font to embed, or `None` to fall back
/// to Latin builtins. See the module doc comment for the full policy. Never
/// panics — every failure path logs and degrades. Returns `(display_name,
/// bytes)` so callers can log which font was chosen.
fn resolve_font_bytes() -> Option<(String, Vec<u8>)> {
    // 1. Explicit override wins and is authoritative — if it fails to read we
    //    go straight to Latin fallback rather than scanning the system, so a
    //    caller (or test) can force a deterministic outcome.
    if let Ok(p) = env::var("MDM_PDF_FONT") {
        let p = p.trim().to_string();
        if !p.is_empty() {
            return match fs::read(&p) {
                Ok(bytes) => Some((p, bytes)),
                Err(e) => {
                    eprintln!("[print-pdf] MDM_PDF_FONT '{p}' 읽기 실패 ({e}) — 라틴 폴백");
                    None
                }
            };
        }
    }

    let allow_system = env::var_os("MDM_PDF_ALLOW_SYSTEM_FONTS").is_some();
    for dir in font_search_dirs() {
        let mut files = Vec::new();
        collect_font_files(&dir, 3, &mut files);
        files.sort();
        for path in files {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_open = OPEN_CJK_FONTS.iter().any(|frag| name.contains(frag));
            if !is_open && !allow_system {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if is_open {
                return Some((path.display().to_string(), bytes));
            }
            // allow_system: only accept a font that actually covers Hangul
            // (U+AC00 '가'), and warn that its redistribution license is the
            // caller's responsibility.
            let mut warnings = Vec::new();
            if let Some(parsed) = ParsedFont::from_bytes(&bytes, 0, &mut warnings) {
                if parsed.lookup_glyph_index(0xAC00).is_some() {
                    eprintln!(
                        "[print-pdf] 시스템 폰트 '{}' 임베딩 — 재배포 라이선스는 호출자 책임 \
                         (MDM_PDF_ALLOW_SYSTEM_FONTS)",
                        path.display()
                    );
                    return Some((path.display().to_string(), bytes));
                }
            }
        }
    }
    None
}

/// Resolves fonts and registers any embedded font on `doc`, returning the
/// role→handle mapping the layout draws with. Falls back to Latin builtins
/// when no CJK font resolves or parsing fails — never panics.
fn build_font_set(doc: &mut PdfDocument) -> FontSet {
    let Some((name, bytes)) = resolve_font_bytes() else {
        return FontSet::latin_fallback();
    };
    let mut warnings = Vec::new();
    let Some(parsed) = ParsedFont::from_bytes(&bytes, 0, &mut warnings) else {
        eprintln!("[print-pdf] 폰트 '{name}' 파싱 실패 — 라틴 폴백");
        return FontSet::latin_fallback();
    };
    let font = FontRef::Embedded(doc.add_font(&parsed));
    FontSet {
        regular: font.clone(),
        bold: font.clone(),
        italic: font.clone(),
        mono: font,
    }
}

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
    fonts: FontSet,
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
        font: &FontRef,
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
                font: font.handle(),
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
                    self.text_line(page, pages, line, body, &self.fonts.regular, BLACK);
                }
                if let Some(url) = href {
                    self.text_line(page, pages, url, body * 0.85, &self.fonts.regular, GRAY);
                }
                if let Some(note) = footnote {
                    self.text_line(page, pages, note, body * 0.85, &self.fonts.italic, GRAY);
                }
            }
            IRBlock::Heading { level, text } => {
                let size = match level {
                    1 => h1,
                    2 => h2,
                    3 => h3,
                    _ => h_rest,
                };
                self.text_line(page, pages, text, size, &self.fonts.bold, BLACK);
            }
            IRBlock::Table(table) => {
                for row in &table.cells {
                    let line = row
                        .iter()
                        .map(|c| c.text.replace('\n', " "))
                        .collect::<Vec<_>>()
                        .join(" | ");
                    self.text_line(page, pages, &line, body * 0.85, &self.fonts.mono, BLACK);
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
                        &self.fonts.regular,
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
                    &self.fonts.italic,
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

    // Register any embedded CJK font before laying out, so the produced ops
    // can reference its `FontId`. `doc` owns the parsed font for the life of
    // the render.
    let mut doc = PdfDocument::new("MDM Print Document");
    let layout = Layout {
        content_left: margin_left,
        content_right: width_mm - margin_right,
        content_top: height_mm - margin_top,
        content_bottom: margin_bottom,
        sizes: preset_sizes(options.preset),
        fonts: build_font_set(&mut doc),
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
                font: layout.fonts.regular.handle(),
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
                font: layout.fonts.regular.handle(),
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
                font: layout.fonts.bold.handle(),
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

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// Probe candidate font files that may exist in this environment (open
    /// fonts first, then well-known system CJK fonts) so the embedded-path
    /// test can run wherever *some* usable font is present, and skip cleanly
    /// where none is (e.g. a bare Linux CI image).
    fn probe_existing_font() -> Option<String> {
        let candidates = [
            "/System/Library/Fonts/AppleSDGothicNeo.ttc", // macOS
            "/System/Library/Fonts/Supplemental/AppleGothic.ttf",
            "/usr/share/fonts/truetype/nanum/NanumGothic.ttf", // Debian/Ubuntu
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        ];
        candidates
            .iter()
            .find(|p| std::path::Path::new(p).exists())
            .map(|p| p.to_string())
    }

    /// Both halves of the font policy in one test to avoid `MDM_PDF_FONT`
    /// env races between parallel test threads.
    #[test]
    fn korean_pdf_embeds_font_or_falls_back_latin() {
        let blocks = vec![
            IRBlock::heading(1, "한글 제목"),
            IRBlock::paragraph("본문 단락 텍스트입니다."),
        ];

        // (b) Forced Latin fallback: a nonexistent override is authoritative,
        // so no system font is picked up regardless of the host. Must not
        // panic, must be a valid PDF, and must reference a builtin font.
        env::set_var("MDM_PDF_FONT", "/nonexistent/none.ttf");
        let bytes = pdf_bytes(&blocks, &RenderOptions::default());
        env::remove_var("MDM_PDF_FONT");
        assert!(bytes.starts_with(b"%PDF"));
        assert!(
            contains(&bytes, b"Helvetica"),
            "fallback PDF should reference a builtin font"
        );

        // (a) Embedded path: only when a real CJK font exists in this env.
        // Pointing MDM_PDF_FONT at it is authoritative (bypasses the
        // open-license allowlist — this is a test, the caller owns the
        // choice). Assert the font got embedded as a composite CID font.
        let Some(font_path) = probe_existing_font() else {
            eprintln!("[test] no system CJK font found — skipping embedded-path assertion");
            return;
        };
        env::set_var("MDM_PDF_FONT", &font_path);
        let bytes = pdf_bytes(&blocks, &RenderOptions::default());
        env::remove_var("MDM_PDF_FONT");
        assert!(bytes.starts_with(b"%PDF"));
        // printpdf subsets+embeds CJK as a Type0 composite font with a
        // CIDFontType2/CIDFontType0 descendant and an embedded FontFile.
        assert!(
            contains(&bytes, b"Type0")
                && (contains(&bytes, b"CIDFont") || contains(&bytes, b"FontFile")),
            "embedded PDF ({font_path}) should carry a Type0/CIDFont program"
        );
    }

    #[test]
    fn resolve_font_bytes_override_is_authoritative() {
        // A readable override returns those exact bytes; an unreadable one
        // returns None (no system search) rather than panicking.
        env::set_var("MDM_PDF_FONT", "/nonexistent/none.ttf");
        let resolved = resolve_font_bytes();
        env::remove_var("MDM_PDF_FONT");
        assert!(
            resolved.is_none(),
            "unreadable override must yield None (Latin fallback), got {:?}",
            resolved.map(|(n, _)| n)
        );
    }
}
