// Ported from kkdoc (MIT): src/render/reflow.ts
//! Tier-2 reflow — 조판 캐시(linesegarray)가 없는 문단에 좌표를 합성한다.
//!
//! 원본 TS는 DOM에 `<hp:linesegarray>`를 append 하지만, Rust 포트는 DOM(roxmltree)이
//! 읽기 전용이라 합성 줄을 `synthetic` 사이드 테이블(NodeId → Vec<Seg>)에 담는다.
//! buildPara/prepass/measure 는 read_para_segs 로 이 테이블을 함께 참조한다.

use super::dom::{elements, find_child_local, ln, num};
use super::metrics::{face_class_of, simulate_wrap, MeasureOptions, WrapMode};
use super::styles::{default_char, default_para_geom, RenderParaGeom, RenderStyles};
use super::svg::{build_para, measure_table_height, ExtentMemo, Seg, Synthetic};
use roxmltree::Node;

/// baseline / textheight 비율 (실측 94/94 일치)
const BASELINE_RATIO: f64 = 0.85;

/// 줄 pitch(다음 줄 vertpos 증분, HWPUNIT) — lineSpacing type별
fn pitch_for(height: f64, geom: &RenderParaGeom) -> f64 {
    let v = geom.line_spacing_value;
    match geom.line_spacing_type.as_str() {
        "PERCENT" => (height * v / 100.0).round(),
        "FIXED" => {
            if v > 0.0 {
                v
            } else {
                (height * 1.6).round()
            }
        }
        "AT_LEAST" => v.max(height),
        _ => (height * 1.6).round(),
    }
}

/// 캐시 보유 문단의 DOM linesegarray 바닥 (vertpos + max(vertsize,textheight) + spacing)
fn cached_para_bottom(p: Node) -> Option<f64> {
    let lsa = elements(p).find(|e| ln(e) == "linesegarray")?;
    let mut bottom = f64::NEG_INFINITY;
    for seg in elements(lsa) {
        if ln(&seg) != "lineseg" {
            continue;
        }
        let th = num(Some(seg), "vertsize", 1000.0).max(num(Some(seg), "textheight", 1000.0));
        bottom = bottom.max(num(Some(seg), "vertpos", 0.0) + th + num(Some(seg), "spacing", 0.0));
    }
    if bottom.is_finite() {
        Some(bottom)
    } else {
        None
    }
}

struct ParaFlow {
    para_bottom: f64,
    space_after: f64,
}

/// 문단 하나의 합성 linesegarray를 계산해 synthetic에 삽입.
/// 반환: 세로 흐름 갱신값 (캐시 있어 건너뛴 경우 None)
fn reflow_para<'a, 'i>(
    p: Node<'a, 'i>,
    styles: &RenderStyles,
    area_w: f64,
    start_v: f64,
    mode: WrapMode,
    synthetic: &mut Synthetic,
) -> Option<ParaFlow> {
    let m = build_para(p, &*synthetic);
    if !m.segs.is_empty() {
        return None; // 이미 캐시 있음 — Tier-1 무회귀
    }

    // 실텍스트 + char 인덱스 → chars 슬롯 매핑
    let mut real_idx: Vec<usize> = Vec::new();
    let mut text = String::new();
    for (i, c) in m.chars.iter().enumerate() {
        if let Some(ch) = c.ch {
            real_idx.push(i);
            text.push(ch);
        }
    }

    let geom = m.para_pr_id.and_then(|id| styles.para_geom.get(id)).cloned().unwrap_or_else(default_para_geom);

    // 문단 지배 charPr
    let mut dom_char = default_char();
    let mut found = false;
    for c in &m.chars {
        if c.ch.is_some() {
            if let Some(id) = c.pr_id {
                if let Some(st) = styles.char_pr.get(id) {
                    dom_char = st.clone();
                    found = true;
                    break;
                }
            }
        }
    }
    if !found {
        for run in elements(p) {
            if ln(&run) != "run" {
                continue;
            }
            if let Some(st) = run.attribute("charPrIDRef").and_then(|id| styles.char_pr.get(id)) {
                dom_char = st.clone();
                break;
            }
        }
    }
    let height = if dom_char.height != 0.0 { dom_char.height } else { 1000.0 };
    let ratio = if dom_char.ratio != 0.0 { dom_char.ratio } else { 100.0 };
    let spacing_pct = dom_char.spacing;

    let margin_l = geom.margin_left;
    let avail = (area_w - margin_l - geom.margin_right).max(1000.0);
    let first_width = avail;
    let cont_width = (avail + geom.margin_intent.min(0.0)).max(500.0);
    let cont_horz = margin_l - geom.margin_intent.min(0.0);

    let para_mode = geom.wrap_mode.unwrap_or(mode);
    let face_class = face_class_of(dom_char.face.as_deref());
    let opts = MeasureOptions { spacing_pct, face_class, ..Default::default() };
    let wrap = if text.is_empty() {
        (vec![0usize], 1usize)
    } else {
        let w = simulate_wrap(&text, first_width, cont_width, height, ratio, para_mode, &opts);
        (w.starts, w.lines)
    };
    let starts = wrap.0;

    let pitch = pitch_for(height, &geom);
    let baseline = (height * BASELINE_RATIO).round();
    let spacing = (pitch - height).max(0.0);

    // 개체 세로 흐름 분류
    let mut memo = ExtentMemo::default();
    let mut float_below = 0.0_f64;
    let mut obj_bottom = start_v;
    for o in &m.objs {
        let eff_h = if o.tag == "tbl" {
            o.height.max(measure_table_height(o.el, styles, &*synthetic, &mut memo))
        } else {
            o.height
        };
        let pos = find_child_local(o.el, "pos");
        let om = find_child_local(o.el, "outMargin");
        let out_t = num(om, "top", 0.0);
        let out_b = num(om, "bottom", 0.0);
        if o.inline {
            obj_bottom = obj_bottom.max(start_v + out_t + eff_h + out_b + spacing);
            continue;
        }
        let wrap_attr = o.el.attribute("textWrap").unwrap_or("");
        if wrap_attr == "BEHIND_TEXT" || wrap_attr == "IN_FRONT_OF_TEXT" {
            continue;
        }
        let vert_rel = pos.and_then(|p| p.attribute("vertRelTo")).unwrap_or("PARA");
        if vert_rel == "PAGE" || vert_rel == "PAPER" {
            continue;
        }
        let vo = num(pos, "vertOffset", 0.0).max(0.0);
        if wrap_attr == "TOP_AND_BOTTOM" {
            float_below = float_below.max(vo + out_t + eff_h + out_b);
        } else {
            obj_bottom = obj_bottom.max(start_v + vo + out_t + eff_h + out_b);
        }
    }
    let text_start_v = start_v + float_below;

    let mut segs: Vec<Seg> = Vec::with_capacity(starts.len());
    for (li, &start_real) in starts.iter().enumerate() {
        let textpos = if start_real < real_idx.len() { real_idx[start_real] as f64 } else { 0.0 };
        let vertpos = text_start_v + (li as f64) * pitch;
        let is_first = li == 0;
        segs.push(Seg {
            textpos,
            vertpos,
            horzpos: if is_first { margin_l } else { cont_horz },
            horzsize: if is_first { first_width } else { cont_width },
            textheight: height,
            baseline,
        });
    }
    synthetic.insert(p.id(), segs);

    let text_bottom = text_start_v + (starts.len() as f64) * pitch;
    Some(ParaFlow { para_bottom: text_bottom.max(obj_bottom), space_after: geom.space_after })
}

/// 문단 run 안의 표를 찾아 각 셀 subList를 셀 로컬로 reflow (중첩 재귀)
fn reflow_tables_in<'a, 'i>(p: Node<'a, 'i>, styles: &RenderStyles, mode: WrapMode, synthetic: &mut Synthetic) {
    for run in elements(p) {
        if ln(&run) != "run" {
            continue;
        }
        for obj in elements(run) {
            if ln(&obj) != "tbl" {
                continue;
            }
            for tr in elements(obj) {
                if ln(&tr) != "tr" {
                    continue;
                }
                for tc in elements(tr) {
                    if ln(&tc) != "tc" {
                        continue;
                    }
                    let csz = find_child_local(tc, "cellSz");
                    let cm = find_child_local(tc, "cellMargin");
                    let cell_w = num(csz, "width", 0.0);
                    let m_l = if cm.is_some() { num(cm, "left", 141.0) } else { 141.0 };
                    let m_r = if cm.is_some() { num(cm, "right", 141.0) } else { 141.0 };
                    let area_w = (cell_w - m_l - m_r).max(500.0);
                    if let Some(sub) = find_child_local(tc, "subList") {
                        reflow_block_flow(sub, styles, area_w, mode, 0.0, synthetic);
                    }
                }
            }
        }
    }
}

/// 문단의 합성 segs vertpos를 delta만큼 이동 (페이지 로컬 리셋용)
fn shift_para_vert(p: Node, delta: f64, synthetic: &mut Synthetic) {
    if let Some(segs) = synthetic.get_mut(&p.id()) {
        for s in segs.iter_mut() {
            s.vertpos += delta;
        }
    }
}

/// 한 블록 컨테이너 안 문단들을 세로 흐름으로 reflow.
fn reflow_block_flow<'a, 'i>(
    container: Node<'a, 'i>,
    styles: &RenderStyles,
    area_w: f64,
    mode: WrapMode,
    body_h: f64,
    synthetic: &mut Synthetic,
) {
    let mut cursor_v = 0.0_f64;
    let mut prev_space_after = 0.0_f64;
    for p in elements(container) {
        if ln(&p) != "p" {
            continue;
        }
        // 문단 안 표 셀을 먼저 셀 로컬 좌표로 reflow
        reflow_tables_in(p, styles, mode, synthetic);
        let g = p.attribute("paraPrIDRef").and_then(|id| styles.para_geom.get(id));
        let space_before = g.map(|g| g.space_before).unwrap_or(0.0);
        let space_after_cached = g.map(|g| g.space_after).unwrap_or(0.0);
        let start_v = cursor_v + prev_space_after + space_before;
        match reflow_para(p, styles, area_w, start_v, mode, synthetic) {
            None => {
                // 캐시 보유 문단(Tier-1) — 한컴 실좌표로 커서 전진
                if let Some(bottom) = cached_para_bottom(p) {
                    cursor_v = bottom;
                    prev_space_after = space_after_cached;
                }
            }
            Some(res) => {
                let para_h = res.para_bottom - start_v;
                if body_h > 0.0 && start_v > 0.0 && res.para_bottom > body_h && para_h <= body_h {
                    shift_para_vert(p, -start_v, synthetic);
                    cursor_v = para_h;
                } else {
                    cursor_v = res.para_bottom;
                }
                prev_space_after = res.space_after;
            }
        }
    }
}

/// section root의 조판 캐시 없는 문단에 합성 조판을 계산해 사이드 테이블로 반환.
pub fn reflow_section<'a, 'i>(root: Node<'a, 'i>, styles: &RenderStyles, body_w: f64, body_h: f64, mode: WrapMode) -> Synthetic {
    let mut synthetic: Synthetic = Synthetic::new();
    reflow_block_flow(root, styles, body_w, mode, body_h, &mut synthetic);
    synthetic
}
