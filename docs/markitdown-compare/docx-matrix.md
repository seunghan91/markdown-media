# DOCX 변환 대조 매트릭스: MDM vs MarkItDown

**Date**: 2026-04-14
**MarkItDown ref**: `reference/markitdown/` (shallow clone, main branch)
**Scope**: `core/src/docx/parser.rs` (MDM) vs `reference/markitdown/packages/markitdown/src/markitdown/converters/_docx_converter.py` + `converter_utils/docx/pre_process.py`

## 아키텍처 비교

| 측면 | MDM | MarkItDown |
|-----|-----|-----------|
| 구현 언어 | Rust (quick_xml 스트리밍) | Python (mammoth + BeautifulSoup + markdownify) |
| 파이프라인 | DOCX ZIP → XML 직접 파싱 → IR → Markdown | DOCX → OMML 전처리 → mammoth(HTML) → markdownify(MD) |
| 중간 단계 | 없음 | HTML 경유 (2단계) |
| 의존성 | 없음 (자체 파서) | mammoth, beautifulsoup4, markdownify |
| 코드 라인 | 2,005 LOC (`parser.rs`) | 약 160 LOC wrapper + 의존 라이브러리 |

## 피처 대조

### 기본 구조

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| H1-H9 헤딩 | ✅ (`pStyle` + `outlineLvl`) | ✅ (mammoth 기본) | MDM이 outline level도 추가 인식 — 한국어 문서에서 우세 |
| 문단 | ✅ | ✅ | 동등 |
| 볼드/이탤릭/취소선/밑줄 | ✅ | ✅ | 동등 |
| 폰트 크기 | ⚠️ 파싱은 함 (Markdown 미표현) | ❌ | Markdown은 폰트 크기 미지원 (둘 다 부분적) |
| 색상 | ❌ | ❌ | Markdown은 색상 미지원 |
| 페이지 레이아웃 | 무시 | 무시 | 동등 |

### 리스트

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 글머리표(bullet) | ✅ | ✅ | 동등 |
| 번호 매기기(decimal) | ✅ | ✅ | 동등 |
| 한국어 번호(가/나/다) | ✅ | ❌ | **MDM 우세** — `ganada_marker` + `chosung_marker` |
| 중첩 레벨 | ✅ (`ilvl`) | ✅ | 동등 |
| 스타일 기반 리스트(`ListBullet`, `ListNumber`) | ✅ | ✅ | 동등 |
| **인접 리스트 병합 버그** | ✅ 분리 유지 | ⚠️ 병합됨 (upstream 버그 #1549) | **MDM 우세** — mammoth→markdownify 경로 한계 |

### 표(Table)

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 단순 표 | ✅ | ✅ | 동등 |
| GFM 파이프 문법 | ✅ | ✅ | 동등 |
| 가로 병합(`gridSpan`) | ✅ | ⚠️ GFM은 병합 미지원, 빈 셀로 표시 | MDM이 더 풍부한 표현 |
| 세로 병합(`vMerge`) | ✅ | ⚠️ 동일 | 동등한 한계 |
| 다중 문단 셀 | ✅ | ✅ | 동등 |
| 셀 내 포매팅(볼드/이탤릭) | ✅ | ✅ | 동등 |
| 파이프 이스케이프(`\|`) | ✅ | ✅ | 동등 |

### 링크 / 각주

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 외부 하이퍼링크(`r:id`) | ✅ | ✅ | 동등 |
| 내부 북마크(`w:anchor`) | ✅ → `#anchor` | ✅ | 동등 |
| 하이퍼링크 제목(title) | ❌ | ✅ (markdownify) | **MarkItDown 우세** (미미함) |
| JavaScript/악성 URL 스트립 | ❌ | ✅ (markdownify) | **MarkItDown 우세** (보안) |
| URL 이스케이프 | 부분적 | ✅ | **MarkItDown 우세** |
| 각주 참조 | ✅ GFM `[^1]` | ⚠️ mammoth는 `[1]` 일반 링크 | **MDM 우세** (GFM 표준) |
| 미주(endnote) | ✅ | ⚠️ 일부만 | **MDM 우세** |

### 이미지

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 이미지 추출 | ✅ assets/ | ⚠️ 기본 변환엔 embed 안 됨 | **MDM 우세** — 미디어 번들 |
| Alt text | ✅ | ✅ | 동등 |
| Content-addressable (hash) | ✅ | ❌ | **MDM 우세** |
| 데이터 URI 잘라냄(data:) | N/A | ✅ | DOCX엔 거의 없음 |
| OCR (이미지 내 텍스트) | ❌ | ⚠️ 별도 플러그인(markitdown-ocr, LLM 필요) | 범위 밖 |

### 수식(Math) — **핵심 차이**

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| OMML 인라인 수식(`w:oMath`) | ❌ 무시됨 | ✅ `$LaTeX$` | **MarkItDown 우세 ⚠️** |
| OMML 블록 수식(`w:oMathPara`) | ❌ 무시됨 | ✅ `$$LaTeX$$` | **MarkItDown 우세 ⚠️** |
| OMML → LaTeX 변환 테이블 | ❌ | ✅ `latex_dict.py` + `omml.py` | MarkItDown에 상세 변환기 존재 |

### 블록 요소

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 블록인용(Quote 스타일) | ✅ | ✅ (mammoth style map) | 동등 |
| 코드 블록 | ⚠️ 부분적 | ⚠️ 부분적 | DOCX 자체 코드 블록 표현 빈약 |
| 수평선(`---`) | 텍스트로 보존 | 텍스트로 보존 | 동등 |

### 변경 내역 / 주석

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 삽입 `w:ins` | ✅ (본문 포함) | ✅ (mammoth accept) | 동등 — "변경 수락" 기본 |
| 삭제 `w:del` | ✅ 자연 제외 (`w:delText` 무시) | ✅ 제외 | 동등 |
| 주석 `w:commentReference` | 무시 | 무시 | 동등 |
| 수정 메타데이터 | ✅ `revision` 수집 | ❌ | **MDM 우세** (미미) |

### 구조화 문서 태그 / 폼

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 콘텐츠 컨트롤(`w:sdt`) | 자식 텍스트만 | 자식 텍스트만 | 동등 |
| 체크박스 (`w:checkBox` in sdt) | ❌ | ⚠️ HTML 경로 한정 `[x]`/`[ ]` | **MarkItDown 우세** (미미 — DOCX 체크박스는 드묾) |

### 메타데이터

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| Title | ✅ | ✅ | 동등 |
| Author | ✅ | ❌ (타이틀만) | **MDM 우세** |
| Subject, Keywords | ✅ | ❌ | **MDM 우세** |
| 프론트매터 YAML | ✅ 자동 생성 | ❌ | **MDM 우세** |

## 합계

| 카테고리 | MDM 우세 | MarkItDown 우세 | 동등 |
|---------|:--------:|:---------------:|:----:|
| 구조 | 0 | 0 | 5 |
| 리스트 | 2 | 0 | 4 |
| 표 | 2 | 0 | 5 |
| 링크/각주 | 2 | 3 | 3 |
| 이미지 | 2 | 0 | 3 |
| **수식** | **0** | **3** ⚠️ | **0** |
| 블록 | 0 | 0 | 3 |
| 변경/주석 | 1 | 0 | 3 |
| 구조화 태그 | 0 | 1 | 1 |
| 메타데이터 | 3 | 0 | 1 |
| **계** | **12** | **7** | **28** |

## 결론

- MDM이 **구조·표·메타데이터·한국어** 영역에서 확실히 우세
- MarkItDown은 **수식(OMML→LaTeX) 변환에서 유일하게 실질적 우세** + 보안/URL 처리 3건의 미미한 우세
- 채용 1순위: **OMML 수식 지원** (MDM의 명백한 누락, MarkItDown에 이미 구현되어 있고 로직이 명료)
- 채용 2순위: URL 이스케이프 강화 (하이퍼링크 보안 측면)
- 기각: 체크박스(DOCX에서 드묾), markdownify 하이퍼링크 title(가치 낮음)
