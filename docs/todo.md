# MDM Project TODO List

> **Last Updated**: 2026.04.02
> **Overall Progress**: Phase 1-3 완료, Phase 4-7 진행 예정

---

## 📊 Implementation Status Overview

```
JavaScript Parser:  ████████████████████ 100%
Python Parser:      ████████████████████ 100%
Rust Core:          ████████████████░░░░  80%
HWP/PDF Converter:  ████████████████░░░░  80%
CLI Tool:           ████████████████████ 100%
CI/CD:              ████████████████████ 100%
WASM:               ████████████████████ 100%
npm Publish:        ░░░░░░░░░░░░░░░░░░░░   0%
─── Phase 4-7 (korea-law 통합) ───
별표/별지 파서:     ░░░░░░░░░░░░░░░░░░░░   0%
HWPX quick-xml:     ░░░░░░░░░░░░░░░░░░░░   0%
날짜 파서:          ░░░░░░░░░░░░░░░░░░░░   0%
체인 도구:          ░░░░░░░░░░░░░░░░░░░░   0%
napi-rs 래퍼:       ░░░░░░░░░░░░░░░░░░░░   0%
@mdm/core npm:      ░░░░░░░░░░░░░░░░░░░░   0%
korea-law MCP 통합: ░░░░░░░░░░░░░░░░░░░░   0%
WASM 범용 배포:     ░░░░░░░░░░░░░░░░░░░░   0%
```

---

## ✅ Completed

### JavaScript Parser (`packages/parser-js/`)

- [x] Tokenizer 구현 (`src/tokenizer.js`)
- [x] Parser 클래스 구현 (`src/parser.js`)
- [x] Renderer 구현 (`src/renderer.js`)
- [x] MDM Loader 구현 (`src/mdm-loader.js`)
- [x] Demo 스크립트 (`src/demo.js`)
- [x] 기본 테스트 케이스 (8개 통과)

### Documentation (`plan/`)

- [x] 프로젝트 아키텍처 수립
- [x] 구현 가이드 작성 (`implementation-guide.md`)
- [x] 테스트 전략 수립 (`testing-strategy.md`)
- [x] 시장 분석 (`market-analysis.md`)
- [x] 로드맵 작성 (`roadmap.md`)

### Viewer (`viewer/`)

- [x] 단일 HTML 뷰어 (`index.html`)

---

## ❌ Not Implemented

### Phase 1: Core Infrastructure (High Priority)

#### 1.1 Rust Core Engine (`core/`)

- [x] Cargo 프로젝트 초기화
  ```bash
  cd core
  cargo init --name mdm-core
  cargo add cfb  # OLE 파싱용
  ```
- [x] HWP 바이너리 파서 (OLE 구조 분석 + 텍스트 추출)
- [x] PDF 바이너리 파서 (텍스트 추출)
- [x] DOCX 파서 (XML 구조)
- [x] 텍스트 추출 엔진 (기본 구조)
- [x] 이미지 추출 기능
- [ ] 성능 벤치마크

#### 1.2 Python Converter (`packages/parser-py/`)

- [x] 프로젝트 구조 설정
  ```bash
  cd packages/parser-py
  python -m venv venv
  pip install pyhwp pdfplumber pillow svgwrite
  ```
- [x] `hwp_to_svg.py` - 표/차트를 SVG로 변환 (기본 구현)
- [x] `pdf_processor.py` - PDF 텍스트/이미지 추출
- [x] OCR 통합 (Tesseract/EasyOCR)
- [ ] PyPI 패키지 준비 (`setup.py`)

#### 1.3 Document Converters (`converters/`)

- [x] HWP → MDX 변환기 (기본 구조)
- [x] HWPX → MDX 변환기
- [x] PDF → MDX 변환기 (기본 구조)
- [x] DOCX → MDX 변환기
- [x] 복잡한 표 → SVG 렌더러
- [ ] 차트 → PNG 캡처
- [ ] 메타데이터 추출기

---

### Phase 2: Enhanced Features (Medium Priority)

#### 2.1 JavaScript Parser 확장

- [x] 프리셋 시스템 구현
  - [x] Size 프리셋: `thumb`, `small`, `medium`, `large`
  - [x] Ratio 프리셋: `square`, `standard`, `widescreen`, `portrait`, `story`
- [ ] WebP/SVG 포맷 지원 확장
- [ ] Sidecar 파일 (.mdm) 완전 지원
- [ ] 에러 핸들링 강화
- [ ] 성능 최적화

#### 2.2 Rust Parser (`packages/parser-rs/`)

- [x] Cargo 프로젝트 설정
- [x] JavaScript 로직 포팅
- [x] WASM 컴파일 설정 (wasm-bindgen)
- [x] JavaScript 바인딩
- [ ] 브라우저 호환성 테스트

#### 2.3 CLI Tool

- [x] 명령어 구조 설계
  ```bash
  mdm convert input.hwp -o output/
  mdm validate bundle/
  mdm serve --port 3000
  ```
- [x] Convert 명령 구현
- [x] Validate 명령 구현
- [x] Serve 명령 구현
- [x] Watch 모드 (실시간 변환)
- [x] 배치 처리 지원

---

### Phase 3: Deployment & Integration (Lower Priority)

#### 3.1 npm Package Publishing

- [x] 배포 스크립트 작성 (deploy.sh)
- [ ] `beasthan2025` 계정으로 로그인 (npm login)
- [ ] `@mdm/parser` 스코프 패키지 배포 (실행 대기)
- [ ] `@mdm/cli` 패키지 배포 (실행 대기)
- [x] 버전 관리 전략 수립

#### 3.2 CI/CD Setup (`.github/`)

- [x] GitHub Actions workflow
  - [x] 자동 테스트 (`test.yml`)
  - [x] 자동 빌드 (`build.yml`)
  - [x] 자동 배포 (`publish.yml`)
- [ ] 코드 커버리지 리포트
- [ ] 자동 릴리스 노트

#### 3.3 Documentation

- [ ] API 문서 (JSDoc → HTML)
- [x] 사용자 가이드 (USER_GUIDE.md)
- [x] 기술 사양 (TECHNICAL_SPEC.md)
- [x] 기여자 가이드 (`CONTRIBUTING.md`)
- [x] Issue 템플릿
- [x] PR 템플릿

#### 3.4 Playground

- [ ] 웹 기반 데모 사이트
- [ ] 실시간 미리보기
- [ ] 코드 에디터 통합

---

## 🎯 Immediate Action Items

### This Week

1. **Rust Core 초기화**

   ```bash
   cd core
   cargo init --name mdm-core
   cargo add cfb
   ```

2. **Python 환경 설정**

   ```bash
   cd packages/parser-py
   touch __init__.py
   touch hwp_to_svg.py
   touch pdf_processor.py
   ```

3. **npm 패키지 배포**
   ```bash
   npm login  # beasthan2025
   npm publish --access public
   ```

### Next Week

1. HWP 바이너리 파싱 프로토타입
2. 표 → SVG 변환 스크립트
3. CLI 도구 기본 구조

---

## 📁 Expected Final Structure

```
markdown-media/
├── README.md
├── package.json
├── index.js
├── core/                      # [Rust] 고속 파서 엔진
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── hwp/
│       │   ├── mod.rs
│       │   ├── parser.rs
│       │   └── ole.rs
│       ├── pdf/
│       │   ├── mod.rs
│       │   └── parser.rs
│       └── docx/
│           ├── mod.rs
│           └── parser.rs
├── packages/
│   ├── parser-js/             # ✅ 완료
│   │   ├── src/
│   │   └── test/
│   ├── parser-py/             # ❌ 미구현
│   │   ├── __init__.py
│   │   ├── hwp_to_svg.py
│   │   ├── pdf_processor.py
│   │   └── setup.py
│   └── parser-rs/             # ❌ 미구현
│       ├── Cargo.toml
│       └── src/
├── converters/                # ❌ 미구현
│   ├── hwp_converter.py
│   ├── pdf_converter.py
│   └── table_to_svg.py
├── cli/                       # ❌ 미구현
│   ├── index.js
│   └── commands/
├── viewer/                    # ✅ 완료
│   └── index.html
├── docs/
│   ├── todo.md               # 이 파일
│   └── api/
├── samples/
│   ├── input/
│   └── output/
└── .github/                   # ❌ 미구현
    └── workflows/
```

---

---

## Phase 4: Rust 코어 강화 — korea-law 통합 (High Priority)

> **설계서**: `docs/superpowers/specs/2026-04-02-korea-law-integration-design.md`
> **목표**: chrisryugj/korean-law-mcp 87개 도구 수준 달성

### 4.1 별표/별지 패턴 감지 & 파서

- [ ] `core/src/legal/patterns.rs` — RE_ANNEX, RE_ANNEX_FORM, RE_ATTACHMENT 정규식 추가
- [ ] `core/src/legal/annex.rs` — AnnexParser 구현 (HWP/HWPX 별표 추출)
- [ ] AnnexInfo 구조체 (type, number, title, tables, markdown)
- [ ] HWP 별표 감지: 본문 텍스트 패턴 매칭 → 테이블 수집
- [ ] HWPX 별표 감지: section XML에서 마커 탐지 → 테이블 노드 파싱
- [ ] TableData::to_markdown() 확장 (병합 셀 렌더링 개선)
- [ ] 테스트: 실제 법제처 별표 HWPX 5건

### 4.2 HWPX 테이블 quick-xml 리팩토링

- [ ] `core/src/hwpx/parser.rs` — string 기반 → quick-xml 이벤트 파싱
- [ ] 셀 병합 정보 추출 (hp:cellAddr → rowSpan/colSpan)
- [ ] HWP의 CellSpan 구조체 재활용
- [ ] 중첩 테이블 지원
- [ ] 기존 테스트 통과 확인 + 새 테스트

### 4.3 한국어 자연어 날짜 파서

- [ ] `core/src/utils/date_parser.rs` — KoreanDateParser 구현
- [ ] 절대 날짜: 2024년 3월 1일, 2024.3.1
- [ ] 상대 날짜: 어제, 내일, 모레, 그제
- [ ] N일/주/월 전후: 3일 전, 2주 후, 6개월 이내
- [ ] 요일: 다음주 화요일, 이번주 금요일
- [ ] 기간: 최근 3개월, 올해 상반기, 작년
- [ ] 분기: 2024년 1분기, 올해 하반기
- [ ] 법률 특화: 시행일, 공포일로부터 30일 이내
- [ ] chrono 크레이트 의존성 추가
- [ ] 테스트: 30+ 한국어 날짜 표현

### 4.4 체인 함수 정의

- [ ] `core/src/legal/chains.rs` — ChainPlan, ChainStep 구조체
- [ ] 8개 ChainType enum 정의
- [ ] from_query() — 자연어 → 실행 계획 생성
- [ ] aggregate_results() — 스텝 결과 → 통합 Markdown
- [ ] 병렬 실행 그룹 표시 (parallel_group)
- [ ] 테스트: 8개 체인 타입 계획 생성

---

## Phase 5: napi-rs Node.js 래퍼 & npm 배포

### 5.1 napi-rs 프로젝트 설정

- [ ] `packages/core-native/` 초기화 (`napi new`)
- [ ] Cargo.toml — mdm-core 의존성, napi v3
- [ ] src/lib.rs — #[napi] 매크로로 API 노출:
  - parse_hwp(Buffer) → ParseResult
  - parse_hwpx(Buffer) → ParseResult
  - parse_annex_hwp(Buffer) → Vec<AnnexResult>
  - parse_annex_hwpx(Buffer) → Vec<AnnexResult>
  - parse_korean_date(String) → DateResult
  - create_chain_plan(String, String) → ChainPlanResult
  - aggregate_chain_results(String) → String
  - parse_legal_document(String, String) → Vec<JSON>

### 5.2 CI/CD & 빌드

- [ ] GitHub Actions — darwin-arm64, darwin-x64, linux-x64-gnu, linux-arm64-gnu
- [ ] TypeScript 타입 자동 생성 (index.d.ts)
- [ ] 플랫폼별 npm 패키지 생성

### 5.3 npm 배포

- [ ] @mdm/core 메타패키지 (optionalDependencies)
- [ ] @mdm/core-darwin-arm64
- [ ] @mdm/core-darwin-x64
- [ ] @mdm/core-linux-x64-gnu
- [ ] @mdm/core-linux-arm64-gnu
- [ ] beasthan2025 계정으로 npm publish --access public

---

## Phase 6: korea-law MCP 통합

### 6.1 @mdm/core 연동

- [ ] korea-law/package.json에 @mdm/core 의존성 추가
- [ ] 기존 HTTP 래퍼에서 @mdm/core 직접 호출로 전환

### 6.2 체인 도구 8개 MCP 등록

- [ ] `korea-law/src/mcp/chain-tools.ts` 신규
- [ ] chain_full_research — 포괄적 법률 조사
- [ ] chain_action_basis — 행정 처분 법적 근거
- [ ] chain_compare_old_new — 개정 전후 비교
- [ ] chain_search_with_interpretation — 조문 + 해석례
- [ ] chain_extract_annexes — 별표/별지 추출
- [ ] chain_compare_delegation — 3단 위임 구조
- [ ] chain_find_similar_precedents — 유사 판례
- [ ] chain_research_specialized — 전문기관 결정례

### 6.3 데이터 소스 확장

- [ ] 조세심판원 결정 API 연동 (ttSpecialDecc)
- [ ] 공정거래위 결정 API 연동
- [ ] 조약 검색/본문 API 연동 (trty/trtyInfo)
- [ ] 자치법규 연계 API 연동 (data.go.kr/15031994)
- [ ] 고용노동부 해석례 API 연동 (moelCgmExpc)

### 6.4 자연어 날짜 + 별표 도구 등록

- [ ] parse_date MCP 도구
- [ ] extract_annexes MCP 도구

### 6.5 배포

- [ ] Render 배포 테스트 (korea-law-mcp.onrender.com)
- [ ] MCP 도구 수 64 → 87+ 확인

---

## Phase 7: WASM 범용 배포 (별도 마일스톤)

- [ ] `packages/core-wasm/` 초기화 (wasm-bindgen)
- [ ] wasm-pack build --target nodejs
- [ ] wasm-opt -Os 최적화
- [ ] @mdm/core-wasm npm 배포
- [ ] 브라우저 HWP 뷰어 데모

---

## 🔗 References

- [HWP 파일 구조](https://www.hancom.com/etc/hwpDownload.do)
- [OLE Compound File](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/)
- [Rust CFB Crate](https://crates.io/crates/cfb)
- [pyhwp Library](https://pypi.org/project/pyhwp/)
- [MDX Official](https://mdxjs.com/)

---

**Author**: seunghan91 (npm: beasthan2025)
