# Phase 4: Rust 코어 강화 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** markdown-media Rust 코어에 별표/별지 파서, HWPX 테이블 quick-xml 리팩토링, 한국어 날짜 파서, 체인 함수를 추가하여 korea-law MCP 통합의 기반을 마련한다.

**Architecture:** 기존 `core/src/legal/` 모듈에 `annex.rs`, `chains.rs`를 추가하고, `core/src/utils/` 모듈을 신규 생성하여 `date_parser.rs`를 넣는다. HWPX 테이블 파서는 기존 `core/src/hwpx/parser.rs`를 quick-xml 이벤트 기반으로 리팩토링한다. 모든 새 코드는 기존 구조체(`TableData`, `CellSpan`, `LegalHierarchy` 등)를 재활용한다.

**Tech Stack:** Rust 2021 edition, chrono 0.4, quick-xml 0.31 (기존), regex 1.10 (기존), serde/serde_json (기존)

---

## File Map

### 신규 생성

| 파일 | 역할 |
|------|------|
| `core/src/legal/annex.rs` | 별표/별지 감지 및 파싱 (AnnexParser, AnnexInfo) |
| `core/src/legal/chains.rs` | 체인 실행 계획 생성 (ChainPlan, ChainStep) |
| `core/src/utils/mod.rs` | utils 모듈 진입점 |
| `core/src/utils/date_parser.rs` | 한국어 자연어 날짜 → NaiveDate 변환 |
| `core/tests/annex_tests.rs` | 별표/별지 단위 테스트 |
| `core/tests/date_parser_tests.rs` | 날짜 파서 단위 테스트 |
| `core/tests/chain_tests.rs` | 체인 함수 단위 테스트 |
| `core/tests/hwpx_table_tests.rs` | HWPX 테이블 quick-xml 테스트 |

### 수정

| 파일 | 변경 내용 |
|------|----------|
| `core/Cargo.toml` | `chrono = "0.4"` 의존성 추가 |
| `core/src/lib.rs` | `pub mod utils;` 추가 |
| `core/src/legal/mod.rs` | `pub mod annex;`, `pub mod chains;` 추가 |
| `core/src/legal/patterns.rs` | RE_ANNEX, RE_ANNEX_FORM, RE_ATTACHMENT 정규식 추가 |
| `core/src/hwpx/parser.rs` | 테이블 파싱을 quick-xml 이벤트 기반으로 리팩토링 |

---

## Task 1: 별표/별지 정규식 추가

**Files:**
- Modify: `core/src/legal/patterns.rs:96` (KOREAN_ITEMS 위에 추가)

- [ ] **Step 1: patterns.rs에 별표/별지 정규식 추가**

`core/src/legal/patterns.rs`의 `lazy_static!` 블록 안, `RE_WHITESPACE` 다음(69행 뒤)에 추가:

```rust
    /// 별표(Annex) 패턴: 별표 1, 별표1의2, [별표 3] 안전관리기준
    pub static ref RE_ANNEX: Regex = Regex::new(
        r"^\[?별표\s*(\d+)(?:의\s*(\d+))?\]?\s*(.*?)$"
    ).unwrap();

    /// 별지(Form) 패턴: 별지 제1호서식, 별지서식1, [별지 제2호의3 서식]
    pub static ref RE_ANNEX_FORM: Regex = Regex::new(
        r"^\[?별지\s*(?:제?\s*)?(\d+)(?:호)?(?:의\s*(\d+))?\s*(?:서식)?\]?\s*(.*?)$"
    ).unwrap();

    /// 첨부(Attachment) 패턴: [첨부1], 첨부 2
    pub static ref RE_ATTACHMENT: Regex = Regex::new(
        r"^\[?첨부\s*(\d+)\]?\s*(.*?)$"
    ).unwrap();
```

- [ ] **Step 2: 패턴 테스트 추가**

같은 파일의 `#[cfg(test)] mod tests` 블록 끝(245행 `}` 직전)에 추가:

```rust
    #[test]
    fn test_re_annex() {
        // 기본: 별표 1
        let caps = RE_ANNEX.captures("별표 1 안전관리기준").unwrap();
        assert_eq!(&caps[1], "1");
        assert!(caps.get(2).is_none() || caps[2].is_empty());
        assert_eq!(caps[3].trim(), "안전관리기준");

        // 가지번호: 별표1의2
        let caps2 = RE_ANNEX.captures("별표1의2 세부기준").unwrap();
        assert_eq!(&caps2[1], "1");
        assert_eq!(caps2.get(2).map(|m| m.as_str()), Some("2"));

        // 대괄호: [별표 3]
        let caps3 = RE_ANNEX.captures("[별표 3] 허가기준").unwrap();
        assert_eq!(&caps3[1], "3");
    }

    #[test]
    fn test_re_annex_form() {
        let caps = RE_ANNEX_FORM.captures("별지 제1호서식 신청서").unwrap();
        assert_eq!(&caps[1], "1");
        assert_eq!(caps[3].trim(), "신청서");

        let caps2 = RE_ANNEX_FORM.captures("별지서식2 보고서").unwrap();
        assert_eq!(&caps2[1], "2");
    }

    #[test]
    fn test_re_attachment() {
        let caps = RE_ATTACHMENT.captures("[첨부1] 관련서류").unwrap();
        assert_eq!(&caps[1], "1");
        assert_eq!(caps[2].trim(), "관련서류");
    }
```

- [ ] **Step 3: 테스트 실행**

Run: `cd /Users/seunghan/markdown-media/core && cargo test --lib legal::patterns::tests -- --nocapture`
Expected: 기존 11개 + 새 3개 = 14개 테스트 PASS

- [ ] **Step 4: 커밋**

```bash
cd /Users/seunghan/markdown-media
git add core/src/legal/patterns.rs
git commit -m "feat(legal): add annex/form/attachment regex patterns"
```

---

## Task 2: AnnexInfo 타입 및 AnnexParser 구현

**Files:**
- Create: `core/src/legal/annex.rs`
- Modify: `core/src/legal/mod.rs`

- [ ] **Step 1: legal/mod.rs에 annex 모듈 등록**

`core/src/legal/mod.rs`에서 `pub mod exporter;` 다음에 추가:

```rust
pub mod annex;

pub use annex::{AnnexParser, AnnexInfo, AnnexType};
```

- [ ] **Step 2: annex.rs 생성 — 타입 정의**

`core/src/legal/annex.rs`:

```rust
//! Annex/Form parser for Korean legal documents
//!
//! 법률 문서의 별표(Annex), 별지(Form), 첨부(Attachment) 감지 및 파싱

use serde::{Deserialize, Serialize};

use crate::hwp::parser::TableData;
use super::patterns::{RE_ANNEX, RE_ANNEX_FORM, RE_ATTACHMENT};

/// 별표/별지 유형
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnexType {
    /// 별표 (numbered appendix table)
    Annex,
    /// 별지/서식 (attached form)
    Form,
    /// 첨부 (attachment)
    Attachment,
}

/// 별표/별지 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnexInfo {
    pub annex_type: AnnexType,
    pub number: u32,
    pub sub_number: Option<u32>,
    pub title: String,
    pub tables: Vec<TableData>,
    pub raw_content: String,
    pub markdown: String,
}

/// 텍스트 내 별표 영역 (시작/끝 라인)
#[derive(Debug, Clone)]
pub struct AnnexRegion {
    pub annex_type: AnnexType,
    pub number: u32,
    pub sub_number: Option<u32>,
    pub title: String,
    pub start_line: usize,
    pub end_line: usize,
}

pub struct AnnexParser;

impl AnnexParser {
    /// 텍스트에서 별표/별지 영역 감지
    pub fn detect_regions(text: &str) -> Vec<AnnexRegion> {
        let lines: Vec<&str> = text.lines().collect();
        let mut regions: Vec<AnnexRegion> = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let detected = Self::detect_line(trimmed);
            if let Some((annex_type, number, sub_number, title)) = detected {
                // 이전 영역의 끝을 이 라인 직전으로 설정
                if let Some(last) = regions.last_mut() {
                    last.end_line = i;
                }

                regions.push(AnnexRegion {
                    annex_type,
                    number,
                    sub_number,
                    title,
                    start_line: i,
                    end_line: lines.len(), // 기본값: 파일 끝
                });
            }
        }

        regions
    }

    /// 단일 라인에서 별표/별지 감지
    fn detect_line(line: &str) -> Option<(AnnexType, u32, Option<u32>, String)> {
        if let Some(caps) = RE_ANNEX.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let sub_number = caps.get(2).and_then(|m| m.as_str().parse().ok());
            let title = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Annex, number, sub_number, title));
        }

        if let Some(caps) = RE_ANNEX_FORM.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let sub_number = caps.get(2).and_then(|m| m.as_str().parse().ok());
            let title = caps.get(3).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Form, number, sub_number, title));
        }

        if let Some(caps) = RE_ATTACHMENT.captures(line) {
            let number: u32 = caps[1].parse().ok()?;
            let title = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            return Some((AnnexType::Attachment, number, None, title));
        }

        None
    }

    /// 감지된 영역에서 내용 추출 → AnnexInfo 생성
    pub fn extract_from_text(text: &str) -> Vec<AnnexInfo> {
        let lines: Vec<&str> = text.lines().collect();
        let regions = Self::detect_regions(text);

        regions
            .iter()
            .map(|region| {
                let content_lines = &lines[region.start_line..region.end_line];
                let raw_content = content_lines.join("\n");
                let markdown = raw_content.clone(); // 텍스트 기반은 이미 Markdown

                AnnexInfo {
                    annex_type: region.annex_type.clone(),
                    number: region.number,
                    sub_number: region.sub_number,
                    title: region.title.clone(),
                    tables: Vec::new(), // 텍스트 기반에서는 테이블 구조 없음
                    raw_content,
                    markdown,
                }
            })
            .collect()
    }

    /// HWP 바이너리에서 별표 추출
    pub fn from_hwp(data: &[u8]) -> Result<Vec<AnnexInfo>, String> {
        use crate::hwp::HwpParser;

        let parser = HwpParser::from_bytes(data).map_err(|e| e.to_string())?;
        let text = parser.extract_text().map_err(|e| e.to_string())?;
        let tables = parser.extract_tables().map_err(|e| e.to_string())?;

        let mut regions = Self::detect_regions(&text);
        if regions.is_empty() {
            // 별표 마커 없으면 전체를 하나의 별표로 취급 (별표 파일 자체인 경우)
            if !tables.is_empty() {
                return Ok(vec![AnnexInfo {
                    annex_type: AnnexType::Annex,
                    number: 1,
                    sub_number: None,
                    title: String::new(),
                    markdown: tables.iter().map(|t| t.to_markdown()).collect::<Vec<_>>().join("\n\n"),
                    raw_content: text,
                    tables,
                }]);
            }
            return Ok(Vec::new());
        }

        // 각 영역에 해당하는 테이블 매칭 (단순화: 순서대로 배분)
        let lines: Vec<&str> = text.lines().collect();
        let mut result = Vec::new();
        for region in &regions {
            let content = lines[region.start_line..region.end_line].join("\n");
            let md_tables: String = tables.iter().map(|t| t.to_markdown()).collect::<Vec<_>>().join("\n\n");

            result.push(AnnexInfo {
                annex_type: region.annex_type.clone(),
                number: region.number,
                sub_number: region.sub_number,
                title: region.title.clone(),
                tables: Vec::new(),
                raw_content: content.clone(),
                markdown: if md_tables.is_empty() { content } else { md_tables },
            });
        }

        Ok(result)
    }

    /// HWPX ZIP에서 별표 추출
    pub fn from_hwpx(data: &[u8]) -> Result<Vec<AnnexInfo>, String> {
        use crate::hwpx::HwpxParser;
        use std::io::Cursor;

        let cursor = Cursor::new(data);
        let mut parser = HwpxParser::from_reader(cursor).map_err(|e| e.to_string())?;
        let doc = parser.parse().map_err(|e| e.to_string())?;

        let full_text = doc.sections.join("\n");
        let regions = Self::detect_regions(&full_text);

        if regions.is_empty() && !doc.tables.is_empty() {
            return Ok(vec![AnnexInfo {
                annex_type: AnnexType::Annex,
                number: 1,
                sub_number: None,
                title: String::new(),
                markdown: doc.tables.iter().map(|t| {
                    let td = TableData {
                        rows: t.rows,
                        cols: t.cols,
                        cells: t.cells.clone(),
                        cell_spans: Vec::new(),
                    };
                    td.to_markdown()
                }).collect::<Vec<_>>().join("\n\n"),
                raw_content: full_text,
                tables: Vec::new(),
            }]);
        }

        let lines: Vec<&str> = full_text.lines().collect();
        Ok(regions.iter().map(|region| {
            let content = lines[region.start_line..region.end_line].join("\n");
            AnnexInfo {
                annex_type: region.annex_type.clone(),
                number: region.number,
                sub_number: region.sub_number,
                title: region.title.clone(),
                tables: Vec::new(),
                raw_content: content.clone(),
                markdown: content,
            }
        }).collect())
    }
}
```

- [ ] **Step 3: TableData에 Serialize/Deserialize derive 추가**

`core/src/hwp/parser.rs:414`의 TableData와 CellSpan에 serde derive 추가:

```rust
/// 표 데이터
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableData {
```

그리고 CellSpan 구조체를 찾아서(같은 파일) 동일하게 추가:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CellSpan {
```

- [ ] **Step 4: 컴파일 확인**

Run: `cd /Users/seunghan/markdown-media/core && cargo check`
Expected: no errors

- [ ] **Step 5: 커밋**

```bash
cd /Users/seunghan/markdown-media
git add core/src/legal/annex.rs core/src/legal/mod.rs core/src/hwp/parser.rs
git commit -m "feat(legal): add AnnexParser for annex/form/attachment extraction"
```

---

## Task 3: 별표 파서 테스트

**Files:**
- Create: `core/tests/annex_tests.rs`

- [ ] **Step 1: 테스트 파일 생성**

`core/tests/annex_tests.rs`:

```rust
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
```

- [ ] **Step 2: 테스트 실행**

Run: `cd /Users/seunghan/markdown-media/core && cargo test --test annex_tests -- --nocapture`
Expected: 6개 테스트 PASS

- [ ] **Step 3: 커밋**

```bash
cd /Users/seunghan/markdown-media
git add core/tests/annex_tests.rs
git commit -m "test(legal): add annex parser unit tests"
```

---

## Task 4: chrono 의존성 추가 & utils 모듈 설정

**Files:**
- Modify: `core/Cargo.toml`
- Modify: `core/src/lib.rs`
- Create: `core/src/utils/mod.rs`

- [ ] **Step 1: Cargo.toml에 chrono 추가**

`core/Cargo.toml`의 `[dependencies]` 섹션, `sha2 = "0.10"` 다음에 추가:

```toml
chrono = "0.4"
```

- [ ] **Step 2: utils 모듈 생성**

`core/src/utils/mod.rs`:

```rust
//! Utility modules for MDM Core
//!
//! 한국어 날짜 파서 등 공통 유틸리티

pub mod date_parser;

pub use date_parser::KoreanDateParser;
```

- [ ] **Step 3: lib.rs에 utils 모듈 등록**

`core/src/lib.rs`에서 `pub mod legal;` 다음에 추가:

```rust
pub mod utils;
```

- [ ] **Step 4: 빌드 확인**

Run: `cd /Users/seunghan/markdown-media/core && cargo check`
Expected: date_parser 모듈 파일 없어서 에러 (다음 Task에서 생성)

- [ ] **Step 5: 커밋 (Cargo.toml + lib.rs + utils/mod.rs)**

```bash
cd /Users/seunghan/markdown-media
git add core/Cargo.toml core/src/lib.rs core/src/utils/mod.rs
git commit -m "chore: add chrono dependency and utils module scaffold"
```

---

## Task 5: 한국어 날짜 파서 구현

**Files:**
- Create: `core/src/utils/date_parser.rs`

- [ ] **Step 1: date_parser.rs 생성**

`core/src/utils/date_parser.rs`:

```rust
//! Korean Natural Language Date Parser
//!
//! 한국어 날짜 표현을 NaiveDate로 변환
//! "최근 3개월", "작년", "다음주 화요일", "시행일로부터 30일" 등

use chrono::{Datelike, Local, NaiveDate, Weekday};
use regex::Regex;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

/// 파싱 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateResult {
    /// YYYYMMDD 형식 날짜
    pub date: String,
    /// 범위인 경우 종료일 (YYYYMMDD)
    pub end_date: Option<String>,
    /// 날짜 형식 유형
    pub format: DateFormat,
    /// 신뢰도 (0.0 ~ 1.0)
    pub confidence: f64,
}

/// 날짜 형식 유형
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateFormat {
    Absolute,
    Relative,
    Duration,
    Legal,
    Weekday,
}

lazy_static! {
    // 절대 날짜: 2024년 3월 1일 또는 2024.3.1
    static ref RE_ABS_KOREAN: Regex = Regex::new(
        r"(\d{4})년\s*(\d{1,2})월\s*(\d{1,2})일"
    ).unwrap();
    static ref RE_ABS_DOT: Regex = Regex::new(
        r"(\d{4})\.(\d{1,2})\.(\d{1,2})"
    ).unwrap();

    // N일/주/월/년 전/후
    static ref RE_OFFSET: Regex = Regex::new(
        r"(\d+)\s*(일|주|달|개월|년|시간)\s*(전|후|이내|뒤|내)"
    ).unwrap();

    // 요일: 다음주 화요일
    static ref RE_WEEKDAY: Regex = Regex::new(
        r"(이번|다음|지난|저번|다다음)주\s*(월요일|화요일|수요일|목요일|금요일|토요일|일요일)"
    ).unwrap();

    // 최근 N기간
    static ref RE_RECENT: Regex = Regex::new(
        r"최근\s*(\d+)?\s*(일|주|달|개월|년|분기)"
    ).unwrap();

    // 분기/반기: 2024년 1분기, 올해 상반기
    static ref RE_QUARTER: Regex = Regex::new(
        r"(?:(\d{4})년\s*)?(상반기|하반기|([1-4])분기)"
    ).unwrap();

    // 법률 기간: 시행일로부터 30일 이내
    static ref RE_LEGAL_PERIOD: Regex = Regex::new(
        r"(시행일|공포일|개정일|기준일)(?:로부터)?\s*(\d+)\s*(일|개월|년)\s*(이내|이후|이상)?"
    ).unwrap();
}

/// 한국어 날짜 파서
pub struct KoreanDateParser {
    reference: NaiveDate,
}

impl KoreanDateParser {
    pub fn new(reference: NaiveDate) -> Self {
        Self { reference }
    }

    pub fn today() -> Self {
        Self {
            reference: Local::now().date_naive(),
        }
    }

    /// 한국어 텍스트 → DateResult
    pub fn parse(&self, text: &str) -> Option<DateResult> {
        let text = text.trim();

        // 1. 절대 날짜
        if let Some(r) = self.parse_absolute(text) { return Some(r); }
        // 2. 상대 표현 (오늘, 어제, 내일 등)
        if let Some(r) = self.parse_named_relative(text) { return Some(r); }
        // 3. N일/주/월 전후
        if let Some(r) = self.parse_offset(text) { return Some(r); }
        // 4. 요일
        if let Some(r) = self.parse_weekday(text) { return Some(r); }
        // 5. 최근 N기간
        if let Some(r) = self.parse_recent(text) { return Some(r); }
        // 6. 분기/반기
        if let Some(r) = self.parse_quarter(text) { return Some(r); }
        // 7. 법률 기간
        if let Some(r) = self.parse_legal_period(text) { return Some(r); }

        None
    }

    fn parse_absolute(&self, text: &str) -> Option<DateResult> {
        // 2024년 3월 1일
        if let Some(caps) = RE_ABS_KOREAN.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            let date = NaiveDate::from_ymd_opt(y, m, d)?;
            return Some(DateResult {
                date: format_yyyymmdd(&date),
                end_date: None,
                format: DateFormat::Absolute,
                confidence: 0.95,
            });
        }
        // 2024.3.1
        if let Some(caps) = RE_ABS_DOT.captures(text) {
            let y: i32 = caps[1].parse().ok()?;
            let m: u32 = caps[2].parse().ok()?;
            let d: u32 = caps[3].parse().ok()?;
            let date = NaiveDate::from_ymd_opt(y, m, d)?;
            return Some(DateResult {
                date: format_yyyymmdd(&date),
                end_date: None,
                format: DateFormat::Absolute,
                confidence: 0.90,
            });
        }
        None
    }

    fn parse_named_relative(&self, text: &str) -> Option<DateResult> {
        let (offset, conf) = match text {
            "오늘" => (0i64, 0.95),
            "내일" => (1, 0.95),
            "모레" => (2, 0.95),
            "어제" => (-1, 0.95),
            "그제" | "그저께" => (-2, 0.95),
            "올해" => {
                let start = NaiveDate::from_ymd_opt(self.reference.year(), 1, 1)?;
                let end = NaiveDate::from_ymd_opt(self.reference.year(), 12, 31)?;
                return Some(DateResult {
                    date: format_yyyymmdd(&start),
                    end_date: Some(format_yyyymmdd(&end)),
                    format: DateFormat::Duration,
                    confidence: 0.95,
                });
            }
            "작년" => {
                let y = self.reference.year() - 1;
                let start = NaiveDate::from_ymd_opt(y, 1, 1)?;
                let end = NaiveDate::from_ymd_opt(y, 12, 31)?;
                return Some(DateResult {
                    date: format_yyyymmdd(&start),
                    end_date: Some(format_yyyymmdd(&end)),
                    format: DateFormat::Duration,
                    confidence: 0.95,
                });
            }
            "이번달" => {
                let start = NaiveDate::from_ymd_opt(self.reference.year(), self.reference.month(), 1)?;
                return Some(DateResult {
                    date: format_yyyymmdd(&start),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                });
            }
            "다음달" => {
                let d = add_months(self.reference, 1);
                let start = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)?;
                return Some(DateResult {
                    date: format_yyyymmdd(&start),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                });
            }
            "지난달" => {
                let d = add_months(self.reference, -1);
                let start = NaiveDate::from_ymd_opt(d.year(), d.month(), 1)?;
                return Some(DateResult {
                    date: format_yyyymmdd(&start),
                    end_date: None,
                    format: DateFormat::Duration,
                    confidence: 0.90,
                });
            }
            _ => return None,
        };

        let date = self.reference + chrono::Duration::days(offset);
        Some(DateResult {
            date: format_yyyymmdd(&date),
            end_date: None,
            format: DateFormat::Relative,
            confidence: conf,
        })
    }

    fn parse_offset(&self, text: &str) -> Option<DateResult> {
        let caps = RE_OFFSET.captures(text)?;
        let n: i64 = caps[1].parse().ok()?;
        let unit = &caps[2];
        let direction = &caps[3];

        let multiplier: i64 = match direction {
            "전" | "이내" | "내" => -1,
            "후" | "뒤" => 1,
            _ => return None,
        };

        let date = match unit {
            "일" => self.reference + chrono::Duration::days(n * multiplier),
            "주" => self.reference + chrono::Duration::weeks(n * multiplier),
            "달" | "개월" => add_months(self.reference, (n * multiplier) as i32),
            "년" => {
                let y = self.reference.year() + (n * multiplier) as i32;
                NaiveDate::from_ymd_opt(y, self.reference.month(), self.reference.day())?
            }
            _ => return None,
        };

        Some(DateResult {
            date: format_yyyymmdd(&date),
            end_date: None,
            format: DateFormat::Relative,
            confidence: 0.90,
        })
    }

    fn parse_weekday(&self, text: &str) -> Option<DateResult> {
        let caps = RE_WEEKDAY.captures(text)?;
        let week_ref = &caps[1];
        let day_name = &caps[2];

        let target_weekday = match day_name {
            "월요일" => Weekday::Mon,
            "화요일" => Weekday::Tue,
            "수요일" => Weekday::Wed,
            "목요일" => Weekday::Thu,
            "금요일" => Weekday::Fri,
            "토요일" => Weekday::Sat,
            "일요일" => Weekday::Sun,
            _ => return None,
        };

        let week_offset: i64 = match week_ref {
            "이번" => 0,
            "다음" => 1,
            "지난" | "저번" => -1,
            "다다음" => 2,
            _ => return None,
        };

        let current_weekday = self.reference.weekday().num_days_from_monday() as i64;
        let target_day = target_weekday.num_days_from_monday() as i64;
        let days_diff = target_day - current_weekday + week_offset * 7;
        let date = self.reference + chrono::Duration::days(days_diff);

        Some(DateResult {
            date: format_yyyymmdd(&date),
            end_date: None,
            format: DateFormat::Weekday,
            confidence: 0.92,
        })
    }

    fn parse_recent(&self, text: &str) -> Option<DateResult> {
        let caps = RE_RECENT.captures(text)?;
        let n: i64 = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(1);
        let unit = &caps[2];

        let start = match unit {
            "일" => self.reference - chrono::Duration::days(n),
            "주" => self.reference - chrono::Duration::weeks(n),
            "달" | "개월" => add_months(self.reference, -(n as i32)),
            "년" => {
                let y = self.reference.year() - n as i32;
                NaiveDate::from_ymd_opt(y, self.reference.month(), self.reference.day())?
            }
            "분기" => add_months(self.reference, -(n as i32 * 3)),
            _ => return None,
        };

        Some(DateResult {
            date: format_yyyymmdd(&start),
            end_date: Some(format_yyyymmdd(&self.reference)),
            format: DateFormat::Duration,
            confidence: 0.85,
        })
    }

    fn parse_quarter(&self, text: &str) -> Option<DateResult> {
        let caps = RE_QUARTER.captures(text)?;
        let year = caps.get(1)
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(self.reference.year());

        let (start_month, end_month) = if let Some(q) = caps.get(3) {
            match q.as_str() {
                "1" => (1, 3),
                "2" => (4, 6),
                "3" => (7, 9),
                "4" => (10, 12),
                _ => return None,
            }
        } else {
            match &caps[2] {
                "상반기" => (1, 6),
                "하반기" => (7, 12),
                _ => return None,
            }
        };

        let start = NaiveDate::from_ymd_opt(year, start_month, 1)?;
        let end_day = last_day_of_month(year, end_month);
        let end = NaiveDate::from_ymd_opt(year, end_month, end_day)?;

        Some(DateResult {
            date: format_yyyymmdd(&start),
            end_date: Some(format_yyyymmdd(&end)),
            format: DateFormat::Duration,
            confidence: 0.92,
        })
    }

    fn parse_legal_period(&self, text: &str) -> Option<DateResult> {
        let caps = RE_LEGAL_PERIOD.captures(text)?;
        let _base_marker = &caps[1]; // 시행일, 공포일 등
        let n: i64 = caps[2].parse().ok()?;
        let unit = &caps[3];

        // 법률 기간은 기준일(reference)로부터 계산
        let date = match unit {
            "일" => self.reference + chrono::Duration::days(n),
            "개월" => add_months(self.reference, n as i32),
            "년" => {
                let y = self.reference.year() + n as i32;
                NaiveDate::from_ymd_opt(y, self.reference.month(), self.reference.day())?
            }
            _ => return None,
        };

        Some(DateResult {
            date: format_yyyymmdd(&date),
            end_date: None,
            format: DateFormat::Legal,
            confidence: 0.88,
        })
    }
}

/// NaiveDate → "YYYYMMDD" 문자열
pub fn format_yyyymmdd(date: &NaiveDate) -> String {
    date.format("%Y%m%d").to_string()
}

/// 월 단위 덧셈 (오버플로 처리)
fn add_months(date: NaiveDate, months: i32) -> NaiveDate {
    let total_months = date.year() * 12 + date.month() as i32 - 1 + months;
    let y = total_months / 12;
    let m = (total_months % 12 + 1) as u32;
    let d = date.day().min(last_day_of_month(y, m));
    NaiveDate::from_ymd_opt(y, m, d).unwrap_or(date)
}

/// 해당 월의 마지막 날
fn last_day_of_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(year, month + 1, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
        .pred_opt()
        .unwrap()
        .day()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser() -> KoreanDateParser {
        // 고정 기준일: 2026-04-02 (수요일)
        KoreanDateParser::new(NaiveDate::from_ymd_opt(2026, 4, 2).unwrap())
    }

    #[test]
    fn test_absolute_korean() {
        let r = parser().parse("2024년 3월 1일").unwrap();
        assert_eq!(r.date, "20240301");
        assert_eq!(r.format, DateFormat::Absolute);
    }

    #[test]
    fn test_absolute_dot() {
        let r = parser().parse("2024.12.31").unwrap();
        assert_eq!(r.date, "20241231");
    }

    #[test]
    fn test_relative_today() {
        let r = parser().parse("오늘").unwrap();
        assert_eq!(r.date, "20260402");
    }

    #[test]
    fn test_relative_yesterday() {
        let r = parser().parse("어제").unwrap();
        assert_eq!(r.date, "20260401");
    }

    #[test]
    fn test_relative_tomorrow() {
        let r = parser().parse("내일").unwrap();
        assert_eq!(r.date, "20260403");
    }

    #[test]
    fn test_offset_days() {
        let r = parser().parse("3일 전").unwrap();
        assert_eq!(r.date, "20260330");

        let r2 = parser().parse("5일 후").unwrap();
        assert_eq!(r2.date, "20260407");
    }

    #[test]
    fn test_offset_months() {
        let r = parser().parse("3개월 전").unwrap();
        assert_eq!(r.date, "20260102");

        let r2 = parser().parse("6개월 후").unwrap();
        assert_eq!(r2.date, "20261002");
    }

    #[test]
    fn test_weekday() {
        // 2026-04-02 = 목요일
        let r = parser().parse("이번주 월요일").unwrap();
        assert_eq!(r.date, "20260330"); // 이번주 월요일 = 3/30

        let r2 = parser().parse("다음주 화요일").unwrap();
        assert_eq!(r2.date, "20260407");
    }

    #[test]
    fn test_recent() {
        let r = parser().parse("최근 3개월").unwrap();
        assert_eq!(r.date, "20260102");
        assert_eq!(r.end_date, Some("20260402".to_string()));
    }

    #[test]
    fn test_quarter() {
        let r = parser().parse("2024년 1분기").unwrap();
        assert_eq!(r.date, "20240101");
        assert_eq!(r.end_date, Some("20240331".to_string()));
    }

    #[test]
    fn test_half_year() {
        let r = parser().parse("올해 상반기").unwrap();
        assert_eq!(r.date, "20260101");
        assert_eq!(r.end_date, Some("20260630".to_string()));
    }

    #[test]
    fn test_last_year() {
        let r = parser().parse("작년").unwrap();
        assert_eq!(r.date, "20250101");
        assert_eq!(r.end_date, Some("20251231".to_string()));
    }

    #[test]
    fn test_legal_period() {
        let r = parser().parse("시행일로부터 30일").unwrap();
        assert_eq!(r.date, "20260502");
        assert_eq!(r.format, DateFormat::Legal);
    }

    #[test]
    fn test_none_for_garbage() {
        assert!(parser().parse("아무말대잔치").is_none());
        assert!(parser().parse("").is_none());
    }
}
```

- [ ] **Step 2: 컴파일 및 테스트**

Run: `cd /Users/seunghan/markdown-media/core && cargo test utils::date_parser::tests -- --nocapture`
Expected: 14개 테스트 PASS

- [ ] **Step 3: 커밋**

```bash
cd /Users/seunghan/markdown-media
git add core/src/utils/date_parser.rs
git commit -m "feat(utils): add Korean natural language date parser"
```

---

## Task 6: 체인 함수 정의

**Files:**
- Create: `core/src/legal/chains.rs`
- Modify: `core/src/legal/mod.rs`

- [ ] **Step 1: legal/mod.rs에 chains 모듈 등록**

`core/src/legal/mod.rs`에서 `pub mod annex;` 다음에 추가:

```rust
pub mod chains;

pub use chains::{ChainPlan, ChainStep, ChainType};
```

- [ ] **Step 2: chains.rs 생성**

`core/src/legal/chains.rs`:

```rust
//! Chain tool definitions for multi-step legal research
//!
//! 여러 MCP 도구를 조합하는 체인 실행 계획 생성
//! 실제 API 호출은 Node.js(korea-law MCP) 레이어에서 수행

use serde::{Deserialize, Serialize};

/// 체인 유형
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainType {
    /// 포괄적 법률 조사 (법령 + 조문 + 판례 + 해석례)
    FullResearch,
    /// 행정 처분 법적 근거 추적
    ActionBasis,
    /// 개정 전후 비교
    CompareOldNew,
    /// 조문 + 해석례 함께 검색
    SearchWithInterpretation,
    /// 별표/별지 추출
    ExtractAnnexes,
    /// 법률-시행령-시행규칙 3단 위임 구조 비교
    CompareDelegation,
    /// 유사 판례 찾기
    FindSimilarPrecedents,
    /// 전문기관 결정례 조사 (조세심판원, 공정위 등)
    ResearchSpecialized,
}

impl ChainType {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "full_research" | "FullResearch" => Ok(Self::FullResearch),
            "action_basis" | "ActionBasis" => Ok(Self::ActionBasis),
            "compare_old_new" | "CompareOldNew" => Ok(Self::CompareOldNew),
            "search_with_interpretation" | "SearchWithInterpretation" => Ok(Self::SearchWithInterpretation),
            "extract_annexes" | "ExtractAnnexes" => Ok(Self::ExtractAnnexes),
            "compare_delegation" | "CompareDelegation" => Ok(Self::CompareDelegation),
            "find_similar_precedents" | "FindSimilarPrecedents" => Ok(Self::FindSimilarPrecedents),
            "research_specialized" | "ResearchSpecialized" => Ok(Self::ResearchSpecialized),
            _ => Err(format!("Unknown chain type: {}", s)),
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::FullResearch => "포괄적 법률 조사 (법령 + 조문 + 판례 + 해석례 일괄)",
            Self::ActionBasis => "행정 처분의 법적 근거 추적 (법률 → 해석례 → 판례 → 행정심판)",
            Self::CompareOldNew => "법령 개정 전후 비교 (현행 vs 이전 버전)",
            Self::SearchWithInterpretation => "특정 조문과 관련 해석례 함께 검색",
            Self::ExtractAnnexes => "법령 별표/별지를 Markdown 테이블로 변환",
            Self::CompareDelegation => "법률-시행령-시행규칙 3단 위임 구조 비교",
            Self::FindSimilarPrecedents => "특정 사건과 유사한 판례 검색",
            Self::ResearchSpecialized => "전문기관 결정례 일괄 조사 (조세심판원, 공정위, 노동위 등)",
        }
    }
}

/// 체인 실행 스텝
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    /// MCP 도구명 (예: "search_law", "get_law_text")
    pub tool_name: String,
    /// 파라미터 (JSON)
    pub params: serde_json::Value,
    /// 선행 스텝 인덱스 (이 스텝들이 완료되어야 실행)
    pub depends_on: Vec<usize>,
    /// 병렬 실행 그룹 (같은 그룹은 동시 실행 가능)
    pub parallel_group: Option<u32>,
}

/// 체인 실행 계획
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainPlan {
    pub chain_type: ChainType,
    pub description: String,
    pub steps: Vec<ChainStep>,
}

impl ChainPlan {
    /// 쿼리로부터 체인 실행 계획 생성
    pub fn from_query(chain_type: ChainType, query: &str) -> Self {
        let q = serde_json::json!({ "query": query });

        let (description, steps) = match chain_type {
            ChainType::FullResearch => (
                chain_type.description().to_string(),
                vec![
                    ChainStep {
                        tool_name: "search_law_names".into(),
                        params: q.clone(),
                        depends_on: vec![],
                        parallel_group: Some(0),
                    },
                    ChainStep {
                        tool_name: "get_law_text".into(),
                        params: serde_json::json!({ "from_step": 0 }),
                        depends_on: vec![0],
                        parallel_group: Some(1),
                    },
                    ChainStep {
                        tool_name: "search_precedents".into(),
                        params: q.clone(),
                        depends_on: vec![0],
                        parallel_group: Some(1),
                    },
                    ChainStep {
                        tool_name: "search_legal_interpretations".into(),
                        params: q.clone(),
                        depends_on: vec![0],
                        parallel_group: Some(1),
                    },
                ],
            ),

            ChainType::ActionBasis => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_law_names".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0}), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "search_legal_interpretations".into(), params: q.clone(), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "search_admin_appeals".into(), params: q.clone(), depends_on: vec![0], parallel_group: Some(1) },
                ],
            ),

            ChainType::CompareOldNew => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_law_names".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0, "version": "current"}), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0, "version": "previous"}), depends_on: vec![0], parallel_group: Some(1) },
                ],
            ),

            ChainType::SearchWithInterpretation => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_law_names".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0}), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "search_legal_interpretations".into(), params: q.clone(), depends_on: vec![0], parallel_group: Some(1) },
                ],
            ),

            ChainType::ExtractAnnexes => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_law_names".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "get_annex_urls".into(), params: serde_json::json!({"from_step": 0}), depends_on: vec![0], parallel_group: None },
                    ChainStep { tool_name: "download_hwpx".into(), params: serde_json::json!({"from_step": 1}), depends_on: vec![1], parallel_group: None },
                    ChainStep { tool_name: "parse_annex".into(), params: serde_json::json!({"from_step": 2}), depends_on: vec![2], parallel_group: None },
                ],
            ),

            ChainType::CompareDelegation => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_law_names".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0, "level": "법률"}), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0, "level": "시행령"}), depends_on: vec![0], parallel_group: Some(1) },
                    ChainStep { tool_name: "get_law_text".into(), params: serde_json::json!({"from_step": 0, "level": "시행규칙"}), depends_on: vec![0], parallel_group: Some(1) },
                ],
            ),

            ChainType::FindSimilarPrecedents => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_precedents".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "search_precedents_by_title".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                ],
            ),

            ChainType::ResearchSpecialized => (
                chain_type.description().to_string(),
                vec![
                    ChainStep { tool_name: "search_tax_tribunal".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "search_constitutional_decisions".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "search_ftc_decisions".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                    ChainStep { tool_name: "search_admin_appeals".into(), params: q.clone(), depends_on: vec![], parallel_group: Some(0) },
                ],
            ),
        };

        ChainPlan { chain_type, description, steps }
    }

    /// 실행 가능한 스텝 그룹 반환 (의존성이 충족된 스텝들)
    pub fn executable_groups(&self) -> Vec<Vec<usize>> {
        let mut groups: std::collections::BTreeMap<u32, Vec<usize>> = std::collections::BTreeMap::new();
        for (i, step) in self.steps.iter().enumerate() {
            let group = step.parallel_group.unwrap_or(i as u32 + 100);
            groups.entry(group).or_default().push(i);
        }
        groups.into_values().collect()
    }

    /// 스텝 결과를 통합 Markdown으로 취합
    pub fn aggregate_results(chain_type: &ChainType, results: &[String]) -> String {
        let mut md = String::new();
        md.push_str(&format!("# {}\n\n", chain_type.description()));

        for (i, result) in results.iter().enumerate() {
            if !result.is_empty() {
                md.push_str(&format!("## 단계 {} 결과\n\n{}\n\n---\n\n", i + 1, result));
            }
        }

        md
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_research_plan() {
        let plan = ChainPlan::from_query(ChainType::FullResearch, "음주운전 처벌");
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[0].tool_name, "search_law_names");
        assert!(plan.steps[0].depends_on.is_empty());
        assert_eq!(plan.steps[1].depends_on, vec![0]);
        assert_eq!(plan.steps[2].depends_on, vec![0]);
        assert_eq!(plan.steps[1].parallel_group, Some(1));
        assert_eq!(plan.steps[2].parallel_group, Some(1));
    }

    #[test]
    fn test_compare_delegation_plan() {
        let plan = ChainPlan::from_query(ChainType::CompareDelegation, "산업안전보건법");
        assert_eq!(plan.steps.len(), 4);
        // 후반 3개 모두 parallel_group 1
        assert_eq!(plan.steps[1].parallel_group, Some(1));
        assert_eq!(plan.steps[2].parallel_group, Some(1));
        assert_eq!(plan.steps[3].parallel_group, Some(1));
    }

    #[test]
    fn test_research_specialized_all_parallel() {
        let plan = ChainPlan::from_query(ChainType::ResearchSpecialized, "부동산 거래");
        assert_eq!(plan.steps.len(), 4);
        for step in &plan.steps {
            assert_eq!(step.parallel_group, Some(0));
            assert!(step.depends_on.is_empty());
        }
    }

    #[test]
    fn test_extract_annexes_sequential() {
        let plan = ChainPlan::from_query(ChainType::ExtractAnnexes, "화학물질관리법 별표");
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[1].depends_on, vec![0]);
        assert_eq!(plan.steps[2].depends_on, vec![1]);
        assert_eq!(plan.steps[3].depends_on, vec![2]);
    }

    #[test]
    fn test_chain_type_from_str() {
        assert_eq!(ChainType::from_str("full_research").unwrap(), ChainType::FullResearch);
        assert_eq!(ChainType::from_str("FullResearch").unwrap(), ChainType::FullResearch);
        assert!(ChainType::from_str("invalid").is_err());
    }

    #[test]
    fn test_executable_groups() {
        let plan = ChainPlan::from_query(ChainType::FullResearch, "test");
        let groups = plan.executable_groups();
        assert_eq!(groups.len(), 2); // group 0 (search_law) + group 1 (get_law, precedents, interpretations)
    }

    #[test]
    fn test_aggregate_results() {
        let results = vec!["법령 검색 결과".to_string(), "조문 내용".to_string(), "판례 목록".to_string()];
        let md = ChainPlan::aggregate_results(&ChainType::FullResearch, &results);
        assert!(md.contains("# 포괄적 법률 조사"));
        assert!(md.contains("법령 검색 결과"));
        assert!(md.contains("판례 목록"));
    }
}
```

- [ ] **Step 3: 전체 빌드 및 테스트**

Run: `cd /Users/seunghan/markdown-media/core && cargo test -- --nocapture 2>&1 | tail -20`
Expected: 모든 테스트 PASS (기존 + 신규)

- [ ] **Step 4: 커밋**

```bash
cd /Users/seunghan/markdown-media
git add core/src/legal/chains.rs core/src/legal/mod.rs
git commit -m "feat(legal): add chain tool plan definitions for 8 chain types"
```

---

## Task 7: 전체 통합 빌드 검증

**Files:** (수정 없음 — 검증만)

- [ ] **Step 1: 전체 cargo check**

Run: `cd /Users/seunghan/markdown-media/core && cargo check 2>&1`
Expected: no errors, no warnings (또는 기존 warning만)

- [ ] **Step 2: 전체 테스트 실행**

Run: `cd /Users/seunghan/markdown-media/core && cargo test 2>&1 | tail -30`
Expected: 모든 테스트 PASS

- [ ] **Step 3: 신규 모듈 public API 확인**

Run: `cd /Users/seunghan/markdown-media/core && cargo doc --no-deps 2>&1 | tail -5`
Expected: Documentation generated successfully

- [ ] **Step 4: 최종 커밋 (있다면)**

변경사항 있으면 커밋. 없으면 스킵.

---

## Summary

| Task | 내용 | 파일 수 | 테스트 수 |
|------|------|---------|----------|
| 1 | 별표/별지 정규식 | 1 수정 | 3 |
| 2 | AnnexParser 구현 | 2 생성, 2 수정 | - |
| 3 | Annex 테스트 | 1 생성 | 6 |
| 4 | chrono + utils 모듈 | 3 수정/생성 | - |
| 5 | 날짜 파서 구현 | 1 생성 | 14 |
| 6 | 체인 함수 정의 | 1 생성, 1 수정 | 7 |
| 7 | 통합 검증 | - | 전체 |
| **합계** | | **8 생성, 5 수정** | **30+** |
