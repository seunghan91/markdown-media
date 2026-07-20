// Ported from kkdoc (MIT): src/hwpx/gen-header.ts
//! HWPX package files (container.xml, content.hpf) and Contents/header.xml.
//!
//! The font table is the reference's generic 7-language set (함초롬바탕 등); the
//! measured report/gaejosik font set is out of scope. charPr/paraPr id spaces
//! are kept sequential so section run references resolve correctly.

use super::ids::{
    border_fill_entry, char_pr, escape_xml, para_pr, ParaOpts, ResolvedTheme, CHAR_QUOTE,
    CHAR_TABLE_HEADER, GONGMUN_CENTER, GONGMUN_LIST_BASE, GONGMUN_LIST_LEVELS, GONGMUN_RIGHT,
    NS_CORE, NS_HEAD, NS_HPF, NS_OCF, NS_OPF, NS_PARA,
};
use super::preset::{level_indent, ResolvedPreset};

pub fn generate_container_xml() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n\
<ocf:container xmlns:ocf=\"{ocf}\" xmlns:hpf=\"{hpf}\">\n\
\x20 <ocf:rootfiles>\n\
\x20   <ocf:rootfile full-path=\"Contents/content.hpf\" media-type=\"application/hwpml-package+xml\"/>\n\
\x20 </ocf:rootfiles>\n\
</ocf:container>",
        ocf = NS_OCF,
        hpf = NS_HPF
    )
}

/// content.hpf manifest. `chart_items`/`image_items` are pre-built
/// `<opf:item .../>` strings (charts listed first, matching part registration
/// order).
pub fn generate_manifest(chart_items: &[String], image_items: &[String], layout: &str) -> String {
    let charts: String = chart_items.iter().map(|x| format!("\n    {x}")).collect();
    let imgs: String = image_items.iter().map(|x| format!("\n    {x}")).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n\
<opf:package xmlns:opf=\"{opf}\" xmlns:hpf=\"{hpf}\" xmlns:hh=\"{head}\">\n\
\x20 <opf:metadata>\n\
\x20   <opf:meta name=\"generator\" content=\"kordoc\"/>\n\
\x20   <opf:meta name=\"kordoc-layout\" content=\"{layout}\"/>\n\
\x20 </opf:metadata>\n\
\x20 <opf:manifest>\n\
\x20   <opf:item id=\"header\" href=\"Contents/header.xml\" media-type=\"application/xml\"/>\n\
\x20   <opf:item id=\"section0\" href=\"Contents/section0.xml\" media-type=\"application/xml\"/>{charts}{imgs}\n\
\x20 </opf:manifest>\n\
\x20 <opf:spine>\n\
\x20   <opf:itemref idref=\"header\" linear=\"no\"/>\n\
\x20   <opf:itemref idref=\"section0\" linear=\"yes\"/>\n\
\x20 </opf:spine>\n\
</opf:package>",
        opf = NS_OPF,
        hpf = NS_HPF,
        head = NS_HEAD,
        layout = escape_xml(layout),
        charts = charts,
        imgs = imgs
    )
}

fn font_entry(id: u32, face: &str, weight: u32) -> String {
    format!(
        "        <hh:font id=\"{id}\" face=\"{face}\" type=\"TTF\" isEmbedded=\"0\">\n\
        \x20         <hh:typeInfo familyType=\"FCAT_GOTHIC\" weight=\"{weight}\" proportion=\"4\" contrast=\"0\" strokeVariation=\"1\" armStyle=\"1\" letterform=\"1\" midline=\"1\" xHeight=\"1\"/>\n\
        \x20       </hh:font>",
        id = id,
        face = escape_xml(face),
        weight = weight
    )
}

fn build_font_faces(body_face: &str) -> String {
    format!(
        "<hh:fontfaces itemCnt=\"7\">\n\
\x20     <hh:fontface lang=\"HANGUL\" fontCnt=\"3\">\n\
{h0}\n{h1}\n{h2}\n\
\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"LATIN\" fontCnt=\"3\">\n\
\x20       <hh:font id=\"0\" face=\"Times New Roman\" type=\"TTF\" isEmbedded=\"0\"><hh:typeInfo familyType=\"FCAT_OLDSTYLE\" weight=\"5\" proportion=\"4\" contrast=\"2\" strokeVariation=\"0\" armStyle=\"0\" letterform=\"0\" midline=\"0\" xHeight=\"4\"/></hh:font>\n\
\x20       <hh:font id=\"1\" face=\"Consolas\" type=\"TTF\" isEmbedded=\"0\"><hh:typeInfo familyType=\"FCAT_MODERN\" weight=\"5\" proportion=\"0\" contrast=\"0\" strokeVariation=\"0\" armStyle=\"0\" letterform=\"0\" midline=\"0\" xHeight=\"0\"/></hh:font>\n\
\x20       <hh:font id=\"2\" face=\"Arial Black\" type=\"TTF\" isEmbedded=\"0\"><hh:typeInfo familyType=\"FCAT_GOTHIC\" weight=\"9\" proportion=\"0\" contrast=\"0\" strokeVariation=\"0\" armStyle=\"0\" letterform=\"0\" midline=\"0\" xHeight=\"0\"/></hh:font>\n\
\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"HANJA\" fontCnt=\"1\">\n{hanja}\n\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"JAPANESE\" fontCnt=\"1\">\n{jp}\n\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"OTHER\" fontCnt=\"1\">\n{other}\n\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"SYMBOL\" fontCnt=\"1\">\n{sym}\n\x20     </hh:fontface>\n\
\x20     <hh:fontface lang=\"USER\" fontCnt=\"1\">\n{user}\n\x20     </hh:fontface>\n\
\x20   </hh:fontfaces>",
        h0 = font_entry(0, body_face, 6),
        h1 = font_entry(1, "함초롬돋움", 6),
        h2 = font_entry(2, "HY견고딕", 9),
        hanja = font_entry(0, "함초롬바탕", 6),
        jp = font_entry(0, "굴림", 6),
        other = font_entry(0, "굴림", 6),
        sym = font_entry(0, "Symbol", 6),
        user = font_entry(0, "굴림", 6),
    )
}

/// Static charPr id count (0..=10, 11 entries) — the base id
/// [`super::profile::profile_char_pr_base`] assumes for profile-appended charPrs.
fn build_char_properties(theme: &ResolvedTheme, preset: Option<&ResolvedPreset>, extra: &[String]) -> String {
    let (body, code, h1, h2, h3, h4) = match preset {
        None => (1000u32, 900u32, 1800u32, 1400u32, 1200u32, 1100u32),
        Some(p) => {
            let body = p.body_height;
            let code = (body.saturating_sub(200)).max(900);
            let h1_min = if matches!(p.preset, super::preset::Preset::Report | super::preset::Preset::Plan) {
                2000
            } else {
                1700
            };
            let h1 = h1_min.max(body);
            let h2 = 1600u32.max(body);
            let h3 = body;
            let h4 = h3.min((body.saturating_sub(100)).max(1300));
            (body, code, h1, h2, h3, h4)
        }
    };
    let body_ratio = if preset.is_some() { 95 } else { 100 };
    let rows = [
        char_pr(0, body, false, false, 0, &theme.body, body_ratio),
        char_pr(1, body, true, false, 0, &theme.body, body_ratio),
        char_pr(2, body, false, true, 0, &theme.body, body_ratio),
        char_pr(3, body, true, true, 0, &theme.body, body_ratio),
        char_pr(4, code, false, false, 1, "#000000", 100),
        char_pr(5, h1, true, false, 1, &theme.h1, 100),
        char_pr(6, h2, true, false, 1, &theme.h2, 100),
        char_pr(7, h3, true, false, 1, &theme.h3, 100),
        char_pr(8, h4, true, false, 1, &theme.h4, 100),
        char_pr(CHAR_TABLE_HEADER, body, theme.table_header_bold, false, 0, &theme.table_header, 100),
        char_pr(CHAR_QUOTE, body, false, true, 0, &theme.quote, 100),
    ];
    let item_cnt = rows.len() + extra.len();
    let mut body_xml = rows.join("\n");
    if !extra.is_empty() {
        body_xml.push('\n');
        body_xml.push_str(&extra.join("\n"));
    }
    format!("<hh:charProperties itemCnt=\"{item_cnt}\">\n{body_xml}\n    </hh:charProperties>")
}

fn build_para_properties(preset: Option<&ResolvedPreset>) -> String {
    let mut rows: Vec<String> = Vec::new();
    match preset {
        None => {
            rows.push(para_pr(0, &ParaOpts { keep_word: true, ..Default::default() }));
            rows.push(para_pr(1, &ParaOpts { align: "LEFT", space_before: 800, space_after: 200, line_spacing: 180, outline_level: Some(0), keep_word: true, ..Default::default() }));
            rows.push(para_pr(2, &ParaOpts { align: "LEFT", space_before: 600, space_after: 150, line_spacing: 170, outline_level: Some(1), keep_word: true, ..Default::default() }));
            rows.push(para_pr(3, &ParaOpts { align: "LEFT", space_before: 400, space_after: 100, line_spacing: 160, outline_level: Some(2), keep_word: true, ..Default::default() }));
            rows.push(para_pr(4, &ParaOpts { align: "LEFT", space_before: 300, space_after: 100, line_spacing: 160, outline_level: Some(3), keep_word: true, ..Default::default() }));
            rows.push(para_pr(5, &ParaOpts { align: "LEFT", line_spacing: 130, indent: 400, keep_word: true, ..Default::default() }));
            rows.push(para_pr(6, &ParaOpts { align: "LEFT", line_spacing: 150, indent: 600, keep_word: true, ..Default::default() }));
            rows.push(para_pr(7, &ParaOpts { align: "LEFT", line_spacing: 160, indent: 600, keep_word: true, ..Default::default() }));
        }
        Some(p) => {
            let ls = p.line_spacing;
            let title_align = if p.center_title { "CENTER" } else { "LEFT" };
            rows.push(para_pr(0, &ParaOpts { line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(1, &ParaOpts { align: title_align, space_before: 400, space_after: 400, line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(2, &ParaOpts { align: "LEFT", space_before: 600, space_after: 150, line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(3, &ParaOpts { align: "LEFT", space_before: 400, space_after: 100, line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(4, &ParaOpts { align: "LEFT", space_before: 300, space_after: 100, line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(5, &ParaOpts { align: "LEFT", line_spacing: 130, indent: 400, keep_word: true, ..Default::default() }));
            rows.push(para_pr(6, &ParaOpts { align: "LEFT", line_spacing: ls, indent: 600, keep_word: true, ..Default::default() }));
            rows.push(para_pr(7, &ParaOpts { align: "LEFT", line_spacing: ls, indent: 600, keep_word: true, ..Default::default() }));
            // list levels 8..15
            for d in 0..GONGMUN_LIST_LEVELS {
                let (left, indent) = level_indent(p, d as usize);
                rows.push(para_pr(
                    GONGMUN_LIST_BASE + d,
                    &ParaOpts { align: "JUSTIFY", line_spacing: ls, left, indent, keep_word: true, ..Default::default() },
                ));
            }
            rows.push(para_pr(GONGMUN_CENTER, &ParaOpts { align: "CENTER", line_spacing: ls, keep_word: true, ..Default::default() }));
            rows.push(para_pr(GONGMUN_RIGHT, &ParaOpts { align: "RIGHT", line_spacing: ls, keep_word: true, ..Default::default() }));
        }
    }
    format!(
        "<hh:paraProperties itemCnt=\"{}\">\n{}\n    </hh:paraProperties>",
        rows.len(),
        rows.join("\n")
    )
}

fn build_numberings() -> String {
    let heads: String = (1..=7)
        .map(|i| {
            format!(
                "        <hh:paraHead start=\"1\" level=\"{i}\" align=\"LEFT\" useInstWidth=\"1\" autoIndent=\"1\" widthAdjust=\"0\" textOffsetType=\"PERCENT\" textOffset=\"50\" numFormat=\"DIGIT\" charPrIDRef=\"4294967295\" checkable=\"0\"/>"
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "<hh:numberings itemCnt=\"1\">\n\
\x20     <hh:numbering id=\"1\" start=\"0\">\n{heads}\n\x20     </hh:numbering>\n\
\x20   </hh:numberings>"
    )
}

/// The static header-shading borderFill id — 2 (general mode) or 3 (preset,
/// which adds an extra shaded entry). Pure function of `preset`, computable
/// before the header XML itself so [`super::profile::build_profile_remap`]'s
/// `border_fill_base` (`header_border_fill_id + 1`) can be derived up front.
pub fn header_border_fill_id(preset: Option<&ResolvedPreset>) -> u32 {
    if preset.is_some() {
        3
    } else {
        2
    }
}

fn build_border_fills(preset: Option<&ResolvedPreset>, extra: &[String]) -> (String, u32) {
    let thin = Some(("0.12 mm", "#000000", None));
    let mut items = vec![
        border_fill_entry(1, None, None, None, None, None),
        border_fill_entry(2, thin, thin, thin, thin, None),
    ];
    let header_bf = header_border_fill_id(preset);
    if preset.is_some() {
        items.push(border_fill_entry(3, thin, thin, thin, thin, Some("#E6E6E6")));
    }
    let item_cnt = items.len() + extra.len();
    let mut body_xml = items.join("\n");
    if !extra.is_empty() {
        body_xml.push('\n');
        body_xml.push_str(&extra.join("\n"));
    }
    let xml = format!("<hh:borderFills itemCnt=\"{item_cnt}\">\n{body_xml}\n    </hh:borderFills>");
    (xml, header_bf)
}

fn build_styles(preset: Option<&ResolvedPreset>) -> String {
    let mut items = vec![
        "<hh:style id=\"0\" type=\"PARA\" name=\"바탕글\" engName=\"Normal\" paraPrIDRef=\"0\" charPrIDRef=\"0\" nextStyleIDRef=\"0\" langIDRef=\"1042\" lockForm=\"0\"/>".to_string(),
    ];
    if preset.is_some() {
        for lvl in 1..=4u32 {
            items.push(format!(
                "<hh:style id=\"{lvl}\" type=\"PARA\" name=\"개요 {lvl}\" engName=\"Outline {lvl}\" paraPrIDRef=\"{lvl}\" charPrIDRef=\"{cp}\" nextStyleIDRef=\"0\" langIDRef=\"1042\" lockForm=\"0\"/>",
                lvl = lvl,
                cp = 4 + lvl
            ));
        }
    }
    format!(
        "<hh:styles itemCnt=\"{}\">\n      {}\n    </hh:styles>",
        items.len(),
        items.join("\n      ")
    )
}

/// Build Contents/header.xml. Returns (xml, header_border_fill_id).
/// `extra_border_fills`/`extra_char_prs` are pre-built `<hh:borderFill>`/
/// `<hh:charPr>` fragments appended after the static entries (format profile
/// remap — see [`super::profile`]); empty when no profile is in play.
pub fn generate_header_xml(
    theme: &ResolvedTheme,
    preset: Option<&ResolvedPreset>,
    extra_border_fills: &[String],
    extra_char_prs: &[String],
) -> (String, u32) {
    let body_face = "함초롬바탕";
    let char_props = build_char_properties(theme, preset, extra_char_prs);
    let para_props = build_para_properties(preset);
    let (border_fills, header_bf) = build_border_fills(preset, extra_border_fills);
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\n\
<hh:head xmlns:hh=\"{head}\" xmlns:hp=\"{para}\" xmlns:hc=\"{core}\" version=\"1.4\" secCnt=\"1\">\n\
\x20 <hh:beginNum page=\"1\" footnote=\"1\" endnote=\"1\" pic=\"1\" tbl=\"1\" equation=\"1\"/>\n\
\x20 <hh:refList>\n\
\x20   {fonts}\n\
\x20   {borders}\n\
\x20   {chars}\n\
\x20   <hh:tabProperties itemCnt=\"0\"/>\n\
\x20   {numberings}\n\
\x20   <hh:bullets itemCnt=\"0\"/>\n\
\x20   {paras}\n\
\x20   {styles}\n\
\x20 </hh:refList>\n\
\x20 <hh:compatibleDocument targetProgram=\"HWP2018\"><hh:layoutCompatibility/></hh:compatibleDocument>\n\
</hh:head>",
        head = NS_HEAD,
        para = NS_PARA,
        core = NS_CORE,
        fonts = build_font_faces(body_face),
        borders = border_fills,
        chars = char_props,
        numberings = build_numberings(),
        paras = para_props,
        styles = build_styles(preset),
    );
    (xml, header_bf)
}
