# Rust 한국 법률 문서 파서 설계서

> Korean Legal Document Parser Design Specification
>
> 버전: 1.0.0
> 작성일: 2026-01-18
> 작성자: Claude Code

---

## 1. 개요

### 1.1 목적

Python으로 구현된 `legal_chunker.py`의 한국 법률 문서 파싱 로직을 Rust로 포팅하여 성능을 개선하고, 기존 `markdown-media` Rust 파서의 HWP 텍스트 추출 버그를 수정한다.

### 1.2 범위

1. **HWP 파서 버그 수정**: 확장 제어 문자(0x16-0x1F) 처리 로직 수정
2. **한국 법률 청커 구현**: 법령 구조 파싱 및 의미적 청킹
3. **JSONL 내보내기**: WeKnora RAG 시스템용 출력 포맷

### 1.3 용어 정의

| 용어 | 정의 |
|------|------|
| HWP | 한글과컴퓨터의 문서 파일 형식 (Hangul Word Processor) |
| OLE | Object Linking and Embedding, HWP 5.0의 컨테이너 형식 |
| 확장 제어 문자 | HWP에서 16바이트(2+14)를 소비하는 특수 문자 코드 |
| 청크(Chunk) | 벡터 임베딩을 위해 분할된 텍스트 단위 |
| 조(Article) | 한국 법령의 기본 단위 (예: 제1조) |

---

## 2. 현황 분석

### 2.1 문제점

#### 2.1.1 Rust 파서의 확장 제어 문자 처리 버그

**현재 코드** (`record.rs:231-238`):
```rust
match char_code {
    0x01..=0x08 | 0x0B | 0x0C | 0x0E | 0x0F | 0x10..=0x15 => {
        i = (i + 14).min(data.len());  // 14바이트 스킵 ✅
    }
    0x16..=0x1F => continue,  // 14바이트 스킵 안함 ❌
}
```

**Python 코드** (`hwp_converter.py:136-139`):
```python
EXTENDED_CTRL_CHARS = {
    0x01, 0x02, ..., 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F  # 추가
}
if char_code in EXTENDED_CTRL_CHARS:
    i += 14  # 모든 확장 제어 문자에서 14바이트 스킵 ✅
```

**영향**: 0x16-0x1F 코드가 포함된 HWP 파일에서 텍스트 추출 시 데이터 오프셋이 어긋나 잘못된 텍스트가 추출됨.

#### 2.1.2 Rust에 법률 구조 파싱 기능 부재

Python `legal_chunker.py`에서 제공하는 다음 기능이 Rust에 없음:
- 법령 계층 구조 파싱 (편/장/절/관/조/항/호/목)
- 조(Article) 단위 의미적 청킹
- 법조문 참조 관계 추출
- JSONL 출력

### 2.2 Python 파서 분석

#### 데이터 흐름
```
Markdown 파일
    ↓
parse_metadata_header() → LegalMetadata 추출
    ↓
parse_article_block() → 조 단위 파싱
    ↓
chunk_large_article() → 큰 조문 분할
    ↓
export_to_jsonl() → JSONL 출력
```

#### 핵심 데이터 구조
```python
@dataclass
class LegalChunk:
    id: str                    # SHA256 해시
    content: str               # 청크 내용
    metadata: LegalMetadata    # 메타데이터
    chunk_type: str            # article | paragraph | definition
    token_count: int           # 토큰 수
    context_path: str          # "제1편 > 제1장 > 제1조"
    parent_chunk_id: Optional[str]
```

---

## 3. 아키텍처 설계

### 3.1 모듈 구조

```
core/
├── src/
│   ├── hwp/
│   │   ├── mod.rs           # 기존
│   │   ├── parser.rs        # 기존
│   │   ├── record.rs        # 🔧 수정 (확장 제어 문자)
│   │   └── ole.rs           # 기존
│   │
│   ├── legal/               # 🆕 신규 모듈
│   │   ├── mod.rs           # 모듈 선언
│   │   ├── types.rs         # 데이터 타입 정의
│   │   ├── patterns.rs      # 정규식 패턴
│   │   ├── chunker.rs       # 청킹 로직
│   │   └── exporter.rs      # JSONL 내보내기
│   │
│   ├── lib.rs               # 🔧 수정 (legal 모듈 추가)
│   └── main.rs              # 🔧 수정 (CLI 명령 추가)
│
├── tests/
│   └── legal_tests.rs       # 🆕 통합 테스트
│
└── Cargo.toml               # 🔧 수정 (의존성 추가)
```

### 3.2 의존성

```toml
[dependencies]
# 기존 의존성
cfb = "0.7"
flate2 = "1.0"
miniz_oxide = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 신규 의존성
regex = "1.10"
lazy_static = "1.4"
sha2 = "0.10"
```

---

## 4. 상세 설계

### 4.1 HWP 파서 수정 (`record.rs`)

#### 4.1.1 확장 제어 문자 상수 수정

**Before:**
```rust
pub const EXTENDED_CTRL_CHARS: [u16; 18] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x0B, 0x0C, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
];
```

**After:**
```rust
pub const EXTENDED_CTRL_CHARS: [u16; 28] = [
    0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    0x0B, 0x0C, 0x0E, 0x0F,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15,
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,  // 추가
];
```

#### 4.1.2 `extract_para_text()` 함수 수정

**Before:**
```rust
0x16..=0x1F => continue,
```

**After:**
```rust
0x16..=0x1F => {
    // Extended control characters - skip 14 bytes payload
    i = (i + 14).min(data.len());
}
```

### 4.2 타입 정의 (`legal/types.rs`)

```rust
use serde::{Deserialize, Serialize};

/// 법령 계층 구조
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalHierarchy {
    Part,        // 편
    Chapter,     // 장
    Section,     // 절
    SubSection,  // 관
    Article,     // 조
    Paragraph,   // 항
    SubParagraph,// 호
    Item,        // 목
}

impl LegalHierarchy {
    pub fn korean_name(&self) -> &'static str {
        match self {
            Self::Part => "편",
            Self::Chapter => "장",
            Self::Section => "절",
            Self::SubSection => "관",
            Self::Article => "조",
            Self::Paragraph => "항",
            Self::SubParagraph => "호",
            Self::Item => "목",
        }
    }

    /// 상위 계층 여부
    pub fn is_structural(&self) -> bool {
        matches!(self, Self::Part | Self::Chapter | Self::Section | Self::SubSection)
    }
}

/// 법조문 참조 정보
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegalReference {
    pub target_law: Option<String>,
    pub target_article: Option<String>,
    #[serde(default)]
    pub reference_type: ReferenceType,
    pub raw_text: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceType {
    #[default]
    Internal,
    External,
}

/// 법률 문서 메타데이터
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegalMetadata {
    pub law_name: String,
    pub law_id: String,
    pub category: String,
    pub revision_date: Option<String>,
    pub revision_number: Option<String>,
    pub effective_date: Option<String>,

    // 계층 구조
    pub part: Option<String>,
    pub chapter: Option<String>,
    pub section: Option<String>,
    pub subsection: Option<String>,

    // 조항 정보
    pub article_number: Option<String>,
    pub article_title: Option<String>,
    pub paragraph_number: Option<String>,

    // 참조 관계
    #[serde(default)]
    pub references: Vec<LegalReference>,

    // 원본 위치
    pub source_file: String,
    pub line_start: usize,
    pub line_end: usize,
}

/// 청크 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    Article,
    Paragraph,
    Definition,
}

impl Default for ChunkType {
    fn default() -> Self {
        Self::Article
    }
}

/// 법률 청크
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalChunk {
    pub id: String,
    pub content: String,
    pub metadata: LegalMetadata,
    #[serde(default)]
    pub chunk_type: ChunkType,
    pub token_count: usize,
    pub context_path: String,
    pub parent_chunk_id: Option<String>,
}

impl LegalChunk {
    pub fn to_embedding_format(&self, include_context: bool) -> EmbeddingData {
        let enhanced_content = if include_context && !self.context_path.is_empty() {
            format!("[{}]\n\n{}", self.context_path, self.content)
        } else {
            self.content.clone()
        };

        EmbeddingData {
            id: self.id.clone(),
            content: enhanced_content,
            raw_content: self.content.clone(),
            metadata: self.to_metadata_map(),
        }
    }

    fn to_metadata_map(&self) -> EmbeddingMetadata {
        EmbeddingMetadata {
            law_name: self.metadata.law_name.clone(),
            law_id: self.metadata.law_id.clone(),
            category: self.metadata.category.clone(),
            revision_date: self.metadata.revision_date.clone(),
            effective_date: self.metadata.effective_date.clone(),
            hierarchy: HierarchyInfo {
                part: self.metadata.part.clone(),
                chapter: self.metadata.chapter.clone(),
                section: self.metadata.section.clone(),
                subsection: self.metadata.subsection.clone(),
            },
            article: ArticleInfo {
                number: self.metadata.article_number.clone(),
                title: self.metadata.article_title.clone(),
                paragraph: self.metadata.paragraph_number.clone(),
            },
            references: self.metadata.references.clone(),
            source_file: self.metadata.source_file.clone(),
            chunk_type: self.chunk_type,
            token_count: self.token_count,
            context_path: self.context_path.clone(),
        }
    }
}

/// 임베딩용 출력 데이터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub id: String,
    pub content: String,
    pub raw_content: String,
    pub metadata: EmbeddingMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    pub law_name: String,
    pub law_id: String,
    pub category: String,
    pub revision_date: Option<String>,
    pub effective_date: Option<String>,
    pub hierarchy: HierarchyInfo,
    pub article: ArticleInfo,
    pub references: Vec<LegalReference>,
    pub source_file: String,
    pub chunk_type: ChunkType,
    pub token_count: usize,
    pub context_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchyInfo {
    pub part: Option<String>,
    pub chapter: Option<String>,
    pub section: Option<String>,
    pub subsection: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArticleInfo {
    pub number: Option<String>,
    pub title: Option<String>,
    pub paragraph: Option<String>,
}
```

### 4.3 정규식 패턴 (`legal/patterns.rs`)

```rust
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::HashMap;

lazy_static! {
    // ===== 구조 패턴 =====

    /// 편 패턴: 제1편 총칙
    pub static ref RE_PART: Regex =
        Regex::new(r"^제(\d+)편\s*(.*)$").unwrap();

    /// 장 패턴: 제1장 통칙
    pub static ref RE_CHAPTER: Regex =
        Regex::new(r"^제(\d+)장\s*(.*)$").unwrap();

    /// 절 패턴: 제1절 목적
    pub static ref RE_SECTION: Regex =
        Regex::new(r"^제(\d+)절\s*(.*)$").unwrap();

    /// 관 패턴: 제1관 정의
    pub static ref RE_SUBSECTION: Regex =
        Regex::new(r"^제(\d+)관\s*(.*)$").unwrap();

    // ===== 조항 패턴 =====

    /// 조 패턴: 제1조(목적), 제2조의2(정의)
    pub static ref RE_ARTICLE: Regex =
        Regex::new(r"^제(\d+)조(?:의(\d+))?(?:\(([^)]+)\))?").unwrap();

    /// 항 패턴: ① ② ③ 또는 (1) (2) (3)
    pub static ref RE_PARAGRAPH: Regex =
        Regex::new(r"^([①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳]|\(\d+\))\s*").unwrap();

    /// 호 패턴: 1. 2. 3.
    pub static ref RE_SUBPARAGRAPH: Regex =
        Regex::new(r"^(\d+)\.\s*").unwrap();

    /// 목 패턴: 가. 나. 다.
    pub static ref RE_ITEM: Regex =
        Regex::new(r"^([가나다라마바사아자차카타파하])\.\s*").unwrap();

    /// 세부 목 패턴: (1) (2) (3)
    pub static ref RE_SUBITEM: Regex =
        Regex::new(r"^\((\d+)\)\s*").unwrap();

    // ===== 참조 패턴 =====

    /// 외부 법률 참조: 「법률명」제1조제2항제3호
    pub static ref RE_LAW_REFERENCE: Regex =
        Regex::new(r"「([^」]+)」(?:\s*제(\d+)조(?:의(\d+))?(?:제(\d+)항)?(?:제(\d+)호)?)?").unwrap();

    /// 내부 참조: 제1조제2항제3호가목
    pub static ref RE_INTERNAL_REFERENCE: Regex =
        Regex::new(r"제(\d+)조(?:의(\d+))?(?:제(\d+)항)?(?:제(\d+)호)?(?:([가-하])목)?").unwrap();

    // ===== 메타데이터 패턴 =====

    /// 개정 정보: [일부개정 2024. 1. 1. <시행일: 2024-01-01>]
    pub static ref RE_REVISION: Regex =
        Regex::new(r"\[(?:일부)?개정\s*(\d{4})\.\s*(\d{1,2})\.\s*(\d{1,2}).*?(?:<시행일\s*:\s*(\d{4}-\d{2}-\d{2})>)?\]").unwrap();

    /// 개정 차수: 제5차 일부개정
    pub static ref RE_REVISION_NUMBER: Regex =
        Regex::new(r"제(\d+)차\s*(?:일부)?개정").unwrap();

    // ===== 매핑 테이블 =====

    /// 원문자 → 숫자 매핑
    pub static ref CIRCLED_NUMBERS: HashMap<char, u8> = {
        let mut m = HashMap::new();
        m.insert('①', 1);  m.insert('②', 2);  m.insert('③', 3);
        m.insert('④', 4);  m.insert('⑤', 5);  m.insert('⑥', 6);
        m.insert('⑦', 7);  m.insert('⑧', 8);  m.insert('⑨', 9);
        m.insert('⑩', 10); m.insert('⑪', 11); m.insert('⑫', 12);
        m.insert('⑬', 13); m.insert('⑭', 14); m.insert('⑮', 15);
        m.insert('⑯', 16); m.insert('⑰', 17); m.insert('⑱', 18);
        m.insert('⑲', 19); m.insert('⑳', 20);
        m
    };

    /// 한글 목 문자 배열
    pub static ref KOREAN_ITEMS: [char; 14] = [
        '가', '나', '다', '라', '마', '바', '사',
        '아', '자', '차', '카', '타', '파', '하'
    ];
}

/// 원문자를 숫자로 변환
pub fn circled_to_number(c: char) -> Option<u8> {
    CIRCLED_NUMBERS.get(&c).copied()
}

/// 한글 목 문자의 인덱스 반환
pub fn korean_item_index(c: char) -> Option<usize> {
    KOREAN_ITEMS.iter().position(|&x| x == c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_article_pattern() {
        let cases = [
            ("제1조(목적)", Some(("1", None, Some("목적")))),
            ("제2조의2(정의)", Some(("2", Some("2"), Some("정의")))),
            ("제10조", Some(("10", None, None))),
        ];

        for (input, expected) in cases {
            let caps = RE_ARTICLE.captures(input);
            match expected {
                Some((num, sub, title)) => {
                    let c = caps.expect("Should match");
                    assert_eq!(c.get(1).map(|m| m.as_str()), Some(num));
                    assert_eq!(c.get(2).map(|m| m.as_str()), sub);
                    assert_eq!(c.get(3).map(|m| m.as_str()), title);
                }
                None => assert!(caps.is_none()),
            }
        }
    }

    #[test]
    fn test_circled_numbers() {
        assert_eq!(circled_to_number('①'), Some(1));
        assert_eq!(circled_to_number('⑩'), Some(10));
        assert_eq!(circled_to_number('⑳'), Some(20));
        assert_eq!(circled_to_number('A'), None);
    }

    #[test]
    fn test_korean_items() {
        assert_eq!(korean_item_index('가'), Some(0));
        assert_eq!(korean_item_index('나'), Some(1));
        assert_eq!(korean_item_index('하'), Some(13));
        assert_eq!(korean_item_index('힣'), None);
    }
}
```

### 4.4 청커 구현 (`legal/chunker.rs`)

```rust
use crate::legal::patterns::*;
use crate::legal::types::*;
use sha2::{Sha256, Digest};
use std::collections::HashMap;
use std::path::Path;
use std::fs;

/// 한국 법률 문서 청커
pub struct KoreanLegalChunker {
    /// 조(Article) 단위로 청킹할지 여부
    pub chunk_by_article: bool,
    /// 상위 계층 컨텍스트 포함 여부
    pub include_context: bool,
    /// 최대 청크 토큰 수
    pub max_chunk_tokens: usize,
    /// 청크 간 오버랩 토큰 수
    pub overlap_tokens: usize,

    /// 현재 파싱 상태
    current_state: ParsingState,
}

#[derive(Debug, Clone, Default)]
struct ParsingState {
    part: Option<String>,
    chapter: Option<String>,
    section: Option<String>,
    subsection: Option<String>,
    article: Option<String>,
    paragraph: Option<String>,
}

impl Default for KoreanLegalChunker {
    fn default() -> Self {
        Self {
            chunk_by_article: true,
            include_context: true,
            max_chunk_tokens: 512,
            overlap_tokens: 50,
            current_state: ParsingState::default(),
        }
    }
}

impl KoreanLegalChunker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_tokens(mut self, tokens: usize) -> Self {
        self.max_chunk_tokens = tokens;
        self
    }

    /// 토큰 수 추정 (한글은 대략 1.5자당 1토큰)
    pub fn estimate_tokens(&self, text: &str) -> usize {
        let korean: usize = text.chars()
            .filter(|c| ('\u{AC00}'..='\u{D7A3}').contains(c))
            .count();
        let alphanumeric: usize = text.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .count();
        let spaces: usize = text.chars()
            .filter(|c| c.is_whitespace())
            .count();

        (korean as f64 / 1.5 + alphanumeric as f64 / 4.0 + spaces as f64 / 4.0) as usize
    }

    /// 청크 고유 ID 생성 (SHA256 기반)
    fn generate_chunk_id(&self, content: &str, metadata: &LegalMetadata) -> String {
        let unique_str = format!(
            "{}:{}:{}",
            metadata.law_name,
            metadata.article_number.as_deref().unwrap_or(""),
            &content[..content.len().min(100)]
        );

        let mut hasher = Sha256::new();
        hasher.update(unique_str.as_bytes());
        let result = hasher.finalize();
        hex::encode(&result[..8])  // 16자 hex
    }

    /// 마크다운 헤더에서 메타데이터 추출
    fn parse_metadata_header(&self, lines: &[&str]) -> (LegalMetadata, usize) {
        let mut metadata = LegalMetadata::default();
        let mut body_start = 0;

        for (i, line) in lines.iter().enumerate() {
            let line = line.trim();

            // 제목 (# 법령명)
            if line.starts_with("# ") {
                metadata.law_name = line[2..].trim().to_string();
                continue;
            }

            // 규정 ID
            if line.starts_with("- **규정 ID**:") || line.starts_with("- **규정ID**:") {
                metadata.law_id = line.split(':').last().unwrap_or("").trim().to_string();
                continue;
            }

            // 분류
            if line.starts_with("- **분류**:") {
                metadata.category = line.split(':').last().unwrap_or("").trim().to_string();
                continue;
            }

            // 개정 정보
            if let Some(caps) = RE_REVISION.captures(line) {
                if let (Some(y), Some(m), Some(d)) = (caps.get(1), caps.get(2), caps.get(3)) {
                    metadata.revision_date = Some(format!(
                        "{}-{:0>2}-{:0>2}",
                        y.as_str(),
                        m.as_str(),
                        d.as_str()
                    ));
                }
                if let Some(eff) = caps.get(4) {
                    metadata.effective_date = Some(eff.as_str().to_string());
                }
            }

            // 개정 차수
            if let Some(caps) = RE_REVISION_NUMBER.captures(line) {
                if let Some(num) = caps.get(1) {
                    metadata.revision_number = Some(num.as_str().to_string());
                }
            }

            // 본문 시작 감지
            if RE_PART.is_match(line) || RE_CHAPTER.is_match(line) || RE_ARTICLE.is_match(line) {
                body_start = i;
                break;
            }
        }

        (metadata, body_start)
    }

    /// 법조문 참조 관계 추출
    fn extract_references(&self, text: &str) -> Vec<LegalReference> {
        let mut references = Vec::new();

        // 외부 법률 참조
        for caps in RE_LAW_REFERENCE.captures_iter(text) {
            let target_article = caps.get(2).map(|m| {
                let mut article = format!("제{}조", m.as_str());
                if let Some(sub) = caps.get(3) {
                    article.push_str(&format!("의{}", sub.as_str()));
                }
                article
            });

            references.push(LegalReference {
                target_law: caps.get(1).map(|m| m.as_str().to_string()),
                target_article,
                reference_type: ReferenceType::External,
                raw_text: caps.get(0).unwrap().as_str().to_string(),
            });
        }

        // 내부 참조 (이미 외부 참조로 처리된 것 제외)
        for caps in RE_INTERNAL_REFERENCE.captures_iter(text) {
            let raw = caps.get(0).unwrap().as_str();
            if references.iter().any(|r| r.raw_text.contains(raw)) {
                continue;
            }

            let mut article = format!("제{}조", caps.get(1).unwrap().as_str());
            if let Some(sub) = caps.get(2) {
                article.push_str(&format!("의{}", sub.as_str()));
            }

            references.push(LegalReference {
                target_law: None,
                target_article: Some(article),
                reference_type: ReferenceType::Internal,
                raw_text: raw.to_string(),
            });
        }

        references
    }

    /// 현재 계층 경로 문자열 생성
    fn build_context_path(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref p) = self.current_state.part {
            parts.push(p.clone());
        }
        if let Some(ref c) = self.current_state.chapter {
            parts.push(c.clone());
        }
        if let Some(ref s) = self.current_state.section {
            parts.push(s.clone());
        }
        if let Some(ref ss) = self.current_state.subsection {
            parts.push(ss.clone());
        }
        if let Some(ref a) = self.current_state.article {
            parts.push(a.clone());
        }

        parts.join(" > ")
    }

    /// 마크다운 파일 파싱
    pub fn parse_markdown(&mut self, filepath: &Path) -> Result<Vec<LegalChunk>, std::io::Error> {
        let content = fs::read_to_string(filepath)?;
        let lines: Vec<&str> = content.lines().collect();

        // 메타데이터 헤더 파싱
        let (mut base_metadata, body_start) = self.parse_metadata_header(&lines);
        base_metadata.source_file = filepath
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // 상태 초기화
        self.current_state = ParsingState::default();

        let mut chunks = Vec::new();
        let mut current_idx = body_start;

        while current_idx < lines.len() {
            let line = lines[current_idx].trim();

            // 빈 줄 스킵
            if line.is_empty() {
                current_idx += 1;
                continue;
            }

            // 편/장/절/관 업데이트
            if let Some(caps) = RE_PART.captures(line) {
                self.current_state.part = Some(format!(
                    "제{}편 {}",
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                ).trim().to_string());
                self.current_state.chapter = None;
                self.current_state.section = None;
                self.current_state.subsection = None;
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_CHAPTER.captures(line) {
                self.current_state.chapter = Some(format!(
                    "제{}장 {}",
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                ).trim().to_string());
                self.current_state.section = None;
                self.current_state.subsection = None;
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SECTION.captures(line) {
                self.current_state.section = Some(format!(
                    "제{}절 {}",
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                ).trim().to_string());
                self.current_state.subsection = None;
                current_idx += 1;
                continue;
            }

            if let Some(caps) = RE_SUBSECTION.captures(line) {
                self.current_state.subsection = Some(format!(
                    "제{}관 {}",
                    caps.get(1).unwrap().as_str(),
                    caps.get(2).map(|m| m.as_str()).unwrap_or("")
                ).trim().to_string());
                current_idx += 1;
                continue;
            }

            // 조 파싱
            if RE_ARTICLE.is_match(line) {
                let (chunk, next_idx) = self.parse_article_block(&lines, current_idx, &base_metadata);
                if !chunk.content.is_empty() {
                    chunks.push(chunk);
                }
                current_idx = next_idx;
            } else {
                current_idx += 1;
            }
        }

        Ok(chunks)
    }

    /// 조(Article) 블록 파싱
    fn parse_article_block(
        &mut self,
        lines: &[&str],
        start_idx: usize,
        base_metadata: &LegalMetadata,
    ) -> (LegalChunk, usize) {
        let mut article_lines = Vec::new();
        let mut current_idx = start_idx;

        // 첫 줄에서 조 정보 추출
        let first_line = lines[start_idx].trim();
        let article_caps = RE_ARTICLE.captures(first_line);

        let (article_num, article_title) = if let Some(caps) = &article_caps {
            let mut num = format!("제{}조", caps.get(1).unwrap().as_str());
            if let Some(sub) = caps.get(2) {
                num.push_str(&format!("의{}", sub.as_str()));
            }
            let title = caps.get(3).map(|m| m.as_str().to_string());

            self.current_state.article = Some(if let Some(ref t) = title {
                format!("{}({})", num, t)
            } else {
                num.clone()
            });

            (Some(num), title)
        } else {
            (None, None)
        };

        // 다음 조가 나올 때까지 수집
        while current_idx < lines.len() {
            let line = lines[current_idx].trim();

            // 다음 조 시작 감지
            if current_idx > start_idx && RE_ARTICLE.is_match(line) {
                break;
            }

            // 편/장/절/관 감지 시 상태 업데이트 후 계속
            if RE_PART.is_match(line) || RE_CHAPTER.is_match(line)
                || RE_SECTION.is_match(line) || RE_SUBSECTION.is_match(line) {
                // 이미 상위 루프에서 처리하므로 여기서는 break
                break;
            }

            article_lines.push(line);
            current_idx += 1;
        }

        let content = article_lines.join("\n").trim().to_string();

        // 메타데이터 생성
        let mut chunk_metadata = base_metadata.clone();
        chunk_metadata.part = self.current_state.part.clone();
        chunk_metadata.chapter = self.current_state.chapter.clone();
        chunk_metadata.section = self.current_state.section.clone();
        chunk_metadata.subsection = self.current_state.subsection.clone();
        chunk_metadata.article_number = article_num;
        chunk_metadata.article_title = article_title;
        chunk_metadata.references = self.extract_references(&content);
        chunk_metadata.line_start = start_idx;
        chunk_metadata.line_end = current_idx.saturating_sub(1);

        let chunk = LegalChunk {
            id: self.generate_chunk_id(&content, &chunk_metadata),
            content: content.clone(),
            metadata: chunk_metadata,
            chunk_type: ChunkType::Article,
            token_count: self.estimate_tokens(&content),
            context_path: self.build_context_path(),
            parent_chunk_id: None,
        };

        (chunk, current_idx)
    }

    /// 큰 조문을 항(Paragraph) 단위로 분할
    pub fn chunk_large_article(&self, chunk: LegalChunk) -> Vec<LegalChunk> {
        if chunk.token_count <= self.max_chunk_tokens {
            return vec![chunk];
        }

        let mut sub_chunks = Vec::new();
        let lines: Vec<&str> = chunk.content.lines().collect();
        let mut current_content = Vec::new();
        let mut current_paragraph: Option<String> = None;

        for line in lines {
            if let Some(caps) = RE_PARAGRAPH.captures(line) {
                // 이전 항 저장
                if !current_content.is_empty() {
                    let content = current_content.join("\n").trim().to_string();
                    if !content.is_empty() {
                        let mut sub_meta = chunk.metadata.clone();
                        sub_meta.paragraph_number = current_paragraph.clone();

                        sub_chunks.push(LegalChunk {
                            id: self.generate_chunk_id(&content, &sub_meta),
                            content,
                            metadata: sub_meta,
                            chunk_type: ChunkType::Paragraph,
                            token_count: self.estimate_tokens(&current_content.join("\n")),
                            context_path: chunk.context_path.clone(),
                            parent_chunk_id: Some(chunk.id.clone()),
                        });
                    }
                    current_content.clear();
                }

                // 원문자를 숫자로 변환
                let circled = caps.get(1).unwrap().as_str();
                current_paragraph = circled.chars().next()
                    .and_then(circled_to_number)
                    .map(|n| n.to_string())
                    .or_else(|| Some(circled.trim_matches(|c| c == '(' || c == ')').to_string()));
            }

            current_content.push(line);
        }

        // 마지막 항 저장
        if !current_content.is_empty() {
            let content = current_content.join("\n").trim().to_string();
            if !content.is_empty() {
                let mut sub_meta = chunk.metadata.clone();
                sub_meta.paragraph_number = current_paragraph;

                sub_chunks.push(LegalChunk {
                    id: self.generate_chunk_id(&content, &sub_meta),
                    content,
                    metadata: sub_meta,
                    chunk_type: ChunkType::Paragraph,
                    token_count: self.estimate_tokens(&current_content.join("\n")),
                    context_path: chunk.context_path.clone(),
                    parent_chunk_id: Some(chunk.id.clone()),
                });
            }
        }

        if sub_chunks.is_empty() {
            vec![chunk]
        } else {
            sub_chunks
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let chunker = KoreanLegalChunker::new();

        // 한글 10자 ≈ 6.67 토큰
        assert!(chunker.estimate_tokens("안녕하세요테스트입니다") < 10);

        // 영문 10자 ≈ 2.5 토큰
        assert!(chunker.estimate_tokens("helloworld") < 5);
    }

    #[test]
    fn test_build_context_path() {
        let mut chunker = KoreanLegalChunker::new();
        chunker.current_state.part = Some("제1편 총칙".to_string());
        chunker.current_state.chapter = Some("제1장 통칙".to_string());
        chunker.current_state.article = Some("제1조(목적)".to_string());

        let path = chunker.build_context_path();
        assert_eq!(path, "제1편 총칙 > 제1장 통칙 > 제1조(목적)");
    }
}
```

### 4.5 내보내기 (`legal/exporter.rs`)

```rust
use crate::legal::types::*;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// WeKnora RAG 시스템용 내보내기
pub struct WeKnoraExporter;

impl WeKnoraExporter {
    /// 임베딩용 데이터 변환
    pub fn export_for_embedding(
        chunks: &[LegalChunk],
        include_context_in_content: bool,
    ) -> Vec<EmbeddingData> {
        chunks.iter()
            .map(|chunk| chunk.to_embedding_format(include_context_in_content))
            .collect()
    }

    /// JSONL 파일로 내보내기
    pub fn export_to_jsonl(
        chunks: &[LegalChunk],
        output_path: &Path,
        include_context_in_content: bool,
    ) -> Result<usize, std::io::Error> {
        let data = Self::export_for_embedding(chunks, include_context_in_content);

        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        for item in &data {
            let json = serde_json::to_string(item)?;
            writeln!(writer, "{}", json)?;
        }

        writer.flush()?;
        Ok(data.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_export_to_jsonl() {
        let chunk = LegalChunk {
            id: "test123".to_string(),
            content: "테스트 내용".to_string(),
            metadata: LegalMetadata {
                law_name: "테스트법".to_string(),
                ..Default::default()
            },
            chunk_type: ChunkType::Article,
            token_count: 10,
            context_path: "제1조".to_string(),
            parent_chunk_id: None,
        };

        let dir = tempdir().unwrap();
        let output_path = dir.path().join("test.jsonl");

        let count = WeKnoraExporter::export_to_jsonl(
            &[chunk],
            &output_path,
            true,
        ).unwrap();

        assert_eq!(count, 1);
        assert!(output_path.exists());
    }
}
```

---

## 5. 테스트 전략

### 5.1 단위 테스트

| 모듈 | 테스트 케이스 |
|------|--------------|
| `record.rs` | `test_extended_ctrl_char_0x16_to_0x1f` |
| `record.rs` | `test_invalid_surrogate_replacement` |
| `patterns.rs` | `test_article_pattern` |
| `patterns.rs` | `test_circled_numbers` |
| `chunker.rs` | `test_estimate_tokens` |
| `chunker.rs` | `test_build_context_path` |
| `exporter.rs` | `test_export_to_jsonl` |

### 5.2 통합 테스트

1. **HWP 텍스트 추출 테스트**
   - 확장 제어 문자 포함 파일로 텍스트 추출
   - Python 출력과 비교

2. **법률 청킹 테스트**
   - 실제 법률 마크다운 파싱
   - 청크 수, 토큰 수 검증

3. **JSONL 출력 테스트**
   - 출력 파일 형식 검증
   - WeKnora API 호환성 확인

---

## 6. 마이그레이션 계획

### Phase 1: 핵심 버그 수정 (1일)
- `record.rs` 수정
- 테스트 추가 및 검증

### Phase 2: 법률 파서 구현 (3일)
- 타입 정의
- 정규식 패턴
- 청커 구현
- 내보내기

### Phase 3: CLI 통합 (1일)
- 명령어 추가
- 사용 문서 작성

### Phase 4: 검증 및 배포 (1일)
- Python 출력과 비교 테스트
- 성능 벤치마크
- 문서화

---

## 7. 참고 자료

### 소스 파일
- Python 파서: `/Users/seunghan/krx_listing/krx_law/legal_chunker.py`
- Python HWP 변환기: `/Users/seunghan/krx_listing/tmp/markdown-media/converters/hwp_converter.py`
- Rust 레코드 파서: `/Users/seunghan/krx_listing/tmp/markdown-media/core/src/hwp/record.rs`

### 외부 문서
- [HWP 5.0 파일 구조](https://www.hancom.com/cs_center/csFaqView.do)
- [Rust regex 크레이트](https://docs.rs/regex/latest/regex/)
- [WeKnora API 문서](https://weknora.com/docs)
