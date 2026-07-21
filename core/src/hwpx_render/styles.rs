// Ported from kkdoc (MIT): src/render/head-styles.ts
//! 레이아웃 보존 렌더 — header.xml 스타일 테이블 (charPr/paraPr/borderFill).

use super::dom::{elements, find_child_local, ln};
use super::metrics::WrapMode;
use std::collections::HashMap;

#[derive(Clone)]
pub struct RenderCharStyle {
    /// 글자 크기 (1/100pt — 1000 = 10pt)
    pub height: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    /// #RRGGBB — 검정이면 None
    pub color: Option<String>,
    /// 장평 %
    pub ratio: f64,
    /// 자간 %
    pub spacing: f64,
    /// CSS font-family 스택
    pub font_family: Option<String>,
    /// HWP 원본 글꼴명 (reflow 폭 테이블 선택용)
    pub face: Option<String>,
}

pub fn default_char() -> RenderCharStyle {
    RenderCharStyle {
        height: 1000.0,
        bold: false,
        italic: false,
        underline: false,
        color: None,
        ratio: 100.0,
        spacing: 0.0,
        font_family: None,
        face: None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ParaAlign {
    Justify,
    Left,
    Right,
    Center,
    Distribute,
    DistributeSpace,
}

impl ParaAlign {
    fn from_attr(s: Option<&str>) -> ParaAlign {
        match s {
            Some("LEFT") => ParaAlign::Left,
            Some("RIGHT") => ParaAlign::Right,
            Some("CENTER") => ParaAlign::Center,
            Some("DISTRIBUTE") => ParaAlign::Distribute,
            Some("DISTRIBUTE_SPACE") => ParaAlign::DistributeSpace,
            _ => ParaAlign::Justify,
        }
    }
}

/// reflow(Tier-2)용 문단 기하 — 줄간격·여백. 단위 HWPUNIT.
#[derive(Clone)]
pub struct RenderParaGeom {
    pub line_spacing_type: String,
    pub line_spacing_value: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub margin_intent: f64,
    pub space_before: f64,
    pub space_after: f64,
    pub wrap_mode: Option<WrapMode>,
}

pub fn default_para_geom() -> RenderParaGeom {
    RenderParaGeom {
        line_spacing_type: "PERCENT".to_string(),
        line_spacing_value: 160.0,
        margin_left: 0.0,
        margin_right: 0.0,
        margin_intent: 0.0,
        space_before: 0.0,
        space_after: 0.0,
        wrap_mode: None,
    }
}

#[derive(Clone)]
pub struct RenderBorderEdge {
    pub edge_type: String,
    pub width_pt: f64,
    pub color: String,
}

#[derive(Clone, Default)]
pub struct RenderBorderFill {
    pub left: Option<RenderBorderEdge>,
    pub right: Option<RenderBorderEdge>,
    pub top: Option<RenderBorderEdge>,
    pub bottom: Option<RenderBorderEdge>,
    pub fill: Option<String>,
}

#[derive(Default)]
pub struct RenderStyles {
    pub char_pr: HashMap<String, RenderCharStyle>,
    pub para_align: HashMap<String, ParaAlign>,
    pub para_geom: HashMap<String, RenderParaGeom>,
    pub border_fill: HashMap<String, RenderBorderFill>,
}

// ─── 글꼴 매핑 ──────────────────────────────────────

fn font_alias(name: &str) -> Option<&'static str> {
    Some(match name {
        "함초롬바탕" | "한컴바탕" => "'HCR Batang','함초롬바탕','한컴바탕'",
        "함초롬돋움" | "한컴돋움" => "'HCR Dotum','함초롬돋움','한컴돋움'",
        "맑은 고딕" | "맑은고딕" => "'Malgun Gothic','맑은 고딕'",
        "굴림" => "'Gulim','굴림'",
        "굴림체" => "'GulimChe','굴림체','Gulim'",
        "돋움" => "'Dotum','돋움'",
        "돋움체" => "'DotumChe','돋움체','Dotum'",
        "바탕" => "'Batang','바탕'",
        "바탕체" => "'BatangChe','바탕체','Batang'",
        "궁서" => "'Gungsuh','궁서'",
        "궁서체" => "'GungsuhChe','궁서체','Gungsuh'",
        "나눔고딕" => "'NanumGothic','나눔고딕'",
        "나눔명조" => "'NanumMyeongjo','나눔명조'",
        "맑은 고딕 Semilight" => "'Malgun Gothic Semilight','맑은 고딕'",
        _ => return None,
    })
}

/// 명조/바탕 계열(serif) 여부 — 아니면 고딕/돋움(sans)
fn is_serif_face(face: &str) -> bool {
    const NEEDLES: [&str; 12] =
        ["바탕", "명조", "궁서", "신명", "순명", "송", "Batang", "Myeong", "Mincho", "Gungsuh", "Serif", "Song"];
    let lower = face.to_lowercase();
    NEEDLES.iter().any(|n| {
        let nl = n.to_lowercase();
        face.contains(n) || lower.contains(&nl)
    })
}

fn css_quote(name: &str) -> String {
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        name.to_string()
    } else {
        let cleaned: String = name.chars().filter(|c| !matches!(c, '\'' | '"' | '\\')).collect();
        format!("'{}'", cleaned)
    }
}

/// HWP 글꼴명 → CSS font-family 스택.
pub fn hwp_face_to_css_stack(face: Option<&str>) -> String {
    let trimmed = face.unwrap_or("").trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let generic = if is_serif_face(trimmed) {
        "'HCR Batang','Batang','Noto Serif KR',serif"
    } else {
        "'Malgun Gothic','HCR Dotum','Noto Sans KR',sans-serif"
    };
    let head = font_alias(trimmed).map(|s| s.to_string()).unwrap_or_else(|| css_quote(trimmed));
    format!("{},{}", head, generic)
}

// ─── 파싱 ──────────────────────────────────────────

/// 서브트리에서 local name 일치 첫 요소 재귀 (switch/case 래핑 대응) — head-styles findDeep
fn find_deep<'a, 'input>(el: roxmltree::Node<'a, 'input>, name: &str, depth: u32) -> Option<roxmltree::Node<'a, 'input>> {
    if depth > 32 {
        return None;
    }
    for ch in elements(el) {
        if ln(&ch) == name {
            return Some(ch);
        }
        if let Some(f) = find_deep(ch, name, depth + 1) {
            return Some(f);
        }
    }
    None
}

/// fontfaces(HANGUL) 그룹의 font id → 글꼴명 맵
fn collect_hangul_fonts(root: roxmltree::Node) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let faces = match find_deep(root, "fontfaces", 0) {
        Some(f) => f,
        None => return map,
    };
    let mut group = None;
    let mut first_group = None;
    for e in elements(faces) {
        if ln(&e) != "fontface" {
            continue;
        }
        if first_group.is_none() {
            first_group = Some(e);
        }
        if e.attribute("lang").map(|l| l.to_uppercase()) == Some("HANGUL".to_string()) {
            group = Some(e);
            break;
        }
    }
    let group = group.or(first_group);
    let group = match group {
        Some(g) => g,
        None => return map,
    };
    for e in elements(group) {
        if ln(&e) != "font" {
            continue;
        }
        if let (Some(id), Some(face)) = (e.attribute("id"), e.attribute("face")) {
            map.insert(id.to_string(), face.to_string());
        }
    }
    map
}

/// "0.12 mm" → pt
fn border_width_pt(v: Option<&str>) -> f64 {
    let n: f64 = v.and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(f64::NAN);
    if !n.is_finite() {
        return 0.34;
    }
    n * 2.834645 // mm → pt
}

fn parse_edge(el: Option<roxmltree::Node>) -> Option<RenderBorderEdge> {
    let el = el?;
    let edge_type = el.attribute("type").unwrap_or("NONE");
    if edge_type == "NONE" {
        return None;
    }
    Some(RenderBorderEdge {
        edge_type: edge_type.to_string(),
        width_pt: border_width_pt(el.attribute("width")),
        color: el.attribute("color").unwrap_or("#000000").to_string(),
    })
}

fn parse_para_geom(el: roxmltree::Node) -> RenderParaGeom {
    let mut g = default_para_geom();
    if let Some(ls) = find_deep(el, "lineSpacing", 0) {
        g.line_spacing_type = ls.attribute("type").unwrap_or("PERCENT").to_string();
        g.line_spacing_value = ls.attribute("value").and_then(|v| v.parse().ok()).unwrap_or(160.0);
    }
    if let Some(margin) = find_deep(el, "margin", 0) {
        let v = |name: &str| -> f64 {
            find_deep(margin, name, 0).and_then(|c| c.attribute("value")).and_then(|s| s.parse().ok()).unwrap_or(0.0)
        };
        g.margin_left = v("left");
        g.margin_right = v("right");
        g.margin_intent = v("intent");
        g.space_before = v("prev");
        g.space_after = v("next");
    }
    if let Some(bs) = find_deep(el, "breakSetting", 0) {
        match bs.attribute("breakNonLatinWord") {
            // 속성 의미 역전: BREAK_WORD=어절, KEEP_WORD=글자
            Some("BREAK_WORD") => g.wrap_mode = Some(WrapMode::Keep),
            Some("KEEP_WORD") => g.wrap_mode = Some(WrapMode::CharAll),
            _ => {}
        }
    }
    g
}

fn parse_char_style(el: roxmltree::Node, hangul_fonts: &HashMap<String, String>) -> RenderCharStyle {
    let ratio_el = find_child_local(el, "ratio");
    let spacing_el = find_child_local(el, "spacing");
    let underline_el = find_child_local(el, "underline");
    let text_color = el.attribute("textColor");
    let font_ref = find_child_local(el, "fontRef");
    let font_id = font_ref.and_then(|fr| fr.attribute("hangul").or_else(|| fr.attribute("latin")));
    let face = font_id.and_then(|id| hangul_fonts.get(id)).cloned();
    let color = match text_color {
        Some(c) if c != "#000000" && c.to_lowercase() != "none" => Some(c.to_string()),
        _ => None,
    };
    let underline = underline_el.map(|u| u.attribute("type").unwrap_or("NONE") != "NONE").unwrap_or(false);
    RenderCharStyle {
        height: el.attribute("height").and_then(|v| v.parse().ok()).unwrap_or(1000.0),
        bold: find_child_local(el, "bold").is_some(),
        italic: find_child_local(el, "italic").is_some(),
        underline,
        color,
        ratio: ratio_el.and_then(|r| r.attribute("hangul")).and_then(|v| v.parse().ok()).unwrap_or(100.0),
        spacing: spacing_el.and_then(|s| s.attribute("hangul")).and_then(|v| v.parse().ok()).unwrap_or(0.0),
        font_family: face.as_deref().map(|f| hwp_face_to_css_stack(Some(f))),
        face,
    }
}

/// header.xml(구버전 head.xml) → 렌더용 스타일 맵
pub fn parse_render_styles(head_xml: &str) -> RenderStyles {
    let mut styles = RenderStyles::default();
    let doc = match roxmltree::Document::parse(head_xml) {
        Ok(d) => d,
        Err(_) => return styles,
    };
    let root = doc.root_element();
    let hangul_fonts = collect_hangul_fonts(root);

    // 전 노드 순회 (재귀 DFS)
    for node in root.descendants() {
        if !node.is_element() {
            continue;
        }
        match ln(&node) {
            "charPr" => {
                if let Some(id) = node.attribute("id") {
                    styles.char_pr.insert(id.to_string(), parse_char_style(node, &hangul_fonts));
                }
            }
            "paraPr" => {
                if let Some(id) = node.attribute("id") {
                    let align = find_child_local(node, "align");
                    styles.para_align.insert(id.to_string(), ParaAlign::from_attr(align.and_then(|a| a.attribute("horizontal"))));
                    styles.para_geom.insert(id.to_string(), parse_para_geom(node));
                }
            }
            "borderFill" => {
                if let Some(id) = node.attribute("id") {
                    let mut bf = RenderBorderFill {
                        left: parse_edge(find_child_local(node, "leftBorder")),
                        right: parse_edge(find_child_local(node, "rightBorder")),
                        top: parse_edge(find_child_local(node, "topBorder")),
                        bottom: parse_edge(find_child_local(node, "bottomBorder")),
                        fill: None,
                    };
                    let win_brush = find_child_local(node, "fillBrush").and_then(|fb| find_child_local(fb, "winBrush"));
                    if let Some(face) = win_brush.and_then(|w| w.attribute("faceColor")) {
                        if face.to_lowercase() != "none" {
                            bf.fill = Some(face.to_string());
                        }
                    }
                    styles.border_fill.insert(id.to_string(), bf);
                }
            }
            _ => {}
        }
    }
    styles
}
