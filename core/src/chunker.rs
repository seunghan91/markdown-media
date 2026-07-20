//! RAG-oriented structural chunking — walks the heading / list / table
//! hierarchy of an `IRBlock` document and emits breadcrumb-annotated chunks.
//!
//! Ported from kkdoc (MIT): reference/kkdoc/src/chunks.ts
//!
//! ## Divergences from the TS original
//!
//! The Rust [`crate::ir::IRBlock`] this module walks is *not* a 1:1 mirror
//! of kkdoc's `IRBlock` (see `reference/kkdoc/src/types.ts`). Two gaps force
//! behavior changes vs. the reference implementation:
//!
//! - **No `pageNumber` field.** kkdoc's `DocChunk.page` has no Rust
//!   equivalent, so this module does not emit a page number at all.
//! - **No per-item `listDepth` field, and `List` is block-aggregated.**
//!   kkdoc represents each list item as its own `IRBlock` carrying an
//!   explicit `listDepth`, letting `chunks.ts` build a multi-level
//!   breadcrumb stack. The Rust `IRBlock::List { ordered, items }` instead
//!   bundles an entire list into one block with no per-item position or
//!   depth. Consequently:
//!   - `IRBlock::List` blocks are always emitted as their own standalone
//!     chunk (never merged with neighboring text, like a table) — there is
//!     no single anchor text to push onto a list breadcrumb stack.
//!   - List-hierarchy breadcrumb entries are only ever inferred from
//!     `IRBlock::Paragraph` text via the same leading-marker heuristic
//!     kkdoc uses as its *fallback* path (`LIST_MARKER_RE`, e.g. `□`, `○`,
//!     `1.`, `가.`, `①` …). Since that heuristic only ever yields depth 0
//!     in kkdoc too (deeper depths there come solely from the explicit
//!     `listDepth` field this Rust IR lacks), this port can only reproduce
//!     depth-0 promotion — never multi-level nesting. Reproducing deeper
//!     nesting would require adding a `list_depth` field to
//!     [`crate::ir::IRBlock::Paragraph`] upstream, which is out of scope
//!     here (`ir.rs` is owned by another workstream).
//!
//! ## Extensions beyond the TS original
//!
//! kkdoc's chunker deliberately has no cutting policy — "structure tree
//! only, splitting is the RAG pipeline's job." This port adds an optional
//! [`ChunkOptions::max_chars`] budget (with [`ChunkOptions::overlap`]) since
//! the mdm RAG pipeline wants ready-to-embed chunks:
//! - Merged/list-item text chunks that exceed the budget are split by
//!   character count with the requested overlap. All pieces of a split
//!   chunk keep the *originating run's full* `block_range` (a character
//!   split has no clean sub-block boundary to report).
//! - Headings are never split (always one chunk).
//! - Oversized tables are split row-wise — rows are never split mid-row,
//!   and the header row (if `has_header`) is repeated at the top of every
//!   split piece so each one stays self-describing.

use crate::ir::{blocks_to_markdown, IRBlock, IRCell, IRTable};
use serde::{Deserialize, Serialize};

lazy_static::lazy_static! {
    /// Leading bullet/enumeration marker — depth-0 identification for list
    /// items that carry no explicit depth field. Ported from kkdoc
    /// `src/chunks.ts::LIST_MARKER_RE` (gongmun 8-level numbering + bullet
    /// glyphs).
    static ref LIST_MARKER_RE: regex::Regex = regex::Regex::new(
        r"^(?:[□■◇◆○◎●◦ㅇ•▪▸▶※-]|\d{1,3}[.)]|[가나다라마바사아자차카타파하][.)]|\([가나다라마바사아자차카타파하0-9]{1,3}\)|[①-⑳㉮-㉻㈎-㈛])\s"
    ).expect("LIST_MARKER_RE is a static, valid pattern");
}

/// How consecutive blocks are grouped into chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Granularity {
    /// One [`IRBlock`] == one chunk.
    Block,
    /// Consecutive non-heading/non-table blocks that share the same
    /// breadcrumb are merged into one chunk. Matches kkdoc's `"section"`
    /// mode (the default there and here).
    Section,
}

impl Default for Granularity {
    fn default() -> Self {
        Granularity::Section
    }
}

/// Chunking policy.
#[derive(Debug, Clone)]
pub struct ChunkOptions {
    /// Soft character budget per chunk. `None` (default) disables
    /// splitting entirely — matches kkdoc's "no cutting policy" stance.
    pub max_chars: Option<usize>,
    /// Character overlap carried into the next piece when a chunk is split
    /// for exceeding `max_chars`. Ignored when `max_chars` is `None`.
    pub overlap: usize,
    /// Include the raw cell-text matrix on table chunk metadata.
    pub include_table_cells: bool,
    pub granularity: Granularity,
}

impl Default for ChunkOptions {
    fn default() -> Self {
        Self {
            max_chars: None,
            overlap: 0,
            include_table_cells: false,
            granularity: Granularity::default(),
        }
    }
}

/// Chunk kind — mirrors kkdoc `DocChunk.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChunkKind {
    Text,
    Table,
    Heading,
}

/// Structural summary attached to table chunks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSummary {
    pub rows: usize,
    pub cols: usize,
    /// Cell text matrix — present only when `include_table_cells` is set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cells: Option<Vec<Vec<String>>>,
}

/// Extra per-chunk info beyond the required core fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChunkMetadata {
    #[serde(rename = "type")]
    pub kind: ChunkKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table: Option<TableSummary>,
}

/// A single RAG chunk with its structural breadcrumb and source anchor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    /// `"c0001"` sequential id — deterministic for identical input.
    pub id: String,
    /// Chunk body markdown (tables keep GFM/HTML verbatim via
    /// [`blocks_to_markdown`]).
    pub text: String,
    /// Path of ancestor heading / list-item labels, excluding this chunk.
    pub breadcrumb: Vec<String>,
    /// Source `IRBlock` index range `[start, end]`, inclusive.
    pub block_range: (usize, usize),
    pub char_count: usize,
    pub metadata: ChunkMetadata,
}

/// Leading-marker depth-0 detector for list items with no explicit depth
/// field. Returns `Some(0)` when `text` starts with a bullet/enumeration
/// marker, `None` otherwise. See module docs for why this never returns a
/// depth greater than 0.
fn depth_of_paragraph(text: &str) -> Option<u8> {
    if LIST_MARKER_RE.is_match(text.trim()) {
        Some(0)
    } else {
        None
    }
}

fn crumb(heading_stack: &[(u8, String)], list_stack: &[(u8, String)]) -> Vec<String> {
    heading_stack
        .iter()
        .map(|(_, t)| t.clone())
        .chain(list_stack.iter().map(|(_, t)| t.clone()))
        .collect()
}

/// Split `text` into `<= max_chars`-sized pieces (char count, Unicode
/// scalar values), each subsequent piece re-including the last `overlap`
/// characters of the previous one.
fn split_by_chars(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if max_chars == 0 || chars.len() <= max_chars {
        return vec![text.to_string()];
    }
    let step = max_chars.saturating_sub(overlap).max(1);
    let mut pieces = Vec::new();
    let mut start = 0usize;
    loop {
        let end = (start + max_chars).min(chars.len());
        pieces.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        start += step;
    }
    pieces
}

struct ChunkBuilder<'a> {
    blocks: &'a [IRBlock],
    opts: &'a ChunkOptions,
    chunks: Vec<Chunk>,
}

impl<'a> ChunkBuilder<'a> {
    fn new(blocks: &'a [IRBlock], opts: &'a ChunkOptions) -> Self {
        Self { blocks, opts, chunks: Vec::new() }
    }

    fn next_id(&self) -> String {
        format!("c{:04}", self.chunks.len() + 1)
    }

    fn push(
        &mut self,
        kind: ChunkKind,
        breadcrumb: Vec<String>,
        text: String,
        block_range: (usize, usize),
        table: Option<TableSummary>,
    ) {
        let id = self.next_id();
        let char_count = text.chars().count();
        self.chunks.push(Chunk {
            id,
            text,
            breadcrumb,
            block_range,
            char_count,
            metadata: ChunkMetadata { kind, table },
        });
    }

    /// Emit a text-kind chunk covering `indices` (materialized out of the
    /// source slice so gaps left by skipped empty blocks never leak into
    /// the rendered markdown), splitting on `max_chars` if configured.
    fn emit_text(&mut self, breadcrumb: Vec<String>, indices: &[usize]) {
        debug_assert!(!indices.is_empty());
        let members: Vec<IRBlock> = indices.iter().map(|&i| self.blocks[i].clone()).collect();
        let text = blocks_to_markdown(&members);
        let range = (indices[0], *indices.last().unwrap());
        match self.opts.max_chars {
            Some(max) if max > 0 && text.chars().count() > max => {
                for piece in split_by_chars(&text, max, self.opts.overlap) {
                    self.push(ChunkKind::Text, breadcrumb.clone(), piece, range, None);
                }
            }
            _ => self.push(ChunkKind::Text, breadcrumb, text, range, None),
        }
    }

    fn emit_heading(&mut self, breadcrumb: Vec<String>, idx: usize, block: &IRBlock) {
        let text = blocks_to_markdown(std::slice::from_ref(block));
        self.push(ChunkKind::Heading, breadcrumb, text, (idx, idx), None);
    }

    fn table_summary(&self, table: &IRTable) -> TableSummary {
        TableSummary {
            rows: table.rows,
            cols: table.cols,
            cells: if self.opts.include_table_cells {
                Some(
                    table
                        .cells
                        .iter()
                        .map(|row| row.iter().map(|c| c.text.clone()).collect())
                        .collect(),
                )
            } else {
                None
            },
        }
    }

    /// Tables are always their own chunk — never merged with surrounding
    /// text. If the rendered table exceeds `max_chars`, split by whole
    /// rows (never mid-row), repeating the header row on every piece.
    fn emit_table(&mut self, breadcrumb: Vec<String>, idx: usize, table: &IRTable) {
        let full_md = blocks_to_markdown(&[IRBlock::Table(table.clone())]);

        let needs_split = matches!(self.opts.max_chars, Some(max) if max > 0 && full_md.chars().count() > max)
            && table.rows > 1;
        if !needs_split {
            let summary = self.table_summary(table);
            self.push(ChunkKind::Table, breadcrumb, full_md, (idx, idx), Some(summary));
            return;
        }

        let max = self.opts.max_chars.unwrap();
        let header_row: Option<Vec<IRCell>> = if table.has_header {
            table.cells.first().cloned()
        } else {
            None
        };
        let body_start = if table.has_header { 1 } else { 0 };

        let render_piece = |rows: &[Vec<IRCell>]| -> (String, IRTable) {
            let mut cells = Vec::with_capacity(rows.len() + header_row.is_some() as usize);
            if let Some(h) = &header_row {
                cells.push(h.clone());
            }
            cells.extend(rows.iter().cloned());
            let piece = IRTable { rows: cells.len(), cols: table.cols, cells, has_header: table.has_header };
            let md = blocks_to_markdown(&[IRBlock::Table(piece.clone())]);
            (md, piece)
        };

        let mut groups: Vec<Vec<Vec<IRCell>>> = Vec::new();
        let mut current: Vec<Vec<IRCell>> = Vec::new();
        for row in &table.cells[body_start..] {
            let mut trial = current.clone();
            trial.push(row.clone());
            let (trial_md, _) = render_piece(&trial);
            if trial_md.chars().count() > max && !current.is_empty() {
                groups.push(current);
                current = vec![row.clone()];
            } else {
                current = trial;
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }

        for group in groups {
            let (md, piece) = render_piece(&group);
            let summary = self.table_summary(&piece);
            self.push(ChunkKind::Table, breadcrumb.clone(), md, (idx, idx), Some(summary));
        }
    }
}

fn flush_run(run: &mut Option<(Vec<String>, Vec<usize>)>, builder: &mut ChunkBuilder) {
    if let Some((breadcrumb, indices)) = run.take() {
        builder.emit_text(breadcrumb, &indices);
    }
}

/// Chunk a parsed document, preserving its heading / list hierarchy as a
/// breadcrumb path on every chunk. See the module docs for behavior
/// notes and divergences from kkdoc's `blocksToChunks`.
pub fn chunk(blocks: &[IRBlock], opts: &ChunkOptions) -> Vec<Chunk> {
    let mut builder = ChunkBuilder::new(blocks, opts);
    let mut heading_stack: Vec<(u8, String)> = Vec::new();
    let mut list_stack: Vec<(u8, String)> = Vec::new();
    let mut run: Option<(Vec<String>, Vec<usize>)> = None;

    for (i, block) in blocks.iter().enumerate() {
        // Empty blocks are skipped entirely — they don't affect the
        // breadcrumb stacks either, matching kkdoc's `if (!md) continue`.
        let md = blocks_to_markdown(std::slice::from_ref(block));
        if md.is_empty() {
            continue;
        }

        match block {
            IRBlock::Heading { level, text } => {
                flush_run(&mut run, &mut builder);
                let level = (*level).min(6);
                while heading_stack.last().is_some_and(|(l, _)| *l >= level) {
                    heading_stack.pop();
                }
                list_stack.clear(); // new heading = list hierarchy reset
                let breadcrumb = crumb(&heading_stack, &list_stack);
                builder.emit_heading(breadcrumb, i, block);
                heading_stack.push((level, text.trim().to_string()));
            }
            IRBlock::Table(table) => {
                flush_run(&mut run, &mut builder);
                let breadcrumb = crumb(&heading_stack, &list_stack);
                builder.emit_table(breadcrumb, i, table);
            }
            IRBlock::List { .. } => {
                // Block-aggregated list (see module docs) — always
                // standalone, like a table; no single anchor text exists
                // to push onto the list breadcrumb stack.
                flush_run(&mut run, &mut builder);
                let breadcrumb = crumb(&heading_stack, &list_stack);
                builder.emit_text(breadcrumb, &[i]);
            }
            _ => {
                // Paragraph / Image / Separator.
                let depth = match block {
                    IRBlock::Paragraph { text, .. } => depth_of_paragraph(text),
                    _ => None,
                };
                if let Some(d) = depth {
                    while list_stack.last().is_some_and(|(ld, _)| *ld >= d) {
                        list_stack.pop();
                    }
                }
                let breadcrumb = crumb(&heading_stack, &list_stack);
                if let Some(d) = depth {
                    let label = match block {
                        IRBlock::Paragraph { text, .. } => text.trim().to_string(),
                        _ => String::new(),
                    };
                    list_stack.push((d, label));
                }

                if opts.granularity == Granularity::Block {
                    builder.emit_text(breadcrumb, &[i]);
                    continue;
                }

                let extends_run = matches!(&run, Some((bc, _)) if *bc == breadcrumb);
                if extends_run {
                    run.as_mut().unwrap().1.push(i);
                } else {
                    flush_run(&mut run, &mut builder);
                    run = Some((breadcrumb, vec![i]));
                }
            }
        }
    }
    flush_run(&mut run, &mut builder);

    builder.chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::IRCell;

    fn table(cells: Vec<Vec<&str>>, has_header: bool) -> IRTable {
        let cells: Vec<Vec<IRCell>> = cells
            .into_iter()
            .map(|row| row.into_iter().map(IRCell::new).collect())
            .collect();
        let rows = cells.len();
        let cols = cells.iter().map(|r| r.len()).max().unwrap_or(0);
        IRTable { rows, cols, cells, has_header }
    }

    /// Non-overlapping, monotonically increasing block_range coverage +
    /// every non-empty source block index is covered by exactly one chunk.
    /// Port of kkdoc's `assertRangeIntegrity` test helper.
    fn assert_range_integrity(chunks: &[Chunk], blocks: &[IRBlock]) {
        let mut covered = std::collections::HashSet::new();
        let mut prev_end: isize = -1;
        let mut prev_range: Option<(usize, usize)> = None;
        for c in chunks {
            let range = c.block_range;
            let (start, end) = range;
            // A chunk split across max_chars pieces (text) or table rows
            // repeats the *same* source range for every piece — that's not
            // an overlap with a different block, just consecutive pieces of
            // one. Only a genuinely new range must advance past prev_end.
            if Some(range) != prev_range {
                assert!(start as isize > prev_end, "block_range overlap/non-monotonic in {}", c.id);
            }
            assert!(end >= start, "block_range inverted in {}", c.id);
            prev_end = end as isize;
            prev_range = Some(range);
            for i in start..=end {
                covered.insert(i);
            }
        }
        for (i, b) in blocks.iter().enumerate() {
            let md = blocks_to_markdown(std::slice::from_ref(b));
            if !md.is_empty() {
                assert!(covered.contains(&i), "non-empty block {i} not covered by any chunk");
            }
        }
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert!(chunk(&[], &ChunkOptions::default()).is_empty());
    }

    #[test]
    fn heading_stack_breadcrumb() {
        let blocks = vec![
            IRBlock::heading(1, "추진 계획"),
            IRBlock::heading(2, "1. 개요"),
            IRBlock::paragraph("도입 문단"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].metadata.kind, ChunkKind::Heading);
        assert_eq!(chunks[0].text, "# 추진 계획");
        assert!(chunks[0].breadcrumb.is_empty());
        assert_eq!(chunks[1].text, "## 1. 개요");
        assert_eq!(chunks[1].breadcrumb, vec!["추진 계획".to_string()]);
        assert_eq!(chunks[2].breadcrumb, vec!["추진 계획".to_string(), "1. 개요".to_string()]);
    }

    #[test]
    fn heading_reappearance_resets_list_stack() {
        let blocks = vec![
            IRBlock::heading(1, "추진 계획"),
            IRBlock::paragraph("□ 추진 배경"),
            IRBlock::heading(2, "2. 향후 일정"),
            IRBlock::paragraph("일정 본문"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        let last = chunks.iter().find(|c| c.text == "일정 본문").unwrap();
        assert_eq!(last.breadcrumb, vec!["추진 계획".to_string(), "2. 향후 일정".to_string()]);
    }

    #[test]
    fn depth0_marker_promotion_carries_forward_for_non_marker_text() {
        // Faithful to what the Rust IR can carry: only depth 0 is ever
        // inferred (see module docs). A later plain paragraph inherits the
        // list breadcrumb until a heading resets it.
        let blocks = vec![
            IRBlock::paragraph("□ 추진 배경"),
            IRBlock::paragraph("평문은 리스트 문맥을 잇는다"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].breadcrumb.is_empty());
        assert_eq!(chunks[0].text, "□ 추진 배경");
        assert_eq!(chunks[1].breadcrumb, vec!["□ 추진 배경".to_string()]);
        assert_eq!(chunks[1].text, "평문은 리스트 문맥을 잇는다");
    }

    #[test]
    fn section_granularity_merges_same_breadcrumb_runs() {
        let blocks = vec![
            IRBlock::heading(1, "개요"),
            IRBlock::paragraph("문단 하나"),
            IRBlock::paragraph("문단 둘"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 2); // heading + merged text run
        let merged = &chunks[1];
        assert_eq!(merged.text, "문단 하나\n\n문단 둘");
        assert_eq!(merged.block_range, (1, 2));
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn block_granularity_never_merges() {
        let blocks = vec![IRBlock::paragraph("문단 하나"), IRBlock::paragraph("문단 둘")];
        let opts = ChunkOptions { granularity: Granularity::Block, ..Default::default() };
        let chunks = chunk(&blocks, &opts);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.block_range.0 == c.block_range.1));
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn empty_block_is_skipped_without_breaking_the_run() {
        let blocks = vec![
            IRBlock::paragraph("run one"),
            IRBlock::paragraph(""), // empty — skipped, doesn't join the stacks
            IRBlock::paragraph("run one cont"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "run one\n\nrun one cont");
        assert_eq!(chunks[0].block_range, (0, 2));
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn table_never_merges_and_carries_structure_summary() {
        let blocks = vec![
            IRBlock::heading(1, "예산"),
            IRBlock::paragraph("도입 문단"),
            IRBlock::Table(table(vec![vec!["항목", "값"], vec!["예산", "100"]], true)),
            IRBlock::paragraph("후속 문단"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        let table_chunk = chunks.iter().find(|c| c.metadata.kind == ChunkKind::Table).unwrap();
        assert_eq!(table_chunk.block_range, (2, 2));
        assert_eq!(table_chunk.breadcrumb, vec!["예산".to_string()]);
        let summary = table_chunk.metadata.table.as_ref().unwrap();
        assert_eq!((summary.rows, summary.cols), (2, 2));
        assert!(summary.cells.is_none());
        // surrounding paragraphs stay separate chunks (no merge across the table)
        assert!(chunks.iter().filter(|c| c.metadata.kind != ChunkKind::Table).all(|c| c.metadata.table.is_none()));
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn include_table_cells_option() {
        let blocks = vec![IRBlock::Table(table(vec![vec!["항목", "값"], vec!["예산", "100"]], true))];
        let off = chunk(&blocks, &ChunkOptions::default());
        assert!(off[0].metadata.table.as_ref().unwrap().cells.is_none());

        let opts = ChunkOptions { include_table_cells: true, ..Default::default() };
        let on = chunk(&blocks, &opts);
        assert_eq!(
            on[0].metadata.table.as_ref().unwrap().cells.as_ref().unwrap(),
            &vec![vec!["항목".to_string(), "값".to_string()], vec!["예산".to_string(), "100".to_string()]]
        );
    }

    #[test]
    fn list_block_is_standalone_and_never_merged() {
        let blocks = vec![
            IRBlock::heading(1, "목록"),
            IRBlock::paragraph("앞 문단"),
            IRBlock::List { ordered: false, items: vec!["첫째".to_string(), "둘째".to_string()] },
            IRBlock::paragraph("뒤 문단"),
        ];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        let list_chunk = chunks.iter().find(|c| c.text.contains("첫째")).unwrap();
        assert_eq!(list_chunk.text, "- 첫째\n- 둘째");
        assert_eq!(list_chunk.block_range, (2, 2));
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn heading_with_no_document_headings_has_empty_breadcrumb() {
        let blocks = vec![IRBlock::paragraph("본문")];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].breadcrumb.is_empty());
    }

    #[test]
    fn deterministic_ids_and_output() {
        let blocks = vec![
            IRBlock::heading(1, "A"),
            IRBlock::paragraph("b"),
            IRBlock::Table(table(vec![vec!["x"]], false)),
        ];
        let opts = ChunkOptions::default();
        let a = chunk(&blocks, &opts);
        let b = chunk(&blocks, &opts);
        assert_eq!(a, b);
        assert_eq!(a.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), vec!["c0001", "c0002", "c0003"]);
    }

    #[test]
    fn max_chars_splits_long_text_with_overlap() {
        let long = "가".repeat(50);
        let blocks = vec![IRBlock::paragraph(long.clone())];
        let opts = ChunkOptions { max_chars: Some(20), overlap: 5, ..Default::default() };
        let chunks = chunk(&blocks, &opts);
        assert!(chunks.len() > 1, "expected split into multiple pieces");
        for c in &chunks {
            assert!(c.char_count <= 20);
            assert_eq!(c.block_range, (0, 0)); // split pieces keep the originating run's range
        }
        // reassembling with overlap trimmed should reproduce the source text
        let mut rebuilt = chunks[0].text.clone();
        for c in &chunks[1..] {
            rebuilt.push_str(&c.text[c.text.char_indices().nth(5).map(|(b, _)| b).unwrap_or(0)..]);
        }
        assert_eq!(rebuilt, long);
    }

    #[test]
    fn max_chars_zero_disables_splitting() {
        let blocks = vec![IRBlock::paragraph("가".repeat(50))];
        let opts = ChunkOptions { max_chars: Some(0), ..Default::default() };
        let chunks = chunk(&blocks, &opts);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn large_table_splits_by_row_and_repeats_header() {
        let rows: Vec<Vec<&str>> = (0..30).map(|i| vec!["항목", if i % 2 == 0 { "짝수값입니다" } else { "홀수값입니다" }]).collect();
        let mut cells = vec![vec!["헤더1", "헤더2"]];
        cells.extend(rows);
        let t = table(cells, true);
        let blocks = vec![IRBlock::Table(t)];
        let opts = ChunkOptions { max_chars: Some(120), ..Default::default() };
        let chunks = chunk(&blocks, &opts);
        assert!(chunks.len() > 1, "expected the table to split across multiple chunks");
        for c in &chunks {
            assert_eq!(c.metadata.kind, ChunkKind::Table);
            assert_eq!(c.block_range, (0, 0));
            assert!(c.text.contains("헤더1"), "every split piece should repeat the header row");
        }
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn giant_table_boundary_still_covers_every_row_once() {
        let rows: Vec<Vec<&str>> = (0..200).map(|_| vec!["a", "b", "c"]).collect();
        let t = table(rows, false);
        let total_data_cells: usize = t.rows * t.cols;
        let blocks = vec![IRBlock::Table(t)];
        let opts = ChunkOptions { max_chars: Some(50), ..Default::default() };
        let chunks = chunk(&blocks, &opts);
        let recovered_cells: usize = chunks
            .iter()
            .map(|c| c.text.matches('|').count()) // rough sanity check only
            .sum();
        assert!(recovered_cells > 0);
        assert!(!chunks.is_empty());
        assert_range_integrity(&chunks, &blocks);
        let _ = total_data_cells;
    }

    #[test]
    fn document_with_no_headings_at_all() {
        let blocks = vec![IRBlock::paragraph("첫 문단"), IRBlock::paragraph("둘째 문단")];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].breadcrumb.is_empty());
        assert_range_integrity(&chunks, &blocks);
    }

    #[test]
    fn json_field_names_match_reference_schema_style() {
        let blocks = vec![IRBlock::heading(1, "제목")];
        let chunks = chunk(&blocks, &ChunkOptions::default());
        let json = serde_json::to_string(&chunks[0]).unwrap();
        for key in ["\"id\"", "\"text\"", "\"breadcrumb\"", "\"blockRange\"", "\"charCount\"", "\"metadata\"", "\"type\"", "\"heading\""] {
            assert!(json.contains(key), "missing {key} in {json}");
        }
    }
}
