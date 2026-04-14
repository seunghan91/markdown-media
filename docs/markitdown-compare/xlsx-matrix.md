# XLSX 변환 대조 매트릭스 + Findings: MDM vs MarkItDown

**Date**: 2026-04-14
**MDM ref**: `core/src/xlsx/mod.rs` (263 LOC, `calamine` 기반)
**MarkItDown ref**: `_xlsx_converter.py` (pandas + openpyxl → HTML → markdownify)

## 아키텍처

| 측면 | MDM | MarkItDown |
|-----|-----|-----------|
| 라이브러리 | calamine (Rust, XLSX+XLS+ODS 네이티브) | pandas + openpyxl / xlrd |
| 파이프라인 | XLSX → calamine Range → GFM 파이프 표 (직접) | XLSX → DataFrame → to_html → markdownify → MD |
| 중간 단계 | 없음 | HTML 경유 (pandas가 float 포맷 변경 위험) |

## 실측 비교 (tests/xlsx_benchmark/test_basic.xlsx — 4 sheets)

| 시트 | 내용 | MDM | MarkItDown | 비고 |
|-----|-----|:---:|:---------:|-----|
| People | 3행 단순 데이터 | ✅ | ✅ | 동등 |
| Sales | 수식 포함 (Qty×Price=Total) | ✅ `5.5`, `55` | ✅ `5.50`, `55.00` | **MDM이 원본에 더 충실** — pandas는 float 컬럼을 강제로 `%.2f` 포매팅 |
| 한글 | 유니코드 시트명 + 한글 셀 | ✅ | ✅ | 동등 |
| Symbols | 셀에 `|` 문자 포함 | ✅ `a\|b` | ❌ **`a|b` (미이스케이프 → 표 구조 파손)** | **MDM 우세** |

## 피처 대조

| 피처 | MDM | MarkItDown | 비고 |
|-----|:---:|:---------:|-----|
| 멀티시트 `## Sheet Name` | ✅ | ✅ | 동등 |
| GFM 파이프 표 | ✅ | ✅ | 동등 |
| 수식 → 캐시값 | ✅ (calamine) | ✅ (openpyxl data_only) | 동등 |
| Float 무결성 (5.5 → "5.5") | ✅ | ⚠️ `5.50` (강제 포매팅) | **MDM 우세** |
| 정수화 (42.0 → "42") | ✅ `format_float` | ⚠️ `42.0` | **MDM 우세** |
| 셀 내 `|` 이스케이프 | ✅ `a\|b` | ❌ **표 파손** | **MDM 우세 ⚠️** |
| 빈 행/열 후행 트리밍 | ✅ | ❌ | **MDM 우세** |
| DateTime 처리 | ✅ DateTime/DateTimeIso/Duration | ⚠️ pandas Timestamp (동일/유사) | 동등 |
| 유니코드 | ✅ | ✅ | 동등 |
| XLS (레거시 BIFF) | ✅ calamine | ✅ xlrd | 동등 |
| **ODS (OpenOffice)** | ✅ calamine | ❌ | **MDM 유일** |
| 하이퍼링크 셀 | ❌ | ❌ | 동등 |
| 셀 서식 (볼드/색상) | ❌ | ❌ | 동등 (Markdown에서 부분 표현 가능하지만 양쪽 모두 포기) |
| 병합 셀 | ❌ | ❌ | 동등 |
| 차트 | ❌ | ❌ | 동등 |

## 합계

| 카테고리 | MDM 우세 | MarkItDown 우세 | 동등 |
|---------|:--------:|:---------------:|:----:|
| 핵심 변환 | 3 | 0 | 2 |
| 데이터 충실성 | 2 | 0 | 1 |
| 포맷 지원 범위 | 1 (ODS) | 0 | 2 |
| **계** | **6** | **0** | **5** |

## 결론 — 채용 없음

MDM이 XLSX에서 **모든 측정 항목에서 MarkItDown 이상**. 채용할 로직 없음.

특히 `a|b` 같은 셀이 있는 경우 MarkItDown은 파이프 이스케이프 누락으로 **표 구조 자체를 파손**. MDM의 `escape_pipe`가 표준 GFM 동작.

스펙의 "1%+" 목표는 이미 달성되어 있음 (측정상 6 우세 / 0 열세).

## 추가 고려 사항 (이번 사이클 범위 밖)

아래는 양쪽 모두 미지원, 후속 사이클 가능:
- 하이퍼링크 셀 (`=HYPERLINK(...)` → `[text](url)`)
- 셀 주석/노트
- 시트 내 차트 → Markdown 표
- 이름 관리자(named range) 해결

위 항목들은 포맷 공통 이슈라 별도 개선 사이클로 처리.

## ROI 결론

- **개선 0건 / 검증 완료** — 스펙 목표 이미 달성
- 다음 사이클: HTML
