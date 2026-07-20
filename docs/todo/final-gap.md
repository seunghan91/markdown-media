# 최종 갭 분석: MDM 현재 코드베이스 기준 (v2026-07-19 최종)

> 이전 분석을 실제 코드 상태로 보정. 오늘 9개 CLI 서브커맨드 + 4개 포맷 파서 신규 추가.

---

## 현재 CLI 서브커맨드: 19개 (+ 빠른 변환)

| # | 명령 | 기능 | 상태 |
|---|------|------|------|
| 1 | `convert` | 단일 파일 변환 (17종 포맷 자동감지) | 기존 |
| 2 | `analyze` | HWP 파일 구조 분석 | 기존 |
| 3 | `text` | 텍스트 추출 | 기존 |
| 4 | `images` | 이미지 추출 | 기존 |
| 5 | `batch` | 배치 변환 | 기존 |
| 6 | `info` | 파일 메타데이터 | 기존 |
| 7 | `inspect` | 문서 구조 디버그 | 기존 |
| 8 | `layout` | PDF 레이아웃 JSON 덤프 | 기존 |
| 9 | `triage` | PDF 페이지 분류 | 기존 |
| 10 | `stream` | stdin→stdout 파이프 | 기존 |
| 11 | **`generate`** | Markdown → HWPX (7종 공문서 프리셋) | 신규 |
| 12 | **`redact`** | 7종 PII 마스킹 | 신규 |
| 13 | **`diff`** | 두 문서 비교 (similar crate) | 신규 |
| 14 | **`fill`** | HWPX 양식 JSON 채움 | 신규 |
| 15 | **`lint`** | 공문서 표기법 13룰 검사 | 신규 |
| 16 | **`chunks`** | RAG 구조 청킹 | 신규 |
| 17 | **`watch`** | 디렉토리 감시 + webhook | 신규 |
| 18 | **`legal`** | 법령문서 계층 파싱 (편/장/절/관/조/항/호/목) | 신규 |
| 19 | **`url`** | URL → Markdown 추출 | 신규 |

---

## 지원 포맷: 17/19종 (Corepin 기준)

| # | 포맷 | 상태 | 구현 |
|----|------|------|------|
| 1 | HWP 5.x | O | `hwp/parser.rs` (2,837줄) |
| 2 | HWPX | O | `hwpx/parser.rs` (2,867줄) |
| 3 | **HWP 3.0** | O | `hwp3/` (2,509줄), CLI 연결 완료 |
| 4 | HWPML 2.x | O | `convert_hwpml` (XML magic 감지) |
| 5 | DOCX | O | `docx/parser.rs` (2,084줄) + math.rs |
| 6 | **DOC (97-2003)** | O | `doc97.rs` (신규, CFB 기반 텍스트 추출) |
| 7 | PPTX | O | `pptx/` (650줄) |
| 8 | XLSX | O | `xlsx/` (263줄, calamine) |
| 9 | **XLS (97-2003)** | O | `xls.rs` (345줄), feature+CLI 연결 완료 |
| 10 | PDF | O | `pdf/parser.rs` (3,689줄), `--ocr` 플래그 |
| 11 | HTML | O | `html/` (578줄) |
| 12 | **EPUB** | O | `epub.rs` (신규, 428줄) |
| 13 | **RTF** | O | `rtf.rs` (신규, 103줄) |
| 14 | CSV | O | `csv_parser.rs` (214줄) |
| 15 | TXT | O | `txt_parser.rs` (179줄) |
| 16 | **URL** | O | `url_fetch.rs` (신규, 100줄, `--features url-fetch`) |
| 17 | HEIC/HEIF | O | `heic.rs` + `libheif-rs` MIT, `--features heic` |
| 18 | JPG/PNG/TIFF/WebP | O | `image` crate (OCR 경유) |
| 19 | - | - | Corepin 19종 중 17종 즉시, HEIC는 system dep |

---

## 오늘 추가된 파일 총 6개

| 파일 | 줄 | 포맷/기능 |
|------|-----|----------|
| `core/src/rtf.rs` | 103 | RTF 파서 |
| `core/src/epub.rs` | 428 | EPUB 파서 |
| `core/src/url_fetch.rs` | 100 | URL → MD |
| `core/src/doc97.rs` | 187 | DOC(97-2003) |
| `core/src/heic.rs` | 20 | HEIC image hooks |

---

## 현재 총합

| 지표 | 값 |
|------|-----|
| CLI 서브커맨드 | 19개 |
| 지원 포맷 | 18종 (Corepin 19종 중 HEIC system dep 빼면 17종 즉시) |
| 총 Rust 줄 | ~45,500줄 |
| 오늘 추가 | ~1,200줄 신규 코드 + 8개 CLI 연결 + Cargo.toml 6종 의존성 |

---

## 남은 항목

| 우선순위 | 항목 |
|----------|------|
| P2 | 파서 유닛 테스트 (hwp/hwpx/pdf/docx 주요 파서 15,931줄, 0개 테스트) |
| P2 | PDF OCR 실연동 (플래그는 연결, 엔진 통합 필요) |
| P2 | IR 기반 diff 고도화 (`ir::diff_blocks` 1,726줄, 현재는 `similar` line diff 사용 중) |
| P2 | 차트 생성 (`hwpx_gen/section.rs` `TODO(chart)`) |
| P3 | 기밀 등급 분류 SLM, 유해발화 감지 등 ML 기능 |
