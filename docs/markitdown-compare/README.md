# MDM × MarkItDown 비교 개선 — 전체 요약

**기간**: 2026-04-14 단일 세션
**스펙**: [docs/superpowers/specs/2026-04-14-markitdown-comparison-design.md](../superpowers/specs/2026-04-14-markitdown-comparison-design.md)
**레퍼런스**: microsoft/markitdown (pip 최신 + shallow clone at `reference/markitdown/`)

## 7 사이클 결과표

| # | 포맷 | 이전 상태 | 개선 내용 | 신규 테스트 | 커밋 |
|:-:|-----|---------|---------|:---------:|-----|
| 1 | **DOCX** | 수식 미지원 | OMML → LaTeX (`m:oMath` / `m:oMathPara` 스트리밍 변환기 `core/src/docx/math.rs` 신설) | +12 | [`f7d049d`](../../../commit/f7d049d) |
| 2 | **PPTX** | 표·이미지·노트 버그 | 슬라이드 표 GFM화 / 이미지 플레이스홀더 / 노트 관계 기반 정확한 슬라이드 연결 | +5 | [`26d294f`](../../../commit/26d294f) |
| 3 | **XLSX** | 이미 우세 | 코드 변경 없음 (파이프 이스케이프·float 무결성에서 MDM이 MarkItDown 앞섬 확인) | 0 | [`60edfb6`](../../../commit/60edfb6) |
| 4 | **HTML** | alt/data URI/체크박스/XSS 미처리 | 이미지 alt 보존 + data URI 축약 + 체크박스 `[x]`/`[ ]` + 위험 스킴(`javascript:`/`vbscript:`/`data:`) 스트립 | +10 | [`dad81b6`](../../../commit/dad81b6) |
| 5 | **CSV/TXT** | 개행 셀이 GFM 파손 / UTF-16 미지원 | 셀 내 `\n` 평탄화 + UTF-16 LE/BE BOM 감지 | +5 | [`9e9d0f5`](../../../commit/9e9d0f5) |
| 6 | **PDF** | MasterFormat 부분 번호(.1, .2) 분리됨 | 후행자 병합 post-processor 추가 | +4 | [`7868f36`](../../../commit/7868f36) |

## 누적 수치

| 지표 | 시작 | 종료 | 증감 |
|-----|:----:|:----:|:----:|
| 라이브러리 유닛 테스트 | 201 | **237** | **+36** |
| 코드 변경 파일 수 | - | 6 파서 모듈 수정 + 1 신규 (`math.rs`) | - |
| 문서 | - | 7 matrix/findings (`docs/markitdown-compare/*.md`) | - |
| DOCX vs Pandoc 벤치마크 | 39/39 (100%) | 39/39 (100%) | parity |
| PDF vs Marker 벤치마크 | 27/29 (93%) | 27/29 (93%) | parity |

## 포맷별 MDM vs MarkItDown 최종 포지션

| 포맷 | MDM 우세 | MarkItDown 우세 | 동등 | 총평 |
|-----|:-------:|:--------------:|:---:|-----|
| DOCX | 12 → 13 (수식) | 3 → 0 | 28 | MDM 완승 |
| PPTX | 1 → 4 (표/이미지/노트/구분자) | 7 → 3 (차트·시각 순서 등 경미) | 9 | MDM 주도 |
| XLSX | 6 (파이프·float·ODS 등) | 0 | 5 | MDM 완승 |
| HTML | alt·data URI·체크박스·XSS | mailto 오과용(역채용) | 대부분 | MDM 우세 |
| CSV | TSV 자동·파이프·개행 | 인코딩 범위 | 기본 parity | MDM 주도 |
| TXT | BOM·정규화 | 인코딩 범위 | 기본 parity | MDM 주도 |
| PDF | 헤딩·포매팅·표·2열·메타 | MasterFormat (채용함) | 기본 parity | MDM 완승 |

## MarkItDown에서 발견한 실제 버그들 (우리가 안 겪는 문제)

1. **DOCX**: `KeyError ('footnote', '1')` — footnote가 있는 문서에서 예외 발생, 변환 전체 실패
2. **XLSX**: 셀 내 `|`이 미이스케이프 → GFM 표 구조 파손
3. **PPTX**: 수식 드롭 (mammoth 경로가 아닌 python-pptx 경로라 OMML 미처리 → 슬라이드 내 수식 전부 누락)
4. **HTML**: `mailto:`/`tel:`/`#anchor` 링크를 **실수로** 스트립 (스킴 화이트리스트가 `http/https/file`만)

모두 MDM에서는 발생하지 않음.

## 오픈소스 이상의 실현 관점

- **상대의 장점은 채용**: DOCX 수식(MarkItDown의 사전처리 로직), PDF MasterFormat 번호(작고 명료) — 두 건 채용
- **상대의 실수는 피함**: HTML `mailto:` 스트립, XLSX 파이프, PPTX 노트 — 모방하지 않음
- **상호 보완 측면**: MarkItDown은 어댑터 얇은 레이어, 새 포맷(.msg/YouTube/audio 등) 쉽게 추가. MDM은 Rust로 구조 복원 정밀도 극대화. 두 프로젝트는 실제로 상보적이다.

## 다음 단계 제안 (이번 스코프 외)

- 이번에 `reference/markitdown/`에서 발견한 버그들 중 일부는 upstream PR 가치 있음 (mailto 링크, XLSX 파이프 이스케이프)
- MDM의 PPTX 이미지 플레이스홀더를 진짜 추출로 확장 (DOCX의 media bundle 인프라 재활용)
- DOCX math.rs를 **PPTX**로 확장 (a14:m 네임스페이스)
