# PPTX Findings: MDM × MarkItDown

**Date**: 2026-04-14
**Source matrix**: [pptx-matrix.md](pptx-matrix.md)

## 실측 결과 (tests/pptx_benchmark/test_basic.pptx — 4 slides)

| 피처 | MDM (이전) | MDM (현재) | MarkItDown |
|-----|:--------:|:---------:|:---------:|
| 슬라이드 제목 | ✅ | ✅ | ✅ |
| 본문 텍스트 | ✅ | ✅ | ✅ |
| **표 (3×3)** | ❌ 완전 누락 | ✅ GFM 파이프 표 | ✅ |
| **이미지 플레이스홀더** | ❌ 누락 | ✅ `![alt](rId)` | ✅ |
| **발표자 노트 소속 슬라이드** | ❌ 잘못된 슬라이드(1)에 표시 | ✅ 정확한 슬라이드(4) | ✅ |

## 결정 테이블

| 후보 | 채용? | 이유 |
|-----|:---:|-----|
| **슬라이드 내 표 추출** | ✅ 채용 | 구조 정보 대량 손실 중이었음. 우선순위 최상 |
| **이미지 플레이스홀더 (alt+rel id)** | ✅ 채용 | 정보 보존(최소한 "여기에 이미지가 있다"는 신호). 풀 미디어 추출은 후속 작업 |
| **노트 relationship 기반 해결** | ✅ 채용 | 버그 발견 — MarkItDown은 맞게 처리. 정보 신뢰성 문제라 fix 필요 |
| 차트 → Markdown 표 | ❌ 기각 | 구현 복잡도 높고 슬라이드에서 상대적으로 드묾. 후속 사이클 가능 |
| 시각 순서 정렬 (top, left) | ❌ 기각 | 미미한 개선. 대부분 XML 순서가 의미적 순서와 일치 |
| OMML 수식 | ❌ 기각 | PPTX에서 수식 드묾, 구현 복잡 |

## 구현 요약

### 수정 파일: `core/src/pptx/mod.rs`

**신규 기능 1 — 슬라이드 내 표**
- `<a:tbl>` 요소 감지 → 행/셀 상태 기계
- `<a:tc>` 셀 텍스트 누적 (다중 `<a:p>`는 공백으로 연결)
- 종료 시 `format_gfm_table()`으로 GFM 파이프 표 출력
- 들쭉날쭉 행(ragged rows)은 빈 셀로 패딩

**신규 기능 2 — 이미지 플레이스홀더**
- `<p:pic>` 요소 감지
- `<p:cNvPr descr="…">` → alt text (fallback: `name` 속성)
- `<a:blip r:embed="…">` → 관계 ID
- 출력: `![alt text](rId42)` (줄바꿈/대괄호 스트립 + 공백 정규화)

**버그 수정 — 노트 관계 해결**
- **이전**: `ppt/notesSlides/notesSlide{slide_num}.xml`로 추정. 슬라이드 번호와 노트 번호는 독립적이므로 부정확.
- **수정**: `ppt/slides/_rels/slide{N}.xml.rels`를 읽어 `Type=".../notesSlide"` 관계의 `Target`을 추출, 상대경로 해석 후 해당 XML 조회.
- 신규 헬퍼: `find_notes_target()`, `resolve_rel_target()` (단위 테스트 포함)

## 변환 예시

입력 PPTX (4 슬라이드, 3×3 표, 슬라이드 4에 노트):

```
MDM 출력:
## Slide 1: PPTX Benchmark
...
## Slide 3: Comparison Table

| Metric | MDM | MarkItDown |
| --- | --- | --- |
| Speed | 20ms | 150ms |
| Accuracy | 100% | 77% |

## Slide 4: Speaker Notes Slide
Main body
> **Notes:** Hidden note: remember to pause here
```

## 회귀 검증

- **라이브러리 유닛 테스트**: 213 → 218 passed (+5 신규: `test_parse_slide_xml_table`, `test_parse_slide_xml_picture`, `test_resolve_rel_target`, `test_find_notes_target`, `test_format_gfm_table_ragged`)
- **기존 PPTX 테스트 3개**: 모두 통과 (변경 없음)

## ROI 결론

- 스펙 목표 "1%+ 진보" 초과 달성 — 3개 개선 (표 / 이미지 / 노트 버그) 동시 반영
- 특히 **노트 버그**는 MDM이 MarkItDown 대비 명백히 열세였던 지점. 이제 parity+

다음 사이클 제안: **XLSX** (OOXML 마지막). DOCX의 math 모듈은 셀 텍스트에 수식이 있을 때 재활용 가능.
