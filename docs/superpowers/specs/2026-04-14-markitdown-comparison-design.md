# MDM × MarkItDown 비교 개선 설계

**Date**: 2026-04-14
**Status**: Approved, in progress
**Owner**: seunghan
**Related**: `docs/markitdown-compare/` (산출물)

## 목표

Microsoft/markitdown을 레퍼런스로 두고 MDM의 **동일 포맷(PDF/DOCX/PPTX/XLSX/HTML/CSV/TXT)** 변환을 **정확도·보존율 기준 최소 1%** 개선한다. 미지원 포맷 포팅(.msg/YouTube/audio 등)은 **스코프 밖**.

## 판정 기준

1. **정확도(Quality)** — 포맷별 피처 테스트 통과율 (예: DOCX 39 피처). MarkItDown이 MDM보다 잘 뽑는 피처가 있으면 원인 파악 → 로직 채용.
2. **보존율(Preservation)** — 원본 구조/서식/미디어의 Markdown 재현 비율. 이미지/표/각주/하이퍼링크/헤딩 계층 등.

속도·메모리는 **비교 대상 아님** (이미 Rust가 우세, 별도 벤치 필요 없음).

## 사이클 (포맷당 1회)

```
① Clone & Research   reference/markitdown/ 클론, WebSearch로 최신 상태·알려진 한계 조사
② 대조 매트릭스       피처 × {MDM 동작, MarkItDown 동작, 차이} 표
③ 차이 식별           findings.md — 채용/기각 판단 + 이유
④ 개선 구현           Rust 코드 수정 (테스트 우선)
⑤ 회귀 검증           cargo test 전체 통과 + 벤치마크 재실행
⑥ 커밋 + ROI 리뷰     atomic commit, 개선 수치 기록, 다음 포맷으로 진행 여부 결정
```

**ROI 중단 조건**: 한 포맷 사이클 종료 시 개선이 1% 미달이거나 채용할 차이가 0건이면 해당 포맷은 "parity 유지"로 기록하고 다음 포맷으로 넘어감(또는 중단 보고).

## 포맷 순서

1. **DOCX** — OOXML 패밀리 기점, 학습 재활용 가능 (→ PPTX, XLSX)
2. **PPTX** — OOXML, DOCX에서 배운 로직 적용
3. **XLSX** — OOXML
4. **PDF** — 별도 파이프라인, 난이도 최상
5. **HTML** — 비교적 단순
6. **CSV / TXT** — 가장 단순, 마지막 정리

## 산출물 구조

```
docs/markitdown-compare/
├── docx-matrix.md         # 사이클 ②
├── docx-findings.md       # 사이클 ③
├── pptx-matrix.md
├── pptx-findings.md
└── ...
reference/
└── markitdown/            # 클론 (gitignored)
```

## 클론 처리

- 위치: `reference/markitdown/` (루트 `reference/` 디렉토리, `.gitignore` 등록)
- 이유: MIT 라이선스라도 외부 프로젝트 전체 코드를 자체 리포에 커밋하면 기여/업스트림 추적이 혼탁해짐. 읽기 전용 참조만 유지.
- 갱신: 사이클 ①에서 `git pull` 또는 최신 릴리즈 태그로 재클론.

## 비교 방법론

**정적 비교 (우선)**: 양쪽 소스를 읽고 동일 입력에 대한 출력 규칙 비교. MarkItDown은 Python + 성숙한 라이브러리(mammoth, openpyxl, pdfminer) 어댑터라 출력 컨벤션과 edge case 처리가 명확.

**동적 비교 (필요 시)**: 동일 샘플 문서를 양쪽에 통과시켜 Markdown diff. `tests/benchmark_engine.py`의 BLEU/edit distance 재활용.

## 안전장치

- 각 사이클 개선은 **기존 MDM 테스트 159개 전체 통과** 필수
- 개선 커밋은 **atomic** (한 사이클 = 한 PR 또는 복수 logical commits)
- 채용 시 MarkItDown 코드를 **직접 복붙하지 않음** — 로직 아이디어만 채용, Rust idiomatic 재작성 (MIT 라이선스 표기는 해당 로직 주석에 attribution 추가)

## 범위 밖 (명시)

- MarkItDown만 지원하는 포맷(.msg, YouTube, audio STT, EPUB, ZIP, Jupyter) — 필요 없음 결정됨
- MDM의 Markdown → 문서 역변환
- 속도/메모리 벤치마크 추가 (이미 우세, 증명 불필요)
- MarkItDown에 기여(PR)하는 작업 (이번 스코프 아님)

## 성공 지표

| 포맷 | 현재 MDM | 목표 |
|------|---------|-----|
| DOCX | 100% (39/39 vs Pandoc) | MarkItDown 대비 ≥1% 우위 확인 또는 신규 edge case 커버 |
| PDF  | 93% (27/29 vs Marker)  | MarkItDown 대비 ≥1% 우위 + 미달 2개 피처 보완 |
| 기타 | 비교 데이터 없음 | MarkItDown 대비 ≥1% 우위 또는 parity |

## 타임라인

포맷당 예상 0.5~1일 (발견되는 차이 수에 따라 가변). 7포맷 전체 약 5~7일.
