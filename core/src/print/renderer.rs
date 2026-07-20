//! Print-oriented HTML renderer for `IRBlock[]`.
//!
//! Ported from kkdoc (MIT): reference/kkdoc/src/print/renderer.ts
//!
//! kkdoc's `renderHtml()` converts `IRBlock[]` → Markdown (via
//! `blocksToMarkdown`) → HTML (via `markdown-it`), then wraps the result in
//! the preset `<style>` + watermark shell. This port renders `IRBlock`s to
//! HTML tags directly (no intermediate Markdown pass, no `markdown-it`
//! dependency) — same preset CSS, same watermark/extra-CSS shell, same
//! three presets (`default` / `gov-formal` / `compact`), but block → HTML
//! tag mapping lives here instead of round-tripping through Markdown.
//!
//! kkdoc's `header` / `footer` / `pageSize` / `orientation` / `margin`
//! options are consumed only by the puppeteer PDF stage (`htmlToPdf`) —
//! `renderHtml()` itself ignores them. This port has no puppeteer stage, so
//! `render_ir_to_html` folds them into the returned HTML directly: page
//! size/margin become an `@page` rule, and header/footer become
//! `position: fixed` bands. Both are best-effort — plain browsers' "print
//! to PDF" only loosely respects `@page` margins and does not repeat
//! `position: fixed` elements per page; a CSS Paged Media renderer
//! (WeasyPrint, Prince, PagedJS) is required for faithful per-page
//! repetition, matching kkdoc's own comment that `renderHtml()`'s output is
//! meant to combine with such external engines.

use crate::ir::IRBlock;

/// CSS preset. Mirrors kkdoc's `PrintPreset` union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrintPreset {
    #[default]
    Default,
    GovFormal,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PageSize {
    #[default]
    A4,
    Letter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Page margin as CSS length strings (e.g. `"20mm"`, `"1in"`). Mirrors
/// kkdoc's `PageMargin` (which additionally accepted bare numbers as
/// millimeters — [`PageMargin::mm`] covers that case here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMargin {
    pub top: String,
    pub right: String,
    pub bottom: String,
    pub left: String,
}

impl PageMargin {
    /// Same margin on all four sides.
    pub fn uniform<S: Into<String>>(v: S) -> Self {
        let v = v.into();
        Self {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }

    /// Millimeter shorthand, avoiding manual `format!("{n}mm")` at call sites.
    pub fn mm(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top: format!("{top}mm"),
            right: format!("{right}mm"),
            bottom: format!("{bottom}mm"),
            left: format!("{left}mm"),
        }
    }

    pub(crate) fn css_shorthand(&self) -> String {
        format!("{} {} {} {}", self.top, self.right, self.bottom, self.left)
    }
}

/// Options controlling [`render_ir_to_html`] / [`render_markdown_to_html`].
/// Mirrors kkdoc's `PrintOptions`.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    pub preset: PrintPreset,
    pub page_size: PageSize,
    pub orientation: Orientation,
    /// Falls back to the preset's own default margin (see
    /// [`default_margin`]) when `None`, matching kkdoc's per-preset `@page`
    /// margins.
    pub margin: Option<PageMargin>,
    /// Page header HTML, rendered as a `position: fixed` band. See the
    /// module doc comment for the per-page-repeat caveat.
    pub header: Option<String>,
    pub footer: Option<String>,
    /// Watermark text — diagonal, low-opacity gray, matching kkdoc.
    pub watermark: Option<String>,
    /// Appends a `@page` margin-box page-number rule
    /// (`counter(page) / counter(pages)`). Same Paged-Media caveat as
    /// `header`/`footer`: not honored by a plain browser's native print.
    pub page_numbers: bool,
    /// Raw CSS appended after the preset + generated rules, so callers can
    /// override anything above it.
    pub extra_css: Option<String>,
}

/// Preset default margin, matching kkdoc's hardcoded per-preset `@page`
/// margin (`default`/`gov-formal`/`compact` in `PRESETS`).
pub(crate) fn default_margin(preset: PrintPreset) -> PageMargin {
    match preset {
        PrintPreset::Default => PageMargin::uniform("20mm"),
        PrintPreset::GovFormal => PageMargin::mm(25.0, 20.0, 25.0, 20.0),
        PrintPreset::Compact => PageMargin::uniform("10mm"),
    }
}

/// Preset body CSS (everything kkdoc's `PRESETS[preset]` has *except* the
/// `@page` rule, which [`page_rule_css`] generates from `RenderOptions` so
/// page size/orientation/margin aren't hardcoded per preset).
fn preset_body_css(preset: PrintPreset) -> &'static str {
    match preset {
        PrintPreset::Default => {
            r#"
    body { font-family: 'Pretendard', 'Malgun Gothic', '맑은 고딕', sans-serif; font-size: 11pt; line-height: 1.6; color: #111; }
    h1 { font-size: 20pt; margin: 1em 0 0.5em; }
    h2 { font-size: 16pt; margin: 1em 0 0.4em; }
    h3 { font-size: 13pt; margin: 0.8em 0 0.3em; }
    h4, h5, h6 { font-size: 11pt; margin: 0.6em 0 0.3em; }
    p { margin: 0.4em 0; }
    ul, ol { margin: 0.4em 0; padding-left: 1.5em; }
    table { border-collapse: collapse; margin: 0.6em 0; width: 100%; }
    th, td { border: 1px solid #555; padding: 4px 8px; text-align: left; vertical-align: top; }
    th { background: #f0f0f0; }
    code { background: #f5f5f5; padding: 1px 4px; border-radius: 2px; font-family: 'D2Coding', Consolas, monospace; }
    pre { background: #f5f5f5; padding: 8px; border-radius: 4px; overflow-x: auto; }
    blockquote { border-left: 3px solid #ccc; padding-left: 12px; color: #555; margin: 0.6em 0; }
    hr { border: none; border-top: 1px solid #ccc; margin: 1em 0; }
    img { max-width: 100%; }
"#
        }
        PrintPreset::GovFormal => {
            r#"
    body { font-family: '함초롬바탕', 'HCR Batang', 'Batang', 'Malgun Gothic', serif; font-size: 11pt; line-height: 1.7; color: #000; }
    h1 { font-size: 18pt; text-align: center; margin: 0.5em 0 1em; letter-spacing: 0.05em; }
    h2 { font-size: 14pt; margin: 1em 0 0.4em; border-bottom: 1px solid #999; padding-bottom: 2px; }
    h3 { font-size: 12pt; margin: 0.8em 0 0.3em; }
    h4, h5, h6 { font-size: 11pt; margin: 0.6em 0 0.3em; }
    p { margin: 0.3em 0; text-indent: 1em; }
    ul, ol { margin: 0.3em 0; padding-left: 1.5em; }
    table { border-collapse: collapse; margin: 0.8em 0; width: 100%; }
    th, td { border: 1px solid #000; padding: 5px 8px; vertical-align: top; }
    th { background: #e8e8e8; font-weight: normal; }
    blockquote { border-left: 2px solid #555; padding-left: 12px; margin: 0.6em 0; }
    hr { border: none; border-top: 1px solid #000; margin: 1em 0; }
"#
        }
        PrintPreset::Compact => {
            r#"
    body { font-family: 'Pretendard', 'Malgun Gothic', sans-serif; font-size: 9pt; line-height: 1.4; color: #111; }
    h1 { font-size: 14pt; margin: 0.5em 0 0.3em; }
    h2 { font-size: 12pt; margin: 0.5em 0 0.3em; }
    h3 { font-size: 10pt; margin: 0.4em 0 0.2em; }
    h4, h5, h6 { font-size: 9pt; margin: 0.3em 0 0.2em; }
    p { margin: 0.2em 0; }
    ul, ol { margin: 0.2em 0; padding-left: 1.2em; }
    table { border-collapse: collapse; margin: 0.3em 0; width: 100%; font-size: 8pt; }
    th, td { border: 1px solid #777; padding: 2px 4px; }
    th { background: #f0f0f0; }
    hr { border: none; border-top: 1px solid #777; margin: 0.5em 0; }
"#
        }
    }
}

const WATERMARK_CSS: &str = r#"
    .watermark {
      position: fixed;
      top: 50%; left: 50%;
      transform: translate(-50%, -50%) rotate(-30deg);
      font-size: 80pt;
      color: rgba(0,0,0,0.08);
      pointer-events: none;
      z-index: 9999;
      white-space: nowrap;
    }
"#;

const HEADER_FOOTER_CSS: &str = r#"
    .print-header { position: fixed; top: 0; left: 0; right: 0; }
    .print-footer { position: fixed; bottom: 0; left: 0; right: 0; }
"#;

fn page_rule_css(options: &RenderOptions) -> String {
    let size_name = match options.page_size {
        PageSize::A4 => "A4",
        PageSize::Letter => "Letter",
    };
    let orientation = match options.orientation {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    let margin = options
        .margin
        .clone()
        .unwrap_or_else(|| default_margin(options.preset));
    let mut css = format!(
        "@page {{ size: {size_name} {orientation}; margin: {}; }}\n",
        margin.css_shorthand()
    );
    if options.page_numbers {
        // CSS3 Paged Media margin box — see module doc comment for the
        // "requires a Paged-Media-aware renderer" caveat.
        css.push_str(
            "@page { @bottom-center { content: counter(page) \" / \" counter(pages); font-size: 8pt; color: #777; } }\n",
        );
    }
    css
}

/// Render a slice of IR blocks to a print-oriented standalone HTML
/// document. Ported from kkdoc `renderHtml()` (see module doc comment for
/// what differs).
pub fn render_ir_to_html(blocks: &[IRBlock], options: &RenderOptions) -> String {
    let page_css = page_rule_css(options);
    let preset_css = preset_body_css(options.preset);

    let watermark_html = options
        .watermark
        .as_deref()
        .map(|w| format!(r#"<div class="watermark">{}</div>"#, html_escape(w)))
        .unwrap_or_default();
    let watermark_css = if options.watermark.is_some() {
        WATERMARK_CSS
    } else {
        ""
    };

    let header_html = options
        .header
        .as_deref()
        .map(|h| format!(r#"<div class="print-header">{h}</div>"#))
        .unwrap_or_default();
    let footer_html = options
        .footer
        .as_deref()
        .map(|f| format!(r#"<div class="print-footer">{f}</div>"#))
        .unwrap_or_default();
    let header_footer_css = if options.header.is_some() || options.footer.is_some() {
        HEADER_FOOTER_CSS
    } else {
        ""
    };

    let extra_css = options.extra_css.as_deref().unwrap_or("");
    let body_html = render_blocks(blocks);

    format!(
        "<!DOCTYPE html>\n<html lang=\"ko\">\n<head>\n<meta charset=\"utf-8\">\n<style>{page_css}{preset_css}{header_footer_css}{watermark_css}{extra_css}</style>\n</head>\n<body>\n{header_html}{watermark_html}\n{body_html}\n{footer_html}\n</body>\n</html>"
    )
}

/// Render the `<body>` content: one HTML element per [`IRBlock`].
fn render_blocks(blocks: &[IRBlock]) -> String {
    let mut out = String::new();
    for block in blocks {
        render_block(block, &mut out);
    }
    out
}

fn render_block(block: &IRBlock, out: &mut String) {
    match block {
        IRBlock::Paragraph { text, footnote, href } => {
            out.push_str("<p>");
            out.push_str(&html_escape(text));
            if let Some(url) = href {
                out.push_str(&format!(
                    r#" <a href="{}">{}</a>"#,
                    html_escape_attr(url),
                    html_escape(url)
                ));
            }
            if let Some(note) = footnote {
                out.push_str(&format!("<br><small>{}</small>", html_escape(note)));
            }
            out.push_str("</p>\n");
        }
        IRBlock::Heading { level, text } => {
            let level = (*level).clamp(1, 6);
            out.push_str(&format!(
                "<h{level}>{}</h{level}>\n",
                html_escape(text)
            ));
        }
        IRBlock::Table(table) => {
            out.push_str(&render_table_html(table));
            out.push('\n');
        }
        IRBlock::List { ordered, items } => {
            let tag = if *ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>\n"));
            for item in items {
                out.push_str(&format!("<li>{}</li>\n", html_escape(item)));
            }
            out.push_str(&format!("</{tag}>\n"));
        }
        IRBlock::Image { alt } => {
            let safe_alt = html_escape_attr(&sanitize_asset_name(alt));
            out.push_str(&format!(
                r#"<img src="assets/{safe_alt}" alt="{safe_alt}">"#
            ));
            out.push('\n');
        }
        IRBlock::Separator => {
            out.push_str("<hr>\n");
        }
    }
}

/// HTML `<table>` for an [`crate::ir::IRTable`], honoring `col_span` /
/// `row_span`. Independent re-implementation of `ir::render_table_html`
/// (that function is private to `ir.rs`, and this porting task treats
/// `ir.rs` as read-only) — same shadow-skip approach: mark cells covered by
/// a preceding merge's span so they aren't re-emitted.
fn render_table_html(table: &crate::ir::IRTable) -> String {
    let mut skip: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut out = String::from("<table>\n");
    for (r, row) in table.cells.iter().enumerate() {
        let tag = if r == 0 && table.has_header { "th" } else { "td" };
        let mut row_html = String::new();
        for c in 0..table.cols {
            if skip.contains(&(r, c)) {
                continue;
            }
            let Some(cell) = row.get(c) else { continue };
            let cs = cell.col_span.max(1) as usize;
            let rs = cell.row_span.max(1) as usize;
            for dr in 0..rs {
                for dc in 0..cs {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    if r + dr < table.rows && c + dc < table.cols {
                        skip.insert((r + dr, c + dc));
                    }
                }
            }
            let escaped = html_escape(cell.text.trim()).replace('\n', "<br>");
            row_html.push('<');
            row_html.push_str(tag);
            if cell.col_span > 1 {
                row_html.push_str(&format!(" colspan=\"{}\"", cell.col_span));
            }
            if cell.row_span > 1 {
                row_html.push_str(&format!(" rowspan=\"{}\"", cell.row_span));
            }
            row_html.push('>');
            row_html.push_str(&escaped);
            row_html.push_str("</");
            row_html.push_str(tag);
            row_html.push('>');
        }
        if !row_html.is_empty() {
            out.push_str("<tr>");
            out.push_str(&row_html);
            out.push_str("</tr>\n");
        }
    }
    out.push_str("</table>");
    out
}

/// HTML-escape text content (`&`, `<`, `>`). Matches kkdoc's `escapeHtml`
/// minus the quote escaping, which is only needed in attribute position —
/// see [`html_escape_attr`].
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

/// HTML-escape for attribute values (adds quote escaping on top of
/// [`html_escape`]). Matches kkdoc's `escapeHtml`.
fn html_escape_attr(s: &str) -> String {
    html_escape(s).replace('"', "&quot;").replace('\'', "&#39;")
}

/// Normalizes an [`IRBlock::Image`]'s `alt` into a bare filename safe to
/// interpolate into `assets/{name}` for `<img src>`. `alt` comes from
/// parsed document content (untrusted for the HWP/HWPX/PDF paths, and
/// directly attacker-controlled for [`super::markdown_to_ir`]'s `![alt](..)`
/// syntax) — without this, `![../../private.png](x)` would produce
/// `assets/../../private.png`, letting a crafted document escape the
/// `assets/` directory when the returned HTML is saved to disk or served
/// statically (path traversal).
///
/// Splits on both `/` and `\` (so a Windows-style traversal payload is
/// blocked on any host OS, not just the one whose path separator matches)
/// and keeps only the final segment, which drops every directory component
/// including `..`. Falls back to a fixed placeholder if what's left is
/// empty, `.`, `..`, or still contains `..` (e.g. a crafted segment like
/// `"..png"` split oddly) — better to lose a cosmetic filename than to
/// re-open the traversal on an edge case.
fn sanitize_asset_name(alt: &str) -> String {
    let candidate = alt.split(['/', '\\']).next_back().unwrap_or("").trim();
    if candidate.is_empty() || candidate == "." || candidate == ".." || candidate.contains("..") {
        "image".to_string()
    } else {
        candidate.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{IRCell, IRTable};

    fn opts() -> RenderOptions {
        RenderOptions::default()
    }

    #[test]
    fn paragraph_renders_p_tag() {
        let blocks = vec![IRBlock::paragraph("본문 텍스트")];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<p>본문 텍스트</p>"));
    }

    #[test]
    fn paragraph_with_href_and_footnote() {
        let blocks = vec![IRBlock::Paragraph {
            text: "본문".into(),
            footnote: Some("[각주] 내용".into()),
            href: Some("https://example.com".into()),
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains(r#"<a href="https://example.com">"#));
        assert!(html.contains("[각주] 내용"));
    }

    #[test]
    fn heading_renders_h_tag_per_level() {
        for level in 1..=6u8 {
            let blocks = vec![IRBlock::heading(level, "제목")];
            let html = render_ir_to_html(&blocks, &opts());
            assert!(
                html.contains(&format!("<h{level}>제목</h{level}>")),
                "level {level} missing in {html}"
            );
        }
    }

    #[test]
    fn heading_level_out_of_range_is_clamped() {
        // IRBlock::heading() itself clamps 1..=6, so this exercises the
        // render-side clamp defensively for a hand-built out-of-range value.
        let blocks = vec![IRBlock::Heading { level: 9, text: "제목".into() }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<h6>제목</h6>"));
    }

    #[test]
    fn table_renders_table_tag_with_rows() {
        let cells = vec![
            vec![IRCell::new("A"), IRCell::new("B")],
            vec![IRCell::new("C"), IRCell::new("D")],
        ];
        let table = IRTable::new(cells);
        let blocks = vec![IRBlock::Table(table)];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<table>"));
        assert!(html.contains("<th>A</th>"));
        assert!(html.contains("<td>C</td>"));
    }

    #[test]
    fn table_colspan_rowspan_renders_attrs() {
        let cells = vec![
            vec![IRCell {
                text: "병합".into(),
                col_span: 2,
                row_span: 1,
            }],
            vec![IRCell::new("값1"), IRCell::new("값2")],
        ];
        let table = IRTable {
            rows: 2,
            cols: 2,
            cells,
            has_header: false,
        };
        let blocks = vec![IRBlock::Table(table)];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains(r#"colspan="2""#));
        assert!(html.contains("병합"));
        assert!(html.contains("값1"));
        assert!(html.contains("값2"));
    }

    #[test]
    fn list_unordered_renders_ul_li() {
        let blocks = vec![IRBlock::List {
            ordered: false,
            items: vec!["첫째".into(), "둘째".into()],
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<ul>"));
        assert!(html.contains("<li>첫째</li>"));
        assert!(html.contains("<li>둘째</li>"));
    }

    #[test]
    fn list_ordered_renders_ol_li() {
        let blocks = vec![IRBlock::List {
            ordered: true,
            items: vec!["하나".into(), "둘".into()],
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<ol>"));
        assert!(html.contains("<li>하나</li>"));
    }

    #[test]
    fn image_renders_img_tag() {
        let blocks = vec![IRBlock::Image { alt: "image12".into() }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains(r#"<img src="assets/image12" alt="image12">"#));
    }

    // ── Path traversal via image alt (codex P1 finding) ──

    #[test]
    fn image_alt_path_traversal_is_sanitized() {
        let blocks = vec![IRBlock::Image {
            alt: "../../private.png".into(),
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(!html.contains(".."), "traversal survived sanitization: {html}");
        assert!(html.contains(r#"src="assets/private.png""#));
    }

    #[test]
    fn image_alt_backslash_traversal_is_sanitized() {
        // Windows-style separators must be blocked even when this renderer
        // runs on a non-Windows host (std::path::Path only special-cases
        // `\` as a separator on Windows).
        let blocks = vec![IRBlock::Image {
            alt: "..\\..\\windows\\system32\\config".into(),
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(!html.contains(".."), "traversal survived sanitization: {html}");
        assert!(html.contains(r#"src="assets/config""#));
    }

    #[test]
    fn image_alt_absolute_path_is_sanitized_to_basename() {
        let blocks = vec![IRBlock::Image {
            alt: "/etc/passwd".into(),
        }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains(r#"src="assets/passwd""#));
    }

    #[test]
    fn image_alt_dot_dot_only_falls_back_to_placeholder() {
        let blocks = vec![IRBlock::Image { alt: "..".into() }];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains(r#"src="assets/image""#));
    }

    #[test]
    fn sanitize_asset_name_direct_cases() {
        assert_eq!(sanitize_asset_name("image12"), "image12");
        assert_eq!(sanitize_asset_name("../../private.png"), "private.png");
        assert_eq!(sanitize_asset_name("a/b/../c.png"), "c.png");
        assert_eq!(sanitize_asset_name(".."), "image");
        assert_eq!(sanitize_asset_name(""), "image");
    }

    #[test]
    fn separator_renders_hr() {
        let blocks = vec![IRBlock::Separator];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(html.contains("<hr>"));
    }

    #[test]
    fn empty_input_renders_valid_html_shell() {
        let html = render_ir_to_html(&[], &opts());
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn huge_table_does_not_panic_and_renders_all_rows() {
        let rows = 200usize;
        let cols = 50usize;
        let cells: Vec<Vec<IRCell>> = (0..rows)
            .map(|r| (0..cols).map(|c| IRCell::new(format!("{r}-{c}"))).collect())
            .collect();
        let table = IRTable::new(cells);
        let blocks = vec![IRBlock::Table(table)];
        let html = render_ir_to_html(&blocks, &opts());
        assert_eq!(html.matches("<tr>").count(), rows);
        assert!(html.contains("199-49"));
    }

    #[test]
    fn html_injection_in_cell_and_paragraph_is_escaped() {
        let blocks = vec![
            IRBlock::paragraph("<script>alert(1)</script>"),
            IRBlock::Table(IRTable::new(vec![vec![IRCell::new("<b>bold</b>")]])),
        ];
        let html = render_ir_to_html(&blocks, &opts());
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<b>bold</b>"));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
    }

    #[test]
    fn watermark_included_when_set() {
        let mut o = opts();
        o.watermark = Some("초안".into());
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains(r#"<div class="watermark">초안</div>"#));
        assert!(html.contains("rotate(-30deg)"));
    }

    #[test]
    fn watermark_omitted_when_unset() {
        let html = render_ir_to_html(&[], &opts());
        assert!(!html.contains("watermark"));
    }

    #[test]
    fn page_size_and_margin_reflected_in_css() {
        let mut o = opts();
        o.page_size = PageSize::Letter;
        o.orientation = Orientation::Landscape;
        o.margin = Some(PageMargin::mm(15.0, 10.0, 15.0, 10.0));
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains("size: Letter landscape;"));
        assert!(html.contains("margin: 15mm 10mm 15mm 10mm;"));
    }

    #[test]
    fn preset_default_margin_used_when_unset() {
        let mut o = opts();
        o.preset = PrintPreset::GovFormal;
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains("margin: 25mm 20mm 25mm 20mm;"));
    }

    #[test]
    fn page_numbers_emit_paged_media_rule() {
        let mut o = opts();
        o.page_numbers = true;
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains("@bottom-center"));
        assert!(html.contains("counter(page)"));
    }

    #[test]
    fn header_footer_rendered_as_fixed_bands() {
        let mut o = opts();
        o.header = Some("문서 제목".into());
        o.footer = Some("기밀".into());
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains(r#"<div class="print-header">문서 제목</div>"#));
        assert!(html.contains(r#"<div class="print-footer">기밀</div>"#));
        assert!(html.contains(".print-header"));
    }

    #[test]
    fn extra_css_appended() {
        let mut o = opts();
        o.extra_css = Some("body { color: red; }".into());
        let html = render_ir_to_html(&[], &o);
        assert!(html.contains("body { color: red; }"));
    }
}
