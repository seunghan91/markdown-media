// Ported from kkdoc (MIT): src/hwpx/gen-table.ts
//! HWPX table XML — GFM grid tables and merged (colspan/rowspan) HTML tables.
//!
//! Column widths are content-proportional and always sum to the table width
//! (HWPX requires per-row cell widths to sum to the table width). The measured
//! government table grammar (border hierarchy, header shading, label columns)
//! is reduced to a single shared border + optional header shade — see mod.rs.

use std::sync::atomic::{AtomicU32, Ordering};

use super::blocks::generate_runs;
use super::ids::{escape_xml, ResolvedTheme, CHAR_BOLD, CHAR_NORMAL, CHAR_TABLE_HEADER, PARA_NORMAL};

const TABLE_ID_BASE: u32 = 1000;
static TABLE_ID: AtomicU32 = AtomicU32::new(TABLE_ID_BASE);

/// Reset the per-document table id counter for deterministic output.
pub fn reset_table_ids() {
    TABLE_ID.store(TABLE_ID_BASE, Ordering::SeqCst);
}
fn next_table_id() -> u32 {
    TABLE_ID.fetch_add(1, Ordering::SeqCst) + 1
}

/// Preset table styling (header shading + preset body width). A reduced form
/// of the reference `GongmunTableStyle`.
#[derive(Debug, Clone)]
pub struct TableStyle {
    pub total_width: i32,
    /// borderFill id used for header-row cells (shaded).
    pub header_bf: u32,
}

const CELL_PAD: f64 = 1200.0;

fn measure_text_width(text: &str, char_height: u32) -> f64 {
    let mut em = 0.0f64;
    for c in text.trim().chars() {
        em += if (c as u32) < 0x80 { 500.0 } else { 1000.0 };
    }
    (em / 1000.0) * char_height as f64
}

/// Longest single line of a cell (splitting on `<br>`), stripping emphasis.
fn cell_content_width(text: &str, char_height: u32) -> f64 {
    let cleaned = strip_emphasis(text);
    let mut max = 0.0f64;
    for seg in split_br(&cleaned) {
        let w = measure_text_width(seg.trim(), char_height);
        if w > max {
            max = w;
        }
    }
    max
}

fn strip_emphasis(text: &str) -> String {
    text.replace("**", "").replace("__", "").replace('`', "")
}

fn split_br(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut parts = Vec::new();
    let mut last = 0usize;
    let bytes = lower.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'<' && lower[i..].starts_with("<br") {
            // find end of tag
            if let Some(end) = lower[i..].find('>') {
                parts.push(text[last..i].to_string());
                i += end + 1;
                last = i;
                continue;
            }
        }
        i += 1;
    }
    parts.push(text[last..].to_string());
    parts
}

/// Distribute `total_width` across columns proportional to content, clamped to
/// [min, 80%], guaranteeing the widths sum to exactly `total_width`.
pub fn compute_col_widths(col_max: &[f64], total_width: i32) -> Vec<i32> {
    let col_cnt = col_max.len().max(1);
    if col_max.is_empty() {
        return vec![total_width];
    }
    let total = total_width as f64;
    let min_w = ((total * 0.06).max(2000.0)).min(total / col_cnt as f64);
    let cap = (total * 0.8).round();
    let raw: Vec<f64> = col_max
        .iter()
        .map(|w| (w + CELL_PAD).max(min_w).min(cap))
        .collect();
    let sum: f64 = raw.iter().sum();
    let mut widths: Vec<i32> = raw
        .iter()
        .map(|r| ((r / sum) * total).floor().max(1.0) as i32)
        .collect();
    // Fix rounding remainder on widest columns first.
    let mut rem = total_width - widths.iter().sum::<i32>();
    let mut order: Vec<usize> = (0..col_cnt).collect();
    order.sort_by(|&a, &b| raw[b].partial_cmp(&raw[a]).unwrap());
    let mut k = 0;
    while rem > 0 {
        widths[order[k % col_cnt]] += 1;
        rem -= 1;
        k += 1;
    }
    while rem < 0 {
        let idx = order[k % col_cnt];
        if widths[idx] > 1 {
            widths[idx] -= 1;
            rem += 1;
        }
        k += 1;
        if k > col_cnt * 4 {
            break;
        }
    }
    widths
}

fn row_height(cells: &[String], widths: &[i32], char_height: u32, preset: bool) -> i32 {
    if !preset {
        return 1500;
    }
    let mut max_lines = 1;
    for (c, cell) in cells.iter().enumerate() {
        let usable = ((*widths.get(c).unwrap_or(widths.last().unwrap()) as f64) - CELL_PAD).max(1000.0);
        let mut lines = 0;
        for seg in split_br(&strip_emphasis(cell)) {
            let w = measure_text_width(seg.trim(), char_height);
            lines += (w / usable).ceil().max(1.0) as i32;
        }
        if lines > max_lines {
            max_lines = lines;
        }
    }
    max_lines * ((char_height as f64 * 1.6).round() as i32) + 282
}

fn tc_xml(
    cell: &str,
    col_addr: usize,
    row_addr: usize,
    col_span: usize,
    row_span: usize,
    width: i32,
    height: i32,
    char_pr: u32,
    bf: u32,
    is_header: bool,
    map_char: Option<&dyn Fn(u32) -> u32>,
) -> String {
    let vert = if is_header { "CENTER" } else { "TOP" };
    let paras: String = split_br(cell)
        .iter()
        .map(|seg| {
            let runs = generate_runs(seg, char_pr, map_char);
            let body = if runs.is_empty() {
                format!("<hp:run charPrIDRef=\"{char_pr}\"><hp:t></hp:t></hp:run>")
            } else {
                runs
            };
            format!("<hp:p paraPrIDRef=\"{PARA_NORMAL}\" styleIDRef=\"0\">{body}</hp:p>")
        })
        .collect();
    format!(
        "<hp:tc name=\"\" header=\"{h}\" hasMargin=\"0\" protect=\"0\" editable=\"1\" dirty=\"0\" borderFillIDRef=\"{bf}\">\
        <hp:subList id=\"\" textDirection=\"HORIZONTAL\" lineWrap=\"BREAK\" vertAlign=\"{vert}\" linkListIDRef=\"0\" linkListNextIDRef=\"0\" textWidth=\"0\" textHeight=\"0\" hasTextRef=\"0\" hasNumRef=\"0\">{paras}</hp:subList>\
        <hp:cellAddr colAddr=\"{ca}\" rowAddr=\"{ra}\"/>\
        <hp:cellSpan colSpan=\"{cs}\" rowSpan=\"{rs}\"/>\
        <hp:cellSz width=\"{w}\" height=\"{ht}\"/>\
        <hp:cellMargin left=\"141\" right=\"141\" top=\"141\" bottom=\"141\"/>\
        </hp:tc>",
        h = if is_header { 1 } else { 0 },
        bf = bf,
        vert = vert,
        paras = paras,
        ca = col_addr,
        ra = row_addr,
        cs = col_span,
        rs = row_span,
        w = width,
        ht = height,
    )
}

fn tbl_wrapper(id: u32, row_cnt: usize, col_cnt: usize, tbl_w: i32, tbl_h: i32, rows: &str, preset: bool) -> String {
    let repeat = if preset { 1 } else { 0 };
    format!(
        "<hp:tbl id=\"{id}\" zOrder=\"0\" numberingType=\"TABLE\" pageBreak=\"CELL\" repeatHeader=\"{repeat}\" rowCnt=\"{rc}\" colCnt=\"{cc}\" cellSpacing=\"0\" borderFillIDRef=\"2\" noShading=\"0\">\
        <hp:sz width=\"{tw}\" widthRelTo=\"ABSOLUTE\" height=\"{th}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
        <hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"0\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"PARA\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
        <hp:outMargin left=\"0\" right=\"0\" top=\"0\" bottom=\"0\"/>\
        <hp:inMargin left=\"510\" right=\"510\" top=\"141\" bottom=\"141\"/>\
        {rows}</hp:tbl>",
        id = id, repeat = repeat, rc = row_cnt, cc = col_cnt, tw = tbl_w, th = tbl_h, rows = rows,
    )
}

/// GFM grid table → a `<hp:p>` hosting the `<hp:tbl>`.
pub fn generate_table(rows: &[Vec<String>], theme: &ResolvedTheme, style: Option<&TableStyle>) -> String {
    let row_cnt = rows.len();
    let col_cnt = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let total_w = style.map(|s| s.total_width).unwrap_or(44000);
    let measure_h = 1000u32;

    let mut col_max = vec![0.0f64; col_cnt];
    for row in rows {
        for (c, cell) in row.iter().enumerate() {
            let w = cell_content_width(cell, measure_h);
            if w > col_max[c] {
                col_max[c] = w;
            }
        }
    }
    let col_widths = compute_col_widths(&col_max, total_w);
    let use_header_style = style.is_none() && (theme.table_header != theme.body || theme.table_header_bold);
    let tbl_id = next_table_id();

    let row_heights: Vec<i32> = rows.iter().map(|r| row_height(r, &col_widths, measure_h, style.is_some())).collect();

    let mut tr_elems = String::new();
    for (row_idx, row) in rows.iter().enumerate() {
        let is_header = row_idx == 0;
        let cell_h = row_heights[row_idx];
        let mut cells: Vec<String> = row.clone();
        while cells.len() < col_cnt {
            cells.push(String::new());
        }
        let mut tds = String::new();
        for (col_idx, cell) in cells.iter().enumerate() {
            let char_pr = if is_header && use_header_style {
                CHAR_TABLE_HEADER
            } else if is_header && style.is_some() {
                CHAR_BOLD
            } else {
                CHAR_NORMAL
            };
            let bf = if is_header {
                style.map(|s| s.header_bf).unwrap_or(2)
            } else {
                2
            };
            tds.push_str(&tc_xml(
                cell, col_idx, row_idx, 1, 1, col_widths[col_idx], cell_h, char_pr, bf, is_header, None,
            ));
        }
        tr_elems.push_str(&format!("<hp:tr>{tds}</hp:tr>"));
    }

    let tbl_w: i32 = col_widths.iter().sum();
    let tbl_h: i32 = row_heights.iter().sum();
    let tbl = tbl_wrapper(tbl_id, row_cnt, col_cnt, tbl_w, tbl_h, &tr_elems, style.is_some());
    format!("<hp:p paraPrIDRef=\"0\" styleIDRef=\"0\"><hp:run charPrIDRef=\"0\">{tbl}</hp:run></hp:p>")
}

// ─── HTML merged-cell tables ────────────────────────

#[derive(Debug, Clone)]
struct HtmlCell {
    col_span: usize,
    row_span: usize,
    inner: String,
    is_header: bool,
}

#[derive(Debug, Clone)]
struct PlacedCell {
    r: usize,
    c: usize,
    col_span: usize,
    row_span: usize,
    inner: String,
    is_header: bool,
}

/// Parse `<tr>`/`<td>`/`<th>` rows, tracking `<table>` nesting depth so nested
/// tables inside a cell are not split as parent rows.
fn parse_html_rows(raw: &str) -> Vec<Vec<HtmlCell>> {
    let lower = raw.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut rows: Vec<Vec<HtmlCell>> = Vec::new();
    let mut i = 0usize;
    let mut table_depth = 0i32;
    let mut cur_row: Option<Vec<HtmlCell>> = None;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &lower[i..];
        if rest.starts_with("<table") {
            table_depth += 1;
            i += advance_tag(&lower[i..]);
            continue;
        }
        if rest.starts_with("</table>") {
            table_depth -= 1;
            i += "</table>".len();
            continue;
        }
        // Only act on the outermost table's structure.
        if table_depth == 1 {
            if rest.starts_with("<tr") {
                cur_row = Some(Vec::new());
                i += advance_tag(&lower[i..]);
                continue;
            }
            if rest.starts_with("</tr>") {
                if let Some(r) = cur_row.take() {
                    rows.push(r);
                }
                i += "</tr>".len();
                continue;
            }
            if rest.starts_with("<td") || rest.starts_with("<th") {
                let is_header = rest.starts_with("<th");
                let tag_len = advance_tag(&lower[i..]);
                let open_tag = &raw[i..i + tag_len];
                let col_span = attr_num(open_tag, "colspan").unwrap_or(1).max(1);
                let row_span = attr_num(open_tag, "rowspan").unwrap_or(1).max(1);
                // find matching close accounting for nested td/th of same tag name
                let close = if is_header { "</th>" } else { "</td>" };
                let open = if is_header { "<th" } else { "<td" };
                let inner_start = i + tag_len;
                let inner_end = find_matching_close(&lower, inner_start, open, close);
                let inner = raw[inner_start..inner_end].to_string();
                if let Some(row) = cur_row.as_mut() {
                    row.push(HtmlCell { col_span, row_span, inner, is_header });
                }
                i = inner_end + close.len();
                continue;
            }
        }
        i += advance_tag(&lower[i..]);
    }
    rows
}

/// Byte length of the tag starting at `s[0] == '<'`, up to and including `>`.
fn advance_tag(s: &str) -> usize {
    s.find('>').map(|p| p + 1).unwrap_or(s.len())
}

fn attr_num(tag: &str, name: &str) -> Option<usize> {
    let lower = tag.to_ascii_lowercase();
    let key = format!("{name}=");
    let pos = lower.find(&key)? + key.len();
    let rest = &tag[pos..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"').or_else(|| rest.strip_prefix('\'')).unwrap_or(rest);
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    num.parse().ok()
}

/// Find the index of the close tag matching an opener, honoring nesting.
fn find_matching_close(lower: &str, from: usize, open: &str, close: &str) -> usize {
    let bytes = lower.as_bytes();
    let mut depth = 1i32;
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &lower[i..];
            if rest.starts_with(open) {
                depth += 1;
                i += advance_tag(rest);
                continue;
            }
            if rest.starts_with(close) {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
                i += close.len();
                continue;
            }
        }
        i += 1;
    }
    lower.len()
}

fn unescape_html(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Strip tags from HTML cell inner, splitting on `<br>` into lines.
fn html_cell_lines(inner: &str) -> Vec<String> {
    // remove nested tables entirely for text purposes
    let no_nested = remove_nested_tables(inner);
    let with_breaks = replace_br(&no_nested);
    with_breaks
        .split('\n')
        .map(|l| unescape_html(&strip_all_tags(l)).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn remove_nested_tables(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0usize;
    while i < s.len() {
        if lower[i..].starts_with("<table") {
            let end = find_matching_close(&lower, i + advance_tag(&lower[i..]), "<table", "</table>");
            i = (end + "</table>".len()).min(s.len());
            continue;
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn replace_br(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut out = String::new();
    let mut i = 0usize;
    while i < s.len() {
        if lower[i..].starts_with("<br") {
            let len = advance_tag(&lower[i..]);
            out.push('\n');
            i += len;
            continue;
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn strip_all_tags(s: &str) -> String {
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
    out
}

/// Place cells on a grid honoring colspan/rowspan, filling holes with empties.
fn layout_html_rows(rows: &[Vec<HtmlCell>]) -> (Vec<PlacedCell>, usize, usize) {
    use std::collections::HashSet;
    let mut occupied: HashSet<(usize, usize)> = HashSet::new();
    let mut placed: Vec<PlacedCell> = Vec::new();
    let mut col_cnt = 0usize;
    for (r, row) in rows.iter().enumerate() {
        let mut c = 0usize;
        for cell in row {
            while occupied.contains(&(r, c)) {
                c += 1;
            }
            let col_span = cell.col_span.min(64).max(1);
            let row_span = cell.row_span.min(256).max(1);
            placed.push(PlacedCell {
                r,
                c,
                col_span,
                row_span,
                inner: cell.inner.clone(),
                is_header: cell.is_header,
            });
            for dr in 0..row_span {
                for dc in 0..col_span {
                    occupied.insert((r + dr, c + dc));
                }
            }
            c += col_span;
            col_cnt = col_cnt.max(c);
        }
    }
    let row_cnt = rows.len();
    // Fill holes so every row's cell widths sum to the table width.
    for r in 0..row_cnt {
        for c in 0..col_cnt {
            if !occupied.contains(&(r, c)) {
                placed.push(PlacedCell {
                    r,
                    c,
                    col_span: 1,
                    row_span: 1,
                    inner: String::new(),
                    is_header: rows[r].first().map(|_| false).unwrap_or(false),
                });
                occupied.insert((r, c));
            }
        }
    }
    placed.sort_by(|a, b| a.r.cmp(&b.r).then(a.c.cmp(&b.c)));
    (placed, row_cnt, col_cnt)
}

/// Merged HTML table → `<hp:tbl>` XML, or None if unparseable.
pub fn generate_html_table_xml(
    raw_html: &str,
    theme: &ResolvedTheme,
    total_width: i32,
    style: Option<&TableStyle>,
) -> Option<String> {
    let rows = parse_html_rows(raw_html);
    if rows.is_empty() {
        return None;
    }
    let (placed, row_cnt, col_cnt) = layout_html_rows(&rows);
    if row_cnt == 0 || col_cnt == 0 {
        return None;
    }
    let measure_h = 1000u32;

    // Column max content widths (colSpan cells contribute width/span per column).
    let mut col_max = vec![0.0f64; col_cnt];
    let cell_lines: Vec<Vec<String>> = placed.iter().map(|p| html_cell_lines(&p.inner)).collect();
    for (i, cell) in placed.iter().enumerate() {
        let w = cell_lines[i]
            .iter()
            .map(|l| measure_text_width(l, measure_h))
            .fold(0.0, f64::max)
            / cell.col_span as f64;
        for dc in 0..cell.col_span {
            let c = cell.c + dc;
            if c < col_cnt && w > col_max[c] {
                col_max[c] = w;
            }
        }
    }
    let col_widths = compute_col_widths(&col_max, total_width);
    let use_header_style = style.is_none() && (theme.table_header != theme.body || theme.table_header_bold);
    let tbl_id = next_table_id();

    let line_h = (measure_h as f64 * 1.6).round() as i32;
    let base_row_h = line_h + 282;
    // Row heights: max content height across cells anchored/spanning the row.
    let mut row_heights = vec![base_row_h; row_cnt];
    for (i, cell) in placed.iter().enumerate() {
        let span_w: i32 = col_widths[cell.c..(cell.c + cell.col_span).min(col_cnt)].iter().sum();
        let usable = (span_w as f64 - CELL_PAD).max(1000.0);
        let mut wrap_lines = 0i32;
        for l in &cell_lines[i] {
            wrap_lines += (measure_text_width(l, measure_h) / usable).ceil().max(1.0) as i32;
        }
        let content_h = (wrap_lines.max(1)) * line_h + 282;
        let per_row = (content_h + cell.row_span as i32 - 1) / cell.row_span as i32;
        for r in cell.r..(cell.r + cell.row_span).min(row_cnt) {
            if per_row > row_heights[r] {
                row_heights[r] = per_row;
            }
        }
    }

    let span_w = |cell: &PlacedCell| -> i32 {
        col_widths[cell.c..(cell.c + cell.col_span).min(col_cnt)].iter().sum()
    };

    let mut tc_by_row: Vec<Vec<String>> = vec![Vec::new(); row_cnt];
    for (i, cell) in placed.iter().enumerate() {
        let is_header = cell.is_header;
        let char_pr = if is_header && use_header_style {
            CHAR_TABLE_HEADER
        } else if is_header && style.is_some() {
            CHAR_BOLD
        } else {
            CHAR_NORMAL
        };
        let bf = if is_header {
            style.map(|s| s.header_bf).unwrap_or(2)
        } else {
            2
        };
        let cell_height: i32 = row_heights[cell.r..(cell.r + cell.row_span).min(row_cnt)].iter().sum();
        let text = if cell_lines[i].is_empty() {
            String::new()
        } else {
            cell_lines[i].join("<br>")
        };
        tc_by_row[cell.r].push(tc_xml(
            &text,
            cell.c,
            cell.r,
            cell.col_span,
            cell.row_span,
            span_w(cell),
            cell_height,
            char_pr,
            bf,
            is_header,
            None,
        ));
    }

    let mut tr_elems = String::new();
    for r in 0..row_cnt {
        tr_elems.push_str(&format!("<hp:tr>{}</hp:tr>", tc_by_row[r].join("")));
    }
    let tbl_w: i32 = col_widths.iter().sum();
    let tbl_h: i32 = row_heights.iter().sum();
    let _ = escape_xml; // used indirectly via generate_runs
    Some(tbl_wrapper(tbl_id, row_cnt, col_cnt, tbl_w, tbl_h, &tr_elems, style.is_some()))
}
