// Ported from kkdoc (MIT): src/roundtrip/source-map.ts (byte-offset splice model)
//! Section-XML source map — walks `<hp:tbl>/<hp:tr>/<hp:tc>/<hp:subList>/<hp:p>/
//! <hp:run>/<hp:t>` recording byte offsets so text can be spliced in place while
//! every unchanged byte (attributes, formatting, sibling elements) is preserved.
//!
//! HWPX nesting reality (verified against real 행정안전부 fixtures):
//!   - tables live inside a run inside a paragraph (`<hp:p><hp:run><hp:tbl>`)
//!   - `<hp:tbl>` → `<hp:tr>` → `<hp:tc>` → `<hp:subList>` → `<hp:p>` …
//!   - a cell's own `<hp:cellAddr>` / `<hp:cellSpan>` are direct children of the
//!     `<hp:tc>`, appearing *after* the cell's `</hp:subList>`.

/// Inner text byte range of a single `<hp:t>…</hp:t>` run.
#[derive(Debug, Clone)]
pub struct TextRun {
    pub start: usize,
    pub end: usize,
}

/// One paragraph, with the byte offsets needed to splice or inject text.
#[derive(Debug, Clone)]
pub struct Para {
    pub runs: Vec<TextRun>,
    /// Byte offset just after the first `<hp:run …>` open tag (injection point
    /// when the run carries no `<hp:t>` yet). `None` if the paragraph has no run.
    pub first_run_open_end: Option<usize>,
    /// Concatenated, entity-decoded text of all runs (nested-table text excluded).
    pub text: String,
}

/// A table cell.
#[derive(Debug, Clone)]
pub struct Cell {
    pub row: u32,
    pub col: u32,
    pub col_span: u32,
    pub row_span: u32,
    pub paras: Vec<Para>,
}

impl Cell {
    /// Concatenated cell text (paragraph texts joined without separators — matches
    /// the reference `extractCellText`).
    pub fn text(&self) -> String {
        self.paras.iter().map(|p| p.text.as_str()).collect()
    }
}

/// A table, DFS-collected. `cells` is flat; build logical rows via [`Table::rows`].
#[derive(Debug, Clone)]
pub struct Table {
    pub cells: Vec<Cell>,
}

impl Table {
    /// Group cells into logical rows by `rowAddr` (ascending), each sorted by `colAddr`.
    pub fn rows(&self) -> Vec<Vec<&Cell>> {
        let mut row_addrs: Vec<u32> = self.cells.iter().map(|c| c.row).collect();
        row_addrs.sort_unstable();
        row_addrs.dedup();
        row_addrs
            .into_iter()
            .map(|r| {
                let mut cells: Vec<&Cell> = self.cells.iter().filter(|c| c.row == r).collect();
                cells.sort_by_key(|c| c.col);
                cells
            })
            .collect()
    }
}

/// Full scan of one section XML.
#[derive(Debug, Clone)]
pub struct Scan {
    pub tables: Vec<Table>,
    /// Section-level paragraphs not inside any cell (for inline "라벨: 값" fills).
    pub body_paras: Vec<Para>,
}

// ─── low-level matched-close scanner ───────────────────────────────────────

/// Find the byte index of the matching close tag for a nestable element whose
/// open variants are `opens` (e.g. `["<hp:p ", "<hp:p>"]`) and close is `close`.
/// `from` is the byte offset to start scanning (just past the element's open tag).
/// Returns the index of the matching `close` (its `<`), or `None`.
fn matching_close(xml: &str, from: usize, opens: &[&str], close: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut pos = from;
    let bytes = xml.as_bytes();
    let end = xml.len();
    while pos < end {
        let next_close = find_from(xml, pos, close);
        let next_open = opens.iter().filter_map(|o| find_from(xml, pos, o)).min();
        match (next_open, next_close) {
            (_, None) => return None,
            (Some(o), Some(c)) if o < c => {
                // guard against self-closing open tags "<hp:p .../>"
                if let Some(gt) = find_from(xml, o, ">") {
                    if gt > 0 && bytes[gt - 1] == b'/' {
                        pos = gt + 1;
                        continue;
                    }
                }
                depth += 1;
                pos = o + 1;
            }
            (_, Some(c)) => {
                depth -= 1;
                if depth == 0 {
                    return Some(c);
                }
                pos = c + close.len();
            }
        }
    }
    None
}

#[inline]
fn find_from(xml: &str, from: usize, pat: &str) -> Option<usize> {
    xml.get(from..).and_then(|s| s.find(pat)).map(|i| i + from)
}

/// Find the next occurrence of any of `pats` at/after `from`, returning (index, which pat).
fn find_any<'a>(xml: &str, from: usize, pats: &[&'a str]) -> Option<(usize, &'a str)> {
    pats.iter()
        .filter_map(|p| find_from(xml, from, p).map(|i| (i, *p)))
        .min_by_key(|(i, _)| *i)
}

const P_OPENS: &[&str] = &["<hp:p ", "<hp:p>"];
const TBL_OPENS: &[&str] = &["<hp:tbl ", "<hp:tbl>"];
const TC_OPENS: &[&str] = &["<hp:tc ", "<hp:tc>"];
const SUBLIST_OPENS: &[&str] = &["<hp:subList ", "<hp:subList>"];
const RUN_OPENS: &[&str] = &["<hp:run ", "<hp:run>"];
const T_OPENS: &[&str] = &["<hp:t>", "<hp:t "];

// ─── public entry ──────────────────────────────────────────────────────────

pub fn scan_section(xml: &str) -> Scan {
    let mut tables = Vec::new();
    collect_tables(xml, 0, xml.len(), &mut tables);
    let body_paras = parse_paragraphs(xml, 0, xml.len());
    Scan { tables, body_paras }
}

/// Collect every top-level table within `[start,end)` (skipping nested ones,
/// which are recursed into via each cell's subList). Parent-first DFS order.
fn collect_tables(xml: &str, start: usize, end: usize, out: &mut Vec<Table>) {
    let mut pos = start;
    while let Some((open, _pat)) = find_any(xml, pos, TBL_OPENS) {
        if open >= end {
            break;
        }
        let open_end = match find_from(xml, open, ">") {
            Some(g) => g + 1,
            None => break,
        };
        let close = match matching_close(xml, open_end, TBL_OPENS, "</hp:tbl>") {
            Some(c) => c,
            None => break,
        };
        let (table, sublist_ranges) = parse_table(xml, open_end, close);
        out.push(table);
        for (s, e) in sublist_ranges {
            collect_tables(xml, s, e, out);
        }
        pos = close + "</hp:tbl>".len();
    }
}

/// Parse a table's direct cells. Returns the table plus each cell's subList
/// inner range (for nested-table recursion).
fn parse_table(xml: &str, start: usize, end: usize) -> (Table, Vec<(usize, usize)>) {
    let mut cells = Vec::new();
    let mut sublists = Vec::new();
    let mut pos = start;
    while let Some((tc_open, _)) = find_any(xml, pos, TC_OPENS) {
        if tc_open >= end {
            break;
        }
        let tc_open_end = match find_from(xml, tc_open, ">") {
            Some(g) => g + 1,
            None => break,
        };
        let tc_close = match matching_close(xml, tc_open_end, TC_OPENS, "</hp:tc>") {
            Some(c) => c,
            None => break,
        };
        let (cell, sub) = parse_cell(xml, tc_open_end, tc_close);
        cells.push(cell);
        if let Some(r) = sub {
            sublists.push(r);
        }
        pos = tc_close + "</hp:tc>".len();
    }
    (Table { cells }, sublists)
}

/// Parse one `<hp:tc>` body `[start,end)`.
fn parse_cell(xml: &str, start: usize, end: usize) -> (Cell, Option<(usize, usize)>) {
    // subList holds the cell content paragraphs
    let (sub_start, sub_end) = match find_any(xml, start, SUBLIST_OPENS) {
        Some((open, _)) if open < end => {
            let open_end = find_from(xml, open, ">").map(|g| g + 1);
            match open_end.and_then(|oe| matching_close(xml, oe, SUBLIST_OPENS, "</hp:subList>").map(|c| (oe, c))) {
                Some((oe, c)) => (oe, c),
                None => (start, end),
            }
        }
        _ => (start, end),
    };
    let paras = parse_paragraphs(xml, sub_start, sub_end);

    // cellAddr / cellSpan are this cell's own children, after </hp:subList>
    let after = sub_end.min(end);
    let tail = &xml[after..end];
    let (row, col) = parse_cell_addr(tail);
    let (col_span, row_span) = parse_cell_span(tail);

    let sub_range = if sub_start < sub_end { Some((sub_start, sub_end)) } else { None };
    (Cell { row, col, col_span, row_span, paras }, sub_range)
}

fn parse_cell_addr(s: &str) -> (u32, u32) {
    // <hp:cellAddr colAddr="C" rowAddr="R"/>
    if let Some(i) = s.find("<hp:cellAddr") {
        let seg = &s[i..s[i..].find("/>").map(|j| i + j).unwrap_or(s.len())];
        let col = attr_u32(seg, "colAddr").unwrap_or(0);
        let row = attr_u32(seg, "rowAddr").unwrap_or(0);
        return (row, col);
    }
    (0, 0)
}

fn parse_cell_span(s: &str) -> (u32, u32) {
    if let Some(i) = s.find("<hp:cellSpan") {
        let seg = &s[i..s[i..].find("/>").map(|j| i + j).unwrap_or(s.len())];
        let cs = attr_u32(seg, "colSpan").unwrap_or(1);
        let rs = attr_u32(seg, "rowSpan").unwrap_or(1);
        return (cs, rs);
    }
    (1, 1)
}

fn attr_u32(seg: &str, name: &str) -> Option<u32> {
    let key = format!("{name}=\"");
    let i = seg.find(&key)? + key.len();
    let rest = &seg[i..];
    let end = rest.find('"')?;
    rest[..end].parse().ok()
}

/// Parse the direct paragraphs of `[start,end)` (nested-table paragraphs excluded
/// automatically because `<hp:p>` matched-close skips over nested tables).
fn parse_paragraphs(xml: &str, start: usize, end: usize) -> Vec<Para> {
    let mut out = Vec::new();
    let mut pos = start;
    while let Some((p_open, _)) = find_any(xml, pos, P_OPENS) {
        if p_open >= end {
            break;
        }
        let gt = match find_from(xml, p_open, ">") {
            Some(g) => g,
            None => break,
        };
        // self-closing empty paragraph
        if gt > 0 && xml.as_bytes()[gt - 1] == b'/' {
            pos = gt + 1;
            continue;
        }
        let p_open_end = gt + 1;
        let p_close = match matching_close(xml, p_open_end, P_OPENS, "</hp:p>") {
            Some(c) => c,
            None => break,
        };
        out.push(parse_para(xml, p_open_end, p_close));
        pos = p_close + "</hp:p>".len();
    }
    out
}

/// Parse a single paragraph body `[start,end)`.
fn parse_para(xml: &str, start: usize, end: usize) -> Para {
    // nested tables inside this paragraph — their text/runs must be excluded
    let nested = find_top_tables_ranges(xml, start, end);
    let in_nested = |p: usize| nested.iter().any(|(s, e)| p >= *s && p < *e);

    // first run open end (injection point), skipping nested-table runs
    let mut first_run_open_end = None;
    {
        let mut pos = start;
        while let Some((r, _)) = find_any(xml, pos, RUN_OPENS) {
            if r >= end {
                break;
            }
            if in_nested(r) {
                pos = r + 1;
                continue;
            }
            if let Some(g) = find_from(xml, r, ">") {
                if g < end {
                    // skip self-closing run
                    if xml.as_bytes()[g - 1] != b'/' {
                        first_run_open_end = Some(g + 1);
                    }
                }
            }
            break;
        }
    }

    // text runs
    let mut runs = Vec::new();
    let mut text = String::new();
    let mut pos = start;
    while let Some((t_open, pat)) = find_any(xml, pos, T_OPENS) {
        if t_open >= end {
            break;
        }
        if in_nested(t_open) {
            // jump past the nested table containing this t
            if let Some((_, ne)) = nested.iter().find(|(s, e)| t_open >= *s && t_open < *e) {
                pos = *ne;
                continue;
            }
        }
        let inner_start = if pat == "<hp:t>" {
            t_open + "<hp:t>".len()
        } else {
            match find_from(xml, t_open, ">") {
                Some(g) => {
                    // self-closing <hp:t .../>
                    if xml.as_bytes()[g - 1] == b'/' {
                        pos = g + 1;
                        continue;
                    }
                    g + 1
                }
                None => break,
            }
        };
        let inner_end = match find_from(xml, inner_start, "</hp:t>") {
            Some(c) => c,
            None => break,
        };
        let raw = &xml[inner_start..inner_end];
        text.push_str(&xml_unescape(raw));
        runs.push(TextRun { start: inner_start, end: inner_end });
        pos = inner_end + "</hp:t>".len();
    }

    Para { runs, first_run_open_end, text }
}

/// Top-level `<hp:tbl>` ranges within `[start,end)` (as [open, close_end)).
fn find_top_tables_ranges(xml: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut pos = start;
    while let Some((open, _)) = find_any(xml, pos, TBL_OPENS) {
        if open >= end {
            break;
        }
        let open_end = match find_from(xml, open, ">") {
            Some(g) => g + 1,
            None => break,
        };
        let close = match matching_close(xml, open_end, TBL_OPENS, "</hp:tbl>") {
            Some(c) => c,
            None => break,
        };
        let close_end = close + "</hp:tbl>".len();
        out.push((open, close_end));
        pos = close_end;
    }
    out
}

// ─── entity helpers ────────────────────────────────────────────────────────

/// Decode the XML entities that appear in HWPX `<hp:t>` content.
pub fn xml_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some(semi) = tail.find(';') {
            let ent = &tail[1..semi];
            let decoded = match ent {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
                }
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            match decoded {
                Some(c) => {
                    out.push(c);
                    rest = &tail[semi + 1..];
                }
                None => {
                    out.push('&');
                    rest = &tail[1..];
                }
            }
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Escape text for insertion into `<hp:t>` content.
pub fn xml_escape_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

// ─── source-map precise editing (public API) ────────────────────────────────
//
// The [`Scan`] returned by [`scan_section`] already is the "XML byte-position
// map" — every paragraph/cell records the exact `<hp:t>` content byte ranges.
// The primitives below turn that map into format-preserving edits: replacing an
// arbitrary sub-range of a paragraph's text while leaving every other byte
// (attributes, run/charPr structure, tab/br siblings, unchanged runs) intact.

/// A `[start, end)` byte-range replacement into a section XML string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpliceEdit {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// Strip XML 1.0-invalid C0 control chars — everything below U+0020 except
/// tab/newline/CR. `xml_escape_text` escapes `&<>` but passes these through
/// verbatim, and a stray NUL/`\x0B` makes 한글 reject the document. Mirrors the
/// reference filler, which sanitizes replacement text before splicing.
fn sanitize_xml_text(s: &str) -> String {
    s.chars()
        .filter(|&c| c >= ' ' || c == '\t' || c == '\n' || c == '\r')
        .collect()
}

/// Apply splices to `xml`. Ranges are sorted and applied back-to-front so
/// earlier offsets stay valid.
///
/// Defensive against untrusted / stale source maps: any splice that is reversed
/// (`start > end`), out of bounds (`end > xml.len()`), or does not land on UTF-8
/// `char` boundaries is **dropped** rather than passed to `replace_range` (which
/// would panic). Overlapping ranges are also dropped, earlier wins. The builders
/// below never emit such splices, so dropping only guards against caller error —
/// the behavior is documented so it is not a silent surprise.
pub fn apply_splices(xml: &str, mut splices: Vec<SpliceEdit>) -> String {
    let len = xml.len();
    splices.retain(|s| {
        s.start <= s.end
            && s.end <= len
            && xml.is_char_boundary(s.start)
            && xml.is_char_boundary(s.end)
    });
    splices.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
    let mut merged: Vec<SpliceEdit> = Vec::new();
    for s in splices {
        if let Some(prev) = merged.last() {
            if s.start < prev.end {
                continue;
            }
        }
        merged.push(s);
    }
    let mut out = xml.to_string();
    for s in merged.into_iter().rev() {
        out.replace_range(s.start..s.end, &s.replacement);
    }
    out
}

/// True when a stored run range still indexes `xml` safely: ordered, in bounds,
/// and on UTF-8 `char` boundaries. Guards against stale (post-edit) or
/// externally-constructed `TextRun` coordinates that would otherwise panic when
/// used to slice `xml`.
fn run_range_valid(r: &TextRun, xml: &str) -> bool {
    r.start <= r.end
        && r.end <= xml.len()
        && xml.is_char_boundary(r.start)
        && xml.is_char_boundary(r.end)
}

/// Raw, undecoded concatenation of a paragraph's `<hp:t>` content — the
/// coordinate system used by [`build_range_splices`]. Returns `None` when a run
/// coordinate is stale/out of bounds (would panic on slice) or when any run's
/// raw content carries an entity/markup char (`<`/`&`), because then a byte
/// offset in this string would not map cleanly onto the XML.
pub fn para_t_text(para: &Para, xml: &str) -> Option<String> {
    let mut out = String::new();
    for r in &para.runs {
        if !run_range_valid(r, xml) {
            return None;
        }
        let raw = &xml[r.start..r.end];
        if raw.contains('<') || raw.contains('&') {
            return None;
        }
        out.push_str(raw);
    }
    Some(out)
}

/// Precise sub-range edit: replace the `[start, end)` byte range of a
/// paragraph's t-domain text (see [`para_t_text`]) with `replacement`, emitting
/// splices that touch only `<hp:t>` content and preserve run/tab/br structure.
///
/// `start == end` inserts at that offset. Returns `None` when the range is out
/// of bounds, when `start`/`end` are not `char` boundaries of the t-domain
/// string (splicing mid-codepoint would corrupt the XML and panic on apply),
/// when a contributing run holds an entity/markup char (offsets would be
/// ambiguous), or when the range covers no `<hp:t>` content. Offsets are byte
/// offsets into the t-domain string. `replacement` is stripped of XML-invalid
/// C0 control chars before escaping.
pub fn build_range_splices(
    para: &Para,
    xml: &str,
    start: usize,
    end: usize,
    replacement: &str,
) -> Option<Vec<SpliceEdit>> {
    if end < start {
        return None;
    }
    // Align t-runs to the t-domain offset, bailing on entity/markup content.
    // `t_text` mirrors the concatenated raw content so offsets can be validated
    // against real codepoint boundaries.
    struct Seg {
        content_start: usize,
        from: usize,
        to: usize,
    }
    let mut segs: Vec<Seg> = Vec::new();
    let mut t_text = String::new();
    let mut offset = 0usize;
    for r in &para.runs {
        // Validate the stored run range *before* slicing — a stale/non-boundary
        // coordinate must yield None, not panic in the slice below.
        if !run_range_valid(r, xml) {
            return None;
        }
        let raw = &xml[r.start..r.end];
        if raw.contains('<') || raw.contains('&') {
            return None;
        }
        let len = raw.len();
        segs.push(Seg { content_start: r.start, from: offset, to: offset + len });
        t_text.push_str(raw);
        offset += len;
    }
    if segs.is_empty() || end > offset {
        return None;
    }
    // Reject offsets that split a multi-byte codepoint — otherwise the emitted
    // splice would land mid-char and panic in `apply_splices::replace_range`.
    if !t_text.is_char_boundary(start) || !t_text.is_char_boundary(end) {
        return None;
    }

    let escaped = xml_escape_text(&sanitize_xml_text(replacement));

    // Insertion — into the first segment containing `start`.
    if start == end {
        for seg in &segs {
            if start >= seg.from && start <= seg.to {
                let at = seg.content_start + (start - seg.from);
                return Some(vec![SpliceEdit { start: at, end: at, replacement: escaped }]);
            }
        }
        return None;
    }

    // Replacement — new text into the first overlapping segment, other overlaps emptied.
    let mut splices = Vec::new();
    let mut placed = false;
    for seg in &segs {
        if seg.to <= start || seg.from >= end {
            continue;
        }
        let local_start = start.max(seg.from) - seg.from;
        let local_end = end.min(seg.to) - seg.from;
        splices.push(SpliceEdit {
            start: seg.content_start + local_start,
            end: seg.content_start + local_end,
            replacement: if placed { String::new() } else { escaped.clone() },
        });
        placed = true;
    }
    if placed {
        Some(splices)
    } else {
        None
    }
}

#[cfg(test)]
mod sourcemap_tests {
    use super::*;

    fn body_para(xml: &str) -> Para {
        let scan = scan_section(xml);
        scan.body_paras.into_iter().next().expect("one body paragraph")
    }

    #[test]
    fn range_replace_within_single_run() {
        let xml = r#"<hp:p><hp:run><hp:t>Hello world</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        assert_eq!(para_t_text(&para, xml).as_deref(), Some("Hello world"));
        // replace "world" (offset 6..11) with "there"
        let splices = build_range_splices(&para, xml, 6, 11, "there").unwrap();
        let out = apply_splices(xml, splices);
        assert!(out.contains("<hp:t>Hello there</hp:t>"), "{out}");
    }

    #[test]
    fn range_insert_is_zero_width() {
        let xml = r#"<hp:p><hp:run><hp:t>AB</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        let splices = build_range_splices(&para, xml, 1, 1, "X").unwrap();
        assert_eq!(splices.len(), 1);
        assert_eq!(splices[0].start, splices[0].end);
        let out = apply_splices(xml, splices);
        assert!(out.contains("<hp:t>AXB</hp:t>"), "{out}");
    }

    #[test]
    fn range_spans_two_runs_preserving_structure() {
        // Two runs (distinct charPr) with a tab-like sibling between them.
        let xml = r#"<hp:p><hp:run charPrIDRef="0"><hp:t>abc</hp:t></hp:run><hp:run charPrIDRef="1"><hp:t>def</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        assert_eq!(para_t_text(&para, xml).as_deref(), Some("abcdef"));
        // replace "cd" (offset 2..4) across the run boundary
        let splices = build_range_splices(&para, xml, 2, 4, "X").unwrap();
        let out = apply_splices(xml, splices);
        // second run's charPr and tag survive; text becomes ab|X + ef
        assert!(out.contains(r#"charPrIDRef="1""#), "second run kept: {out}");
        assert!(out.contains("<hp:t>abX</hp:t>"), "{out}");
        assert!(out.contains("<hp:t>ef</hp:t>"), "{out}");
    }

    #[test]
    fn entity_content_bails() {
        let xml = r#"<hp:p><hp:run><hp:t>a&amp;b</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        assert!(para_t_text(&para, xml).is_none());
        assert!(build_range_splices(&para, xml, 0, 1, "z").is_none());
    }

    #[test]
    fn out_of_bounds_bails() {
        let xml = r#"<hp:p><hp:run><hp:t>ab</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        assert!(build_range_splices(&para, xml, 0, 99, "z").is_none());
    }

    #[test]
    fn multibyte_non_char_boundary_bails() {
        // "가나다" — each char is 3 UTF-8 bytes, so offset 1 splits 가.
        let xml = r#"<hp:p><hp:run><hp:t>가나다</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        assert_eq!(para_t_text(&para, xml).as_deref(), Some("가나다"));
        // start mid-codepoint
        assert!(build_range_splices(&para, xml, 1, 3, "X").is_none());
        // end mid-codepoint
        assert!(build_range_splices(&para, xml, 0, 2, "X").is_none());
        // whole valid boundary still works (replace first char)
        let splices = build_range_splices(&para, xml, 0, 3, "X").unwrap();
        let out = apply_splices(xml, splices);
        assert!(out.contains("<hp:t>X나다</hp:t>"), "{out}");
    }

    #[test]
    fn apply_splices_drops_invalid_ranges_without_panic() {
        // multibyte content so a mid-byte offset is reachable
        let xml = "가나";
        let len = xml.len(); // 6
        // reversed, out-of-bounds, and mid-codepoint splices must be dropped
        let bad = vec![
            SpliceEdit { start: 3, end: 1, replacement: "z".into() }, // reversed
            SpliceEdit { start: len + 1, end: len + 1, replacement: "z".into() }, // OOB
            SpliceEdit { start: 1, end: 4, replacement: "z".into() }, // mid-codepoint
        ];
        // no panic; every invalid splice dropped → xml unchanged
        assert_eq!(apply_splices(xml, bad), xml);
        // a valid splice still applies alongside dropped ones
        let mixed = vec![
            SpliceEdit { start: 1, end: 2, replacement: "!".into() }, // mid-codepoint, dropped
            SpliceEdit { start: 0, end: 3, replacement: "X".into() }, // valid: replace 가
        ];
        assert_eq!(apply_splices(xml, mixed), "X나");
    }

    #[test]
    fn stale_run_coords_bail_without_panic() {
        let xml = r#"<hp:p><hp:run><hp:t>가나</hp:t></hp:run></hp:p>"#;
        let mut para = body_para(xml);
        // simulate a source map gone stale against a shorter/edited xml, plus a
        // run range landing mid-codepoint — both must bail, not panic.
        let short_xml = "가";
        assert!(build_range_splices(&para, short_xml, 0, 1, "X").is_none());
        assert!(para_t_text(&para, short_xml).is_none());
        // externally-mutated TextRun with a mid-codepoint boundary
        para.runs = vec![TextRun { start: 0, end: 1 }]; // splits 가 (3 bytes)
        let g = "가";
        assert!(build_range_splices(&para, g, 0, 1, "X").is_none());
        assert!(para_t_text(&para, g).is_none());
    }

    #[test]
    fn c0_control_chars_stripped() {
        let xml = r#"<hp:p><hp:run><hp:t>ab</hp:t></hp:run></hp:p>"#;
        let para = body_para(xml);
        // NUL and vertical-tab are XML-invalid; tab/newline are kept
        let splices = build_range_splices(&para, xml, 0, 2, "x\u{0}y\u{0B}\tz\n").unwrap();
        assert_eq!(splices.len(), 1);
        let rep = &splices[0].replacement;
        assert!(!rep.contains('\u{0}'), "NUL stripped: {rep:?}");
        assert!(!rep.contains('\u{0B}'), "VT stripped: {rep:?}");
        assert!(rep.contains('\t') && rep.contains('\n'), "tab/newline kept: {rep:?}");
        let out = apply_splices(xml, splices);
        assert!(!out.contains('\u{0}') && !out.contains('\u{0B}'), "{out:?}");
    }
}
