//! Minimal Markdown → `IRBlock` parser feeding the print HTML pipeline.
//!
//! Loosely ports the *shape* of kkdoc's Markdown-input side of
//! `reference/kkdoc/src/print/renderer.ts::markdownToPdf` (Markdown →
//! `markdown-it` → HTML), but not its implementation: kkdoc uses the
//! `markdown-it` npm package, and this workspace has no equivalent Rust
//! dependency (no `pulldown-cmark` or similar in `core/Cargo.toml`). Adding
//! a full CommonMark parser for a single call site would cut against this
//! crate's existing minimal-dependency convention (see `crate::html` —
//! hand-rolled regex tag matching rather than a full HTML parser crate), so
//! this instead hand-parses the subset [`crate::ir::blocks_to_markdown`]
//! itself emits: ATX headings, paragraphs, unordered/ordered lists, GFM
//! pipe tables (no col/rowspan — a merged-cell table serializes to a raw
//! `<table>` HTML block instead, per `ir.rs`, which this parser does not
//! read back into an `IRTable`), image placeholders (`![alt](path)`), and
//! `---`/`***`/`___` separators.
//!
//! judgment call: if full CommonMark fidelity (nested emphasis, inline
//! code, blockquotes, fenced code blocks, HTML passthrough) is needed
//! later, swap this for `pulldown-cmark` — that's a Cargo.toml addition
//! plus reimplementing `markdown_to_ir` on top of its event stream, not a
//! source-compatible change to today's signature.

use crate::ir::{IRBlock, IRCell, IRTable, ListItem};

/// Parse a Markdown subset (see module doc comment) into `IRBlock`s.
pub fn markdown_to_ir(markdown: &str) -> Vec<IRBlock> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut blocks = Vec::new();
    let mut i = 0usize;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        if let Some((level, text)) = parse_heading(trimmed) {
            blocks.push(IRBlock::heading(level, text));
            i += 1;
            continue;
        }

        if is_separator(trimmed) {
            blocks.push(IRBlock::Separator);
            i += 1;
            continue;
        }

        if let Some(alt) = parse_image(trimmed) {
            blocks.push(IRBlock::Image { alt });
            i += 1;
            continue;
        }

        if trimmed.starts_with('|') || is_table_row(trimmed) {
            if let Some((table, consumed)) = try_parse_table(&lines, i) {
                blocks.push(IRBlock::Table(table));
                i += consumed;
                continue;
            }
        }

        if let Some(kind) = list_item_kind(trimmed) {
            let (items, consumed) = collect_list(&lines, i, kind);
            blocks.push(IRBlock::List {
                ordered: kind == ListKind::Ordered,
                items,
            });
            i += consumed;
            continue;
        }

        // Paragraph: consume consecutive non-blank lines that don't open
        // one of the block kinds above.
        let start = i;
        let mut text_lines = Vec::new();
        while i < lines.len() {
            let t = lines[i].trim();
            if t.is_empty()
                || parse_heading(t).is_some()
                || is_separator(t)
                || parse_image(t).is_some()
                || t.starts_with('|')
                || list_item_kind(t).is_some()
            {
                break;
            }
            text_lines.push(t);
            i += 1;
        }
        if i == start {
            // Defensive: shouldn't happen (paragraph branch only reached
            // when none of the above matched), but avoids an infinite loop
            // if a future block kind check above forgets to advance `i`.
            i += 1;
            continue;
        }
        blocks.push(IRBlock::paragraph(text_lines.join("\n")));
    }

    blocks
}

/// Markdown text → print HTML in one step: [`markdown_to_ir`] then
/// [`super::render_ir_to_html`].
pub fn render_markdown_to_html(markdown: &str, options: &super::RenderOptions) -> String {
    let blocks = markdown_to_ir(markdown);
    super::render_ir_to_html(&blocks, options)
}

fn parse_heading(line: &str) -> Option<(u8, &str)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    let text = rest.strip_prefix(' ')?;
    Some((hashes as u8, text.trim()))
}

fn is_separator(line: &str) -> bool {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() < 3 {
        return false;
    }
    let first = compact.chars().next().unwrap();
    (first == '-' || first == '*' || first == '_') && compact.chars().all(|c| c == first)
}

fn parse_image(line: &str) -> Option<String> {
    let rest = line.strip_prefix("![")?;
    let (alt, rest) = rest.split_once(']')?;
    let rest = rest.strip_prefix('(')?;
    if !rest.ends_with(')') {
        return None;
    }
    Some(alt.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ListKind {
    Ordered,
    Unordered,
}

fn list_item_kind(line: &str) -> Option<ListKind> {
    if line.starts_with("- ") || line.starts_with("* ") {
        return Some(ListKind::Unordered);
    }
    let digits: String = line.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &line[digits.len()..];
        if rest.starts_with(". ") {
            return Some(ListKind::Ordered);
        }
    }
    None
}

fn list_item_text(line: &str, kind: ListKind) -> String {
    match kind {
        ListKind::Unordered => line[2..].trim().to_string(),
        ListKind::Ordered => {
            let digits: usize = line.chars().take_while(|c| c.is_ascii_digit()).count();
            line[digits + 2..].trim().to_string()
        }
    }
}

/// Collects consecutive list items of the same `kind` starting at `lines[start]`.
/// Returns `(items, lines_consumed)`.
fn collect_list(lines: &[&str], start: usize, kind: ListKind) -> (Vec<ListItem>, usize) {
    let mut items = Vec::new();
    let mut i = start;
    while i < lines.len() {
        let raw = lines[i];
        let t = raw.trim();
        if t.is_empty() {
            break;
        }
        match list_item_kind(t) {
            Some(k) if k == kind => {
                let depth = markdown_indent_depth(raw);
                items.push(ListItem::new(list_item_text(t, kind), depth));
                i += 1;
            }
            _ => break,
        }
    }
    (items, i - start)
}

/// Nesting depth of a Markdown list item from its leading indentation:
/// two spaces (or one tab) per level, capped at 8. Top-level items (no
/// indent) are depth 0, keeping flat lists identical to the prior behavior.
fn markdown_indent_depth(raw: &str) -> u8 {
    let mut spaces = 0usize;
    for c in raw.chars() {
        match c {
            ' ' => spaces += 1,
            '\t' => spaces += 2,
            _ => break,
        }
    }
    ((spaces / 2).min(8)) as u8
}

/// A GFM table separator row, e.g. `|---|:--:|--:|`.
fn is_table_row(line: &str) -> bool {
    let inner = line.trim_matches('|');
    !inner.is_empty()
        && inner.split('|').all(|cell| {
            let c = cell.trim();
            !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':')
        })
}

/// Attempts to parse a GFM pipe table starting at `lines[start]`. Returns
/// `(table, lines_consumed)` on success. The table must have at least a
/// header row and a separator row (`|---|---|`) — a lone `|`-prefixed line
/// with no valid separator row underneath is left for the paragraph
/// fallback to consume instead.
fn try_parse_table(lines: &[&str], start: usize) -> Option<(IRTable, usize)> {
    if start + 1 >= lines.len() {
        return None;
    }
    let header = lines[start].trim();
    if !header.starts_with('|') && !header.contains('|') {
        return None;
    }
    let sep = lines[start + 1].trim();
    if !is_table_row(sep) {
        return None;
    }

    let mut rows = vec![split_table_row(header)];
    let mut i = start + 2;
    while i < lines.len() {
        let t = lines[i].trim();
        if t.is_empty() || !t.contains('|') {
            break;
        }
        rows.push(split_table_row(t));
        i += 1;
    }

    let cells: Vec<Vec<IRCell>> = rows
        .into_iter()
        .map(|row| row.into_iter().map(IRCell::new).collect())
        .collect();
    let table = IRTable::new(cells);
    Some((table, i - start))
}

fn split_table_row(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    // Split on unescaped `|`, then unescape `\|` → `|` — mirrors
    // `ir::render_table`'s own escaping (`cell.text.replace('|', "\\|")`).
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            current.push('|');
            chars.next();
        } else if c == '|' {
            cells.push(std::mem::take(&mut current).trim().to_string());
        } else {
            current.push(c);
        }
    }
    cells.push(current.trim().to_string());
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_parses_level_and_text() {
        let blocks = markdown_to_ir("# 장\n\n### 절");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], IRBlock::Heading { level: 1, text } if text == "장"));
        assert!(matches!(&blocks[1], IRBlock::Heading { level: 3, text } if text == "절"));
    }

    #[test]
    fn paragraph_parses_plain_text() {
        let blocks = markdown_to_ir("본문 첫째 줄\n본문 둘째 줄");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::Paragraph { text, .. } => assert_eq!(text, "본문 첫째 줄\n본문 둘째 줄"),
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn multiple_paragraphs_split_on_blank_line() {
        let blocks = markdown_to_ir("첫 단락\n\n둘째 단락");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn unordered_list_parses_items() {
        let blocks = markdown_to_ir("- 하나\n- 둘\n- 셋");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::List { ordered, items } => {
                assert!(!ordered);
                let texts: Vec<&str> = items.iter().map(|it| it.text.as_str()).collect();
                assert_eq!(texts, vec!["하나", "둘", "셋"]);
                assert!(items.iter().all(|it| it.depth == 0));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn nested_list_indentation_sets_item_depth() {
        // Two-space indentation → depth 1; the whole thing stays one list.
        let blocks = markdown_to_ir("- 상위\n  - 하위 A\n  - 하위 B\n- 다음 상위");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::List { ordered, items } => {
                assert!(!ordered);
                let depths: Vec<u8> = items.iter().map(|it| it.depth).collect();
                assert_eq!(depths, vec![0, 1, 1, 0]);
                assert_eq!(items[1].text, "하위 A");
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list_parses_items() {
        let blocks = markdown_to_ir("1. 첫째\n2. 둘째");
        match &blocks[0] {
            IRBlock::List { ordered, items } => {
                assert!(ordered);
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn gfm_table_parses_into_ir_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let blocks = markdown_to_ir(md);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            IRBlock::Table(t) => {
                assert_eq!(t.rows, 3);
                assert_eq!(t.cols, 2);
                assert_eq!(t.cells[0][0].text, "A");
                assert_eq!(t.cells[2][1].text, "4");
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn separator_parses() {
        let blocks = markdown_to_ir("본문\n\n---\n\n다음 본문");
        assert!(blocks.iter().any(|b| matches!(b, IRBlock::Separator)));
        assert_eq!(blocks.len(), 3);
    }

    #[test]
    fn image_placeholder_parses() {
        let blocks = markdown_to_ir("![image12](assets/image12)");
        assert!(matches!(&blocks[0], IRBlock::Image { alt } if alt == "image12"));
    }

    #[test]
    fn empty_input_yields_no_blocks() {
        assert!(markdown_to_ir("").is_empty());
        assert!(markdown_to_ir("\n\n\n").is_empty());
    }

    #[test]
    fn render_markdown_to_html_end_to_end() {
        let md = "# 제목\n\n본문입니다.\n\n- 항목1\n- 항목2";
        let html = super::super::render_markdown_to_html(md, &super::super::RenderOptions::default());
        assert!(html.contains("<h1>제목</h1>"));
        assert!(html.contains("<p>본문입니다.</p>"));
        assert!(html.contains("<li>항목1</li>"));
    }
}
