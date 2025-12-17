# MDM Project TODO List

> **Last Updated**: 2025.12.17
> **Overall Progress**: 20%

---

## 📊 Implementation Status Overview

```
JavaScript Parser:  ████████████████████ 100%
Python Parser:      ████████░░░░░░░░░░░░  40%
Rust Core:          ████████░░░░░░░░░░░░  40%
HWP/PDF Converter:  ██████████░░░░░░░░░░  50%
CLI Tool:           ████████████████████ 100%
CI/CD:              ████████████████████ 100%
npm Publish:        ░░░░░░░░░░░░░░░░░░░░   0%
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
- [x] HWP 바이너리 파서 (OLE 구조 분석)
- [ ] PDF 바이너리 파서
- [ ] DOCX 파서 (XML 구조)
- [x] 텍스트 추출 엔진 (기본 구조)
- [ ] 성능 벤치마크

#### 1.2 Python Converter (`packages/parser-py/`)

- [x] 프로젝트 구조 설정
  ```bash
  cd packages/parser-py
  python -m venv venv
  pip install pyhwp pdfplumber pillow svgwrite
  ```
- [ ] `hwp_to_svg.py` - 표/차트를 SVG로 변환
- [ ] `pdf_processor.py` - PDF 텍스트/이미지 추출
- [ ] OCR 통합 (Tesseract/EasyOCR)
- [ ] PyPI 패키지 준비 (`setup.py`)

#### 1.3 Document Converters (`converters/`)

- [x] HWP → MDX 변환기 (기본 구조)
- [ ] HWPX → MDX 변환기
- [x] PDF → MDX 변환기 (기본 구조)
- [ ] DOCX → MDX 변환기
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

- [ ] Cargo 프로젝트 설정
- [ ] JavaScript 로직 포팅
- [ ] WASM 컴파일 설정 (wasm-bindgen)
- [ ] JavaScript 바인딩
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
- [ ] Watch 모드 (실시간 변환)
- [ ] 배치 처리 지원

---

### Phase 3: Deployment & Integration (Lower Priority)

#### 3.1 npm Package Publishing

- [ ] `beasthan2025` 계정으로 로그인
- [ ] `markdown-media` 패키지 배포
  ```bash
  npm login
  npm publish --access public
  ```
- [ ] `@mdm/parser` 스코프 패키지 배포
- [ ] 버전 관리 전략 수립

#### 3.2 CI/CD Setup (`.github/`)

- [x] GitHub Actions workflow
  - [x] 자동 테스트 (`test.yml`)
  - [x] 자동 빌드 (`build.yml`)
  - [x] 자동 배포 (`publish.yml`)
- [ ] 코드 커버리지 리포트
- [ ] 자동 릴리스 노트

#### 3.3 Documentation

- [ ] API 문서 (JSDoc → HTML)
- [ ] 사용자 가이드
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

## 🔗 References

- [HWP 파일 구조](https://www.hancom.com/etc/hwpDownload.do)
- [OLE Compound File](https://docs.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/)
- [Rust CFB Crate](https://crates.io/crates/cfb)
- [pyhwp Library](https://pypi.org/project/pyhwp/)
- [MDX Official](https://mdxjs.com/)

---

**Author**: seunghan91 (npm: beasthan2025)
