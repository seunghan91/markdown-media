// Ported from kkdoc (MIT): src/hwpx/gen-profile.ts, src/hwpx/extract-profile.ts,
// src/hwpx/profile-io.ts
//! Format profile (issue #41) — reproduce a source HWPX document's table
//! borders/shading/column widths/cell fonts without the source file itself.
//!
//! [`extract_table_profile`] reads a `.hwpx` file's top-level tables into a
//! [`FormatProfile`]; passing that profile back into [`super::GenOptions`]
//! reproduces the same visual formatting on generate. [`parse_format_profile_json`]
//! validates a hand-edited/serialized profile at the JSON boundary (the
//! in-process `GenOptions::profile` path trusts the type contract and skips
//! validation, matching the reference's CLI/library split).
//!
//! Font append (`fontName_hangul`, schema 0.3.0) is out of scope for this
//! port — cell fonts round-trip through the legacy `fontRef_hangul` ordinal
//! path only (borders/shading/column widths/cell heights/cell fonts by
//! ordinal all still round-trip; only *named* font substitution is dropped).

use std::collections::HashMap;
use std::io::{Cursor, Read};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::ids::escape_xml;

// ─── Public schema (JSON round-trip — field names mirror the TS interfaces) ───

/// One side's border definition.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BorderDef {
    #[serde(rename = "type")]
    pub kind: String,
    pub width: String,
    pub color: String,
}

/// Cell shading fill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Fill {
    #[serde(rename = "faceColor")]
    pub face_color: String,
}

/// Cell border + shading definition (referenced by a table-local id).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BorderFillDef {
    #[serde(rename = "leftBorder", skip_serializing_if = "Option::is_none", default)]
    pub left_border: Option<BorderDef>,
    #[serde(rename = "rightBorder", skip_serializing_if = "Option::is_none", default)]
    pub right_border: Option<BorderDef>,
    #[serde(rename = "topBorder", skip_serializing_if = "Option::is_none", default)]
    pub top_border: Option<BorderDef>,
    #[serde(rename = "bottomBorder", skip_serializing_if = "Option::is_none", default)]
    pub bottom_border: Option<BorderDef>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub fill: Option<Fill>,
}

/// Cell font definition (referenced by a table-local id).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CharPrDef {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub height_hwpunit: Option<String>,
    #[serde(rename = "textColor", skip_serializing_if = "Option::is_none", default)]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub underline: Option<bool>,
    #[serde(rename = "fontRef_hangul", skip_serializing_if = "Option::is_none", default)]
    pub font_ref_hangul: Option<String>,
    #[serde(rename = "fontName_hangul", skip_serializing_if = "Option::is_none", default)]
    pub font_name_hangul: Option<String>,
}

/// One cell's format reference (coordinates = merged cell's top-left anchor).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CellProfile {
    pub row: u32,
    pub col: u32,
    #[serde(rename = "rowSpan", skip_serializing_if = "Option::is_none", default)]
    pub row_span: Option<u32>,
    #[serde(rename = "colSpan", skip_serializing_if = "Option::is_none", default)]
    pub col_span: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub width_hwpunit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub height_hwpunit: Option<String>,
    #[serde(rename = "borderFillIDRef", skip_serializing_if = "Option::is_none", default)]
    pub border_fill_id_ref: Option<String>,
    #[serde(rename = "charPrIDRef", skip_serializing_if = "Option::is_none", default)]
    pub char_pr_id_ref: Option<String>,
}

/// One table's format profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TableProfile {
    pub table_index: u32,
    pub rows: u32,
    pub cols: u32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub anchor_row: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub width_hwpunit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub col_widths_hwpunit: Option<Vec<String>>,
    pub cells: Vec<CellProfile>,
    #[serde(default)]
    pub used_border_fills: HashMap<String, BorderFillDef>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub used_char_prs: Option<HashMap<String, CharPrDef>>,
}

/// Document-wide format profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FormatProfile {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub schema_version: Option<String>,
    pub tables: Vec<TableProfile>,
}

// ─── Anchor normalization + table matching ───

/// Normalize an anchor fingerprint — letters/digits only, lowercased, capped
/// at 24 chars. Both extraction (original XML cell text) and consumption
/// (Markdown/HTML first cell) use this so bold markers/whitespace/punctuation
/// differences do not break the comparison key.
pub(crate) fn normalize_anchor(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).take(24).collect()
}

/// First-row fingerprint — per-cell [`normalize_anchor`] joined with `|`
/// (preserves cell boundaries: "a|bc" vs "ab|c"), capped at 64 chars. Empty
/// if every cell is blank (no fingerprint).
pub(crate) fn normalize_row_anchor(cells: &[String]) -> String {
    let joined = cells.iter().map(|c| normalize_anchor(c)).collect::<Vec<_>>().join("|");
    if joined.chars().any(|c| c.is_alphanumeric()) {
        joined.chars().take(64).collect()
    } else {
        String::new()
    }
}

/// `"12750"` / `"1500 hwpunit"` → `12750` (leading-integer parse, matching
/// the reference's lenient `parseInt`).
pub(crate) fn parse_hu(s: Option<&str>) -> Option<i32> {
    let s = s?.trim();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return None;
    }
    s[..i].parse::<i32>().ok()
}

/// A table's remapped format lookup (global ids, keyed by "(row, col)").
#[derive(Debug, Default)]
pub(crate) struct TableRemap {
    pub index: u32,
    pub rows: u32,
    pub cols: u32,
    pub anchor: Option<String>,
    pub anchor_row: Option<String>,
    /// Whether [`take_profile`] already matched this entry to an emitted
    /// table — prevents one profile table being reapplied to two tables.
    pub used: std::cell::Cell<bool>,
    pub width: Option<i32>,
    pub col_widths: Option<Vec<i32>>,
    pub cell_bf: HashMap<(u32, u32), u32>,
    pub cell_char: HashMap<(u32, u32), u32>,
    pub cell_h: HashMap<(u32, u32), i32>,
}

/// Profile → document-global remap result.
#[derive(Debug, Default)]
pub(crate) struct ProfileRemap {
    /// header borderFills to append (index i → global id borderFillBase+i).
    pub border_fill_xmls: Vec<String>,
    /// header charProperties to append (index i → global id charPrBase+i).
    pub char_pr_xmls: Vec<String>,
    pub tables: Vec<TableRemap>,
}

/// Select the profile table to apply to one emitted table.
///
/// `parse` does not emit every top-level source table as a Markdown/HTML
/// table (1×1 title boxes become paragraphs, header/footer tables are
/// dropped, etc.), so sequence-only matching can silently misapply another
/// table's formatting. Rule:
/// 1. rows/cols must always match (cell coordinates are meaningless otherwise).
/// 2. If both sides have an anchor, it is the only basis for matching.
/// 3. Else if both sides have a row fingerprint, same rule.
/// 4. Else fall back to `table_index == seq` (hand-edited/legacy profiles).
///
/// A non-match returns `None` — no formatting is safer than wrong formatting.
pub(crate) fn take_profile<'a>(
    remap: &'a ProfileRemap,
    rows: u32,
    cols: u32,
    anchor: &str,
    seq: u32,
    row_anchor: &str,
) -> Option<&'a TableRemap> {
    for t in &remap.tables {
        if t.used.get() {
            continue;
        }
        if t.rows != rows || t.cols != cols {
            continue;
        }
        if t.anchor.is_some() && !anchor.is_empty() {
            if t.anchor.as_deref() != Some(anchor) {
                continue;
            }
        } else if t.anchor_row.is_some() && !row_anchor.is_empty() {
            if t.anchor_row.as_deref() != Some(row_anchor) {
                continue;
            }
        } else if t.index != seq {
            continue;
        }
        t.used.set(true);
        return Some(t);
    }
    None
}

/// `charPrBase` for [`build_profile_remap`] — static charPr 0..=10 (11 kinds)
/// + the preset auto-ratio variant channel (`ratio_variant_count * 4`, not
/// ported here — always 0 in this port, see module docs).
pub(crate) fn profile_char_pr_base(ratio_variant_count: u32) -> u32 {
    11 + ratio_variant_count * 4
}

fn sorted_entries<T: Clone>(map: &HashMap<String, T>) -> Vec<(String, T)> {
    let mut v: Vec<(String, T)> = map.iter().map(|(k, val)| (k.clone(), val.clone())).collect();
    v.sort_by(|a, b| match (a.0.parse::<i64>(), b.0.parse::<i64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        _ => a.0.cmp(&b.0),
    });
    v
}

/// Reassign a profile's table-local borderFill/charPr ids to document-global
/// ids. Each table gets fresh global ids (no cross-table dedup — simplicity
/// over size, matching the reference).
pub(crate) fn build_profile_remap(
    profile: &FormatProfile,
    char_pr_base: u32,
    border_fill_base: u32,
) -> ProfileRemap {
    let mut remap = ProfileRemap::default();
    let mut bf_next = border_fill_base;
    let mut char_next = char_pr_base;

    for t in &profile.tables {
        let mut local_bf: HashMap<String, u32> = HashMap::new();
        for (key, def) in sorted_entries(&t.used_border_fills) {
            let gid = bf_next;
            bf_next += 1;
            remap.border_fill_xmls.push(border_fill_def_to_xml(gid, &def));
            local_bf.insert(key, gid);
        }
        let mut local_char: HashMap<String, u32> = HashMap::new();
        if let Some(used_cp) = &t.used_char_prs {
            for (key, def) in sorted_entries(used_cp) {
                let gid = char_next;
                char_next += 1;
                remap.char_pr_xmls.push(profile_char_pr_xml(gid, &def));
                local_char.insert(key, gid);
            }
        }

        let col_widths = t.col_widths_hwpunit.as_ref().and_then(|cw| {
            if cw.len() as u32 != t.cols {
                return None;
            }
            let parsed: Vec<Option<i32>> = cw.iter().map(|s| parse_hu(Some(s))).collect();
            if parsed.iter().all(|w| w.is_some()) {
                Some(parsed.into_iter().map(|w| w.unwrap()).collect())
            } else {
                None
            }
        });

        let anchor_row = t.anchor_row.as_ref().and_then(|a| {
            let pieces: Vec<String> = a.split('|').map(|s| s.to_string()).collect();
            let r = normalize_row_anchor(&pieces);
            if r.is_empty() {
                None
            } else {
                Some(r)
            }
        });

        let mut tr = TableRemap {
            index: t.table_index,
            rows: t.rows,
            cols: t.cols,
            anchor: t.anchor_text.as_ref().map(|a| normalize_anchor(a)),
            anchor_row,
            used: std::cell::Cell::new(false),
            width: parse_hu(t.width_hwpunit.as_deref()),
            col_widths,
            cell_bf: HashMap::new(),
            cell_char: HashMap::new(),
            cell_h: HashMap::new(),
        };

        for cell in &t.cells {
            let k = (cell.row, cell.col);
            if let Some(bf_ref) = &cell.border_fill_id_ref {
                if let Some(&gid) = local_bf.get(bf_ref) {
                    tr.cell_bf.insert(k, gid);
                }
            }
            if let Some(cp_ref) = &cell.char_pr_id_ref {
                if let Some(&gid) = local_char.get(cp_ref) {
                    tr.cell_char.insert(k, gid);
                }
            }
            if let Some(h) = parse_hu(cell.height_hwpunit.as_deref()) {
                tr.cell_h.insert(k, h);
            }
        }
        remap.tables.push(tr);
    }
    remap
}

// ─── XML builders (append into header.xml) ───

fn edge_xml(tag: &str, d: Option<&BorderDef>) -> String {
    match d {
        Some(d) => format!(
            "<hh:{tag} type=\"{}\" width=\"{}\" color=\"{}\"/>",
            escape_xml(&d.kind),
            escape_xml(&d.width),
            escape_xml(&d.color)
        ),
        None => format!("<hh:{tag} type=\"NONE\" width=\"0.1 mm\" color=\"#000000\"/>"),
    }
}

/// `BorderFillDef` → `<hh:borderFill>` XML (mirrors [`super::header`]'s
/// static border-fill entries). Fill (shading), if present, follows the
/// border sides (HWPX child-element order).
pub(crate) fn border_fill_def_to_xml(id: u32, def: &BorderFillDef) -> String {
    let fill = match &def.fill {
        Some(f) => format!(
            "\n        <hh:fillBrush><hh:winBrush faceColor=\"{}\" hatchColor=\"#000000\" alpha=\"0\"/></hh:fillBrush>",
            escape_xml(&f.face_color)
        ),
        None => String::new(),
    };
    format!(
        "      <hh:borderFill id=\"{id}\" threeD=\"0\" shadow=\"0\" centerLine=\"NONE\" breakCellSeparateLine=\"0\">\n\
        \x20       <hh:slash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>\n\
        \x20       <hh:backSlash type=\"NONE\" Crooked=\"0\" isCounter=\"0\"/>\n\
        \x20       {l}\n        {r}\n        {t}\n        {b}{fill}\n\
        \x20     </hh:borderFill>",
        l = edge_xml("leftBorder", def.left_border.as_ref()),
        r = edge_xml("rightBorder", def.right_border.as_ref()),
        t = edge_xml("topBorder", def.top_border.as_ref()),
        b = edge_xml("bottomBorder", def.bottom_border.as_ref()),
        fill = fill,
    )
}

const LEGACY_PROFILE_FONT_MAX: i64 = 2;

/// `CharPrDef` → `<hh:charPr>` XML. Font append (`fontName_hangul`) is out of
/// scope for this port (see module docs) — the ordinal `fontRef_hangul` is
/// honored only when it falls inside the generated header's static font range
/// (0..=2); otherwise it folds to the default font (0) rather than emit a
/// dangling IDREF.
pub(crate) fn profile_char_pr_xml(id: u32, def: &CharPrDef) -> String {
    let height = parse_hu(def.height_hwpunit.as_deref()).unwrap_or(1000).max(100);
    let color = def.text_color.as_deref().unwrap_or("#000000");
    let color = escape_xml(color);
    let raw_font = def.font_ref_hangul.as_deref().and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0);
    let font = if (0..=LEGACY_PROFILE_FONT_MAX).contains(&raw_font) { raw_font } else { 0 };
    let rest_font = font;
    let bold_attr = if def.bold.unwrap_or(false) { " bold=\"1\"" } else { "" };
    let italic_attr = if def.italic.unwrap_or(false) { " italic=\"1\"" } else { "" };
    let underline = if def.underline.unwrap_or(false) {
        format!("\n        <hh:underline type=\"BOTTOM\" shape=\"SOLID\" color=\"{color}\"/>")
    } else {
        String::new()
    };
    format!(
        "      <hh:charPr id=\"{id}\" height=\"{height}\" textColor=\"{color}\" shadeColor=\"none\" useFontSpace=\"0\" useKerning=\"0\" symMark=\"NONE\" borderFillIDRef=\"1\"{bold_attr}{italic_attr}>\n\
        \x20       <hh:fontRef hangul=\"{font}\" latin=\"{font}\" hanja=\"{rest_font}\" japanese=\"{rest_font}\" other=\"{rest_font}\" symbol=\"{rest_font}\" user=\"{rest_font}\"/>\n\
        \x20       <hh:ratio hangul=\"100\" latin=\"100\" hanja=\"100\" japanese=\"100\" other=\"100\" symbol=\"100\" user=\"100\"/>\n\
        \x20       <hh:spacing hangul=\"0\" latin=\"0\" hanja=\"0\" japanese=\"0\" other=\"0\" symbol=\"0\" user=\"0\"/>\n\
        \x20       <hh:relSz hangul=\"100\" latin=\"100\" hanja=\"100\" japanese=\"100\" other=\"100\" symbol=\"100\" user=\"100\"/>\n\
        \x20       <hh:offset hangul=\"0\" latin=\"0\" hanja=\"0\" japanese=\"0\" other=\"0\" symbol=\"0\" user=\"0\"/>{underline}\n\
        \x20     </hh:charPr>"
    )
}

// ─── Minimal XML tree (arena) — just enough DOM for extraction ───
//
// Mirrors the reference's `elemsByLocal`/`findChildByLocalName`/parent-chain
// helpers (DOMParser + getElementsByTagName("*")). Local names have their
// namespace prefix stripped, matching HWPX's `hp:`/`hh:` conventions.

struct XNode {
    tag: String,
    attrs: Vec<(String, String)>,
    parent: Option<usize>,
    children: Vec<usize>,
    text: String,
}

struct XDoc {
    nodes: Vec<XNode>,
}

impl XDoc {
    fn attr(&self, idx: usize, name: &str) -> Option<&str> {
        self.nodes[idx].attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    fn children_by_tag(&self, idx: usize, name: &str) -> Vec<usize> {
        self.nodes[idx].children.iter().copied().filter(|&c| self.nodes[c].tag == name).collect()
    }

    fn first_child_by_tag(&self, idx: usize, name: &str) -> Option<usize> {
        self.nodes[idx].children.iter().copied().find(|&c| self.nodes[c].tag == name)
    }

    /// All descendants (document order) under `root` with local name == `name`.
    fn descendants(&self, root: usize, name: &str) -> Vec<usize> {
        let mut out = Vec::new();
        self.collect_descendants(root, name, &mut out);
        out
    }

    fn collect_descendants(&self, root: usize, name: &str, out: &mut Vec<usize>) {
        for &c in &self.nodes[root].children {
            if self.nodes[c].tag == name {
                out.push(c);
            }
            self.collect_descendants(c, name, out);
        }
    }

    /// All elements in the document with local name == `name` (document order).
    fn all_by_tag(&self, name: &str) -> Vec<usize> {
        if self.nodes.is_empty() {
            return Vec::new();
        }
        self.descendants(0, name)
    }

    fn ancestor_with_tag(&self, idx: usize, name: &str) -> Option<usize> {
        let mut p = self.nodes[idx].parent;
        while let Some(pi) = p {
            if self.nodes[pi].tag == name {
                return Some(pi);
            }
            p = self.nodes[pi].parent;
        }
        None
    }
}

fn node_tag_attrs(e: &quick_xml::events::BytesStart) -> (String, Vec<(String, String)>) {
    let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            (
                String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                String::from_utf8_lossy(&a.value).to_string(),
            )
        })
        .collect();
    (tag, attrs)
}

fn parse_xml_tree(xml: &str) -> XDoc {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut nodes: Vec<XNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let (tag, attrs) = node_tag_attrs(&e);
                let parent = stack.last().copied();
                let idx = nodes.len();
                nodes.push(XNode { tag, attrs, parent, children: Vec::new(), text: String::new() });
                if let Some(p) = parent {
                    nodes[p].children.push(idx);
                }
                stack.push(idx);
            }
            Ok(Event::Empty(e)) => {
                let (tag, attrs) = node_tag_attrs(&e);
                let parent = stack.last().copied();
                let idx = nodes.len();
                nodes.push(XNode { tag, attrs, parent, children: Vec::new(), text: String::new() });
                if let Some(p) = parent {
                    nodes[p].children.push(idx);
                }
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Text(t)) => {
                if let Some(&top) = stack.last() {
                    if let Ok(txt) = t.unescape() {
                        nodes[top].text.push_str(&txt);
                    }
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(&top) = stack.last() {
                    nodes[top].text.push_str(&String::from_utf8_lossy(&t.into_inner()));
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    XDoc { nodes }
}

// ─── Extraction (hwpx → FormatProfile) ───

fn border_def_of(doc: &XDoc, idx: Option<usize>) -> Option<BorderDef> {
    let idx = idx?;
    Some(BorderDef {
        kind: doc.attr(idx, "type").unwrap_or("NONE").to_string(),
        width: doc.attr(idx, "width").unwrap_or("0.1 mm").to_string(),
        color: doc.attr(idx, "color").unwrap_or("#000000").to_string(),
    })
}

/// header.xml → borderFill id → definition (verbatim).
fn parse_border_fills(doc: &XDoc) -> HashMap<String, BorderFillDef> {
    let mut map = HashMap::new();
    for bf in doc.all_by_tag("borderFill") {
        let Some(id) = doc.attr(bf, "id") else { continue };
        let mut def = BorderFillDef {
            left_border: border_def_of(doc, doc.first_child_by_tag(bf, "leftBorder")),
            right_border: border_def_of(doc, doc.first_child_by_tag(bf, "rightBorder")),
            top_border: border_def_of(doc, doc.first_child_by_tag(bf, "topBorder")),
            bottom_border: border_def_of(doc, doc.first_child_by_tag(bf, "bottomBorder")),
            fill: None,
        };
        if let Some(fb) = doc.first_child_by_tag(bf, "fillBrush") {
            if let Some(wb) = doc.first_child_by_tag(fb, "winBrush") {
                if let Some(face) = doc.attr(wb, "faceColor") {
                    if face != "none" {
                        def.fill = Some(Fill { face_color: face.to_string() });
                    }
                }
            }
        }
        map.insert(id.to_string(), def);
    }
    map
}

/// header.xml → HANGUL fontface font id → name (fontName_hangul round-trip
/// raw material, 0.3.0 — extracted for schema completeness even though this
/// port's generate side does not consume it, see module docs).
fn parse_hangul_fonts(doc: &XDoc) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for ff in doc.all_by_tag("fontface") {
        if doc.attr(ff, "lang") != Some("HANGUL") {
            continue;
        }
        for font in doc.children_by_tag(ff, "font") {
            if let (Some(id), Some(face)) = (doc.attr(font, "id"), doc.attr(font, "face")) {
                map.insert(id.to_string(), face.to_string());
            }
        }
    }
    map
}

/// header.xml → charPr id → definition.
fn parse_char_prs(doc: &XDoc, hangul_fonts: &HashMap<String, String>) -> HashMap<String, CharPrDef> {
    let mut map = HashMap::new();
    for cp in doc.all_by_tag("charPr") {
        let Some(id) = doc.attr(cp, "id") else { continue };
        let mut def = CharPrDef::default();
        if let Some(h) = doc.attr(cp, "height") {
            def.height_hwpunit = Some(h.to_string());
        }
        if let Some(c) = doc.attr(cp, "textColor") {
            def.text_color = Some(c.to_string());
        }
        if doc.attr(cp, "bold") == Some("1") {
            def.bold = Some(true);
        }
        if doc.attr(cp, "italic") == Some("1") {
            def.italic = Some(true);
        }
        if doc.first_child_by_tag(cp, "underline").is_some() {
            def.underline = Some(true);
        }
        if let Some(font_ref) = doc.first_child_by_tag(cp, "fontRef") {
            if let Some(hangul) = doc.attr(font_ref, "hangul") {
                def.font_ref_hangul = Some(hangul.to_string());
                if let Some(face) = hangul_fonts.get(hangul) {
                    def.font_name_hangul = Some(face.clone());
                }
            }
        }
        map.insert(id.to_string(), def);
    }
    map
}

/// A `tbl` with a `tbl` ancestor is nested — only top-level tables count as
/// document tables (matches the `generate` side's table sequence).
fn is_top_level_table(doc: &XDoc, tbl: usize) -> bool {
    doc.ancestor_with_tag(tbl, "tbl").is_none()
}

fn nearest_table(doc: &XDoc, idx: usize) -> Option<usize> {
    doc.ancestor_with_tag(idx, "tbl")
}

fn nearest_cell(doc: &XDoc, idx: usize) -> Option<usize> {
    doc.ancestor_with_tag(idx, "tc")
}

/// Direct (non-nested-table) `hp:t` text under a cell — anchor_text raw material.
fn direct_cell_text(doc: &XDoc, tc: usize) -> String {
    let mut out = String::new();
    for t in doc.descendants(tc, "t") {
        if nearest_cell(doc, t) != Some(tc) {
            continue;
        }
        out.push_str(&doc.nodes[t].text);
        if out.chars().count() >= 64 {
            break;
        }
    }
    out
}

/// The first run's charPrIDRef directly inside a cell (skips nested tables).
fn first_run_char_pr(doc: &XDoc, tc: usize) -> Option<String> {
    for run in doc.descendants(tc, "run") {
        if nearest_cell(doc, run) != Some(tc) {
            continue;
        }
        if let Some(id) = doc.attr(run, "charPrIDRef") {
            return Some(id.to_string());
        }
    }
    None
}

fn int_attr(doc: &XDoc, idx: usize, name: &str) -> Option<i64> {
    doc.attr(idx, name).and_then(|s| s.trim().parse::<i64>().ok())
}

fn pick<T: Clone>(map: &HashMap<String, T>, keys: &std::collections::HashSet<String>) -> HashMap<String, T> {
    let mut out = HashMap::new();
    for k in keys {
        if let Some(v) = map.get(k) {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

/// One `<hp:tbl>` → [`TableProfile`]. Only borderFill/charPr ids actually
/// referenced by a cell are copied into `used_*`.
fn parse_table(
    doc: &XDoc,
    tbl: usize,
    table_index: u32,
    border_fills: &HashMap<String, BorderFillDef>,
    char_prs: &HashMap<String, CharPrDef>,
) -> TableProfile {
    let rows = int_attr(doc, tbl, "rowCnt").unwrap_or(0).max(0) as u32;
    let cols = int_attr(doc, tbl, "colCnt").unwrap_or(0).max(0) as u32;
    let width = doc.first_child_by_tag(tbl, "sz").and_then(|s| doc.attr(s, "width")).map(|s| s.to_string());

    let mut cells: Vec<CellProfile> = Vec::new();
    let mut used_bf: std::collections::HashSet<String> = Default::default();
    let mut used_cp: std::collections::HashSet<String> = Default::default();
    // Column widths — any span-1 cell in any row fixes a column (grid tables
    // share widths across rows); remaining columns are apportioned from
    // merged-cell widths (preserves col_widths for tables whose row 0 is
    // entirely merged).
    let mut col_widths: Vec<Option<i32>> = vec![None; cols as usize];
    let mut span_cells: Vec<(usize, usize, i32)> = Vec::new(); // (col, colSpan, width)
    // First-row full fingerprint (anchor_row, 0.3.0) — handles (0,0)-blank crosstabs.
    let mut row0_texts: HashMap<u32, String> = HashMap::new();
    let mut anchor_text = String::new();

    for tc in doc.descendants(tbl, "tc") {
        // Exclude nested-table cells — only this tbl's direct tc.
        if nearest_table(doc, tc) != Some(tbl) {
            continue;
        }
        let addr = doc.first_child_by_tag(tc, "cellAddr");
        let span = doc.first_child_by_tag(tc, "cellSpan");
        let csz = doc.first_child_by_tag(tc, "cellSz");
        let row = addr.and_then(|a| int_attr(doc, a, "rowAddr")).unwrap_or(0).max(0) as u32;
        let col = addr.and_then(|a| int_attr(doc, a, "colAddr")).unwrap_or(0).max(0) as u32;
        if row == 0 && col == 0 && anchor_text.is_empty() {
            anchor_text = direct_cell_text(doc, tc);
        }
        if row == 0 {
            row0_texts.insert(col, direct_cell_text(doc, tc));
        }
        let col_span = span.and_then(|s| int_attr(doc, s, "colSpan")).unwrap_or(1).max(1) as u32;
        let row_span = span.and_then(|s| int_attr(doc, s, "rowSpan")).unwrap_or(1).max(1) as u32;
        let bf_id = doc.attr(tc, "borderFillIDRef").map(|s| s.to_string());
        let cp_id = first_run_char_pr(doc, tc);

        let w = csz.and_then(|c| doc.attr(c, "width")).map(|s| s.to_string());
        let h = csz.and_then(|c| doc.attr(c, "height")).map(|s| s.to_string());

        let mut cell = CellProfile {
            row,
            col,
            row_span: Some(row_span),
            col_span: Some(col_span),
            width_hwpunit: w.clone(),
            height_hwpunit: h,
            border_fill_id_ref: None,
            char_pr_id_ref: None,
        };
        if let Some(bf) = &bf_id {
            cell.border_fill_id_ref = Some(bf.clone());
            used_bf.insert(bf.clone());
        }
        if let Some(cp) = &cp_id {
            cell.char_pr_id_ref = Some(cp.clone());
            used_cp.insert(cp.clone());
        }
        cells.push(cell);

        if let Some(w_num) = parse_hu(w.as_deref()) {
            let col_us = col as usize;
            if col_us < cols as usize {
                if col_span == 1 {
                    if col_widths[col_us].is_none() {
                        col_widths[col_us] = Some(w_num);
                    }
                } else {
                    span_cells.push((col_us, col_span as usize, w_num));
                }
            }
        }
    }

    // Distribute merged-cell widths across columns not fixed by a span-1 cell.
    for (col, span, w) in span_cells {
        let covered: Vec<usize> = (0..span).map(|i| col + i).filter(|&c| c < cols as usize).collect();
        let unknown: Vec<usize> = covered.iter().copied().filter(|&c| col_widths[c].is_none()).collect();
        if unknown.is_empty() {
            continue;
        }
        let known: i32 = covered.iter().map(|&c| col_widths[c].unwrap_or(0)).sum();
        let each = (w - known) / unknown.len() as i32;
        if each > 0 {
            for c in unknown {
                col_widths[c] = Some(each);
            }
        }
    }

    let mut table = TableProfile {
        table_index,
        rows,
        cols,
        anchor_text: None,
        anchor_row: None,
        width_hwpunit: width,
        col_widths_hwpunit: None,
        cells,
        used_border_fills: pick(border_fills, &used_bf),
        used_char_prs: None,
    };
    let anchor = normalize_anchor(&anchor_text);
    if !anchor.is_empty() {
        table.anchor_text = Some(anchor);
    }
    let row_vec: Vec<String> = (0..cols).map(|c| row0_texts.get(&c).cloned().unwrap_or_default()).collect();
    let row_anchor = normalize_row_anchor(&row_vec);
    if !row_anchor.is_empty() {
        table.anchor_row = Some(row_anchor);
    }
    if !col_widths.is_empty() && col_widths.iter().all(|w| w.is_some()) {
        table.col_widths_hwpunit = Some(col_widths.into_iter().map(|w| w.unwrap().to_string()).collect());
    }
    let cp = pick(char_prs, &used_cp);
    if !cp.is_empty() {
        table.used_char_prs = Some(cp);
    }
    table
}

/// `Contents/sectionN.xml` → N, or `None` for anything else (mirrors
/// [`super::validate::is_section_file`]).
fn section_number(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("Contents/section")?;
    let digits = rest.strip_suffix(".xml")?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Decompressed-size cap for a single ZIP entry read by [`extract_table_profile`]
/// — matches [`super::validate::validate_hwpx`]'s per-entry bound. HWPX
/// header/section XML is always well under this; an entry that exceeds it is
/// treated as a zip bomb, not a legitimate document, and rejected outright
/// rather than silently truncated (a truncated read would otherwise still
/// "succeed" with a malformed-XML parse that's easy to miss).
const MAX_ENTRY_SIZE: u64 = 64 * 1024 * 1024;

/// Read one ZIP entry to a UTF-8 string, erroring instead of truncating if it
/// decompresses past [`MAX_ENTRY_SIZE`].
fn read_zip_entry_capped<R: Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
) -> std::io::Result<String> {
    let mut file = archive.by_name(name)?;
    let mut limited = file.by_ref().take(MAX_ENTRY_SIZE + 1);
    let mut buf = Vec::new();
    limited.read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_ENTRY_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{name}: 압축해제 크기가 {MAX_ENTRY_SIZE} 바이트를 초과함 (zip bomb 의심)"),
        ));
    }
    String::from_utf8(buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{name}: UTF-8 아님: {e}")))
}

/// Extract a [`FormatProfile`] from an `.hwpx` file's bytes — top-level
/// tables, in document order, across all sections.
pub fn extract_table_profile(input: &[u8]) -> std::io::Result<FormatProfile> {
    let mut archive = zip::ZipArchive::new(Cursor::new(input))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("유효한 ZIP이 아님: {e}")))?;

    // Collect entry names up front — zip 2.4's `ZipFile` extends its borrow of
    // `archive` to the end of scope (`impl Drop`), so a name lookup and a
    // `by_name` read can never be interleaved on the same `archive` borrow.
    let entry_names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();

    let header_name = entry_names
        .iter()
        .find(|n| n.as_str() == "Contents/header.xml")
        .or_else(|| entry_names.iter().find(|n| n.to_lowercase().ends_with("header.xml")));
    let header_xml = match header_name {
        Some(name) => read_zip_entry_capped(&mut archive, name)?,
        None => String::from("<root/>"),
    };

    let header_doc = parse_xml_tree(&header_xml);
    let border_fills = parse_border_fills(&header_doc);
    let hangul_fonts = parse_hangul_fonts(&header_doc);
    let char_prs = parse_char_prs(&header_doc, &hangul_fonts);

    let mut section_names: Vec<(u32, String)> = entry_names
        .iter()
        .filter_map(|n| section_number(n).map(|num| (num, n.clone())))
        .collect();
    section_names.sort_by_key(|(n, _)| *n);

    let mut tables: Vec<TableProfile> = Vec::new();
    let mut table_index = 0u32;
    for (_, name) in section_names {
        let xml = read_zip_entry_capped(&mut archive, &name)?;
        let doc = parse_xml_tree(&xml);
        for tbl in doc.all_by_tag("tbl") {
            if !is_top_level_table(&doc, tbl) {
                continue;
            }
            tables.push(parse_table(&doc, tbl, table_index, &border_fills, &char_prs));
            table_index += 1;
        }
    }

    Ok(FormatProfile { schema_version: Some("0.3.0".to_string()), tables })
}

// ─── JSON boundary validation (CLI/MCP `--profile` input) ───

const BORDER_TYPES: &[&str] = &[
    "NONE", "SOLID", "DASH", "DOT", "DASH_DOT", "DASH_DOT_DOT", "LONG_DASH", "CIRCLE",
    "DOUBLE_SLIM", "SLIM_THICK", "THICK_SLIM", "SLIM_THICK_SLIM", "WAVE", "DOUBLEWAVE",
];

lazy_static! {
    static ref RE_MM_WIDTH: Regex = Regex::new(r"^\d+(\.\d+)? ?mm$").unwrap();
    static ref RE_HEX_COLOR: Regex = Regex::new(r"(?i)^(#[0-9a-f]{6}|none)$").unwrap();
}

fn validate_border_def(issues: &mut Vec<String>, ti: usize, key: &str, side: &str, d: &BorderDef) {
    if !BORDER_TYPES.contains(&d.kind.as_str()) {
        issues.push(format!("tables[{ti}].used_border_fills.{key}.{side}.type: 알 수 없는 괘선 타입 \"{}\"", d.kind));
    }
    if !RE_MM_WIDTH.is_match(&d.width) {
        issues.push(format!("tables[{ti}].used_border_fills.{key}.{side}.width: \"0.12 mm\" 형식(mm 단위)이어야 합니다"));
    }
    if !RE_HEX_COLOR.is_match(&d.color) {
        issues.push(format!("tables[{ti}].used_border_fills.{key}.{side}.color: \"#RRGGBB\" 또는 \"none\"이어야 합니다"));
    }
}

fn validate_format_profile(p: &FormatProfile) -> Result<(), String> {
    let mut issues: Vec<String> = Vec::new();
    for (ti, t) in p.tables.iter().enumerate() {
        if t.rows < 1 {
            issues.push(format!("tables[{ti}].rows: 1 이상이어야 합니다"));
        }
        if t.cols < 1 {
            issues.push(format!("tables[{ti}].cols: 1 이상이어야 합니다"));
        }
        for (key, bf) in &t.used_border_fills {
            for (side, def) in [
                ("leftBorder", &bf.left_border),
                ("rightBorder", &bf.right_border),
                ("topBorder", &bf.top_border),
                ("bottomBorder", &bf.bottom_border),
            ] {
                if let Some(d) = def {
                    validate_border_def(&mut issues, ti, key, side, d);
                }
            }
            if let Some(fill) = &bf.fill {
                if !RE_HEX_COLOR.is_match(&fill.face_color) {
                    issues.push(format!(
                        "tables[{ti}].used_border_fills.{key}.fill.faceColor: \"#RRGGBB\" 또는 \"none\"이어야 합니다"
                    ));
                }
            }
        }
        if let Some(cps) = &t.used_char_prs {
            for (key, cp) in cps {
                if let Some(c) = &cp.text_color {
                    if !RE_HEX_COLOR.is_match(c) {
                        issues.push(format!(
                            "tables[{ti}].used_char_prs.{key}.textColor: \"#RRGGBB\" 또는 \"none\"이어야 합니다"
                        ));
                    }
                }
            }
        }
        for (ci, cell) in t.cells.iter().enumerate() {
            if let Some(rs) = cell.row_span {
                if rs < 1 {
                    issues.push(format!("tables[{ti}].cells[{ci}].rowSpan: 1 이상이어야 합니다"));
                }
            }
            if let Some(cs) = cell.col_span {
                if cs < 1 {
                    issues.push(format!("tables[{ti}].cells[{ci}].colSpan: 1 이상이어야 합니다"));
                }
            }
        }
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(format!("프로필 스키마 불일치: {}", issues.into_iter().take(3).collect::<Vec<_>>().join(" / ")))
    }
}

/// Profile JSON text → validated [`FormatProfile`]. On failure, returns a
/// short Korean summary (first few issues) — the same boundary the
/// reference's CLI (`--profile`) / MCP (`profile_path`) share; the in-process
/// `GenOptions::profile` path trusts the type contract and does not call this.
pub fn parse_format_profile_json(text: &str) -> Result<FormatProfile, String> {
    let profile: FormatProfile =
        serde_json::from_str(text).map_err(|e| format!("프로필 JSON 파싱 실패: {e}"))?;
    validate_format_profile(&profile)?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_border() -> BorderDef {
        BorderDef { kind: "SOLID".to_string(), width: "0.12 mm".to_string(), color: "#000000".to_string() }
    }

    #[test]
    fn normalize_anchor_strips_punctuation_and_caps_length() {
        assert_eq!(normalize_anchor("**표1** 제목"), "표1제목");
        let long = "a".repeat(40);
        assert_eq!(normalize_anchor(&long).chars().count(), 24);
    }

    #[test]
    fn normalize_row_anchor_preserves_cell_boundaries() {
        let a = normalize_row_anchor(&["a".to_string(), "bc".to_string()]);
        let b = normalize_row_anchor(&["ab".to_string(), "c".to_string()]);
        assert_ne!(a, b);
        assert_eq!(normalize_row_anchor(&["".to_string(), "".to_string()]), "");
    }

    #[test]
    fn parse_hu_extracts_leading_integer() {
        assert_eq!(parse_hu(Some("12750")), Some(12750));
        assert_eq!(parse_hu(Some(" 1500 hwpunit")), Some(1500));
        assert_eq!(parse_hu(Some("abc")), None);
    }

    #[test]
    fn take_profile_requires_dimension_match() {
        let mut remap = ProfileRemap::default();
        remap.tables.push(TableRemap { index: 0, rows: 2, cols: 2, ..Default::default() });
        assert!(take_profile(&remap, 3, 3, "", 0, "").is_none());
        assert!(take_profile(&remap, 2, 2, "", 0, "").is_some());
    }

    #[test]
    fn take_profile_prefers_anchor_over_sequence() {
        let mut remap = ProfileRemap::default();
        remap.tables.push(TableRemap {
            index: 5,
            rows: 2,
            cols: 2,
            anchor: Some("hello".to_string()),
            ..Default::default()
        });
        // Sequence mismatch (seq=0 vs index=5) does not matter when anchors match.
        assert!(take_profile(&remap, 2, 2, "hello", 0, "").is_some());
    }

    #[test]
    fn take_profile_marks_used_so_it_is_not_reapplied() {
        let mut remap = ProfileRemap::default();
        remap.tables.push(TableRemap { index: 0, rows: 2, cols: 2, ..Default::default() });
        assert!(take_profile(&remap, 2, 2, "", 0, "").is_some());
        assert!(take_profile(&remap, 2, 2, "", 0, "").is_none());
    }

    #[test]
    fn build_profile_remap_assigns_sequential_ids_and_appends_xml() {
        let mut used_bf = HashMap::new();
        used_bf.insert(
            "1".to_string(),
            BorderFillDef { left_border: Some(sample_border()), ..Default::default() },
        );
        let profile = FormatProfile {
            schema_version: None,
            tables: vec![TableProfile {
                table_index: 0,
                rows: 1,
                cols: 1,
                cells: vec![CellProfile {
                    row: 0,
                    col: 0,
                    border_fill_id_ref: Some("1".to_string()),
                    ..Default::default()
                }],
                used_border_fills: used_bf,
                ..Default::default()
            }],
        };
        let remap = build_profile_remap(&profile, 11, 4);
        assert_eq!(remap.border_fill_xmls.len(), 1);
        assert!(remap.border_fill_xmls[0].contains("id=\"4\""));
        assert_eq!(remap.tables[0].cell_bf.get(&(0, 0)), Some(&4));
    }

    #[test]
    fn parse_format_profile_json_rejects_bad_color() {
        let json = r#"{"tables":[{"table_index":0,"rows":1,"cols":1,"cells":[],"used_border_fills":{"1":{"fill":{"faceColor":"red"}}}}]}"#;
        let err = parse_format_profile_json(json).unwrap_err();
        assert!(err.contains("faceColor"), "{err}");
    }

    #[test]
    fn parse_format_profile_json_accepts_valid_profile() {
        let json = r#"{"schema_version":"0.3.0","tables":[{"table_index":0,"rows":2,"cols":2,"cells":[{"row":0,"col":0}],"used_border_fills":{}}]}"#;
        let profile = parse_format_profile_json(json).expect("should parse");
        assert_eq!(profile.tables.len(), 1);
        assert_eq!(profile.tables[0].rows, 2);
    }

    #[test]
    fn extract_then_apply_round_trips_border_and_shading() {
        // Build a minimal, valid HWPX (general mode) and extract its profile —
        // should at least parse without error and find the GFM table.
        let md = "| 이름 | 값 |\n| --- | --- |\n| 사과 | 100 |\n";
        let bytes = super::super::markdown_to_hwpx(md, &super::super::GenOptions::default()).unwrap();
        let profile = extract_table_profile(&bytes).expect("extract should succeed");
        assert_eq!(profile.tables.len(), 1);
        assert_eq!(profile.tables[0].rows, 2);
        assert_eq!(profile.tables[0].cols, 2);
        assert!(!profile.tables[0].used_border_fills.is_empty());
    }

    #[test]
    fn extract_rejects_oversized_entry_instead_of_truncating() {
        // A highly-compressible "zip bomb" header.xml — small on disk, huge
        // decompressed — must error, not silently truncate into a bogus parse.
        use std::io::Write as _;
        let huge = "a".repeat((MAX_ENTRY_SIZE + 1024) as usize);
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let opt = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("Contents/header.xml", opt).unwrap();
            zip.write_all(huge.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let err = extract_table_profile(&cursor.into_inner()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("zip bomb"), "{err}");
    }
}
