use mdm_core::legal::annex::{AnnexParser, AnnexType};

#[test]
fn test_detect_regions_basic() {
    let text = "제1조(목적) 이 법은...\n\n별표 1 안전관리기준\n\n| 항목 | 기준 |\n| --- | --- |\n| 가스 | 0.1ppm |\n\n별표 2 벌금기준\n\n| 위반 | 금액 |\n| --- | --- |\n| 경미 | 50만원 |\n";

    let regions = AnnexParser::detect_regions(text);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].annex_type, AnnexType::Annex);
    assert_eq!(regions[0].number, 1);
    assert_eq!(regions[0].title, "안전관리기준");
    assert_eq!(regions[1].number, 2);
}

#[test]
fn test_detect_regions_form() {
    let text = "별지 제1호서식 신청서\n이름:\n주소:\n\n별지 제2호 보고서\n내용:";
    let regions = AnnexParser::detect_regions(text);
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].annex_type, AnnexType::Form);
    assert_eq!(regions[0].number, 1);
    assert_eq!(regions[1].number, 2);
}

#[test]
fn test_detect_regions_mixed() {
    let text = "본문 내용\n\n별표 1 기준표\n표 내용\n\n별지 제1호서식 양식\n양식 내용\n\n[첨부1] 참고서류\n서류 목록";
    let regions = AnnexParser::detect_regions(text);
    assert_eq!(regions.len(), 3);
    assert_eq!(regions[0].annex_type, AnnexType::Annex);
    assert_eq!(regions[1].annex_type, AnnexType::Form);
    assert_eq!(regions[2].annex_type, AnnexType::Attachment);
}

#[test]
fn test_detect_regions_with_sub_number() {
    let text = "별표1의2 세부기준\n세부 내용";
    let regions = AnnexParser::detect_regions(text);
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].number, 1);
    assert_eq!(regions[0].sub_number, Some(2));
}

#[test]
fn test_extract_from_text() {
    let text = "본문\n\n별표 1 기준\n항목A\n항목B\n\n별표 2 기준2\n항목C";
    let annexes = AnnexParser::extract_from_text(text);
    assert_eq!(annexes.len(), 2);
    assert_eq!(annexes[0].number, 1);
    assert!(annexes[0].raw_content.contains("항목A"));
    assert!(annexes[1].raw_content.contains("항목C"));
}

#[test]
fn test_no_annex_detected() {
    let text = "제1조(목적) 이 법은 목적으로 한다.\n제2조(정의) 이 법에서 사용하는 용어의 뜻은 다음과 같다.";
    let regions = AnnexParser::detect_regions(text);
    assert!(regions.is_empty());
}
