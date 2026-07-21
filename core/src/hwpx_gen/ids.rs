// Ported from kkdoc (MIT): src/hwpx/gen-ids.ts
//! HWPX generation constants, theme resolution, and XML atoms (charPr/paraPr/borderFill).
//!
//! Namespaces, charPr/paraPr id constants, escape_xml, and the low-level XML
//! fragment builders shared by the header and section generators.

// ─── XML namespaces ─────────────────────────────────
pub const NS_SECTION: &str = "http://www.hancom.co.kr/hwpml/2011/section";
pub const NS_PARA: &str = "http://www.hancom.co.kr/hwpml/2011/paragraph";
pub const NS_HEAD: &str = "http://www.hancom.co.kr/hwpml/2011/head";
pub const NS_CORE: &str = "http://www.hancom.co.kr/hwpml/2011/core";
pub const NS_OPF: &str = "http://www.idpf.org/2007/opf/";
pub const NS_HPF: &str = "http://www.hancom.co.kr/schema/2011/hpf";
pub const NS_OCF: &str = "urn:oasis:names:tc:opendocument:xmlns:container";

// ─── Style id mapping ───────────────────────────────
// charPr: 0=body, 1=bold, 2=italic, 3=bold-italic, 4=inline-code,
//         5=h1, 6=h2, 7=h3, 8=h4~h6, 9=table header, 10=quote
// paraPr: 0=body, 1=h1, 2=h2, 3=h3, 4=h4~h6, 5=code block, 6=quote, 7=list
pub const CHAR_NORMAL: u32 = 0;
pub const CHAR_BOLD: u32 = 1;
pub const CHAR_ITALIC: u32 = 2;
pub const CHAR_BOLD_ITALIC: u32 = 3;
pub const CHAR_CODE: u32 = 4;
pub const CHAR_H1: u32 = 5;
pub const CHAR_TABLE_HEADER: u32 = 9;
pub const CHAR_QUOTE: u32 = 10;

pub const PARA_NORMAL: u32 = 0;
pub const PARA_CODE: u32 = 5;
pub const PARA_QUOTE: u32 = 6;
pub const PARA_LIST: u32 = 7;

// Preset (gongmun) paraPr id partition — 8 list levels then CENTER/RIGHT.
pub const GONGMUN_LIST_BASE: u32 = 8;
pub const GONGMUN_LIST_LEVELS: u32 = 8;
pub const GONGMUN_CENTER: u32 = GONGMUN_LIST_BASE + GONGMUN_LIST_LEVELS; // 16
pub const GONGMUN_RIGHT: u32 = GONGMUN_CENTER + 1; // 17

const DEFAULT_TEXT_COLOR: &str = "#000000";

/// Visual theme applied during HWPX generation (all optional).
#[derive(Debug, Clone)]
#[derive(Default)]
pub struct HwpxTheme {
    /// Heading text colors, levels 1..=4 (h5/h6 share h4).
    pub heading_colors: [Option<String>; 4],
    pub body_color: Option<String>,
    pub quote_color: Option<String>,
    pub table_header_color: Option<String>,
    pub table_header_bold: bool,
}


/// Theme with all colors resolved to concrete values.
#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    pub h1: String,
    pub h2: String,
    pub h3: String,
    pub h4: String,
    pub body: String,
    pub quote: String,
    /// Whether quote_color was explicitly set (drives blockquote charPr branch).
    pub has_quote_option: bool,
    pub table_header: String,
    pub table_header_bold: bool,
}

pub fn resolve_theme(theme: &HwpxTheme) -> ResolvedTheme {
    let d = DEFAULT_TEXT_COLOR.to_string();
    let hc = |i: usize| theme.heading_colors[i].clone();
    let h3 = hc(2).unwrap_or_else(|| d.clone());
    ResolvedTheme {
        h1: hc(0).unwrap_or_else(|| d.clone()),
        h2: hc(1).unwrap_or_else(|| d.clone()),
        h3: h3.clone(),
        h4: hc(3).or_else(|| hc(2)).unwrap_or_else(|| d.clone()),
        body: theme.body_color.clone().unwrap_or_else(|| d.clone()),
        quote: theme.quote_color.clone().unwrap_or_else(|| d.clone()),
        has_quote_option: theme.quote_color.is_some(),
        table_header: theme
            .table_header_color
            .clone()
            .or_else(|| theme.body_color.clone())
            .unwrap_or_else(|| d.clone()),
        table_header_bold: theme.table_header_bold,
    }
}

// ─── XML helpers ────────────────────────────────────

/// Escape text for XML 1.0. Strips forbidden C0 control chars (keeps \t \n \r)
/// so optional fields cannot break well-formedness, then escapes & < > ".
pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}' => {}
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn heading_para_pr_id(level: u32) -> u32 {
    match level {
        1 => 1,
        2 => 2,
        3 => 3,
        _ => 4,
    }
}

pub fn heading_char_pr_id(level: u32) -> u32 {
    match level {
        1 => 5,
        2 => 6,
        3 => 7,
        _ => 8,
    }
}

/// One `<hh:charPr>` entry. `bold` is emitted both as attribute (legacy Hangul)
/// and the canonical `<hh:bold/>` child element.
#[allow(clippy::too_many_arguments)]
pub fn char_pr(
    id: u32,
    height: u32,
    bold: bool,
    italic: bool,
    font_id: u32,
    text_color: &str,
    ratio_pct: u32,
) -> String {
    let bold_attr = if bold { " bold=\"1\"" } else { "" };
    let italic_attr = if italic { " italic=\"1\"" } else { "" };
    let bold_el = if bold { "<hh:bold/>" } else { "" };
    format!(
        "      <hh:charPr id=\"{id}\" height=\"{height}\" textColor=\"{color}\" shadeColor=\"none\" useFontSpace=\"0\" useKerning=\"0\" symMark=\"NONE\" borderFillIDRef=\"1\"{bold_attr}{italic_attr}>\n\
        \x20       <hh:fontRef hangul=\"{f}\" latin=\"{f}\" hanja=\"{f}\" japanese=\"{f}\" other=\"{f}\" symbol=\"{f}\" user=\"{f}\"/>\n\
        \x20       <hh:ratio hangul=\"{r}\" latin=\"{r}\" hanja=\"{r}\" japanese=\"100\" other=\"100\" symbol=\"100\" user=\"100\"/>\n\
        \x20       <hh:spacing hangul=\"0\" latin=\"0\" hanja=\"0\" japanese=\"0\" other=\"0\" symbol=\"0\" user=\"0\"/>\n\
        \x20       <hh:relSz hangul=\"100\" latin=\"100\" hanja=\"100\" japanese=\"100\" other=\"100\" symbol=\"100\" user=\"100\"/>\n\
        \x20       <hh:offset hangul=\"0\" latin=\"0\" hanja=\"0\" japanese=\"0\" other=\"0\" symbol=\"0\" user=\"0\"/>{bold_el}\n\
        \x20     </hh:charPr>",
        id = id,
        height = height,
        color = escape_xml(text_color),
        f = font_id,
        r = ratio_pct,
    )
}

/// Optional attributes for [`para_pr`].
#[derive(Debug, Clone)]
pub struct ParaOpts {
    pub align: &'static str,
    pub space_before: i32,
    pub space_after: i32,
    pub line_spacing: u32,
    pub indent: i32,
    pub left: i32,
    pub keep_word: bool,
    pub keep_with_next: bool,
    pub outline_level: Option<u32>,
}

impl Default for ParaOpts {
    fn default() -> Self {
        ParaOpts {
            align: "JUSTIFY",
            space_before: 0,
            space_after: 0,
            line_spacing: 160,
            indent: 0,
            left: 0,
            keep_word: false,
            keep_with_next: false,
            outline_level: None,
        }
    }
}

/// One `<hh:paraPr>` entry. See kkdoc gen-ids.ts for the breakNonLatinWord
/// name-inversion note (BREAK_WORD = keep word, KEEP_WORD = per-character).
pub fn para_pr(id: u32, opts: &ParaOpts) -> String {
    let break_non_latin = if opts.keep_word { "BREAK_WORD" } else { "KEEP_WORD" };
    let snap_grid = if opts.keep_word { "0" } else { "1" };
    let heading = match opts.outline_level {
        Some(l) => format!("<hh:heading type=\"OUTLINE\" idRef=\"0\" level=\"{l}\"/>"),
        None => "<hh:heading type=\"NONE\" idRef=\"0\" level=\"0\"/>".to_string(),
    };
    let keep_next = if opts.keep_with_next { 1 } else { 0 };
    format!(
        "      <hh:paraPr id=\"{id}\" tabPrIDRef=\"0\" condense=\"0\" fontLineHeight=\"0\" snapToGrid=\"{snap}\" suppressLineNumbers=\"0\" checked=\"0\" textDir=\"AUTO\">\n\
        \x20       <hh:align horizontal=\"{align}\" vertical=\"BASELINE\"/>\n\
        \x20       {heading}\n\
        \x20       <hh:breakSetting breakLatinWord=\"KEEP_WORD\" breakNonLatinWord=\"{bnl}\" widowOrphan=\"0\" keepWithNext=\"{keep_next}\" keepLines=\"0\" pageBreakBefore=\"0\" lineWrap=\"BREAK\"/>\n\
        \x20       <hh:autoSpacing eAsianEng=\"0\" eAsianNum=\"0\"/>\n\
        \x20       <hh:margin><hc:intent value=\"{indent}\" unit=\"HWPUNIT\"/><hc:left value=\"{left}\" unit=\"HWPUNIT\"/><hc:right value=\"0\" unit=\"HWPUNIT\"/><hc:prev value=\"{sb}\" unit=\"HWPUNIT\"/><hc:next value=\"{sa}\" unit=\"HWPUNIT\"/></hh:margin>\n\
        \x20       <hh:lineSpacing type=\"PERCENT\" value=\"{ls}\"/>\n\
        \x20       <hh:border borderFillIDRef=\"1\" offsetLeft=\"0\" offsetRight=\"0\" offsetTop=\"0\" offsetBottom=\"0\" connect=\"0\" ignoreMargin=\"0\"/>\n\
        \x20     </hh:paraPr>",
        id = id,
        snap = snap_grid,
        align = opts.align,
        heading = heading,
        bnl = break_non_latin,
        keep_next = keep_next,
        indent = opts.indent,
        left = opts.left,
        sb = opts.space_before,
        sa = opts.space_after,
        ls = opts.line_spacing,
    )
}

/// A border side: (width, color) with default SOLID line, or (width, color, type).
pub type BorderSide<'a> = (&'a str, &'a str, Option<&'a str>);

/// One `<hh:borderFill>` — four sides plus an optional solid fill color.
pub fn border_fill_entry(
    id: u32,
    left: Option<BorderSide>,
    right: Option<BorderSide>,
    top: Option<BorderSide>,
    bottom: Option<BorderSide>,
    fill: Option<&str>,
) -> String {
    let side = |name: &str, v: Option<BorderSide>| match v {
        Some((w, c, ty)) => format!(
            "        <hh:{name} type=\"{ty}\" width=\"{w}\" color=\"{c}\"/>",
            name = name,
            ty = ty.unwrap_or("SOLID"),
            w = w,
            c = c,
        ),
        None => format!(
            "        <hh:{name} type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>",
            name = name
        ),
    };
    let brush = match fill {
        Some(color) => format!(
            "\n        <hc:fillBrush><hc:winBrush faceColor=\"{color}\" hatchColor=\"#000000\" alpha=\"0\"/></hc:fillBrush>"
        ),
        None => String::new(),
    };
    format!(
        "      <hh:borderFill id=\"{id}\" threeD=\"0\" shadow=\"0\" centerLine=\"NONE\" breakCellSeparateLine=\"0\">\n\
        \x20       <hh:slash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>\n\
        \x20       <hh:backSlash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>\n\
        {l}\n{r}\n{t}\n{b}{brush}\n\
        \x20     </hh:borderFill>",
        id = id,
        l = side("leftBorder", left),
        r = side("rightBorder", right),
        t = side("topBorder", top),
        b = side("bottomBorder", bottom),
        brush = brush,
    )
}

/// Page-number ctrl — bottom-center "- 1 -".
pub fn page_num_ctrl() -> &'static str {
    "<hp:ctrl><hp:pageNum pos=\"BOTTOM_CENTER\" formatType=\"DIGIT\" sideChar=\"-\"/></hp:ctrl>"
}
