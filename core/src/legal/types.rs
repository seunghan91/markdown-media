//! Type definitions for Korean Legal Document Parser
//!
//! 한국 법령 문서 파싱에 필요한 타입 정의

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 법령 계층 구조 열거형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalHierarchy {
    /// 편 (Part)
    Part,
    /// 장 (Chapter)
    Chapter,
    /// 절 (Section)
    Section,
    /// 관 (Sub-Section)
    SubSection,
    /// 조 (Article) - 기본 단위
    Article,
    /// 항 (Paragraph)
    Paragraph,
    /// 호 (Subparagraph)
    SubParagraph,
    /// 목 (Item)
    Item,
}

impl LegalHierarchy {
    /// 한글 이름 반환
    pub fn korean_name(&self) -> &'static str {
        match self {
            LegalHierarchy::Part => "편",
            LegalHierarchy::Chapter => "장",
            LegalHierarchy::Section => "절",
            LegalHierarchy::SubSection => "관",
            LegalHierarchy::Article => "조",
            LegalHierarchy::Paragraph => "항",
            LegalHierarchy::SubParagraph => "호",
            LegalHierarchy::Item => "목",
        }
    }

    /// 계층 레벨 (상위가 낮은 숫자)
    pub fn level(&self) -> u8 {
        match self {
            LegalHierarchy::Part => 1,
            LegalHierarchy::Chapter => 2,
            LegalHierarchy::Section => 3,
            LegalHierarchy::SubSection => 4,
            LegalHierarchy::Article => 5,
            LegalHierarchy::Paragraph => 6,
            LegalHierarchy::SubParagraph => 7,
            LegalHierarchy::Item => 8,
        }
    }
}

/// 법조문 참조 정보
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegalReference {
    /// 참조 법률명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_law: Option<String>,
    /// 참조 조항
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_article: Option<String>,
    /// 참조 유형: internal | external
    pub reference_type: String,
    /// 원본 참조 텍스트
    pub raw_text: String,
}

impl LegalReference {
    pub fn internal(target_article: String, raw_text: String) -> Self {
        Self {
            target_law: None,
            target_article: Some(target_article),
            reference_type: "internal".to_string(),
            raw_text,
        }
    }

    pub fn external(target_law: String, target_article: Option<String>, raw_text: String) -> Self {
        Self {
            target_law: Some(target_law),
            target_article,
            reference_type: "external".to_string(),
            raw_text,
        }
    }
}

/// 법률 문서 메타데이터
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegalMetadata {
    /// 법령명
    pub law_name: String,
    /// 법령 ID
    pub law_id: String,
    /// 분류 (유가증권시장규정, 코스닥시장규정 등)
    pub category: String,
    /// 개정일
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_date: Option<String>,
    /// 개정차수
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_number: Option<String>,
    /// 시행일
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_date: Option<String>,

    // 계층 구조 정보
    /// 현재 편
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    /// 현재 장
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    /// 현재 절
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    /// 현재 관
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subsection: Option<String>,

    // 조항 정보
    /// 조 번호 (예: "1", "2")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_number: Option<String>,
    /// 조 제목 (예: "목적", "정의")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_title: Option<String>,
    /// 항 번호
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph_number: Option<String>,

    /// 참조 관계
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<LegalReference>,

    /// 원본 파일명
    pub source_file: String,
    /// 시작 라인
    pub line_start: usize,
    /// 종료 라인
    pub line_end: usize,
}

/// 청크 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    /// 조(Article) 단위 청크
    Article,
    /// 항(Paragraph) 단위 청크 (큰 조문 분할 시)
    Paragraph,
    /// 정의 청크
    Definition,
}

impl Default for ChunkType {
    fn default() -> Self {
        ChunkType::Article
    }
}

/// 법률 청크 데이터 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalChunk {
    /// 고유 ID (해시)
    pub id: String,
    /// 청크 내용
    pub content: String,
    /// 메타데이터
    pub metadata: LegalMetadata,
    /// 청크 유형
    pub chunk_type: ChunkType,
    /// 토큰 수 (추정)
    pub token_count: usize,
    /// 컨텍스트 경로 (예: "제1편 총칙 > 제1장 통칙 > 제1조")
    pub context_path: String,
    /// 상위 청크 ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_chunk_id: Option<String>,
}

impl LegalChunk {
    /// JSON 객체로 변환 (serde_json::Value)
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

/// 파싱 상태 추적
#[derive(Debug, Clone, Default)]
pub struct ParsingState {
    pub part: Option<String>,
    pub chapter: Option<String>,
    pub section: Option<String>,
    pub subsection: Option<String>,
    pub article: Option<String>,
    pub paragraph: Option<String>,
}

impl ParsingState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 상태 초기화
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// 편 설정 시 하위 계층 초기화
    pub fn set_part(&mut self, value: String) {
        self.part = Some(value);
        self.chapter = None;
        self.section = None;
        self.subsection = None;
    }

    /// 장 설정 시 하위 계층 초기화
    pub fn set_chapter(&mut self, value: String) {
        self.chapter = Some(value);
        self.section = None;
        self.subsection = None;
    }

    /// 절 설정 시 하위 계층 초기화
    pub fn set_section(&mut self, value: String) {
        self.section = Some(value);
        self.subsection = None;
    }

    /// 관 설정
    pub fn set_subsection(&mut self, value: String) {
        self.subsection = Some(value);
    }

    /// 조 설정
    pub fn set_article(&mut self, value: String) {
        self.article = Some(value);
        self.paragraph = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legal_hierarchy_korean_name() {
        assert_eq!(LegalHierarchy::Part.korean_name(), "편");
        assert_eq!(LegalHierarchy::Article.korean_name(), "조");
        assert_eq!(LegalHierarchy::Item.korean_name(), "목");
    }

    #[test]
    fn test_legal_hierarchy_level() {
        assert!(LegalHierarchy::Part.level() < LegalHierarchy::Chapter.level());
        assert!(LegalHierarchy::Article.level() < LegalHierarchy::Paragraph.level());
    }

    #[test]
    fn test_parsing_state() {
        let mut state = ParsingState::new();
        state.set_part("제1편 총칙".to_string());
        assert_eq!(state.part, Some("제1편 총칙".to_string()));
        
        state.set_chapter("제1장 통칙".to_string());
        assert_eq!(state.chapter, Some("제1장 통칙".to_string()));
        
        // 편 설정 시 장 초기화 확인
        state.set_part("제2편 시장".to_string());
        assert_eq!(state.part, Some("제2편 시장".to_string()));
        assert_eq!(state.chapter, None);
    }

    #[test]
    fn test_legal_reference() {
        let internal = LegalReference::internal(
            "제5조".to_string(),
            "제5조제1항".to_string(),
        );
        assert_eq!(internal.reference_type, "internal");
        assert!(internal.target_law.is_none());

        let external = LegalReference::external(
            "상법".to_string(),
            Some("제42조".to_string()),
            "「상법」 제42조".to_string(),
        );
        assert_eq!(external.reference_type, "external");
        assert_eq!(external.target_law, Some("상법".to_string()));
    }
}
