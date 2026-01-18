//! Regex patterns for Korean Legal Document Parser
//!
//! 한국 법령 구조 파싱을 위한 정규식 패턴 정의

use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

lazy_static! {
    /// 편(Part) 패턴: 제1편 총칙
    pub static ref RE_PART: Regex = Regex::new(r"^제(\d+)편\s*(.*)$").unwrap();
    
    /// 장(Chapter) 패턴: 제1장 통칙
    pub static ref RE_CHAPTER: Regex = Regex::new(r"^제(\d+)장\s*(.*)$").unwrap();
    
    /// 절(Section) 패턴: 제1절 목적
    pub static ref RE_SECTION: Regex = Regex::new(r"^제(\d+)절\s*(.*)$").unwrap();
    
    /// 관(Sub-Section) 패턴: 제1관 정의
    pub static ref RE_SUBSECTION: Regex = Regex::new(r"^제(\d+)관\s*(.*)$").unwrap();
    
    /// 조(Article) 패턴: 제1조(목적), 제2조의2(정의)
    /// Groups: (1) 조 번호, (2) 가지번호(의X), (3) 조 제목
    pub static ref RE_ARTICLE: Regex = Regex::new(
        r"^제(\d+)조(?:의(\d+))?(?:\(([^)]+)\))?"
    ).unwrap();
    
    /// 항(Paragraph) 패턴: 원문자(①②③...) 또는 괄호 숫자((1)(2)...)
    pub static ref RE_PARAGRAPH: Regex = Regex::new(
        r"^([①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳]|\(\d+\))\s*"
    ).unwrap();
    
    /// 호(Subparagraph) 패턴: 1. 2. 3.
    pub static ref RE_SUBPARAGRAPH: Regex = Regex::new(r"^(\d+)\.\s*").unwrap();
    
    /// 목(Item) 패턴: 가. 나. 다.
    pub static ref RE_ITEM: Regex = Regex::new(
        r"^([가나다라마바사아자차카타파하])\.\s*"
    ).unwrap();
    
    /// 세부 목 패턴: (1) (2) (3)
    pub static ref RE_SUBITEM: Regex = Regex::new(r"^\((\d+)\)\s*").unwrap();
    
    /// 외부 법률 참조 패턴: 「상법」 제42조제1항제2호
    pub static ref RE_LAW_REFERENCE: Regex = Regex::new(
        r"「([^」]+)」(?:\s*제(\d+)조(?:의(\d+))?(?:제(\d+)항)?(?:제(\d+)호)?)?"
    ).unwrap();
    
    /// 내부 참조 패턴: 제5조제1항제2호가목
    pub static ref RE_INTERNAL_REFERENCE: Regex = Regex::new(
        r"제(\d+)조(?:의(\d+))?(?:제(\d+)항)?(?:제(\d+)호)?(?:([가-하])목)?"
    ).unwrap();
    
    /// 개정 정보 패턴: [일부개정 2024. 1. 15. <시행일: 2024-02-01>]
    pub static ref RE_REVISION: Regex = Regex::new(
        r"\[(?:일부)?개정\s*(\d{4})\.\s*(\d{1,2})\.\s*(\d{1,2}).*?(?:<시행일\s*:\s*(\d{4}-\d{2}-\d{2})>)?\]"
    ).unwrap();
    
    /// 개정 차수 패턴: 제15차 일부개정
    pub static ref RE_REVISION_NUMBER: Regex = Regex::new(r"제(\d+)차\s*(?:일부)?개정").unwrap();
    
    /// 한글 계산용 패턴
    pub static ref RE_KOREAN: Regex = Regex::new(r"[가-힣]").unwrap();
    
    /// 영숫자 패턴
    pub static ref RE_ALPHANUMERIC: Regex = Regex::new(r"[a-zA-Z0-9]").unwrap();
    
    /// 공백 패턴
    pub static ref RE_WHITESPACE: Regex = Regex::new(r"\s").unwrap();
    
    /// 원문자-숫자 매핑
    pub static ref CIRCLED_NUMBERS: HashMap<char, u8> = {
        let mut m = HashMap::new();
        m.insert('①', 1);
        m.insert('②', 2);
        m.insert('③', 3);
        m.insert('④', 4);
        m.insert('⑤', 5);
        m.insert('⑥', 6);
        m.insert('⑦', 7);
        m.insert('⑧', 8);
        m.insert('⑨', 9);
        m.insert('⑩', 10);
        m.insert('⑪', 11);
        m.insert('⑫', 12);
        m.insert('⑬', 13);
        m.insert('⑭', 14);
        m.insert('⑮', 15);
        m.insert('⑯', 16);
        m.insert('⑰', 17);
        m.insert('⑱', 18);
        m.insert('⑲', 19);
        m.insert('⑳', 20);
        m
    };
}

/// 한글 목 번호 배열
pub const KOREAN_ITEMS: [char; 14] = [
    '가', '나', '다', '라', '마', '바', '사',
    '아', '자', '차', '카', '타', '파', '하',
];

/// 원문자를 숫자로 변환
pub fn circled_to_number(c: char) -> Option<u8> {
    CIRCLED_NUMBERS.get(&c).copied()
}

/// 문자열에서 첫 번째 원문자를 숫자로 변환
pub fn parse_circled_number(s: &str) -> Option<u8> {
    s.chars().next().and_then(circled_to_number)
}

/// 괄호 숫자 파싱: "(1)" -> 1
pub fn parse_paren_number(s: &str) -> Option<u8> {
    let trimmed = s.trim_start_matches('(').trim_end_matches(')');
    trimmed.parse().ok()
}

/// 조 번호 문자열 생성: (1, Some(2)) -> "제1조의2"
pub fn format_article_number(num: &str, branch: Option<&str>) -> String {
    match branch {
        Some(b) => format!("제{}조의{}", num, b),
        None => format!("제{}조", num),
    }
}

/// 조 번호와 제목 문자열 생성: (1, Some(2), Some("정의")) -> "제1조의2(정의)"
pub fn format_article_with_title(num: &str, branch: Option<&str>, title: Option<&str>) -> String {
    let article = format_article_number(num, branch);
    match title {
        Some(t) if !t.is_empty() => format!("{}({})", article, t),
        _ => article,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_re_part() {
        let caps = RE_PART.captures("제1편 총칙").unwrap();
        assert_eq!(&caps[1], "1");
        assert_eq!(&caps[2], "총칙");
    }

    #[test]
    fn test_re_chapter() {
        let caps = RE_CHAPTER.captures("제2장 시장조직").unwrap();
        assert_eq!(&caps[1], "2");
        assert_eq!(&caps[2], "시장조직");
    }

    #[test]
    fn test_re_article() {
        // 기본 조
        let caps = RE_ARTICLE.captures("제1조(목적)").unwrap();
        assert_eq!(&caps[1], "1");
        assert!(caps.get(2).is_none() || caps.get(2).map(|m| m.as_str()).unwrap_or("").is_empty());
        assert_eq!(caps.get(3).map(|m| m.as_str()), Some("목적"));

        // 가지번호가 있는 조
        let caps2 = RE_ARTICLE.captures("제2조의3(정의)").unwrap();
        assert_eq!(&caps2[1], "2");
        assert_eq!(caps2.get(2).map(|m| m.as_str()), Some("3"));
        assert_eq!(caps2.get(3).map(|m| m.as_str()), Some("정의"));

        // 제목이 없는 조
        let caps3 = RE_ARTICLE.captures("제5조").unwrap();
        assert_eq!(&caps3[1], "5");
        assert!(caps3.get(3).is_none());
    }

    #[test]
    fn test_re_paragraph() {
        assert!(RE_PARAGRAPH.is_match("① 이 규정은..."));
        assert!(RE_PARAGRAPH.is_match("⑳ 마지막 항"));
        assert!(RE_PARAGRAPH.is_match("(1) 괄호 숫자"));
    }

    #[test]
    fn test_re_subparagraph() {
        let caps = RE_SUBPARAGRAPH.captures("1. 첫 번째 호").unwrap();
        assert_eq!(&caps[1], "1");
    }

    #[test]
    fn test_re_item() {
        let caps = RE_ITEM.captures("가. 첫 번째 목").unwrap();
        assert_eq!(&caps[1], "가");
    }

    #[test]
    fn test_re_law_reference() {
        let caps = RE_LAW_REFERENCE.captures("「상법」 제42조제1항").unwrap();
        assert_eq!(&caps[1], "상법");
        assert_eq!(caps.get(2).map(|m| m.as_str()), Some("42"));
        assert_eq!(caps.get(4).map(|m| m.as_str()), Some("1"));
    }

    #[test]
    fn test_re_internal_reference() {
        let caps = RE_INTERNAL_REFERENCE.captures("제5조제1항제2호가목").unwrap();
        assert_eq!(&caps[1], "5");
        assert_eq!(caps.get(3).map(|m| m.as_str()), Some("1"));
        assert_eq!(caps.get(4).map(|m| m.as_str()), Some("2"));
        assert_eq!(caps.get(5).map(|m| m.as_str()), Some("가"));
    }

    #[test]
    fn test_circled_to_number() {
        assert_eq!(circled_to_number('①'), Some(1));
        assert_eq!(circled_to_number('⑩'), Some(10));
        assert_eq!(circled_to_number('⑳'), Some(20));
        assert_eq!(circled_to_number('A'), None);
    }

    #[test]
    fn test_parse_paren_number() {
        assert_eq!(parse_paren_number("(1)"), Some(1));
        assert_eq!(parse_paren_number("(15)"), Some(15));
    }

    #[test]
    fn test_format_article_number() {
        assert_eq!(format_article_number("1", None), "제1조");
        assert_eq!(format_article_number("2", Some("3")), "제2조의3");
    }

    #[test]
    fn test_format_article_with_title() {
        assert_eq!(
            format_article_with_title("1", None, Some("목적")),
            "제1조(목적)"
        );
        assert_eq!(
            format_article_with_title("2", Some("3"), Some("정의")),
            "제2조의3(정의)"
        );
        assert_eq!(
            format_article_with_title("5", None, None),
            "제5조"
        );
    }
}
