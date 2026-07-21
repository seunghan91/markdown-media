// Ported from kkdoc (MIT): src/pdf/line-types.ts, src/pdf/line-extract.ts,
// src/pdf/table-grid.ts, src/pdf/cell-extract.ts, src/pdf/undersegmented.ts
//
//! Ruling-line ("line-based") PDF table detection plus under-segmentation
//! reconstruction, and a dual-strategy merge with the existing text-cluster
//! detector (`detect_tables_from_positions` in `parser.rs`).
//!
//! The line-based path recovers the grid geometry of tables that draw explicit
//! horizontal/vertical rules — the dominant style in Korean government PDFs —
//! by walking the page content stream for path-construction operators, snapping
//! segments to axis-aligned rules, computing grid vertices, and slicing the
//! grid into (possibly merged) cells. It shares the exact CTM convention used
//! by `PdfParser::extract_positioned_text`, so ruling lines and positioned text
//! land in the same page-space coordinate system and text can be assigned to
//! cells by containment.
//!
//! Numeric constants are ported verbatim from kkdoc; see the constant table in
//! each section. Deferred kkdoc refinements (closeOpenTableEdges,
//! splitStackedGroup, mergeAdjacentGrids, the full cluster-detector rewrite)
//! are documented at the bottom of this file.

use super::parser::{PdfTable, PositionedText};
use crate::ir::{IRCell, IRTable};
use lopdf::content::Content;
use lopdf::Object;
use std::collections::HashMap;

// ── kkdoc line-extract constants ──────────────────────────────────────────────
const ORIENTATION_TOL: f64 = 2.0; // dy<=2 → horizontal, dx<=2 → vertical
const MIN_LINE_LENGTH: f64 = 15.0; // discard shorter segments
const MAX_LINE_WIDTH: f64 = 5.0; // thick-line filter
const MERGE_TOL: f64 = 3.0; // parallel-line merge perpendicular tolerance
const STACK_GAP: f64 = 2.0; // shading-stack consecutive-gap threshold
const STACK_MIN_LINES: usize = 6; // min run length to count as a shading stack

// ── kkdoc table-grid constants ────────────────────────────────────────────────
const VERTEX_MERGE_FACTOR: f64 = 4.0;
const CONNECT_TOL: f64 = 5.0; // same-table line distance / intersection tol
const MIN_COL_WIDTH: f64 = 15.0;
const MIN_ROW_HEIGHT: f64 = 6.0;
const MIN_COORD_MERGE_TOL: f64 = 8.0;
const GROUP_BUCKET_CELL: f64 = 100.0;

// ── kkdoc undersegmented constants ────────────────────────────────────────────
const US_MAX_ROWS: usize = 2;
const US_MIN_COLS: usize = 3;
const US_MIN_TEXT_LINES: usize = 8;
const US_MIN_BAND_MISMATCH: usize = 2;
const US_MIN_BAND_EPSILON: f64 = 3.0;
const US_BAND_EPSILON_RATIO: f64 = 0.6;

// ── kkdoc closeOpenTableEdges constants (line-extract.ts) ──────────────────────
const EDGE_ALIGN_TOL: f64 = 3.0; // endpoint-aligned grouping tolerance
const EDGE_MIN_RULES: usize = 3; // ≥3 rules (2+ rows) to synthesize
const EDGE_MIN_SPAN: f64 = 12.0; // min group y-span (guards decorative doubles)
const EDGE_INSET: f64 = 15.0; // interior-vertical test inset from a side
const EDGE_NEAR: f64 = 10.0; // an existing vertical this close ⇒ side already closed
const EDGE_CONNECT_TOL: f64 = 5.0; // crossing tolerance (== CONNECT_TOL)
const EDGE_YGAP_SPLIT_K: f64 = 2.5; // group split when y-gap > median * K
const EDGE_YGAP_ABS_MIN: f64 = 30.0; // absolute y-gap floor for a split
const CHAIN_Y_TOL: f64 = 1.5; // chain view: same-logical-rule y tolerance
const CHAIN_GAP: f64 = 3.0; // chain view: collinear connect gap

// ── kkdoc splitStackedGroup constants (table-grid.ts) ─────────────────────────
const CUT_FULLWIDTH_RATIO: f64 = 0.9; // cut line must span ≥ ratio of group width
const CUT_CROSS_EPS: f64 = 2.0; // cut-line / crossing y tolerance
const CUT_MIN_SIDE_VERTICALS: usize = 2; // each side must look table-shaped
const CUT_EDGE_MARGIN: f64 = 12.0; // interior-vertical margin from group edges
const CUT_INTERIOR_MATCH_TOL: f64 = 8.0; // interior column x-match tolerance
const CUT_MAX_INTERIOR_OVERLAP: f64 = 0.5; // >half interior overlap ⇒ same table
const CUT_VCHAIN_X_TOL: f64 = 1.5; // vertical chain view: same-x tolerance
const CUT_VCHAIN_GAP: f64 = 1.0; // vertical chain view: collinear connect gap

// ──────────────────────────────────────────────────────────────────────────────
// Types (ported from line-types.ts)
// ──────────────────────────────────────────────────────────────────────────────

/// An axis-aligned ruling line segment in page space. For a horizontal segment
/// `y1 == y2`; for a vertical one `x1 == x2`.
#[derive(Debug, Clone, Copy)]
pub struct LineSegment {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub line_width: f64,
    /// Segment came from a `fill` op — its `line_width` is a stale inherited
    /// value (the `w` operator only affects stroking).
    pub from_fill: bool,
}

/// A rectangular grid: row Y boundaries (descending, top→bottom in PDF space)
/// and column X boundaries (ascending, left→right).
#[derive(Debug, Clone)]
pub struct TableGrid {
    pub row_ys: Vec<f64>,
    pub col_xs: Vec<f64>,
    pub bbox: BBox,
    pub vertex_radius: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BBox {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// A cell sliced out of a grid, carrying its span and pixel bbox.
#[derive(Debug, Clone, Copy)]
pub struct ExtractedCell {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Copy)]
struct Vertex {
    x: f64,
    y: f64,
    radius: f64,
}

/// A detected table exposed in two shapes: `pdf` for the existing
/// flat-markdown render path (`PdfTable::to_markdown`, interleaved by
/// `y_top`/`y_bottom`), and `ir` for callers that want merged-cell fidelity
/// (`IRTable` → `blocks_to_markdown`, which emits an HTML `<table>` when the
/// table has any colspan/rowspan > 1).
#[derive(Debug, Clone)]
pub struct DetectedTable {
    pub pdf: PdfTable,
    pub ir: IRTable,
}

// ──────────────────────────────────────────────────────────────────────────────
// Line extraction from the content stream (ported from line-extract.ts,
// reusing the exact CTM convention of PdfParser::extract_positioned_text so
// ruling lines share page-space with positioned text).
// ──────────────────────────────────────────────────────────────────────────────

/// Extract horizontal and vertical ruling lines from a page's content stream.
///
/// Returns `(horizontals, verticals)` already classified and snapped. Curves
/// are skipped. Rectangles are decomposed: a thin rectangle becomes a single
/// mid-axis line, a real box becomes its four edges.
pub fn extract_ruling_lines(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> (Vec<LineSegment>, Vec<LineSegment>) {
    let mut horizontals = Vec::new();
    let mut verticals = Vec::new();

    let content_bytes = match doc.get_page_content(page_id) {
        Ok(c) => c,
        Err(_) => return (horizontals, verticals),
    };
    let content = match Content::decode(&content_bytes) {
        Ok(c) => c,
        Err(_) => return (horizontals, verticals),
    };

    // CTM in the same row representation as parser.rs: [a c e; b d f].
    let mut a = 1.0_f64;
    let mut b = 0.0_f64;
    let mut c = 0.0_f64;
    let mut d = 1.0_f64;
    let mut e = 0.0_f64;
    let mut f = 0.0_f64;
    let mut gs_stack: Vec<(f64, f64, f64, f64, f64, f64)> = Vec::new();
    let mut line_width = 1.0_f64;
    let mut lw_stack: Vec<f64> = Vec::new();

    // Current path (segments already transformed to page space) and pen.
    let mut path: Vec<LineSegment> = Vec::new();
    let mut cur_x = 0.0_f64;
    let mut cur_y = 0.0_f64;
    let mut start_x = 0.0_f64;
    let mut start_y = 0.0_f64;

    let read_num = |obj: &Object| -> Option<f64> {
        match obj {
            Object::Integer(n) => Some(*n as f64),
            Object::Real(n) => Some(*n as f64),
            _ => None,
        }
    };
    let apply = |x: f64, y: f64, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64| -> (f64, f64) {
        (a * x + c * y + e, b * x + d * y + f)
    };

    for op in &content.operations {
        match op.operator.as_str() {
            "q" => {
                gs_stack.push((a, b, c, d, e, f));
                lw_stack.push(line_width);
            }
            "Q" => {
                if let Some((na, nb, nc, nd, ne, nf)) = gs_stack.pop() {
                    a = na; b = nb; c = nc; d = nd; e = ne; f = nf;
                }
                if let Some(w) = lw_stack.pop() {
                    line_width = w;
                }
            }
            "cm" => {
                if op.operands.len() >= 6 {
                    let v: Vec<f64> = op.operands.iter().take(6).map(|o| read_num(o).unwrap_or(0.0)).collect();
                    let (a1, b1, c1, d1, e1, f1) = (v[0], v[1], v[2], v[3], v[4], v[5]);
                    let (a2, b2, c2, d2, e2, f2) = (a, b, c, d, e, f);
                    a = a1 * a2 + b1 * c2;
                    b = a1 * b2 + b1 * d2;
                    c = c1 * a2 + d1 * c2;
                    d = c1 * b2 + d1 * d2;
                    e = e1 * a2 + f1 * c2 + e2;
                    f = e1 * b2 + f1 * d2 + f2;
                }
            }
            "w" => {
                if let Some(w) = op.operands.first().and_then(read_num) {
                    line_width = w;
                }
            }
            "m" => {
                if op.operands.len() >= 2 {
                    cur_x = read_num(&op.operands[0]).unwrap_or(0.0);
                    cur_y = read_num(&op.operands[1]).unwrap_or(0.0);
                    start_x = cur_x;
                    start_y = cur_y;
                }
            }
            "l" => {
                if op.operands.len() >= 2 {
                    let x2 = read_num(&op.operands[0]).unwrap_or(0.0);
                    let y2 = read_num(&op.operands[1]).unwrap_or(0.0);
                    let (px1, py1) = apply(cur_x, cur_y, a, b, c, d, e, f);
                    let (px2, py2) = apply(x2, y2, a, b, c, d, e, f);
                    path.push(raw_seg(px1, py1, px2, py2));
                    cur_x = x2;
                    cur_y = y2;
                }
            }
            "c" => { advance_curve(&op.operands, 3, &mut cur_x, &mut cur_y, read_num); }
            "v" | "y" => { advance_curve(&op.operands, 2, &mut cur_x, &mut cur_y, read_num); }
            "h" => {
                if (cur_x - start_x).abs() > f64::EPSILON || (cur_y - start_y).abs() > f64::EPSILON {
                    let (px1, py1) = apply(cur_x, cur_y, a, b, c, d, e, f);
                    let (px2, py2) = apply(start_x, start_y, a, b, c, d, e, f);
                    path.push(raw_seg(px1, py1, px2, py2));
                }
                cur_x = start_x;
                cur_y = start_y;
            }
            "re" => {
                if op.operands.len() >= 4 {
                    let rx = read_num(&op.operands[0]).unwrap_or(0.0);
                    let ry = read_num(&op.operands[1]).unwrap_or(0.0);
                    let rw = read_num(&op.operands[2]).unwrap_or(0.0);
                    let rh = read_num(&op.operands[3]).unwrap_or(0.0);
                    push_rectangle(&mut path, rx, ry, rw, rh, a, b, c, d, e, f);
                    // `re` sets the current point to (rx, ry) per PDF spec.
                    cur_x = rx;
                    cur_y = ry;
                    start_x = rx;
                    start_y = ry;
                }
            }
            // Paint operators flush the path.
            "S" | "s" => flush_path(&mut path, line_width, ctm_scale(a, b, c, d), false, &mut horizontals, &mut verticals),
            "f" | "F" | "f*" => flush_path(&mut path, line_width, ctm_scale(a, b, c, d), true, &mut horizontals, &mut verticals),
            "B" | "B*" | "b" | "b*" => flush_path(&mut path, line_width, ctm_scale(a, b, c, d), false, &mut horizontals, &mut verticals),
            "n" => path.clear(),
            _ => {}
        }
    }

    (horizontals, verticals)
}

fn advance_curve(
    operands: &[Object],
    points: usize,
    cur_x: &mut f64,
    cur_y: &mut f64,
    read_num: impl Fn(&Object) -> Option<f64>,
) {
    // The last coordinate pair of a Bézier op is the new current point.
    let need = points * 2;
    if operands.len() >= need {
        *cur_x = read_num(&operands[need - 2]).unwrap_or(*cur_x);
        *cur_y = read_num(&operands[need - 1]).unwrap_or(*cur_y);
    }
}

/// Untyped raw segment (page-space endpoints, width filled at flush time).
fn raw_seg(x1: f64, y1: f64, x2: f64, y2: f64) -> LineSegment {
    LineSegment { x1, y1, x2, y2, line_width: 0.0, from_fill: false }
}

fn ctm_scale(a: f64, b: f64, c: f64, d: f64) -> f64 {
    (a.hypot(b) + c.hypot(d)) / 2.0
}

/// Decompose a rectangle into ruling lines, using CTM-scaled effective
/// thickness to decide thin-line vs. real-box (kkdoc pushRectangle).
#[allow(clippy::too_many_arguments)]
fn push_rectangle(
    path: &mut Vec<LineSegment>,
    rx: f64, ry: f64, rw: f64, rh: f64,
    a: f64, b: f64, c: f64, d: f64, e: f64, f: f64,
) {
    let apply = |x: f64, y: f64| -> (f64, f64) { (a * x + c * y + e, b * x + d * y + f) };
    let eff_h = rh.abs() * c.hypot(d);
    let eff_w = rw.abs() * a.hypot(b);
    let thin = ORIENTATION_TOL * 2.0; // < 4
    if eff_h < thin {
        let (p1x, p1y) = apply(rx, ry + rh / 2.0);
        let (p2x, p2y) = apply(rx + rw, ry + rh / 2.0);
        path.push(raw_seg(p1x, p1y, p2x, p2y));
    } else if eff_w < thin {
        let (p1x, p1y) = apply(rx + rw / 2.0, ry);
        let (p2x, p2y) = apply(rx + rw / 2.0, ry + rh);
        path.push(raw_seg(p1x, p1y, p2x, p2y));
    } else {
        let (blx, bly) = apply(rx, ry);
        let (brx, bry) = apply(rx + rw, ry);
        let (trx, try_) = apply(rx + rw, ry + rh);
        let (tlx, tly) = apply(rx, ry + rh);
        path.push(raw_seg(blx, bly, brx, bry)); // bottom
        path.push(raw_seg(brx, bry, trx, try_)); // right
        path.push(raw_seg(trx, try_, tlx, tly)); // top
        path.push(raw_seg(tlx, tly, blx, bly)); // left
    }
}

fn flush_path(
    path: &mut Vec<LineSegment>,
    line_width: f64,
    scale: f64,
    from_fill: bool,
    horizontals: &mut Vec<LineSegment>,
    verticals: &mut Vec<LineSegment>,
) {
    let eff_width = line_width * scale;
    for seg in path.iter() {
        classify_and_add(seg, eff_width, from_fill, horizontals, verticals);
    }
    path.clear();
}

/// Classify a segment as horizontal/vertical (or drop it) and snap it to a
/// single averaged perpendicular coordinate (kkdoc classifyAndAdd).
fn classify_and_add(
    seg: &LineSegment,
    line_width: f64,
    from_fill: bool,
    horizontals: &mut Vec<LineSegment>,
    verticals: &mut Vec<LineSegment>,
) {
    let dx = (seg.x2 - seg.x1).abs();
    let dy = (seg.y2 - seg.y1).abs();
    let length = (dx * dx + dy * dy).sqrt();
    if length < MIN_LINE_LENGTH {
        return;
    }
    if dy <= ORIENTATION_TOL {
        let y = (seg.y1 + seg.y2) / 2.0;
        let x1 = seg.x1.min(seg.x2);
        let x2 = seg.x1.max(seg.x2);
        horizontals.push(LineSegment { x1, y1: y, x2, y2: y, line_width, from_fill });
    } else if dx <= ORIENTATION_TOL {
        let x = (seg.x1 + seg.x2) / 2.0;
        let y1 = seg.y1.min(seg.y2);
        let y2 = seg.y1.max(seg.y2);
        verticals.push(LineSegment { x1: x, y1, x2: x, y2, line_width, from_fill });
    }
    // diagonal → dropped
}

// ──────────────────────────────────────────────────────────────────────────────
// Preprocessing (line-extract.ts): thick filter → shading stacks → merge.
// ──────────────────────────────────────────────────────────────────────────────

/// Run the preprocessing pipeline on both orientations.
pub fn preprocess_lines(
    horizontals: Vec<LineSegment>,
    verticals: Vec<LineSegment>,
) -> (Vec<LineSegment>, Vec<LineSegment>) {
    let h = merge_parallel_lines(drop_shading_stacks(thick_filter(horizontals), true), true);
    let v = merge_parallel_lines(drop_shading_stacks(thick_filter(verticals), false), false);
    (h, v)
}

fn thick_filter(lines: Vec<LineSegment>) -> Vec<LineSegment> {
    lines.into_iter().filter(|l| l.line_width <= MAX_LINE_WIDTH).collect()
}

/// Perpendicular position of a line (y for horizontal, x for vertical).
#[inline]
fn perp(l: &LineSegment, horizontal: bool) -> f64 {
    if horizontal { l.y1 } else { l.x1 }
}
/// Start along the shared axis.
#[inline]
fn along_start(l: &LineSegment, horizontal: bool) -> f64 {
    if horizontal { l.x1 } else { l.y1 }
}
/// End along the shared axis.
#[inline]
fn along_end(l: &LineSegment, horizontal: bool) -> f64 {
    if horizontal { l.x2 } else { l.y2 }
}

/// Drop shading/gradient stacks — dense runs of identical-span parallel lines
/// (kkdoc dropShadingStacks), with end-trimming to protect real borders that
/// happen to touch the stack.
fn drop_shading_stacks(lines: Vec<LineSegment>, horizontal: bool) -> Vec<LineSegment> {
    if lines.len() < STACK_MIN_LINES {
        return lines;
    }
    // Group by identical rounded span.
    let mut groups: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, l) in lines.iter().enumerate() {
        let key = (
            along_start(l, horizontal).round() as i64,
            along_end(l, horizontal).round() as i64,
        );
        groups.entry(key).or_default().push(i);
    }
    let mut dropped = vec![false; lines.len()];
    let mut any_dropped = false;
    for (_, idxs) in groups.iter() {
        if idxs.len() < STACK_MIN_LINES {
            continue;
        }
        let mut members = idxs.clone();
        members.sort_by(|&x, &y| {
            perp(&lines[x], horizontal)
                .partial_cmp(&perp(&lines[y], horizontal))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Walk runs where consecutive gap < STACK_GAP.
        let mut run_start = 0usize;
        let n = members.len();
        for i in 1..=n {
            let gap = if i < n {
                perp(&lines[members[i]], horizontal) - perp(&lines[members[i - 1]], horizontal)
            } else {
                f64::INFINITY
            };
            if gap >= STACK_GAP {
                let s = run_start;
                let ep = i - 1;
                if ep - s + 1 >= STACK_MIN_LINES {
                    if let Some((ts, te)) = trim_stack(&members, &lines, horizontal, s, ep) {
                        if te - ts + 1 >= STACK_MIN_LINES {
                            for k in ts..=te {
                                dropped[members[k]] = true;
                                any_dropped = true;
                            }
                        }
                    }
                }
                run_start = i;
            }
        }
    }
    if !any_dropped {
        return lines;
    }
    lines
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !dropped[*i])
        .map(|(_, l)| l)
        .collect()
}

/// End-trim a stack run so a real border line touching the shading is not
/// swallowed (kkdoc edgeAlien logic). Returns the trimmed `(start, end)` index
/// range into `members`, or None.
fn trim_stack(
    members: &[usize],
    lines: &[LineSegment],
    horizontal: bool,
    mut s: usize,
    mut e: usize,
) -> Option<(usize, usize)> {
    // Dominant width bucket.
    let wkey = |l: &LineSegment| (l.line_width * 100.0).round() as i64;
    let mut wcount: HashMap<i64, usize> = HashMap::new();
    for k in s..=e {
        *wcount.entry(wkey(&lines[members[k]])).or_default() += 1;
    }
    let dom_w = wcount.iter().max_by_key(|(_, c)| **c).map(|(w, _)| *w).unwrap_or(0);
    // Median internal pitch.
    let mut pitches: Vec<f64> = Vec::new();
    for k in (s + 1)..=e {
        pitches.push(perp(&lines[members[k]], horizontal) - perp(&lines[members[k - 1]], horizontal));
    }
    pitches.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let med_pitch = if pitches.is_empty() { 0.0 } else { pitches[pitches.len() / 2] };
    // Fill majority.
    let fill_n = (s..=e).filter(|&k| lines[members[k]].from_fill).count();
    let stack_is_fill = fill_n * 2 > (e - s + 1);

    let edge_alien = |idx: usize, inward_gap: f64| -> bool {
        let l = &lines[members[idx]];
        (stack_is_fill && !l.from_fill)
            || wkey(l) != dom_w
            || (med_pitch > 0.0 && inward_gap > med_pitch * 1.8)
    };

    while e - s + 1 >= STACK_MIN_LINES {
        let lead_gap = perp(&lines[members[s + 1]], horizontal) - perp(&lines[members[s]], horizontal);
        let trail_gap = perp(&lines[members[e]], horizontal) - perp(&lines[members[e - 1]], horizontal);
        if edge_alien(s, lead_gap) {
            s += 1;
        } else if edge_alien(e, trail_gap) {
            e -= 1;
        } else {
            break;
        }
    }
    Some((s, e))
}

/// Merge near-parallel duplicate lines (kkdoc mergeParallelLines).
fn merge_parallel_lines(mut lines: Vec<LineSegment>, horizontal: bool) -> Vec<LineSegment> {
    if lines.len() <= 1 {
        return lines;
    }
    lines.sort_by(|x, y| {
        let px = perp(x, horizontal);
        let py = perp(y, horizontal);
        if (px - py).abs() <= 0.1 {
            along_start(x, horizontal)
                .partial_cmp(&along_start(y, horizontal))
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            px.partial_cmp(&py).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    let mut result: Vec<LineSegment> = Vec::new();
    for curr in lines {
        if let Some(prev) = result.last_mut() {
            let prev_pos = perp(prev, horizontal);
            let curr_pos = perp(&curr, horizontal);
            if (prev_pos - curr_pos).abs() <= MERGE_TOL {
                let prev_start = along_start(prev, horizontal);
                let prev_end = along_end(prev, horizontal);
                let curr_start = along_start(&curr, horizontal);
                let curr_end = along_end(&curr, horizontal);
                let overlap = prev_end.min(curr_end) - prev_start.max(curr_start);
                let min_len = (prev_end - prev_start).min(curr_end - curr_start);
                if overlap > min_len * 0.3 {
                    let new_start = prev_start.min(curr_start);
                    let new_end = prev_end.max(curr_end);
                    let new_perp = (prev_pos + curr_pos) / 2.0;
                    let new_w = prev.line_width.max(curr.line_width);
                    if horizontal {
                        prev.x1 = new_start; prev.x2 = new_end; prev.y1 = new_perp; prev.y2 = new_perp;
                    } else {
                        prev.y1 = new_start; prev.y2 = new_end; prev.x1 = new_perp; prev.x2 = new_perp;
                    }
                    prev.line_width = new_w;
                    continue;
                }
            }
        }
        result.push(curr);
    }
    result
}

// ──────────────────────────────────────────────────────────────────────────────
// Open-edge border synthesis (line-extract.ts closeOpenTableEdges)
// ──────────────────────────────────────────────────────────────────────────────

/// Chain view — join collinear same-y horizontal segments (rules drawn cell by
/// cell) into one logical rule. Endpoint-alignment judgement only; the physical
/// horizontals are untouched. Near-parallel decorative doubles are absorbed too.
fn chain_collinear_rules(horizontals: &[LineSegment]) -> Vec<LineSegment> {
    if horizontals.len() <= 1 {
        return horizontals.to_vec();
    }
    let mut sorted = horizontals.to_vec();
    sorted.sort_by(|a, b| {
        a.y1
            .partial_cmp(&b.y1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.x1.partial_cmp(&b.x1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut rules = Vec::new();
    let n = sorted.len();
    let mut band_start = 0usize;
    for i in 1..=n {
        if i == n || sorted[i].y1 - sorted[band_start].y1 > CHAIN_Y_TOL {
            let mut band: Vec<LineSegment> = sorted[band_start..i].to_vec();
            band.sort_by(|a, b| a.x1.partial_cmp(&b.x1).unwrap_or(std::cmp::Ordering::Equal));
            let mut cur = band[0];
            for seg in band.iter().skip(1) {
                if seg.x1 - cur.x2 <= CHAIN_GAP {
                    if seg.x2 > cur.x2 {
                        cur.x2 = seg.x2;
                    }
                    if seg.line_width > cur.line_width {
                        cur.line_width = seg.line_width;
                    }
                } else {
                    rules.push(cur);
                    cur = *seg;
                }
            }
            rules.push(cur);
            band_start = i;
        }
    }
    rules
}

/// Synthesize missing outer side borders — Korean-gov tables commonly omit the
/// left/right outer borders (horizontal rules span the full width; verticals
/// draw only interior separators). Vertex-based grid construction loses the
/// column at an unruled side entirely, so for an endpoint-aligned bundle of
/// horizontal rules (≥3) that has ≥1 interior vertical crossing 2+ of those
/// rules, synthesize a phantom vertical border at each endpoint x that has no
/// existing vertical nearby, closing the grid.
///
/// Global endpoint grouping tends to lump similar-width tables stacked on a
/// page into one bundle with a wide y-range; a large intra-bundle y-gap that is
/// not bridged by a penetrating interior vertical splits the bundle, so a
/// phantom border is never welded across the prose between two separate tables.
fn close_open_table_edges(
    horizontals: &[LineSegment],
    verticals: &[LineSegment],
) -> Vec<LineSegment> {
    if horizontals.len() < EDGE_MIN_RULES {
        return verticals.to_vec();
    }

    // 1) endpoint-aligned grouping over chained logical rules
    let mut groups: Vec<Vec<LineSegment>> = Vec::new();
    for hl in chain_collinear_rules(horizontals) {
        let mut placed = false;
        for g in groups.iter_mut() {
            if (g[0].x1 - hl.x1).abs() <= EDGE_ALIGN_TOL && (g[0].x2 - hl.x2).abs() <= EDGE_ALIGN_TOL
            {
                g.push(hl);
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push(vec![hl]);
        }
    }

    // 1b) y-gap split — separate stacked same-width tables lumped into one group
    //     at the large vertical gap between them, unless a vertical bridges the
    //     gap within this table's x-range (a merged tall row, not a table break).
    let mut split_groups: Vec<Vec<LineSegment>> = Vec::new();
    for g in &groups {
        let mut sorted = g.clone();
        sorted.sort_by(|a, b| a.y1.partial_cmp(&b.y1).unwrap_or(std::cmp::Ordering::Equal));
        let mut gaps: Vec<f64> = Vec::new();
        for i in 1..sorted.len() {
            gaps.push(sorted[i].y1 - sorted[i - 1].y1);
        }
        let median = if gaps.is_empty() {
            0.0
        } else {
            let mut gs = gaps.clone();
            gs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            gs[gs.len() >> 1]
        };
        let threshold = median * EDGE_YGAP_SPLIT_K;
        let mut cur: Vec<LineSegment> = if sorted.is_empty() { Vec::new() } else { vec![sorted[0]] };
        for i in 1..sorted.len() {
            let y_lo = sorted[i - 1].y1;
            let y_hi = sorted[i].y1;
            let gap = y_hi - y_lo;
            let gx1 = sorted[i - 1].x1.min(sorted[i].x1);
            let gx2 = sorted[i - 1].x2.max(sorted[i].x2);
            let bridged = verticals.iter().any(|vl| {
                vl.y1 <= y_lo + EDGE_NEAR
                    && vl.y2 >= y_hi - EDGE_NEAR
                    && vl.x1 >= gx1 - EDGE_CONNECT_TOL
                    && vl.x1 <= gx2 + EDGE_CONNECT_TOL
            });
            if median > 0.0
                && gap > threshold
                && gap > EDGE_YGAP_ABS_MIN
                && !bridged
                && cur.len() >= EDGE_MIN_RULES
                && sorted.len() - i >= EDGE_MIN_RULES
            {
                split_groups.push(std::mem::take(&mut cur));
            }
            cur.push(sorted[i]);
        }
        if !cur.is_empty() {
            split_groups.push(cur);
        }
    }

    let mut synthesized: Vec<LineSegment> = Vec::new();
    for g in &split_groups {
        if g.len() < EDGE_MIN_RULES {
            continue;
        }
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        let mut x1 = 0.0;
        let mut x2 = 0.0;
        for hl in g {
            if hl.y1 < y_min {
                y_min = hl.y1;
            }
            if hl.y1 > y_max {
                y_max = hl.y1;
            }
            x1 += hl.x1;
            x2 += hl.x2;
        }
        let count = g.len() as f64;
        x1 /= count;
        x2 /= count;
        if y_max - y_min < EDGE_MIN_SPAN {
            continue;
        }

        // 2) require a real interior vertical crossing ≥2 group rules
        let cross_count = |v: &LineSegment| -> usize {
            let mut n = 0;
            for hl in g {
                if v.x1 >= hl.x1 - EDGE_CONNECT_TOL
                    && v.x1 <= hl.x2 + EDGE_CONNECT_TOL
                    && hl.y1 >= v.y1 - EDGE_CONNECT_TOL
                    && hl.y1 <= v.y2 + EDGE_CONNECT_TOL
                {
                    n += 1;
                }
            }
            n
        };
        let has_interior = verticals
            .iter()
            .any(|v| v.x1 > x1 + EDGE_INSET && v.x1 < x2 - EDGE_INSET && cross_count(v) >= 2);
        if !has_interior {
            continue;
        }

        // 3) synthesize a phantom border at each side lacking an existing vertical
        for edge_x in [x1, x2] {
            let closed = verticals.iter().any(|v| {
                (v.x1 - edge_x).abs() <= EDGE_NEAR
                    && v.y1 <= y_max + EDGE_CONNECT_TOL
                    && v.y2 >= y_min - EDGE_CONNECT_TOL
            });
            if !closed {
                synthesized.push(LineSegment {
                    x1: edge_x,
                    y1: y_min,
                    x2: edge_x,
                    y2: y_max,
                    line_width: 0.5,
                    from_fill: false,
                });
            }
        }
    }

    if synthesized.is_empty() {
        verticals.to_vec()
    } else {
        let mut out = verticals.to_vec();
        out.extend(synthesized);
        out
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Grid construction (table-grid.ts)
// ──────────────────────────────────────────────────────────────────────────────

fn lines_intersect(a: &LineSegment, b: &LineSegment) -> bool {
    let a_h = (a.y2 - a.y1).abs() <= f64::EPSILON;
    let b_h = (b.y2 - b.y1).abs() <= f64::EPSILON;
    let t = CONNECT_TOL;
    match (a_h, b_h) {
        (true, true) => {
            (a.y1 - b.y1).abs() <= t && a.x2.min(b.x2) >= a.x1.max(b.x1) - t
        }
        (false, false) => {
            (a.x1 - b.x1).abs() <= t && a.y2.min(b.y2) >= a.y1.max(b.y1) - t
        }
        _ => {
            let (h, v) = if a_h { (a, b) } else { (b, a) };
            v.x1 >= h.x1 - t && v.x1 <= h.x2 + t && h.y1 >= v.y1 - t && h.y1 <= v.y2 + t
        }
    }
}

fn build_vertices(horizontals: &[LineSegment], verticals: &[LineSegment]) -> Vec<Vertex> {
    let mut out = Vec::new();
    let t = CONNECT_TOL;
    for h in horizontals {
        for v in verticals {
            if v.x1 >= h.x1 - t && v.x1 <= h.x2 + t && h.y1 >= v.y1 - t && h.y1 <= v.y2 + t {
                out.push(Vertex { x: v.x1, y: h.y1, radius: h.line_width.max(v.line_width).max(1.0) });
            }
        }
    }
    out
}

/// Merge near-coincident vertices via a bucket grid. Faithfully replicates
/// kkdoc's asymmetry: distance is tested against the *seed* vertex while the
/// tolerance grows with the accumulated max radius.
fn merge_vertices(vertices: &[Vertex]) -> Vec<Vertex> {
    if vertices.len() <= 1 {
        return vertices.to_vec();
    }
    let max_radius_all = vertices.iter().map(|v| v.radius).fold(1.0_f64, f64::max);
    let cell = (VERTEX_MERGE_FACTOR * max_radius_all).max(1.0);
    let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, v) in vertices.iter().enumerate() {
        let key = ((v.x / cell).floor() as i64, (v.y / cell).floor() as i64);
        buckets.entry(key).or_default().push(i);
    }
    let mut used = vec![false; vertices.len()];
    let mut out = Vec::new();
    for i in 0..vertices.len() {
        if used[i] {
            continue;
        }
        let seed = vertices[i];
        let mut sum_x = seed.x;
        let mut sum_y = seed.y;
        let mut max_radius = seed.radius;
        let mut count = 1.0_f64;
        used[i] = true;
        // Gather candidates j>i from the 3x3 neighborhood.
        let bx = (seed.x / cell).floor() as i64;
        let by = (seed.y / cell).floor() as i64;
        let mut candidates: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = buckets.get(&(bx + dx, by + dy)) {
                    for &j in v {
                        if j > i && !used[j] {
                            candidates.push(j);
                        }
                    }
                }
            }
        }
        candidates.sort_unstable();
        for j in candidates {
            if used[j] {
                continue;
            }
            let merge_tol = VERTEX_MERGE_FACTOR * max_radius.max(vertices[j].radius);
            if (seed.x - vertices[j].x).abs() <= merge_tol && (seed.y - vertices[j].y).abs() <= merge_tol {
                sum_x += vertices[j].x;
                sum_y += vertices[j].y;
                max_radius = max_radius.max(vertices[j].radius);
                count += 1.0;
                used[j] = true;
            }
        }
        out.push(Vertex { x: sum_x / count, y: sum_y / count, radius: max_radius });
    }
    out
}

/// Union-Find connected components over lines, bucket-pruned (kkdoc
/// groupConnectedLines). Returns groups of `(is_horizontal, index)`.
fn group_connected_lines(
    horizontals: &[LineSegment],
    verticals: &[LineSegment],
) -> Vec<Vec<(bool, usize)>> {
    // Flatten to a single index space: [0..h) horizontals, [h..) verticals.
    let nh = horizontals.len();
    let all: Vec<LineSegment> =
        horizontals.iter().chain(verticals.iter()).copied().collect();
    let n = all.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    let t = CONNECT_TOL;
    let mut buckets: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, l) in all.iter().enumerate() {
        let cx1 = ((l.x1.min(l.x2) - t) / GROUP_BUCKET_CELL).floor() as i64;
        let cx2 = ((l.x1.max(l.x2) + t) / GROUP_BUCKET_CELL).floor() as i64;
        let cy1 = ((l.y1.min(l.y2) - t) / GROUP_BUCKET_CELL).floor() as i64;
        let cy2 = ((l.y1.max(l.y2) + t) / GROUP_BUCKET_CELL).floor() as i64;
        for cx in cx1..=cx2 {
            for cy in cy1..=cy2 {
                buckets.entry((cx, cy)).or_default().push(i);
            }
        }
    }
    let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for idxs in buckets.values() {
        for (a_pos, &i) in idxs.iter().enumerate() {
            for &j in idxs.iter().skip(a_pos + 1) {
                let (lo, hi) = if i < j { (i, j) } else { (j, i) };
                let key = (lo as u64) * (n as u64) + (hi as u64);
                if !seen.insert(key) {
                    continue;
                }
                if lines_intersect(&all[i], &all[j]) {
                    union(&mut parent, i, j);
                }
            }
        }
    }
    let mut comps: HashMap<usize, Vec<(bool, usize)>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        let entry = if i < nh { (true, i) } else { (false, i - nh) };
        comps.entry(r).or_default().push(entry);
    }
    comps.into_values().collect()
}

/// 1-D running-mean clustering (kkdoc clusterCoordinates). Comparison is
/// against the running average, allowing a cluster to drift.
fn cluster_coordinates(values: &[f64], tolerance: f64) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = Vec::new();
    let mut sum = sorted[0];
    let mut count = 1.0_f64;
    for &v in &sorted[1..] {
        let avg = sum / count;
        if (v - avg).abs() <= tolerance {
            sum += v;
            count += 1.0;
        } else {
            out.push(sum / count);
            sum = v;
            count = 1.0;
        }
    }
    out.push(sum / count);
    out
}

fn enforce_min_width(col_xs: &[f64]) -> Vec<f64> {
    enforce_min(col_xs, MIN_COL_WIDTH, false)
}
fn enforce_min_height(row_ys: &[f64]) -> Vec<f64> {
    // row_ys are descending: gap = prev - curr.
    enforce_min(row_ys, MIN_ROW_HEIGHT, true)
}
fn enforce_min(coords: &[f64], min_delta: f64, descending: bool) -> Vec<f64> {
    if coords.len() <= 2 {
        return coords.to_vec();
    }
    let n = coords.len();
    let mut result = vec![coords[0]];
    for i in 1..n {
        let last = *result.last().unwrap();
        let delta = if descending { last - coords[i] } else { coords[i] - last };
        if delta < min_delta && i < n - 1 {
            continue; // merge into next
        }
        result.push(coords[i]);
    }
    result
}

/// Vertical chain view (kkdoc chainVerticals) — join collinear same-x vertical
/// segments (outer borders / separators drawn section by section) into logical
/// vertical spans `(y1, y2)`. A single table's segments abut (gap≈0); separate
/// tables leave a real gap. Judgement only; physical verticals are untouched.
fn chain_verticals(vs: &[LineSegment]) -> Vec<(f64, f64)> {
    if vs.len() <= 1 {
        return vs.iter().map(|v| (v.y1, v.y2)).collect();
    }
    let mut sorted = vs.to_vec();
    sorted.sort_by(|a, b| {
        a.x1
            .partial_cmp(&b.x1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.y1.partial_cmp(&b.y1).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut rules = Vec::new();
    let n = sorted.len();
    let mut band_start = 0usize;
    for i in 1..=n {
        if i == n || sorted[i].x1 - sorted[band_start].x1 > CUT_VCHAIN_X_TOL {
            let mut band: Vec<LineSegment> = sorted[band_start..i].to_vec();
            band.sort_by(|a, b| a.y1.partial_cmp(&b.y1).unwrap_or(std::cmp::Ordering::Equal));
            let mut cur = (band[0].y1, band[0].y2);
            for seg in band.iter().skip(1) {
                if seg.y1 - cur.1 <= CUT_VCHAIN_GAP {
                    if seg.y2 > cur.1 {
                        cur.1 = seg.y2;
                    }
                } else {
                    rules.push(cur);
                    cur = (seg.y1, seg.y2);
                }
            }
            rules.push(cur);
            band_start = i;
        }
    }
    rules
}

/// Split a stacked group (kkdoc splitStackedGroup) — two separate tables abutting
/// vertically that share one boundary rule (so Union-Find lumped them) are split
/// into vertical bands. A cut line is a near-full-width horizontal that (a) is not
/// penetrated by any logical vertical (chain view) — a real interior boundary has
/// its outer verticals run continuously through it, (b) has 2+ verticals on each
/// side, and (c) whose interior column x-sets overlap by ≤ half. The cut line
/// itself is duplicated into both bands (bottom edge of the upper / top of lower).
fn split_stacked_group(group: &[(bool, LineSegment)]) -> Vec<Vec<(bool, LineSegment)>> {
    let hs: Vec<LineSegment> = group.iter().filter(|(h, _)| *h).map(|(_, l)| *l).collect();
    let vs: Vec<LineSegment> = group.iter().filter(|(h, _)| !*h).map(|(_, l)| *l).collect();
    if hs.len() < 3 || vs.len() < 4 {
        return vec![group.to_vec()];
    }
    let mut gx1 = f64::INFINITY;
    let mut gx2 = f64::NEG_INFINITY;
    for (_, l) in group {
        if l.x1 < gx1 {
            gx1 = l.x1;
        }
        if l.x2 > gx2 {
            gx2 = l.x2;
        }
    }
    let group_w = gx2 - gx1;
    if group_w <= 0.0 {
        return vec![group.to_vec()];
    }
    let is_interior = |v: &LineSegment| v.x1 > gx1 + CUT_EDGE_MARGIN && v.x1 < gx2 - CUT_EDGE_MARGIN;
    let chained = chain_verticals(&vs);
    let mut cuts: Vec<f64> = Vec::new();
    for h in &hs {
        let y = h.y1;
        if h.x2 - h.x1 < group_w * CUT_FULLWIDTH_RATIO {
            continue;
        }
        if cuts.iter().any(|c| (c - y).abs() <= CUT_CROSS_EPS) {
            continue;
        }
        // (a) a penetrating logical vertical ⇒ same-table interior boundary
        if chained
            .iter()
            .any(|(vy1, vy2)| *vy1 < y - CUT_CROSS_EPS && *vy2 > y + CUT_CROSS_EPS)
        {
            continue;
        }
        // (b) both sides must be table-shaped
        let above: Vec<LineSegment> =
            vs.iter().filter(|v| v.y1 >= y - CUT_CROSS_EPS).copied().collect();
        let below: Vec<LineSegment> =
            vs.iter().filter(|v| v.y2 <= y + CUT_CROSS_EPS).copied().collect();
        if above.len() < CUT_MIN_SIDE_VERTICALS || below.len() < CUT_MIN_SIDE_VERTICALS {
            continue;
        }
        // (c) interior column x-set overlap
        let ia: Vec<LineSegment> = above.iter().filter(|v| is_interior(v)).copied().collect();
        let ib: Vec<LineSegment> = below.iter().filter(|v| is_interior(v)).copied().collect();
        if ia.is_empty() || ib.is_empty() {
            continue;
        }
        let mut matched = 0usize;
        for a in &ia {
            if ib.iter().any(|b| (a.x1 - b.x1).abs() <= CUT_INTERIOR_MATCH_TOL) {
                matched += 1;
            }
        }
        if matched as f64 / (ia.len().min(ib.len()) as f64) > CUT_MAX_INTERIOR_OVERLAP {
            continue;
        }
        cuts.push(y);
    }
    if cuts.is_empty() {
        return vec![group.to_vec()];
    }

    cuts.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)); // top→bottom
    let band_of = |y: f64| -> usize {
        let mut k = 0usize;
        while k < cuts.len() && y < cuts[k] {
            k += 1;
        }
        k
    };
    let mut bands: Vec<Vec<(bool, LineSegment)>> = vec![Vec::new(); cuts.len() + 1];
    for v in &vs {
        // by (a) no vertical straddles a cut, so its midpoint lands in one band.
        bands[band_of((v.y1 + v.y2) / 2.0)].push((false, *v));
    }
    for h in &hs {
        if let Some(at_cut) = cuts.iter().position(|c| (h.y1 - c).abs() <= CUT_CROSS_EPS) {
            bands[at_cut].push((true, *h));
            bands[at_cut + 1].push((true, *h));
        } else {
            bands[band_of(h.y1)].push((true, *h));
        }
    }
    bands.into_iter().filter(|b| !b.is_empty()).collect()
}

// NOTE: kkdoc's `mergeAdjacentGrids` (per-page, TableGrid-level) is intentionally
// NOT applied here. In kkdoc it stitches *same-page* over-segmented grids, but the
// only continuation this project actually needs is *cross-page* — and that is a
// separate kkdoc function, `mergeCrossPageTables` (page-blocks.ts), run at the
// block-aggregation layer with x-bbox alignment + reading-order block adjacency +
// repeated-header removal. mdm's aggregation layer is a flat `Vec<PdfTable>` that
// lacks x-extent (see `PdfTable`, parser.rs) and whose renderer keys inline-text
// dedup by `t.page == elem.page` (parser.rs) — so merging two `PdfTable`s across a
// page break would leave the continuation page's inline text un-suppressed and
// double-render it. A faithful cross-page stitch therefore needs a renderer/IR
// change beyond this refinement's scope; it is deferred (see the report / bottom
// doc block). The per-page grid merge was dropped because it cannot deliver that
// goal and risks fusing two independent same-page tables (column-matched, ≤20pt
// apart) with no demonstrated corpus benefit.

/// Build table grids from preprocessed ruling lines (kkdoc buildTableGrids),
/// including the stacked-group split refinement.
pub fn build_table_grids(horizontals: &[LineSegment], verticals: &[LineSegment]) -> Vec<TableGrid> {
    if horizontals.len() < 2 || verticals.len() < 2 {
        return Vec::new();
    }
    let all_vertices = build_vertices(horizontals, verticals);
    let vertices = merge_vertices(&all_vertices);
    if vertices.len() < 4 {
        return Vec::new();
    }
    let global_radius = vertices.iter().map(|v| v.radius).fold(1.0_f64, f64::max);

    let mut grids = Vec::new();
    // Split stacked tables sharing a boundary line into per-band groups. A split
    // band recomputes its vertices from its own lines, because the shared cut
    // line's vertices otherwise carry the other table's column x's into the band.
    let mut bands: Vec<(Vec<(bool, LineSegment)>, bool)> = Vec::new();
    for group in group_connected_lines(horizontals, verticals) {
        let typed: Vec<(bool, LineSegment)> = group
            .iter()
            .map(|(is_h, i)| if *is_h { (true, horizontals[*i]) } else { (false, verticals[*i]) })
            .collect();
        let split = split_stacked_group(&typed);
        let from_split = split.len() > 1;
        for band in split {
            bands.push((band, from_split));
        }
    }
    for (band, from_split) in &bands {
        let h_lines: Vec<LineSegment> =
            band.iter().filter(|(is_h, _)| *is_h).map(|(_, l)| *l).collect();
        let v_lines: Vec<LineSegment> =
            band.iter().filter(|(is_h, _)| !*is_h).map(|(_, l)| *l).collect();
        if h_lines.len() < 2 || v_lines.len() < 2 {
            continue;
        }
        // Group bbox from vertical x's and horizontal y's, padded by CONNECT_TOL.
        let gx1 = v_lines.iter().map(|l| l.x1).fold(f64::INFINITY, f64::min) - CONNECT_TOL;
        let gx2 = v_lines.iter().map(|l| l.x1).fold(f64::NEG_INFINITY, f64::max) + CONNECT_TOL;
        let gy1 = h_lines.iter().map(|l| l.y1).fold(f64::INFINITY, f64::min) - CONNECT_TOL;
        let gy2 = h_lines.iter().map(|l| l.y1).fold(f64::NEG_INFINITY, f64::max) + CONNECT_TOL;
        // Split bands recompute their vertices from their own lines; unsplit
        // groups keep the global-vertex bbox filter (identical to before).
        let group_vertices: Vec<Vertex> = if *from_split {
            merge_vertices(&build_vertices(&h_lines, &v_lines))
        } else {
            vertices
                .iter()
                .copied()
                .filter(|vx| vx.x >= gx1 && vx.x <= gx2 && vx.y >= gy1 && vx.y <= gy2)
                .collect()
        };
        let group_radius = if group_vertices.is_empty() {
            global_radius
        } else {
            group_vertices.iter().map(|v| v.radius).fold(1.0_f64, f64::max)
        };
        let coord_merge_tol = (VERTEX_MERGE_FACTOR * group_radius).max(MIN_COORD_MERGE_TOL);

        let mut raw_ys: Vec<f64> = h_lines.iter().map(|l| l.y1).collect();
        raw_ys.extend(group_vertices.iter().map(|v| v.y));
        let mut row_ys = cluster_coordinates(&raw_ys, coord_merge_tol);
        row_ys.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal)); // descending

        let mut raw_xs: Vec<f64> = v_lines.iter().map(|l| l.x1).collect();
        raw_xs.extend(group_vertices.iter().map(|v| v.x));
        let mut col_xs = cluster_coordinates(&raw_xs, coord_merge_tol);
        col_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)); // ascending

        if row_ys.len() < 2 || col_xs.len() < 2 {
            continue;
        }
        let valid_cols = enforce_min_width(&col_xs);
        let valid_rows = enforce_min_height(&row_ys);
        if valid_cols.len() < 2 || valid_rows.len() < 2 {
            continue;
        }
        let bbox = BBox {
            x1: valid_cols[0],
            y1: *valid_rows.last().unwrap(),
            x2: *valid_cols.last().unwrap(),
            y2: valid_rows[0],
        };
        grids.push(TableGrid { row_ys: valid_rows, col_xs: valid_cols, bbox, vertex_radius: group_radius });
    }
    grids
}

// ──────────────────────────────────────────────────────────────────────────────
// Cell extraction (cell-extract.ts)
// ──────────────────────────────────────────────────────────────────────────────

fn has_vertical_line(verticals: &[LineSegment], x: f64, top_y: f64, bot_y: f64, vertex_radius: f64) -> bool {
    let tol = (VERTEX_MERGE_FACTOR * vertex_radius).max(4.0);
    let cell_h = (top_y - bot_y).abs();
    if cell_h < 0.1 {
        return false;
    }
    for v in verticals {
        if (v.x1 - x).abs() <= tol {
            let overlap = v.y2.min(top_y) - v.y1.max(bot_y);
            if overlap >= cell_h * 0.75 {
                return true;
            }
        }
    }
    false
}
fn has_horizontal_line(horizontals: &[LineSegment], y: f64, left_x: f64, right_x: f64, vertex_radius: f64) -> bool {
    let tol = (VERTEX_MERGE_FACTOR * vertex_radius).max(4.0);
    let cell_w = (right_x - left_x).abs();
    if cell_w < 0.1 {
        return false;
    }
    for h in horizontals {
        if (h.y1 - y).abs() <= tol {
            let overlap = h.x2.min(right_x) - h.x1.max(left_x);
            if overlap >= cell_w * 0.75 {
                return true;
            }
        }
    }
    false
}

/// Slice a grid into merged cells (kkdoc extractCells).
pub fn extract_cells(grid: &TableGrid, horizontals: &[LineSegment], verticals: &[LineSegment]) -> Vec<ExtractedCell> {
    let row_ys = &grid.row_ys;
    let col_xs = &grid.col_xs;
    let num_rows = row_ys.len().saturating_sub(1);
    let num_cols = col_xs.len().saturating_sub(1);
    if num_rows == 0 || num_cols == 0 {
        return Vec::new();
    }
    let vr = grid.vertex_radius;
    // vBorders[r][c] for c in 0..=num_cols
    let mut v_borders = vec![vec![false; num_cols + 1]; num_rows];
    for (r, row) in v_borders.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = has_vertical_line(verticals, col_xs[c], row_ys[r], row_ys[r + 1], vr);
        }
    }
    // hBorders[r][c] for r in 0..=num_rows
    let mut h_borders = vec![vec![false; num_cols]; num_rows + 1];
    for (r, row) in h_borders.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = has_horizontal_line(horizontals, row_ys[r], col_xs[c], col_xs[c + 1], vr);
        }
    }

    let mut occupied = vec![vec![false; num_cols]; num_rows];
    let mut cells = Vec::new();
    for r in 0..num_rows {
        for c in 0..num_cols {
            if occupied[r][c] {
                continue;
            }
            // colSpan
            let mut col_span = 1;
            while c + col_span < num_cols {
                if v_borders[r][c + col_span] {
                    break;
                }
                // require no vertical border across covered rows so far
                cells_no_op();
                col_span += 1;
            }
            // rowSpan
            let mut row_span = 1;
            'outer: while r + row_span < num_rows {
                for dc in 0..col_span {
                    if h_borders[r + row_span][c + dc] {
                        break 'outer;
                    }
                }
                row_span += 1;
            }
            for dr in 0..row_span {
                for dc in 0..col_span {
                    occupied[r + dr][c + dc] = true;
                }
            }
            cells.push(ExtractedCell {
                row: r,
                col: c,
                row_span,
                col_span,
                bbox: BBox {
                    x1: col_xs[c],
                    y1: row_ys[r + row_span],
                    x2: col_xs[c + col_span],
                    y2: row_ys[r],
                },
            });
        }
    }
    cells
}
#[inline]
fn cells_no_op() {}

// ──────────────────────────────────────────────────────────────────────────────
// Text → cell assignment (simplified cellTextToString) and matrix build.
// ──────────────────────────────────────────────────────────────────────────────

/// Assign positioned text to grid cells by containment, join per cell into a
/// dense `rows × cols` string matrix. Merged cells hold their text in the
/// top-left slot; spanned slots stay empty.
fn build_cell_matrix(
    grid: &TableGrid,
    cells: &[ExtractedCell],
    texts: &[PositionedText],
) -> Vec<Vec<String>> {
    let num_rows = grid.row_ys.len().saturating_sub(1);
    let num_cols = grid.col_xs.len().saturating_sub(1);
    let mut matrix = vec![vec![String::new(); num_cols]; num_rows];
    if num_rows == 0 || num_cols == 0 {
        return matrix;
    }
    // For each cell, collect texts whose start point falls within its bbox.
    for cell in cells {
        let bb = cell.bbox;
        let mut inside: Vec<&PositionedText> = texts
            .iter()
            .filter(|t| t.x >= bb.x1 - 1.0 && t.x <= bb.x2 + 1.0 && t.y >= bb.y1 - 1.0 && t.y <= bb.y2 + 1.0)
            .collect();
        if inside.is_empty() {
            continue;
        }
        // Group into visual lines (descending y), join lines top→bottom.
        inside.sort_by(|a, b| {
            b.y.partial_cmp(&a.y)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        });
        let mut lines: Vec<String> = Vec::new();
        let mut cur_line = String::new();
        let mut cur_y = inside[0].y;
        let mut first_fs = inside[0].font_size.unwrap_or(12.0);
        for t in &inside {
            let fs = t.font_size.unwrap_or(12.0);
            let tol = (fs.min(first_fs) * 0.6).max(3.0);
            if (t.y - cur_y).abs() <= tol {
                if !cur_line.is_empty() && !t.text.is_empty() {
                    cur_line.push(' ');
                }
                cur_line.push_str(&t.text);
            } else {
                lines.push(std::mem::take(&mut cur_line));
                cur_line.push_str(&t.text);
                cur_y = t.y;
                first_fs = fs;
            }
        }
        if !cur_line.is_empty() {
            lines.push(cur_line);
        }
        let joined = lines.join(" ").trim().to_string();
        matrix[cell.row][cell.col] = joined;
    }
    matrix
}

// ──────────────────────────────────────────────────────────────────────────────
// Under-segmentation reconstruction (undersegmented.ts)
// ──────────────────────────────────────────────────────────────────────────────

struct RowBand {
    center_y: f64,
    avg_height: f64,
    top_y: f64,
    bottom_y: f64,
    line_count: f64,
    items_by_col: Vec<Vec<usize>>, // indices into `items`
}

fn item_center_y(t: &PositionedText) -> f64 {
    // PositionedText carries no height; use font_size as a proxy for h.
    let h = t.font_size.unwrap_or(12.0);
    t.y + h / 2.0
}
fn item_height(t: &PositionedText) -> f64 {
    t.font_size.unwrap_or(12.0)
}
fn find_column_index(x: f64, col_xs: &[f64]) -> usize {
    // col_xs ascending; num_cols = len-1.
    let n = col_xs.len().saturating_sub(1);
    for c in 0..n {
        if x >= col_xs[c] && x <= col_xs[c + 1] {
            return c;
        }
    }
    // nearest column center
    let mut best = 0;
    let mut best_d = f64::INFINITY;
    for c in 0..n {
        let center = (col_xs[c] + col_xs[c + 1]) / 2.0;
        let d = (x - center).abs();
        if d < best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

/// Group items into visual lines by descending-Y with adaptive tolerance.
fn group_items_to_visual_lines(items: &[PositionedText]) -> Vec<Vec<usize>> {
    let mut idx: Vec<usize> = (0..items.len()).collect();
    idx.sort_by(|&a, &b| {
        items[b]
            .y
            .partial_cmp(&items[a].y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(items[a].x.partial_cmp(&items[b].x).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut lines: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut cur_y = 0.0;
    let mut first_fs = 0.0;
    for &i in &idx {
        let fs = items[i].font_size.unwrap_or(12.0);
        if cur.is_empty() {
            cur.push(i);
            cur_y = items[i].y;
            first_fs = fs;
        } else {
            let tol = (fs.min(first_fs) * 0.6).max(3.0);
            if (items[i].y - cur_y).abs() <= tol {
                cur.push(i);
            } else {
                lines.push(std::mem::take(&mut cur));
                cur.push(i);
                cur_y = items[i].y;
                first_fs = fs;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Reconstruct an under-segmented (merged) table by re-deriving row bands from
/// the raw text lines. Returns a new string matrix only if it strictly
/// improves quality (rows increase, columns don't decrease). kkdoc
/// normalizeUndersegmentedTable.
pub fn normalize_undersegmented(
    original_cells: &[Vec<String>],
    col_xs: &[f64],
    items: &[PositionedText],
) -> Option<Vec<Vec<String>>> {
    let num_rows = original_cells.len();
    let num_cols = col_xs.len().saturating_sub(1);
    if num_rows > US_MAX_ROWS || num_cols < US_MIN_COLS || items.is_empty() {
        return None;
    }
    // Non-empty items only.
    let non_empty: Vec<PositionedText> = items.iter().filter(|t| !t.text.trim().is_empty()).cloned().collect();
    if non_empty.is_empty() {
        return None;
    }
    // Dense-column test.
    let mut per_col: Vec<Vec<usize>> = vec![Vec::new(); num_cols];
    for (i, t) in non_empty.iter().enumerate() {
        let cx = t.x; // start x (no width available)
        let c = find_column_index(cx, col_xs);
        per_col[c].push(i);
    }
    let dense_columns = per_col
        .iter()
        .filter(|col| {
            let sub: Vec<PositionedText> = col.iter().map(|&i| non_empty[i].clone()).collect();
            group_items_to_visual_lines(&sub).len() >= US_MIN_TEXT_LINES
        })
        .count();
    if dense_columns < 2 {
        return None;
    }

    // Re-derive row bands.
    let lines = group_items_to_visual_lines(&non_empty);
    let mut bands: Vec<RowBand> = Vec::new();
    for line in &lines {
        let cy = line.iter().map(|&i| item_center_y(&non_empty[i])).sum::<f64>() / line.len() as f64;
        let h = line.iter().map(|&i| item_height(&non_empty[i])).sum::<f64>() / line.len() as f64;
        let top = cy + h / 2.0;
        let bottom = cy - h / 2.0;
        // Match to an existing band.
        let mut matched: Option<usize> = None;
        for (bi, band) in bands.iter().enumerate() {
            let epsilon = US_MIN_BAND_EPSILON.max(band.avg_height.min(h) * US_BAND_EPSILON_RATIO);
            let overlaps = bottom <= band.top_y && top >= band.bottom_y;
            if (band.center_y - cy).abs() <= epsilon || overlaps {
                matched = Some(bi);
                break;
            }
        }
        let bi = match matched {
            Some(bi) => bi,
            None => {
                bands.push(RowBand {
                    center_y: 0.0,
                    avg_height: 0.0,
                    top_y: f64::NEG_INFINITY,
                    bottom_y: f64::INFINITY,
                    line_count: 0.0,
                    items_by_col: vec![Vec::new(); num_cols],
                });
                bands.len() - 1
            }
        };
        let band = &mut bands[bi];
        let lc = band.line_count;
        band.center_y = (band.center_y * lc + cy) / (lc + 1.0);
        band.avg_height = (band.avg_height * lc + h) / (lc + 1.0);
        band.top_y = band.top_y.max(top);
        band.bottom_y = band.bottom_y.min(bottom);
        band.line_count += 1.0;
        for &i in line {
            let c = find_column_index(non_empty[i].x, col_xs);
            band.items_by_col[c].push(i);
        }
    }

    if bands.len() < num_rows + US_MIN_BAND_MISMATCH {
        return None;
    }
    bands.sort_by(|a, b| b.center_y.partial_cmp(&a.center_y).unwrap_or(std::cmp::Ordering::Equal));

    // Rebuild matrix.
    let mut rebuilt: Vec<Vec<String>> = Vec::with_capacity(bands.len());
    for band in &bands {
        let mut row = vec![String::new(); num_cols];
        for (c, col_items) in band.items_by_col.iter().enumerate() {
            if !col_items.is_empty() {
                let sub: Vec<PositionedText> = col_items.iter().map(|&i| non_empty[i].clone()).collect();
                row[c] = join_cell_lines(&sub);
            }
        }
        rebuilt.push(row);
    }

    // Quality gate: rows must strictly increase, columns must not decrease.
    if count_non_empty_rows(&rebuilt) <= count_non_empty_rows(original_cells) {
        return None;
    }
    if count_non_empty_cols(&rebuilt, num_cols) < count_non_empty_cols(original_cells, num_cols) {
        return None;
    }
    Some(rebuilt)
}

fn join_cell_lines(items: &[PositionedText]) -> String {
    let lines = group_items_to_visual_lines(items);
    let mut parts = Vec::new();
    for line in lines {
        let mut ordered: Vec<usize> = line;
        ordered.sort_by(|&a, &b| items[a].x.partial_cmp(&items[b].x).unwrap_or(std::cmp::Ordering::Equal));
        let joined: String = ordered
            .iter()
            .map(|&i| items[i].text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(joined);
    }
    parts.join(" ").trim().to_string()
}

fn count_non_empty_rows(cells: &[Vec<String>]) -> usize {
    cells.iter().filter(|r| r.iter().any(|c| !c.trim().is_empty())).count()
}
fn count_non_empty_cols(cells: &[Vec<String>], cols: usize) -> usize {
    let mut n = 0;
    for c in 0..cols {
        if cells.iter().any(|r| r.get(c).map(|s| !s.trim().is_empty()).unwrap_or(false)) {
            n += 1;
        }
    }
    n
}

// ──────────────────────────────────────────────────────────────────────────────
// Public detection entry points + PdfTable / IRTable conversion
// ──────────────────────────────────────────────────────────────────────────────

/// Detect line-based (ruled) tables on a single page. Returns rich
/// `DetectedTable`s (both flat `PdfTable` and merged-cell `IRTable`).
pub fn detect_line_tables(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    texts: &[PositionedText],
    page: usize,
) -> Vec<DetectedTable> {
    let (h0, v0) = extract_ruling_lines(doc, page_id);
    let (horizontals, verticals) = preprocess_lines(h0, v0);
    // Synthesize missing outer side borders (open-edge Korean-gov tables) before
    // building grids; conservative — needs interior verticals crossing the rules.
    let verticals = close_open_table_edges(&horizontals, &verticals);
    let grids = build_table_grids(&horizontals, &verticals);

    let mut out = Vec::new();
    for grid in &grids {
        let cells = extract_cells(grid, &horizontals, &verticals);
        if cells.is_empty() {
            continue;
        }
        let mut matrix = build_cell_matrix(grid, &cells, texts);

        // Under-segmentation reconstruction: a ≤2-row, ≥3-col grid whose cells
        // actually hold many stacked text lines is a merged table; rebuild it.
        let num_cols = grid.col_xs.len().saturating_sub(1);
        if matrix.len() <= US_MAX_ROWS && num_cols >= US_MIN_COLS {
            let region_texts: Vec<PositionedText> = texts
                .iter()
                .filter(|t| {
                    t.x >= grid.bbox.x1 - 1.0
                        && t.x <= grid.bbox.x2 + 1.0
                        && t.y >= grid.bbox.y1 - 1.0
                        && t.y <= grid.bbox.y2 + 1.0
                })
                .cloned()
                .collect();
            if let Some(rebuilt) = normalize_undersegmented(&matrix, &grid.col_xs, &region_texts) {
                out.push(matrix_to_detected(rebuilt, page, grid.bbox));
                continue;
            }
        }

        // Drop entirely-empty tables.
        if matrix.iter().all(|r| r.iter().all(|c| c.trim().is_empty())) {
            continue;
        }
        // Prefer the span-aware IR from the extracted cells.
        let ir = cells_to_ir_table(grid, &cells, &mut matrix);
        let pdf = matrix_to_pdf_table(&matrix, page, grid.bbox);
        out.push(DetectedTable { pdf, ir });
    }
    out
}

fn matrix_to_detected(matrix: Vec<Vec<String>>, page: usize, bbox: BBox) -> DetectedTable {
    let pdf = matrix_to_pdf_table(&matrix, page, bbox);
    let ir_cells: Vec<Vec<IRCell>> =
        matrix.iter().map(|r| r.iter().map(IRCell::new).collect()).collect();
    let ir = IRTable::new(ir_cells);
    DetectedTable { pdf, ir }
}

fn matrix_to_pdf_table(matrix: &[Vec<String>], page: usize, bbox: BBox) -> PdfTable {
    let column_count = matrix.iter().map(|r| r.len()).max().unwrap_or(0);
    PdfTable {
        page,
        rows: matrix.to_vec(),
        column_count,
        y_top: bbox.y2,
        y_bottom: bbox.y1,
    }
}

/// Build an `IRTable` carrying colspan/rowspan from the extracted cells.
fn cells_to_ir_table(grid: &TableGrid, cells: &[ExtractedCell], matrix: &mut [Vec<String>]) -> IRTable {
    let num_rows = grid.row_ys.len().saturating_sub(1);
    let num_cols = grid.col_xs.len().saturating_sub(1);
    // Placeholder marks spanned-over slots so we can drop them per row.
    let mut ir_rows: Vec<Vec<IRCell>> = Vec::with_capacity(num_rows);
    // Map (row,col) → (col_span,row_span) for anchor cells.
    let mut anchor: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut covered = vec![vec![false; num_cols]; num_rows];
    for c in cells {
        anchor.insert((c.row, c.col), (c.col_span, c.row_span));
        for dr in 0..c.row_span {
            for dc in 0..c.col_span {
                if dr == 0 && dc == 0 {
                    continue;
                }
                if c.row + dr < num_rows && c.col + dc < num_cols {
                    covered[c.row + dr][c.col + dc] = true;
                }
            }
        }
    }
    for r in 0..num_rows {
        let mut row_cells: Vec<IRCell> = Vec::new();
        for c in 0..num_cols {
            if covered[r][c] {
                continue; // spanned over by an anchor to the left/above
            }
            let (cspan, rspan) = anchor.get(&(r, c)).copied().unwrap_or((1, 1));
            let text = matrix.get(r).and_then(|row| row.get(c)).cloned().unwrap_or_default();
            row_cells.push(IRCell {
                text,
                col_span: cspan as u16,
                row_span: rspan as u16,
            });
        }
        ir_rows.push(row_cells);
    }
    IRTable {
        rows: num_rows,
        cols: num_cols,
        cells: ir_rows,
        has_header: num_rows > 1,
    }
}

/// Merge line-based tables with text-cluster tables: a cluster table is
/// dropped when its vertical span overlaps any line table on the same page
/// (line-based geometry is more precise). Line tables come first.
pub fn merge_line_and_cluster(
    line_tables: Vec<PdfTable>,
    cluster_tables: Vec<PdfTable>,
) -> Vec<PdfTable> {
    let mut out = line_tables;
    for ct in cluster_tables {
        let overlaps = out.iter().any(|lt| {
            lt.page == ct.page
                && ct.y_top >= lt.y_bottom
                && ct.y_bottom <= lt.y_top
        });
        if !overlaps {
            out.push(ct);
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────────────────────
// Ported refinements (wave-2 follow-up):
//   * closeOpenTableEdges — synthesize missing outer side borders for
//     Korean-gov tables that omit them. (`close_open_table_edges`, wired into
//     `detect_line_tables` after preprocessing.)
//   * splitStackedGroup — separate two stacked tables sharing one boundary.
//     (`split_stacked_group`, run per group inside `build_table_grids`.)
//
// Deferred / intentionally not ported:
//   * mergeAdjacentGrids (kkdoc, same-page TableGrid stitch) — dropped: it can't
//     deliver cross-page continuation (its results are per-page here) and risks
//     fusing independent same-page tables. See the note above `build_table_grids`.
//   * Cross-page table continuation (kkdoc `mergeCrossPageTables`, page-blocks.ts)
//     — the real "stitch a table continued across a page break" feature. Deferred:
//     it belongs at the block-aggregation layer and needs signals mdm's flat
//     `Vec<PdfTable>` lacks (x-bbox alignment, reading-order block adjacency) plus
//     a renderer change so the continuation page's inline text stays suppressed
//     (`to_markdown_with_layout` keys table dedup by `t.page == elem.page`).
//   * The full cluster-detector.ts rewrite (header-anchored column model,
//     two-column-prose demotion). The existing `detect_tables_from_positions`
//     in parser.rs serves as the line-less fallback; the line-based path above
//     (now with open-edge synthesis) covers the ruled-table majority.
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn hseg(x1: f64, x2: f64, y: f64) -> LineSegment {
        LineSegment { x1, y1: y, x2, y2: y, line_width: 1.0, from_fill: false }
    }
    fn vseg(y1: f64, y2: f64, x: f64) -> LineSegment {
        LineSegment { x1: x, y1, x2: x, y2, line_width: 1.0, from_fill: false }
    }
    fn txt(s: &str, x: f64, y: f64) -> PositionedText {
        PositionedText { text: s.to_string(), x, y, page: 0, font_size: Some(10.0), font_name: None }
    }

    // A simple 3-col × 3-row grid: x at 0/100/200/300, y at 300/200/100/0.
    fn simple_grid_lines() -> (Vec<LineSegment>, Vec<LineSegment>) {
        let xs = [0.0, 100.0, 200.0, 300.0];
        let ys = [0.0, 100.0, 200.0, 300.0];
        let mut h = Vec::new();
        for &y in &ys {
            h.push(hseg(0.0, 300.0, y));
        }
        let mut v = Vec::new();
        for &x in &xs {
            v.push(vseg(0.0, 300.0, x));
        }
        (h, v)
    }

    #[test]
    fn classify_horizontal_vertical_and_length_filter() {
        let mut h = Vec::new();
        let mut v = Vec::new();
        classify_and_add(&raw_seg(0.0, 10.0, 100.0, 10.5), 1.0, false, &mut h, &mut v);
        classify_and_add(&raw_seg(10.0, 0.0, 10.5, 100.0), 1.0, false, &mut h, &mut v);
        // too short → dropped
        classify_and_add(&raw_seg(0.0, 0.0, 5.0, 0.0), 1.0, false, &mut h, &mut v);
        // diagonal → dropped
        classify_and_add(&raw_seg(0.0, 0.0, 100.0, 100.0), 1.0, false, &mut h, &mut v);
        assert_eq!(h.len(), 1);
        assert_eq!(v.len(), 1);
        assert!((h[0].y1 - 10.25).abs() < 1e-9, "horizontal snapped to averaged y");
    }

    #[test]
    fn thick_lines_are_filtered() {
        let lines = vec![
            LineSegment { x1: 0.0, y1: 0.0, x2: 100.0, y2: 0.0, line_width: 3.0, from_fill: false },
            LineSegment { x1: 0.0, y1: 5.0, x2: 100.0, y2: 5.0, line_width: 6.0, from_fill: false },
        ];
        let kept = thick_filter(lines);
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn merge_parallel_close_lines() {
        let lines = vec![hseg(0.0, 100.0, 10.0), hseg(5.0, 105.0, 11.0)];
        let merged = merge_parallel_lines(lines, true);
        assert_eq!(merged.len(), 1);
        assert!((merged[0].x1 - 0.0).abs() < 1e-9);
        assert!((merged[0].x2 - 105.0).abs() < 1e-9);
        assert!((merged[0].y1 - 10.5).abs() < 1e-9, "perp averaged");
    }

    #[test]
    fn shading_stack_dropped() {
        // 8 identical-span horizontals spaced 1pt apart → a shading stack.
        let mut lines = Vec::new();
        for i in 0..8 {
            lines.push(hseg(0.0, 100.0, 50.0 + i as f64));
        }
        let kept = drop_shading_stacks(lines, true);
        assert!(kept.len() < 8, "dense stack should be dropped, got {}", kept.len());
    }

    #[test]
    fn cluster_coordinates_running_mean() {
        let vals = vec![0.0, 1.0, 2.0, 100.0, 101.0];
        let clusters = cluster_coordinates(&vals, 8.0);
        assert_eq!(clusters.len(), 2);
        assert!((clusters[0] - 1.0).abs() < 1e-9);
        assert!((clusters[1] - 100.5).abs() < 1e-9);
    }

    #[test]
    fn enforce_min_width_merges_narrow() {
        let cols = vec![0.0, 5.0, 100.0, 200.0]; // 0→5 is < MIN_COL_WIDTH
        let out = enforce_min_width(&cols);
        assert_eq!(out, vec![0.0, 100.0, 200.0]);
    }

    #[test]
    fn build_grid_from_simple_lines() {
        let (h, v) = simple_grid_lines();
        let grids = build_table_grids(&h, &v);
        assert_eq!(grids.len(), 1);
        let g = &grids[0];
        assert_eq!(g.col_xs.len(), 4, "4 column boundaries");
        assert_eq!(g.row_ys.len(), 4, "4 row boundaries");
        assert!(g.row_ys.windows(2).all(|w| w[0] > w[1]), "rows descending");
        assert!(g.col_xs.windows(2).all(|w| w[0] < w[1]), "cols ascending");
    }

    #[test]
    fn extract_cells_full_grid_has_9_cells() {
        let (h, v) = simple_grid_lines();
        let grids = build_table_grids(&h, &v);
        let cells = extract_cells(&grids[0], &h, &v);
        assert_eq!(cells.len(), 9, "3x3 fully-ruled grid → 9 unit cells");
        assert!(cells.iter().all(|c| c.row_span == 1 && c.col_span == 1));
    }

    #[test]
    fn extract_cells_merged_when_border_missing() {
        // Same grid but drop the vertical between col1 and col2 on the top row
        // region entirely → top-left cell spans 2 columns.
        let xs = [0.0, 100.0, 200.0];
        let ys = [0.0, 100.0, 200.0];
        let mut h = Vec::new();
        for &y in &ys {
            h.push(hseg(0.0, 200.0, y));
        }
        // verticals: full left (0) and right (200); middle (100) only spans the
        // BOTTOM row (y 0..100), so the top row's middle border is absent.
        let v = vec![
            vseg(0.0, 200.0, 0.0),
            vseg(0.0, 100.0, 100.0),
            vseg(0.0, 200.0, 200.0),
        ];
        let grids = build_table_grids(&h, &v);
        assert_eq!(grids.len(), 1);
        let cells = extract_cells(&grids[0], &h, &v);
        // Top row: one cell spanning 2 cols. Bottom row: two unit cells.
        let top_span = cells.iter().find(|c| c.row == 0 && c.col == 0).unwrap();
        assert_eq!(top_span.col_span, 2, "missing middle border → colspan 2");
    }

    #[test]
    fn cell_matrix_assigns_text_by_containment() {
        let (h, v) = simple_grid_lines();
        let grids = build_table_grids(&h, &v);
        let cells = extract_cells(&grids[0], &h, &v);
        // Put text in the middle cell (row 1, col 1): x in (100,200), y in (100,200).
        let texts = vec![txt("hello", 120.0, 150.0), txt("A", 20.0, 250.0)];
        let matrix = build_cell_matrix(&grids[0], &cells, &texts);
        assert_eq!(matrix[1][1], "hello");
        assert_eq!(matrix[0][0], "A");
    }

    #[test]
    fn ir_table_carries_colspan() {
        let xs = [0.0, 100.0, 200.0];
        let ys = [0.0, 100.0, 200.0];
        let mut h = Vec::new();
        for &y in &ys {
            h.push(hseg(0.0, 200.0, y));
        }
        let v = vec![vseg(0.0, 200.0, 0.0), vseg(0.0, 100.0, 100.0), vseg(0.0, 200.0, 200.0)];
        let grids = build_table_grids(&h, &v);
        let cells = extract_cells(&grids[0], &h, &v);
        let mut matrix = vec![vec![String::new(); 2]; 2];
        matrix[0][0] = "spanning".into();
        let ir = cells_to_ir_table(&grids[0], &cells, &mut matrix);
        // Top row should have a single cell with col_span 2.
        assert_eq!(ir.cells[0].len(), 1);
        assert_eq!(ir.cells[0][0].col_span, 2);
        assert_eq!(ir.cells[1].len(), 2, "bottom row keeps two unit cells");
    }

    #[test]
    fn undersegmented_rebuild_splits_stacked_rows() {
        // A 1-row × 3-col original where each column really holds 8 stacked
        // lines → should rebuild into 8 rows.
        let original = vec![vec!["merged".to_string(), "merged".to_string(), "merged".to_string()]];
        let col_xs = vec![0.0, 100.0, 200.0, 300.0];
        let mut items = Vec::new();
        for line in 0..8 {
            let y = 200.0 - line as f64 * 20.0;
            items.push(txt(&format!("a{}", line), 10.0, y));
            items.push(txt(&format!("b{}", line), 110.0, y));
            items.push(txt(&format!("c{}", line), 210.0, y));
        }
        let rebuilt = normalize_undersegmented(&original, &col_xs, &items)
            .expect("should reconstruct under-segmented table");
        assert!(rebuilt.len() >= 8, "expected >=8 rebuilt rows, got {}", rebuilt.len());
        assert_eq!(rebuilt[0][0], "a0");
        assert_eq!(rebuilt[0][2], "c0");
    }

    #[test]
    fn merge_prefers_line_over_overlapping_cluster() {
        let line = PdfTable { page: 1, rows: vec![vec!["L".into()]], column_count: 1, y_top: 200.0, y_bottom: 100.0 };
        let overlap = PdfTable { page: 1, rows: vec![vec!["C".into()]], column_count: 1, y_top: 150.0, y_bottom: 120.0 };
        let disjoint = PdfTable { page: 1, rows: vec![vec!["D".into()]], column_count: 1, y_top: 90.0, y_bottom: 50.0 };
        let out = merge_line_and_cluster(vec![line], vec![overlap, disjoint]);
        assert_eq!(out.len(), 2, "overlapping cluster dropped, disjoint kept");
        assert_eq!(out[0].rows[0][0], "L");
        assert_eq!(out[1].rows[0][0], "D");
    }

    // ── closeOpenTableEdges ──────────────────────────────────────────────────

    #[test]
    fn close_open_edges_synthesizes_both_side_borders() {
        // 3 full-width rules + one interior vertical crossing all of them, no
        // left/right outer borders → synthesize both side borders.
        let horizontals = vec![hseg(0.0, 200.0, 0.0), hseg(0.0, 200.0, 50.0), hseg(0.0, 200.0, 100.0)];
        let verticals = vec![vseg(0.0, 100.0, 100.0)];
        let out = close_open_table_edges(&horizontals, &verticals);
        assert_eq!(out.len(), 3, "two side borders synthesized alongside the interior");
        assert!(out.iter().any(|v| (v.x1 - 0.0).abs() < 1e-9), "left border synthesized");
        assert!(out.iter().any(|v| (v.x1 - 200.0).abs() < 1e-9), "right border synthesized");
        // The interior separator is preserved.
        assert!(out.iter().any(|v| (v.x1 - 100.0).abs() < 1e-9));
    }

    #[test]
    fn close_open_edges_only_fills_missing_side() {
        // Left border already present at x=0 → only the right side is synthesized.
        let horizontals = vec![hseg(0.0, 200.0, 0.0), hseg(0.0, 200.0, 50.0), hseg(0.0, 200.0, 100.0)];
        let verticals = vec![vseg(0.0, 100.0, 0.0), vseg(0.0, 100.0, 100.0)];
        let out = close_open_table_edges(&horizontals, &verticals);
        assert_eq!(out.len(), 3, "only the missing right border is added");
        assert_eq!(out.iter().filter(|v| (v.x1 - 200.0).abs() < 1e-9).count(), 1);
    }

    #[test]
    fn close_open_edges_no_synthesis_without_interior_vertical() {
        // No interior vertical → nothing to anchor a table geometry → no-op.
        let horizontals = vec![hseg(0.0, 200.0, 0.0), hseg(0.0, 200.0, 50.0), hseg(0.0, 200.0, 100.0)];
        let out = close_open_table_edges(&horizontals, &[]);
        assert!(out.is_empty(), "no interior vertical ⇒ no phantom borders");
    }

    // ── splitStackedGroup (via build_table_grids) ────────────────────────────

    #[test]
    fn stacked_tables_sharing_a_boundary_split_into_two_grids() {
        // A narrow top strip stacked on a wider bottom table, sharing the
        // full-width boundary line at y=100. Outer borders sit at different x's
        // (no chaining) and interior columns don't align (x=70/120 vs x=100),
        // so the boundary is a valid cut → two grids from one connected group.
        let mut h = Vec::new();
        // top strip (narrow, x 30..170)
        h.push(hseg(30.0, 170.0, 200.0));
        h.push(hseg(30.0, 170.0, 150.0));
        h.push(hseg(30.0, 170.0, 100.0));
        // bottom table (wide, x 0..200) — its top edge (y=100) is the shared cut
        h.push(hseg(0.0, 200.0, 100.0));
        h.push(hseg(0.0, 200.0, 50.0));
        h.push(hseg(0.0, 200.0, 0.0));
        let mut v = Vec::new();
        // top strip verticals (y 100..200): outer 30/170 + interior 70/120
        v.push(vseg(100.0, 200.0, 30.0));
        v.push(vseg(100.0, 200.0, 170.0));
        v.push(vseg(100.0, 200.0, 70.0));
        v.push(vseg(100.0, 200.0, 120.0));
        // bottom verticals (y 0..100): outer 0/200 + interior 100
        v.push(vseg(0.0, 100.0, 0.0));
        v.push(vseg(0.0, 100.0, 200.0));
        v.push(vseg(0.0, 100.0, 100.0));

        // All lines are one connected component (strip verticals touch the wide
        // boundary line), so two grids can only arise from the stacked split.
        let groups = group_connected_lines(&h, &v);
        assert_eq!(groups.len(), 1, "single connected group before splitting");

        let grids = build_table_grids(&h, &v);
        assert_eq!(grids.len(), 2, "stacked group split into two grids");
        // Bands carry different column counts (strip 3 cols, bottom 2 cols).
        let mut ncols: Vec<usize> = grids.iter().map(|g| g.col_xs.len()).collect();
        ncols.sort_unstable();
        assert_eq!(ncols, vec![3, 4], "strip 4 col-boundaries, bottom 3");
    }

    #[test]
    fn single_table_is_not_split() {
        // The plain 3×3 grid must stay a single grid (no spurious cut).
        let (h, v) = simple_grid_lines();
        assert_eq!(build_table_grids(&h, &v).len(), 1, "no false split on a plain grid");
    }
}
