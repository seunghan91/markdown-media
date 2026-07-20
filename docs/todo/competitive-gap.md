# Corepin 스펙 분석 & MDM 보강 라이브러리

> Date: 2026-07-19
> 대상: Corepin API (한국형 통합 문서 파서 SaaS)

---

## Corepin 제공 기능 vs MDM 현황

### 문서 입력 포맷 (19종)

| 포맷 | Corepin | MDM 현재 | 갭 | 보강 라이브러리 |
|------|---------|----------|-----|----------------|
| HWP 5.x | O | O | - | - |
| HWPX | O | O | - | - |
| HWP 3.0 (1996-2002) | O | X | DOC/HWP3 미지원 | `cfb` (이미 의존성) + 바이너리 파서 직접 구현 |
| HWPML 2.x | O | X | HWPML 미지원 | `quick-xml` (이미 의존성) + HWPML 스키마 구현 |
| DOCX | O | O | - | - |
| DOC (97-2003) | O | X | 구버전 워드 | `cfb` (이미 의존성) + OLE2 바이너리 직접 구현 |
| PPTX | O | O | - | - |
| XLSX | O | O | - | - |
| XLS (97-2003) | O | X | `calamine`이 이미 지원 | `calamine` 0.26 → 0.36 업그레이드 (Xls struct 내장) |
| PDF | O | O | - | - |
| HTML | O | O | - | - |
| EPUB | O | X | 전자책 미지원 | `zip` + `quick-xml` 직접 파싱 (EPUB = ZIP+HTML. `epub` crate는 GPL-3.0) |
| RTF | O | X | 리치텍스트 미지원 | `rtf-parser` 0.4 (MIT, 2026-06-11 최신) |
| CSV | O | O | - | - |
| JPG | O | O | - | - |
| PNG | O | O | - | - |
| HEIC | O | X | 아이폰 사진 | `libheif-sys` 5.3 (MIT, system libheif 필요) |
| TIFF | O | O (`image` crate) | - | - |
| WebP | O | O (`image-webp`) | - | - |

### URL 입력

| 기능 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| URL → 본문 추출 | O (URL당 1원) | X | `reqwest` + `htmd` 0.5 (HTML→MD, Apache-2.0) |

### 출력 형식 (4종)

| 출력 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| text | O | O | - |
| markdown | O | O | - |
| json (구조화 블록) | O | 부분 (IR 없음) | IR 도입 필요 |
| html | O | X | `pulldown-cmark` + HTML 렌더러 직접 구현 |

### 역변환 (Markdown → 문서)

| 출력 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| → HWPX | O (2원/p) | X | `quick-xml` (이미 있음) + HWPX XML 생성기 직접 구현 |
| → DOCX | O (2원/p) | X | `docx-rs` 0.4 (MIT, 2026-04-02, 2.8M DL) |
| → PDF (A4) | O (2원/p) | X | `printpdf` 0.11 (MIT, 2026-07-18, 1.8M DL) |

### OCR

| 기능 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| PP-OCRv5 korean | O (자체, 1위 주장) | Tesseract/EasyOCR (Python) | `ort` 2.0.0-rc (MIT/Apache-2.0) — Rust ONNX 추론 |
| Tesseract 폴백 | - | O | `tesseract` 0.15 (MIT, Rust 바인딩) |
| OCR 자동감지 (needsOcr) | O | X | `pdf-extract` + 한글/제어문자 비율 분석 |
| OCR 페이지만 과금 | O (+2원/p) | - | 아키텍처 설계 이슈 |

### 개인정보 마스킹 (+5원/호출)

| 기능 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| 19종 PII 탐지 | O | X | `regex` (이미 있음) + 패턴 직접 구현 |
| 주민등록번호 | O | X | `\d{6}-[1-4]\d{6}` + Luhn 검증 |
| 전화번호 | O | X | 정규식 패턴 |
| 이메일 | O | X | 정규식 패턴 |
| 계좌번호 | O | X | 은행별 계좌번호 포맷 |
| 카드번호 | O | X | Luhn + BIN range |
| 이름 | O | X | ML 필요 (한국인 이름 패턴) |
| 서식 보존 마스킹 | O | X | IR 기반 마스킹 필요 |

### 기밀 등급 분류 (+20원/호출)

| 기능 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| 6단계 + 11유형 분류 | O | X | SLM/분류기 직접 학습 필요 (Rust crate 없음) |

### 유해발화 감지 (+5원/호출)

| 기능 | Corepin | MDM 현재 | 보강 |
|------|---------|----------|------|
| 욕설·차별·인젝션 감지 | O | X | `regex` 키워드 + SLM (Rust crate 없음) |

### 문서 비교 (diff)

| 기능 | Corepin | MDM 현재 | 보강 라이브러리 |
|------|---------|----------|----------------|
| 크로스포맷 diff | O (2원/p) | 기본 LCS diff | `similar` 3.1 (Apache-2.0, 161M DL, Myers+patience) |
| 신구대조표 | O | X | IR 기반 블록 diff 설계 필요 |

### 양식 채우기

| 기능 | Corepin | MDM 현재 | 보강 |
|------|---------|----------|------|
| HWPX 양식 + JSON → 채움 | O (2원/p) | X | `quick-xml` + XML 직접 조작 |

### PDF 테이블 정밀 감지

| 기능 | Corepin | MDM 현재 | 보강 |
|------|---------|----------|------|
| 시각 기반 (layout-aware) | O (자체 주장) | `pdf-extract` 기본 | `lopdf` + 직접 구현. Rust 전용 크레이트 없음 |
| 병합 셀 복원 | O | 부분 | 2-pass 그리드 빌더 직접 구현 |
| 읽기 순서 보존 | O | 부분 | XY-Cut + 휴리스틱 직접 구현 |

### 배치/대량 처리

| 기능 | Corepin | MDM 현재 | 보강 |
|------|---------|----------|------|
| Async batch (≤100파일) | O | X | `tokio` + `rayon` (이미 있음) |

---

## 추천 Cargo.toml 추가 의존성

```toml
# === P0 — 바로 추가 (MIT/Apache-2.0, 시스템 의존성 없음) ===

# DOCX 출력 (역변환)
docx-rs = { version = "0.4", optional = true }

# HTML → Markdown (URL 파싱용)
htmd = { version = "0.5", optional = true }

# 문서 diff
similar = { version = "3", optional = true }

# RTF 파서
rtf-parser = { version = "0.4", optional = true }

# PDF 생성
printpdf = { version = "0.11", optional = true }

# === P1 — 시스템 의존성 있음 (feature flag) ===

# ONNX 추론 (PP-OCRv5 OCR)
ort = { version = "2.0.0-rc", optional = true }

# Tesseract 바인딩 (OCR 폴백)
tesseract = { version = "0.15", optional = true }

# HEIC 이미지
libheif-sys = { version = "5", optional = true }

# === 업그레이드 ===

# XLS 지원 포함 (0.26 → 0.36)
calamine = "0.36"
```

---

## 우선 구현 로드맵 (Corepin 갭 기준)

### Sprint 1 (1-2주): 당장 가능한 것
1. **calamine 0.36 업그레이드** — XLS(97-2003) 즉시 지원
2. **rtf-parser 추가** — RTF 즉시 지원
3. **EPUB 파서** — `zip` + `quick-xml`로 간단 구현

### Sprint 2 (2-4주): 중간 난이도
4. **htmd 추가** — HTML→MD, URL 본문 추출
5. **similar 추가** — 문서 diff 고도화
6. **PII 마스킹** — `regex`로 7종 패턴 구현 (주민번호·전화·이메일·카드·계좌·여권·운전면허)
7. **PDF OCR 자동감지** — `needsOcr` 판정 로직

### Sprint 3 (4-8주): 고난이도
8. **ort + PP-OCRv5** — 내장 OCR 엔진
9. **PDF 정밀 테이블 감지** — 선 기반 + 클러스터 기반 (레퍼런스 설계 참고)
10. **IR 도입** — 6블록 타입, 공통 IR로 모든 파서 통합

### Sprint 4 (8-12주): 역변환
11. **docx-rs 추가** — Markdown → DOCX
12. **printpdf 추가** — Markdown → PDF
13. **quick-xml HWPX 생성** — Markdown → HWPX

### Sprint 5 (12-16주): ML 기능
14. **기밀 등급 분류** — SLM 학습
15. **유해발화 감지** — 키워드 + 모델
16. **한국어 이름 PII** — 패턴 + 컨텍스트

---

## 즉시 보강 가능한 5가지 (MVP)

1. **XLS 지원** — `calamine` 0.36 업그레이드 한 줄
2. **RTF 지원** — `rtf-parser` 추가 + 100줄 래퍼
3. **EPUB 지원** — EPUB = ZIP+XHTML, `quick-xml`으로 200줄
4. **PII 마스킹** — `regex` 7종 패턴, 300줄
5. **문서 diff** — `similar` crate, 200줄

위 5가지는 모두 MIT/Apache-2.0 호환, 시스템 의존성 없음, 합쳐서 1주일 작업.
