# Gap Analysis: markdown-media vs 레퍼런스 엔진

> Reference: `reference/kkdoc/` (한국 문서 파서 레퍼런스, MIT)
> Date: 2026-07-19

## 요약

레퍼런스 엔진은 **양방향(파싱 + 생성)** 한국 문서 엔진이고, markdown-media(MDM)는 **단방향(문서 → 마크다운/미디어번들)** 추출 엔진이다. MDM이 흡수할 수 있는 25개 기능 갭을 P0/P1/P2로 분류했다.

---

## MDM > 레퍼런스 (MDM이 앞서는 영역)

| 영역 | MDM | 레퍼런스 |
|------|-----|--------|
| 코어 언어 | Rust (21K+ LOC, 고성능) | TypeScript (Node.js) |
| 데스크톱 앱 | Tauri 2 (macOS/Windows) | 없음 |
| 크롬 확장 | WASM 기반 오프라인 변환 | 없음 |
| 미디어 번들 | `.mdx` + `.mdm` + `assets/` SHA-256 dedup | 단순 markdown string |
| 미디어 참조 문법 | `@[[image]]` `~[[table]]` `$[[equation]]` 등 | 없음 |
| Python 패키지 | PyPI 배포 (mdm-parser) | 없음 |
| FastAPI 서버 | REST API + 파일 업로드 | 없음 |
| Docker | 멀티스테이지 + OCR + CJK 폰트 | 없음 |
| Rails 웹서비스 | Rails 8 + Inertia + Svelte | 없음 |
| PPTX 파서 | OOXML 슬라이드 추출 | 없음 |
| CSV/TSV 파서 | 네이티브 | 없음 |
| HTML/블로그 파서 | 네이버블로그, 티스토리 등 | 없음 |
| 법률문서 처리 | 한국 법령 패턴, 챕터 분할 | 없음 |
| Image optimizer | image crate 기반 최적화 | 없음 |
| DRM 감지 | Fasoo DRMONE 탐지 | 없음 |
| SVG 렌더링 | resvg 기반 | HWPX 전용 |
| Rust WASM | wasm-bindgen 기반 완전 WASM | JS 전용 |

---

## 레퍼런스 > MDM (MDM이 흡수해야 할 기능 갭)

### P0 — Critical (MDM 생존/경쟁력에 필수)

| # | 기능 | 레퍼런스 구현 | MDM 현황 | 우선순위 |
|---|------|-----------|----------|---------|
| 1 | **내장 OCR 엔진** | PP-OCRv5 korean, ONNX 로컬 추론, ~1s/page, 11,945자 사전, API 키 불필요 | Tesseract/EasyOCR 의존 (pipeline) | P0 |
| 2 | **MCP 서버 풀도구** | 15개 도구 (parse_document, fill_form, patch_document, generate_document, render_document, redact_document 등) | 기본 게이트웨이만 존재 | P0 |
| 3 | **HWPX/Markdown 생성기** | markdownToHwpx() — 양방향, 공문서 7종 프리셋, 표·수식·차트 생성 | 단방향 추출만 (명시적 out-of-scope) | P0 |
| 4 | **양식 인식/채움** | extractFormFields, extractFormSchema, fillForm, fillHwpx (서식 100% 보존) | 없음 | P0 |
| 5 | **문서 비교 (diff)** | compare() — 크로스포맷 블록/셀 레벨 LCS diff | 기본 LCS diff만 존재 | P0 |
| 6 | **PII 탐지/마스킹** | 주민번호·전화·이메일·카드·계좌·여권·운전면허, 서식 보존 마스킹 | 없음 | P0 |
| 7 | **PDF 테이블 감지 고도화** | 선 기반 + 클러스터 기반 이중 감지, 과소분할 재구성 | 기본 테이블 추출만 | P0 |
| 8 | **수식 파싱/생성** | HULK → LaTeX 변환, Markdown math → HWPX equation 왕복 | 부분 구현 | P0 |
| 9 | **차트 생성** | OOXML chartSpace 20종 (```chart 펜스드 블록 → HWPX 차트) | 없음 | P0 |

### P1 — Important (MDM 완성도 향상)

| # | 기능 | 레퍼런스 구현 | MDM 현황 | 우선순위 |
|---|------|-----------|----------|---------|
| 10 | **RAG 청킹** | 구조적 청킹 (헤딩·listDepth·표 위계 → breadcrumb JSON) | 없음 | P1 |
| 11 | **인쇄 렌더링** | IRBlock → HTML → PDF (puppeteer), markdown/block → PDF API | 없음 | P1 |
| 12 | **HWPX 레이아웃 렌더러** | 조판 캐시 기반 SVG 절대배치, Reflow 엔진 (98% 한컴 정합) | 없음 | P1 |
| 13 | **공문서 프리셋** | 7종 (기안문·보고서·계획서·통지·회의록·개조식·보도자료) | 없음 | P1 |
| 14 | **Lossless roundtrip patch** | patchHwpx/patchHwp — 서식 100% 보존 텍스트 치환, session API | 없음 | P1 |
| 15 | **HWP3 파서** | 1996-2002 레거시, 조합형→유니코드 (5,893자 lookup) | 없음 | P1 |
| 16 | **HWPML 2.x 파서** | XML 기반 HWP, HeadingType 헤딩 감지 | 없음 | P1 |
| 17 | **XLS (BIFF8) 파서** | Excel 97-2003 OLE2+BIFF8, Workbook stream, SST | 없음 (XLSX만) | P1 |
| 18 | **표 형식 프로필** | 참조 HWPX에서 표 서식 추출 → 생성 시 적용 | 없음 | P1 |
| 19 | **HWPX 검증** | ZIP 무결성, mimetype, 필수 파일, XML well-formedness | 없음 | P1 |
| 20 | **AI 클라이언트 자동설치** | npx setup → Claude/Cursor/Windsurf 등 8종 자동 감지+설정 | 없음 | P1 |

### P2 — Nice to have

| # | 기능 | 레퍼런스 구현 | MDM 현황 | 우선순위 |
|---|------|-----------|----------|---------|
| 21 | **공문서 표기법 린트** | 13개 규칙 (날짜·시간·금액·붙임 등) | 없음 | P2 |
| 22 | **감시 모드** | 디렉토리 watch + webhook 알림 | 없음 | P2 |
| 23 | **도장 날인** | placeSealHwpx — 도장 이미지 부유 배치 | 없음 | P2 |
| 24 | **Source map** | XML 바이트 위치 매핑 (정밀 편집용) | 없음 | P2 |
| 25 | **HWP 배포용 복호화** | AES-128 ECB + MSVC LCG | 없음 | P2 |

---

## 아키텍처 설계 패턴 갭

레퍼런스가 가진 아키텍처 패턴 중 MDM에도 적용할 가치가 높은 것들:

### 1. IR (Intermediate Representation) 패턴
```
Buffer → detectFormat() → format-specific parser → IRBlock[] → blocksToMarkdown() → Markdown
```
- MDM은 각 포맷 파서가 직접 마크다운을 생성 → IR 방식은 크로스포맷 diff, 통합 청킹, 일관된 후처리 가능

### 2. IRBlock[] 공통 타입
- 6개 블록 타입 (heading, paragraph, table, list, image, separator)
- `IRSpan` 인라인 서식 (bold, italic, code)
- `IRCell` (colSpan, rowSpan, nested blocks[])
- MDM은 자체 IR이 없음 → 도입하면 모든 포맷에서 균일한 출력 보장

### 3. 2-pass 테이블 빌더
- Pass 1: colSpan/rowSpan 고려한 그리드 계산
- Pass 2: 셀 배치
- MDM의 Rust table builder에 적용 가능

### 4. 오류 복원 전략
- 깨진 ZIP: Local File Header 직접 스캔 (PK\x03\x04)
- 손상된 CFB: LenientCfbReader
- HWP 배포용 복호화 폴백

### 5. OCR 통합 설계
- 페이지별 품질 신호 (`needsOcr` 판정: 한글 비율, 제어문자 비율, PUA 비율)
- 선택적 OCR: 필요한 페이지만 OCR
- 외부 프로바이더 주입 가능 (Tesseract, Claude Vision 등)

---

## 우선 구현 추천 로드맵

### Phase 1: 기반 다지기 (2-4주)
1. **IRBlock[] 도입** — IR 타입을 Rust로 구현, 모든 파서가 IR을 출력하도록 리팩터
2. **내장 OCR 엔진** — PP-OCRv5 ONNX 추론을 Rust에 통합 (ort crate)
3. **PDF 테이블 감지 고도화** — 선 기반 + 클러스터 기반 이중 감지

### Phase 2: 기능 확장 (4-8주)
4. **MCP 서버 풀도구** — 15개 도구 구현 (parse_document, fill_form, render_document 등)
5. **HWPX 생성기** — Markdown → HWPX, 공문서 7종 프리셋
6. **양식 인식/채움** — extractFormFields, fillHwpx
7. **문서 비교 (diff)** — IR 기반 크로스포맷 diff
8. **PII 탐지/마스킹** — 한국 특화 개인정보 패턴

### Phase 3: 완성도 (8-12주)
9. **RAG 청킹** — 구조적 청킹 + breadcrumb
10. **인쇄 렌더링** — IR → HTML → PDF
11. **HWPX 레이아웃 렌더러** — SVG/PNG
12. **Lossless roundtrip patch**
13. **HWP3 / HWPML / XLS(BIFF8) 파서**

### Phase 4: 편의기능 (12-16주)
14. **AI 클라이언트 자동설치** (npx mdm setup)
15. **공문서 표기법 린트**
16. **감시 모드**
17. **차트 생성**, **수식 생성**, **도장 날인**, **Source map**

---

## 레퍼런스 코드 재사용 가능성 평가

| 모듈 | 재사용 난이도 | 이유 |
|------|------------|------|
| IR 타입 정의 | 낮음 | 순수 TS 타입 → Rust struct 변환 간단 |
| PP-OCRv5 ONNX 추론 | 중간 | Rust에서 ort crate로 직접 구현, 모델 사양/사전만 그대로 사용 |
| PDF 테이블 감지 알고리즘 | 중간 | TS → Rust 포팅, 수학적 로직은 동일 |
| 공문서 로직 | 중간 | 한국 공문서 규칙은 언어 독립적 |
| 수식 변환 (HULK ↔ LaTeX) | 중간 | 토큰맵과 파싱 규칙은 포팅 가능 |
| HWP3 조합형 변환 | 낮음 | 5,893자 lookup table → Rust const array |
| HWP5 바이너리 파서 | 높음 | OLE2/CFB 구조체 완전 포팅 필요하나 MDM은 이미 Rust HWP5 파서 보유 |

---

## 결론

MDM은 **추출 파이프라인 성능**에서 Rust로 레퍼런스를 크게 앞서나, **기능 완성도**에서는 레퍼런스의 양방향·문서조작·AI연동 기능들이 대거 누락되어 있다. 특히 OCR / MCP 서버 / 양식처리 / diff / PII 마스킹 / 공문서 생성의 6개 영역이 핵심 갭이다. 레퍼런스 설계를 참고해 이 기능들을 Rust로 구현하면 MDM은 한국 문서 처리의 압도적 솔루션이 될 수 있다.
