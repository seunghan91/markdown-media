//! HWPX 레이아웃 렌더러 통합 테스트 (feature = "hwpx-render").
//! 실제 파일 픽스처가 없으므로 최소 HWPX ZIP 을 인라인 조립한다
//! (관례: core/tests/golden_hwpx.rs::build_minimal_hwpx).

#![cfg(feature = "hwpx-render")]

use std::io::Write;
use zip::write::SimpleFileOptions;

const HEADER_XML: &str = r##"<?xml version="1.0" encoding="UTF-8"?>
<hh:head xmlns:hh="http://www.hancom.co.kr/hwpml/2011/head" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
 <hh:fontfaces>
  <hh:fontface lang="HANGUL">
   <hh:font id="0" face="함초롬바탕"/>
  </hh:fontface>
 </hh:fontfaces>
 <hh:charProperties>
  <hh:charPr id="0" height="1000" textColor="#000000">
   <hh:fontRef hangul="0"/>
   <hh:ratio hangul="100"/>
   <hh:spacing hangul="0"/>
  </hh:charPr>
 </hh:charProperties>
 <hh:paraProperties>
  <hh:paraPr id="0">
   <hh:align horizontal="JUSTIFY"/>
   <hh:lineSpacing type="PERCENT" value="160"/>
  </hh:paraPr>
 </hh:paraProperties>
 <hh:borderFills>
  <hh:borderFill id="1">
   <hh:leftBorder type="SOLID" width="0.12 mm" color="#000000"/>
   <hh:rightBorder type="SOLID" width="0.12 mm" color="#000000"/>
   <hh:topBorder type="SOLID" width="0.12 mm" color="#000000"/>
   <hh:bottomBorder type="SOLID" width="0.12 mm" color="#000000"/>
  </hh:borderFill>
 </hh:borderFills>
</hh:head>"##;

fn cell(col: u32, row: u32, text: &str) -> String {
    format!(
        r#"<hp:tc borderFillIDRef="1"><hp:cellAddr colAddr="{c}" rowAddr="{r}"/><hp:cellSpan colSpan="1" rowSpan="1"/><hp:cellSz width="10000" height="2000"/><hp:cellMargin left="141" right="141" top="141" bottom="141"/><hp:subList vertAlign="TOP"><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>{t}</hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1000" textheight="1000" baseline="850" spacing="0" horzpos="0" horzsize="9718"/></hp:linesegarray></hp:p></hp:subList></hp:tc>"#,
        c = col,
        r = row,
        t = text
    )
}

/// linesegarray 조판 캐시가 있는 섹션 (Tier-1).
fn section_cached() -> String {
    let table = format!(
        r#"<hp:tbl rowCnt="2" colCnt="2"><hp:sz width="20000" height="4000"/><hp:pos treatAsChar="1"/><hp:inMargin left="141" right="141" top="141" bottom="141"/><hp:tr>{a1}{b1}</hp:tr><hp:tr>{a2}{b2}</hp:tr></hp:tbl>"#,
        a1 = cell(0, 0, "A1"),
        b1 = cell(1, 0, "B1"),
        a2 = cell(0, 1, "A2"),
        b2 = cell(1, 1, "B2"),
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:secPr><hp:pagePr width="59528" height="84188"><hp:margin left="8504" right="8504" top="5668" bottom="4252" header="0" footer="0"/></hp:pagePr></hp:secPr></hp:run></hp:p>
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>안녕하세요 렌더 테스트</hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1000" textheight="1000" baseline="850" spacing="600" horzpos="0" horzsize="42520" flags="393216"/></hp:linesegarray></hp:p>
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0">{table}</hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="2000" vertsize="1000" textheight="1000" baseline="850" spacing="0" horzpos="0" horzsize="42520" flags="393216"/></hp:linesegarray></hp:p>
</hs:sec>"#,
        table = table
    )
}

/// linesegarray 없는 섹션 (reflow 필요).
fn section_no_cache() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:secPr><hp:pagePr width="59528" height="84188"><hp:margin left="8504" right="8504" top="5668" bottom="4252" header="0" footer="0"/></hp:pagePr></hp:secPr></hp:run></hp:p>
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>조판 캐시 없는 문단입니다 리플로우로 좌표를 합성합니다</hp:t></hp:run></hp:p>
</hs:sec>"#.to_string()
}

/// 거대 colAddr/rowSpan 셀 — 상한 클램프가 없으면 n_cols/n_rows 폭증 → OOM.
fn section_huge_table_addr() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<hs:sec xmlns:hs="http://www.hancom.co.kr/hwpml/2011/section" xmlns:hp="http://www.hancom.co.kr/hwpml/2011/paragraph" xmlns:hc="http://www.hancom.co.kr/hwpml/2011/core">
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:secPr><hp:pagePr width="59528" height="84188"><hp:margin left="8504" right="8504" top="5668" bottom="4252" header="0" footer="0"/></hp:pagePr></hp:secPr></hp:run></hp:p>
 <hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:tbl rowCnt="1" colCnt="1"><hp:sz width="20000" height="2000"/><hp:pos treatAsChar="1"/><hp:inMargin left="141" right="141" top="141" bottom="141"/><hp:tr><hp:tc borderFillIDRef="1"><hp:cellAddr colAddr="2147483647" rowAddr="0"/><hp:cellSpan colSpan="1" rowSpan="2147483647"/><hp:cellSz width="10000" height="2000"/><hp:cellMargin left="141" right="141" top="141" bottom="141"/><hp:subList vertAlign="TOP"><hp:p paraPrIDRef="0"><hp:run charPrIDRef="0"><hp:t>X</hp:t></hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1000" textheight="1000" baseline="850" spacing="0" horzpos="0" horzsize="9718"/></hp:linesegarray></hp:p></hp:subList></hp:tc></hp:tr></hp:tbl></hp:run><hp:linesegarray><hp:lineseg textpos="0" vertpos="0" vertsize="1000" textheight="1000" baseline="850" spacing="0" horzpos="0" horzsize="42520"/></hp:linesegarray></hp:p>
</hs:sec>"#.to_string()
}

fn build_hwpx(section_xml: &str) -> Vec<u8> {
    let buf = Vec::new();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(buf));
    // mimetype (STORED, 압축 없음)
    zip.start_file("mimetype", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored))
        .unwrap();
    zip.write_all(b"application/hwp+zip").unwrap();
    let opts = SimpleFileOptions::default();
    zip.start_file("version.xml", opts).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><hv:HCFVersion xmlns:hv="http://www.hancom.co.kr/hwpml/2011/version" tagetApplication="WORDPROCESSOR"/>"#).unwrap();
    zip.start_file("Contents/header.xml", opts).unwrap();
    zip.write_all(HEADER_XML.as_bytes()).unwrap();
    zip.start_file("Contents/section0.xml", opts).unwrap();
    zip.write_all(section_xml.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

use mdm_core::hwpx_render::{render_hwpx_svg, render_hwpx_svg_detailed, RenderError, RenderOptions, WrapMode};

#[test]
fn hwpx_render_cached_text_and_table() {
    let hwpx = build_hwpx(&section_cached());
    let out = render_hwpx_svg_detailed(&hwpx, &RenderOptions::default()).expect("render ok");
    assert_eq!(out.pages.len(), 1, "단일 페이지");
    let svg = &out.pages[0];
    // 텍스트가 포함되고
    assert!(svg.contains("안녕하세요"), "본문 텍스트 포함");
    assert!(svg.contains("A1") && svg.contains("B2"), "표 셀 텍스트 포함");
    // 표 테두리(line)와 텍스트 요소가 방출되고
    assert!(svg.contains("<line "), "표 테두리 line 방출");
    assert!(svg.contains("<text "), "text 요소 방출");
    // 자립 SVG 골격
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
    assert!(svg.contains("clip-path=\"url(#pgclip)\""));
    assert!(out.stats.tables >= 1 && out.stats.texts >= 3);
}

#[test]
fn hwpx_render_no_cache_then_reflow() {
    let hwpx = build_hwpx(&section_no_cache());
    // reflow 없이는 캐시 없음 에러
    let err = render_hwpx_svg(&hwpx, &RenderOptions::default()).unwrap_err();
    assert!(matches!(err, RenderError::NoCache), "캐시 없음 에러: {:?}", err);
    // reflow 켜면 좌표 합성 후 렌더
    let opts = RenderOptions { reflow: true, reflow_mode: WrapMode::Keep, ..Default::default() };
    let pages = render_hwpx_svg(&hwpx, &opts).expect("reflow render ok");
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("조판"), "reflow 로 본문 렌더");
    assert!(pages[0].contains("<text "));
}

#[test]
fn hwpx_render_huge_table_addr_no_oom() {
    // colAddr=2147483647, rowSpan=2147483647 — 상한 클램프(MAX_TABLE_DIM)로
    // n_cols/n_rows 가 폭증하지 않아 OOM 없이 정상 렌더된다.
    let hwpx = build_hwpx(&section_huge_table_addr());
    let out = render_hwpx_svg_detailed(&hwpx, &RenderOptions::default()).expect("거대 좌표에도 렌더 성공");
    assert_eq!(out.pages.len(), 1);
    assert!(out.stats.tables >= 1, "표가 그려짐");
    // 셀 내용이 방출됨(클램프 후에도 셀이 유효)
    assert!(out.pages[0].contains(">X<"), "클램프된 셀 콘텐츠 렌더");
}

#[test]
fn hwpx_render_rejects_non_zip() {
    let err = render_hwpx_svg(b"not a zip at all", &RenderOptions::default()).unwrap_err();
    assert!(matches!(err, RenderError::NotZip));
}

/// 회귀 스냅샷 — 캐시 픽스처 첫 페이지 SVG. UPDATE_GOLDEN=1 로 재생성.
#[test]
fn hwpx_render_snapshot_page1() {
    let hwpx = build_hwpx(&section_cached());
    let pages = render_hwpx_svg(&hwpx, &RenderOptions::default()).unwrap();
    let got = &pages[0];
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/hwpx_render_cached_page1.svg");
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(path, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).expect("golden 파일 (UPDATE_GOLDEN=1 로 생성)");
    assert_eq!(got, &want, "SVG 회귀 — 변경 의도면 UPDATE_GOLDEN=1 로 갱신");
}

// ─── 순수 로직 단위 테스트 ─────────────────────────

use mdm_core::hwpx_render as hr;

#[test]
fn hwpx_render_unit_pure_functions() {
    // pt(): HWPUNIT → pt 최소표기
    assert_eq!(hr::testonly::pt(1234.0), "12.34");
    assert_eq!(hr::testonly::pt(1200.0), "12");
    assert_eq!(hr::testonly::pt(1230.0), "12.3");
    assert_eq!(hr::testonly::pt(-30.0), "-0.3");

    // to_int32(): uint32 음수 복원
    assert_eq!(hr::testonly::to_int32(Some("4294967103"), 0.0), -193.0);
    assert_eq!(hr::testonly::to_int32(Some("100"), 0.0), 100.0);
    assert_eq!(hr::testonly::to_int32(None, 7.0), 7.0);

    // measure_text_width(): 한글 3자 × 10pt × 0.97em = 2910 HWPUNIT
    let w = hr::testonly::measure_hangul("가나다", 1000.0, 100.0);
    assert!((w - 2910.0).abs() < 0.5, "한글 폭 = {}", w);

    // simulate_wrap(): 좁은 폭에서 여러 줄
    let lines = hr::testonly::wrap_lines("가나다라마바사아자차카타파하", 3000.0, 3000.0, 1000.0, 100.0);
    assert!(lines >= 4, "좁은 폭 줄 수 = {}", lines);
}
