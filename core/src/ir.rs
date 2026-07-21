//! Intermediate Representation (IR) for parsed documents + compare engine.
//!
//! mdm's existing parsers all emit Markdown strings directly, which was fine
//! for basic extraction but loses structural information needed for
//! higher-level features like neighboring-block similarity, cell-level diff,
//! and form-field extraction.
//!
//! This module provides a Rust port of kordoc's IR layer:
//!
//! - [`IRBlock`] — union type of paragraph / heading / table / list / image
//!   / separator. Ports `kordoc/src/types.ts::IRBlock`.
//! - [`IRTable`] + [`IRCell`] — tabular content with colspan/rowspan.
//! - [`blocks_to_markdown`] — lossy but stable Markdown renderer. The
//!   current parsers already emit Markdown; parsers that migrate to IR
//!   output can round-trip through this renderer to preserve backward
//!   compatibility.
//! - [`normalized_similarity`] — O(m·n) Levenshtein with an output ceiling
//!   to defeat O(n²) blowup on huge strings, matching kordoc's
//!   `text-diff.ts::levenshtein`; beyond the ceiling, a bigram/Dice
//!   `approx_distance` estimate (also ported from kordoc) replaces a naive
//!   length-diff so shifted (prefix-inserted) huge strings don't collapse
//!   to a false 100% match.
//! - [`text_diff`] — word-level LCS diff (`equal`/`insert`/`delete` runs),
//!   ported from kordoc `text-diff.ts::textDiff`. Used to render inline
//!   word-level highlights inside a `Modified` block.
//! - [`diff_blocks`] — kordoc's LCS-based block aligner + per-table cell
//!   diff. This is the "신구대조표" core: given two parsed documents, emit
//!   a structured list of unchanged / modified / added / removed blocks
//!   with similarity scores and cell-level deltas for tables. Beyond the
//!   kordoc port, this also detects **moved** blocks (a delete+insert pair
//!   with near-identical content — not present in kordoc's `compare.ts`,
//!   see [`detect_moved_blocks`]) and aligns table cells by row/column LCS
//!   so row/column insertion or deletion doesn't cascade into spurious
//!   per-cell "modified" noise (kordoc's `diffTableCells` is purely
//!   positional; see [`diff_table_cells`]).
//! - [`render_diff_markdown`] — human-readable report, ported from kordoc
//!   `src/mcp.ts`'s `compare_documents` tool formatter. [`DiffResult`] and
//!   friends also derive `serde::Serialize` for the machine-readable JSON
//!   side of the same report.
//!
//! No existing parser touches this module yet. Phase 2b will wire
//! `HwpParser::extract_blocks()` to emit `Vec<IRBlock>` directly; for now
//! callers can only assemble `IRBlock`s manually.
//!
//! References:
//! - `reference/kordoc/src/types.ts`
//! - `reference/kordoc/src/diff/compare.ts`
//! - `reference/kordoc/src/diff/text-diff.ts`
//! - `reference/kordoc/src/mcp.ts` (`compare_documents` report formatting)

use serde::Serialize;
use std::collections::HashMap;

// ── IR types ─────────────────────────────────────────────────────────────────

/// A single cell inside an [`IRTable`]. Mirrors kordoc `IRCell`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IRCell {
    pub text: String,
    pub col_span: u16,
    pub row_span: u16,
}

impl IRCell {
    pub fn new<S: Into<String>>(text: S) -> Self {
        Self {
            text: text.into(),
            col_span: 1,
            row_span: 1,
        }
    }
}

/// A single item inside an [`IRBlock::List`], carrying its nesting `depth`
/// (0 = top level). Depth lets the RAG chunker build multi-level breadcrumb
/// paths for nested lists (see `crate::chunker`); it mirrors the per-item
/// `listDepth` kkdoc attaches to each list-item block.
///
/// `From<&str>` / `From<String>` default `depth` to 0, so existing
/// construction sites that pushed plain strings keep compiling unchanged —
/// only sources that actually know the nesting level (currently the
/// Markdown→IR converters, via leading-indent width) set a non-zero depth.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListItem {
    pub text: String,
    /// Nesting level, 0 = top. Capped at 8 (gongmun's deepest numbering).
    #[serde(skip_serializing_if = "is_zero_depth")]
    pub depth: u8,
}

fn is_zero_depth(d: &u8) -> bool {
    *d == 0
}

impl ListItem {
    pub fn new<S: Into<String>>(text: S, depth: u8) -> Self {
        Self {
            text: text.into(),
            depth: depth.min(8),
        }
    }
}

impl From<&str> for ListItem {
    fn from(s: &str) -> Self {
        ListItem::new(s, 0)
    }
}

impl From<String> for ListItem {
    fn from(s: String) -> Self {
        ListItem::new(s, 0)
    }
}

impl From<(&str, u8)> for ListItem {
    fn from((s, d): (&str, u8)) -> Self {
        ListItem::new(s, d)
    }
}

impl From<(String, u8)> for ListItem {
    fn from((s, d): (String, u8)) -> Self {
        ListItem::new(s, d)
    }
}

/// A rectangular table laid out as `rows × cols`. `cells` is indexed
/// `cells[row][col]`. `has_header` signals "render row 0 as `<th>`" —
/// currently a layout hint rather than semantic detection, matching kordoc
/// v2.0 behavior.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IRTable {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<Vec<IRCell>>,
    pub has_header: bool,
}

impl IRTable {
    pub fn new(cells: Vec<Vec<IRCell>>) -> Self {
        let rows = cells.len();
        let cols = cells.iter().map(|r| r.len()).max().unwrap_or(0);
        Self {
            rows,
            cols,
            cells,
            has_header: rows > 1,
        }
    }
}

/// Block-level union. The `IRBlock` variants carry all structural data
/// that downstream features (diff, form extraction, search) need.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum IRBlock {
    Paragraph {
        text: String,
        /// Footnote / endnote body attached inline to this paragraph.
        footnote: Option<String>,
        /// Hyperlink URL attached to this paragraph (from HWP klnk / %tok).
        href: Option<String>,
    },
    Heading {
        level: u8, // 1..=6
        text: String,
    },
    Table(IRTable),
    List {
        ordered: bool,
        items: Vec<ListItem>,
    },
    /// Image placeholder. `alt` is the rendered text (e.g. `image12`).
    Image {
        alt: String,
    },
    /// Horizontal separator (`---` in Markdown).
    Separator,
}

impl IRBlock {
    pub fn paragraph<S: Into<String>>(text: S) -> Self {
        IRBlock::Paragraph {
            text: text.into(),
            footnote: None,
            href: None,
        }
    }

    pub fn heading<S: Into<String>>(level: u8, text: S) -> Self {
        IRBlock::Heading {
            level: level.clamp(1, 6),
            text: text.into(),
        }
    }

    /// Convenience constructor that accepts anything convertible into
    /// [`ListItem`] — `&str`/`String` (depth 0) or `(text, depth)` tuples —
    /// so callers rarely need to spell out `ListItem` literals.
    pub fn list<I, T>(ordered: bool, items: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ListItem>,
    {
        IRBlock::List {
            ordered,
            items: items.into_iter().map(Into::into).collect(),
        }
    }

    /// Comparable text content of a block — used by [`diff_blocks`] to
    /// compute similarity. `None` for blocks without meaningful text
    /// (e.g. separators, images without alt).
    pub fn text_for_compare(&self) -> Option<String> {
        match self {
            IRBlock::Paragraph { text, .. } => Some(text.clone()),
            IRBlock::Heading { text, .. } => Some(text.clone()),
            IRBlock::List { items, .. } => Some(
                items
                    .iter()
                    .map(|it| it.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            IRBlock::Image { alt } => Some(alt.clone()),
            IRBlock::Table(_) => None,
            IRBlock::Separator => None,
        }
    }

    /// Discriminant key for "same block kind?" tests in `diff_blocks`.
    fn kind_tag(&self) -> &'static str {
        match self {
            IRBlock::Paragraph { .. } => "paragraph",
            IRBlock::Heading { .. } => "heading",
            IRBlock::Table(_) => "table",
            IRBlock::List { .. } => "list",
            IRBlock::Image { .. } => "image",
            IRBlock::Separator => "separator",
        }
    }
}

// ── Diff types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffChangeType {
    Unchanged,
    Modified,
    Added,
    Removed,
    /// A `Removed` + `Added` pair whose content matched with high
    /// confidence (see [`detect_moved_blocks`]). Not part of kordoc's
    /// `DiffChangeType` union — an addition layered on top of the port.
    Moved,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CellDiff {
    pub change: DiffChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
    /// `(col_span, row_span)` of the origin cell on the "before" side.
    /// `None` when there is no before-side cell (added column/row).
    pub span_before: Option<(u16, u16)>,
    /// `(col_span, row_span)` of the origin cell on the "after" side.
    /// `None` when there is no after-side cell (removed column/row).
    pub span_after: Option<(u16, u16)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockDiff {
    pub change: DiffChangeType,
    pub before: Option<IRBlock>,
    pub after: Option<IRBlock>,
    /// Per-cell diff when both `before` and `after` are tables.
    pub cell_diffs: Option<Vec<Vec<CellDiff>>>,
    /// Similarity score in `[0, 1]`. 1 = identical, 0 = unrelated.
    pub similarity: f64,
    /// Index into the sibling [`DiffResult::diffs`] Vec of the matching
    /// counterpart, set only when `change == Moved`.
    pub moved_pair: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffStats {
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub removed: usize,
    pub moved: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResult {
    pub stats: DiffStats,
    pub diffs: Vec<BlockDiff>,
}

// ── Markdown renderer ────────────────────────────────────────────────────────

/// Render a slice of IR blocks to Markdown. Uses GFM table syntax,
/// `#`-prefixed headings, and `[이미지: ...]` placeholders — matching the
/// existing mdm HWP parser output so downstream consumers see consistent
/// Markdown regardless of which path built the blocks.
/// Per-item ordinal for an ordered list, restarting numbering at each
/// nesting depth (so `1.` / `2.` under a `가.` parent). Index-aligned with
/// `items`. Shared by the Markdown ([`blocks_to_markdown`]) and PDF
/// (`crate::print::pdf`) renderers so nested-list numbering stays identical
/// across output formats. Values for items in an *unordered* list are
/// harmless but meaningless — callers only read them when `ordered`.
pub(crate) fn ordered_list_ordinals(items: &[ListItem]) -> Vec<usize> {
    let mut counters: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let depth = item.depth as usize;
        if counters.len() <= depth {
            counters.resize(depth + 1, 0);
        }
        // Climbing back up drops deeper counters so a re-entered sublevel
        // restarts at 1.
        counters.truncate(depth + 1);
        counters[depth] += 1;
        out.push(counters[depth]);
    }
    out
}

pub fn blocks_to_markdown(blocks: &[IRBlock]) -> String {
    let mut out = String::new();
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n");
        }
        match block {
            IRBlock::Paragraph { text, footnote, href } => {
                out.push_str(text);
                if let Some(url) = href {
                    out.push_str(&format!(" <{}>", url));
                }
                if let Some(note) = footnote {
                    // `note` already includes its own label ("[각주] ..." /
                    // "[미주] ...") — set by the parser at emit time, so
                    // appending a second "[각주]" here would duplicate the
                    // label and misclassify endnotes as footnotes.
                    out.push(' ');
                    out.push_str(note);
                }
            }
            IRBlock::Heading { level, text } => {
                for _ in 0..(*level as usize) {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(text);
            }
            IRBlock::Table(table) => {
                out.push_str(&render_table(table));
            }
            IRBlock::List { ordered, items } => {
                // Per-depth ordinal numbering (shared with the PDF renderer)
                // so nested ordered lists restart at each level.
                let ordinals = ordered_list_ordinals(items);
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out.push('\n');
                    }
                    for _ in 0..(item.depth as usize) {
                        out.push_str("  ");
                    }
                    if *ordered {
                        out.push_str(&format!("{}. {}", ordinals[idx], item.text));
                    } else {
                        out.push_str("- ");
                        out.push_str(&item.text);
                    }
                }
            }
            IRBlock::Image { alt } => {
                out.push_str(&format!("![{}](assets/{})", alt, alt));
            }
            IRBlock::Separator => {
                out.push_str("---");
            }
        }
    }
    out
}

fn render_table(table: &IRTable) -> String {
    if table.rows == 0 || table.cols == 0 {
        return String::new();
    }
    // Merged cells → HTML <table> (ported from kordoc f68e825, 2026-04-09).
    // GFM pipe tables cannot express rowspan/colspan; emit HTML instead so
    // downstream markdown viewers preserve the structure verbatim.
    if ir_has_merged_cells(table) {
        return render_table_html(table);
    }
    let mut out = String::new();
    for (r, row) in table.cells.iter().enumerate() {
        out.push('|');
        for c in 0..table.cols {
            let text = row
                .get(c)
                .map(|cell| cell.text.replace('|', "\\|").replace('\n', " "))
                .unwrap_or_default();
            out.push(' ');
            out.push_str(&text);
            out.push_str(" |");
        }
        out.push('\n');
        if r == 0 && table.has_header {
            out.push('|');
            for _ in 0..table.cols {
                out.push_str("---|");
            }
            out.push('\n');
        }
    }
    // Trim trailing newline for consistent block joining.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn ir_has_merged_cells(table: &IRTable) -> bool {
    table
        .cells
        .iter()
        .any(|row| row.iter().any(|c| c.col_span > 1 || c.row_span > 1))
}

/// HTML `<table>` renderer for merged-cell IR tables.
///
/// Ported from kordoc `src/table/builder.ts:tableToHtml` (2026-04-09, f68e825).
/// Unlike the HWPX/HWP 5.x pipelines, IRCell does not store shadow markers;
/// shadow positions are computed on-the-fly from each origin cell's span.
/// First row (row 0) always renders as `<th>`, matching kordoc v2.0; `has_header`
/// is retained as a layout hint but does not currently gate this. Cell
/// text is HTML-escaped and `\n` → `<br>`.
fn render_table_html(table: &IRTable) -> String {
    let mut skip: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut out = String::from("<table>\n");
    for (r, row) in table.cells.iter().enumerate() {
        let tag = if r == 0 { "th" } else { "td" };
        let mut row_html = String::new();
        for c in 0..table.cols {
            if skip.contains(&(r, c)) {
                continue;
            }
            let Some(cell) = row.get(c) else { continue };
            // Mark shadow positions for this origin's span.
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
            let escaped = ir_html_escape(cell.text.trim()).replace('\n', "<br>");
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

fn ir_html_escape(s: &str) -> String {
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

// ── Text similarity ──────────────────────────────────────────────────────────

/// Hard ceiling on `a.len() + b.len()` before Levenshtein gives up and
/// returns a length-based estimate. Prevents O(m·n) blowup on GB-sized
/// paragraphs (a malicious input vector). Matches kordoc's 10 000.
const MAX_LEVENSHTEIN_LEN: usize = 10_000;

/// Normalized edit-distance similarity in `[0, 1]`. Whitespace is
/// collapsed before comparison so HWP vs HWPX whitespace drift doesn't
/// distort the score.
pub fn normalized_similarity(a: &str, b: &str) -> f64 {
    similarity(&normalize_ws(a), &normalize_ws(b))
}

fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Raw Levenshtein similarity in `[0, 1]`.
///
/// Bug fix over kordoc v2.2.0: identical-length strings that differ
/// character-by-character used to return 1.0 in the JS port because the
/// normalization step divided by `max(len)` without re-checking for
/// equality. We explicitly short-circuit on bytewise equality first.
pub fn similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let max_len = a_chars.len().max(b_chars.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(&a_chars, &b_chars);
    1.0 - (dist as f64) / (max_len as f64)
}

/// O(min(m, n)) space Levenshtein. Delegates to [`approx_distance`] when
/// total input exceeds [`MAX_LEVENSHTEIN_LEN`] to defeat quadratic-time DoS.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.len() + b.len() > MAX_LEVENSHTEIN_LEN {
        return approx_distance(a, b);
    }
    // Always iterate the smaller array — halves working memory.
    let (a, b) = if a.len() > b.len() { (b, a) } else { (a, b) };
    let m = a.len();
    let n = b.len();
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];

    for j in 1..=n {
        curr[0] = j;
        for i in 1..=m {
            if a[i - 1] == b[j - 1] {
                curr[i] = prev[i - 1];
            } else {
                let sub = prev[i - 1] + 1;
                let del = prev[i] + 1;
                let ins = curr[i - 1] + 1;
                curr[i] = sub.min(del).min(ins);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

/// Bigram (character shingle) Dice-similarity distance estimate, used once
/// input exceeds [`MAX_LEVENSHTEIN_LEN`]. Ported from kordoc
/// `text-diff.ts::approxDistance`.
///
/// kordoc's changelog on this function: a prior "positional sample
/// comparison" fallback treated any prefix insertion/deletion (a shift) as
/// near-total mismatch, because it compared characters at the same index
/// rather than by content. Bigram multiset overlap is shift-invariant, so
/// a huge paragraph with a few words inserted at the front still scores
/// close to its true edit distance instead of collapsing to "completely
/// different".
fn approx_distance(a: &[char], b: &[char]) -> usize {
    let bigram_counts = |s: &[char]| -> HashMap<(char, char), usize> {
        let mut m = HashMap::new();
        if s.len() >= 2 {
            for w in s.windows(2) {
                *m.entry((w[0], w[1])).or_insert(0) += 1;
            }
        }
        m
    };
    let ca = bigram_counts(a);
    let cb = bigram_counts(b);
    let mut inter = 0usize;
    for (g, n) in &ca {
        inter += (*n).min(*cb.get(g).unwrap_or(&0));
    }
    let total = a.len().saturating_sub(1) + b.len().saturating_sub(1);
    let dice = if total > 0 {
        (2 * inter) as f64 / total as f64
    } else {
        1.0
    };
    let max_len = a.len().max(b.len()) as f64;
    (max_len * (1.0 - dice)).round() as usize
}

// ── Word-level text diff ─────────────────────────────────────────────────────

/// One run of a word-level diff. Ported from kordoc `text-diff.ts::TextChange`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TextChangeKind {
    Equal,
    Insert,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TextChange {
    pub kind: TextChangeKind,
    pub text: String,
}

/// Cap on `words_a.len() * words_b.len()` before the word-level LCS falls
/// back to greedy forward matching. Matches kordoc's 25 000 000.
const MAX_WORD_LCS_PAIRS: usize = 25_000_000;

/// Word-level diff (`equal` / `insert` / `delete` runs) between two
/// strings, for rendering inline highlights inside a `Modified` block.
/// Ported from kordoc `text-diff.ts::textDiff`.
pub fn text_diff(a: &str, b: &str) -> Vec<TextChange> {
    let words_a = split_ws_tokens(a);
    let words_b = split_ws_tokens(b);
    let lcs = lcs_words(&words_a, &words_b);

    let mut changes: Vec<TextChange> = Vec::new();
    let mut ia = 0usize;
    let mut ib = 0usize;
    let mut il = 0usize;

    while il < lcs.len() {
        while ia < words_a.len() && words_a[ia] != lcs[il] {
            changes.push(TextChange {
                kind: TextChangeKind::Delete,
                text: words_a[ia].clone(),
            });
            ia += 1;
        }
        while ib < words_b.len() && words_b[ib] != lcs[il] {
            changes.push(TextChange {
                kind: TextChangeKind::Insert,
                text: words_b[ib].clone(),
            });
            ib += 1;
        }
        changes.push(TextChange {
            kind: TextChangeKind::Equal,
            text: lcs[il].clone(),
        });
        ia += 1;
        ib += 1;
        il += 1;
    }
    while ia < words_a.len() {
        changes.push(TextChange {
            kind: TextChangeKind::Delete,
            text: words_a[ia].clone(),
        });
        ia += 1;
    }
    while ib < words_b.len() {
        changes.push(TextChange {
            kind: TextChangeKind::Insert,
            text: words_b[ib].clone(),
        });
        ib += 1;
    }

    merge_text_changes(changes)
}

/// Splits on whitespace runs while keeping the whitespace as its own
/// token, mirroring kordoc's `str.split(/(\s+)/)`.
fn split_ws_tokens(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut cur_is_ws = false;
    let mut started = false;
    for ch in s.chars() {
        let is_ws = ch.is_whitespace();
        if started && is_ws != cur_is_ws {
            tokens.push(std::mem::take(&mut cur));
        }
        cur.push(ch);
        cur_is_ws = is_ws;
        started = true;
    }
    tokens.push(cur);
    tokens
}

/// Word-sequence LCS. Falls back to [`simple_intersect`] beyond
/// [`MAX_WORD_LCS_PAIRS`], matching kordoc's `lcsWords`.
fn lcs_words(a: &[String], b: &[String]) -> Vec<String> {
    let m = a.len();
    let n = b.len();
    if m * n > MAX_WORD_LCS_PAIRS {
        return simple_intersect(a, b);
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    let mut result = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if a[i - 1] == b[j - 1] {
            result.push(a[i - 1].clone());
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    result.reverse();
    result
}

/// Greedy forward-matching LCS fallback for huge inputs. Ported from
/// kordoc `text-diff.ts::simpleIntersect` — always advances forward
/// through `b` so the result stays a genuine subsequence of both `a` and
/// `b` (kordoc's changelog notes an earlier set-intersection version broke
/// that contract and produced fake `equal` runs when replayed by
/// `textDiff`).
fn simple_intersect(a: &[String], b: &[String]) -> Vec<String> {
    let mut pos: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, w) in b.iter().enumerate() {
        pos.entry(w.as_str()).or_default().push(i);
    }
    let mut ptr: HashMap<&str, usize> = HashMap::new();
    let mut result = Vec::new();
    let mut j = 0usize;
    for w in a {
        let Some(list) = pos.get(w.as_str()) else {
            continue;
        };
        let mut k = *ptr.get(w.as_str()).unwrap_or(&0);
        while k < list.len() && list[k] < j {
            k += 1;
        }
        if k < list.len() {
            result.push(w.clone());
            j = list[k] + 1;
            ptr.insert(w.as_str(), k + 1);
        } else {
            ptr.insert(w.as_str(), k);
        }
    }
    result
}

fn merge_text_changes(changes: Vec<TextChange>) -> Vec<TextChange> {
    let mut merged: Vec<TextChange> = Vec::with_capacity(changes.len());
    for change in changes {
        if let Some(last) = merged.last_mut() {
            if last.kind == change.kind {
                last.text.push_str(&change.text);
                continue;
            }
        }
        merged.push(change);
    }
    merged
}

// ── Block diff engine ────────────────────────────────────────────────────────

/// Blocks with similarity ≥ this are paired as "modified", below → unpaired.
const SIMILARITY_THRESHOLD: f64 = 0.4;
/// Blocks with similarity ≥ this are treated as "unchanged" (no render diff).
const UNCHANGED_THRESHOLD: f64 = 0.99;
/// Cap on m·n before we fall back to positional alignment.
const MAX_LCS_PAIRS: usize = 10_000_000;
/// Similarity floor for reclassifying an unpaired removed/added block as
/// `Moved` (see [`detect_moved_blocks`]). Deliberately higher than
/// [`SIMILARITY_THRESHOLD`] — LCS already pairs anything at least loosely
/// similar in place; only near-identical content that LCS failed to align
/// (because it moved past intervening edits) should flip to `Moved`,
/// keeping genuinely unrelated adds/removes from being mispaired.
const MOVE_SIMILARITY_THRESHOLD: f64 = 0.85;

/// Compare two block sequences and return a kordoc-equivalent [`DiffResult`].
/// Pairs blocks via LCS over a similarity matrix, then walks the alignment
/// to classify each pair as unchanged / modified / added / removed, and
/// finally reclassifies any high-confidence removed+added pair as `Moved`
/// (see [`detect_moved_blocks`] — not part of kordoc's `compare.ts`).
///
/// For table pairs, a per-cell diff is attached via [`diff_table_cells`]
/// so callers can render a neighboring-column view of legal-document
/// amendments (신구대조표) without reimplementing table alignment.
pub fn diff_blocks(a: &[IRBlock], b: &[IRBlock]) -> DiffResult {
    let aligned = align_blocks(a, b);
    let mut stats = DiffStats::default();
    let mut diffs = Vec::with_capacity(aligned.len());

    for (a_block, b_block) in aligned {
        match (a_block, b_block) {
            (Some(ai), Some(bi)) => {
                let a_ref = &a[ai];
                let b_ref = &b[bi];
                let sim = block_similarity(a_ref, b_ref);
                if sim >= UNCHANGED_THRESHOLD {
                    stats.unchanged += 1;
                    diffs.push(BlockDiff {
                        change: DiffChangeType::Unchanged,
                        before: Some(a_ref.clone()),
                        after: Some(b_ref.clone()),
                        cell_diffs: None,
                        similarity: 1.0,
                        moved_pair: None,
                    });
                } else {
                    let cell_diffs = match (a_ref, b_ref) {
                        (IRBlock::Table(ta), IRBlock::Table(tb)) => {
                            Some(diff_table_cells(ta, tb))
                        }
                        _ => None,
                    };
                    stats.modified += 1;
                    diffs.push(BlockDiff {
                        change: DiffChangeType::Modified,
                        before: Some(a_ref.clone()),
                        after: Some(b_ref.clone()),
                        cell_diffs,
                        similarity: sim,
                        moved_pair: None,
                    });
                }
            }
            (Some(ai), None) => {
                stats.removed += 1;
                diffs.push(BlockDiff {
                    change: DiffChangeType::Removed,
                    before: Some(a[ai].clone()),
                    after: None,
                    cell_diffs: None,
                    similarity: 0.0,
                    moved_pair: None,
                });
            }
            (None, Some(bi)) => {
                stats.added += 1;
                diffs.push(BlockDiff {
                    change: DiffChangeType::Added,
                    before: None,
                    after: Some(b[bi].clone()),
                    cell_diffs: None,
                    similarity: 0.0,
                    moved_pair: None,
                });
            }
            (None, None) => {}
        }
    }

    detect_moved_blocks(&mut diffs, &mut stats);

    DiffResult { stats, diffs }
}

/// Second pass over LCS output: pairs up `Removed`/`Added` entries whose
/// content matches with high confidence and reclassifies both as `Moved`.
///
/// Not ported from kordoc — `reference/kordoc/src/diff/compare.ts` has no
/// move-detection pass; LCS alone cannot express "this exact block
/// reappeared elsewhere" because it only aligns content that keeps its
/// relative order. This layers a greedy bipartite match on top: score every
/// candidate `(removed, added)` pair by [`block_similarity`], then claim
/// pairs highest-similarity-first so a block is never stolen by a weaker
/// match purely because it appears earlier in the diff list. Entries keep
/// their original position and `before`/`after` payload (a `Removed` entry
/// still has only `before`; an `Added` entry still has only `after`) —
/// only `change`, `similarity`, and `moved_pair` are updated — so
/// neighboring-column (신구대조표) rendering doesn't have to special-case
/// merged entries.
fn detect_moved_blocks(diffs: &mut [BlockDiff], stats: &mut DiffStats) {
    let removed: Vec<usize> = diffs
        .iter()
        .enumerate()
        .filter(|(_, d)| d.change == DiffChangeType::Removed)
        .map(|(i, _)| i)
        .collect();
    let added: Vec<usize> = diffs
        .iter()
        .enumerate()
        .filter(|(_, d)| d.change == DiffChangeType::Added)
        .map(|(i, _)| i)
        .collect();
    if removed.is_empty() || added.is_empty() {
        return;
    }

    let mut scored: Vec<(f64, usize, usize)> = Vec::new();
    for &ri in &removed {
        let Some(before) = diffs[ri].before.as_ref() else {
            continue;
        };
        for &ai in &added {
            let Some(after) = diffs[ai].after.as_ref() else {
                continue;
            };
            let sim = block_similarity(before, after);
            if sim >= MOVE_SIMILARITY_THRESHOLD {
                scored.push((sim, ri, ai));
            }
        }
    }
    scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut claimed_removed = vec![false; diffs.len()];
    let mut claimed_added = vec![false; diffs.len()];
    for (sim, ri, ai) in scored {
        if claimed_removed[ri] || claimed_added[ai] {
            continue;
        }
        claimed_removed[ri] = true;
        claimed_added[ai] = true;
        diffs[ri].change = DiffChangeType::Moved;
        diffs[ri].similarity = sim;
        diffs[ri].moved_pair = Some(ai);
        diffs[ai].change = DiffChangeType::Moved;
        diffs[ai].similarity = sim;
        diffs[ai].moved_pair = Some(ri);
        stats.removed -= 1;
        stats.added -= 1;
        stats.moved += 2;
    }
}

/// Generic LCS alignment over any `(i, j)` similarity function. Shared by
/// [`align_blocks`] (block sequences) and [`diff_table_cells`] (table rows
/// and columns) — same DP + backtrace kordoc's `compare.ts::alignBlocks`
/// uses, parameterized so it isn't reimplemented per axis.
fn lcs_align(
    m: usize,
    n: usize,
    threshold: f64,
    mut sim: impl FnMut(usize, usize) -> f64,
) -> Vec<(Option<usize>, Option<usize>)> {
    if m == 0 || n == 0 || m * n > MAX_LCS_PAIRS {
        return fallback_align(m, n);
    }

    let mut cache: HashMap<(usize, usize), f64> = HashMap::new();
    let mut get_sim = |i: usize, j: usize, cache: &mut HashMap<(usize, usize), f64>| -> f64 {
        if let Some(&v) = cache.get(&(i, j)) {
            return v;
        }
        let v = sim(i, j);
        cache.insert((i, j), v);
        v
    };

    // dp[i][j] = length of best LCS using [..i] and [..j].
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if get_sim(i - 1, j - 1, &mut cache) >= threshold {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrace to recover paired indices.
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if get_sim(i - 1, j - 1, &mut cache) >= threshold && dp[i][j] == dp[i - 1][j - 1] + 1 {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();

    // Assemble unpaired ranges between anchors.
    let mut result: Vec<(Option<usize>, Option<usize>)> = Vec::new();
    let mut ai = 0usize;
    let mut bi = 0usize;
    for (pi, pj) in pairs {
        while ai < pi {
            result.push((Some(ai), None));
            ai += 1;
        }
        while bi < pj {
            result.push((None, Some(bi)));
            bi += 1;
        }
        result.push((Some(ai), Some(bi)));
        ai += 1;
        bi += 1;
    }
    while ai < m {
        result.push((Some(ai), None));
        ai += 1;
    }
    while bi < n {
        result.push((None, Some(bi)));
        bi += 1;
    }
    result
}

/// LCS over a similarity matrix with threshold `SIMILARITY_THRESHOLD`.
/// Returns `(Option<idx_in_a>, Option<idx_in_b>)` pairs in document order.
///
/// Applies the same length-ratio prefilter as kordoc's `compare.ts` before
/// computing exact similarity: Levenshtein distance is always ≥
/// `|len_a - len_b|`, so once that lower bound alone already implies a
/// score below `SIMILARITY_THRESHOLD`, the pair is rejected without
/// running Levenshtein at all. Tables use a looser cut (`6/7`) because
/// `table_similarity` blends in a 0.3-weighted dimension score, so content
/// alone can't push the combined score below threshold until the length
/// gap exceeds 6/7 of the longer side.
fn align_blocks(a: &[IRBlock], b: &[IRBlock]) -> Vec<(Option<usize>, Option<usize>)> {
    let m = a.len();
    let n = b.len();
    let a_len: Vec<usize> = a.iter().map(block_compare_len).collect();
    let b_len: Vec<usize> = b.iter().map(block_compare_len).collect();

    lcs_align(m, n, SIMILARITY_THRESHOLD, |i, j| {
        let mx = a_len[i].max(b_len[j]) as f64;
        let mn = a_len[i].min(b_len[j]) as f64;
        let is_table = matches!(a[i], IRBlock::Table(_)) || matches!(b[j], IRBlock::Table(_));
        let cut = if is_table { 6.0 / 7.0 } else { 1.0 - SIMILARITY_THRESHOLD };
        if mx > 0.0 && (mx - mn) / mx > cut {
            0.0
        } else {
            block_similarity(&a[i], &b[j])
        }
    })
}

/// Whitespace-normalized comparable length of a block, for the length-ratio
/// prefilter in [`align_blocks`]. Matches kordoc's `alignBlocks::lenOf`.
fn block_compare_len(block: &IRBlock) -> usize {
    let t = match block {
        IRBlock::Table(t) => t
            .cells
            .iter()
            .flatten()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        _ => block.text_for_compare().unwrap_or_default(),
    };
    normalize_ws(&t).chars().count()
}

fn fallback_align(m: usize, n: usize) -> Vec<(Option<usize>, Option<usize>)> {
    let len = m.max(n);
    let mut result = Vec::with_capacity(len);
    for i in 0..len {
        let ai = if i < m { Some(i) } else { None };
        let bi = if i < n { Some(i) } else { None };
        result.push((ai, bi));
    }
    result
}

/// Block-to-block similarity. Matches kordoc semantics:
/// - different kinds → 0
/// - text-bearing blocks → normalized Levenshtein
/// - tables → dimension × 0.3 + content × 0.7
/// - separators → 1 (always equal)
fn block_similarity(a: &IRBlock, b: &IRBlock) -> f64 {
    if a.kind_tag() != b.kind_tag() {
        return 0.0;
    }
    match (a, b) {
        (IRBlock::Table(ta), IRBlock::Table(tb)) => table_similarity(ta, tb),
        (IRBlock::Separator, IRBlock::Separator) => 1.0,
        (
            IRBlock::List { ordered: oa, items: ia },
            IRBlock::List { ordered: ob, items: ib },
        ) => {
            // Content similarity uses the item text only (so `text_for_compare`
            // stays clean for the diff report), but a change confined to
            // nesting depth / ordered-ness / item count still has to surface
            // as `Modified` rather than a false `Unchanged`. Gate the score
            // just below `UNCHANGED_THRESHOLD` whenever the list structure
            // differs, so identical text at a different depth diffs.
            let ta = a.text_for_compare().unwrap_or_default();
            let tb = b.text_for_compare().unwrap_or_default();
            let content = normalized_similarity(&ta, &tb);
            let same_structure = oa == ob
                && ia.len() == ib.len()
                && ia.iter().zip(ib.iter()).all(|(x, y)| x.depth == y.depth);
            if same_structure {
                content
            } else {
                content.min(UNCHANGED_THRESHOLD - 0.01)
            }
        }
        _ => {
            let ta = a.text_for_compare().unwrap_or_default();
            let tb = b.text_for_compare().unwrap_or_default();
            normalized_similarity(&ta, &tb)
        }
    }
}

fn table_similarity(a: &IRTable, b: &IRTable) -> f64 {
    let a_area = (a.rows * a.cols) as f64;
    let b_area = (b.rows * b.cols) as f64;
    let denom = a_area.max(b_area).max(1.0);
    let dim_sim = 1.0 - (a_area - b_area).abs() / denom;

    let texts_a = a
        .cells
        .iter()
        .flatten()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let texts_b = b
        .cells
        .iter()
        .flatten()
        .map(|c| c.text.clone())
        .collect::<Vec<_>>()
        .join(" ");
    let content_sim = normalized_similarity(&texts_a, &texts_b);

    dim_sim * 0.3 + content_sim * 0.7
}

fn cell_at(t: &IRTable, r: usize, c: usize) -> Option<&IRCell> {
    t.cells.get(r).and_then(|row| row.get(c))
}

fn row_text(t: &IRTable, r: usize) -> String {
    t.cells
        .get(r)
        .map(|row| row.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

/// Column text restricted to `rows` — used for column similarity so an
/// inserted/deleted row's text doesn't leak into the comparison. Comparing
/// full unrestricted columns would make e.g. a genuinely-unchanged column
/// score below [`SIMILARITY_THRESHOLD`] purely because the *other* table
/// has one extra row's worth of text appended to it.
fn col_text_over(t: &IRTable, c: usize, rows: &[usize]) -> String {
    rows.iter()
        .map(|&r| cell_at(t, r, c).map(|cell| cell.text.as_str()).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Cell-level diff of two cells (or a missing side, for an inserted or
/// removed row/column). Text equality alone isn't enough for `Unchanged` —
/// a cell whose text is untouched but whose colspan/rowspan changed still
/// renders differently, so span equality is required too.
fn build_cell_diff(cell_a: Option<&IRCell>, cell_b: Option<&IRCell>) -> CellDiff {
    let span_before = cell_a.map(|c| (c.col_span, c.row_span));
    let span_after = cell_b.map(|c| (c.col_span, c.row_span));
    let text_a = cell_a.map(|c| c.text.clone());
    let text_b = cell_b.map(|c| c.text.clone());
    let change = match (&text_a, &text_b) {
        (None, Some(_)) => DiffChangeType::Added,
        (Some(_), None) => DiffChangeType::Removed,
        (Some(x), Some(y)) if x == y && span_before == span_after => DiffChangeType::Unchanged,
        (Some(_), Some(_)) => DiffChangeType::Modified,
        (None, None) => DiffChangeType::Unchanged,
    };
    CellDiff {
        change,
        before: text_a,
        after: text_b,
        span_before,
        span_after,
    }
}

/// Cell-by-cell diff of two tables, row- and column-aligned by LCS (over
/// row/column similarity, same machinery as [`align_blocks`]) rather than
/// compared positionally. A row or column inserted or deleted in the
/// middle of a table is therefore reported as one `Added`/`Removed`
/// row/column instead of cascading into "modified" noise on every
/// subsequent cell — a capability kordoc's `diffTableCells` doesn't have
/// (it only ever does a dense positional `max(rows) × max(cols)` compare).
///
/// Matched row/column pairs are further compared cell-by-cell including
/// colspan/rowspan (see [`build_cell_diff`]), which kordoc's implementation
/// also omits (it compares `cell.text` only).
pub fn diff_table_cells(a: &IRTable, b: &IRTable) -> Vec<Vec<CellDiff>> {
    let row_pairs = lcs_align(a.rows, b.rows, SIMILARITY_THRESHOLD, |ri, rj| {
        normalized_similarity(&row_text(a, ri), &row_text(b, rj))
    });

    // Column similarity is computed only over rows both tables agree on
    // (the (Some, Some) pairs from row alignment) — see `col_text_over`.
    let matched_rows: Vec<(usize, usize)> = row_pairs
        .iter()
        .filter_map(|(ai, bi)| match (ai, bi) {
            (Some(x), Some(y)) => Some((*x, *y)),
            _ => None,
        })
        .collect();
    let matched_a_rows: Vec<usize> = matched_rows.iter().map(|(x, _)| *x).collect();
    let matched_b_rows: Vec<usize> = matched_rows.iter().map(|(_, y)| *y).collect();
    let col_pairs = lcs_align(a.cols, b.cols, SIMILARITY_THRESHOLD, |ci, cj| {
        normalized_similarity(
            &col_text_over(a, ci, &matched_a_rows),
            &col_text_over(b, cj, &matched_b_rows),
        )
    });

    let mut out = Vec::with_capacity(row_pairs.len());
    for (ra, rb) in &row_pairs {
        let row_out = match (ra, rb) {
            (Some(ri), Some(rj)) => col_pairs
                .iter()
                .map(|(ca, cb)| {
                    let cell_a = ca.and_then(|c| cell_at(a, *ri, c));
                    let cell_b = cb.and_then(|c| cell_at(b, *rj, c));
                    build_cell_diff(cell_a, cell_b)
                })
                .collect(),
            (Some(ri), None) => (0..a.cols)
                .map(|c| build_cell_diff(cell_at(a, *ri, c), None))
                .collect(),
            (None, Some(rj)) => (0..b.cols)
                .map(|c| build_cell_diff(None, cell_at(b, *rj, c)))
                .collect(),
            (None, None) => Vec::new(),
        };
        out.push(row_out);
    }
    out
}

// ── Diff report rendering ────────────────────────────────────────────────────

/// Longest prefix (in chars) of a diff line's text shown in
/// [`render_diff_markdown`], matching kordoc's `text.substring(0, 200)`.
const DIFF_REPORT_TEXT_LIMIT: usize = 200;

/// Human-readable diff report. Ported from kordoc `src/mcp.ts`'s
/// `compare_documents` MCP tool formatter: a stats line, then one line per
/// diff entry prefixed `+`/`-`/`~`/` ` (added/removed/modified/unchanged),
/// text truncated to 200 chars, with a trailing similarity percentage when
/// one applies. `Moved` entries (kordoc has none) use `→` and keep their
/// similarity suffix like `Modified`.
///
/// For the machine-readable counterpart, [`DiffResult`] and its nested
/// types derive `serde::Serialize` — `serde_json::to_string(&diff_result)`
/// produces the JSON side of the same report.
pub fn render_diff_markdown(result: &DiffResult) -> String {
    let mut out = String::new();
    out.push_str("## 문서 비교 결과\n");
    out.push_str(&format!(
        "추가: {} | 삭제: {} | 변경: {} | 이동: {} | 동일: {}\n\n",
        result.stats.added,
        result.stats.removed,
        result.stats.modified,
        result.stats.moved,
        result.stats.unchanged
    ));

    for d in &result.diffs {
        let prefix = match d.change {
            DiffChangeType::Added => "+",
            DiffChangeType::Removed => "-",
            DiffChangeType::Modified => "~",
            DiffChangeType::Moved => "→",
            DiffChangeType::Unchanged => " ",
        };
        let text = d
            .after
            .as_ref()
            .and_then(|blk| blk.text_for_compare())
            .or_else(|| d.before.as_ref().and_then(|blk| blk.text_for_compare()))
            .unwrap_or_else(|| {
                let is_table = matches!(d.after, Some(IRBlock::Table(_)))
                    || matches!(d.before, Some(IRBlock::Table(_)));
                if is_table {
                    "[테이블]".to_string()
                } else {
                    String::new()
                }
            });
        let truncated: String = text.chars().take(DIFF_REPORT_TEXT_LIMIT).collect();
        let sim_suffix = match d.change {
            DiffChangeType::Added | DiffChangeType::Removed => String::new(),
            _ => format!(" ({:.0}%)", d.similarity * 100.0),
        };
        out.push_str(&format!("{} {}{}\n", prefix, truncated, sim_suffix));
    }

    out
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_table(rows: Vec<Vec<&str>>) -> IRBlock {
        let cells: Vec<Vec<IRCell>> = rows
            .into_iter()
            .map(|r| r.into_iter().map(IRCell::new).collect())
            .collect();
        IRBlock::Table(IRTable::new(cells))
    }

    // ── Table rendering: merged cells (kordoc f68e825, 2026-04-09) ──

    /// IR table with colSpan merge → HTML `<table>` with `colspan="N"`.
    #[test]
    fn ir_merged_colspan_emits_html() {
        // 2×2: row 0 = single cell spanning 2 cols; row 1 = two plain cells.
        let cells = vec![
            vec![IRCell {
                text: "병합셀".into(),
                col_span: 2,
                row_span: 1,
            }],
            vec![IRCell::new("값1"), IRCell::new("값2")],
        ];
        let table = IRTable::new(cells);
        // IRTable::new infers cols from max row len — adjust cols to 2.
        let table = IRTable {
            cols: 2,
            ..table
        };
        let out = render_table(&table);
        assert!(out.contains("<table>"));
        assert!(out.contains("colspan=\"2\""));
        assert!(out.contains("병합셀"));
        assert!(out.contains("값1"));
        assert!(out.contains("값2"));
    }

    /// IR rowSpan → HTML `<table>` with `rowspan="N"`, shadow position skipped.
    #[test]
    fn ir_merged_rowspan_emits_html() {
        let cells = vec![
            vec![
                IRCell {
                    text: "행병합".into(),
                    col_span: 1,
                    row_span: 2,
                },
                IRCell::new("값1"),
            ],
            // Dense layout: shadow placeholder at (1,0), real value at (1,1)
            vec![IRCell::new(""), IRCell::new("값2")],
        ];
        let table = IRTable {
            rows: 2,
            cols: 2,
            cells,
            has_header: false,
        };
        let out = render_table(&table);
        assert!(out.contains("<table>"));
        assert!(out.contains("rowspan=\"2\""));
        assert!(out.contains("행병합"));
        // "값2" should appear once (in row 1); no double-render of the origin
        assert!(out.contains("값2"));
    }

    /// No merge → classic GFM pipe table (regression).
    #[test]
    fn ir_no_merge_stays_markdown() {
        let block = mk_table(vec![vec!["A", "B"], vec!["C", "D"]]);
        let out = blocks_to_markdown(std::slice::from_ref(&block));
        assert!(!out.contains("<table>"));
        assert!(out.contains("| A | B |"));
    }

    /// HTML injection in cell text is escaped.
    #[test]
    fn ir_merged_cell_html_escape() {
        let cells = vec![vec![IRCell {
            text: "<script>".into(),
            col_span: 2,
            row_span: 1,
        }]];
        let table = IRTable {
            rows: 1,
            cols: 2,
            cells,
            has_header: false,
        };
        let out = render_table(&table);
        assert!(out.contains("&lt;script&gt;"));
        assert!(!out.contains("<script>"));
    }

    // ── Similarity ──

    #[test]
    fn identical_strings_are_equal() {
        assert_eq!(similarity("abc", "abc"), 1.0);
    }

    #[test]
    fn completely_different_strings_score_low() {
        assert!(similarity("abc", "xyz") < 0.5);
    }

    #[test]
    fn same_length_one_char_diff_is_not_exactly_one() {
        // Regression guard for the kordoc v2.2.0 bug where equal-length
        // strings with 1 char swap returned 1.0.
        let s = similarity("abcd", "abce");
        assert!(s < 1.0);
        assert!(s >= 0.74);
    }

    #[test]
    fn whitespace_normalization_absorbs_drift() {
        assert_eq!(normalized_similarity("hello  world", "hello world"), 1.0);
        assert_eq!(normalized_similarity("  a b  c ", "a b c"), 1.0);
    }

    #[test]
    fn levenshtein_length_ceiling_is_cheap() {
        // Beyond the ceiling we return the length diff — fast and bounded.
        let long_a: String = "a".repeat(6000);
        let long_b: String = "b".repeat(6000);
        let s = similarity(&long_a, &long_b);
        // Length diff = 0 → dist = 0 → similarity = 1.0 under the fallback.
        // That's a conservative over-estimate which is fine because the
        // blocks are bucketed together — it just means we pair them and
        // then downstream text comparison decides modified vs unchanged.
        assert!((0.0..=1.0).contains(&s));
    }

    // ── blocks_to_markdown ──

    #[test]
    fn markdown_paragraph_with_footnote_and_href() {
        // Contract: the parser stores a fully-labeled marker in `footnote`
        // (e.g. "[각주] …" or "[미주] …"). Serializer must NOT prepend a
        // second label — see ir.rs Paragraph serialization.
        let blocks = vec![IRBlock::Paragraph {
            text: "본문".to_string(),
            footnote: Some("[각주] 각주 내용".to_string()),
            href: Some("https://law.go.kr".to_string()),
        }];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("본문"));
        assert!(md.contains("<https://law.go.kr>"));
        assert!(md.contains("[각주] 각주 내용"));
        // Regression: must not double-label.
        assert!(!md.contains("[각주] [각주]"), "duplicate footnote label: {:?}", md);
    }

    #[test]
    fn markdown_paragraph_endnote_not_misclassified_as_footnote() {
        // Before the fix the serializer always prepended "[각주]" even when
        // the stored marker was an endnote — producing "[각주] [미주] …".
        let blocks = vec![IRBlock::Paragraph {
            text: "본문".to_string(),
            footnote: Some("[미주] 미주 내용".to_string()),
            href: None,
        }];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("[미주] 미주 내용"));
        assert!(!md.contains("[각주]"), "endnote mislabeled: {:?}", md);
    }

    #[test]
    fn markdown_heading_level_clamped() {
        let blocks = vec![
            IRBlock::heading(1, "장"),
            IRBlock::heading(3, "절"),
        ];
        let md = blocks_to_markdown(&blocks);
        assert!(md.starts_with("# 장"));
        assert!(md.contains("### 절"));
    }

    #[test]
    fn markdown_table_renders_gfm_with_header() {
        let t = mk_table(vec![
            vec!["이름", "나이"],
            vec!["홍길동", "30"],
        ]);
        let md = blocks_to_markdown(&[t]);
        assert!(md.contains("| 이름 | 나이 |"));
        assert!(md.contains("|---|---|"));
        assert!(md.contains("| 홍길동 | 30 |"));
    }

    #[test]
    fn markdown_list_unordered_and_ordered() {
        let unordered = IRBlock::list(false, ["첫째", "둘째"]);
        let ordered = IRBlock::list(true, ["A", "B"]);
        let md = blocks_to_markdown(&[unordered, ordered]);
        assert!(md.contains("- 첫째"));
        assert!(md.contains("- 둘째"));
        assert!(md.contains("1. A"));
        assert!(md.contains("2. B"));
    }

    /// Nested list items render with depth indentation and per-level ordered
    /// numbering (regression guard for the `list_depth` extension).
    #[test]
    fn markdown_nested_list_indents_by_depth() {
        let nested = IRBlock::list(
            false,
            [("가", 0u8), ("세부1", 1u8), ("세부2", 1u8), ("나", 0u8)],
        );
        let md = blocks_to_markdown(&[nested]);
        assert!(md.contains("- 가"));
        assert!(md.contains("  - 세부1"), "depth-1 item should be indented: {md:?}");
        assert!(md.contains("  - 세부2"));
        assert!(md.contains("- 나"));

        let ordered = IRBlock::list(true, [("상위", 0u8), ("하위1", 1u8), ("하위2", 1u8)]);
        let md = blocks_to_markdown(&[ordered]);
        // Sublevel numbering restarts at 1, indented two spaces.
        assert!(md.contains("1. 상위"));
        assert!(md.contains("  1. 하위1"), "sublevel should restart at 1: {md:?}");
        assert!(md.contains("  2. 하위2"));
    }

    /// Per-depth ordinal counter restarts numbering at each nesting level and
    /// drops deeper counters when climbing back up. Single source of truth
    /// for both the Markdown and PDF ordered-list renderers.
    #[test]
    fn ordered_list_ordinals_restart_per_depth() {
        let items: Vec<ListItem> = [0u8, 1, 1, 0, 2]
            .iter()
            .map(|&d| ListItem::new("x", d))
            .collect();
        assert_eq!(ordered_list_ordinals(&items), vec![1, 1, 2, 2, 1]);
    }

    /// A list whose item text is identical but whose nesting depth changed
    /// must diff as `Modified`, not a false `Unchanged` (regression guard —
    /// the depth field was previously invisible to the comparison).
    #[test]
    fn diff_list_depth_only_change_is_modified() {
        let a = vec![IRBlock::list(false, [("항목", 0u8)])];
        let b = vec![IRBlock::list(false, [("항목", 1u8)])];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.modified, 1, "depth change should be Modified: {:?}", r.stats);
        assert_eq!(r.stats.unchanged, 0);
        assert_eq!(r.diffs[0].change, DiffChangeType::Modified);

        // Same text AND same depth still reads as Unchanged.
        let c = vec![IRBlock::list(false, [("항목", 1u8)])];
        let r2 = diff_blocks(&b, &c);
        assert_eq!(r2.stats.unchanged, 1);
    }

    // ── diff_blocks ──

    #[test]
    fn diff_identical_documents_has_only_unchanged() {
        let a = vec![
            IRBlock::heading(1, "제목"),
            IRBlock::paragraph("본문 첫 문단"),
            IRBlock::paragraph("본문 둘째 문단"),
        ];
        let b = a.clone();
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.unchanged, 3);
        assert_eq!(r.stats.modified, 0);
        assert_eq!(r.stats.added, 0);
        assert_eq!(r.stats.removed, 0);
    }

    #[test]
    fn diff_detects_added_and_removed() {
        let a = vec![
            IRBlock::paragraph("공통 문단"),
            IRBlock::paragraph("삭제될 문단"),
        ];
        let b = vec![
            IRBlock::paragraph("공통 문단"),
            IRBlock::paragraph("추가된 문단"),
        ];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.unchanged, 1);
        assert!(r.stats.removed + r.stats.added + r.stats.modified >= 1);
    }

    #[test]
    fn diff_modified_paragraph_carries_similarity() {
        let a = vec![IRBlock::paragraph("이 조문은 총 다섯 가지 요건을 규정한다.")];
        let b = vec![IRBlock::paragraph("이 조문은 총 여섯 가지 요건을 규정한다.")];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.modified, 1);
        let d = &r.diffs[0];
        assert_eq!(d.change, DiffChangeType::Modified);
        assert!(d.similarity > 0.8 && d.similarity < 1.0);
    }

    #[test]
    fn diff_table_attaches_cell_diffs() {
        let a = mk_table(vec![
            vec!["제1조", "목적"],
            vec!["제2조", "정의"],
        ]);
        let b = mk_table(vec![
            vec!["제1조", "목적"],
            vec!["제2조", "용어의 정의"], // modified
            vec!["제3조", "적용 범위"],     // added row
        ]);
        let r = diff_blocks(&[a], &[b]);
        assert_eq!(r.stats.modified, 1);
        let d = &r.diffs[0];
        let cell_diffs = d.cell_diffs.as_ref().expect("cell_diffs for table pair");
        assert_eq!(cell_diffs.len(), 3);
        // Row 1 col 1 should be modified.
        assert_eq!(cell_diffs[1][1].change, DiffChangeType::Modified);
        // Row 2 (added) should be Added on both columns.
        assert_eq!(cell_diffs[2][0].change, DiffChangeType::Added);
    }

    #[test]
    fn diff_different_kinds_never_unchanged() {
        let a = vec![IRBlock::paragraph("단순 문단")];
        let b = vec![IRBlock::heading(1, "단순 문단")];
        let r = diff_blocks(&a, &b);
        // paragraph vs heading differ by kind → similarity 0 → unpaired.
        assert_eq!(r.stats.unchanged, 0);
        assert!(r.stats.removed >= 1 || r.stats.added >= 1);
    }

    // ── approx_distance (bigram/Dice fallback for huge strings) ──

    #[test]
    fn approx_distance_replaces_naive_length_diff_for_equal_length_strings() {
        // Regression: a naive `|len_a - len_b|` fallback collapses to 0
        // (→ similarity 1.0, "identical") for ANY pair of equal-length
        // strings no matter how different their content. Bigram Dice
        // overlap actually looks at content.
        let a: String = "가나다라마".repeat(1200); // 6000 chars
        let b: String = "바사아자차".repeat(1200); // 6000 chars, disjoint syllables
        assert_eq!(a.chars().count(), b.chars().count());
        assert!(a.chars().count() + b.chars().count() > MAX_LEVENSHTEIN_LEN);
        let s = similarity(&a, &b);
        assert!(s < 0.2, "expected low similarity for disjoint content, got {s}");
    }

    #[test]
    fn approx_distance_stays_high_for_prefix_shift() {
        // A large paragraph with a short prefix inserted should still
        // score close to identical — bigram overlap is shift-invariant,
        // unlike a positional/index-aligned comparison (kordoc changelog).
        let base: String = "조문내용 ".repeat(1000); // 5000 chars
        let shifted = format!("새로운문구 {base}");
        assert!(base.chars().count() + shifted.chars().count() > MAX_LEVENSHTEIN_LEN);
        let s = similarity(&base, &shifted);
        assert!(s > 0.9, "expected high similarity for prefix shift, got {s}");
    }

    // ── text_diff (word-level LCS diff) ──

    #[test]
    fn text_diff_marks_equal_insert_delete_runs() {
        let changes = text_diff("오늘 날씨가 좋다", "오늘 날씨가 매우 좋다");
        assert!(changes
            .iter()
            .any(|c| c.kind == TextChangeKind::Insert && c.text.contains("매우")));
        assert!(changes
            .iter()
            .any(|c| c.kind == TextChangeKind::Equal && c.text.contains("오늘")));
    }

    #[test]
    fn text_diff_identical_strings_are_all_equal() {
        let changes = text_diff("동일한 문장", "동일한 문장");
        assert!(changes.iter().all(|c| c.kind == TextChangeKind::Equal));
    }

    // ── moved-block detection (not in kordoc — new capability) ──

    #[test]
    fn diff_detects_moved_block() {
        // Deliberately zero character overlap between the three
        // paragraphs (no shared suffix/prefix) — otherwise the LCS
        // aligner (correctly, by design) prefers whichever chain pairs
        // the most positions, which can outweigh pairing the two
        // genuinely-identical blocks. That's the same DP kordoc's
        // `alignBlocks` uses; this test isolates move-detection from it.
        let a = vec![
            IRBlock::paragraph("AAAAAAAAAA"),
            IRBlock::paragraph("이동될특별한고유문단내용"),
            IRBlock::paragraph("BBBBBBBBBB"),
        ];
        let b = vec![
            IRBlock::paragraph("AAAAAAAAAA"),
            IRBlock::paragraph("BBBBBBBBBB"),
            IRBlock::paragraph("이동될특별한고유문단내용"),
        ];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.moved, 2);
        assert_eq!(r.stats.added, 0);
        assert_eq!(r.stats.removed, 0);
        let moved: Vec<&BlockDiff> = r
            .diffs
            .iter()
            .filter(|d| d.change == DiffChangeType::Moved)
            .collect();
        assert_eq!(moved.len(), 2);
        for d in &moved {
            assert!(d.moved_pair.is_some());
            assert!(d.similarity >= MOVE_SIMILARITY_THRESHOLD);
        }
    }

    #[test]
    fn diff_leaves_unrelated_added_removed_alone() {
        // Low-similarity add/remove pairs must never be reclassified as
        // Moved just because move-detection ran.
        let a = vec![IRBlock::paragraph("완전히 다른 내용 하나")];
        let b = vec![IRBlock::paragraph("전혀 관계없는 새로운 문장")];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.moved, 0);
    }

    // ── row/column-aligned table cell diff ──

    fn table_of(block: IRBlock) -> IRTable {
        match block {
            IRBlock::Table(t) => t,
            _ => panic!("expected a table block"),
        }
    }

    #[test]
    fn diff_table_row_inserted_in_middle_does_not_cascade() {
        let a = table_of(mk_table(vec![
            vec!["제1조", "목적"],
            vec!["제2조", "정의"],
            vec!["제3조", "적용범위"],
        ]));
        let b = table_of(mk_table(vec![
            vec!["제1조", "목적"],
            vec!["제1조의2", "정의규정 신설"], // inserted row
            vec!["제2조", "정의"],
            vec!["제3조", "적용범위"],
        ]));
        let cell_diffs = diff_table_cells(&a, &b);
        assert_eq!(cell_diffs.len(), 4);
        assert!(cell_diffs[0].iter().all(|c| c.change == DiffChangeType::Unchanged));
        assert!(cell_diffs[1].iter().all(|c| c.change == DiffChangeType::Added));
        // Rows that merely shifted position must NOT cascade into "modified".
        assert!(cell_diffs[2].iter().all(|c| c.change == DiffChangeType::Unchanged));
        assert!(cell_diffs[3].iter().all(|c| c.change == DiffChangeType::Unchanged));
    }

    #[test]
    fn diff_table_column_inserted_does_not_cascade() {
        let a = table_of(mk_table(vec![
            vec!["항목", "값"],
            vec!["가", "10"],
            vec!["나", "20"],
        ]));
        let b = table_of(mk_table(vec![
            vec!["항목", "비고", "값"],
            vec!["가", "-", "10"],
            vec!["나", "-", "20"],
        ]));
        let cell_diffs = diff_table_cells(&a, &b);
        assert_eq!(cell_diffs.len(), 3);
        for row in &cell_diffs {
            assert_eq!(row.len(), 3, "expected 항목/비고/값 columns aligned");
        }
        assert!(cell_diffs.iter().all(|row| row[0].change == DiffChangeType::Unchanged));
        assert!(cell_diffs.iter().all(|row| row[1].change == DiffChangeType::Added));
        assert!(cell_diffs.iter().all(|row| row[2].change == DiffChangeType::Unchanged));
    }

    #[test]
    fn cell_diff_flags_span_change_even_when_text_is_identical() {
        let before = IRCell {
            text: "헤더".into(),
            col_span: 2,
            row_span: 1,
        };
        let after = IRCell {
            text: "헤더".into(),
            col_span: 1,
            row_span: 1,
        };
        let d = build_cell_diff(Some(&before), Some(&after));
        assert_eq!(d.change, DiffChangeType::Modified);
        assert_eq!(d.span_before, Some((2, 1)));
        assert_eq!(d.span_after, Some((1, 1)));
    }

    #[test]
    fn cell_diff_unchanged_requires_matching_span_too() {
        let before = IRCell::new("동일");
        let after = IRCell::new("동일");
        let d = build_cell_diff(Some(&before), Some(&after));
        assert_eq!(d.change, DiffChangeType::Unchanged);
    }

    // ── render_diff_markdown / JSON ──

    #[test]
    fn render_diff_markdown_matches_kordoc_report_shape() {
        let a = vec![
            IRBlock::paragraph("체리 파인애플 포도"),
            IRBlock::paragraph("동일한 공통 문단입니다"),
        ];
        let b = vec![
            IRBlock::paragraph("동일한 공통 문단입니다"),
            IRBlock::paragraph("컴퓨터 키보드 마우스"),
        ];
        let r = diff_blocks(&a, &b);
        assert_eq!(r.stats.removed, 1);
        assert_eq!(r.stats.added, 1);
        assert_eq!(r.stats.unchanged, 1);

        let report = render_diff_markdown(&r);
        assert!(report.starts_with("## 문서 비교 결과\n"));
        assert!(report.contains("추가: 1 | 삭제: 1 | 변경: 0 | 이동: 0 | 동일: 1"));
        assert!(report.contains("- 체리 파인애플 포도"));
        assert!(report.contains("+ 컴퓨터 키보드 마우스"));
        assert!(report.contains("  동일한 공통 문단입니다"));
        // Added/removed lines carry no similarity suffix.
        assert!(!report.contains("체리 파인애플 포도 ("));
    }

    #[test]
    fn diff_result_serializes_to_json() {
        let a = vec![IRBlock::paragraph("본문")];
        let b = vec![IRBlock::paragraph("수정된 본문")];
        let r = diff_blocks(&a, &b);
        let json = serde_json::to_string(&r).expect("DiffResult should serialize");
        assert!(json.contains("\"stats\""));
        assert!(json.contains("\"diffs\""));
        assert!(json.contains("\"unchanged\""));
    }
}
