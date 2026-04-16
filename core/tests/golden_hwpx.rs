//! Golden-file regression tests for the HWPX parser.
//!
//! Inspired by rhwp's sample-driven test suite (783+ tests across
//! `src/wasm_api/tests.rs`). The goal is to catch parser regressions
//! that unit tests miss by comparing full-document markdown output
//! against a committed expected file.
//!
//! ## Workflow
//!
//! ```text
//! 1. cargo test --test golden_hwpx
//!       → runs comparison, prints diff on mismatch
//!
//! 2. UPDATE_GOLDEN=1 cargo test --test golden_hwpx
//!       → regenerates golden files (use after an intentional parser change,
//!         then review the diff before committing)
//! ```
//!
//! ## Adding a new fixture
//!
//! - Synthetic: call `build_minimal_hwpx(...)` with the XML bodies you
//!   want to exercise, then `check_golden("my-case", &doc, ...)`.
//! - Real-world: save the HWPX under `core/tests/fixtures/hwpx/`, add
//!   a test that reads it via `HwpxParser::open`, and `check_golden`.
//!
//! The scaffold deliberately stays small — the point is to have
//! something to extend, not to check every record type today.

use std::fs;
use std::io::{Cursor, Write};
use std::path::PathBuf;

use mdm_core::HwpxParser;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Directory holding `{name}.md` golden outputs.
fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

/// Compare a fixture's rendered sections to `golden/{name}.md`.
///
/// Set `UPDATE_GOLDEN=1` to rewrite the golden file from the current output.
fn check_golden(name: &str, actual_sections: &[String]) {
    let golden_path = golden_dir().join(format!("{name}.md"));
    // Sections joined with a delimiter so multi-section outputs are preserved
    // verbatim in the golden file.
    let actual = actual_sections.join("\n\n<!-- ──── section break ──── -->\n\n");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        fs::write(&golden_path, &actual)
            .unwrap_or_else(|e| panic!("failed to write {}: {}", golden_path.display(), e));
        println!("UPDATE_GOLDEN: wrote {}", golden_path.display());
        return;
    }

    let expected = fs::read_to_string(&golden_path).unwrap_or_else(|_| {
        panic!(
            "golden file missing: {}\n\nrun with UPDATE_GOLDEN=1 to create it.",
            golden_path.display()
        )
    });

    if actual != expected {
        // Produce a compact diff-ish report. `pretty_assertions` would be
        // nicer but keeping deps tight for now.
        let mut report = String::new();
        report.push_str(&format!(
            "\n╭─ golden mismatch: {name} ─╮\n\
             │ golden: {}\n│ to update: UPDATE_GOLDEN=1 cargo test --test golden_hwpx {name}\n\
             ╰───────────────────────────────╯\n\n",
            golden_path.display()
        ));
        // Line-level diff
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();
        let max = expected_lines.len().max(actual_lines.len());
        for i in 0..max {
            let e = expected_lines.get(i).copied().unwrap_or("");
            let a = actual_lines.get(i).copied().unwrap_or("");
            if e != a {
                report.push_str(&format!("  line {:>4}\n    - {}\n    + {}\n", i + 1, e, a));
            }
        }
        panic!("{}", report);
    }
}

/// Build a minimal valid HWPX archive in memory.
///
/// `char_properties_xml` goes inside `<hh:refList>` in `Contents/header.xml`.
/// `section_xml` goes inside `Contents/section0.xml` as the body.
fn build_minimal_hwpx(char_properties_xml: &str, section_xml: &str) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::<u8>::new());
    {
        let mut zip = ZipWriter::new(&mut buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // mimetype — stored first, uncompressed, per HWPX spec.
        zip.start_file(
            "mimetype",
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"application/hwp+zip").unwrap();

        zip.start_file("version.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<hv:HWPML version="2.5" targetApplication="WORDPROCESSOR"
          xmlns:hv="http://www.hancom.co.kr/hwpml/2011/version"/>"#,
        )
        .unwrap();

        // Minimal container manifest — one section, no binaries.
        zip.start_file("Contents/content.hpf", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<opf:package xmlns:opf="http://www.idpf.org/2007/opf/"
             xmlns:ocf="urn:oasis:names:tc:opendocument:xmlns:container">
  <opf:spine>
    <opf:itemref idref="section0"/>
  </opf:spine>
  <opf:manifest>
    <opf:item id="section0" href="Contents/section0.xml"
              media-type="application/xml"/>
  </opf:manifest>
</opf:package>"##,
        )
        .unwrap();

        // header.xml — parser reads this for charPr definitions.
        zip.start_file("Contents/header.xml", opts).unwrap();
        let header = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head"
         xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
  <hh:refList>
{char_properties_xml}
  </hh:refList>
</hh:head>"#
        );
        zip.write_all(header.as_bytes()).unwrap();

        // section0.xml — body content.
        zip.start_file("Contents/section0.xml", opts).unwrap();
        let section = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section"
        xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph">
{section_xml}
</hs:sec>"#
        );
        zip.write_all(section.as_bytes()).unwrap();

        zip.finish().unwrap();
    }
    buf.into_inner()
}

fn parse_synthetic(char_props: &str, section: &str) -> Vec<String> {
    let bytes = build_minimal_hwpx(char_props, section);
    let mut parser = HwpxParser::from_bytes(bytes)
        .expect("synthetic HWPX should parse");
    let doc = parser.parse().expect("parse succeeds");
    doc.sections
}

// ─── Golden test cases ───────────────────────────────────────────────────────

/// Body text with Hancom's `shape="3D"` placeholder on strikeout must NOT
/// be rendered as strikethrough (regression: 251113 venture press release).
#[test]
fn golden_strikeout_3d_placeholder_is_noop() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:bold/>
        <hh:underline type="NONE"/>
        <hh:strikeout shape="3D"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>본문 문장입니다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("strikeout_3d_placeholder", &sections);
}

/// Real strikeout shape ("CONT") IS rendered with ~~...~~.
#[test]
fn golden_strikeout_cont_shape_is_real() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="CONT"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>삭제된 항목</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("strikeout_cont_real", &sections);
}

/// Underline with unknown `type="3D"` placeholder is fail-closed (no underline).
#[test]
fn golden_underline_unknown_type_is_noop() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="3D"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>평범한 본문 문장.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("underline_unknown_type", &sections);
}

/// Inline equation with a single-line script is emitted as `$…$`.
/// Hancom's script is near-LaTeX for most common cases, so round-tripping
/// into math markdown gives LLMs something they can actually read.
#[test]
fn golden_equation_inline() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>피타고라스 정리:</hp:t>
      </hp:run>
      <hp:equation version="6.0" baseLine="500" textColor="0" baseUnit="1000">
        <hp:script>a^2 + b^2 = c^2</hp:script>
      </hp:equation>
      <hp:run charPrIDRef="0">
        <hp:t>이 성립한다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("equation_inline", &sections);
}

/// Multi-line script promotes to a `$$ block $$`.
#[test]
fn golden_equation_block() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>분수:</hp:t>
      </hp:run>
      <hp:equation version="6.0">
        <hp:script>x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
y = \frac{c}{a}</hp:script>
      </hp:equation>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("equation_block", &sections);
}

/// Footnote is expanded inline as `[각주: …]` right after the text it
/// attaches to. Matches the existing `[이미지: …]` placeholder convention
/// so all annotations share one visual grammar in the extracted markdown.
#[test]
fn golden_footnote_inline_expansion() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>주요 사항</hp:t>
      </hp:run>
      <hp:footNote number="1" instId="100">
        <hp:subList>
          <hp:p>
            <hp:run charPrIDRef="0">
              <hp:t>근거: 공공기관 운영에 관한 법률 제2조.</hp:t>
            </hp:run>
          </hp:p>
        </hp:subList>
      </hp:footNote>
      <hp:run charPrIDRef="0">
        <hp:t>은 다음과 같다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("footnote_inline", &sections);
}

/// Header/footer controls share the subList structure of footnotes.
/// Emitted as `[머리말: …]` / `[꼬리말: …]` at their paragraph position so
/// downstream search and LLM pipelines can see the recurring per-page
/// content without confusing it with the main flow.
#[test]
fn golden_header_footer_inline() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>문서 본문 첫 문단.</hp:t>
      </hp:run>
      <hp:header pageRange="BOTH">
        <hp:subList>
          <hp:p>
            <hp:run charPrIDRef="0">
              <hp:t>행정안전부 보도자료</hp:t>
            </hp:run>
          </hp:p>
        </hp:subList>
      </hp:header>
      <hp:footer pageRange="BOTH">
        <hp:subList>
          <hp:p>
            <hp:run charPrIDRef="0">
              <hp:t>- 1 -</hp:t>
            </hp:run>
          </hp:p>
        </hp:subList>
      </hp:footer>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("header_footer_inline", &sections);
}

/// Endnote uses "미주" label instead of "각주" but is otherwise identical.
#[test]
fn golden_endnote_inline_expansion() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>본 보고서</hp:t>
      </hp:run>
      <hp:endNote number="1" instId="200">
        <hp:subList>
          <hp:p>
            <hp:run charPrIDRef="0">
              <hp:t>2026년 1분기 재정자료 참고.</hp:t>
            </hp:run>
          </hp:p>
        </hp:subList>
      </hp:endNote>
      <hp:run charPrIDRef="0">
        <hp:t>의 자료는 확정치이다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("endnote_inline", &sections);
}

/// Ruby (덧말 / 루비) annotation preserved as parenthetical. Base text
/// is in a sibling `<hp:run>` and must NOT be duplicated by the dutmal
/// mainText scan.
#[test]
fn golden_ruby_annotation_parenthetical() {
    let char_props = r##"
    <hh:charProperties itemCnt="1">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    // HWPX dutmal: mainText duplicated in flow, subText is the annotation.
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>한자</hp:t>
      </hp:run>
      <hp:dutmal posType="TOP" align="CENTER">
        <hp:mainText><hp:t>한자</hp:t></hp:mainText>
        <hp:subText><hp:t>hanja</hp:t></hp:subText>
      </hp:dutmal>
      <hp:run charPrIDRef="0">
        <hp:t>는 중요합니다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("ruby_annotation", &sections);
}

/// Emphasis dot (강조점) is preserved as `<mark>...</mark>` — Korean
/// government documents lean on this to highlight key terms.
#[test]
fn golden_emphasis_dot_preserved_as_mark() {
    let char_props = r##"
    <hh:charProperties itemCnt="2">
      <hh:charPr id="0" height="1000">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
      <hh:charPr id="1" height="1000" symMark="DOT">
        <hh:underline type="NONE"/>
        <hh:strikeout shape="NONE"/>
        <hh:fontRef hangul="0" latin="0" hanja="0" japanese="0" other="0" symbol="0" user="0"/>
      </hh:charPr>
    </hh:charProperties>
    "##;
    let section = r#"
    <hp:p>
      <hp:run charPrIDRef="0">
        <hp:t>신청자는 </hp:t>
      </hp:run>
      <hp:run charPrIDRef="1">
        <hp:t>반드시</hp:t>
      </hp:run>
      <hp:run charPrIDRef="0">
        <hp:t> 서류를 지참해야 합니다.</hp:t>
      </hp:run>
    </hp:p>
    "#;
    let sections = parse_synthetic(char_props, section);
    check_golden("emphasis_dot_preserved", &sections);
}
