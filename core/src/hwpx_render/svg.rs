// Ported from kkdoc (MIT): src/render/svg-render.ts
//! 레이아웃 보존 렌더 — HWPX 조판 캐시(lineseg·cellAddr·hp:pos)를 SVG 절대배치로 그린다.
//!
//! 조판 엔진 없음: 한컴이 저장 시 기록한 좌표를 그대로 사용한다. 캐시가 없는 문단은
//! reflow(합성 linesegarray)로 좌표를 만들어 넣는다(reflow.rs). 원본 TS와 달리 DOM은
//! 읽기 전용(roxmltree)이라, 합성 조판 캐시는 DOM에 주입하지 않고 `synthetic`
//! 사이드 테이블(NodeId → Vec<Seg>)에 담아 buildPara/prepass가 함께 참조한다.

use super::dom::{elements, escape_xml, find_child_local, find_first, ln, num};
use super::layout::{solve_boundaries, solve_row_heights, RowCell, SpanConstraint};
use super::metrics::{measure_text_width, MeasureOptions};
use super::styles::{default_char, ParaAlign, RenderBorderEdge, RenderCharStyle, RenderStyles};
use roxmltree::{Node, NodeId};
use std::collections::{HashMap, HashSet};

// ─── 좌표/포맷 ─────────────────────────────────────

/// HWPUNIT → pt 문자열 (round(u)/100, JS Number→String 최소 표기)
pub fn pt(u: f64) -> String {
    let n = u.round() as i64;
    let neg = n < 0;
    let a = n.abs();
    let whole = a / 100;
    let frac = a % 100;
    let sign = if neg { "-" } else { "" };
    if frac == 0 {
        format!("{}{}", sign, whole)
    } else if frac % 10 == 0 {
        format!("{}{}.{}", sign, whole, frac / 10)
    } else {
        format!("{}{}.{:02}", sign, whole, frac)
    }
}

// ─── 모델 ──────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Seg {
    pub textpos: f64,
    pub vertpos: f64,
    pub horzpos: f64,
    pub horzsize: f64,
    pub textheight: f64,
    pub baseline: f64,
}

pub struct ParaChar<'a> {
    /// None = 필러(컨트롤 자리·서로게이트 둘째 유닛), Some(c) = 실문자/공백
    pub ch: Option<char>,
    pub pr_id: Option<&'a str>,
}

pub struct ParaObj<'a, 'i> {
    pub el: Node<'a, 'i>,
    pub tag: String,
    pub index: usize,
    pub inline: bool,
    pub width: f64,
    pub height: f64,
}

pub struct ParaModel<'a, 'i> {
    pub chars: Vec<ParaChar<'a>>,
    pub segs: Vec<Seg>,
    pub objs: Vec<ParaObj<'a, 'i>>,
    pub para_pr_id: Option<&'a str>,
}

/// 렌더 대상 개체 태그 — 이 외(도형류)는 경고 후 생략
fn is_obj_tag(t: &str) -> bool {
    matches!(
        t,
        "tbl" | "pic" | "container" | "equation" | "rect" | "ellipse" | "polygon" | "curv" | "line" | "arc" | "ole" | "textart"
    )
}

fn is_shape_tag(t: &str) -> bool {
    matches!(t, "rect" | "ellipse" | "line" | "polygon" | "curv" | "arc")
}

/// hp:t 안에서 1슬롯을 차지하는 문자형 컨트롤 (탭 제외 — 탭은 8슬롯)
fn is_char_ctrl_1slot(t: &str) -> bool {
    matches!(t, "lineBreak" | "hyphen" | "nbSpace" | "fwSpace")
}

// ─── 조판 캐시 사이드 테이블 ─────────────────────────

pub type Synthetic = HashMap<NodeId, Vec<Seg>>;

/// 표 좌표/차원 상한 — colAddr/rowAddr/colSpan/rowSpan 값과 파생 n_cols/n_rows 에
/// 적용해 손상·악성 입력이 거대 벡터 할당(OOM)을 유발하지 못하게 한다. 실문서
/// 표는 이 한계 근처에도 오지 않는다(수천 열/행은 병리적 입력).
const MAX_TABLE_DIM: usize = 4096;

/// 문단의 조판 줄들 — DOM linesegarray 우선, 없으면 synthetic 사이드 테이블
pub fn read_para_segs<'a, 'i>(p: Node<'a, 'i>, synthetic: &Synthetic) -> Vec<Seg> {
    for run_el in elements(p) {
        if ln(&run_el) == "linesegarray" {
            return elements(run_el)
                .filter(|s| ln(s) == "lineseg")
                .map(|s| Seg {
                    textpos: num(Some(s), "textpos", 0.0),
                    vertpos: num(Some(s), "vertpos", 0.0),
                    horzpos: num(Some(s), "horzpos", 0.0),
                    horzsize: num(Some(s), "horzsize", 0.0),
                    textheight: num(Some(s), "textheight", 1000.0),
                    baseline: num(Some(s), "baseline", 850.0),
                })
                .collect();
        }
    }
    synthetic.get(&p.id()).cloned().unwrap_or_default()
}

// ─── 문단 모델 구축 ────────────────────────────────

fn push_text_slots<'a, 'i>(t: Node<'a, 'i>, chars: &mut Vec<ParaChar<'a>>, pr_id: Option<&'a str>, depth: u32) {
    if depth > 32 {
        return;
    }
    for c in t.children() {
        if c.is_text() {
            if let Some(txt) = c.text() {
                for cp in txt.chars() {
                    chars.push(ParaChar { ch: Some(cp), pr_id });
                    if (cp as u32) > 0xFFFF {
                        chars.push(ParaChar { ch: None, pr_id }); // UTF-16 둘째 유닛 슬롯
                    }
                }
            }
        } else if c.is_element() {
            let tag = ln(&c);
            if tag == "tab" {
                for _ in 0..8 {
                    chars.push(ParaChar { ch: None, pr_id }); // inline 컨트롤 8슬롯
                }
            } else if is_char_ctrl_1slot(tag) {
                let ch = if tag == "nbSpace" || tag == "fwSpace" { Some(' ') } else { None };
                chars.push(ParaChar { ch, pr_id });
            } else {
                push_text_slots(c, chars, pr_id, depth + 1);
            }
        }
    }
}

pub fn build_para<'a, 'i>(p: Node<'a, 'i>, synthetic: &Synthetic) -> ParaModel<'a, 'i> {
    let mut chars: Vec<ParaChar> = Vec::new();
    let mut objs: Vec<ParaObj> = Vec::new();
    for run_el in elements(p) {
        let tag = ln(&run_el);
        if tag == "run" {
            let pr_id = run_el.attribute("charPrIDRef");
            for ch in elements(run_el) {
                let cn = ln(&ch);
                if cn == "t" {
                    push_text_slots(ch, &mut chars, pr_id, 0);
                } else if is_obj_tag(cn) {
                    let sz = find_child_local(ch, "sz");
                    let pos = find_child_local(ch, "pos");
                    let w = {
                        let a = num(sz, "width", 0.0);
                        if a != 0.0 {
                            a
                        } else {
                            let b = num(find_child_local(ch, "curSz"), "width", 0.0);
                            if b != 0.0 {
                                b
                            } else {
                                num(find_child_local(ch, "orgSz"), "width", 0.0)
                            }
                        }
                    };
                    let h = {
                        let a = num(sz, "height", 0.0);
                        if a != 0.0 {
                            a
                        } else {
                            let b = num(find_child_local(ch, "curSz"), "height", 0.0);
                            if b != 0.0 {
                                b
                            } else {
                                num(find_child_local(ch, "orgSz"), "height", 0.0)
                            }
                        }
                    };
                    objs.push(ParaObj {
                        el: ch,
                        tag: cn.to_string(),
                        index: chars.len(),
                        inline: pos.and_then(|p| p.attribute("treatAsChar")) == Some("1"),
                        width: w,
                        height: h,
                    });
                    for _ in 0..8 {
                        chars.push(ParaChar { ch: None, pr_id }); // 확장 컨트롤 8슬롯
                    }
                } else {
                    for _ in 0..8 {
                        chars.push(ParaChar { ch: None, pr_id }); // 기타 run 자식 8슬롯
                    }
                }
            }
        }
    }
    let segs = read_para_segs(p, synthetic);
    ParaModel { chars, segs, objs, para_pr_id: p.attribute("paraPrIDRef") }
}

// ─── 줄 계획 ───────────────────────────────────────

struct LinePlan {
    seg: Seg,
    xoff: f64,
    scale: f64,
    start: usize,
    end: usize,
}

fn char_w(c: &ParaChar, styles: &RenderStyles) -> f64 {
    let ch = match c.ch {
        Some(c) => c,
        None => return 0.0,
    };
    let st = c.pr_id.and_then(|id| styles.char_pr.get(id));
    let (height, ratio, spacing) = match st {
        Some(s) => (s.height, s.ratio, s.spacing),
        None => (1000.0, 100.0, 0.0),
    };
    let mut buf = [0u8; 4];
    measure_text_width(ch.encode_utf8(&mut buf), height, ratio, &MeasureOptions { spacing_pct: spacing, ..Default::default() })
}

/// 줄 자연폭 = 텍스트 조각 + 인라인 개체 폭
fn line_natural_width(m: &ParaModel, styles: &RenderStyles, start: usize, end: usize) -> (f64, f64) {
    let mut text = 0.0;
    let mut i = start;
    while i < end && i < m.chars.len() {
        text += char_w(&m.chars[i], styles);
        i += 1;
    }
    let mut obj = 0.0;
    for o in &m.objs {
        if o.inline && o.index >= start && o.index < end {
            obj += o.width;
        }
    }
    (text, obj)
}

fn plan_lines(m: &ParaModel, styles: &RenderStyles) -> Vec<LinePlan> {
    let align = m.para_pr_id.and_then(|id| styles.para_align.get(id)).copied().unwrap_or(ParaAlign::Justify);
    let mut plans = Vec::new();
    for i in 0..m.segs.len() {
        let seg = m.segs[i];
        let start = seg.textpos as usize;
        let end = if i + 1 < m.segs.len() {
            m.segs[i + 1].textpos as usize
        } else {
            m.chars.len().max(start)
        };
        let (nat_text, nat_obj) = line_natural_width(m, styles, start, end);
        let is_last = i == m.segs.len() - 1;
        let mut xoff = 0.0;
        let mut scale = 1.0;
        let avail = seg.horzsize - nat_obj;
        if nat_text > 0.0
            && (!is_last || align == ParaAlign::Distribute || align == ParaAlign::DistributeSpace)
        {
            scale = if avail > 0.0 { avail / nat_text } else { 1.0 };
        } else if nat_text + nat_obj > 0.0 && is_last {
            let w = nat_text + nat_obj;
            if align == ParaAlign::Center {
                xoff = ((seg.horzsize - w) / 2.0).max(0.0);
            } else if align == ParaAlign::Right {
                xoff = (seg.horzsize - w).max(0.0);
            }
        }
        if !scale.is_finite() || scale <= 0.0 {
            scale = 1.0;
        }
        scale = scale.clamp(0.25, 4.0);
        plans.push(LinePlan { seg, xoff, scale, start, end });
    }
    plans
}

/// 줄 안 [start, upto) 구간의 전진폭
fn advance_to(m: &ParaModel, styles: &RenderStyles, plan: &LinePlan, upto: usize) -> f64 {
    let mut x = 0.0;
    let mut i = plan.start;
    while i < upto && i < m.chars.len() {
        x += char_w(&m.chars[i], styles) * plan.scale;
        i += 1;
    }
    for o in &m.objs {
        if o.inline && o.index >= plan.start && o.index < upto {
            x += o.width;
        }
    }
    x
}

// ─── 렌더 컨텍스트 ─────────────────────────────────

pub struct LoadedImage {
    pub data_uri: String,
    pub sym_id: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct PageGeom {
    pub pw: f64,
    pub ph: f64,
    pub ml: f64,
    pub mt: f64,
    pub body_w: f64,
    pub body_h: f64,
}

#[derive(Default)]
pub struct ExtentMemo {
    pub cell: HashMap<NodeId, f64>,
    pub table: HashMap<NodeId, f64>,
}

#[derive(Default, Clone, Copy)]
pub struct RenderStats {
    pub texts: usize,
    pub images: usize,
    pub tables: usize,
}

/// 렌더 컨텍스트. Document(노드) 수명과 무관 — 노드 참조를 저장하지 않고
/// synthetic/extent_memo 는 NodeId(Copy)로 키잉한다. 'styles 만 대여.
pub struct Ctx<'s> {
    pub pages: Vec<Vec<String>>,
    pub page: usize,
    pub geom: PageGeom,
    pub styles: &'s RenderStyles,
    pub images: HashMap<String, LoadedImage>,
    pub defs: Vec<String>,
    pub highlights: Vec<String>,
    pub warnings: Vec<String>,
    pub warned: HashSet<String>,
    pub stats: RenderStats,
    pub extent_memo: ExtentMemo,
    pub synthetic: Synthetic,
}

fn emit(ctx: &mut Ctx, s: String) {
    let page = ctx.page;
    ctx.pages[page].push(s);
}

fn warn_once(ctx: &mut Ctx, key: &str, msg: &str) {
    if ctx.warned.contains(key) {
        return;
    }
    ctx.warned.insert(key.to_string());
    ctx.warnings.push(msg.to_string());
}

// ─── 문단 드로잉 ───────────────────────────────────

fn draw_para<'a, 'i>(p: Node<'a, 'i>, ox: f64, oy: f64, area_w: f64, ctx: &mut Ctx<'_>, depth: u32, seg_pages: Option<&[usize]>) {
    if depth > 16 {
        warn_once(ctx, "depth", "중첩 깊이 16 초과 — 이하 생략");
        return;
    }
    let m = build_para(p, &ctx.synthetic);
    if m.segs.is_empty() {
        if m.chars.iter().any(|c| c.ch.is_some()) {
            warn_once(ctx, "no-lineseg", "조판 캐시 없는 문단 텍스트 생략 — reflow 옵션으로 합성 가능");
        }
        for o in &m.objs {
            draw_object(o, ox, oy, ctx, depth);
        }
        return;
    }
    let plans = plan_lines(&m, ctx.styles);
    let base_v = m.segs[0].vertpos;

    for (li, plan) in plans.iter().enumerate() {
        if let Some(sp) = seg_pages {
            if let Some(&pg) = sp.get(li) {
                ctx.page = pg;
            }
        }
        let seg = plan.seg;
        let mut i = plan.start;
        let mut cursor = ox + seg.horzpos + plan.xoff;
        let y = oy + seg.vertpos + seg.baseline;
        while i < plan.end && i < m.chars.len() {
            // 필러 슬롯은 그리지 않고 건너뛰되, 인라인 개체 폭 전진은 개체 첫 슬롯에서 1회
            if m.chars[i].ch.is_none() {
                for o in &m.objs {
                    if o.inline && o.index == i {
                        cursor += o.width;
                    }
                }
                i += 1;
                continue;
            }
            let pr_id = m.chars[i].pr_id;
            // 같은 prId 실문자 런 수집
            let mut piece: Vec<char> = Vec::new();
            let mut j = i;
            while j < plan.end && j < m.chars.len() && m.chars[j].pr_id == pr_id {
                match m.chars[j].ch {
                    Some(c) => {
                        piece.push(c);
                        j += 1;
                    }
                    None => break,
                }
            }
            // 연속 공백(2+) 경계 절단
            {
                let dbl = find_double_space(&piece);
                if let Some(cut) = dbl {
                    if cut > 0 {
                        piece.truncate(cut);
                        j = i + cut;
                    } else {
                        // cut == 0: 선행 공백 런
                        let run_end = piece.iter().take_while(|&&c| c == ' ').count();
                        piece.truncate(run_end);
                        j = i + run_end;
                    }
                }
            }
            let st = pr_id.and_then(|id| ctx.styles.char_pr.get(id)).cloned().unwrap_or_else(default_char);
            let piece_str: String = piece.iter().collect();

            // 형광펜 매치 병합
            let merged = if !ctx.highlights.is_empty() && !piece_str.trim().is_empty() {
                collect_highlights(&piece, &ctx.highlights)
            } else {
                Vec::new()
            };

            let seg_top = oy + seg.vertpos;
            if merged.is_empty() {
                cursor += render_seg(ctx, &piece_str, cursor, false, &st, plan.scale, seg_top, seg.textheight, y);
            } else {
                let mut seg_cur = cursor;
                let mut last = 0usize;
                for (s, e) in merged {
                    let plain: String = piece[last..s].iter().collect();
                    seg_cur += render_seg(ctx, &plain, seg_cur, false, &st, plan.scale, seg_top, seg.textheight, y);
                    let hit: String = piece[s..e].iter().collect();
                    seg_cur += render_seg(ctx, &hit, seg_cur, true, &st, plan.scale, seg_top, seg.textheight, y);
                    last = e;
                }
                let tail: String = piece[last..].iter().collect();
                seg_cur += render_seg(ctx, &tail, seg_cur, false, &st, plan.scale, seg_top, seg.textheight, y);
                cursor = seg_cur;
            }
            i = j;
        }
    }

    // 개체 배치
    for o in &m.objs {
        if o.inline {
            let mut plan_idx = 0;
            for k in 0..plans.len() {
                let pl = &plans[k];
                if pl.start <= o.index && (o.index < pl.end || k == plans.len() - 1) {
                    plan_idx = k;
                }
            }
            if let Some(sp) = seg_pages {
                if let Some(&pg) = sp.get(plan_idx) {
                    ctx.page = pg;
                }
            }
            let plan = &plans[plan_idx];
            let x = ox + plan.seg.horzpos + plan.xoff + advance_to(&m, ctx.styles, plan, o.index);
            let y_top = oy + plan.seg.vertpos + (plan.seg.baseline - o.height).max(0.0);
            draw_object(o, x, y_top, ctx, depth);
        } else {
            if let Some(sp) = seg_pages {
                if let Some(&pg) = sp.first() {
                    ctx.page = pg;
                }
            }
            let (x, y) = anchor_object(o, ox, oy, base_v, area_w, ctx);
            draw_object(o, x, y, ctx, depth);
        }
    }
}

fn find_double_space(piece: &[char]) -> Option<usize> {
    let mut k = 0;
    while k + 1 < piece.len() {
        if piece[k] == ' ' && piece[k + 1] == ' ' {
            return Some(k);
        }
        k += 1;
    }
    None
}

/// 형광펜 매치 구간(문자 인덱스) 수집 → 병합(겹침 제거)
fn collect_highlights(piece: &[char], terms: &[String]) -> Vec<(usize, usize)> {
    let lower: Vec<char> = piece.iter().flat_map(|c| c.to_lowercase()).collect();
    // to_lowercase 는 문자수 보존이 보장되지 않으나, 검색 매칭 근사로 충분(대부분 1:1).
    // 안전을 위해 길이 불일치 시 형광펜 생략.
    if lower.len() != piece.len() {
        return Vec::new();
    }
    let mut found: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let t: Vec<char> = term.chars().collect();
        if t.is_empty() {
            continue;
        }
        let mut f = 0;
        while f + t.len() <= lower.len() {
            if lower[f..f + t.len()] == t[..] {
                found.push((f, f + t.len()));
                f += t.len();
            } else {
                f += 1;
            }
        }
    }
    found.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (s, e) in found {
        if let Some(tail) = merged.last_mut() {
            if s <= tail.1 {
                tail.1 = tail.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }
    merged
}

#[allow(clippy::too_many_arguments)]
fn render_seg(
    ctx: &mut Ctx,
    text: &str,
    cx: f64,
    hit: bool,
    st: &RenderCharStyle,
    scale: f64,
    seg_top: f64,
    seg_textheight: f64,
    y: f64,
) -> f64 {
    let sw = measure_text_width(text, st.height, st.ratio, &MeasureOptions { spacing_pct: st.spacing, ..Default::default() }) * scale;
    if hit {
        emit(
            ctx,
            format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#ffd54f" fill-opacity="0.45"/>"##,
                pt(cx),
                pt(seg_top),
                pt(sw),
                pt(seg_textheight)
            ),
        );
    }
    if !text.trim().is_empty() {
        let mut attrs: Vec<String> = vec![
            format!(r#"x="{}""#, pt(cx)),
            format!(r#"y="{}""#, pt(y)),
            format!(r#"font-size="{}""#, pt(st.height)),
        ];
        if let Some(ff) = &st.font_family {
            attrs.push(format!(r#"font-family="{}""#, escape_xml(ff)));
        }
        if text.chars().count() > 1 && sw > 50.0 {
            attrs.push(format!(r#"textLength="{}""#, pt(sw)));
            attrs.push(format!(r#"lengthAdjust="{}""#, if scale < 1.0 { "spacingAndGlyphs" } else { "spacing" }));
        }
        if st.bold {
            attrs.push(r#"font-weight="bold""#.to_string());
        }
        if st.italic {
            attrs.push(r#"font-style="italic""#.to_string());
        }
        if st.underline {
            attrs.push(r#"text-decoration="underline""#.to_string());
        }
        if let Some(color) = &st.color {
            attrs.push(format!(r#"fill="{}""#, escape_xml(color)));
        }
        emit(ctx, format!("<text {}>{}</text>", attrs.join(" "), escape_xml(text)));
        ctx.stats.texts += 1;
    }
    sw
}

/// hp:pos 기준계 해석 → 개체 좌상단 절대좌표 (tac=0)
fn anchor_object(o: &ParaObj, ox: f64, oy: f64, base_v: f64, area_w: f64, ctx: &Ctx<'_>) -> (f64, f64) {
    let PageGeom { pw, ph, ml, mt, body_w, body_h } = ctx.geom;
    let pos = find_child_local(o.el, "pos");
    let om = find_child_local(o.el, "outMargin");
    let om_t = num(om, "top", 0.0);
    let om_b = num(om, "bottom", 0.0);
    let w = o.width;
    let h = o.height;
    let pos = match pos {
        Some(p) => p,
        None => return (ox, oy + base_v),
    };
    let vo = num(Some(pos), "vertOffset", 0.0);
    let ho = num(Some(pos), "horzOffset", 0.0);
    let vrel = pos.attribute("vertRelTo").unwrap_or("PARA");
    let hrel = pos.attribute("horzRelTo").unwrap_or("PARA");
    let va = pos.attribute("vertAlign").unwrap_or("TOP");
    let ha = pos.attribute("horzAlign").unwrap_or("LEFT");
    let wrap = o.el.attribute("textWrap").unwrap_or("TOP_AND_BOTTOM");

    let y = if vrel == "PAPER" {
        match va {
            "BOTTOM" => ph - h - vo,
            "CENTER" => (ph - h) / 2.0 + vo,
            _ => vo,
        }
    } else if vrel == "PAGE" {
        match va {
            "BOTTOM" => mt + body_h - h - vo,
            "CENTER" => mt + (body_h - h) / 2.0 + vo,
            _ => mt + vo,
        }
    } else if wrap == "TOP_AND_BOTTOM" {
        let pushed = base_v - (om_t + h + om_b);
        let anchor = if pushed >= -100.0 { pushed } else { base_v };
        oy + anchor + om_t + vo
    } else {
        oy + base_v + vo
    };

    let x = if hrel == "PAGE" {
        match ha {
            "RIGHT" => ml + body_w - w - ho,
            "CENTER" => ml + (body_w - w) / 2.0 + ho,
            _ => ml + ho,
        }
    } else if hrel == "PAPER" {
        match ha {
            "RIGHT" => pw - w - ho,
            "CENTER" => (pw - w) / 2.0 + ho,
            _ => ho,
        }
    } else {
        match ha {
            "RIGHT" => ox + area_w - w - ho,
            "CENTER" => ox + (area_w - w) / 2.0 + ho,
            _ => ox + ho,
        }
    };
    (x, y)
}

fn draw_object<'a, 'i>(o: &ParaObj<'a, 'i>, x: f64, y: f64, ctx: &mut Ctx<'_>, depth: u32) {
    match o.tag.as_str() {
        "tbl" => draw_table(o.el, x, y, ctx, depth + 1),
        "pic" => draw_pic(o.el, x, y, ctx),
        "container" => {
            for ch in elements(o.el) {
                let tag = ln(&ch);
                if !is_obj_tag(tag) {
                    continue;
                }
                let sz = find_child_local(ch, "sz");
                let off = find_child_local(ch, "offset");
                let sub = ParaObj {
                    el: ch,
                    tag: tag.to_string(),
                    index: 0,
                    inline: true,
                    width: num(sz, "width", 0.0),
                    height: num(sz, "height", 0.0),
                };
                draw_object(&sub, x + num(off, "x", 0.0), y + num(off, "y", 0.0), ctx, depth + 1);
            }
        }
        "equation" => warn_once(ctx, "equation", "수식 개체는 렌더 미지원 — 생략"),
        t if is_shape_tag(t) => draw_shape(o, x, y, ctx, depth),
        t => {
            let key = format!("shape:{}", t);
            let msg = format!("개체({}) 렌더 미지원 — 생략", t);
            warn_once(ctx, &key, &msg);
        }
    }
}

// ─── 그리기 도형 ───────────────────────────────────

fn shape_stroke_pt(v: f64) -> f64 {
    ((v / 100.0) * 2.834645).max(0.2)
}

fn draw_shape<'a, 'i>(o: &ParaObj<'a, 'i>, x: f64, y: f64, ctx: &mut Ctx<'_>, depth: u32) {
    let el = o.el;
    let org_sz = find_child_local(el, "orgSz");
    let cur_sz = find_child_local(el, "curSz");
    let ow = num(org_sz, "width", 0.0);
    let oh = num(org_sz, "height", 0.0);
    let w = {
        let a = num(cur_sz, "width", 0.0);
        if a != 0.0 {
            a
        } else if ow != 0.0 {
            ow
        } else {
            o.width
        }
    };
    let h = {
        let a = num(cur_sz, "height", 0.0);
        if a != 0.0 {
            a
        } else if oh != 0.0 {
            oh
        } else {
            o.height
        }
    };
    let sx = if ow > 0.0 { w / ow } else { 1.0 };
    let sy = if oh > 0.0 { h / oh } else { 1.0 };

    let line_shape = find_child_local(el, "lineShape");
    let lstyle = line_shape.and_then(|l| l.attribute("style")).unwrap_or("SOLID");
    let stroke_col = line_shape.and_then(|l| l.attribute("color")).filter(|s| !s.is_empty()).unwrap_or("#000000");
    let has_stroke = lstyle != "NONE";
    let stroke_w = if has_stroke {
        shape_stroke_pt(if line_shape.is_some() { num(line_shape, "width", 0.0) } else { 33.0 })
    } else {
        0.0
    };
    let dash = if lstyle.contains("DASH") || lstyle.contains("DOT") {
        format!(r#" stroke-dasharray="{}""#, if lstyle.contains("DOT") { "1,1.5" } else { "3,1.5" })
    } else {
        String::new()
    };
    let stroke_attr = if has_stroke {
        format!(r#" stroke="{}" stroke-width="{:.2}"{}"#, escape_xml(stroke_col), stroke_w, dash)
    } else {
        String::new()
    };

    let fill_brush = find_child_local(el, "fillBrush");
    let win_brush = fill_brush.and_then(|fb| find_child_local(fb, "winBrush"));
    let face = win_brush.and_then(|w| w.attribute("faceColor"));
    let fill = match face {
        Some(f) if f.to_lowercase() != "none" => f,
        _ => "none",
    };
    let fill_attr = if fill == "none" {
        r#" fill="none""#.to_string()
    } else {
        format!(r#" fill="{}""#, escape_xml(fill))
    };

    match o.tag.as_str() {
        "rect" => emit(
            ctx,
            format!(r#"<rect x="{}" y="{}" width="{}" height="{}"{}{}/>"#, pt(x), pt(y), pt(w), pt(h), fill_attr, stroke_attr),
        ),
        "ellipse" => emit(
            ctx,
            format!(
                r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}"{}{}/>"#,
                pt(x + w / 2.0),
                pt(y + h / 2.0),
                pt(w / 2.0),
                pt(h / 2.0),
                fill_attr,
                stroke_attr
            ),
        ),
        "line" => {
            let s = find_child_local(el, "startPt");
            let e = find_child_local(el, "endPt");
            let x1 = x + num(s, "x", 0.0) * sx;
            let y1 = y + num(s, "y", 0.0) * sy;
            let x2 = x + num(e, "x", 0.0) * sx;
            let y2 = y + num(e, "y", 0.0) * sy;
            let sw = if stroke_w > 0.0 { stroke_w } else { 0.3 };
            emit(
                ctx,
                format!(
                    r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
                    pt(x1),
                    pt(y1),
                    pt(x2),
                    pt(y2),
                    escape_xml(stroke_col),
                    sw,
                    dash
                ),
            );
        }
        "polygon" | "curv" => {
            let mut pts: Vec<String> = Vec::new();
            for c in elements(el) {
                if ln(&c) == "pt" {
                    pts.push(format!("{},{}", pt(x + num(Some(c), "x", 0.0) * sx), pt(y + num(Some(c), "y", 0.0) * sy)));
                }
            }
            if pts.len() >= 2 {
                emit(ctx, format!(r#"<polygon points="{}"{}{}/>"#, pts.join(" "), fill_attr, stroke_attr));
            }
        }
        "arc" => {
            let sa = if !stroke_attr.is_empty() {
                stroke_attr.clone()
            } else {
                format!(r#" stroke="{}" stroke-width="0.3""#, escape_xml(stroke_col))
            };
            emit(
                ctx,
                format!(
                    r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="none"{}/>"#,
                    pt(x + w / 2.0),
                    pt(y + h / 2.0),
                    pt(w / 2.0),
                    pt(h / 2.0),
                    sa
                ),
            );
        }
        _ => {}
    }

    // 도형 안 텍스트
    let dt = find_child_local(el, "drawText");
    let sub = dt.and_then(|d| find_child_local(d, "subList"));
    if let Some(sub) = sub {
        for p in elements(sub) {
            if ln(&p) == "p" {
                draw_para(p, x, y, w, ctx, depth + 1, None);
            }
        }
    }
}

// ─── 표 ───────────────────────────────────────────

struct CellModel<'a, 'i> {
    el: Node<'a, 'i>,
    ca: usize,
    ra: usize,
    cs: usize,
    rs: usize,
    w: f64,
    h: f64,
    bf_id: Option<&'a str>,
    sub: Option<Node<'a, 'i>>,
    margin_l: f64,
    margin_r: f64,
    margin_t: f64,
    margin_b: f64,
}

fn collect_cells<'a, 'i>(tbl: Node<'a, 'i>) -> Vec<CellModel<'a, 'i>> {
    let in_margin = find_child_local(tbl, "inMargin");
    let def_l = num(in_margin, "left", 141.0);
    let def_r = num(in_margin, "right", 141.0);
    let def_t = num(in_margin, "top", 141.0);
    let def_b = num(in_margin, "bottom", 141.0);
    let mut cells = Vec::new();
    for tr in elements(tbl) {
        if ln(&tr) != "tr" {
            continue;
        }
        for tc in elements(tr) {
            if ln(&tc) != "tc" {
                continue;
            }
            let addr = find_child_local(tc, "cellAddr");
            let span = find_child_local(tc, "cellSpan");
            let csz = find_child_local(tc, "cellSz");
            let cm = find_child_local(tc, "cellMargin");
            if addr.is_none() || csz.is_none() {
                continue;
            }
            cells.push(CellModel {
                el: tc,
                // 좌표/span 상한 클램프 — colAddr/rowAddr/span 이 사실상 무제한 usize 로
                // 변환되면 n_cols/n_rows 가 폭증해 solve_boundaries 가 count+1 벡터를
                // 할당하며 작은 입력만으로 OOM. f64→usize 는 saturating(범위초과·NaN→0),
                // .min(MAX_TABLE_DIM) 로 한 셀 값이 표 차원을 폭증시키지 못하게 막는다.
                ca: (num(addr, "colAddr", 0.0).max(0.0) as usize).min(MAX_TABLE_DIM),
                ra: (num(addr, "rowAddr", 0.0).max(0.0) as usize).min(MAX_TABLE_DIM),
                cs: ((num(span, "colSpan", 1.0).max(1.0)) as usize).min(MAX_TABLE_DIM),
                rs: ((num(span, "rowSpan", 1.0).max(1.0)) as usize).min(MAX_TABLE_DIM),
                w: num(csz, "width", 0.0),
                h: num(csz, "height", 0.0),
                bf_id: tc.attribute("borderFillIDRef"),
                sub: find_child_local(tc, "subList"),
                margin_l: if cm.is_some() { num(cm, "left", def_l) } else { def_l },
                margin_r: if cm.is_some() { num(cm, "right", def_r) } else { def_r },
                margin_t: if cm.is_some() { num(cm, "top", def_t) } else { def_t },
                margin_b: if cm.is_some() { num(cm, "bottom", def_b) } else { def_b },
            });
        }
    }
    cells
}

/// 셀 콘텐츠 세로 범위
fn cell_content_extent(cell: &CellModel, styles: &RenderStyles, synthetic: &Synthetic, memo: &mut ExtentMemo) -> f64 {
    let sub = match cell.sub {
        Some(s) => s,
        None => return 0.0,
    };
    if let Some(&hit) = memo.cell.get(&cell.el.id()) {
        return hit;
    }
    let mut ext = 0.0_f64;
    for p in elements(sub) {
        if ln(&p) != "p" {
            continue;
        }
        let m = build_para(p, synthetic);
        for s in &m.segs {
            ext = ext.max(s.vertpos + s.textheight);
        }
        let base_v = m.segs.first().map(|s| s.vertpos).unwrap_or(0.0);
        for o in &m.objs {
            if o.inline {
                let h = if o.tag == "tbl" {
                    o.height.max(measure_table_height(o.el, styles, synthetic, memo))
                } else {
                    o.height
                };
                ext = ext.max(base_v + h);
                continue;
            }
            let pos = find_child_local(o.el, "pos");
            if pos.and_then(|p| p.attribute("vertRelTo")).unwrap_or("PARA") != "PARA" {
                continue;
            }
            let om = find_child_local(o.el, "outMargin");
            let pushed = base_v - (num(om, "top", 0.0) + o.height + num(om, "bottom", 0.0));
            let anchor = if pushed >= -100.0 { pushed } else { base_v };
            ext = ext.max(anchor + num(om, "top", 0.0) + num(pos, "vertOffset", 0.0) + o.height);
        }
    }
    memo.cell.insert(cell.el.id(), ext);
    ext
}

/// 표 실효 높이(HWPUNIT)
pub fn measure_table_height(tbl: Node, styles: &RenderStyles, synthetic: &Synthetic, memo: &mut ExtentMemo) -> f64 {
    if let Some(&hit) = memo.table.get(&tbl.id()) {
        return hit;
    }
    let cells = collect_cells(tbl);
    if cells.is_empty() || cells.len() > 4096 {
        return 0.0;
    }
    let n_rows = cells.iter().map(|c| c.ra + c.rs).max().unwrap_or(0);
    let row_cells: Vec<RowCell> = cells
        .iter()
        .map(|c| RowCell {
            row_addr: c.ra,
            row_span: c.rs,
            height: c.h,
            content_h: if c.rs == 1 { Some(cell_content_extent(c, styles, synthetic, memo)) } else { None },
        })
        .collect();
    let row_h = solve_row_heights(&row_cells, n_rows);
    let sum: f64 = row_h.iter().sum();
    memo.table.insert(tbl.id(), sum);
    sum
}

fn edge_line(x1: f64, y1: f64, x2: f64, y2: f64, e: &RenderBorderEdge) -> String {
    let dash = if e.edge_type.contains("DASH") || e.edge_type.contains("DOT") {
        format!(r#" stroke-dasharray="{}""#, if e.edge_type.contains("DOT") { "1,1.5" } else { "3,1.5" })
    } else {
        String::new()
    };
    format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
        pt(x1),
        pt(y1),
        pt(x2),
        pt(y2),
        escape_xml(&e.color),
        e.width_pt,
        dash
    )
}

fn draw_table<'a, 'i>(tbl: Node<'a, 'i>, tx: f64, ty: f64, ctx: &mut Ctx<'_>, depth: u32) {
    if depth > 16 {
        warn_once(ctx, "depth", "중첩 깊이 16 초과 — 이하 생략");
        return;
    }
    ctx.stats.tables += 1;
    let tbl_sz = find_child_local(tbl, "sz");
    let cells = collect_cells(tbl);
    if cells.is_empty() || cells.len() > 4096 {
        return;
    }
    let n_cols = cells.iter().map(|c| c.ca + c.cs).max().unwrap_or(0);
    let n_rows = cells.iter().map(|c| c.ra + c.rs).max().unwrap_or(0);
    let col_cons: Vec<SpanConstraint> = cells.iter().map(|c| SpanConstraint { a: c.ca, b: c.ca + c.cs, size: c.w }).collect();
    let total_w = num(tbl_sz, "width", 0.0);
    let col_x = solve_boundaries(&col_cons, n_cols, if total_w != 0.0 { Some(total_w) } else { None });

    // 셀 콘텐츠 extent 선계산 (rowH contentH + pass2 yoff 공용) — 필드 disjoint 대여
    let extents: Vec<f64> = {
        let styles = ctx.styles;
        let synthetic = &ctx.synthetic;
        let memo = &mut ctx.extent_memo;
        cells.iter().map(|c| cell_content_extent(c, styles, synthetic, memo)).collect()
    };

    let row_cells: Vec<RowCell> = cells
        .iter()
        .enumerate()
        .map(|(idx, c)| RowCell {
            row_addr: c.ra,
            row_span: c.rs,
            height: c.h,
            content_h: if c.rs == 1 { Some(extents[idx]) } else { None },
        })
        .collect();
    let row_h = solve_row_heights(&row_cells, n_rows);
    let mut row_y = vec![0.0_f64];
    for r in 0..n_rows {
        row_y.push(row_y[r] + row_h[r]);
    }

    struct CellGeom {
        idx: usize,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    let geom: Vec<CellGeom> = cells
        .iter()
        .enumerate()
        .map(|(idx, c)| CellGeom {
            idx,
            x: tx + col_x[c.ca],
            y: ty + row_y[c.ra],
            w: col_x[(c.ca + c.cs).min(n_cols)] - col_x[c.ca],
            h: row_y[(c.ra + c.rs).min(n_rows)] - row_y[c.ra],
        })
        .collect();

    // 1패스: 배경
    for g in &geom {
        if let Some(bf) = cells[g.idx].bf_id.and_then(|id| ctx.styles.border_fill.get(id)) {
            if let Some(fill) = bf.fill.clone() {
                emit(
                    ctx,
                    format!(r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>"#, pt(g.x), pt(g.y), pt(g.w), pt(g.h), escape_xml(&fill)),
                );
            }
        }
    }
    // 2패스: 콘텐츠
    for g in &geom {
        let c = &cells[g.idx];
        let sub = match c.sub {
            Some(s) => s,
            None => continue,
        };
        let inner_h = g.h - c.margin_t - c.margin_b;
        let extent = extents[g.idx];
        let va = sub.attribute("vertAlign").unwrap_or("TOP");
        let yoff = match va {
            "CENTER" => ((inner_h - extent) / 2.0).max(0.0),
            "BOTTOM" => (inner_h - extent).max(0.0),
            _ => 0.0,
        };
        for p in elements(sub) {
            if ln(&p) != "p" {
                continue;
            }
            draw_para(p, g.x + c.margin_l, g.y + c.margin_t + yoff, g.w - c.margin_l - c.margin_r, ctx, depth + 1, None);
        }
    }
    // 3패스: 테두리
    for g in &geom {
        let bf = match cells[g.idx].bf_id.and_then(|id| ctx.styles.border_fill.get(id)).cloned() {
            Some(b) => b,
            None => continue,
        };
        if let Some(e) = &bf.top {
            emit(ctx, edge_line(g.x, g.y, g.x + g.w, g.y, e));
        }
        if let Some(e) = &bf.bottom {
            emit(ctx, edge_line(g.x, g.y + g.h, g.x + g.w, g.y + g.h, e));
        }
        if let Some(e) = &bf.left {
            emit(ctx, edge_line(g.x, g.y, g.x, g.y + g.h, e));
        }
        if let Some(e) = &bf.right {
            emit(ctx, edge_line(g.x + g.w, g.y, g.x + g.w, g.y + g.h, e));
        }
    }
}

// ─── 이미지 ───────────────────────────────────────

fn image_symbol(loaded: &mut LoadedImage, defs: &mut Vec<String>) -> usize {
    if loaded.sym_id.is_none() {
        let id = defs.len();
        loaded.sym_id = Some(id);
        defs.push(format!(
            r#"<symbol id="bin{}" viewBox="0 0 100 100" preserveAspectRatio="none"><image width="100" height="100" preserveAspectRatio="none" href="{}"/></symbol>"#,
            id, loaded.data_uri
        ));
    }
    loaded.sym_id.unwrap()
}

fn draw_pic<'a, 'i>(pic: Node<'a, 'i>, x: f64, y: f64, ctx: &mut Ctx<'_>) {
    let sz = find_child_local(pic, "sz");
    let w = num(sz, "width", 5669.0);
    let h = num(sz, "height", 5669.0);
    let img = find_first(pic, "img");
    let refr = img.and_then(|i| i.attribute("binaryItemIDRef"));
    let has = refr.map(|r| ctx.images.contains_key(r)).unwrap_or(false);
    if !has {
        emit(
            ctx,
            format!(r##"<rect x="{}" y="{}" width="{}" height="{}" fill="#eee" stroke="#c00" stroke-width="0.5"/>"##, pt(x), pt(y), pt(w), pt(h)),
        );
        let key = format!("img:{}", refr.unwrap_or("(none)"));
        let msg = format!("이미지 바이너리 누락: {}", refr.unwrap_or("(ref 없음)"));
        warn_once(ctx, &key, &msg);
        return;
    }
    ctx.stats.images += 1;
    let clip = find_child_local(pic, "imgClip");
    let img_dim = find_child_local(pic, "imgDim");
    let org_sz = find_child_local(pic, "orgSz");
    let dim_w = num(img_dim, "dimwidth", 0.0);
    let dim_h = num(img_dim, "dimheight", 0.0);
    let ref_w = if dim_w > 0.0 { dim_w } else { num(org_sz, "width", 0.0) };
    let ref_h = if dim_h > 0.0 { dim_h } else { num(org_sz, "height", 0.0) };
    let cl = num(clip, "left", 0.0);
    let ct = num(clip, "top", 0.0);
    let cr = num(clip, "right", ref_w);
    let cb = num(clip, "bottom", ref_h);
    let cropped = ref_w > 0.0
        && ref_h > 0.0
        && clip.is_some()
        && (cl > 0.0 || ct > 0.0 || cr < ref_w || cb < ref_h)
        && cr > cl
        && cb > ct;

    let refr = refr.unwrap();
    let sym_id = {
        let entry = ctx.images.get_mut(refr).unwrap();
        image_symbol(entry, &mut ctx.defs)
    };
    if cropped {
        emit(
            ctx,
            format!(
                r##"<svg x="{}" y="{}" width="{}" height="{}" viewBox="{} {} {} {}" preserveAspectRatio="none"><use href="#bin{}" x="0" y="0" width="{}" height="{}"/></svg>"##,
                pt(x),
                pt(y),
                pt(w),
                pt(h),
                pt(cl),
                pt(ct),
                pt(cr - cl),
                pt(cb - ct),
                sym_id,
                pt(ref_w),
                pt(ref_h)
            ),
        );
    } else {
        emit(ctx, format!(r##"<use href="#bin{}" x="{}" y="{}" width="{}" height="{}"/>"##, sym_id, pt(x), pt(y), pt(w), pt(h)));
    }
}

// ─── 페이지 지오메트리 ──────────────────────────────

pub fn read_section_geom(root: Node) -> PageGeom {
    let page_pr = find_first(root, "pagePr");
    let margin = page_pr.and_then(|p| find_child_local(p, "margin"));
    let mut pw = num(page_pr, "width", 59528.0);
    let mut ph = num(page_pr, "height", 84188.0);
    if page_pr.and_then(|p| p.attribute("landscape")) == Some("NARROWLY") && pw < ph {
        std::mem::swap(&mut pw, &mut ph);
    }
    let ml = num(margin, "left", 8504.0);
    let mt = num(margin, "top", 5668.0) + num(margin, "header", 0.0);
    let body_h = ph - mt - num(margin, "bottom", 4252.0) - num(margin, "footer", 0.0);
    let body_w = pw - ml - num(margin, "right", 8504.0);
    PageGeom { pw, ph, ml, mt, body_w, body_h }
}

/// 한 구역을 렌더 → (페이지 버퍼들, pageH). ctx의 공유 자원(images/defs/stats/warnings)은 누적.
pub fn render_section_to_pages<'a, 'i>(
    root: Node<'a, 'i>,
    geom: PageGeom,
    ctx: &mut Ctx<'_>,
    do_reflow: bool,
    reflow_mode: super::metrics::WrapMode,
) -> (Vec<Vec<String>>, f64) {
    // 구역 진입마다 조판 사이드 테이블/메모 리셋 (NodeId는 Document 로컬)
    ctx.synthetic.clear();
    ctx.extent_memo = ExtentMemo::default();
    ctx.geom = geom;

    if do_reflow {
        ctx.synthetic = super::reflow::reflow_section(root, ctx.styles, geom.body_w, geom.body_h, reflow_mode);
    }

    // 페이지 분할 프리패스
    let col_pr = find_first(root, "colPr");
    let multi_col = num(col_pr, "colCount", 1.0) > 1.0;
    let mut para_seg_pages: HashMap<NodeId, Vec<usize>> = HashMap::new();
    let mut n_pages = 1usize;
    let mut max_top_v = 0.0_f64;
    {
        let mut prev_v = f64::NEG_INFINITY;
        let mut prev_h = f64::NEG_INFINITY;
        let mut cur = 0usize;
        for p in elements(root) {
            if ln(&p) != "p" {
                continue;
            }
            let segs = read_para_segs(p, &ctx.synthetic);
            let mut pages_of: Vec<usize> = Vec::new();
            let mut para_first = true;
            for s in &segs {
                let v = s.vertpos;
                let hh = s.horzpos;
                let brk = if v < prev_v {
                    !multi_col || hh <= prev_h
                } else {
                    para_first && v == prev_v && hh <= prev_h
                };
                if brk {
                    cur += 1;
                }
                para_first = false;
                pages_of.push(cur);
                max_top_v = max_top_v.max(v + s.textheight);
                prev_v = v;
                prev_h = hh;
            }
            para_seg_pages.insert(p.id(), pages_of);
            n_pages = n_pages.max(cur + 1);
        }
    }

    ctx.pages = (0..n_pages).map(|_| Vec::new()).collect();
    ctx.page = 0;
    let ml = geom.ml;
    let mt = geom.mt;
    let body_w = geom.body_w;
    // top-level 문단 노드를 미리 수집 (borrow 안정)
    let top_paras: Vec<Node> = elements(root).filter(|p| ln(p) == "p").collect();
    for p in top_paras {
        let sp = para_seg_pages.get(&p.id()).cloned();
        draw_para(p, ml, mt, body_w, ctx, 0, sp.as_deref());
    }

    let page_h = if n_pages == 1 { geom.ph.max(mt + max_top_v + 2000.0) } else { geom.ph };
    (std::mem::take(&mut ctx.pages), page_h)
}

pub fn new_ctx<'s>(styles: &'s RenderStyles, images: HashMap<String, LoadedImage>, highlights: Vec<String>) -> Ctx<'s> {
    Ctx {
        pages: Vec::new(),
        page: 0,
        geom: PageGeom { pw: 0.0, ph: 0.0, ml: 0.0, mt: 0.0, body_w: 0.0, body_h: 0.0 },
        styles,
        images,
        defs: Vec::new(),
        highlights,
        warnings: Vec::new(),
        warned: HashSet::new(),
        stats: RenderStats::default(),
        extent_memo: ExtentMemo::default(),
        synthetic: HashMap::new(),
    }
}
