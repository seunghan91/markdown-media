//! HWPX drawing-object XML fragment → self-contained per-object SVG.
//!
//! Ported from `hwpx_render::svg::draw_shape` (`core/src/hwpx_render/svg.rs`,
//! gated behind the opt-in `hwpx-render` feature) into a pure, always-on
//! function. `hwpx_render` is deliberately NOT reused directly:
//!
//! - It requires a whole-page layout cache (`Ctx`, synthetic linesegarray,
//!   page-global warning/stat buffers) that only makes sense when rendering
//!   an entire section, not one drawing object in isolation.
//! - It lives behind `hwpx-render` (default-off), but this module needs to
//!   be part of the default HWPX conversion path (see the P2 plan).
//!
//! `hwpx_render` itself is left unmodified — see the P2 design plan
//! (`~/.claude/plans/inherited-soaring-hippo-agent-aplan-p2-svg-3c37f47a5d8008d4.md`,
//! judgment call #1) for the "why a new module instead of reuse" reasoning.
//!
//! Scope (M1, first release): the 6 pure-drawing shapes HWPX represents as
//! plain XML — `hp:rect`, `hp:ellipse`, `hp:line`, `hp:polygon`, `hp:curv`,
//! `hp:arc`. Text inside a shape (`hp:drawText`) is intentionally NOT drawn
//! into the SVG (see [`ShapeSvg::has_drawtext`]); geometry approximations
//! (arc → ellipse, curv → straight-line polygon) mirror `hwpx_render` and
//! are a known M1 limitation (real bezier/arc geometry is M2).

use roxmltree::{Document, Node};

/// A single HWPX drawing object rendered as a self-contained SVG fragment.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeSvg {
    /// Self-contained `<svg xmlns=... viewBox="0 0 w h">...</svg>` markup.
    /// The object is normalized to its own (0,0) origin — no page-anchor
    /// offset is applied (there is no page context for a standalone asset).
    pub svg: String,
    /// Rendered width in points (`curSz`, falling back to `orgSz`).
    pub width_pt: f64,
    /// Rendered height in points (`curSz`, falling back to `orgSz`).
    pub height_pt: f64,
    /// Short human-readable description, for alt text / non-visual
    /// consumers. Includes a text summary when [`Self::has_drawtext`].
    pub alt: String,
    /// True when the shape carries an `hp:drawText` body. The text itself is
    /// deliberately excluded from `svg` (see module docs) — callers should
    /// keep showing it in the document body rather than only in the SVG
    /// asset. Per the P2 plan's textbox policy, callers default to NOT
    /// emitting an asset at all for `has_drawtext` shapes (`assets/images/`)
    /// unless an opt-in flag is set — that policy decision lives with the
    /// caller, not here.
    pub has_drawtext: bool,
}

/// Namespace prefixes shape XML fragments use, wrapped around the input so
/// callers can pass either a bare fragment (as cut by a tag-boundary scanner
/// with no xmlns in scope) or one that already declares its own — inner
/// re-declarations just shadow these, which is valid XML.
const NS_WRAPPER_OPEN: &str = concat!(
    r#"<mdm-shape-root"#,
    r#" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph""#,
    r#" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core""#,
    r#" xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section""#,
    r#" xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head""#,
    r#">"#,
);
const NS_WRAPPER_CLOSE: &str = "</mdm-shape-root>";

/// Parse a single HWPX drawing-object element (`hp:rect` / `hp:ellipse` /
/// `hp:line` / `hp:polygon` / `hp:curv` / `hp:arc`) into a self-contained
/// SVG. `xml_fragment` must be the shape's own outer element, e.g.
/// `<hp:rect id="...">...</hp:rect>`. Returns `None` when the fragment
/// doesn't parse as XML, its root isn't a recognized shape tag, or the
/// shape's geometry is too degenerate to draw anything (e.g. a
/// `polygon`/`curv` with fewer than 2 points).
pub fn shape_to_svg(xml_fragment: &str) -> Option<ShapeSvg> {
    let wrapped = format!("{NS_WRAPPER_OPEN}{xml_fragment}{NS_WRAPPER_CLOSE}");
    let doc = Document::parse(&wrapped).ok()?;
    let el = doc.root_element().children().find(|c| c.is_element())?;
    let tag = local_name(&el);
    if !matches!(tag, "rect" | "ellipse" | "line" | "polygon" | "curv" | "arc") {
        return None;
    }

    let org_sz = find_child(el, "orgSz");
    let cur_sz = find_child(el, "curSz");
    let ow = num(org_sz, "width", 0.0);
    let oh = num(org_sz, "height", 0.0);
    let w = {
        let a = num(cur_sz, "width", 0.0);
        if a != 0.0 {
            a
        } else {
            ow
        }
    };
    let h = {
        let a = num(cur_sz, "height", 0.0);
        if a != 0.0 {
            a
        } else {
            oh
        }
    };
    // Area shapes need positive extent to draw anything meaningful; `line`
    // is legitimately zero-height (horizontal) or zero-width (vertical).
    if tag != "line" && (w <= 0.0 || h <= 0.0) {
        return None;
    }
    let sx = if ow > 0.0 { w / ow } else { 1.0 };
    let sy = if oh > 0.0 { h / oh } else { 1.0 };

    let line_shape = find_child(el, "lineShape");
    let lstyle = line_shape.and_then(|l| l.attribute("style")).unwrap_or("SOLID");
    let stroke_col = line_shape
        .and_then(|l| l.attribute("color"))
        .filter(|s| !s.is_empty())
        .unwrap_or("#000000");
    let has_stroke = lstyle != "NONE";
    let stroke_w = if has_stroke {
        stroke_pt(if line_shape.is_some() {
            num(line_shape, "width", 0.0)
        } else {
            33.0
        })
    } else {
        0.0
    };
    let dash = if lstyle.contains("DASH") || lstyle.contains("DOT") {
        format!(
            r#" stroke-dasharray="{}""#,
            if lstyle.contains("DOT") { "1,1.5" } else { "3,1.5" }
        )
    } else {
        String::new()
    };
    let stroke_attr = if has_stroke {
        format!(
            r#" stroke="{}" stroke-width="{:.2}"{}"#,
            escape_xml(stroke_col),
            stroke_w,
            dash
        )
    } else {
        String::new()
    };

    let fill_brush = find_child(el, "fillBrush");
    let win_brush = fill_brush.and_then(|fb| find_child(fb, "winBrush"));
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

    let body = match tag {
        "rect" => Some(rect_body(el, sx, sy, w, h, &fill_attr, &stroke_attr)),
        "ellipse" => Some(ellipse_body(w, h, &fill_attr, &stroke_attr)),
        "line" => Some(line_body(el, sx, sy, stroke_col, stroke_w, &dash)),
        "polygon" => polygon_body(el, sx, sy, &fill_attr, &stroke_attr),
        "curv" => curv_body(el, sx, sy, &fill_attr, &stroke_attr)
            .or_else(|| polygon_body(el, sx, sy, &fill_attr, &stroke_attr)),
        "arc" => arc_path_body(el, sx, sy, &fill_attr, &stroke_attr, stroke_col)
            .or_else(|| Some(arc_body(w, h, &stroke_attr, stroke_col))),
        _ => None,
    }?;

    // Rotation: only `hp:rotationInfo/@angle` is honored (M1). The full
    // `hp:renderingInfo` transform/scale/rotation matrices (`hc:transMatrix`
    // / `hc:scaMatrix` / `hc:rotMatrix`) are M2 — sample corpus inspection
    // (see P2 plan) found `angle="0"` dominant, so this covers the common
    // case without pulling in matrix decomposition.
    //
    // ASSUMPTION (unverified — the corpus has no non-zero-angle sample):
    // `angle` is taken as plain degrees, matching typical XML angle
    // attributes. If real HWPX files instead store degrees×100 (the
    // convention `hwp::record`'s binary SHAPE_COMPONENT rotation uses),
    // this needs `/ 100.0` added before use.
    let angle: f64 = find_child(el, "rotationInfo")
        .and_then(|r| r.attribute("angle"))
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    let body = if angle != 0.0 {
        format!(
            r#"<g transform="rotate({} {} {})">{}</g>"#,
            fmt_angle(angle),
            pt(w / 2.0),
            pt(h / 2.0),
            body
        )
    } else {
        body
    };

    let dt = find_child(el, "drawText");
    let has_drawtext = dt.is_some();
    let alt = match dt.map(collect_drawtext) {
        Some(text) if !text.trim().is_empty() => {
            let summary: String = text.trim().chars().take(60).collect();
            format!("{tag} shape ({summary})")
        }
        _ => format!("{tag} shape"),
    };

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}pt" height="{h}pt">{body}</svg>"#,
        w = pt(w),
        h = pt(h),
        body = body
    );

    Some(ShapeSvg {
        svg,
        width_pt: hwpunit_to_pt(w),
        height_pt: hwpunit_to_pt(h),
        alt,
        has_drawtext,
    })
}

// ─── shape body renderers ──────────────────────────────

fn rect_body(el: Node, sx: f64, sy: f64, w: f64, h: f64, fill_attr: &str, stroke_attr: &str) -> String {
    // Improvement over hwpx_render's `<rect x y width height>` simplification:
    // use the actual `hc:pt0`..`hc:pt3` corner points (scaled into curSz
    // space) as a 4-point polygon. Falls back to an axis-aligned box when a
    // rect lacks the corner points (older documents / hand-written fixtures).
    let pts: Vec<String> = ["pt0", "pt1", "pt2", "pt3"]
        .iter()
        .filter_map(|name| find_child(el, name))
        .map(|p| {
            format!(
                "{},{}",
                pt(num(Some(p), "x", 0.0) * sx),
                pt(num(Some(p), "y", 0.0) * sy)
            )
        })
        .collect();
    if pts.len() == 4 {
        format!(r#"<polygon points="{}"{}{}/>"#, pts.join(" "), fill_attr, stroke_attr)
    } else {
        format!(
            r#"<rect x="0" y="0" width="{}" height="{}"{}{}/>"#,
            pt(w),
            pt(h),
            fill_attr,
            stroke_attr
        )
    }
}

fn ellipse_body(w: f64, h: f64, fill_attr: &str, stroke_attr: &str) -> String {
    format!(
        r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}"{}{}/>"#,
        pt(w / 2.0),
        pt(h / 2.0),
        pt(w / 2.0),
        pt(h / 2.0),
        fill_attr,
        stroke_attr
    )
}

fn line_body(el: Node, sx: f64, sy: f64, stroke_col: &str, stroke_w: f64, dash: &str) -> String {
    let s = find_child(el, "startPt");
    let e = find_child(el, "endPt");
    let x1 = num(s, "x", 0.0) * sx;
    let y1 = num(s, "y", 0.0) * sy;
    let x2 = num(e, "x", 0.0) * sx;
    let y2 = num(e, "y", 0.0) * sy;
    let sw = if stroke_w > 0.0 { stroke_w } else { 0.3 };
    format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{:.2}"{}/>"#,
        pt(x1),
        pt(y1),
        pt(x2),
        pt(y2),
        escape_xml(stroke_col),
        sw,
        dash
    )
}

/// `polygon` point-list geometry. Also the fallback for `curv` when no
/// `hp:seg` children exist (control points then render as their
/// straight-line hull — the M1 behavior; real bezier geometry lives in
/// `curv_body`).
fn polygon_body(el: Node, sx: f64, sy: f64, fill_attr: &str, stroke_attr: &str) -> Option<String> {
    let mut pts: Vec<String> = Vec::new();
    for c in el.children().filter(|c| c.is_element()) {
        if local_name(&c) == "pt" {
            pts.push(format!(
                "{},{}",
                pt(num(Some(c), "x", 0.0) * sx),
                pt(num(Some(c), "y", 0.0) * sy)
            ));
        }
    }
    if pts.len() >= 2 {
        Some(format!(r#"<polygon points="{}"{}{}/>"#, pts.join(" "), fill_attr, stroke_attr))
    } else {
        None
    }
}

/// `curv` real geometry (P2-M2): the `hc:pt` list already interleaves control
/// points; each `hp:seg` describes how many points the next segment consumes —
/// CURVE = 3 (`C ctrl1 ctrl2 end`), LINE = 1 (`L end`). Port of rhwp
/// `shape_layout.rs::curve_to_path_commands_scaled`.
///
/// ASSUMPTION (unverified — the corpus has no curv sample, mirroring the
/// `rotationInfo` precedent above): segments appear as `<hp:seg type="…">`
/// children where type is `"CURVE"`/`"1"` for bezier and anything else for
/// line. When no `seg` children exist at all we return None so the caller
/// falls back to the M1 straight-hull rendering.
fn curv_body(el: Node, sx: f64, sy: f64, fill_attr: &str, stroke_attr: &str) -> Option<String> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut segs: Vec<bool> = Vec::new(); // true = bezier
    for c in el.children().filter(|c| c.is_element()) {
        match local_name(&c) {
            "pt" => pts.push((num(Some(c), "x", 0.0) * sx, num(Some(c), "y", 0.0) * sy)),
            "seg" => {
                let t = c.attribute("type").unwrap_or("");
                segs.push(t.eq_ignore_ascii_case("curve") || t == "1");
            }
            _ => {}
        }
    }
    if segs.is_empty() || pts.len() < 2 {
        return None;
    }
    let mut d = format!("M {} {}", pt(pts[0].0), pt(pts[0].1));
    let mut i = 1;
    let mut seg_idx = 0;
    while i < pts.len() {
        let bezier = segs.get(seg_idx).copied().unwrap_or(false);
        if bezier && i + 2 < pts.len() {
            d.push_str(&format!(
                " C {} {} {} {} {} {}",
                pt(pts[i].0),
                pt(pts[i].1),
                pt(pts[i + 1].0),
                pt(pts[i + 1].1),
                pt(pts[i + 2].0),
                pt(pts[i + 2].1)
            ));
            i += 3;
        } else {
            d.push_str(&format!(" L {} {}", pt(pts[i].0), pt(pts[i].1)));
            i += 1;
        }
        seg_idx += 1;
    }
    Some(format!(r#"<path d="{}"{}{}/>"#, d, fill_attr, stroke_attr))
}

/// `arc` real geometry (P2-M2): `hc:center`/`hc:ax1`/`hc:ax2` describe the
/// ellipse center and the two arc endpoints; the sweep runs axis1→axis2 with
/// sweep=0 (SVG Y-down counter-clockwise), large_arc from the angular span.
/// `@arcType` closes the path: 1/PIE = sector (`L center Z`), 2/CHORD = bow
/// (`Z`), otherwise an open arc. Port of rhwp `shape_layout.rs` Arc branch.
/// Returns None (→ M1 full-ellipse fallback) when `hc:center` is absent.
fn arc_path_body(
    el: Node,
    sx: f64,
    sy: f64,
    fill_attr: &str,
    stroke_attr: &str,
    stroke_col: &str,
) -> Option<String> {
    let center = find_child(el, "center")?;
    let ax1 = find_child(el, "ax1")?;
    let ax2 = find_child(el, "ax2")?;
    let cx = num(Some(center), "x", 0.0) * sx;
    let cy = num(Some(center), "y", 0.0) * sy;
    let (x1, y1) = (num(Some(ax1), "x", 0.0) * sx, num(Some(ax1), "y", 0.0) * sy);
    let (x2, y2) = (num(Some(ax2), "x", 0.0) * sx, num(Some(ax2), "y", 0.0) * sy);

    let (dx1, dy1) = (x1 - cx, y1 - cy);
    let (dx2, dy2) = (x2 - cx, y2 - cy);
    let r1 = (dx1 * dx1 + dy1 * dy1).sqrt();
    let r2 = (dx2 * dx2 + dy2 * dy2).sqrt();
    if r1 <= 0.1 || r2 <= 0.1 {
        return None;
    }
    let a1 = dy1.atan2(dx1);
    let a2 = dy2.atan2(dx2);
    let (rx, ry) = {
        let a1_abs = a1.abs();
        let a2_abs = a2.abs();
        if (a1_abs - std::f64::consts::FRAC_PI_2).abs() < 0.3 && a2_abs < 0.3 {
            (r2, r1)
        } else if a1_abs < 0.3 && (a2_abs - std::f64::consts::FRAC_PI_2).abs() < 0.3 {
            (r1, r2)
        } else {
            (r1.max(r2), r1.min(r2))
        }
    };
    let mut sweep = a1 - a2;
    if sweep < 0.0 {
        sweep += 2.0 * std::f64::consts::PI;
    }
    let large_arc = i32::from(sweep > std::f64::consts::PI);

    let mut d = format!(
        "M {} {} A {} {} 0 {} 0 {} {}",
        pt(x1),
        pt(y1),
        pt(rx),
        pt(ry),
        large_arc,
        pt(x2),
        pt(y2)
    );
    let arc_type = el.attribute("arcType").unwrap_or("0");
    match arc_type {
        "1" | "PIE" | "pie" | "CIRCULARSECTOR" => {
            d.push_str(&format!(" L {} {} Z", pt(cx), pt(cy)));
        }
        "2" | "CHORD" | "chord" | "BOW" | "bow" => d.push_str(" Z"),
        _ => {}
    }
    let sa = if stroke_attr.is_empty() {
        format!(r#" stroke="{}" stroke-width="0.3""#, escape_xml(stroke_col))
    } else {
        stroke_attr.to_string()
    };
    // 열린 호는 채우면 시각적으로 왜곡되므로 닫힌 타입에만 fill 적용
    let fa = if arc_type == "0" || arc_type.eq_ignore_ascii_case("arc") {
        r#" fill="none""#
    } else {
        fill_attr
    };
    Some(format!(r#"<path d="{}"{}{}/>"#, d, fa, sa))
}

/// `arc` fallback: approximated as a full unfilled ellipse (matches
/// `hwpx_render`) when `hc:center`/`hc:ax1`/`hc:ax2` are absent.
fn arc_body(w: f64, h: f64, stroke_attr: &str, stroke_col: &str) -> String {
    let sa = if !stroke_attr.is_empty() {
        stroke_attr.to_string()
    } else {
        format!(r#" stroke="{}" stroke-width="0.3""#, escape_xml(stroke_col))
    };
    format!(
        r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}" fill="none"{}/>"#,
        pt(w / 2.0),
        pt(h / 2.0),
        pt(w / 2.0),
        pt(h / 2.0),
        sa
    )
}

/// Recursively collect every `hp:t` text run under an `hp:drawText` subtree,
/// joined by single spaces. Used only for the `alt` summary — the text is
/// never drawn into the SVG itself (see module docs).
fn collect_drawtext(dt: Node) -> String {
    fn walk(n: Node, out: &mut String) {
        for c in n.children() {
            if c.is_element() && local_name(&c) == "t" {
                if let Some(text) = c.text() {
                    if !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
            } else if c.is_element() {
                walk(c, out);
            }
        }
    }
    let mut out = String::new();
    walk(dt, &mut out);
    out
}

// ─── small XML/number helpers (ported from hwpx_render::dom — that module is
// private to hwpx_render and gated behind the opt-in `hwpx-render` feature,
// so this duplicates rather than importing; see module docs) ─────────

/// Local (namespace-prefix-stripped) tag name, e.g. `"rect"` for `hp:rect`.
fn local_name<'a, 'input>(n: &Node<'a, 'input>) -> &'a str {
    n.tag_name().name()
}

/// Direct child element with the given local name.
fn find_child<'a, 'input>(n: Node<'a, 'input>, name: &str) -> Option<Node<'a, 'input>> {
    n.children().find(|c| c.is_element() && local_name(c) == name)
}

/// Integer attribute value, restoring uint32-stored negatives to signed —
/// HWPX occasionally encodes negative coordinates as their two's-complement
/// uint32 representation.
fn to_int32(v: Option<&str>, fallback: f64) -> f64 {
    match v {
        None => fallback,
        Some("") => fallback,
        Some(s) => match s.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => {
                if n > 0x7fff_ffff as f64 {
                    n - 0x1_0000_0000u64 as f64
                } else {
                    n
                }
            }
            _ => fallback,
        },
    }
}

fn num(n: Option<Node>, name: &str, fallback: f64) -> f64 {
    to_int32(n.and_then(|nd| nd.attribute(name)), fallback)
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// HWPUNIT (1/7200 inch) → pt, i.e. `round(u) / 100`. This is the
/// coordinate/size unit convention (NOT the separate 1/100mm convention
/// HWP uses for line width — see [`stroke_pt`]).
fn hwpunit_to_pt(u: f64) -> f64 {
    u.round() / 100.0
}

/// HWPUNIT coordinate/size → formatted pt string. Ported verbatim from
/// `hwpx_render::svg::pt` for visual-output parity.
fn pt(u: f64) -> String {
    let n = u.round() as i64;
    let neg = n < 0;
    let a = n.abs();
    let whole = a / 100;
    let frac = a % 100;
    let sign = if neg { "-" } else { "" };
    if frac == 0 {
        format!("{sign}{whole}")
    } else if frac % 10 == 0 {
        format!("{sign}{whole}.{}", frac / 10)
    } else {
        format!("{sign}{whole}.{frac:02}")
    }
}

/// Line width, stored in 1/100mm (a different convention than coordinates —
/// see [`hwpunit_to_pt`]), → pt. Ported verbatim from
/// `hwpx_render::svg::shape_stroke_pt`.
fn stroke_pt(v: f64) -> f64 {
    ((v / 100.0) * 2.834645).max(0.2)
}

fn fmt_angle(a: f64) -> String {
    if a.fract() == 0.0 {
        format!("{}", a as i64)
    } else {
        format!("{a:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_without_points_falls_back_to_box() {
        let xml = r##"<hp:rect id="1"><hp:orgSz width="1000" height="500"/><hp:curSz width="1000" height="500"/><hp:lineShape color="#FF0000" width="33" style="SOLID"/><hc:fillBrush><hc:winBrush faceColor="#00FF00"/></hc:fillBrush></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.svg.contains(r#"<rect x="0" y="0" width="10" height="5""#), "{}", shape.svg);
        assert!(shape.svg.contains(r##"fill="#00FF00""##));
        assert!(shape.svg.contains(r##"stroke="#FF0000""##));
        assert_eq!(shape.width_pt, 10.0);
        assert_eq!(shape.height_pt, 5.0);
        assert!(!shape.has_drawtext);
        assert_eq!(shape.alt, "rect shape");
    }

    #[test]
    fn test_rect_with_points_renders_polygon() {
        let xml = r##"<hp:rect id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="2000" height="500"/><hc:pt0 x="0" y="0"/><hc:pt1 x="1000" y="0"/><hc:pt2 x="1000" y="1000"/><hc:pt3 x="0" y="1000"/></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses");
        // sx = 2000/1000 = 2, sy = 500/1000 = 0.5
        assert!(
            shape.svg.contains(r#"<polygon points="0,0 20,0 20,5 0,5""#),
            "{}",
            shape.svg
        );
        assert_eq!(shape.width_pt, 20.0);
        assert_eq!(shape.height_pt, 5.0);
    }

    #[test]
    fn test_ellipse() {
        let xml = r##"<hp:ellipse id="1"><hp:orgSz width="2000" height="1000"/><hp:curSz width="2000" height="1000"/><hp:lineShape color="#000000" width="33" style="SOLID"/></hp:ellipse>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r#"<ellipse cx="10" cy="5" rx="10" ry="5""#),
            "{}",
            shape.svg
        );
        assert_eq!(shape.alt, "ellipse shape");
    }

    #[test]
    fn test_line() {
        let xml = r##"<hp:line id="1"><hp:orgSz width="1000" height="0"/><hp:curSz width="1000" height="0"/><hp:startPt x="0" y="0"/><hp:endPt x="1000" y="0"/><hp:lineShape color="#0000FF" width="100" style="SOLID"/></hp:line>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r##"<line x1="0" y1="0" x2="10" y2="0" stroke="#0000FF""##),
            "{}",
            shape.svg
        );
    }

    #[test]
    fn test_polygon() {
        let xml = r##"<hp:polygon id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hc:pt x="0" y="0"/><hc:pt x="1000" y="0"/><hc:pt x="500" y="1000"/></hp:polygon>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r#"<polygon points="0,0 10,0 5,10""#),
            "{}",
            shape.svg
        );
    }

    #[test]
    fn test_curv_renders_as_straight_line_polygon() {
        // seg 자식이 없으면 M1 직선 헐 폴백 유지
        let xml = r##"<hp:curv id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hc:pt x="0" y="0"/><hc:pt x="1000" y="1000"/></hp:curv>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.svg.contains(r#"<polygon points="0,0 10,10""#), "{}", shape.svg);
    }

    #[test]
    fn test_curv_bezier_segments_render_as_cubic_path() {
        // CURVE seg = 점 3개 소비 (ctrl1, ctrl2, end) — P2-M2 실기하
        let xml = r##"<hp:curv id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hc:pt x="0" y="0"/><hc:pt x="300" y="0"/><hc:pt x="700" y="1000"/><hc:pt x="1000" y="1000"/><hp:seg type="CURVE"/></hp:curv>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r#"<path d="M 0 0 C 3 0 7 10 10 10""#),
            "{}",
            shape.svg
        );
    }

    #[test]
    fn test_curv_mixed_line_and_curve_segments() {
        // LINE seg = 점 1개 소비, 이후 CURVE seg = 점 3개 소비
        let xml = r##"<hp:curv id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hc:pt x="0" y="0"/><hc:pt x="500" y="0"/><hc:pt x="600" y="0"/><hc:pt x="900" y="1000"/><hc:pt x="1000" y="1000"/><hp:seg type="LINE"/><hp:seg type="CURVE"/></hp:curv>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r#"d="M 0 0 L 5 0 C 6 0 9 10 10 10""#),
            "{}",
            shape.svg
        );
    }

    #[test]
    fn test_arc_renders_as_ellipse_approximation() {
        // center/ax1/ax2 부재 시 M1 타원 폴백 유지
        let xml = r##"<hp:arc id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hp:lineShape color="#000000" width="33" style="SOLID"/></hp:arc>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(
            shape.svg.contains(r#"<ellipse cx="5" cy="5" rx="5" ry="5" fill="none""#),
            "{}",
            shape.svg
        );
    }

    #[test]
    fn test_arc_real_geometry_open_arc() {
        // ax1=위(90°), ax2=오른쪽(0°) — 분리 규칙에 따라 rx=r2, ry=r1.
        // 열린 호(arcType 기본 0): A 커맨드 + fill=none, Z 없음.
        let xml = r##"<hp:arc id="1"><hp:orgSz width="2000" height="1000"/><hp:curSz width="2000" height="1000"/><hc:center x="1000" y="500"/><hc:ax1 x="1000" y="1000"/><hc:ax2 x="2000" y="500"/></hp:arc>"##;
        let shape = shape_to_svg(xml).expect("parses");
        // 90° 스윕(ax1=아래 90° → ax2=오른쪽 0°)이므로 large_arc=0
        assert!(
            shape.svg.contains(r#"<path d="M 10 10 A 10 5 0 0 0 20 5""#),
            "{}",
            shape.svg
        );
        assert!(shape.svg.contains(r#"fill="none""#), "{}", shape.svg);
        assert!(!shape.svg.contains('Z'), "open arc must not close: {}", shape.svg);
    }

    #[test]
    fn test_arc_pie_closes_through_center() {
        let xml = r##"<hp:arc id="1" arcType="1"><hp:orgSz width="2000" height="1000"/><hp:curSz width="2000" height="1000"/><hc:center x="1000" y="500"/><hc:ax1 x="1000" y="1000"/><hc:ax2 x="2000" y="500"/></hp:arc>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.svg.contains(" L 10 5 Z"), "pie must close via center: {}", shape.svg);
    }

    #[test]
    fn test_arc_chord_closes_directly() {
        let xml = r##"<hp:arc id="1" arcType="CHORD"><hp:orgSz width="2000" height="1000"/><hp:curSz width="2000" height="1000"/><hc:center x="1000" y="500"/><hc:ax1 x="1000" y="1000"/><hc:ax2 x="2000" y="500"/></hp:arc>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.svg.contains("Z\""), "chord must close: {}", shape.svg);
        assert!(!shape.svg.contains(" L 10 5 Z"), "chord must not pass center: {}", shape.svg);
    }

    #[test]
    fn test_rotation_angle_applied() {
        let xml = r##"<hp:rect id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hp:rotationInfo angle="45" centerX="500" centerY="500"/></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.svg.contains(r#"<g transform="rotate(45 5 5)">"#), "{}", shape.svg);
    }

    #[test]
    fn test_no_rotation_when_angle_zero() {
        let xml = r##"<hp:rect id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hp:rotationInfo angle="0" centerX="500" centerY="500"/></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(!shape.svg.contains("<g transform"), "{}", shape.svg);
    }

    #[test]
    fn test_drawtext_summarized_in_alt_not_drawn_in_svg() {
        let xml = r##"<hp:rect id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hp:drawText><hp:subList><hp:p><hp:run><hp:t>Hello</hp:t></hp:run><hp:run><hp:t>World</hp:t></hp:run></hp:p></hp:subList></hp:drawText></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses");
        assert!(shape.has_drawtext);
        assert_eq!(shape.alt, "rect shape (Hello World)");
        assert!(!shape.svg.contains("Hello"), "SVG must not embed drawText: {}", shape.svg);
        assert!(!shape.svg.contains("<text"), "{}", shape.svg);
    }

    #[test]
    fn test_unrecognized_tag_returns_none() {
        let xml = r##"<hp:table id="1"><hp:orgSz width="1000" height="1000"/></hp:table>"##;
        assert!(shape_to_svg(xml).is_none());
    }

    #[test]
    fn test_malformed_xml_returns_none() {
        assert!(shape_to_svg("<hp:rect not even valid").is_none());
    }

    #[test]
    fn test_polygon_with_one_point_returns_none() {
        let xml = r##"<hp:polygon id="1"><hp:orgSz width="1000" height="1000"/><hp:curSz width="1000" height="1000"/><hc:pt x="0" y="0"/></hp:polygon>"##;
        assert!(shape_to_svg(xml).is_none());
    }

    /// Real `hp:rect` fragment captured from
    /// `samples/input/2026년 제1기 행정안전부 청년인턴 채용 공고(최종).hwpx`
    /// (Contents/section0.xml) — a title-banner textbox, the dominant
    /// real-world pattern per the P2 plan's corpus survey (all 7 `hp:rect`
    /// occurrences in that document are drawText banners).
    #[test]
    fn test_real_mois_hwpx_rect_fragment() {
        let xml = r##"<hp:rect id="1151124716" zOrder="2" numberingType="PICTURE" textWrap="TOP_AND_BOTTOM" textFlow="BOTH_SIDES" lock="0" dropcapstyle="None" href="" groupLevel="0" instid="77382893" ratio="0"><hp:offset x="932" y="0"/><hp:orgSz width="2835" height="2835"/><hp:curSz width="72972" height="2502"/><hp:flip horizontal="0" vertical="0"/><hp:rotationInfo angle="0" centerX="36486" centerY="1251" rotateimage="1"/><hp:renderingInfo><hc:transMatrix e1="1" e2="0" e3="932" e4="0" e5="1" e6="0"/><hc:scaMatrix e1="25.739683" e2="0" e3="-932" e4="0" e5="0.88254" e6="0"/><hc:rotMatrix e1="1" e2="0" e3="0" e4="0" e5="1" e6="0"/></hp:renderingInfo><hp:lineShape color="#000000" width="33" style="SOLID" endCap="FLAT" headStyle="NORMAL" tailStyle="NORMAL" headfill="1" tailfill="1" headSz="MEDIUM_MEDIUM" tailSz="MEDIUM_MEDIUM" outlineStyle="NORMAL" alpha="0"/><hc:fillBrush><hc:winBrush faceColor="#364878" hatchColor="#000000" alpha="0"/></hc:fillBrush><hp:shadow type="NONE" color="#B2B2B2" offsetX="0" offsetY="0" alpha="0"/><hp:drawText lastWidth="72972" name="" editable="0"><hp:subList id="" textDirection="HORIZONTAL" lineWrap="BREAK" vertAlign="CENTER" linkListIDRef="0" linkListNextIDRef="0" textWidth="0" textHeight="0" hasTextRef="0" hasNumRef="0"><hp:p id="2147483648" paraPrIDRef="34" styleIDRef="0" pageBreak="0" columnBreak="0" merged="0"><hp:run charPrIDRef="42"><hp:t> </hp:t></hp:run><hp:run charPrIDRef="43"><hp:t>1. 선발예정인원 (총 114명)</hp:t></hp:run><hp:run charPrIDRef="44"/><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1600" textheight="1600" baseline="1360" spacing="160" horzpos="0" horzsize="72404" flags="393216"/></hp:linesegarray></hp:p></hp:subList><hp:textMargin left="283" right="283" top="283" bottom="283"/></hp:drawText><hc:pt0 x="0" y="0"/><hc:pt1 x="2835" y="0"/><hc:pt2 x="2835" y="2835"/><hc:pt3 x="0" y="2835"/><hp:sz width="72972" widthRelTo="ABSOLUTE" height="2502" heightRelTo="ABSOLUTE" protect="0"/><hp:pos treatAsChar="1" affectLSpacing="0" flowWithText="1" allowOverlap="0" holdAnchorAndSO="0" vertRelTo="PARA" horzRelTo="PARA" vertAlign="TOP" horzAlign="LEFT" vertOffset="0" horzOffset="0"/><hp:outMargin left="0" right="0" top="0" bottom="0"/></hp:rect>"##;
        let shape = shape_to_svg(xml).expect("parses real-world fragment");
        assert!(shape.has_drawtext);
        assert_eq!(shape.alt, "rect shape (1. 선발예정인원 (총 114명))");
        // pt0..pt3 scaled by sx=72972/2835, sy=2502/2835 land exactly on the
        // curSz bounding box after rounding.
        assert!(
            shape.svg.contains(r#"<polygon points="0,0 729.72,0 729.72,25.02 0,25.02""#),
            "{}",
            shape.svg
        );
        assert!(shape.svg.contains(r##"fill="#364878""##));
        assert!(shape.svg.contains(r##"stroke="#000000""##));
        assert!(!shape.svg.contains("선발예정인원"), "drawText must not leak into svg: {}", shape.svg);
        assert_eq!(shape.width_pt, 729.72);
        assert_eq!(shape.height_pt, 25.02);
    }
}
