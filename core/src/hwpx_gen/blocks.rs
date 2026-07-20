// Ported from kkdoc (MIT): src/hwpx/md-runs.ts
//! Markdown block decomposition + inline span parsing.
//!
//! `parse_markdown_to_blocks` splits source into typed blocks; `parse_inline`
//! resolves bold/italic/code runs. The inline scanner is a hand-rolled port
//! (Rust `regex` lacks the look-around the TS reference used).

use lazy_static::lazy_static;
use regex::Regex;

use super::ids::{CHAR_BOLD, CHAR_BOLD_ITALIC, CHAR_CODE, CHAR_ITALIC, CHAR_NORMAL};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading,
    Table,
    HtmlTable,
    CodeBlock,
    Equation,
    Hr,
    Blockquote,
    ListItem,
}

#[derive(Debug, Clone)]
pub struct MdBlock {
    pub kind: BlockKind,
    pub text: String,
    pub level: u32,
    pub rows: Vec<Vec<String>>,
    pub lang: String,
    pub ordered: bool,
    pub indent: usize,
    /// Original list marker ("2." "3)" "-" "*") — preserved for round-trip.
    pub marker: String,
}

impl MdBlock {
    fn new(kind: BlockKind) -> Self {
        MdBlock {
            kind,
            text: String::new(),
            level: 0,
            rows: Vec::new(),
            lang: String::new(),
            ordered: false,
            indent: 0,
            marker: String::new(),
        }
    }
    fn para(text: &str) -> Self {
        let mut b = MdBlock::new(BlockKind::Paragraph);
        b.text = text.to_string();
        b
    }
}

lazy_static! {
    static ref RE_FENCE: Regex = Regex::new(r"^ {0,3}(`{3,}|~{3,})(.*)$").unwrap();
    static ref RE_HR: Regex = Regex::new(r"^(\*{3,}|-{3,}|_{3,})\s*$").unwrap();
    static ref RE_HEADING: Regex = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    static ref RE_LIST: Regex = Regex::new(r"^(\s*)([-*+]|\d+[.)]) (.+)$").unwrap();
    static ref RE_SEP_CELL: Regex = Regex::new(r"^\s*:?-+:?\s*$").unwrap();
}

/// Split a GFM table row on unescaped `|`, restoring `\|` → `|` in each cell.
fn split_table_row(row: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut escaped = false;
    for c in row.chars() {
        if escaped {
            if c == '|' {
                cur.push('|');
            } else {
                cur.push('\\');
                cur.push(c);
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    if escaped {
        cur.push('\\');
    }
    cells.push(cur.trim().to_string());
    // Drop leading/trailing empties from the surrounding pipes.
    if cells.first().map(|s| s.is_empty()).unwrap_or(false) {
        cells.remove(0);
    }
    if cells.last().map(|s| s.is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells
}

/// Decompose Markdown into typed blocks (headings, lists, tables, code, …).
pub fn parse_markdown_to_blocks(md: &str) -> Vec<MdBlock> {
    let normalized = md.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut blocks: Vec<MdBlock> = Vec::new();
    let mut i = 0usize;
    let mut list_stack: Vec<usize> = Vec::new();

    while i < lines.len() {
        let line = lines[i];

        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // Display math block: $$ ... $$
        if line.trim_start().starts_with("$$") {
            let after = &line.trim_start()[2..];
            if let Some(pos) = after.find("$$") {
                let inner = after[..pos].trim();
                let trailing = after[pos + 2..].trim();
                if !inner.is_empty() {
                    let mut b = MdBlock::new(BlockKind::Equation);
                    b.text = inner.to_string();
                    blocks.push(b);
                }
                if !trailing.is_empty() {
                    blocks.push(MdBlock::para(trailing));
                }
                i += 1;
                continue;
            }
            // Multi-line collect until closing $$ (bail on blank / fence).
            let mut math_lines: Vec<String> = Vec::new();
            if !after.trim().is_empty() {
                math_lines.push(after.to_string());
            }
            let mut closed = false;
            let mut trailing = String::new();
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                if l.trim().is_empty() || l.trim_start().starts_with("```") || l.trim_start().starts_with("~~~") {
                    break;
                }
                if let Some(end) = l.find("$$") {
                    let before = &l[..end];
                    if !before.trim().is_empty() {
                        math_lines.push(before.to_string());
                    }
                    trailing = l[end + 2..].trim().to_string();
                    closed = true;
                    j += 1;
                    break;
                }
                math_lines.push(l.to_string());
                j += 1;
            }
            if closed {
                let text = math_lines.join("\n").trim().to_string();
                if !text.is_empty() {
                    let mut b = MdBlock::new(BlockKind::Equation);
                    b.text = text;
                    blocks.push(b);
                }
                if !trailing.is_empty() {
                    blocks.push(MdBlock::para(&trailing));
                }
                i = j;
                continue;
            }
            // unterminated — fall through as normal block
        }

        // Code fence
        if let Some(caps) = RE_FENCE.captures(line) {
            let fence = caps.get(1).unwrap().as_str().to_string();
            let lang = caps.get(2).unwrap().as_str().trim().to_string();
            let mut code_lines: Vec<String> = Vec::new();
            i += 1;
            while i < lines.len() {
                let stripped = lines[i].trim_start_matches(|c| c == ' ');
                // allow up to 3 leading spaces on closing fence
                let lead = lines[i].len() - lines[i].trim_start_matches(' ').len();
                if lead <= 3 && stripped.starts_with(&fence) {
                    break;
                }
                code_lines.push(lines[i].to_string());
                i += 1;
            }
            if i < lines.len() {
                i += 1; // closing fence
            }
            let mut b = MdBlock::new(BlockKind::CodeBlock);
            b.text = code_lines.join("\n");
            b.lang = lang;
            blocks.push(b);
            continue;
        }

        // Horizontal rule
        if RE_HR.is_match(line.trim()) {
            blocks.push(MdBlock::new(BlockKind::Hr));
            i += 1;
            continue;
        }

        // Heading
        if let Some(caps) = RE_HEADING.captures(line) {
            let mut b = MdBlock::new(BlockKind::Heading);
            b.text = caps.get(2).unwrap().as_str().trim().to_string();
            b.level = caps.get(1).unwrap().as_str().len() as u32;
            blocks.push(b);
            i += 1;
            continue;
        }

        // HTML table (merged / nested)
        let ltrim = line.trim_start();
        if starts_with_ci(ltrim, "<table")
            && ltrim
                .as_bytes()
                .get(6)
                .map(|&b| b == b' ' || b == b'>' || b == b'\t' || b == b'\n')
                .unwrap_or(true)
        {
            let mut html_lines: Vec<String> = Vec::new();
            let mut depth: i32 = 0;
            let mut closed = false;
            let mut j = i;
            while j < lines.len() {
                let l = lines[j];
                html_lines.push(l.to_string());
                depth += count_ci(l, "<table") as i32;
                depth -= count_ci(l, "</table>") as i32;
                j += 1;
                if depth <= 0 {
                    closed = true;
                    break;
                }
            }
            if closed {
                let mut b = MdBlock::new(BlockKind::HtmlTable);
                b.text = html_lines.join("\n");
                blocks.push(b);
                i = j;
                continue;
            }
            // unterminated — fall through
        }

        // GFM table
        if line.trim_start().starts_with('|') {
            let mut table_rows: Vec<Vec<String>> = Vec::new();
            let mut sep_seen = false;
            while i < lines.len() && lines[i].trim_start().starts_with('|') {
                let row = lines[i];
                if table_rows.len() == 1 && !sep_seen {
                    let trimmed = row.trim();
                    let inner = trimmed
                        .strip_prefix('|')
                        .unwrap_or(trimmed)
                        .strip_suffix('|')
                        .unwrap_or(trimmed);
                    let sep_cells: Vec<&str> = inner.split('|').collect();
                    if !sep_cells.is_empty() && sep_cells.iter().all(|c| RE_SEP_CELL.is_match(c)) {
                        sep_seen = true;
                        i += 1;
                        continue;
                    }
                }
                let cells = split_table_row(row);
                if !cells.is_empty() {
                    table_rows.push(cells);
                }
                i += 1;
            }
            if !table_rows.is_empty() {
                let mut b = MdBlock::new(BlockKind::Table);
                b.rows = table_rows;
                blocks.push(b);
            }
            continue;
        }

        // Blockquote — join consecutive `>` lines, split on blank `>`.
        if line.trim_start().starts_with("> ") {
            let mut quote_lines: Vec<String> = Vec::new();
            while i < lines.len()
                && (lines[i].trim_start().starts_with("> ") || lines[i].trim_start() == ">")
            {
                let stripped = lines[i]
                    .trim_start()
                    .strip_prefix('>')
                    .unwrap_or("")
                    .strip_prefix(' ')
                    .unwrap_or_else(|| lines[i].trim_start().strip_prefix('>').unwrap_or(""));
                quote_lines.push(stripped.trim().to_string());
                i += 1;
            }
            let mut joined: Vec<String> = Vec::new();
            for ql in quote_lines {
                if !ql.is_empty() {
                    joined.push(ql);
                    continue;
                }
                if !joined.is_empty() {
                    let mut b = MdBlock::new(BlockKind::Blockquote);
                    b.text = joined.join("\n");
                    blocks.push(b);
                }
                joined = Vec::new();
            }
            if !joined.is_empty() {
                let mut b = MdBlock::new(BlockKind::Blockquote);
                b.text = joined.join("\n");
                blocks.push(b);
            }
            continue;
        }

        // List — expand leading tabs to 2 spaces, indentation stack for depth.
        let list_line = expand_leading_tabs(line);
        if let Some(caps) = RE_LIST.captures(&list_line) {
            if blocks.is_empty() || blocks.last().unwrap().kind != BlockKind::ListItem {
                list_stack.clear();
            }
            let phys = caps.get(1).unwrap().as_str().len();
            let indent: usize;
            if list_stack.is_empty() {
                indent = phys / 2;
                for d in 0..indent {
                    list_stack.push(d * 2);
                }
                list_stack.push(phys);
            } else {
                while list_stack.len() > 1 && phys < *list_stack.last().unwrap() {
                    list_stack.pop();
                }
                if phys > *list_stack.last().unwrap() {
                    list_stack.push(phys);
                }
                indent = list_stack.len() - 1;
            }
            let marker = caps.get(2).unwrap().as_str();
            let ordered = marker.chars().any(|c| c.is_ascii_digit());
            let mut b = MdBlock::new(BlockKind::ListItem);
            b.text = caps.get(3).unwrap().as_str().trim().to_string();
            b.ordered = ordered;
            b.indent = indent;
            b.marker = marker.to_string();
            blocks.push(b);
            i += 1;
            continue;
        }

        // Plain paragraph
        blocks.push(MdBlock::para(line.trim()));
        i += 1;
    }

    blocks
}

/// ASCII case-insensitive prefix test (byte-safe on UTF-8 input).
fn starts_with_ci(s: &str, prefix: &str) -> bool {
    let sb = s.as_bytes();
    let pb = prefix.as_bytes();
    sb.len() >= pb.len() && sb[..pb.len()].eq_ignore_ascii_case(pb)
}

fn count_ci(haystack: &str, needle: &str) -> usize {
    let h = haystack.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    if n.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = h[start..].find(&n) {
        count += 1;
        start += pos + n.len();
    }
    count
}

fn expand_leading_tabs(line: &str) -> String {
    let ws_len = line.len() - line.trim_start_matches([' ', '\t']).len();
    let (ws, rest) = line.split_at(ws_len);
    let expanded: String = ws.chars().map(|c| if c == '\t' { "  ".to_string() } else { c.to_string() }).collect();
    format!("{expanded}{rest}")
}

// ─── Inline markdown → spans ────────────────────────

#[derive(Debug, Clone)]
pub struct InlineSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

lazy_static! {
    static ref RE_IMG: Regex = Regex::new(r"!\[([^\]]*)\]\([^)]*\)").unwrap();
    static ref RE_LINK: Regex = Regex::new(r"\[([^\]]*)\]\(([^)]*)\)").unwrap();
    static ref RE_STRIKE: Regex = Regex::new(r"~~([^~]+)~~").unwrap();
}

fn is_boundary(c: Option<char>) -> bool {
    match c {
        None => true,
        Some(c) => c.is_whitespace() || c.is_ascii_punctuation(),
    }
}

/// Parse one paragraph of inline markdown into styled spans.
pub fn parse_inline(text: &str) -> Vec<InlineSpan> {
    // Preprocess links/images/strike → text only.
    let text = RE_IMG.replace_all(text, "$1");
    let text = RE_LINK.replace_all(&text, |c: &regex::Captures| {
        let t = c.get(1).map(|m| m.as_str()).unwrap_or("");
        if t.is_empty() {
            c.get(2).map(|m| m.as_str()).unwrap_or("").to_string()
        } else {
            t.to_string()
        }
    });
    let text = RE_STRIKE.replace_all(&text, "$1");
    let chars: Vec<char> = text.chars().collect();

    let mut spans: Vec<InlineSpan> = Vec::new();
    let mut plain = String::new();
    let mut i = 0usize;

    let flush = |plain: &mut String, spans: &mut Vec<InlineSpan>| {
        if !plain.is_empty() {
            spans.push(InlineSpan {
                text: std::mem::take(plain),
                bold: false,
                italic: false,
                code: false,
            });
        }
    };

    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            // inline code: to next backtick
            if let Some(end) = find_char(&chars, i + 1, '`') {
                flush(&mut plain, &mut spans);
                let inner: String = chars[i + 1..end].iter().collect();
                spans.push(InlineSpan { text: inner, bold: false, italic: false, code: true });
                i = end + 1;
                continue;
            }
        } else if c == '*' || c == '_' {
            let run = marker_run(&chars, i, c).min(3);
            if let Some((inner_start, inner_end, close_end)) = find_emph(&chars, i, c, run) {
                flush(&mut plain, &mut spans);
                let inner: String = chars[inner_start..inner_end].iter().collect();
                let (bold, italic) = match run {
                    3 => (true, true),
                    2 => (true, false),
                    _ => (false, true),
                };
                spans.push(InlineSpan { text: inner, bold, italic, code: false });
                i = close_end;
                continue;
            }
        }
        plain.push(c);
        i += 1;
    }
    flush(&mut plain, &mut spans);

    if spans.is_empty() {
        spans.push(InlineSpan {
            text: text.to_string(),
            bold: false,
            italic: false,
            code: false,
        });
    }
    spans
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|&k| chars[k] == target)
}

/// Length of the run of `marker` starting at `i`.
fn marker_run(chars: &[char], i: usize, marker: char) -> usize {
    let mut n = 0;
    while i + n < chars.len() && chars[i + n] == marker {
        n += 1;
    }
    n
}

/// Find a closing run of `run` copies of `marker` after an opener at `i`.
/// Returns (inner_start, inner_end, close_end) on success.
fn find_emph(chars: &[char], i: usize, marker: char, run: usize) -> Option<(usize, usize, usize)> {
    let inner_start = i + run;
    if inner_start >= chars.len() {
        return None;
    }
    // Opening boundary rules.
    if chars[inner_start].is_whitespace() {
        return None;
    }
    if marker == '_' && !is_boundary(if i == 0 { None } else { Some(chars[i - 1]) }) {
        return None;
    }
    let mut k = inner_start;
    while k < chars.len() {
        if chars[k] == marker && marker_run(chars, k, marker) >= run {
            // candidate close at k..k+run
            let inner_end = k;
            if inner_end <= inner_start {
                return None;
            }
            // closing boundary: prev char not whitespace
            if chars[inner_end - 1].is_whitespace() {
                k += 1;
                continue;
            }
            let close_end = k + run;
            if marker == '_' {
                let after = chars.get(close_end).copied();
                if !is_boundary(after) {
                    k += 1;
                    continue;
                }
            }
            return Some((inner_start, inner_end, close_end));
        }
        k += 1;
    }
    None
}

fn span_char_pr_id(span: &InlineSpan) -> u32 {
    if span.code {
        CHAR_CODE
    } else if span.bold && span.italic {
        CHAR_BOLD_ITALIC
    } else if span.bold {
        CHAR_BOLD
    } else if span.italic {
        CHAR_ITALIC
    } else {
        CHAR_NORMAL
    }
}

/// Render inline markdown into `<hp:run>` XML. `map_char_id` optionally remaps
/// resolved charPr ids (used by table/preset styling).
pub fn generate_runs(
    text: &str,
    default_char_pr: u32,
    map_char_id: Option<&dyn Fn(u32) -> u32>,
) -> String {
    let spans = parse_inline(text);
    let mut out = String::new();
    for span in &spans {
        let mut char_id = if span.code || span.bold || span.italic {
            span_char_pr_id(span)
        } else {
            default_char_pr
        };
        if let Some(f) = map_char_id {
            char_id = f(char_id);
        }
        out.push_str(&format!(
            "<hp:run charPrIDRef=\"{}\"><hp:t>{}</hp:t></hp:run>",
            char_id,
            super::ids::escape_xml(&span.text)
        ));
    }
    out
}

/// Build a `<hp:p>` paragraph. Code blocks skip inline parsing.
pub fn generate_paragraph(
    text: &str,
    para_pr_id: u32,
    char_pr_id: u32,
    map_char_id: Option<&dyn Fn(u32) -> u32>,
    style_id: u32,
) -> String {
    if para_pr_id == PARA_CODE {
        return format!(
            "<hp:p paraPrIDRef=\"{para}\" styleIDRef=\"0\"><hp:run charPrIDRef=\"{code}\"><hp:t>{t}</hp:t></hp:run></hp:p>",
            para = para_pr_id,
            code = CHAR_CODE,
            t = super::ids::escape_xml(text)
        );
    }
    let runs = generate_runs(text, char_pr_id, map_char_id);
    format!(
        "<hp:p paraPrIDRef=\"{para}\" styleIDRef=\"{style}\">{runs}</hp:p>",
        para = para_pr_id,
        style = style_id,
        runs = runs
    )
}

use super::ids::PARA_CODE;

/// Preview/PrvText.txt — leading text snapshot (≤1KB).
pub fn build_prv_text(blocks: &[MdBlock]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut bytes = 0usize;
    for b in blocks {
        let mut text = if !b.text.is_empty() {
            b.text.clone()
        } else if !b.rows.is_empty() {
            b.rows.iter().map(|r| r.join(" ")).collect::<Vec<_>>().join("\n")
        } else {
            String::new()
        };
        if b.kind == BlockKind::CodeBlock && b.lang.eq_ignore_ascii_case("chart") {
            text = "[차트]".to_string();
        } else if b.kind == BlockKind::HtmlTable {
            text = strip_tags(&text);
        }
        if text.is_empty() {
            continue;
        }
        bytes += text.len() * 3;
        lines.push(text);
        if bytes > 1024 {
            break;
        }
    }
    let joined = lines.join("\n");
    joined.chars().take(1024).collect()
}

fn strip_tags(s: &str) -> String {
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
