//! OOXML chartSpace (`c:chartSpace`) → structured data → Markdown table.
//!
//! HWPX charts are not OLE blobs: they are `Chart/chartN.xml` parts holding a
//! DrawingML `chartSpace`, referenced from the body via
//! `<hp:chart chartIDRef="…">` (see `crate::hwpx_gen::chart` for the writing
//! direction). DOCX charts use the same schema under `word/charts/chartN.xml`,
//! so this parser serves both.
//!
//! Rendering charts as pictures would defeat the point for an AI/LLM
//! pipeline — the numbers are already right there in the `c:numCache` /
//! `c:strCache` blocks, so we emit them as a Markdown data table instead
//! (P2-M4). Series names, categories and values survive as text a retrieval
//! pipeline can index, which a rasterized chart image never could.

use roxmltree::{Document, Node};

/// One data series: its name plus one value per category (`None` = the cache
/// had no point at that index, i.e. a hole in the data).
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<Option<String>>,
}

/// A parsed chart: kind label, optional title, category axis labels and
/// every series.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartData {
    /// Human-readable chart kind, e.g. `"세로 막대(묶은)"`.
    pub kind: String,
    pub title: Option<String>,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
}

fn local<'a>(n: &Node<'a, '_>) -> &'a str {
    n.tag_name().name()
}

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && local(c) == name)
}

/// Depth-first search for the first descendant with the given local name.
fn descendant<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.descendants().find(|c| c.is_element() && local(c) == name)
}

/// Read a `c:strCache` / `c:numCache` point list into an index-ordered vector.
/// `c:pt/@idx` is authoritative (points may be sparse or out of order); the
/// vector is sized from `c:ptCount` when present.
fn read_cache(cache: Node) -> Vec<Option<String>> {
    let count = child(cache, "ptCount")
        .and_then(|n| n.attribute("val"))
        .and_then(|v| v.parse::<usize>().ok());
    let mut pts: Vec<(usize, String)> = Vec::new();
    for pt in cache.children().filter(|c| c.is_element() && local(c) == "pt") {
        let idx = pt.attribute("idx").and_then(|v| v.parse::<usize>().ok()).unwrap_or(pts.len());
        let v = child(pt, "v").and_then(|n| n.text()).unwrap_or("").to_string();
        pts.push((idx, v));
    }
    let len = count.unwrap_or_else(|| pts.iter().map(|(i, _)| i + 1).max().unwrap_or(0));
    let mut out = vec![None; len];
    for (idx, v) in pts {
        if idx < out.len() {
            out[idx] = Some(v);
        } else {
            out.push(Some(v));
        }
    }
    out
}

/// `c:cat` / `c:val` / `c:tx` all wrap their data in a `*Ref` element holding
/// either a `strCache` or a `numCache` — grab whichever is present.
fn read_ref_cache(holder: Node) -> Vec<Option<String>> {
    for name in ["strCache", "numCache"] {
        if let Some(cache) = descendant(holder, name) {
            return read_cache(cache);
        }
    }
    Vec::new()
}

/// Map a chart-type element (plus `c:barDir`/`c:grouping` modifiers) to a
/// Korean label. Mirrors the 20-type table in `crate::hwpx_gen::chart`.
fn kind_label(plot_el: &str, dir: Option<&str>, grouping: Option<&str>) -> String {
    let base = match plot_el {
        "barChart" | "bar3DChart" => match dir {
            Some("bar") => "가로 막대",
            _ => "세로 막대",
        },
        "lineChart" | "line3DChart" => "꺾은선",
        "pieChart" | "pie3DChart" => "원",
        "doughnutChart" => "도넛",
        "areaChart" | "area3DChart" => "영역",
        "scatterChart" => "분산",
        "radarChart" => "방사형",
        "bubbleChart" => "거품",
        "stockChart" => "주식",
        "surfaceChart" | "surface3DChart" => "표면",
        other => other,
    };
    let group = match grouping {
        Some("stacked") => Some("누적"),
        Some("percentStacked") => Some("100% 누적"),
        Some("clustered") => Some("묶은"),
        _ => None,
    };
    let three_d = plot_el.contains("3D");
    match (group, three_d) {
        (Some(g), true) => format!("{base}({g}, 3D)"),
        (Some(g), false) => format!("{base}({g})"),
        (None, true) => format!("{base}(3D)"),
        (None, false) => base.to_string(),
    }
}

/// Collect the rich-text runs of a `c:title` into a single string.
fn read_title(chart: Node) -> Option<String> {
    let title = child(chart, "title")?;
    let mut out = String::new();
    for t in title.descendants().filter(|n| n.is_element() && local(n) == "t") {
        if let Some(text) = t.text() {
            out.push_str(text);
        }
    }
    let out = out.trim().to_string();
    (!out.is_empty()).then_some(out)
}

/// Parse an OOXML `chartSpace` part into [`ChartData`].
///
/// Returns `None` when the XML doesn't parse, has no recognizable plot area,
/// or carries no series at all — the caller then leaves the original chart
/// marker in place rather than emitting an empty table.
pub fn parse_chart_xml(xml: &str) -> Option<ChartData> {
    let doc = Document::parse(xml).ok()?;
    let root = doc.root_element();
    // `c:chartSpace` → `c:chart` → `c:plotArea`
    let chart = descendant(root, "chart")?;
    let plot = descendant(chart, "plotArea")?;

    // The plot area holds exactly one chart-type element among its children
    // (multi-plot combo charts take the first — the series list below is
    // gathered across all of them anyway).
    let plot_el = plot
        .children()
        .filter(|c| c.is_element())
        .map(|c| local(&c).to_string())
        .find(|n| n.ends_with("Chart"))?;
    let type_node = child(plot, &plot_el)?;
    let dir = child(type_node, "barDir").and_then(|n| n.attribute("val")).map(str::to_string);
    let grouping = child(type_node, "grouping").and_then(|n| n.attribute("val")).map(str::to_string);

    let mut categories: Vec<String> = Vec::new();
    let mut series: Vec<ChartSeries> = Vec::new();

    for ser in plot.descendants().filter(|n| n.is_element() && local(n) == "ser") {
        let name = child(ser, "tx")
            .map(read_ref_cache)
            .and_then(|v| v.into_iter().flatten().next())
            .unwrap_or_else(|| format!("계열 {}", series.len() + 1));

        // `c:cat` is repeated identically on every series; keep the longest.
        if let Some(cat) = child(ser, "cat") {
            let cats: Vec<String> = read_ref_cache(cat)
                .into_iter()
                .map(|c| c.unwrap_or_default())
                .collect();
            if cats.len() > categories.len() {
                categories = cats;
            }
        }
        // Scatter series use `c:yVal` instead of `c:val`.
        let values = child(ser, "val")
            .or_else(|| child(ser, "yVal"))
            .map(read_ref_cache)
            .unwrap_or_default();
        series.push(ChartSeries { name, values });
    }

    if series.is_empty() {
        return None;
    }

    Some(ChartData {
        kind: kind_label(&plot_el, dir.as_deref(), grouping.as_deref()),
        title: read_title(chart),
        categories,
        series,
    })
}

impl ChartData {
    /// Render as a GFM table: one row per category, one column per series,
    /// preceded by a caption line naming the chart kind (and title, if any).
    ///
    /// Charts with no category axis (e.g. a bare scatter series) fall back to
    /// numbering rows `1..n` so the values still land in a table.
    pub fn to_markdown_table(&self) -> String {
        let rows = self
            .series
            .iter()
            .map(|s| s.values.len())
            .chain(std::iter::once(self.categories.len()))
            .max()
            .unwrap_or(0);

        let caption = match &self.title {
            Some(t) => format!("**차트: {} ({})**", t, self.kind),
            None => format!("**차트: {}**", self.kind),
        };

        let mut out = String::new();
        out.push_str(&caption);
        out.push_str("\n\n");

        // Header
        out.push_str("| 항목 |");
        for s in &self.series {
            out.push_str(&format!(" {} |", escape_cell(&s.name)));
        }
        out.push('\n');
        out.push_str("| --- |");
        for _ in &self.series {
            out.push_str(" --- |");
        }
        out.push('\n');

        for r in 0..rows {
            let cat = self
                .categories
                .get(r)
                .filter(|c| !c.is_empty())
                .cloned()
                .unwrap_or_else(|| (r + 1).to_string());
            out.push_str(&format!("| {} |", escape_cell(&cat)));
            for s in &self.series {
                let v = s.values.get(r).and_then(Option::as_deref).unwrap_or("");
                out.push_str(&format!(" {} |", escape_cell(v)));
            }
            out.push('\n');
        }
        out
    }
}

/// Pipes and newlines would break the surrounding GFM table.
fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal chartSpace matching what `hwpx_gen::chart` writes (verified
    /// against a real generated `Chart/chart1.xml` — see the M4 fixture).
    fn sample_xml() -> String {
        r#"<?xml version="1.0" encoding="UTF-8"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
<c:chart><c:plotArea><c:layout/><c:barChart>
<c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>
<c:ser><c:idx val="0"/><c:order val="0"/>
<c:tx><c:strRef><c:f>Sheet1!$B$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>목표</c:v></c:pt></c:strCache></c:strRef></c:tx>
<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>1분기</c:v></c:pt><c:pt idx="1"><c:v>2분기</c:v></c:pt></c:strCache></c:strRef></c:cat>
<c:val><c:numRef><c:f>Sheet1!$B$2:$B$3</c:f><c:numCache><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>120</c:v></c:pt><c:pt idx="1"><c:v>150</c:v></c:pt></c:numCache></c:numRef></c:val>
</c:ser>
<c:ser><c:idx val="1"/><c:order val="1"/>
<c:tx><c:strRef><c:f>Sheet1!$C$1</c:f><c:strCache><c:ptCount val="1"/><c:pt idx="0"><c:v>실적</c:v></c:pt></c:strCache></c:strRef></c:tx>
<c:cat><c:strRef><c:f>Sheet1!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/><c:pt idx="0"><c:v>1분기</c:v></c:pt><c:pt idx="1"><c:v>2분기</c:v></c:pt></c:strCache></c:strRef></c:cat>
<c:val><c:numRef><c:f>Sheet1!$C$2:$C$3</c:f><c:numCache><c:ptCount val="2"/><c:pt idx="0"><c:v>110</c:v></c:pt><c:pt idx="1"><c:v>165</c:v></c:pt></c:numCache></c:numRef></c:val>
</c:ser>
</c:barChart></c:plotArea></c:chart></c:chartSpace>"#
            .to_string()
    }

    #[test]
    fn parses_series_categories_and_values() {
        let data = parse_chart_xml(&sample_xml()).expect("parses");
        assert_eq!(data.kind, "세로 막대(묶은)");
        assert_eq!(data.categories, vec!["1분기", "2분기"]);
        assert_eq!(data.series.len(), 2);
        assert_eq!(data.series[0].name, "목표");
        assert_eq!(
            data.series[0].values,
            vec![Some("120".into()), Some("150".into())]
        );
        assert_eq!(data.series[1].name, "실적");
        assert_eq!(
            data.series[1].values,
            vec![Some("110".into()), Some("165".into())]
        );
    }

    #[test]
    fn renders_gfm_table() {
        let data = parse_chart_xml(&sample_xml()).expect("parses");
        let md = data.to_markdown_table();
        assert!(md.starts_with("**차트: 세로 막대(묶은)**"), "{md}");
        assert!(md.contains("| 항목 | 목표 | 실적 |"), "{md}");
        assert!(md.contains("| 1분기 | 120 | 110 |"), "{md}");
        assert!(md.contains("| 2분기 | 150 | 165 |"), "{md}");
    }

    #[test]
    fn title_appears_in_caption() {
        let xml = sample_xml().replace(
            "<c:plotArea>",
            r#"<c:title><c:tx><c:rich><a:p><a:r><a:t>분기 실적</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea>"#,
        );
        let data = parse_chart_xml(&xml).expect("parses");
        assert_eq!(data.title.as_deref(), Some("분기 실적"));
        assert!(data.to_markdown_table().starts_with("**차트: 분기 실적 (세로 막대(묶은))**"));
    }

    #[test]
    fn sparse_cache_points_keep_their_index() {
        let xml = sample_xml().replace(
            r#"<c:ptCount val="2"/><c:pt idx="0"><c:v>120</c:v></c:pt><c:pt idx="1"><c:v>150</c:v></c:pt>"#,
            r#"<c:ptCount val="2"/><c:pt idx="1"><c:v>150</c:v></c:pt>"#,
        );
        let data = parse_chart_xml(&xml).expect("parses");
        assert_eq!(data.series[0].values, vec![None, Some("150".into())]);
        // A hole renders as an empty cell, not a shifted value.
        assert!(data.to_markdown_table().contains("| 1분기 |  | 110 |"));
    }

    #[test]
    fn pie_chart_label_has_no_grouping() {
        let xml = sample_xml()
            .replace("c:barChart", "c:pieChart")
            .replace(r#"<c:barDir val="col"/><c:grouping val="clustered"/>"#, "");
        let data = parse_chart_xml(&xml).expect("parses");
        assert_eq!(data.kind, "원");
    }

    #[test]
    fn garbage_and_seriesless_xml_return_none() {
        assert!(parse_chart_xml("not xml at all <<<").is_none());
        assert!(parse_chart_xml(
            r#"<c:chartSpace xmlns:c="x"><c:chart><c:plotArea><c:barChart/></c:plotArea></c:chart></c:chartSpace>"#
        )
        .is_none());
    }

    #[test]
    fn pipes_in_labels_are_escaped() {
        let xml = sample_xml().replace("<c:v>목표</c:v>", "<c:v>a|b</c:v>");
        let md = parse_chart_xml(&xml).expect("parses").to_markdown_table();
        assert!(md.contains(r"a\|b"), "{md}");
    }
}
