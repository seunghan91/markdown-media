# MDM 프로젝트 Phase 1-3 전체 구현 계획

## 개요
MDM(Markdown+Media) 프로젝트의 핵심 변환 엔진 완성, 기능 확장, 패키지 배포까지 전체 구현 계획

**작업 범위**: 3개 Phase, 약 50개 파일 수정/생성
**예상 기간**: 4주 (집중 개발 시)
**진행 방식**: 문서화 먼저 → 순차 구현

### 확정 설정
- **GitHub**: `seunghan91/markdown-media`
- **npm**: `@mdm/parser` (scoped package)
- **PyPI**: `mdm-parser`
- **Crates.io**: `mdm-core`

---

## Phase 1: 핵심 변환 엔진 (Week 1-2)

### 1.1 Rust DOCX 파서 구현 [P0]
**파일**: `/Users/seunghan/MDM/markdown-media/core/src/docx/parser.rs`

현재: 151줄 스켈레톤
목표: 완전한 DOCX 파싱 (텍스트, 서식, 표, 이미지)

```rust
// 구현할 구조체
pub struct DocxParser { archive, relationships, styles }
pub struct DocxDocument { content, paragraphs, tables, images, metadata }
pub struct Paragraph { text, style, runs }
pub struct TextRun { text, bold, italic, underline }
```

**의존성 추가** (`Cargo.toml`):
```toml
quick-xml = "0.31"
```

### 1.2 HWP 파서 보완 [P1]
**파일**: `/Users/seunghan/MDM/markdown-media/core/src/hwp/record.rs`

추가 구현:
- `CellSpan` 구조체 (병합 셀 지원)
- `ShapeComponent` 구조체 (도형/그림 처리)
- `parse_table_info()` 병합 셀 로직

### 1.3 PDF 파서 개선 [P1]
**파일**: `/Users/seunghan/MDM/markdown-media/core/src/pdf/parser.rs`

추가 구현:
- `is_encrypted()` / `decrypt()` 메서드
- `LayoutElement` 구조체 (레이아웃 보존)

### 1.4 Python OCR 브릿지 [P1]
**새 파일**: `/Users/seunghan/MDM/markdown-media/packages/parser-py/ocr_bridge.py`

```python
class RustOcrBridge:
    def process_rust_output(rust_output_path) -> dict
    def enhance_mdx_with_ocr(mdx_path, ocr_results) -> str
```

### 1.5 테이블 SVG 렌더러 개선 [P2]
**파일**: `/Users/seunghan/MDM/markdown-media/converters/table_to_svg.py`

→ 새 파일: `table_to_svg_enhanced.py`
- 병합 셀 지원 (`rowspan`, `colspan`)
- 스타일링 옵션 확장

### 1.6 차트 PNG 렌더러 [P2]
**새 파일**: `/Users/seunghan/MDM/markdown-media/converters/chart_to_png.py`

```python
class ChartRenderer:
    def render(chart_data, output_path) -> str
    # bar, line, pie, scatter, area 지원
```

### 1.7 E2E 파이프라인 오케스트레이터 [P1]
**새 파일**: `/Users/seunghan/MDM/markdown-media/pipeline/orchestrator.py`

```python
class MdmPipeline:
    def convert(input_path, output_dir, options) -> dict
    # Rust 파싱 → OCR → 테이블 SVG → 차트 PNG → MDX 생성
```

### 1.8 테스트 [P1]
**새 파일들**:
- `/Users/seunghan/MDM/markdown-media/core/tests/parser_tests.rs`
- `/Users/seunghan/MDM/markdown-media/tests/test_pipeline.py`
- `/Users/seunghan/MDM/markdown-media/tests/e2e_test.sh`

---

## Phase 2: 기능 확장 (Week 2-3)

### 2.1 WebP/SVG 포맷 지원 [P1]
**파일**: `/Users/seunghan/MDM/packages/parser-js/src/renderer.js`

추가:
- `renderSVG()` 메서드 (인라인 vs img 선택)
- `renderResponsiveImage()` (picture/source 요소)

**파일**: `/Users/seunghan/MDM/markdown-media/core/src/renderer.rs`

추가:
- `render_svg_to_png()` (resvg 사용)
- `render_to_webp()` (image 크레이트)

**파일**: `/Users/seunghan/MDM/markdown-media/core/src/optimizer.rs`

`optimize()` 메서드 실제 구현 (현재 TODO 상태)

### 2.2 Sidecar 파일 완전 구현 [P1]
**파일**: `/Users/seunghan/MDM/packages/parser-js/src/mdm-loader.js`

추가 기능:
- `parsePresets()` - 글로벌 프리셋 정의
- `parseDefaults()` - 리소스 타입별 기본값
- `parseAliases()` - 리소스 별칭
- `parseCacheConfig()` - 캐시 설정

**새 파일**: `/Users/seunghan/MDM/packages/parser-js/src/presets.js`
- 내장 프리셋 정의 (thumb, small, medium, large, square, widescreen)

### 2.3 Rust WASM 컴파일 [P2]
**파일**: `/Users/seunghan/MDM/markdown-media/core/Cargo.toml`

```toml
[features]
default = ["hwp", "docx", "pdf"]
wasm = ["wasm-bindgen", "console_error_panic_hook"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

**새 파일**: `/Users/seunghan/MDM/packages/parser-wasm/package.json`
- npm 배포용 WASM 래퍼

### 2.4 CLI 도구 완성 [P2]
**파일**: `/Users/seunghan/MDM/markdown-media/core/src/main.rs`

서브커맨드 완성:
```bash
mdm convert --input file.hwp --output ./out
mdm text file.hwp
mdm images file.hwp --output ./media
mdm info file.hwp
```

---

## Phase 3: 패키지 배포 및 CI/CD (Week 3-4)

### 3.1 npm 패키지 준비 (@mdm/parser) [P0]
**파일**: `/Users/seunghan/MDM/packages/parser-js/package.json`

추가:
```json
{
  "main": "dist/index.cjs",
  "module": "dist/index.mjs",
  "types": "dist/index.d.ts",
  "exports": { ... },
  "files": ["dist", "src", "README.md", "LICENSE"],
  "scripts": {
    "build": "rollup -c",
    "prepublishOnly": "npm run build && npm test"
  },
  "repository": {
    "type": "git",
    "url": "https://github.com/seunghan91/markdown-media.git"
  },
  "publishConfig": { "access": "public" }
}
```

**새 파일**: `/Users/seunghan/MDM/packages/parser-js/rollup.config.js`

### 3.2 PyPI 패키지 준비 (mdm-parser) [P0]
**파일**: `/Users/seunghan/MDM/markdown-media/packages/parser-py/pyproject.toml`

추가:
```toml
[project.optional-dependencies]
dev = ["pytest>=7.0.0", "pytest-cov>=4.0.0", "black>=23.0.0", "ruff>=0.1.0"]

[tool.pytest.ini_options]
testpaths = ["tests"]
```

**삭제**: `/Users/seunghan/MDM/packages/parser-py/setup.py` (레거시)

### 3.3 Crates.io 패키지 준비 (mdm-core) [P0]
**파일**: `/Users/seunghan/MDM/markdown-media/core/Cargo.toml`

추가:
```toml
authors = ["seunghan91"]
homepage = "https://github.com/seunghan91/markdown-media"
repository = "https://github.com/seunghan91/markdown-media"
documentation = "https://docs.rs/mdm-core"
keywords = ["markdown", "media", "hwp", "converter"]
categories = ["parsing", "text-processing"]
```

### 3.4 GitHub Actions CI [P0]
**새 파일**: `/Users/seunghan/MDM/.github/workflows/ci.yml`

```yaml
jobs:
  test-js:     # Node 18, 20, 22 매트릭스
  test-py:     # Python 3.8-3.12 매트릭스
  test-rust:   # cargo fmt, clippy, test
  build-wasm:  # wasm-pack build
```

### 3.5 GitHub Actions Release [P1]
**새 파일**: `/Users/seunghan/MDM/.github/workflows/release.yml`

태그 기반 자동 릴리스:
- `v*.*.*` → 모든 패키지
- `js-v*` → npm만
- `py-v*` → PyPI만
- `rs-v*` → Crates.io만

### 3.6 Docker 컨테이너 [P2]
**새 파일**: `/Users/seunghan/MDM/Dockerfile`

멀티스테이지 빌드:
1. Rust 빌드
2. Python 의존성
3. 최종 slim 이미지

### 3.7 통합 테스트 (Spec Tests) [P1]
**새 디렉토리**: `/Users/seunghan/MDM/tests/spec/`

Cross-language 스펙 테스트:
- `basic/` - 기본 이미지 파싱
- `presets/` - 프리셋 테스트
- `sidecar/` - MDM 파일 테스트

---

## 파일 변경 요약

### 신규 생성 파일 (16개)
```
markdown-media/packages/parser-py/ocr_bridge.py
markdown-media/converters/table_to_svg_enhanced.py
markdown-media/converters/chart_to_png.py
markdown-media/pipeline/__init__.py
markdown-media/pipeline/orchestrator.py
markdown-media/core/tests/parser_tests.rs
markdown-media/tests/test_pipeline.py
markdown-media/tests/e2e_test.sh
packages/parser-js/src/presets.js
packages/parser-js/rollup.config.js
packages/parser-wasm/package.json
.github/workflows/ci.yml
.github/workflows/release.yml
Dockerfile
tests/spec/basic/001-simple-image.md
tests/runners/run-js.js
```

### 수정 파일 (12개)
```
markdown-media/core/src/docx/parser.rs      # DOCX 파서 구현
markdown-media/core/src/hwp/record.rs       # 병합셀 지원
markdown-media/core/src/pdf/parser.rs       # 암호화 처리
markdown-media/core/src/renderer.rs         # SVG/WebP 렌더링
markdown-media/core/src/optimizer.rs        # 이미지 최적화
markdown-media/core/src/main.rs             # CLI 완성
markdown-media/core/Cargo.toml              # 의존성 + 메타데이터
packages/parser-js/src/renderer.js          # WebP/SVG 지원
packages/parser-js/src/mdm-loader.js        # Sidecar 완성
packages/parser-js/package.json             # npm 메타데이터
markdown-media/packages/parser-py/pyproject.toml  # dev 의존성
converters/table_to_svg.py                  # 향상 또는 대체
```

---

## 실행 순서

### Week 1
1. [ ] DOCX 파서 구현 (parser.rs)
2. [ ] HWP 병합셀 지원 (record.rs)
3. [ ] PDF 암호화 처리 (pdf/parser.rs)
4. [ ] OCR 브릿지 (ocr_bridge.py)

### Week 2
5. [ ] E2E 파이프라인 (orchestrator.py)
6. [ ] 테이블 SVG 개선 (table_to_svg_enhanced.py)
7. [ ] 차트 PNG 렌더러 (chart_to_png.py)
8. [ ] WebP/SVG 지원 (renderer.js, renderer.rs)

### Week 3
9. [ ] Sidecar 완전 구현 (mdm-loader.js, presets.js)
10. [ ] WASM 컴파일 설정 (Cargo.toml)
11. [ ] CLI 완성 (main.rs)
12. [ ] 패키지 메타데이터 (package.json, Cargo.toml, pyproject.toml)

### Week 4
13. [ ] GitHub Actions CI (ci.yml)
14. [ ] GitHub Actions Release (release.yml)
15. [ ] Docker 컨테이너 (Dockerfile)
16. [ ] 통합 테스트 (tests/spec/)
17. [ ] 패키지 퍼블리시 (npm, PyPI, Crates.io)

---

## 의존성

### Rust (Cargo.toml 추가)
```toml
quick-xml = "0.31"        # DOCX XML 파싱
webp = "0.2"              # WebP 인코딩 (선택)
```

### Python (requirements.txt 추가)
```
matplotlib>=3.8.0         # 차트 렌더링
cairosvg>=2.7.0          # SVG → PNG
```

### Node.js (devDependencies 추가)
```json
"rollup": "^4.0.0"
"@rollup/plugin-node-resolve": "^15.0.0"
```

---

## 리스크 및 완화

| 리스크 | 완화 방안 |
|--------|----------|
| WASM 번들 크기 | wasm-opt 최적화, feature flag |
| 크로스 플랫폼 호환성 | CI 매트릭스 (ubuntu, macos, windows) |
| npm 스코프 권한 | @mdm org 생성, access: public |
| Python 3.8 호환 | typing_extensions 사용 |
