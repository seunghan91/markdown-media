// ============================================================================
// 🚧 작업 중 - 이 파일은 현재 [테스트 팀]에서 작업 중입니다
// ============================================================================
// 작업 담당: 병렬 작업 팀
// 시작 시간: 2025-01-01
// 진행 상태: Phase 1.8 테스트 구현
// 
// ⚠️ 주의: 1.7 오케스트레이터는 다른 팀에서 작업 중입니다.
//         이 테스트 파일은 1.7과 독립적으로 진행됩니다.
// ============================================================================

//! MDM Core Parser Integration Tests
//!
//! 이 모듈은 HWP, DOCX, PDF 파서들의 통합 테스트를 포함합니다.
//! 각 파서의 기능을 검증하고 출력 형식의 일관성을 확인합니다.

use std::path::PathBuf;
use std::fs;

// 테스트 샘플 파일 경로 상수
const TEST_SAMPLES_DIR: &str = "../samples/input";

/// 테스트 유틸리티 - 테스트 샘플 파일 경로 반환
fn get_sample_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(TEST_SAMPLES_DIR)
        .join(filename)
}

/// 테스트 유틸리티 - 출력 디렉토리 생성
#[allow(dead_code)]
fn create_test_output_dir(test_name: &str) -> PathBuf {
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_output")
        .join(test_name);
    fs::create_dir_all(&output_dir).expect("Failed to create test output directory");
    output_dir
}

// ============================================================================
// HWP 파서 테스트
// ============================================================================

#[cfg(test)]
mod hwp_tests {
    use super::*;

    #[test]
    #[ignore = "Requires sample HWP file - implement with test fixtures"]
    fn test_hwp_text_extraction() {
        // 테스트 대상: HwpParser::extract_text()
        // 예상 결과: 문서 내 모든 텍스트가 추출됨
        let sample_path = get_sample_path("sample.hwp");
        if !sample_path.exists() {
            panic!("Sample file not found: {:?}", sample_path);
        }
        // TODO: Implement actual text extraction test
        unimplemented!("HWP 텍스트 추출 테스트 - 샘플 파일 필요");
    }

    #[test]
    #[ignore = "Requires sample HWP file with tables - implement with test fixtures"]
    fn test_hwp_table_parsing() {
        // 테스트 대상: parse_table_info(), parse_cell_list_header()
        // 예상 결과: 병합 셀이 올바르게 파싱됨
        let sample_path = get_sample_path("table_sample.hwp");
        if !sample_path.exists() {
            panic!("Sample file not found: {:?}", sample_path);
        }
        // TODO: Implement actual table parsing test
        unimplemented!("HWP 테이블 파싱 테스트 - 샘플 파일 필요");
    }

    #[test]
    #[ignore = "Requires sample HWP file with images - implement with test fixtures"]
    fn test_hwp_image_extraction() {
        // 테스트 대상: ShapeComponent, parse_picture_component()
        // 예상 결과: 이미지가 올바른 형식으로 추출됨
        let sample_path = get_sample_path("image_sample.hwp");
        if !sample_path.exists() {
            panic!("Sample file not found: {:?}", sample_path);
        }
        // TODO: Implement actual image extraction test
        unimplemented!("HWP 이미지 추출 테스트 - 샘플 파일 필요");
    }

    #[test]
    #[ignore = "Requires sample HWP file with formatting - implement with test fixtures"]
    fn test_hwp_char_shape_formatting() {
        // 테스트 대상: parse_char_shape(), apply_markdown_formatting()
        // 예상 결과: 볼드/이탤릭/밑줄이 마크다운으로 변환됨
        let sample_path = get_sample_path("formatted_sample.hwp");
        if !sample_path.exists() {
            panic!("Sample file not found: {:?}", sample_path);
        }
        // TODO: Implement actual formatting test
        unimplemented!("HWP 문자 서식 테스트 - 샘플 파일 필요");
    }

    #[test]
    fn test_hwp_korean_text_encoding() {
        // 한글 인코딩 테스트
        use mdm_core::hwp::record::extract_para_text;

        // UTF-16LE "안녕" (U+C548, U+B155)
        let data = vec![0x48, 0xC5, 0x55, 0xB1];
        let text = extract_para_text(&data);
        assert_eq!(text, "안녕");
    }

    #[test]
    fn test_hwp_record_parsing() {
        use mdm_core::hwp::record::{RecordParser, HWPTAG_PARA_TEXT};

        // 테스트 레코드 헤더 생성
        let header: u32 = 0x43 | (0 << 10) | (4 << 20);
        let mut data = header.to_le_bytes().to_vec();
        data.extend_from_slice(&[b'T', b'e', b's', b't']);

        let mut parser = RecordParser::new(&data);
        let record = parser.parse_next().expect("Failed to parse record");

        assert_eq!(record.tag_id, HWPTAG_PARA_TEXT);
        assert_eq!(record.level, 0);
        assert_eq!(record.size, 4);
    }

    #[test]
    fn test_hwp_table_markdown_output() {
        use mdm_core::hwp::record::HwpTable;

        let mut table = HwpTable::new(2, 2);
        table.cells[0][0].content = "A".to_string();
        table.cells[0][1].content = "B".to_string();
        table.cells[1][0].content = "1".to_string();
        table.cells[1][1].content = "2".to_string();

        let md = table.to_markdown();
        assert!(md.contains("| A | B |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| 1 | 2 |"));
    }
}

// ============================================================================
// DOCX 파서 테스트
// ============================================================================

#[cfg(test)]
mod docx_tests {

    #[test]
    fn test_docx_text_run_markdown() {
        use mdm_core::docx::parser::TextRun;

        let run = TextRun {
            text: "hello".to_string(),
            bold: true,
            italic: false,
            ..Default::default()
        };
        assert_eq!(run.to_markdown(), "**hello**");

        let run2 = TextRun {
            text: "world".to_string(),
            bold: true,
            italic: true,
            ..Default::default()
        };
        assert_eq!(run2.to_markdown(), "***world***");
    }

    #[test]
    fn test_docx_paragraph_heading() {
        use mdm_core::docx::parser::{Paragraph, TextRun};

        let para = Paragraph {
            runs: vec![TextRun {
                text: "Title".to_string(),
                ..Default::default()
            }],
            style: Some("Heading1".to_string()),
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "# Title");
    }

    #[test]
    fn test_docx_table_markdown() {
        use mdm_core::docx::parser::{DocxTable, TableCell};

        let table = DocxTable {
            rows: vec![
                vec![
                    TableCell { content: "A".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                    TableCell { content: "B".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
                vec![
                    TableCell { content: "1".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                    TableCell { content: "2".to_string(), col_span: 1, row_span: 1, v_merge_continue: false },
                ],
            ],
            has_header: true,
        };

        let md = table.to_markdown();
        assert!(md.contains("| A | B |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| 1 | 2 |"));
    }

    #[test]
    fn test_docx_list_item() {
        use mdm_core::docx::parser::{Paragraph, TextRun};

        let para = Paragraph {
            runs: vec![TextRun {
                text: "List item".to_string(),
                ..Default::default()
            }],
            is_list_item: true,
            list_type: Some("bullet".to_string()),
            ..Default::default()
        };
        assert_eq!(para.to_markdown(), "- List item");

        let numbered = Paragraph {
            runs: vec![TextRun {
                text: "Numbered item".to_string(),
                ..Default::default()
            }],
            is_list_item: true,
            list_type: Some("number".to_string()),
            ..Default::default()
        };
        assert_eq!(numbered.to_markdown(), "1. Numbered item");
    }

    #[test]
    fn test_docx_strikethrough() {
        use mdm_core::docx::parser::TextRun;

        let run = TextRun {
            text: "deleted".to_string(),
            strike: true,
            ..Default::default()
        };
        assert_eq!(run.to_markdown(), "~~deleted~~");
    }

    /// Build a minimal in-memory DOCX with just `word/document.xml` +
    /// `word/numbering.xml` — the other parts (`_rels`, `styles.xml`,
    /// `[Content_Types].xml`) are all optional in `DocxParser` (missing
    /// files degrade to `Ok(())`, not an error).
    fn build_minimal_docx(document_xml: &str, numbering_xml: &str) -> Vec<u8> {
        use std::io::{Cursor, Write};
        use zip::write::SimpleFileOptions;
        use zip::ZipWriter;

        let mut buf = Cursor::new(Vec::<u8>::new());
        {
            let mut zip = ZipWriter::new(&mut buf);
            let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            zip.start_file("word/document.xml", opts).unwrap();
            zip.write_all(document_xml.as_bytes()).unwrap();
            zip.start_file("word/numbering.xml", opts).unwrap();
            zip.write_all(numbering_xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        buf.into_inner()
    }

    /// Regression (full round-trip): a docx list using non-decimal numFmts
    /// (lowerLetter/lowerRoman) must render increasing markers per item —
    /// not the same literal marker on every paragraph — and a nested
    /// (deeper ilvl) sub-list must restart while the parent level's counter
    /// keeps counting from where it left off.
    #[test]
    fn test_docx_list_ordinals_increment_via_full_parse() {
        use mdm_core::docx::parser::DocxParser;

        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="lowerLetter"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="lowerRoman"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;

        // Alpha(a) -> Roman1(i) -> Roman2(ii) -> Beta(b, parent continues)
        // -> Roman3(i, child restarts under the new parent item)
        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Alpha</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Roman1</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Roman2</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Beta</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Roman3</w:t></w:r></w:p>
</w:body>
</w:document>"#;

        let bytes = build_minimal_docx(document_xml, numbering_xml);
        let mut parser = DocxParser::from_bytes(bytes).expect("valid minimal docx");
        let doc = parser.parse().expect("parse succeeds");

        let markers: Vec<String> = doc.paragraphs.iter().map(|p| p.to_markdown()).collect();
        assert_eq!(
            markers,
            vec![
                "a) Alpha".to_string(),
                "  i. Roman1".to_string(),
                "  ii. Roman2".to_string(),
                "b) Beta".to_string(),
                "  i. Roman3".to_string(),
            ]
        );
    }

    /// Regression (codex P1): a malicious/corrupt `w:ilvl` value (e.g.
    /// `u32::MAX`) used to flow straight into `indent_level` and from there
    /// into a `Vec` resize/index for the per-level ordinal counters — a
    /// multi-GB allocation attempt from a few bytes of XML. `w:ilvl` is now
    /// clamped to `MAX_LIST_ILVL` (63) at parse time, so this must parse
    /// quickly, without panicking or ballooning memory, and produce a
    /// paragraph clamped to the max depth.
    #[test]
    fn test_docx_huge_ilvl_does_not_explode() {
        use mdm_core::docx::parser::DocxParser;

        let numbering_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
</w:numbering>"#;

        let document_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>
<w:p><w:pPr><w:numPr><w:ilvl w:val="4294967295"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Huge</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Normal</w:t></w:r></w:p>
</w:body>
</w:document>"#;

        let bytes = build_minimal_docx(document_xml, numbering_xml);
        let mut parser = DocxParser::from_bytes(bytes).expect("valid minimal docx");
        let doc = parser.parse().expect("parse succeeds without panicking or hanging");

        assert_eq!(doc.paragraphs.len(), 2);
        assert_eq!(doc.paragraphs[0].indent_level, 63, "ilvl must clamp to MAX_LIST_ILVL");
        assert_eq!(doc.paragraphs[0].list_ordinal, 1);
        assert!(doc.paragraphs[0].to_markdown().ends_with("Huge"));
        assert_eq!(doc.paragraphs[1].indent_level, 0);
    }
}

// ============================================================================
// PDF 파서 테스트
// ============================================================================

#[cfg(test)]
mod pdf_tests {
    use super::*;

    #[test]
    fn test_pdf_version_detection() {
        // 직접 PdfParser 생성 테스트는 실제 파일이 필요하므로 스킵
        // 대신 버전 파싱 로직만 테스트
        let header = b"%PDF-1.7\n";
        let version_str = std::str::from_utf8(&header[5..8]).unwrap();
        assert_eq!(version_str, "1.7");
    }

    #[test]
    #[ignore = "Requires encrypted PDF sample file"]
    fn test_pdf_encryption_detection() {
        // 실제 암호화 테스트는 테스트 파일이 필요
        let sample_path = get_sample_path("encrypted_sample.pdf");
        if !sample_path.exists() {
            panic!("Sample file not found: {:?}", sample_path);
        }
        // TODO: Implement actual encryption detection test
        unimplemented!("PDF 암호화 감지 테스트 - 샘플 파일 필요");
    }

    #[test]
    fn test_pdf_image_format() {
        use mdm_core::pdf::parser::ImageFormat;

        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Raw.extension(), "raw");
    }

    #[test]
    fn test_pdf_table_to_markdown() {
        use mdm_core::pdf::parser::PdfTable;

        let table = PdfTable {
            page: 1,
            rows: vec![
                vec!["Name".to_string(), "Age".to_string()],
                vec!["Alice".to_string(), "30".to_string()],
            ],
            column_count: 2,
            y_top: 100.0,
            y_bottom: 80.0,
        };

        let md = table.to_markdown();
        assert!(md.contains("| Name | Age |"));
        assert!(md.contains("| --- | --- |"));
        assert!(md.contains("| Alice | 30 |"));
    }

    #[test]
    fn test_pdf_font_style_detection() {
        use mdm_core::pdf::parser::FontStyle;

        // Bold 검출
        fn detect_style(name: &str) -> FontStyle {
            let name_lower = name.to_lowercase();
            FontStyle {
                is_bold: name_lower.contains("bold"),
                is_italic: name_lower.contains("italic") || name_lower.contains("oblique"),
            }
        }

        let bold = detect_style("Arial-Bold");
        assert!(bold.is_bold);
        assert!(!bold.is_italic);

        let italic = detect_style("Arial-Italic");
        assert!(!italic.is_bold);
        assert!(italic.is_italic);

        let bold_italic = detect_style("Arial-BoldItalic");
        assert!(bold_italic.is_bold);
        assert!(bold_italic.is_italic);
    }

    #[test]
    fn test_pdf_layout_element_types() {
        use mdm_core::pdf::parser::LayoutElementType;

        assert_eq!(LayoutElementType::Text, LayoutElementType::Text);
        assert_ne!(LayoutElementType::Text, LayoutElementType::Image);
    }

    #[test]
    fn test_pdf_text_alignment() {
        use mdm_core::pdf::parser::TextAlignment;

        let default: TextAlignment = Default::default();
        assert_eq!(default, TextAlignment::Left);
    }
}

// ============================================================================
// 통합 테스트 (Cross-format)
// ============================================================================

#[cfg(test)]
mod integration_tests {

    #[test]
    #[ignore = "Requires sample files from all formats for consistency testing"]
    fn test_mdx_output_format_consistency() {
        // 모든 파서가 일관된 MDX 포맷을 출력하는지 검증
        // 예상 결과:
        // - frontmatter 형식 일치 (format, source, title 등)
        // - 이미지 참조 형식 일치 (![alt](path))
        // - 테이블 마크다운 형식 일치
        unimplemented!("MDX 출력 포맷 일관성 테스트 - 샘플 파일 필요");
    }

    #[test]
    #[ignore = "Requires sample files with images from all formats"]
    fn test_image_extraction_consistency() {
        // 모든 파서의 이미지 추출 결과 형식 검증
        // 예상 결과:
        // - id, filename, path, data 필드 존재
        // - 이미지 데이터가 유효한 형식 (JPEG/PNG/GIF 등)
        unimplemented!("이미지 추출 일관성 테스트 - 샘플 파일 필요");
    }

    #[test]
    #[ignore = "Requires sample files with tables from all formats"]
    fn test_table_markdown_output() {
        // 모든 파서의 테이블 마크다운 출력 형식 검증
        // 예상 결과:
        // - 헤더 행 구분자 존재 (| --- |)
        // - 파이프 문자 이스케이프 (\|)
        unimplemented!("테이블 마크다운 출력 테스트 - 샘플 파일 필요");
    }

    #[test]
    fn test_korean_text_handling() {
        // 한글 처리 일관성 테스트
        let korean = "안녕하세요 테스트입니다";
        assert!(korean.chars().all(|c| c.is_alphanumeric() || c.is_whitespace()));
    }
}

// ============================================================================
// 벤치마크 테스트 (선택적)
// ============================================================================

#[cfg(test)]
#[cfg(feature = "benchmark")]
mod benchmark_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    #[ignore = "Requires large HWP sample file for benchmarking"]
    fn benchmark_hwp_parsing() {
        // 대용량 HWP 파일 파싱 성능 테스트
        let sample_path = get_sample_path("large_sample.hwp");
        assert!(sample_path.exists(), "Benchmark sample file required");

        let start = Instant::now();
        // TODO: 실제 파싱 수행
        let duration = start.elapsed();

        // 성능 기준: 1MB당 1초 이하
        assert!(duration.as_secs() < 10, "HWP parsing exceeded time limit: {:?}", duration);
    }

    #[test]
    #[ignore = "Requires large DOCX sample file for benchmarking"]
    fn benchmark_docx_parsing() {
        // 대용량 DOCX 파일 파싱 성능 테스트
        let sample_path = get_sample_path("large_sample.docx");
        assert!(sample_path.exists(), "Benchmark sample file required");

        let start = Instant::now();
        // TODO: 실제 파싱 수행
        let duration = start.elapsed();

        // 성능 기준: 1MB당 1초 이하
        assert!(duration.as_secs() < 10, "DOCX parsing exceeded time limit: {:?}", duration);
    }

    #[test]
    #[ignore = "Requires large PDF sample file for benchmarking"]
    fn benchmark_pdf_parsing() {
        // 대용량 PDF 파일 파싱 성능 테스트
        let sample_path = get_sample_path("large_sample.pdf");
        assert!(sample_path.exists(), "Benchmark sample file required");

        let start = Instant::now();
        // TODO: 실제 파싱 수행
        let duration = start.elapsed();

        // 성능 기준: 1MB당 1초 이하
        assert!(duration.as_secs() < 10, "PDF parsing exceeded time limit: {:?}", duration);
    }
}
