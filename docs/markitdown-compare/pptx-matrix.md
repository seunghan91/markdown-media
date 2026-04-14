# PPTX 변환 대조 매트릭스: MDM vs MarkItDown

**Date**: 2026-04-14
**MDM ref**: `core/src/pptx/mod.rs` (373 LOC)
**MarkItDown ref**: `reference/markitdown/packages/markitdown/src/markitdown/converters/_pptx_converter.py` (265 LOC, python-pptx 기반)

## 아키텍처 비교

| 측면 | MDM | MarkItDown |
|-----|-----|-----------|
| 기반 | quick_xml 스트리밍 (DOCX 파서와 동일 인프라) | python-pptx |
| 슬라이드 구분 | `## Slide N: Title` + `---` | `<!-- Slide number: N -->` |
| 도형 순회 순서 | XML 순서 | (top, left) 좌표 기반 시각 순서 |

## 피처 대조

### 기본 구조

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 슬라이드 제목 (`ph type=title/ctrTitle`) | ✅ | ✅ (`# Title`) | 헤딩 레벨 다름(MDM=H2, MI=H1) |
| 본문 텍스트 | ✅ | ✅ | 동등 |
| 발표자 노트 | ✅ (`> **Notes:**`) | ✅ (`### Notes:`) | 표현 다름, 동등 |
| 슬라이드 번호 표시 | ✅ 헤딩에 포함 | ✅ 주석 | 서로 다른 관례 |
| 슬라이드 구분자 | ✅ `---` | ❌ (비어 있음) | **MDM 우세** (명시적 구분) |

### 표 (Tables)

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 슬라이드 내 표 | ❌ 완전 누락 | ✅ GFM 파이프 표 | **MarkItDown 우세 ⚠️** |
| 표 헤더 행 | ❌ | ✅ (첫 행을 `<th>`로) | MarkItDown만 |

### 이미지 / 도형

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 슬라이드 내 이미지 | ❌ 완전 누락 | ✅ `![alt](filename.jpg)` | **MarkItDown 우세 ⚠️** |
| Alt text 추출 (`descr` 속성) | ❌ | ✅ | MarkItDown만 |
| 이미지 실제 추출 (파일 저장) | ❌ | ❌ (플레이스홀더만) | 둘 다 없음 (LLM 옵션만 있음) |
| 그룹 도형 재귀 | ❌ (XML 자연 순회) | ✅ 명시적 | 미미한 차이 |

### 차트

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 차트 → Markdown 표 | ❌ | ✅ (카테고리 × 시리즈) | **MarkItDown 우세** |
| 차트 제목 | ❌ | ✅ | MarkItDown만 |

### 도형 순회

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 시각 순서 (top→bottom, left→right) | ❌ | ✅ | **MarkItDown 우세** (미미) — 읽기 순서 개선 |

### 수식 (Math)

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| OMML 수식 | ❌ | ❌ (PPTX 경로에선 pre_process_docx가 호출되지 않음) | 동등 (둘 다 미지원) |

### 포매팅

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 볼드/이탤릭 텍스트 | ❌ (평문만) | ❌ (python-pptx `shape.text`는 포매팅 없이 평문) | 동등 |
| 번호/글머리표 | ❌ | ❌ | 동등 |

## 합계

| 카테고리 | MDM 우세 | MarkItDown 우세 | 동등 |
|---------|:--------:|:---------------:|:----:|
| 기본 구조 | 1 (구분자) | 0 | 4 |
| 표 | 0 | 2 ⚠️ | 0 |
| 이미지 | 0 | 2 ⚠️ | 2 |
| 차트 | 0 | 2 | 0 |
| 순회 순서 | 0 | 1 | 0 |
| 수식 | 0 | 0 | 1 |
| 포매팅 | 0 | 0 | 2 |
| **계** | **1** | **7** | **9** |

## 결론

- PPTX는 DOCX와 반대로 **MarkItDown이 명백히 더 풍부**. MDM은 "슬라이드 텍스트 추출" 수준.
- 채용 1순위: **슬라이드 내 표** (구조 정보 대량 손실 중)
- 채용 2순위: **이미지 플레이스홀더** + alt text
- 기각: 차트(복잡하고 상대적으로 드묾), 시각 순서 정렬(미미한 개선)
