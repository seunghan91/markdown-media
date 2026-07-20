//! HWPX generation — Markdown → HWPX (OWPML) writer, document presets, and a
//! container validator. Companion to the read-side [`crate::hwpx`] parser.
//!
//! Ported from kkdoc (MIT): src/hwpx/generator.ts and its sibling modules.
//!
//! # Scope
//! This is a faithful port of the reference's **general** Markdown → HWPX path
//! (headings, paragraphs, bold/italic/code, ordered/unordered nested lists,
//! blockquotes, rules, GFM tables, merged colspan/rowspan HTML tables, image
//! embedding) plus a **bounded preset layer** (7 공문서 presets adjusting
//! margins, body size, line spacing, numbering system, page numbers, and h2
//! markers). The reference's measured decorative assets — gaejosik cover/toc/
//! chapter boxes, 결재란 (approval tables), docframe 두문/결문, and the format
//! profile system — are intentionally out of scope; presets here produce valid,
//! well-styled documents without those measured constants.
//!
//! Equation blocks (`$$ … $$`) render as real `<hp:equation>` elements via
//! [`crate::equation::latex_to_hulk`]; ```chart``` fences render as real
//! `Chart/chartN.xml` (OOXML DrawingML) parts referenced via `<hp:chart>`
//! (falling back to a plain code block when the fence has no numeric data).
//! [`GenOptions::profile`] optionally reproduces a source document's table
//! borders/shading/column widths/cell fonts without the source file itself.
//!
//! # Example
//! ```
//! use mdm_core::hwpx_gen::{markdown_to_hwpx, validate_hwpx, GenOptions};
//! let bytes = markdown_to_hwpx("# Title\n\nHello **world**.", &GenOptions::default()).unwrap();
//! assert!(validate_hwpx(&bytes).ok);
//! ```

mod blocks;
mod chart;
mod header;
mod ids;
mod image;
mod preset;
mod profile;
mod section;
mod table;
pub mod validate;

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

pub use ids::HwpxTheme;
pub use preset::Preset;
pub use profile::{
    extract_table_profile, parse_format_profile_json, BorderDef, BorderFillDef, CellProfile,
    CharPrDef, Fill, FormatProfile, TableProfile,
};
pub use validate::{validate_hwpx, ValidateIssue, ValidateResult};

/// Options for [`markdown_to_hwpx`].
#[derive(Debug, Clone, Default)]
pub struct GenOptions {
    /// Visual theme (heading/body/quote colors, table header style).
    pub theme: HwpxTheme,
    /// Document preset. `None` = general Markdown mode.
    pub preset: Option<Preset>,
    /// Format profile (issue #41) — reproduces a source document's table
    /// borders/shading/column widths/cell fonts without the source file.
    /// `None` = default styling (single solid border, or the preset's
    /// measured table grammar).
    pub profile: Option<profile::FormatProfile>,
}

impl GenOptions {
    /// Build options for a named preset (Korean or English alias).
    pub fn with_preset(name: &str) -> Self {
        GenOptions {
            theme: HwpxTheme::default(),
            preset: Some(Preset::from_alias(name)),
            profile: None,
        }
    }
}

/// Convert Markdown text into an HWPX document (ZIP bytes).
pub fn markdown_to_hwpx(markdown: &str, options: &GenOptions) -> std::io::Result<Vec<u8>> {
    let theme = ids::resolve_theme(&options.theme);
    let resolved = options.preset.map(preset::resolve_preset);
    let resolved_ref = resolved.as_ref();

    let blocks = blocks::parse_markdown_to_blocks(markdown);
    table::reset_table_ids();

    // Profile remap — allocated after the static borderFill/charPr id ranges
    // (see header::header_border_fill_id / profile::profile_char_pr_base).
    let header_bf_id = header::header_border_fill_id(resolved_ref);
    let remap = options
        .profile
        .as_ref()
        .map(|p| profile::build_profile_remap(p, profile::profile_char_pr_base(0), header_bf_id + 1));
    let remap_ref = remap.as_ref();
    let extra_border_fills: Vec<String> =
        remap_ref.map(|r| r.border_fill_xmls.clone()).unwrap_or_default();
    let extra_char_prs: Vec<String> = remap_ref.map(|r| r.char_pr_xmls.clone()).unwrap_or_default();

    let (header_xml, header_bf) =
        header::generate_header_xml(&theme, resolved_ref, &extra_border_fills, &extra_char_prs);
    let mut images = image::ImageRegistry::new();
    let mut charts = chart::ChartRegistry::new();
    let section_xml = section::blocks_to_section_xml(
        &blocks, &theme, resolved_ref, header_bf, &mut images, &mut charts, remap_ref,
    );
    let prv_text = blocks::build_prv_text(&blocks);

    let layout = if resolved_ref.is_some() { "gongmun" } else { "default" };
    let manifest =
        header::generate_manifest(&charts.manifest_items(), &images.manifest_items(), layout);
    let container = header::generate_container_xml();

    // ── ZIP packaging — mimetype must be the first entry and STORED ──
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("mimetype", stored)?;
        zip.write_all(b"application/hwp+zip")?;

        zip.start_file("META-INF/container.xml", deflated)?;
        zip.write_all(container.as_bytes())?;

        zip.start_file("Contents/content.hpf", deflated)?;
        zip.write_all(manifest.as_bytes())?;

        for part in &images.parts {
            zip.start_file(&part.name, deflated)?;
            zip.write_all(&part.data)?;
        }

        zip.start_file("Contents/header.xml", deflated)?;
        zip.write_all(header_xml.as_bytes())?;

        zip.start_file("Contents/section0.xml", deflated)?;
        zip.write_all(section_xml.as_bytes())?;

        for part in &charts.parts {
            zip.start_file(&part.name, deflated)?;
            zip.write_all(part.xml.as_bytes())?;
        }

        zip.start_file("Preview/PrvText.txt", deflated)?;
        zip.write_all(prv_text.as_bytes())?;

        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hwpx::HwpxParser;

    /// Parse generated bytes, returning (joined section text, table info).
    /// Table info is (merged_flag, flattened cell text) to avoid naming the
    /// parser's unexported `Table`/`HwpxDocument` types.
    fn roundtrip(bytes: Vec<u8>) -> (String, Vec<(bool, String)>) {
        let mut p = HwpxParser::from_bytes(bytes).expect("open");
        let doc = p.parse().expect("parse");
        let text = doc.sections.join("\n");
        let tables = doc
            .tables
            .iter()
            .map(|t| {
                let flat = t.cells.iter().flatten().cloned().collect::<Vec<_>>().join("|");
                (t.has_merged_cells(), flat)
            })
            .collect();
        (text, tables)
    }

    #[test]
    fn basic_document_validates() {
        let md = "# 제목\n\n본문 **굵게** 그리고 *기울임*.\n\n- 하나\n- 둘\n";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        let res = validate_hwpx(&bytes);
        assert!(res.ok, "issues: {:?}", res.issues);
        assert!(res.entry_count >= 5);
    }

    #[test]
    fn heading_and_paragraph_roundtrip() {
        let md = "# 보고서 제목\n\n첫 문단입니다.\n\n## 소제목\n\n둘째 문단.";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        assert!(validate_hwpx(&bytes).ok);
        let (text, _) = roundtrip(bytes);
        assert!(text.contains("보고서 제목"), "text: {text}");
        assert!(text.contains("첫 문단입니다"), "text: {text}");
        assert!(text.contains("소제목"), "text: {text}");
    }

    #[test]
    fn gfm_table_roundtrip() {
        let md = "| 이름 | 값 |\n| --- | --- |\n| 사과 | 100 |\n| 배 | 200 |\n";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        assert!(validate_hwpx(&bytes).ok);
        let (_, tables) = roundtrip(bytes);
        assert_eq!(tables.len(), 1, "one table expected");
        let flat = &tables[0].1;
        assert!(flat.contains("이름"), "cells: {flat}");
        assert!(flat.contains("사과"), "cells: {flat}");
        assert!(flat.contains("200"), "cells: {flat}");
    }

    #[test]
    fn merged_html_table_preserves_spans() {
        let md = "<table><tr><th colspan=\"2\">머리</th></tr><tr><td>A</td><td>B</td></tr></table>";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        assert!(validate_hwpx(&bytes).ok, "{:?}", validate_hwpx(&bytes).issues);
        let (_, tables) = roundtrip(bytes);
        assert_eq!(tables.len(), 1);
        assert!(tables[0].0, "colspan should survive");
        let flat = &tables[0].1;
        assert!(flat.contains("머리"));
        assert!(flat.contains('A') && flat.contains('B'));
    }

    #[test]
    fn all_seven_presets_generate_valid_hwpx() {
        for name in ["기안문", "보고서", "계획서", "통지", "회의록", "개조식", "보도자료"] {
            let md = "# 문서 제목\n\n## 개요\n\n- 첫째 항목\n- 둘째 항목\n\n본문 내용입니다.";
            let bytes = markdown_to_hwpx(md, &GenOptions::with_preset(name)).unwrap();
            let res = validate_hwpx(&bytes);
            assert!(res.ok, "preset {name} invalid: {:?}", res.issues);
        }
    }

    #[test]
    fn preset_legal_numbering_renders_markers() {
        let md = "- 첫째\n- 둘째\n  - 하위\n";
        let bytes = markdown_to_hwpx(md, &GenOptions::with_preset("기안문")).unwrap();
        assert!(validate_hwpx(&bytes).ok);
        let (text, _) = roundtrip(bytes);
        assert!(text.contains("1."), "expected legal marker: {text}");
    }

    #[test]
    fn embedded_data_uri_image() {
        // 1×1 transparent PNG data URI
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
        let md = format!("![alt]({png})");
        let bytes = markdown_to_hwpx(&md, &GenOptions::default()).unwrap();
        let res = validate_hwpx(&bytes);
        assert!(res.ok, "issues: {:?}", res.issues);
        // BinData part embedded
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let has_bindata = (0..archive.len())
            .any(|i| archive.by_index(i).map(|f| f.name().starts_with("BinData/")).unwrap_or(false));
        assert!(has_bindata, "image bytes should be embedded in BinData/");
    }

    #[test]
    fn equation_block_emits_hp_equation() {
        let md = "본문.\n\n$$ \\frac{a}{b} $$\n\n끝.";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        let res = validate_hwpx(&bytes);
        assert!(res.ok, "issues: {:?}", res.issues);
        // section0.xml should contain an <hp:equation> with a script.
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut section = String::new();
        {
            use std::io::Read as _;
            let mut f = archive.by_name("Contents/section0.xml").unwrap();
            f.read_to_string(&mut section).unwrap();
        }
        assert!(section.contains("<hp:equation"), "no equation element: {section}");
        assert!(section.contains("<hp:script>"), "no script element");
        assert!(section.contains("over"), "frac should become HULK 'over': {section}");
    }

    #[test]
    fn chart_fence_emits_hp_chart_and_chart_part() {
        let md = "본문.\n\n```chart\ntype: column\ncat: 1분기, 2분기, 3분기\n예산: 10, 20, 30\n집행: 5, 15, 25\n```\n\n끝.";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        let res = validate_hwpx(&bytes);
        assert!(res.ok, "issues: {:?}", res.issues);

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut section = String::new();
        let mut manifest = String::new();
        let mut chart_part = String::new();
        {
            use std::io::Read as _;
            archive.by_name("Contents/section0.xml").unwrap().read_to_string(&mut section).unwrap();
            archive.by_name("Contents/content.hpf").unwrap().read_to_string(&mut manifest).unwrap();
            archive.by_name("Chart/chart1.xml").unwrap().read_to_string(&mut chart_part).unwrap();
        }
        assert!(section.contains("<hp:chart"), "no chart element: {section}");
        assert!(section.contains("chartIDRef=\"Chart/chart1.xml\""), "{section}");
        assert!(manifest.contains("href=\"Chart/chart1.xml\""), "{manifest}");
        assert!(manifest.contains("media-type=\"application/xml\""), "{manifest}");
        assert!(chart_part.contains("<c:chartSpace"), "{chart_part}");
        assert!(chart_part.contains("<c:barChart>"), "{chart_part}");
    }

    #[test]
    fn unparseable_chart_fence_falls_back_to_code_block() {
        // No numeric series → parse_chart_fence returns None → plain code block.
        let md = "```chart\ntitle: 그냥 텍스트\n```\n";
        let bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        assert!(validate_hwpx(&bytes).ok);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(
            (0..archive.len()).all(|i| !archive.by_index(i).unwrap().name().starts_with("Chart/")),
            "no Chart/ part should be emitted for an unparseable fence"
        );
    }

    #[test]
    fn format_profile_round_trips_border_shading_and_column_widths() {
        use super::profile::{BorderDef, BorderFillDef, CellProfile, Fill, FormatProfile, TableProfile};

        let mut used_bf = std::collections::HashMap::new();
        let red_edge = BorderDef { kind: "SOLID".to_string(), width: "0.5 mm".to_string(), color: "#FF0000".to_string() };
        used_bf.insert(
            "1".to_string(),
            BorderFillDef {
                left_border: Some(red_edge.clone()),
                right_border: Some(red_edge.clone()),
                top_border: Some(red_edge.clone()),
                bottom_border: Some(red_edge),
                fill: Some(Fill { face_color: "#ABCDEF".to_string() }),
            },
        );
        let profile = FormatProfile {
            schema_version: None,
            tables: vec![TableProfile {
                table_index: 0,
                rows: 2,
                cols: 2,
                cells: vec![CellProfile {
                    row: 0,
                    col: 0,
                    border_fill_id_ref: Some("1".to_string()),
                    height_hwpunit: Some("2500".to_string()),
                    ..Default::default()
                }],
                col_widths_hwpunit: Some(vec!["9000".to_string(), "9000".to_string()]),
                used_border_fills: used_bf,
                ..Default::default()
            }],
        };

        let md = "| 이름 | 값 |\n| --- | --- |\n| 사과 | 100 |\n";
        let options = GenOptions { profile: Some(profile), ..GenOptions::default() };
        let bytes = markdown_to_hwpx(md, &options).unwrap();
        let res = validate_hwpx(&bytes);
        assert!(res.ok, "issues: {:?}", res.issues);

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut header = String::new();
        let mut section = String::new();
        {
            use std::io::Read as _;
            archive.by_name("Contents/header.xml").unwrap().read_to_string(&mut header).unwrap();
            archive.by_name("Contents/section0.xml").unwrap().read_to_string(&mut section).unwrap();
        }
        assert!(header.contains("#ABCDEF"), "cell shading not applied: {header}");
        assert!(header.contains("#FF0000"), "border color not applied: {header}");
        assert!(section.contains("width=\"9000\""), "column width override not applied: {section}");
        assert!(section.contains("height=\"2500\""), "cell height override not applied: {section}");
    }

    #[test]
    fn format_profile_falls_back_to_even_split_of_table_width_without_col_widths() {
        // No col_widths_hwpunit — only the table's overall width — should
        // still override the generic content-based column split (codex
        // review: TableRemap.width was previously dropped on the floor).
        use super::profile::{FormatProfile, TableProfile};

        let profile = FormatProfile {
            schema_version: None,
            tables: vec![TableProfile {
                table_index: 0,
                rows: 2,
                cols: 2,
                width_hwpunit: Some("20000".to_string()),
                cells: Vec::new(),
                ..Default::default()
            }],
        };

        let md = "| 이름 | 값 |\n| --- | --- |\n| 사과 | 100 |\n";
        let options = GenOptions { profile: Some(profile), ..GenOptions::default() };
        let bytes = markdown_to_hwpx(md, &options).unwrap();
        assert!(validate_hwpx(&bytes).ok);

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut section = String::new();
        {
            use std::io::Read as _;
            archive.by_name("Contents/section0.xml").unwrap().read_to_string(&mut section).unwrap();
        }
        // 20000 / 2 cols = 10000 each — distinct from the ~44000-wide default
        // content-based split, so this only passes if the width fallback ran.
        assert!(section.contains("width=\"10000\""), "even-split width fallback not applied: {section}");
    }

    #[test]
    fn format_profile_with_unusable_table_width_falls_back_to_content_based_widths() {
        // width_hwpunit="0" (or too small to give every column a sane floor)
        // must not produce zero/near-zero <hp:cellSz width="..."/> cells —
        // it should fall back to the generic content-based split instead
        // (codex re-review: the even-split fallback itself needs a floor).
        // Verified by comparing against the same table generated with no
        // profile at all, after masking the `<hp:tbl id="...">` sequence —
        // markdown_to_hwpx resets its table-id counter per call, but two
        // separate calls in this test each start their own sequence, so the
        // *id* legitimately differs even when every width is identical.
        use super::profile::{FormatProfile, TableProfile};

        let md = "| 이름 | 값 |\n| --- | --- |\n| 사과 | 100 |\n";
        let baseline_bytes = markdown_to_hwpx(md, &GenOptions::default()).unwrap();
        let baseline_section = mask_tbl_id(&section_xml_of(baseline_bytes));
        assert!(!baseline_section.contains("width=\"0\""), "sanity: baseline itself has a zero-width cell");

        for bad_width in ["0", "1"] {
            let profile = FormatProfile {
                schema_version: None,
                tables: vec![TableProfile {
                    table_index: 0,
                    rows: 2,
                    cols: 2,
                    width_hwpunit: Some(bad_width.to_string()),
                    cells: Vec::new(),
                    ..Default::default()
                }],
            };
            let options = GenOptions { profile: Some(profile), ..GenOptions::default() };
            let bytes = markdown_to_hwpx(md, &options).unwrap();
            assert!(validate_hwpx(&bytes).ok, "width={bad_width}");

            let section = mask_tbl_id(&section_xml_of(bytes));
            assert!(!section.contains("width=\"0\""), "width={bad_width} produced a zero-width cell: {section}");
            assert_eq!(
                section, baseline_section,
                "width={bad_width}: expected content-based fallback identical to the no-profile baseline (ids masked)"
            );
        }
    }

    /// Read Contents/section0.xml out of generated HWPX bytes.
    fn section_xml_of(bytes: Vec<u8>) -> String {
        use std::io::Read as _;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut section = String::new();
        archive.by_name("Contents/section0.xml").unwrap().read_to_string(&mut section).unwrap();
        section
    }

    /// Mask `<hp:tbl id="N">` so two independently-generated documents (each
    /// with its own reset-per-call table-id counter) compare equal on
    /// everything except the id itself.
    fn mask_tbl_id(xml: &str) -> String {
        regex::Regex::new(r#"<hp:tbl id="\d+""#).unwrap().replace_all(xml, r#"<hp:tbl id="N""#).into_owned()
    }

    #[test]
    fn extracted_profile_round_trips_through_generate() {
        // Build a document with a shaded HTML table, extract its profile,
        // then feed that profile back into a fresh generate of the same
        // table shape — the shading/borders should survive without the
        // original source file.
        let md_src = "<table><tr><th>이름</th><th>값</th></tr><tr><td>사과</td><td>100</td></tr></table>";
        let src_bytes = markdown_to_hwpx(md_src, &GenOptions::default()).unwrap();
        let profile = extract_table_profile(&src_bytes).expect("extract should succeed");
        assert_eq!(profile.tables.len(), 1);
        assert!(!profile.tables[0].used_border_fills.is_empty());

        let md_out = "<table><tr><th>이름</th><th>값</th></tr><tr><td>배</td><td>200</td></tr></table>";
        let options = GenOptions { profile: Some(profile), ..GenOptions::default() };
        let bytes = markdown_to_hwpx(md_out, &options).unwrap();
        assert!(validate_hwpx(&bytes).ok);
    }

    #[test]
    fn empty_document_is_valid() {
        let bytes = markdown_to_hwpx("", &GenOptions::default()).unwrap();
        assert!(validate_hwpx(&bytes).ok);
    }

    #[test]
    fn detects_broken_mimetype() {
        // Hand-build a zip whose first entry is not mimetype.
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut cursor);
            let opt = SimpleFileOptions::default();
            zip.start_file("Contents/header.xml", opt).unwrap();
            zip.write_all(b"<x/>").unwrap();
            zip.finish().unwrap();
        }
        let res = validate_hwpx(&cursor.into_inner());
        assert!(!res.ok);
    }
}
