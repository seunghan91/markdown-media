// Ported from kkdoc (MIT): src/hwpx/gen-section.ts
//! Section XML assembly — secPr (standard margins) + block list → section0.xml.
//!
//! The reference's measured preamble/postamble (cover, toc, chapter boxes,
//! approval tables, docframe) is out of scope; this covers the body block
//! pipeline: headings, paragraphs, lists, tables, code, quotes, images, rules.

use super::blocks::{generate_paragraph, generate_runs, BlockKind, MdBlock};
use super::chart::ChartRegistry;
use super::ids::{
    escape_xml, heading_char_pr_id, heading_para_pr_id, page_num_ctrl, ResolvedTheme, CHAR_CODE,
    CHAR_NORMAL, CHAR_QUOTE, GONGMUN_LIST_BASE, PARA_CODE, PARA_LIST, PARA_NORMAL, PARA_QUOTE,
    NS_PARA, NS_SECTION,
};
use super::image::{split_image_refs, ImageRegistry};
use super::preset::{mm_to_hwpunit, Numberer, ResolvedPreset, A4_H_HU, A4_W_HU, H2Marker, Numbering};
use super::profile::ProfileRemap;
use super::table::{generate_html_table_xml, generate_table, TableStyle};

/// Build `<hp:secPr>` + colPr (+ page number ctrl for presets).
fn generate_sec_pr(preset: Option<&ResolvedPreset>) -> String {
    let (top, bottom, left, right, header, footer) = match preset {
        Some(p) => (
            mm_to_hwpunit(p.margins.top),
            mm_to_hwpunit(p.margins.bottom),
            mm_to_hwpunit(p.margins.left),
            mm_to_hwpunit(p.margins.right),
            p.header_footer as i32,
            p.header_footer as i32,
        ),
        None => (8504, 4252, 5670, 4252, 2835, 2835),
    };
    let sec_pr = format!(
        "<hp:secPr textDirection=\"HORIZONTAL\" spaceColumns=\"1134\" tabStop=\"8000\" outlineShapeIDRef=\"1\" memoShapeIDRef=\"0\" textVerticalWidthHead=\"0\" masterPageCnt=\"0\">\
        <hp:grid lineGrid=\"0\" charGrid=\"0\" wonggojiFormat=\"0\"/>\
        <hp:startNum pageStartsOn=\"BOTH\" page=\"0\" pic=\"0\" tbl=\"0\" equation=\"0\"/>\
        <hp:visibility hideFirstHeader=\"0\" hideFirstFooter=\"0\" hideFirstMasterPage=\"0\" border=\"SHOW_ALL\" fill=\"SHOW_ALL\" hideFirstPageNum=\"0\" hideFirstEmptyLine=\"0\" showLineNumber=\"0\"/>\
        <hp:pagePr landscape=\"WIDELY\" width=\"{w}\" height=\"{h}\" gutterType=\"LEFT_ONLY\">\
        <hp:margin header=\"{header}\" footer=\"{footer}\" gutter=\"0\" left=\"{left}\" right=\"{right}\" top=\"{top}\" bottom=\"{bottom}\"/>\
        </hp:pagePr>\
        <hp:footNotePr><hp:autoNumFormat type=\"DIGIT\" userChar=\"\" prefixChar=\"\" suffixChar=\")\" supscript=\"0\"/><hp:noteLine length=\"-1\" type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/><hp:noteSpacing betweenNotes=\"283\" belowLine=\"567\" aboveLine=\"850\"/><hp:numbering type=\"CONTINUOUS\" newNum=\"1\"/><hp:placement place=\"EACH_COLUMN\" beneathText=\"0\"/></hp:footNotePr>\
        <hp:endNotePr><hp:autoNumFormat type=\"DIGIT\" userChar=\"\" prefixChar=\"\" suffixChar=\")\" supscript=\"0\"/><hp:noteLine length=\"14692344\" type=\"SOLID\" width=\"0.12 mm\" color=\"#000000\"/><hp:noteSpacing betweenNotes=\"0\" belowLine=\"567\" aboveLine=\"850\"/><hp:numbering type=\"CONTINUOUS\" newNum=\"1\"/><hp:placement place=\"END_OF_DOCUMENT\" beneathText=\"0\"/></hp:endNotePr>\
        </hp:secPr>",
        w = A4_W_HU, h = A4_H_HU,
        header = header, footer = footer, left = left, right = right, top = top, bottom = bottom,
    );
    let col_pr = "<hp:ctrl><hp:colPr id=\"\" type=\"NEWSPAPER\" layout=\"LEFT\" colCount=\"1\" sameSz=\"1\" sameGap=\"0\"/></hp:ctrl>";
    let page_num = if preset.map(|p| p.page_numbers).unwrap_or(false) {
        page_num_ctrl()
    } else {
        ""
    };
    format!("{sec_pr}{col_pr}{page_num}")
}

/// Assembly context shared across block handlers.
struct SectionCtx<'a> {
    theme: &'a ResolvedTheme,
    preset: Option<&'a ResolvedPreset>,
    table_style: Option<TableStyle>,
    images: &'a mut ImageRegistry,
    charts: &'a mut ChartRegistry,
    profile: Option<&'a ProfileRemap>,
    para_xmls: Vec<String>,
    opener_pending: bool,
    // list state (non-preset running counters)
    ordered_counters: std::collections::HashMap<usize, usize>,
    prev_was_ordered: bool,
    // preset heading marker counter for h2 "number" mode
    h2_seq: usize,
    // equation zOrder counter (distinct <hp:equation> ids)
    eq_seq: usize,
    // emitted-table sequence counter — format profile matching key (table_index fallback)
    table_seq: usize,
    numberer: Option<Numberer>,
}

impl<'a> SectionCtx<'a> {
    /// Inject secPr into the first run of a paragraph XML (consumes opener).
    fn inject_opener(&mut self, xml: &str) -> String {
        self.opener_pending = false;
        inject_first_run(xml, &generate_sec_pr(self.preset))
    }

    /// Emit a carrier empty paragraph carrying secPr before a table/image block.
    fn emit_carrier(&mut self) {
        if !self.opener_pending {
            return;
        }
        self.opener_pending = false;
        let sec = generate_sec_pr(self.preset);
        self.para_xmls.push(format!(
            "<hp:p paraPrIDRef=\"0\" styleIDRef=\"0\"><hp:run charPrIDRef=\"0\">{sec}<hp:t></hp:t></hp:run></hp:p>"
        ));
    }
}

/// Replace the first `<hp:run charPrIDRef="N">` open tag with itself + payload.
fn inject_first_run(xml: &str, payload: &str) -> String {
    if let Some(pos) = xml.find("<hp:run charPrIDRef=\"") {
        if let Some(close) = xml[pos..].find('>') {
            let insert_at = pos + close + 1;
            let mut out = String::with_capacity(xml.len() + payload.len());
            out.push_str(&xml[..insert_at]);
            out.push_str(payload);
            out.push_str(&xml[insert_at..]);
            return out;
        }
    }
    xml.to_string()
}

fn strip_leading_number(text: &str) -> String {
    // Remove a leading "1." / "1)" / "Ⅰ." chapter number if present.
    let t = text.trim_start();
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
        return t[i + 1..].trim_start().to_string();
    }
    text.to_string()
}

fn render_heading(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    let level = block.level.max(1);
    let p_id = heading_para_pr_id(level);
    let c_id = heading_char_pr_id(level);
    let style_id = if ctx.preset.is_some() { level.min(4) } else { 0 };
    let mut h_text = block.text.clone();
    if let Some(p) = ctx.preset {
        if level == 2 && p.h2_marker != H2Marker::None {
            let title = strip_leading_number(&h_text);
            h_text = match p.h2_marker {
                H2Marker::Box => format!("□ {title}"),
                H2Marker::Number => {
                    ctx.h2_seq += 1;
                    format!("{}. {title}", ctx.h2_seq)
                }
                H2Marker::None => title,
            };
        }
    }
    generate_paragraph(&h_text, p_id, c_id, None, style_id)
}

fn render_paragraph(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    // Image-only paragraph → placeholder <hp:pic>(s).
    let (rest, urls) = split_image_refs(&block.text);
    if !urls.is_empty() && rest.trim().is_empty() {
        let mut pics = String::new();
        let mut all_ok = true;
        for u in &urls {
            match ctx.images.take(u) {
                Some(part) => pics.push_str(&ctx.images.inline_pic_xml(&part)),
                None => {
                    all_ok = false;
                    break;
                }
            }
        }
        if all_ok {
            return format!(
                "<hp:p paraPrIDRef=\"{PARA_NORMAL}\" styleIDRef=\"0\"><hp:run charPrIDRef=\"{CHAR_NORMAL}\">{pics}</hp:run></hp:p>"
            );
        }
    }
    generate_paragraph(&block.text, PARA_NORMAL, CHAR_NORMAL, None, 0)
}

fn render_code_block(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    // ```chart fence → chart part + <hp:chart> (falls back to a plain code
    // block when the fence has no parseable numeric series).
    if block.lang.eq_ignore_ascii_case("chart") {
        if let Some(fence) = super::chart::parse_chart_fence(&block.text) {
            ctx.emit_carrier();
            let chart_el = ctx.charts.register(&fence);
            return format!(
                "<hp:p paraPrIDRef=\"{PARA_NORMAL}\" styleIDRef=\"0\"><hp:run charPrIDRef=\"{CHAR_NORMAL}\">{chart_el}</hp:run></hp:p>"
            );
        }
    }
    block
        .text
        .split('\n')
        .map(|line| generate_paragraph(if line.is_empty() { " " } else { line }, PARA_CODE, CHAR_CODE, None, 0))
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn render_equation(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    // LaTeX → HULK/EqEdit script via crate::equation, wrapped in <hp:equation>.
    // Ported from kkdoc generateEquationParagraph/generateEquationXml.
    ctx.emit_carrier();
    let z_order = ctx.eq_seq;
    ctx.eq_seq += 1;
    let script = crate::equation::latex_to_hulk(&block.text);
    let eq = equation_xml(&script, z_order);
    format!(
        "<hp:p paraPrIDRef=\"{PARA_NORMAL}\" styleIDRef=\"0\"><hp:run charPrIDRef=\"{CHAR_NORMAL}\">{eq}</hp:run></hp:p>"
    )
}

/// Estimate equation box metrics (width, height, baseline) from the script.
fn estimate_equation_metrics(script: &str) -> (i32, i32, i32) {
    let cleaned: String = script
        .chars()
        .filter(|c| !matches!(c, '{' | '}' | '\\' | '^' | '_'))
        .collect();
    let cleaned_len = cleaned.split_whitespace().collect::<Vec<_>>().join(" ").chars().count();
    let width = ((cleaned_len.max(5) as i32) * 700 + 2000).min(40000);
    let row_count = script.matches('#').count() + 1;
    if script.contains("matrix") || script.contains('#') {
        if row_count >= 4 {
            return (width, 5500, 55);
        }
        if row_count == 3 {
            return (width, 4500, 60);
        }
        return (width, 3260, 63);
    }
    if script.contains("over") || script.contains("root") || script.contains("sqrt") {
        return (width, 3010, 69);
    }
    (width, 1450, 71)
}

/// Build a `<hp:equation>` element (treatAsChar) for an EqEdit script.
fn equation_xml(script: &str, z_order: usize) -> String {
    let (width, height, baseline) = estimate_equation_metrics(script);
    let eq_id = 2_000_000_001u64 + z_order as u64;
    format!(
        "<hp:equation id=\"{id}\" zOrder=\"{z}\" numberingType=\"EQUATION\" textWrap=\"TOP_AND_BOTTOM\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" version=\"Equation Version 60\" baseLine=\"{base}\" textColor=\"#000000\" baseUnit=\"1200\" lineMode=\"CHAR\" font=\"HYhwpEQ\">\
        <hp:sz width=\"{w}\" widthRelTo=\"ABSOLUTE\" height=\"{h}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
        <hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"1\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"PARA\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
        <hp:outMargin left=\"56\" right=\"56\" top=\"0\" bottom=\"0\"/>\
        <hp:shapeComment>수식입니다.</hp:shapeComment>\
        <hp:script>{script}</hp:script>\
        </hp:equation>",
        id = eq_id, z = z_order, base = baseline, w = width, h = height,
        script = escape_xml(script)
    )
}

fn render_blockquote(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    let quote_char = if ctx.theme.has_quote_option { CHAR_QUOTE } else { CHAR_NORMAL };
    block
        .text
        .split('\n')
        .map(|line| generate_paragraph(line, PARA_QUOTE, quote_char, None, 0))
        .collect::<Vec<_>>()
        .join("\n  ")
}

fn render_list_item(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    if let Some(p) = ctx.preset {
        let depth = block.indent;
        let marker = ctx.numberer.as_mut().unwrap().next(depth);
        let text = if marker.is_empty() {
            block.text.clone()
        } else {
            format!("{marker} {}", block.text)
        };
        let para_id = GONGMUN_LIST_BASE + depth.min(7) as u32;
        let char_id = if p.numbering == Numbering::Report && depth == 0 {
            super::ids::CHAR_BOLD
        } else {
            CHAR_NORMAL
        };
        return generate_paragraph(&text, para_id, char_id, None, 0);
    }
    // Non-preset: running counters + preserved markers.
    let indent = block.indent;
    let marker;
    if !block.marker.is_empty() {
        marker = format!("{} ", block.marker);
        ctx.prev_was_ordered = block.ordered;
    } else if block.ordered {
        *ctx.ordered_counters.entry(indent).or_insert(0) += 1;
        let deeper: Vec<usize> = ctx.ordered_counters.keys().copied().filter(|&k| k > indent).collect();
        for k in deeper {
            ctx.ordered_counters.remove(&k);
        }
        marker = format!("{}. ", ctx.ordered_counters[&indent]);
        ctx.prev_was_ordered = true;
    } else {
        marker = "· ".to_string();
        if ctx.prev_was_ordered {
            ctx.ordered_counters.clear();
        }
        ctx.prev_was_ordered = false;
    }
    let prefix = "  ".repeat(indent);
    generate_paragraph(&format!("{prefix}{marker}{}", block.text), PARA_LIST, CHAR_NORMAL, None, 0)
}

fn render_hr(ctx: &SectionCtx) -> String {
    if ctx.preset.is_some() {
        format!("<hp:p paraPrIDRef=\"{PARA_NORMAL}\" styleIDRef=\"0\"><hp:run charPrIDRef=\"{CHAR_NORMAL}\"><hp:t></hp:t></hp:run></hp:p>")
    } else {
        "<hp:p paraPrIDRef=\"0\" styleIDRef=\"0\"><hp:run charPrIDRef=\"0\"><hp:t>────────────────────────────────────────</hp:t></hp:run></hp:p>".to_string()
    }
}

fn render_table(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    ctx.emit_carrier();
    let seq = ctx.table_seq;
    ctx.table_seq += 1;
    generate_table(&block.rows, ctx.theme, ctx.table_style.as_ref(), ctx.profile, seq)
}

fn render_html_table(block: &MdBlock, ctx: &mut SectionCtx) -> String {
    let total_w = ctx.table_style.as_ref().map(|s| s.total_width).unwrap_or(44000);
    let seq = ctx.table_seq;
    ctx.table_seq += 1;
    match generate_html_table_xml(&block.text, ctx.theme, total_w, ctx.table_style.as_ref(), ctx.profile, seq) {
        Some(tbl) => {
            ctx.emit_carrier();
            format!("<hp:p paraPrIDRef=\"0\" styleIDRef=\"0\"><hp:run charPrIDRef=\"0\">{tbl}</hp:run></hp:p>")
        }
        None => {
            let plain = strip_tags_ws(&block.text);
            if plain.is_empty() {
                String::new()
            } else {
                generate_paragraph(&plain, PARA_NORMAL, CHAR_NORMAL, None, 0)
            }
        }
    }
}

fn strip_tags_ws(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Assemble section0.xml from blocks.
#[allow(clippy::too_many_arguments)]
pub fn blocks_to_section_xml(
    blocks: &[MdBlock],
    theme: &ResolvedTheme,
    preset: Option<&ResolvedPreset>,
    header_bf: u32,
    images: &mut ImageRegistry,
    charts: &mut ChartRegistry,
    profile: Option<&ProfileRemap>,
) -> String {
    let table_style = preset.map(|p| TableStyle {
        total_width: mm_to_hwpunit(210.0 - p.margins.left - p.margins.right),
        header_bf,
    });
    let numberer = preset.map(Numberer::new);

    let mut ctx = SectionCtx {
        theme,
        preset,
        table_style,
        images,
        charts,
        profile,
        para_xmls: Vec::new(),
        opener_pending: true,
        ordered_counters: std::collections::HashMap::new(),
        prev_was_ordered: false,
        h2_seq: 0,
        eq_seq: 0,
        table_seq: 0,
        numberer,
    };

    for block in blocks {
        // Reset ordered counters when a non-ordered-list block interrupts.
        if block.kind != BlockKind::ListItem || !block.ordered {
            if ctx.prev_was_ordered {
                ctx.ordered_counters.clear();
            }
            ctx.prev_was_ordered = false;
        }

        let mut xml = match block.kind {
            BlockKind::Heading => render_heading(block, &mut ctx),
            BlockKind::Paragraph => render_paragraph(block, &mut ctx),
            BlockKind::CodeBlock => render_code_block(block, &mut ctx),
            BlockKind::Equation => render_equation(block, &mut ctx),
            BlockKind::Blockquote => render_blockquote(block, &mut ctx),
            BlockKind::ListItem => render_list_item(block, &mut ctx),
            BlockKind::Hr => render_hr(&ctx),
            BlockKind::Table => render_table(block, &mut ctx),
            BlockKind::HtmlTable => render_html_table(block, &mut ctx),
        };

        if xml.is_empty() {
            continue;
        }

        if ctx.opener_pending && block.kind != BlockKind::Table && block.kind != BlockKind::HtmlTable {
            xml = ctx.inject_opener(&xml);
        }

        ctx.para_xmls.push(xml);
    }

    if ctx.para_xmls.is_empty() {
        ctx.opener_pending = false;
        let sec = generate_sec_pr(preset);
        ctx.para_xmls.push(format!(
            "<hp:p paraPrIDRef=\"0\" styleIDRef=\"0\"><hp:run charPrIDRef=\"0\">{sec}<hp:t></hp:t></hp:run></hp:p>"
        ));
    }

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n\
<hs:sec xmlns:hs=\"{sec}\" xmlns:hp=\"{para}\">\n\
\x20 {body}\n\
</hs:sec>",
        sec = NS_SECTION,
        para = NS_PARA,
        body = ctx.para_xmls.join("\n  "),
    )
}