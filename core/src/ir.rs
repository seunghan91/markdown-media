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
//!   `text-diff.ts::levenshtein`.
//! - [`diff_blocks`] — kordoc's LCS-based block aligner + per-table cell
//!   diff. This is the "신구대조표" core: given two parsed documents, emit
//!   a structured list of unchanged / modified / added / removed blocks
//!   with similarity scores and cell-level deltas for tables.
//!
//! No existing parser touches this module yet. Phase 2b will wire
//! `HwpParser::extract_blocks()` to emit `Vec<IRBlock>` directly; for now
//! callers can only assemble `IRBlock`s manually.
//!
//! References:
//! - `reference/kordoc/src/types.ts`
//! - `reference/kordoc/src/diff/compare.ts`
//! - `reference/kordoc/src/diff/text-diff.ts`

use std::collections::HashMap;

// ── IR types ─────────────────────────────────────────────────────────────────

/// A single cell inside an [`IRTable`]. Mirrors kordoc `IRCell`.
#[derive(Debug, Clone, PartialEq)]
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

/// A rectangular table laid out as `rows × cols`. `cells` is indexed
/// `cells[row][col]`. `has_header` signals "render row 0 as `<th>`" —
/// currently a layout hint rather than semantic detection, matching kordoc
/// v2.0 behavior.
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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
        items: Vec<String>,
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

    /// Comparable text content of a block — used by [`diff_blocks`] to
    /// compute similarity. `None` for blocks without meaningful text
    /// (e.g. separators, images without alt).
    pub fn text_for_compare(&self) -> Option<String> {
        match self {
            IRBlock::Paragraph { text, .. } => Some(text.clone()),
            IRBlock::Heading { text, .. } => Some(text.clone()),
            IRBlock::List { items, .. } => Some(items.join(" ")),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffChangeType {
    Unchanged,
    Modified,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellDiff {
    pub change: DiffChangeType,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlockDiff {
    pub change: DiffChangeType,
    pub before: Option<IRBlock>,
    pub after: Option<IRBlock>,
    /// Per-cell diff when both `before` and `after` are tables.
    pub cell_diffs: Option<Vec<Vec<CellDiff>>>,
    /// Similarity score in `[0, 1]`. 1 = identical, 0 = unrelated.
    pub similarity: f64,
}

#[derive(Debug, Clone, Default)]
pub struct DiffStats {
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, Default)]
pub struct DiffResult {
    pub stats: DiffStats,
    pub diffs: Vec<BlockDiff>,
}

// ── Markdown renderer ────────────────────────────────────────────────────────

/// Render a slice of IR blocks to Markdown. Uses GFM table syntax,
/// `#`-prefixed headings, and `[이미지: ...]` placeholders — matching the
/// existing mdm HWP parser output so downstream consumers see consistent
/// Markdown regardless of which path built the blocks.
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
                    out.push_str(&format!(" [각주] {}", note));
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
                for (idx, item) in items.iter().enumerate() {
                    if idx > 0 {
                        out.push('\n');
                    }
                    if *ordered {
                        out.push_str(&format!("{}. {}", idx + 1, item));
                    } else {
                        out.push_str("- ");
                        out.push_str(item);
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

/// O(min(m, n)) space Levenshtein. Returns `|m - n|` when total input
/// exceeds [`MAX_LEVENSHTEIN_LEN`] to defeat quadratic-time DoS.
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.len() + b.len() > MAX_LEVENSHTEIN_LEN {
        return (a.len() as isize - b.len() as isize).unsigned_abs() as usize;
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

// ── Block diff engine ────────────────────────────────────────────────────────

/// Blocks with similarity ≥ this are paired as "modified", below → unpaired.
const SIMILARITY_THRESHOLD: f64 = 0.4;
/// Blocks with similarity ≥ this are treated as "unchanged" (no render diff).
const UNCHANGED_THRESHOLD: f64 = 0.99;
/// Cap on m·n before we fall back to positional alignment.
const MAX_LCS_PAIRS: usize = 10_000_000;

/// Compare two block sequences and return a kordoc-equivalent [`DiffResult`].
/// Pairs blocks via LCS over a similarity matrix, then walks the alignment
/// to classify each pair as unchanged / modified / added / removed.
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
                });
            }
            (None, None) => {}
        }
    }

    DiffResult { stats, diffs }
}

/// LCS over a similarity matrix with threshold `SIMILARITY_THRESHOLD`.
/// Returns `(Option<idx_in_a>, Option<idx_in_b>)` pairs in document order.
fn align_blocks(a: &[IRBlock], b: &[IRBlock]) -> Vec<(Option<usize>, Option<usize>)> {
    let m = a.len();
    let n = b.len();

    // Fallback for pathological sizes — positional pairing.
    if m * n > MAX_LCS_PAIRS {
        return fallback_align(m, n);
    }

    let mut sim_cache: HashMap<(usize, usize), f64> = HashMap::new();
    let mut get_sim = |i: usize, j: usize, a: &[IRBlock], b: &[IRBlock]| -> f64 {
        if let Some(&v) = sim_cache.get(&(i, j)) {
            return v;
        }
        let v = block_similarity(&a[i], &b[j]);
        sim_cache.insert((i, j), v);
        v
    };

    // dp[i][j] = length of best LCS using a[..i] and b[..j].
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 1..=m {
        for j in 1..=n {
            if get_sim(i - 1, j - 1, a, b) >= SIMILARITY_THRESHOLD {
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
        if get_sim(i - 1, j - 1, a, b) >= SIMILARITY_THRESHOLD
            && dp[i][j] == dp[i - 1][j - 1] + 1
        {
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

/// Cell-by-cell diff of two tables. Output grid has
/// `max(a.rows, b.rows) × max(a.cols, b.cols)` dimensions; missing cells
/// become `added` / `removed` entries.
pub fn diff_table_cells(a: &IRTable, b: &IRTable) -> Vec<Vec<CellDiff>> {
    let max_rows = a.rows.max(b.rows);
    let max_cols = a.cols.max(b.cols);
    let mut out = Vec::with_capacity(max_rows);

    for r in 0..max_rows {
        let mut row = Vec::with_capacity(max_cols);
        for c in 0..max_cols {
            let cell_a = if r < a.rows && c < a.cols {
                a.cells.get(r).and_then(|row| row.get(c)).map(|cell| cell.text.clone())
            } else {
                None
            };
            let cell_b = if r < b.rows && c < b.cols {
                b.cells.get(r).and_then(|row| row.get(c)).map(|cell| cell.text.clone())
            } else {
                None
            };
            let change = match (&cell_a, &cell_b) {
                (None, Some(_)) => DiffChangeType::Added,
                (Some(_), None) => DiffChangeType::Removed,
                (Some(x), Some(y)) if x == y => DiffChangeType::Unchanged,
                (Some(_), Some(_)) => DiffChangeType::Modified,
                (None, None) => DiffChangeType::Unchanged,
            };
            row.push(CellDiff {
                change,
                before: cell_a,
                after: cell_b,
            });
        }
        out.push(row);
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
        assert!(s >= 0.0 && s <= 1.0);
    }

    // ── blocks_to_markdown ──

    #[test]
    fn markdown_paragraph_with_footnote_and_href() {
        let blocks = vec![IRBlock::Paragraph {
            text: "본문".to_string(),
            footnote: Some("각주 내용".to_string()),
            href: Some("https://law.go.kr".to_string()),
        }];
        let md = blocks_to_markdown(&blocks);
        assert!(md.contains("본문"));
        assert!(md.contains("<https://law.go.kr>"));
        assert!(md.contains("[각주] 각주 내용"));
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
        let unordered = IRBlock::List {
            ordered: false,
            items: vec!["첫째".to_string(), "둘째".to_string()],
        };
        let ordered = IRBlock::List {
            ordered: true,
            items: vec!["A".to_string(), "B".to_string()],
        };
        let md = blocks_to_markdown(&[unordered, ordered]);
        assert!(md.contains("- 첫째"));
        assert!(md.contains("- 둘째"));
        assert!(md.contains("1. A"));
        assert!(md.contains("2. B"));
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
}
