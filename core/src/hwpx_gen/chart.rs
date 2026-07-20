// Ported from kkdoc (MIT): src/hwpx/chart-gen.ts
//! Chart fence (```chart) → OOXML chartSpace part + `<hp:chart>` reference.
//!
//! HWPX charts are not OLE objects — they are `Chart/chartN.xml` (OOXML
//! DrawingML chartSpace) parts registered in the manifest, referenced from the
//! body via `<hp:chart chartIDRef="…">`. The chartSpace assembly and the
//! 20-type table are ported from claw-hwp (DoHyun468, MIT) via kkdoc's TS port.
//!
//! Fence syntax (colon-separated lines, order-independent):
//! ```text
//! ```chart
//! type: column          ← column|bar|line|area|pie|doughnut|scatter|radar
//!                          (+_stacked, 3d variants), defaults to column
//! cat: Q1, Q2, Q3
//! size: 120x70          ← mm, optional (default 113.8×66.1)
//! colors: #304D68, accent2   ← per-series colors (pie: per-slice), optional
//! budget: 10, 20, 30    ← any other "name: numbers" line is a data series
//! actual: 5, 15, 25
//! ```
//! ```

use lazy_static::lazy_static;
use regex::Regex;

use super::ids::escape_xml;

const HU_PER_MM: f64 = 7200.0 / 25.4;

/// One of the 20 Hancom Docs chart types (claw-hwp GT).
#[derive(Debug, Clone, Copy)]
pub struct ChartTypeSpec {
    pub el: &'static str,
    pub dir: Option<&'static str>,
    pub grp: Option<&'static str>,
    pub overlap: Option<i32>,
    pub marker: bool,
    pub scatter: bool,
    pub pie: bool,
    pub explode: bool,
    pub hole: Option<i32>,
    pub radar: bool,
}

const fn spec(el: &'static str) -> ChartTypeSpec {
    ChartTypeSpec {
        el,
        dir: None,
        grp: None,
        overlap: None,
        marker: false,
        scatter: false,
        pie: false,
        explode: false,
        hole: None,
        radar: false,
    }
}

fn chart_type(id: u32) -> ChartTypeSpec {
    match id {
        0 => ChartTypeSpec { dir: Some("col"), grp: Some("clustered"), ..spec("barChart") },
        1 => ChartTypeSpec { dir: Some("col"), grp: Some("stacked"), overlap: Some(100), ..spec("barChart") },
        2 => ChartTypeSpec { grp: Some("standard"), marker: true, ..spec("lineChart") },
        3 => ChartTypeSpec { dir: Some("bar"), grp: Some("clustered"), ..spec("barChart") },
        4 => ChartTypeSpec { dir: Some("bar"), grp: Some("stacked"), overlap: Some(100), ..spec("barChart") },
        5 => ChartTypeSpec { scatter: true, ..spec("scatterChart") },
        6 => ChartTypeSpec { pie: true, ..spec("pieChart") },
        7 => ChartTypeSpec { pie: true, explode: true, ..spec("pieChart") },
        8 => ChartTypeSpec { pie: true, hole: Some(50), ..spec("doughnutChart") },
        9 => ChartTypeSpec { grp: Some("standard"), ..spec("areaChart") },
        10 => ChartTypeSpec { grp: Some("stacked"), ..spec("areaChart") },
        11 => ChartTypeSpec { radar: true, ..spec("radarChart") },
        12 => ChartTypeSpec { dir: Some("col"), grp: Some("clustered"), ..spec("bar3DChart") },
        13 => ChartTypeSpec { dir: Some("col"), grp: Some("stacked"), overlap: Some(100), ..spec("bar3DChart") },
        14 => ChartTypeSpec { dir: Some("bar"), grp: Some("clustered"), ..spec("bar3DChart") },
        15 => ChartTypeSpec { dir: Some("bar"), grp: Some("stacked"), overlap: Some(100), ..spec("bar3DChart") },
        16 => ChartTypeSpec { pie: true, ..spec("pie3DChart") },
        17 => ChartTypeSpec { pie: true, explode: true, ..spec("pie3DChart") },
        18 => ChartTypeSpec { grp: Some("standard"), ..spec("area3DChart") },
        19 => ChartTypeSpec { grp: Some("stacked"), ..spec("area3DChart") },
        _ => chart_type(0),
    }
}

/// `CHART_ALIAS` — Korean/English chart-type aliases → numeric id.
fn chart_alias(lower: &str) -> Option<u32> {
    match lower {
        "column" | "col" | "세로막대" | "막대" => Some(0),
        "column_stacked" | "세로막대_누적" => Some(1),
        "line" | "선" | "꺾은선" => Some(2),
        "bar" | "가로막대" => Some(3),
        "bar_stacked" => Some(4),
        "scatter" | "분산" => Some(5),
        "pie" | "원" | "파이" => Some(6),
        "pie_explode" => Some(7),
        "doughnut" | "donut" | "도넛" => Some(8),
        "area" | "영역" => Some(9),
        "area_stacked" => Some(10),
        "radar" | "방사형" => Some(11),
        "bar3d" | "column3d" => Some(12),
        "pie3d" => Some(16),
        _ => None,
    }
}

fn chart_spec(t: Option<&str>) -> ChartTypeSpec {
    let Some(t) = t else { return chart_type(0) };
    let lower = t.to_lowercase();
    let key = chart_alias(&lower).or_else(|| lower.parse::<u32>().ok());
    match key {
        Some(k) => chart_type(k),
        None => chart_type(0),
    }
}

/// One data series (name + values), optional per-series/per-point colors.
#[derive(Debug, Clone)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
    pub color: Option<String>,
    pub point_colors: Option<Vec<String>>,
}

/// A parsed ```chart fence.
#[derive(Debug, Clone)]
pub struct ChartFence {
    pub spec: ChartTypeSpec,
    pub cat: Vec<String>,
    pub series: Vec<ChartSeries>,
    pub width_hu: i32,
    pub height_hu: i32,
}

const RESERVED_KEYS: &[&str] = &["type", "cat", "size", "colors", "point_colors", "title"];

lazy_static! {
    static ref RE_SIZE: Regex =
        Regex::new(r"(?i)^(\d+(?:\.\d+)?)\s*[x×]\s*(\d+(?:\.\d+)?)$").unwrap();
}

fn clamp_mm(n: f64) -> f64 {
    n.max(10.0).min(500.0)
}

fn split_trim_nonempty(s: &str) -> Vec<String> {
    s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// Collapse thousands-separator commas (`1,000` → `1000`) before splitting a
/// series-data line on the real comma delimiter. Ported from the reference's
/// lookahead regex `(\d),(?=\d{3}(?:\D|$))` — Rust's `regex` crate has no
/// look-around, so this walks the original char array directly instead.
fn collapse_thousands(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(value.len());
    let mut i = 0usize;
    while i < n {
        if chars[i] == ',' && i > 0 && chars[i - 1].is_ascii_digit() {
            // lookahead: exactly 3 digits after the comma, then non-digit or end
            let end = i + 4;
            if end <= n
                && chars[i + 1..end].iter().all(|c| c.is_ascii_digit())
                && (end == n || !chars[end].is_ascii_digit())
            {
                i += 1; // drop the separator comma
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Parse a ```chart fence body. Returns `None` if no data series were found,
/// or a data line contains a non-numeric token (caller falls back to a plain
/// code block so the raw fence text stays visible rather than silently
/// dropping data).
pub fn parse_chart_fence(text: &str) -> Option<ChartFence> {
    let mut type_: Option<String> = None;
    let mut cat: Option<Vec<String>> = None;
    let mut width_mm = 32250.0 / HU_PER_MM;
    let mut height_mm = 18750.0 / HU_PER_MM;
    let mut colors: Option<Vec<String>> = None;
    let mut point_colors: Option<Vec<String>> = None;
    let mut series: Vec<(String, Vec<f64>)> = Vec::new();

    for raw_line in text.split('\n') {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(ci) = line.find([':', '：']) else { continue };
        if ci == 0 {
            continue;
        }
        let colon_len = line[ci..].chars().next().unwrap().len_utf8();
        let key = line[..ci].trim();
        let value = line[ci + colon_len..].trim();
        let key_lower = key.to_lowercase();

        if key_lower == "type" {
            type_ = Some(value.to_string());
        } else if key_lower == "cat" {
            cat = Some(split_trim_nonempty(value));
        } else if key_lower == "size" {
            if let Some(caps) = RE_SIZE.captures(value) {
                let w: f64 = caps[1].parse().unwrap_or(width_mm);
                let h: f64 = caps[2].parse().unwrap_or(height_mm);
                width_mm = clamp_mm(w);
                height_mm = clamp_mm(h);
            }
        } else if key_lower == "colors" {
            colors = Some(split_trim_nonempty(value));
        } else if key_lower == "point_colors" {
            point_colors = Some(split_trim_nonempty(value));
        } else if key_lower == "title" {
            // Chart titles are recommended as body text — ignored for compat, not an error.
        } else if !RESERVED_KEYS.contains(&key_lower.as_str()) {
            let collapsed = collapse_thousands(value);
            let segs = split_trim_nonempty(&collapsed);
            if segs.is_empty() {
                continue; // no values — Number("")===0 style [0] series is not intended
            }
            let mut nums = Vec::with_capacity(segs.len());
            let mut ok = true;
            for s in &segs {
                match s.parse::<f64>() {
                    Ok(n) if n.is_finite() => nums.push(n),
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                return None; // non-numeric token — fall back to a plain code block
            }
            series.push((key.to_string(), nums));
        }
    }

    if series.is_empty() {
        return None;
    }
    let spec = chart_spec(type_.as_deref());
    let mut final_series: Vec<ChartSeries> = if spec.pie {
        vec![series[0].clone()]
    } else {
        series.clone()
    }
    .into_iter()
    .map(|(name, values)| ChartSeries { name, values, color: None, point_colors: None })
    .collect();

    // cat label count = max(explicit cat, longest series). A series longer
    // than the labels does not get its tail truncated — labels expand to
    // "항목 N" instead, preserving data without silent loss.
    let pt_len = final_series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0)
        .max(cat.as_ref().map(|c| c.len()).unwrap_or(0));
    let cat_final: Vec<String> = (0..pt_len)
        .map(|i| cat.as_ref().and_then(|c| c.get(i)).cloned().unwrap_or_else(|| format!("항목 {}", i + 1)))
        .collect();

    // cat/values count must match (OOXML strCache/numCache ptCount contract).
    // scatter uses independent xVal/yVal axes, so it is excluded.
    if !spec.scatter {
        for s in &mut final_series {
            s.values = (0..cat_final.len()).map(|i| s.values.get(i).copied().unwrap_or(0.0)).collect();
        }
    }

    if spec.pie {
        // An explicit but empty `colors:`/`point_colors:` line should not
        // shadow the other channel if it has real values (see the non-pie
        // branch below for the same empty-vs-absent distinction).
        let slice = colors.filter(|c| !c.is_empty()).or_else(|| point_colors.filter(|c| !c.is_empty()));
        if let (Some(slice), Some(first)) = (slice, final_series.first_mut()) {
            first.point_colors = Some(slice);
        }
    } else {
        // An explicit but empty `colors:` line (e.g. "colors:\n") parses to
        // `Some(vec![])` — treat that the same as "no colors given" rather
        // than divide by zero below.
        if let Some(colors) = colors.as_ref().filter(|c| !c.is_empty()) {
            let n = colors.len();
            for (i, s) in final_series.iter_mut().enumerate() {
                s.color = colors.get(i % n).cloned();
            }
        }
        if let (Some(pc), Some(first)) = (point_colors, final_series.first_mut()) {
            first.point_colors = Some(pc);
        }
    }

    Some(ChartFence {
        spec,
        cat: cat_final,
        series: final_series,
        width_hu: (width_mm * HU_PER_MM).round() as i32,
        height_hu: (height_mm * HU_PER_MM).round() as i32,
    })
}

// ─── chartSpace OOXML assembly (claw-hwp buildChartSpace port) ───

fn col_letter(i: usize) -> char {
    (b'B' + i as u8) as char // 0 → B
}

fn str_cache_pts(vals: &[String]) -> String {
    let pts: String = vals
        .iter()
        .enumerate()
        .map(|(i, v)| format!("<c:pt idx=\"{i}\"><c:v>{}</c:v></c:pt>", escape_xml(v)))
        .collect();
    format!("<c:ptCount val=\"{}\"/>{pts}", vals.len())
}

fn num_cache_pts(vals: &[f64]) -> String {
    let pts: String = vals
        .iter()
        .enumerate()
        .map(|(i, v)| format!("<c:pt idx=\"{i}\"><c:v>{v}</c:v></c:pt>"))
        .collect();
    format!("<c:formatCode>General</c:formatCode><c:ptCount val=\"{}\"/>{pts}", vals.len())
}

/// Color → solidFill. `accent1`~`accent6` = Hancom built-in palette,
/// `#RRGGBB` = literal. Anything else is ignored.
fn chart_color_fill(color: Option<&str>) -> Option<String> {
    let c = color?.trim();
    let lower = c.to_lowercase();
    if let Some(n) = lower.strip_prefix("accent") {
        if n.len() == 1 && matches!(n.chars().next(), Some('1'..='6')) {
            return Some(format!("<a:solidFill><a:schemeClr val=\"{lower}\"/></a:solidFill>"));
        }
    }
    let hex = c.strip_prefix('#').unwrap_or(c).to_uppercase();
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(format!("<a:solidFill><a:srgbClr val=\"{hex}\"/></a:solidFill>"));
    }
    None
}

/// Line-type (line/radar) series colors go inside `<a:ln>`; area/bar series
/// use a bare solidFill (claw-hwp GT).
fn ser_sp_pr(color: Option<&str>, stroke: bool) -> String {
    match chart_color_fill(color) {
        None => "<c:spPr/>".to_string(),
        Some(f) => {
            if stroke {
                format!("<c:spPr><a:ln w=\"28575\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\">{f}<a:prstDash val=\"solid\"/><a:round/></a:ln></c:spPr>")
            } else {
                format!("<c:spPr>{f}</c:spPr>")
            }
        }
    }
}

/// Per-point (bar/slice) color overrides.
fn d_pt_xml(point_colors: Option<&[String]>, pie: bool) -> String {
    let Some(cols) = point_colors else { return String::new() };
    cols.iter()
        .enumerate()
        .filter_map(|(i, col)| {
            let f = chart_color_fill(Some(col))?;
            let mid = if pie {
                "<c:invertIfNegative val=\"0\"/><c:bubble3D val=\"0\"/><c:explosion val=\"0\"/>"
            } else {
                "<c:bubble3D val=\"0\"/>"
            };
            Some(format!("<c:dPt><c:idx val=\"{i}\"/>{mid}<c:spPr>{f}</c:spPr></c:dPt>"))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn std_ser(
    idx: usize,
    name: &str,
    cat: &[String],
    values: &[f64],
    explode: bool,
    color: Option<&str>,
    point_colors: Option<&[String]>,
    stroke: bool,
    pie: bool,
) -> String {
    let cl = col_letter(idx);
    format!(
        "<c:ser><c:idx val=\"{idx}\"/><c:order val=\"{idx}\"/>\
        <c:tx><c:strRef><c:f>Sheet1!${cl}$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>\
        {sp}<c:invertIfNegative val=\"0\"/>{explosion}{dpt}\
        <c:cat><c:strRef><c:f>Sheet1!$A$2:$A${cat_last}</c:f><c:strCache>{cat_cache}</c:strCache></c:strRef></c:cat>\
        <c:val><c:numRef><c:f>Sheet1!${cl}$2:${cl}${val_last}</c:f><c:numCache>{val_cache}</c:numCache></c:numRef></c:val>\
        </c:ser>",
        idx = idx,
        cl = cl,
        name = escape_xml(name),
        sp = ser_sp_pr(color, stroke),
        explosion = if explode { "<c:explosion val=\"25\"/>" } else { "" },
        dpt = d_pt_xml(point_colors, pie),
        cat_last = cat.len() + 1,
        cat_cache = str_cache_pts(cat),
        val_last = values.len() + 1,
        val_cache = num_cache_pts(values),
    )
}

fn scatter_ser(idx: usize, name: &str, xvals: &[f64], yvals: &[f64]) -> String {
    let cl = col_letter(idx);
    format!(
        "<c:ser><c:idx val=\"{idx}\"/><c:order val=\"{idx}\"/>\
        <c:tx><c:strRef><c:f>Sheet1!${cl}$1</c:f><c:strCache><c:ptCount val=\"1\"/><c:pt idx=\"0\"><c:v>{name}</c:v></c:pt></c:strCache></c:strRef></c:tx>\
        <c:spPr><a:ln w=\"28575\"><a:noFill/></a:ln></c:spPr><c:marker><c:symbol val=\"circle\"/><c:size val=\"7\"/></c:marker>\
        <c:xVal><c:numRef><c:f>Sheet1!$A$2:$A${x_last}</c:f><c:numCache>{x_cache}</c:numCache></c:numRef></c:xVal>\
        <c:yVal><c:numRef><c:f>Sheet1!${cl}$2:${cl}${y_last}</c:f><c:numCache>{y_cache}</c:numCache></c:numRef></c:yVal>\
        </c:ser>",
        idx = idx,
        cl = cl,
        name = escape_xml(name),
        x_last = xvals.len() + 1,
        x_cache = num_cache_pts(xvals),
        y_last = yvals.len() + 1,
        y_cache = num_cache_pts(yvals),
    )
}

fn cat_ax_xml(id: &str, pos: &str, cross: &str) -> String {
    format!(
        "<c:catAx><c:axId val=\"{id}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"{pos}\"/><c:crossAx val=\"{cross}\"/><c:delete val=\"0\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crosses val=\"autoZero\"/><c:auto val=\"1\"/><c:lblAlgn val=\"ctr\"/><c:lblOffset val=\"100\"/><c:noMultiLvlLbl val=\"0\"/></c:catAx>"
    )
}

fn val_ax_xml(id: &str, pos: &str, cross: &str) -> String {
    format!(
        "<c:valAx><c:axId val=\"{id}\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:axPos val=\"{pos}\"/><c:majorGridlines/><c:numFmt formatCode=\"General\" sourceLinked=\"1\"/><c:crossAx val=\"{cross}\"/><c:delete val=\"0\"/><c:majorTickMark val=\"out\"/><c:minorTickMark val=\"none\"/><c:tickLblPos val=\"nextTo\"/><c:crosses val=\"autoZero\"/><c:crossBetween val=\"between\"/></c:valAx>"
    )
}

/// Chart fence → chartSpace XML (full document, OOXML DrawingML chart namespace).
pub fn build_chart_space_xml(fence: &ChartFence) -> String {
    let spec = &fence.spec;
    let cat = &fence.cat;
    let series = &fence.series;
    const NS: &str = "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\"";
    let ax1 = "111111111";
    let ax2 = "222222222";

    let plot = if spec.scatter {
        let n = series.iter().map(|s| s.values.len()).max().unwrap_or(0);
        let xs: Vec<f64> = (0..n)
            .map(|i| {
                cat.get(i)
                    .and_then(|c| if c.is_empty() { None } else { c.parse::<f64>().ok() })
                    .filter(|v| v.is_finite())
                    .unwrap_or((i + 1) as f64)
            })
            .collect();
        let sers: String =
            series.iter().enumerate().map(|(i, s)| scatter_ser(i, &s.name, &xs, &s.values)).collect();
        format!(
            "<c:scatterChart><c:scatterStyle val=\"lineMarker\"/><c:varyColors val=\"0\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:scatterChart>{}{}",
            val_ax_xml(ax1, "b", ax2),
            val_ax_xml(ax2, "l", ax1),
        )
    } else if spec.pie {
        let s0 = &series[0];
        let hole = spec.hole.map(|h| format!("<c:holeSize val=\"{h}\"/>")).unwrap_or_default();
        format!(
            "<c:{el}><c:varyColors val=\"1\"/>{ser}<c:firstSliceAng val=\"0\"/>{hole}</c:{el}>",
            el = spec.el,
            ser = std_ser(0, &s0.name, cat, &s0.values, spec.explode, s0.color.as_deref(), s0.point_colors.as_deref(), false, true),
        )
    } else {
        let stroke = spec.el == "lineChart" || spec.el == "radarChart" || spec.radar;
        let sers: String = series
            .iter()
            .enumerate()
            .map(|(i, s)| std_ser(i, &s.name, cat, &s.values, false, s.color.as_deref(), s.point_colors.as_deref(), stroke, false))
            .collect();
        let horiz = spec.dir == Some("bar");
        let mut inner = String::new();
        if let Some(dir) = spec.dir {
            inner.push_str(&format!("<c:barDir val=\"{dir}\"/>"));
        }
        if let Some(grp) = spec.grp {
            inner.push_str(&format!("<c:grouping val=\"{grp}\"/>"));
        }
        if spec.radar {
            inner.push_str("<c:radarStyle val=\"standard\"/>");
        }
        inner.push_str(&format!("<c:varyColors val=\"0\"/>{sers}"));
        if spec.marker {
            inner.push_str("<c:marker val=\"1\"/>");
        }
        if spec.el.starts_with("bar") {
            inner.push_str(&format!("<c:gapWidth val=\"150\"/><c:overlap val=\"{}\"/>", spec.overlap.unwrap_or(0)));
        }
        inner.push_str(&format!("<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/>"));
        format!(
            "<c:{el}>{inner}</c:{el}>{cat_ax}{val_ax}",
            el = spec.el,
            cat_ax = cat_ax_xml(ax1, if horiz { "l" } else { "b" }, ax2),
            val_ax = val_ax_xml(ax2, if horiz { "b" } else { "l" }, ax1),
        )
    };

    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\" ?>\
        <c:chartSpace {NS}><c:date1904 val=\"0\"/><c:roundedCorners val=\"0\"/>\
        <c:chart><c:autoTitleDeleted val=\"0\"/><c:plotArea><c:layout/>{plot}</c:plotArea>\
        <c:legend><c:legendPos val=\"r\"/><c:overlay val=\"0\"/></c:legend>\
        <c:plotVisOnly val=\"1\"/><c:dispBlanksAs val=\"gap\"/></c:chart></c:chartSpace>"
    )
}

/// Body `<hp:chart>` — treated as a character (treatAsChar=1), so it sits
/// exactly at the insertion point and does not float to another page
/// (claw-hwp GT convention). `chartIDRef` is the full ZIP part path.
pub fn build_chart_element_xml(part_name: &str, width_hu: i32, height_hu: i32, id: u64) -> String {
    format!(
        "<hp:chart id=\"{id}\" zOrder=\"0\" numberingType=\"PICTURE\" textWrap=\"TOP_AND_BOTTOM\" textFlow=\"BOTH_SIDES\" lock=\"0\" dropcapstyle=\"None\" chartIDRef=\"{part}\">\
        <hp:sz width=\"{width_hu}\" widthRelTo=\"ABSOLUTE\" height=\"{height_hu}\" heightRelTo=\"ABSOLUTE\" protect=\"0\"/>\
        <hp:pos treatAsChar=\"1\" affectLSpacing=\"0\" flowWithText=\"1\" allowOverlap=\"0\" holdAnchorAndSO=\"0\" vertRelTo=\"PARA\" horzRelTo=\"PARA\" vertAlign=\"TOP\" horzAlign=\"LEFT\" vertOffset=\"0\" horzOffset=\"0\"/>\
        <hp:outMargin left=\"709\" right=\"709\" top=\"709\" bottom=\"709\"/></hp:chart>",
        part = escape_xml(part_name),
    )
}

// ─── Registry — assigns Chart/chartN.xml parts + <hp:chart> ids ───

/// One registered chart part (ZIP path + chartSpace XML bytes).
#[derive(Debug, Clone)]
pub struct ChartPart {
    pub name: String,
    pub xml: String,
}

/// Per-document chart registry — assigns `Chart/chart{N}.xml` parts (1-based)
/// and `<hp:chart>` object ids (`9_100_000 + idx`, 0-based).
#[derive(Debug, Default)]
pub struct ChartRegistry {
    pub parts: Vec<ChartPart>,
}

impl ChartRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a parsed chart fence; returns the inline `<hp:chart>` XML to
    /// embed in the run stream.
    pub fn register(&mut self, fence: &ChartFence) -> String {
        let idx = self.parts.len();
        let part_name = format!("Chart/chart{}.xml", idx + 1);
        let xml = build_chart_space_xml(fence);
        self.parts.push(ChartPart { name: part_name.clone(), xml });
        build_chart_element_xml(&part_name, fence.width_hu, fence.height_hu, 9_100_000 + idx as u64)
    }

    /// `<opf:item>` manifest fragments (media-type="application/xml").
    pub fn manifest_items(&self) -> Vec<String> {
        self.parts
            .iter()
            .enumerate()
            .map(|(i, p)| format!("<opf:item id=\"chart{}\" href=\"{}\" media-type=\"application/xml\"/>", i + 1, p.name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_column_chart() {
        let text = "type: column\ncat: 1분기, 2분기, 3분기\n예산: 10, 20, 30\n집행: 5, 15, 25\n";
        let fence = parse_chart_fence(text).expect("should parse");
        assert_eq!(fence.cat, vec!["1분기", "2분기", "3분기"]);
        assert_eq!(fence.series.len(), 2);
        assert_eq!(fence.series[0].name, "예산");
        assert_eq!(fence.series[0].values, vec![10.0, 20.0, 30.0]);
        assert_eq!(fence.spec.el, "barChart");
    }

    #[test]
    fn no_series_returns_none() {
        assert!(parse_chart_fence("type: column\ncat: a, b\n").is_none());
    }

    #[test]
    fn non_numeric_token_falls_back() {
        assert!(parse_chart_fence("값: 1, two, 3\n").is_none());
    }

    #[test]
    fn thousands_separator_collapses() {
        let fence = parse_chart_fence("매출: 1,000, 2,345,678\n").unwrap();
        assert_eq!(fence.series[0].values, vec![1000.0, 2345678.0]);
    }

    #[test]
    fn empty_colors_line_does_not_panic() {
        // "colors:" with no value parses to Some(vec![]) — must not divide by
        // zero when assigning per-series colors (codex review finding).
        let fence = parse_chart_fence("colors:\nrevenue: 1, 2\n").expect("should parse");
        assert_eq!(fence.series[0].values, vec![1.0, 2.0]);
        assert!(fence.series[0].color.is_none());
    }

    #[test]
    fn empty_colors_line_does_not_shadow_point_colors_in_pie() {
        let fence = parse_chart_fence("type: pie\ncolors:\npoint_colors: #FF0000, #00FF00\ncat: A, B\nv: 1, 2\n")
            .expect("should parse");
        assert_eq!(
            fence.series[0].point_colors.as_deref(),
            Some(&["#FF0000".to_string(), "#00FF00".to_string()][..])
        );
    }

    #[test]
    fn pie_keeps_only_first_series_and_uses_point_colors() {
        let fence = parse_chart_fence("type: pie\ncat: A, B\ncolors: #FF0000, #00FF00\n값: 1, 2\n둘째: 3, 4\n").unwrap();
        assert_eq!(fence.series.len(), 1);
        assert_eq!(fence.series[0].point_colors.as_deref(), Some(&["#FF0000".to_string(), "#00FF00".to_string()][..]));
    }

    #[test]
    fn series_longer_than_cat_expands_labels_instead_of_truncating() {
        let fence = parse_chart_fence("cat: A\nv: 1, 2, 3\n").unwrap();
        assert_eq!(fence.cat, vec!["A", "항목 2", "항목 3"]);
        assert_eq!(fence.series[0].values, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn size_line_parses_and_clamps() {
        let fence = parse_chart_fence("size: 5x600\nv: 1\n").unwrap();
        // clamp(5)=10mm, clamp(600)=500mm
        let expect_w = (10.0 * HU_PER_MM).round() as i32;
        let expect_h = (500.0 * HU_PER_MM).round() as i32;
        assert_eq!(fence.width_hu, expect_w);
        assert_eq!(fence.height_hu, expect_h);
    }

    #[test]
    fn build_chart_space_xml_is_well_formed_for_each_type() {
        for (i, alias) in ["column", "bar", "line", "area", "pie", "doughnut", "scatter", "radar", "column_stacked", "pie3d"]
            .iter()
            .enumerate()
        {
            let text = format!("type: {alias}\ncat: A, B, C\n계열: 1, 2, {}\n", i + 3);
            let fence = parse_chart_fence(&text).unwrap_or_else(|| panic!("fence should parse for {alias}"));
            let xml = build_chart_space_xml(&fence);
            assert!(xml.starts_with("<?xml"), "{alias}: {xml}");
            assert!(xml.contains("<c:chartSpace"), "{alias}");
            // quick well-formedness check via quick-xml
            let mut reader = quick_xml::Reader::from_str(&xml);
            loop {
                match reader.read_event() {
                    Ok(quick_xml::events::Event::Eof) => break,
                    Ok(_) => {}
                    Err(e) => panic!("{alias} chartSpace not well-formed: {e} in {xml}"),
                }
            }
        }
    }

    #[test]
    fn chart_element_xml_uses_full_part_path_as_chart_id_ref() {
        let xml = build_chart_element_xml("Chart/chart1.xml", 32250, 18750, 9_100_000);
        assert!(xml.contains("chartIDRef=\"Chart/chart1.xml\""), "{xml}");
        assert!(xml.contains("id=\"9100000\""), "{xml}");
        assert!(xml.contains("treatAsChar=\"1\""));
    }

    #[test]
    fn registry_assigns_sequential_parts_and_ids() {
        let mut reg = ChartRegistry::new();
        let fence = parse_chart_fence("v: 1, 2\n").unwrap();
        let el1 = reg.register(&fence);
        let el2 = reg.register(&fence);
        assert!(el1.contains("Chart/chart1.xml"));
        assert!(el1.contains("id=\"9100000\""));
        assert!(el2.contains("Chart/chart2.xml"));
        assert!(el2.contains("id=\"9100001\""));
        assert_eq!(reg.parts.len(), 2);
        let items = reg.manifest_items();
        assert_eq!(items.len(), 2);
        assert!(items[0].contains("media-type=\"application/xml\""));
        assert!(items[0].contains("href=\"Chart/chart1.xml\""));
    }
}
