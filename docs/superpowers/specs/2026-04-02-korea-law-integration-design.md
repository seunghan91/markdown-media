# MDM korea-law 통합 설계서

> **Date**: 2026-04-02
> **Author**: seunghan91
> **Status**: Draft
> **Scope**: markdown-media Rust 코어 강화 → napi-rs npm 배포 → korea-law MCP 통합

---

## 1. 배경 및 목적

### 문제
- `korea-law` MCP 서버(자체 개발, 45+ 도구)가 chrisryugj/korean-law-mcp(87개 도구) 대비 부족한 영역:
  - **별표/별지** HWP/HWPX 파싱 및 Markdown 테이블 변환
  - **체인 도구** (원스톱 법률 조사)
  - **자연어 날짜 파서** (한국어 → YYYYMMDD)
  - **위원회/조약 등 데이터 소스** 확장
- `markdown-media` Rust 코어에 HWP/HWPX 파서가 이미 있지만, 별표/별지 미지원, korea-law와 미연동

### 목표
1. `markdown-media` Rust 코어에 **별표/별지 파서 + 자연어 날짜 파서 + 체인 함수** 추가
2. **napi-rs**로 Node.js 네이티브 애드온 빌드 → `@mdm/core` npm 배포
3. `korea-law` MCP 서버에서 `@mdm/core` 임포트하여 87개+ 도구 달성
4. 추후 **WASM**으로 브라우저/범용 배포

### 아키텍처 개요

```
┌─────────────────────────────────────────────────────────────┐
│  markdown-media (Rust workspace)                            │
│                                                              │
│  core/                    순수 Rust 라이브러리 (비즈니스 로직)  │
│  ├── src/hwp/             HWP 바이너리 파서                   │
│  ├── src/hwpx/            HWPX XML 파서                      │
│  ├── src/legal/           법률 문서 처리                      │
│  │   ├── patterns.rs      별표/별지 정규식 (NEW)               │
│  │   ├── annex.rs         별표/별지 파서 (NEW)                │
│  │   ├── chains.rs        체인 함수 (NEW)                    │
│  │   └── chunker.rs       기존 법률 청커                      │
│  ├── src/utils/                                              │
│  │   └── date_parser.rs   한국어 날짜 파서 (NEW)              │
│  └── src/pdf/             PDF 파서                           │
│                                                              │
│  packages/                                                   │
│  ├── core-native/         napi-rs Node.js 래퍼 (Phase 2)     │
│  └── core-wasm/           wasm-bindgen 래퍼 (Phase 4)        │
└──────────────┬──────────────────────────────────────────────┘
               │  npm: @mdm/core
               ▼
┌─────────────────────────────────────────────────────────────┐
│  korea-law MCP (Node.js)                                     │
│                                                              │
│  import { parseHwp, parseHwpx, parseAnnex,                  │
│           parseKoreanDate, chainFullResearch } from '@mdm/core'│
│                                                              │
│  src/mcp/server.ts        MCP 도구 등록 (87+)                │
│  src/mcp/chain-tools.ts   체인 도구 8개 (NEW)                │
│  src/api/extended-api.ts  위원회/조약 API (NEW)               │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Phase 4: Rust 코어 강화

### 4.1 별표/별지 패턴 감지

**파일**: `core/src/legal/patterns.rs`

```rust
// 추가할 정규식
pub static RE_ANNEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^별표\s*(\d+)(?:의(\d+))?\s*(.*)$").unwrap()
});

pub static RE_ANNEX_FORM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^별지\s*(?:제?\s*)?(\d+)(?:호\s*)?(?:서식)?\s*(.*)$").unwrap()
});

pub static RE_ATTACHMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\[?첨부\s*(\d+)\]?\s*(.*)$").unwrap()
});
```

### 4.2 별표/별지 파서

**새 파일**: `core/src/legal/annex.rs`

```rust
pub enum AnnexType {
    Annex,       // 별표
    Form,        // 별지/서식
    Attachment,  // 첨부
}

pub struct AnnexInfo {
    pub annex_type: AnnexType,
    pub number: u32,
    pub sub_number: Option<u32>,  // 별표1의2 → sub_number=2
    pub title: String,
    pub tables: Vec<TableData>,   // 재활용: hwp/record.rs의 TableData
    pub raw_content: String,
    pub markdown: String,         // 변환된 Markdown
}

pub struct AnnexParser;

impl AnnexParser {
    /// HWP 바이너리에서 별표 추출
    pub fn from_hwp(data: &[u8]) -> Result<Vec<AnnexInfo>>;

    /// HWPX ZIP에서 별표 추출
    pub fn from_hwpx(data: &[u8]) -> Result<Vec<AnnexInfo>>;

    /// 텍스트에서 별표 영역 감지 (Legal Chunker 연동)
    pub fn detect_annexes(text: &str) -> Vec<AnnexRegion>;

    /// 테이블 → Markdown 변환 (기존 TableData::to_markdown() 확장)
    pub fn table_to_markdown(table: &TableData) -> String;
}
```

**핵심 로직**:
1. HWP: 본문 텍스트에서 `별표\d+` 패턴 탐지 → 해당 위치부터 다음 별표까지를 하나의 AnnexInfo로
2. HWPX: `Contents/section*.xml`에서 별표 마커 찾기 → 테이블 노드 수집
3. 외부 첨부파일: 법제처 API 응답의 별표 URL에서 HWPX 다운로드 → 파싱 (korea-law MCP 레이어에서 처리)

### 4.3 HWPX 테이블 리팩토링

**파일**: `core/src/hwpx/parser.rs`

현재: 문자열 기반 `find()`/`split()` → XML 파싱
변경: `quick-xml` 이벤트 기반 파싱

```rust
// 변경 전 (현재)
fn parse_section_xml(xml: &str) -> ... {
    while let Some(start) = xml[pos..].find("<hp:tbl>") { ... }
}

// 변경 후
fn parse_section_xml(xml: &[u8]) -> Result<SectionContent> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"hp:tbl" => {
                parse_table_element(&mut reader, &e)?;
            }
            // ...
        }
    }
}
```

**추가 구현**:
- 셀 병합 정보 추출 (`<hp:cellAddr>` → rowSpan/colSpan)
- HWP의 `CellSpan` 구조체 재활용
- 중첩 테이블 지원

### 4.4 자연어 날짜 파서

**새 파일**: `core/src/utils/date_parser.rs`

```rust
pub struct DateResult {
    pub date: NaiveDate,           // chrono::NaiveDate
    pub end_date: Option<NaiveDate>, // 범위인 경우 (최근 3개월)
    pub format: DateFormat,
    pub confidence: f32,
}

pub enum DateFormat {
    Absolute,    // 2024년 3월 1일
    Relative,    // 어제, 내일, 3일 전
    Duration,    // 최근 3개월, 올해 상반기
    Legal,       // 시행일, 공포일
    Weekday,     // 다음주 화요일
}

pub struct KoreanDateParser {
    reference_date: NaiveDate,
}

impl KoreanDateParser {
    pub fn new(reference: NaiveDate) -> Self;
    pub fn parse(&self, text: &str) -> Option<DateResult>;
    pub fn to_yyyymmdd(date: &NaiveDate) -> String;
}
```

**지원 패턴**:

| 유형 | 예시 | 변환 |
|------|------|------|
| 절대 날짜 | `2024년 3월 1일`, `2024.3.1` | 20240301 |
| 상대 날짜 | `어제`, `내일`, `모레`, `그제` | 기준일 ± N |
| N일/주/월 전후 | `3일 전`, `2주 후`, `6개월 이내` | 기준일 ± 계산 |
| 요일 | `다음주 화요일`, `이번주 금요일` | 요일 계산 |
| 기간 | `최근 3개월`, `올해 상반기`, `작년` | 시작~종료일 |
| 분기 | `2024년 1분기`, `올해 하반기` | 분기 시작일 |
| 법률 특화 | `시행일`, `공포일로부터 30일 이내` | 기준일 + 오프셋 |

### 4.5 체인 함수

**새 파일**: `core/src/legal/chains.rs`

체인 함수는 여러 개별 도구를 조합하는 **오케스트레이션 레이어**. Rust에서는 데이터 구조와 조합 로직만 정의하고, 실제 API 호출은 Node.js(korea-law MCP) 레이어에서 수행.

```rust
/// 체인 실행 계획 (korea-law MCP에서 실행)
pub struct ChainPlan {
    pub chain_type: ChainType,
    pub steps: Vec<ChainStep>,
}

pub enum ChainType {
    FullResearch,           // 포괄적 법률 조사
    ActionBasis,            // 행정 처분 법적 근거
    CompareOldNew,          // 개정 전후 비교
    SearchWithInterpretation, // 조문 + 해석례
    ExtractAnnexes,         // 별표/별지 추출
    CompareDelegation,      // 3단 위임 구조 비교
    FindSimilarPrecedents,  // 유사 판례 찾기
    ResearchSpecialized,    // 전문기관 결정례 조사
}

pub struct ChainStep {
    pub tool_name: String,           // MCP 도구명
    pub params: serde_json::Value,   // 파라미터
    pub depends_on: Vec<usize>,      // 선행 스텝 인덱스
    pub parallel_group: Option<u32>, // 병렬 실행 그룹
}

impl ChainPlan {
    /// 자연어 쿼리 → 실행 계획 생성
    pub fn from_query(chain_type: ChainType, query: &str) -> Self;

    /// 결과 취합 (각 스텝 결과 → 통합 Markdown)
    pub fn aggregate_results(results: &[StepResult]) -> String;
}
```

**8개 체인 도구 정의**:

| # | 체인 | 스텝 구성 | 병렬 가능 |
|---|------|----------|----------|
| 1 | `chain_full_research` | search_law → get_law_text → [search_precedents ∥ search_interpretations] | 후반 2개 |
| 2 | `chain_action_basis` | search_law → get_law_text → search_interpretations → search_admin_appeals | 순차 |
| 3 | `chain_compare_old_new` | get_law_text(현행) ∥ get_law_text(이전) → diff | 전반 2개 |
| 4 | `chain_search_with_interpretation` | search_law → [get_law_text ∥ search_interpretations] | 후반 2개 |
| 5 | `chain_extract_annexes` | search_law → get_annex_urls → download_hwpx → parse_annex | 순차 |
| 6 | `chain_compare_delegation` | get_law_text(법률) ∥ get_law_text(시행령) ∥ get_law_text(시행규칙) | 전체 병렬 |
| 7 | `chain_find_similar_precedents` | search_precedents → filter_by_similarity | 순차 |
| 8 | `chain_research_specialized` | [search_tax_tribunal ∥ search_constitutional ∥ search_ftc] | 전체 병렬 |

---

## 3. Phase 5: napi-rs Node.js 래퍼 & npm 배포

### 5.1 프로젝트 구조

```
packages/core-native/
├── Cargo.toml
├── src/
│   └── lib.rs              # #[napi] 매크로로 core API 노출
├── package.json            # @mdm/core 메타패키지
├── npm/                    # 플랫폼별 패키지
│   ├── darwin-arm64/
│   ├── darwin-x64/
│   ├── linux-x64-gnu/
│   └── linux-arm64-gnu/
├── index.js                # 자동생성 (플랫폼 선택 로더)
├── index.d.ts              # 자동생성 (TypeScript 타입)
└── .github/
    └── workflows/
        └── build-native.yml
```

### 5.2 napi-rs API 노출

```rust
// packages/core-native/src/lib.rs
use napi_derive::napi;
use mdm_core::{hwp, hwpx, legal, utils};

#[napi(object)]
pub struct ParseResult {
    pub text: String,
    pub tables: Vec<TableResult>,
    pub images: Vec<ImageInfo>,
    pub metadata: serde_json::Value,
}

#[napi(object)]
pub struct AnnexResult {
    pub annex_type: String,
    pub number: u32,
    pub title: String,
    pub markdown: String,
    pub tables: Vec<TableResult>,
}

#[napi(object)]
pub struct DateResult {
    pub date: String,          // YYYYMMDD
    pub end_date: Option<String>,
    pub format: String,
    pub confidence: f64,
}

#[napi(object)]
pub struct ChainPlanResult {
    pub chain_type: String,
    pub steps: Vec<serde_json::Value>,
}

#[napi]
pub fn parse_hwp(data: Buffer) -> napi::Result<ParseResult> {
    let result = hwp::HwpParser::from_bytes(&data)?;
    Ok(result.into())
}

#[napi]
pub fn parse_hwpx(data: Buffer) -> napi::Result<ParseResult> {
    let result = hwpx::HwpxParser::from_bytes(&data)?;
    Ok(result.into())
}

#[napi]
pub fn parse_annex_hwp(data: Buffer) -> napi::Result<Vec<AnnexResult>> {
    let annexes = legal::AnnexParser::from_hwp(&data)?;
    Ok(annexes.into_iter().map(Into::into).collect())
}

#[napi]
pub fn parse_annex_hwpx(data: Buffer) -> napi::Result<Vec<AnnexResult>> {
    let annexes = legal::AnnexParser::from_hwpx(&data)?;
    Ok(annexes.into_iter().map(Into::into).collect())
}

#[napi]
pub fn parse_korean_date(text: String, reference_date: Option<String>) -> napi::Result<DateResult> {
    let ref_date = reference_date
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y%m%d").ok())
        .unwrap_or_else(|| Local::now().date_naive());
    let parser = utils::KoreanDateParser::new(ref_date);
    parser.parse(&text).map(Into::into).ok_or_else(|| napi::Error::from_reason("날짜 파싱 실패"))
}

#[napi]
pub fn create_chain_plan(chain_type: String, query: String) -> napi::Result<ChainPlanResult> {
    let ct = ChainType::from_str(&chain_type)?;
    let plan = legal::ChainPlan::from_query(ct, &query);
    Ok(plan.into())
}

#[napi]
pub fn aggregate_chain_results(results_json: String) -> napi::Result<String> {
    let results: Vec<StepResult> = serde_json::from_str(&results_json)?;
    Ok(legal::ChainPlan::aggregate_results(&results))
}

#[napi]
pub fn parse_legal_document(text: String, law_name: String) -> napi::Result<Vec<serde_json::Value>> {
    let chunks = legal::KoreanLegalChunker::new(&law_name).chunk(&text)?;
    Ok(chunks.into_iter().map(|c| serde_json::to_value(c).unwrap()).collect())
}
```

### 5.3 빌드 타겟 (GitHub Actions)

| OS | Arch | Target | 패키지명 |
|----|------|--------|---------|
| macOS | ARM64 | `aarch64-apple-darwin` | `@mdm/core-darwin-arm64` |
| macOS | x64 | `x86_64-apple-darwin` | `@mdm/core-darwin-x64` |
| Linux | x64 | `x86_64-unknown-linux-gnu` | `@mdm/core-linux-x64-gnu` |
| Linux | ARM64 | `aarch64-unknown-linux-gnu` | `@mdm/core-linux-arm64-gnu` |

> Windows는 korea-law MCP가 Render(Linux)에서만 돌아가므로 Phase 5에서는 제외. 필요시 추가.

### 5.4 npm 배포 구조

```json
// @mdm/core package.json
{
  "name": "@mdm/core",
  "version": "1.0.0",
  "main": "index.js",
  "types": "index.d.ts",
  "optionalDependencies": {
    "@mdm/core-darwin-arm64": "1.0.0",
    "@mdm/core-darwin-x64": "1.0.0",
    "@mdm/core-linux-x64-gnu": "1.0.0",
    "@mdm/core-linux-arm64-gnu": "1.0.0"
  },
  "publishConfig": { "access": "public" }
}
```

---

## 4. Phase 6: korea-law MCP 통합

### 6.1 @mdm/core 의존성 추가

**파일**: `law/korea-law/package.json`

```json
{
  "dependencies": {
    "@mdm/core": "^1.0.0"
  }
}
```

### 6.2 체인 도구 MCP 등록

**새 파일**: `law/korea-law/src/mcp/chain-tools.ts`

```typescript
import { createChainPlan, aggregateChainResults } from '@mdm/core';

// MCP server.ts에 체인 도구 8개 등록
server.tool('chain_full_research', {
  description: '포괄적 법률 조사 (법령 + 조문 + 판례 + 해석례 일괄)',
  inputSchema: { type: 'object', properties: { query: { type: 'string' } } },
  handler: async ({ query }) => {
    const plan = createChainPlan('FullResearch', query);
    const results = await executeChainSteps(plan.steps);
    return aggregateChainResults(JSON.stringify(results));
  }
});
// ... 나머지 7개 동일 패턴
```

### 6.3 별표 도구 MCP 등록

```typescript
import { parseAnnexHwpx } from '@mdm/core';
import axios from 'axios';

server.tool('extract_annexes', {
  description: '법령 별표/별지를 Markdown 테이블로 변환',
  inputSchema: {
    type: 'object',
    properties: {
      law_mst: { type: 'string' },
      annex_number: { type: 'number' }
    }
  },
  handler: async ({ law_mst, annex_number }) => {
    // 1. 법제처 API에서 별표 HWPX URL 조회
    const annexUrl = await getAnnexUrl(law_mst, annex_number);
    // 2. HWPX 다운로드
    const { data } = await axios.get(annexUrl, { responseType: 'arraybuffer' });
    // 3. @mdm/core로 파싱
    const annexes = parseAnnexHwpx(Buffer.from(data));
    return annexes.map(a => a.markdown).join('\n\n');
  }
});
```

### 6.4 위원회/조약 API 확장

**파일**: `law/korea-law/src/api/extended-api.ts`

법제처 Open API 엔드포인트 추가:

| API | 엔드포인트 | target 파라미터 |
|-----|----------|----------------|
| 조세심판원 결정 | `law.go.kr/DRF/lawService.do` | `ttSpecialDecc` |
| 공정거래위 결정 | `law.go.kr/DRF/lawService.do` | (cmtInfo 계열) |
| 조약 목록 | `law.go.kr/DRF/lawSearch.do` | `trty` |
| 조약 본문 | `law.go.kr/DRF/lawService.do` | `trtyInfo` |
| 자치법규 연계 | `data.go.kr/15031994` | 별도 API |
| 법령해석례 | `law.go.kr/DRF/lawSearch.do` | `expc` |
| 고용노동부 해석 | `law.go.kr/DRF/lawSearch.do` | `moelCgmExpc` |

### 6.5 자연어 날짜 도구 MCP 등록

```typescript
import { parseKoreanDate } from '@mdm/core';

server.tool('parse_date', {
  description: '한국어 날짜 표현 → YYYYMMDD 변환 ("최근 3개월", "작년", "시행일로부터 30일")',
  inputSchema: {
    type: 'object',
    properties: {
      text: { type: 'string' },
      reference_date: { type: 'string', description: 'YYYYMMDD 기준일 (생략시 오늘)' }
    }
  },
  handler: ({ text, reference_date }) => {
    return parseKoreanDate(text, reference_date);
  }
});
```

### 6.6 Render 배포

`law/render.yaml`의 `korea-law-mcp` 서비스:
- **빌드 변경**: napi-rs 프리빌드 바이너리는 npm install 시 자동 다운로드 → 추가 빌드 불필요
- `@mdm/core-linux-x64-gnu`가 Render의 Linux x64 환경에 자동 매칭

---

## 5. Phase 7: WASM 범용 배포 (별도 마일스톤)

### 7.1 wasm-bindgen 래퍼

```
packages/core-wasm/
├── Cargo.toml
├── src/lib.rs       # #[wasm_bindgen]으로 core API 노출
└── pkg/             # wasm-pack build 출력
```

### 7.2 빌드 최적화

```toml
# Cargo.toml
[profile.release]
lto = true
opt-level = 's'
```

Post-processing: `wasm-opt -Os`

### 7.3 npm 배포

- `@mdm/core-wasm` — 브라우저 + Node.js 범용
- 입력: `Uint8Array` (파일시스템 접근 불가)
- 성능: napi-rs 대비 ~45% 느림 (허용 가능)

---

## 6. 테스트 전략

### 단위 테스트 (Rust)

| 모듈 | 테스트 파일 | 커버리지 목표 |
|------|-----------|-------------|
| 별표 감지 | `tests/annex_tests.rs` | 패턴 10+ 케이스 |
| 별표 파서 | `tests/annex_parser_tests.rs` | 실제 법제처 HWPX |
| HWPX 테이블 | `tests/hwpx_table_tests.rs` | 병합 셀 포함 |
| 날짜 파서 | `tests/date_parser_tests.rs` | 30+ 한국어 표현 |
| 체인 계획 | `tests/chain_tests.rs` | 8개 체인 타입 |

### 통합 테스트 (Node.js)

| 테스트 | 내용 |
|--------|------|
| napi-rs 바인딩 | `@mdm/core` 함수 호출 검증 |
| MCP 도구 | 체인 도구 E2E (mock API) |
| 별표 파이프라인 | URL → 다운로드 → 파싱 → Markdown |

### 실제 데이터 테스트

- 법제처에서 별표 HWPX 5건 다운로드 → 파싱 검증
- KRX 법규 마크다운 파일 (기존 `legal_integration_test.rs`)
- 날짜 파서: 법률 문서 실제 날짜 표현 수집

---

## 7. 의존성

### Rust (core/Cargo.toml 추가)

```toml
chrono = "0.4"            # 날짜 계산
# quick-xml 이미 있음
# 기타 기존 의존성 유지
```

### Rust (packages/core-native/Cargo.toml)

```toml
[dependencies]
mdm-core = { path = "../../core" }
napi = { version = "3", features = ["serde-json"] }
napi-derive = "3"
serde_json = "1"
chrono = "0.4"

[build-dependencies]
napi-build = "2"
```

### Node.js (korea-law)

```json
{
  "@mdm/core": "^1.0.0"
}
```

---

## 8. 일정

| Phase | 작업 | 예상 기간 |
|-------|------|----------|
| **4.1-4.2** | 별표/별지 패턴 + 파서 | 2일 |
| **4.3** | HWPX 테이블 quick-xml 리팩토링 | 1일 |
| **4.4** | 자연어 날짜 파서 | 1일 |
| **4.5** | 체인 함수 정의 | 0.5일 |
| **4.6** | Rust 테스트 | 1일 |
| **5.1-5.4** | napi-rs 래퍼 + npm 배포 | 2일 |
| **6.1-6.6** | korea-law MCP 통합 + 배포 | 2일 |
| **7** | WASM (별도 마일스톤) | 2일 |
| **합계** | Phase 4-6 | **~10일** |

---

## 9. 리스크 및 완화

| 리스크 | 확률 | 영향 | 완화 |
|--------|------|------|------|
| napi-rs Render 빌드 실패 | 중 | 높 | Linux x64 프리빌드로 해결, 로컬 빌드 불필요 |
| 법제처 별표 HWPX 형식 변경 | 낮 | 중 | 다양한 별표 파일로 테스트 |
| quick-xml 마이그레이션 호환성 | 중 | 중 | 기존 테스트 + 새 테스트로 검증 |
| WASM 크레이트 호환성 | 낮 | 낮 | Phase 7에서 별도 검증, cfb/zip/quick-xml 모두 호환 확인됨 |
| npm 스코프 충돌 | 낮 | 낮 | @mdm org 이미 확보 (beasthan2025) |

---

## 10. 성공 기준

- [ ] `@mdm/core` npm 배포 완료 (darwin-arm64, darwin-x64, linux-x64)
- [ ] korea-law MCP 도구 수 64 → 87+ 달성
- [ ] 별표 HWPX 5건 → Markdown 테이블 변환 성공률 95%+
- [ ] 한국어 날짜 30개 표현 → YYYYMMDD 변환 정확도 95%+
- [ ] 체인 도구 8개 MCP 등록 및 E2E 테스트 통과
- [ ] Render 배포 후 `korea-law-mcp.onrender.com` 정상 응답
