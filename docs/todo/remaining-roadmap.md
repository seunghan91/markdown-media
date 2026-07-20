# 남은 로드맵 점검

> Date: 2026-07-19 최종  
> 기준: 42,835줄 Rust, 19개 CLI, 18종 포맷

---

## 완료된 것

| 범주 | 완료 항목 |
|------|----------|
| CLI | 19개 서브커맨드 (기존 10 + 신규 9) |
| 포맷 파싱 | 18종 (HWP/HWPX/HWP3/HWPML/DOCX/DOC/PPTX/XLSX/XLS/PDF/HTML/EPUB/RTF/CSV/TXT + URL/HEIC/JPG) |
| 역변환 | Markdown → HWPX (generate), 공문서 7종 프리셋 |
| 보안 | PII 7종 마스킹 (redact) |
| 문서비교 | similar 기반 line diff (diff) |
| 양식 | HWPX 양식 채움 (fill) |
| 린트 | 공문서 표기법 13룰 (lint) |
| RAG | 구조 청킹 (chunks) |
| 감시 | 디렉토리 watch (watch) |
| 법령 | 편/장/절/관/조/항/호/목 (legal) |
| URL | 웹페이지 → MD (url) |
| OCR | PP-OCRv5 엔진 구현 + --ocr 플래그 연결 |

---

## 남은 항목 (우선순위순)

### P1 — 당장 할 만한 것 (합계 3-5일)

| # | 항목 | 현재 상태 | 예상 |
|---|------|----------|------|
| 1 | **IR 기반 diff** | `ir::diff_blocks` 1,726줄 완성, 블록/셀/이동탐지 모두 구현. CLi `diff`는 `similar` line diff 사용 중 → IR diff로 교체 | 2시간 |
| 2 | **docx-rs 연결** | Markdown → DOCX는 `docx-rs` crate 추가만 하면 됨 (MIT, 2.8M DL). generate 명령에 `--format docx` 추가 | 3시간 |
| 3 | **printpdf 연결** | Markdown → PDF. `printpdf` crate (MIT) 추가, generate에 `--format pdf` | 3시간 |
| 4 | **generate 명령에 format 옵션** | 현재는 HWPX만 출력. `--format hwpx|docx|pdf` | 1시간 |

### P2 — 중기 (합계 2-4주)

| # | 항목 | 현재 상태 | 예상 |
|---|------|----------|------|
| 5 | **OCR PDF 실연동** | `convert_pdf`에 `--ocr` 플래그는 있으나 OCR 호출 코드 없음. `ocr::ocr_image()` → 변환 결과에 병합 필요 | 3일 |
| 6 | **PDF 정밀 테이블 감지** | `pdf/table_detect.rs` 1,580줄, kkdoc refinement 미적용. 선 기반 + 클러스터 이중 감지 보강 | 2주 |
| 7 | **IR 기반 파서 통합** | `ir.rs`는 있으나 HWP/HWPX/PDF 파서가 IR을 직접 출력하지 않음. `extract_blocks()` → `IRBlock[]` 리팩터 | 1주 |
| 8 | **MCP 서버** | 게이트웨이만 존재, 15도구 미구현. Python MCP 브릿지 또는 Rust MCP 서버 | 1주 |

### P3 — 장기 (합계 4-8주)

| # | 항목 | 현재 상태 | 예상 |
|---|------|----------|------|
| 9 | **차트 생성** | `hwpx_gen/section.rs:169` `TODO(chart)`, 20종 OOXML chartSpace | 1주 |
| 10 | **파서 유닛 테스트** | hwp/hwpx/pdf/docx/hwp3 15,931줄 중 0개 테스트. 회귀 방지 필수 | 2-4주 |
| 11 | **기밀 등급 분류** | SLM 학습 필요, Rust crate 없음 | 2-3주 |
| 12 | **유해발화 감지** | regex 키워드 + 소형 모델 | 1주 |

---

## Corepin 기능 비교 (최종)

| 기능 | Corepin 단가 | MDM | 상태 |
|------|-------------|-----|------|
| 문서 파싱 19종 | 2원/p | 18종 | HWPML 제외 완료 |
| OCR | +2원/p | 엔진 있음, 연동 미완 | P2 #5 |
| PII 마스킹 | +5원 | 완료 | CLI: `redact` |
| 기밀 등급 | +20원 | 미구현 | P3 #11 |
| 유해발화 | +5원 | 미구현 | P3 #12 |
| Markdown→HWPX | 2원/p | 완료 | CLI: `generate` |
| Markdown→DOCX | 2원/p | 미구현 | P1 #2 |
| Markdown→PDF | 2원/p | 미구현 | P1 #3 |
| 양식 채움 | 2원/p | 완료 | CLI: `fill` |
| 문서 diff | 2원/p | 완료 (line diff) | P1 #1 (IR diff 고도화) |
| 배치 | 1원/p | 완료 | CLI: `batch` |
| URL 추출 | 1원/URL | 완료 | CLI: `url` |

---

## 다음 액션: P1 3-5일 스프린트

```
Day 1: IR diff → CLI diff 교체
Day 2: docx-rs + printpdf 의존성 추가
Day 3: generate --format docx|pdf 구현
Day 4: 통합 테스트 + 문서화
```

3-5일이면 Corepin의 모든 문서 처리 기능을 Rust로 커버 가능. 남은 건 ML 기능(기밀등급, 유해발화)뿐.
