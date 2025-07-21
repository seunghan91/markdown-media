# MDM Implementation Guide

이 문서는 MDM 프로젝트의 구체적인 구현 방안과 기술적 세부사항을 정리합니다.

## 📋 프로젝트 개요

MDM (Markdown+Media)은 마크다운에서 미디어를 효과적으로 제어할 수 있는 확장 문법을 제공하는 프로젝트입니다.

### 핵심 목표
- CommonMark와 100% 호환성 유지
- 직관적인 `![[]]` 문법 제공
- 고성능 파싱 구현
- 다양한 프로그래밍 언어 지원 (JavaScript, Python, Rust)

## 🏗️ 아키텍처 설계

### 1. Parser 구조

```
┌─────────────────┐
│   Input (.md)   │
└────────┬────────┘
         │
┌────────▼────────┐
│    Tokenizer    │ ← 입력 텍스트를 토큰으로 분리
└────────┬────────┘
         │
┌────────▼────────┐
│  Parser (AST)   │ ← 토큰을 추상 구문 트리로 변환
└────────┬────────┘
         │
┌────────▼────────┐
│   Transformer   │ ← AST를 목표 포맷으로 변환
└────────┬────────┘
         │
┌────────▼────────┐
│  Output (HTML)  │
└─────────────────┘
```

### 2. 모듈 구성

- **Core Module**: 핵심 파싱 로직
- **Media Module**: 미디어 처리 전용 모듈
- **Sidecar Module**: `.mdm` 파일 처리
- **Renderer Module**: HTML 렌더링
- **Plugin System**: 확장 가능한 플러그인 아키텍처

## 🔧 구현 세부사항

### Phase 1: JavaScript Parser (MVP)

#### 1.1 기본 이미지 파싱 구현

**주요 컴포넌트:**
- `tokenizer.js`: 텍스트를 토큰으로 분리
- `parser.js`: 토큰을 AST로 변환
- `renderer.js`: AST를 HTML로 렌더링

**구현할 속성:**
```javascript
const imageAttributes = {
  width: 'string|number',
  height: 'string|number',
  align: 'left|center|right',
  alt: 'string',
  caption: 'string'
};
```

**파싱 예제:**
```markdown
![[image.jpg]{width=500 align=center alt="Example" caption="예제 이미지"}]]
```

#### 1.2 이미지 프리셋 시스템

**Size 프리셋:**
```javascript
const sizePresets = {
  thumb: { width: '150px' },
  small: { width: '480px' },
  medium: { width: '768px' },
  large: { width: '1024px' }
};
```

**Ratio 프리셋:**
```javascript
const ratioPresets = {
  square: { aspectRatio: '1/1' },
  standard: { aspectRatio: '4/3' },
  widescreen: { aspectRatio: '16/9' },
  portrait: { aspectRatio: '3/4' },
  story: { aspectRatio: '9/16' }
};
```

#### 1.3 Sidecar 파일 처리

**.mdm 파일 형식:**
```yaml
media_root: ./assets/images
version: 1.0
metadata:
  created: 2024-01-01
  author: MDM Team
```

### Phase 2: Python 구현

#### 2.1 Python Parser 구조

```python
class MDMParser:
    def __init__(self):
        self.tokenizer = Tokenizer()
        self.ast_builder = ASTBuilder()
        self.renderer = HTMLRenderer()
    
    def parse(self, markdown_text):
        tokens = self.tokenizer.tokenize(markdown_text)
        ast = self.ast_builder.build(tokens)
        return self.renderer.render(ast)
```

#### 2.2 PyPI 패키지 구성

```
mdm-parser/
├── mdm/
│   ├── __init__.py
│   ├── parser.py
│   ├── tokenizer.py
│   ├── renderer.py
│   └── media/
│       ├── __init__.py
│       ├── image.py
│       └── presets.py
├── tests/
├── setup.py
└── README.md
```

### Phase 3: Rust Core 구현

#### 3.1 Rust 모듈 구조

```rust
// src/lib.rs
pub mod tokenizer;
pub mod parser;
pub mod renderer;
pub mod media;

pub struct MDMParser {
    // Parser implementation
}

impl MDMParser {
    pub fn parse(&self, input: &str) -> Result<String, ParseError> {
        // Parsing logic
    }
}
```

#### 3.2 WASM 바인딩

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse_mdm(input: &str) -> Result<String, JsValue> {
    // WASM-compatible parsing
}
```

## 📁 프로젝트 구조

```
mdm/
├── packages/
│   ├── parser-js/      # JavaScript 구현
│   ├── parser-py/      # Python 구현
│   └── parser-rs/      # Rust 구현
├── tests/              # 공통 테스트 케이스
│   ├── fixtures/       # 테스트 입력 파일
│   └── expected/       # 예상 출력 파일
├── playground/         # 웹 기반 데모
├── docs/              # 문서
└── tools/             # 개발 도구
```

## 🧪 테스트 전략

### 1. 단위 테스트
- 각 언어별 개별 테스트
- 토크나이저, 파서, 렌더러 각각 테스트

### 2. 통합 테스트
- End-to-end 파싱 테스트
- 크로스 언어 호환성 테스트

### 3. 성능 테스트
- 대용량 문서 파싱 벤치마크
- 메모리 사용량 측정

## 🚀 배포 계획

### JavaScript (NPM)
```bash
npm publish @mdm/parser
```

### Python (PyPI)
```bash
python -m build
twine upload dist/*
```

### Rust (Crates.io)
```bash
cargo publish
```

## 📊 성능 목표

- 1MB 문서 파싱: < 100ms
- 메모리 사용량: < 50MB
- CommonMark 호환성: 100%

## 🔐 보안 고려사항

- XSS 방지를 위한 HTML 이스케이핑
- 파일 경로 검증
- 악성 입력 방어

## 📈 향후 확장 계획

1. **비디오/오디오 지원**
2. **갤러리 기능**
3. **플러그인 시스템**
4. **실시간 협업 기능**