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
