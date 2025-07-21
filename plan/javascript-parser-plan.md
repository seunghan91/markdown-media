# JavaScript Parser 구현 계획

## 📌 개요

MDM JavaScript 파서는 프로젝트의 첫 번째 구현체로, 다른 언어 구현의 참조 모델이 됩니다.

## 🏗️ 프로젝트 구조

```
packages/parser-js/
├── src/
│   ├── index.js            # 진입점
│   ├── tokenizer/
│   │   ├── index.js        # 토크나이저 메인
│   │   ├── patterns.js     # 정규식 패턴
│   │   └── tokens.js       # 토큰 타입 정의
│   ├── parser/
│   │   ├── index.js        # 파서 메인
│   │   ├── ast.js          # AST 노드 정의
│   │   └── rules.js        # 파싱 규칙
│   ├── renderer/
│   │   ├── index.js        # 렌더러 메인
│   │   ├── html.js         # HTML 렌더링
│   │   └── helpers.js      # 렌더링 헬퍼
│   ├── media/
│   │   ├── image.js        # 이미지 처리
│   │   ├── presets.js      # 프리셋 정의
│   │   └── attributes.js   # 속성 파싱
│   └── utils/
│       ├── errors.js       # 에러 처리
│       └── validation.js   # 유효성 검사
├── test/
│   ├── tokenizer.test.js
│   ├── parser.test.js
│   ├── renderer.test.js
│   └── fixtures/
├── package.json
├── .eslintrc.js
└── README.md
```

## 🔧 구현 상세

### 1. Tokenizer 구현

#### Token 타입 정의
```javascript
// src/tokenizer/tokens.js
export const TokenType = {
  TEXT: 'TEXT',
  MDM_OPEN: 'MDM_OPEN',      // ![[
  MDM_CLOSE: 'MDM_CLOSE',    // ]]
  ATTR_OPEN: 'ATTR_OPEN',    // {
  ATTR_CLOSE: 'ATTR_CLOSE',  // }
  FILENAME: 'FILENAME',
  ATTRIBUTE: 'ATTRIBUTE',
  NEWLINE: 'NEWLINE',
  EOF: 'EOF'
};
```

#### 정규식 패턴
```javascript
// src/tokenizer/patterns.js
export const patterns = {
  mdmBlock: /^!\[\[([^\]]+)\]\](\{[^}]+\})?/,
  mdmOpen: /^!\[\[/,
  mdmClose: /^\]\]/,
  attrBlock: /^\{([^}]+)\}/,
  filename: /^[^\]{\s]+/,
  attribute: /^(\w+)=([^}\s]+)/
};
```

#### Tokenizer 클래스
```javascript
// src/tokenizer/index.js
export class Tokenizer {
  constructor(input) {
    this.input = input;
    this.position = 0;
    this.tokens = [];
  }

  tokenize() {
    while (!this.isEOF()) {
      this.skipWhitespace();
      
      if (this.match(patterns.mdmOpen)) {
        this.addToken(TokenType.MDM_OPEN);
        this.parseMediaBlock();
      } else {
        this.parseText();
      }
    }
    
    this.addToken(TokenType.EOF);
    return this.tokens;
  }

  parseMediaBlock() {
    // 미디어 블록 파싱 로직
  }
}
```

### 2. Parser (AST Builder) 구현

#### AST 노드 정의
```javascript
// src/parser/ast.js
export class ASTNode {
  constructor(type, props = {}) {
    this.type = type;
    this.props = props;
    this.children = [];
  }
}

export const NodeType = {
  DOCUMENT: 'document',
  PARAGRAPH: 'paragraph',
  TEXT: 'text',
  MDM_IMAGE: 'mdm_image',
  MDM_VIDEO: 'mdm_video',
  MDM_AUDIO: 'mdm_audio'
};
```

#### Parser 클래스
```javascript
// src/parser/index.js
export class Parser {
  constructor(tokens) {
    this.tokens = tokens;
    this.current = 0;
  }

  parse() {
    const root = new ASTNode(NodeType.DOCUMENT);
    
    while (!this.isAtEnd()) {
      const node = this.parseNode();
      if (node) root.children.push(node);
    }
    
    return root;
  }

  parseNode() {
    const token = this.peek();
    
    switch (token.type) {
      case TokenType.MDM_OPEN:
        return this.parseMDMBlock();
      case TokenType.TEXT:
        return this.parseText();
      default:
        this.advance();
        return null;
    }
  }

  parseMDMBlock() {
    this.consume(TokenType.MDM_OPEN);
    const filename = this.consume(TokenType.FILENAME).value;
    
    let attributes = {};
    if (this.check(TokenType.ATTR_OPEN)) {
      attributes = this.parseAttributes();
    }
    
    this.consume(TokenType.MDM_CLOSE);
    
    return this.createMediaNode(filename, attributes);
  }
}
```

### 3. Renderer 구현

#### HTML Renderer
```javascript
// src/renderer/html.js
export class HTMLRenderer {
  render(ast) {
    return this.renderNode(ast);
  }

  renderNode(node) {
    switch (node.type) {
      case NodeType.DOCUMENT:
        return this.renderChildren(node);
      case NodeType.MDM_IMAGE:
        return this.renderImage(node);
      case NodeType.TEXT:
        return this.escapeHTML(node.value);
      default:
        return '';
    }
  }

  renderImage(node) {
    const { filename, attributes } = node.props;
    const attrs = this.buildImageAttributes(attributes);
    
    let html = `<img src="${filename}"${attrs}>`;
    
    if (attributes.caption) {
      html = `<figure>${html}<figcaption>${attributes.caption}</figcaption></figure>`;
    }
    
    return html;
  }

  buildImageAttributes(attributes) {
    const attrs = [];
    
    if (attributes.width) attrs.push(`width="${attributes.width}"`);
    if (attributes.height) attrs.push(`height="${attributes.height}"`);
    if (attributes.alt) attrs.push(`alt="${attributes.alt}"`);
    if (attributes.align) attrs.push(`class="align-${attributes.align}"`);
    
    return attrs.length > 0 ? ' ' + attrs.join(' ') : '';
  }
}
```

### 4. 이미지 프리셋 시스템

```javascript
// src/media/presets.js
export const sizePresets = {
  thumb: { width: '150px' },
  small: { width: '480px' },
  medium: { width: '768px' },
  large: { width: '1024px' },
  full: { width: '100%' }
};

export const ratioPresets = {
  square: { 
    aspectRatio: '1/1',
    objectFit: 'cover'
  },
  standard: { 
    aspectRatio: '4/3',
    objectFit: 'cover'
  },
  widescreen: { 
    aspectRatio: '16/9',
    objectFit: 'cover'
  },
  portrait: { 
    aspectRatio: '3/4',
    objectFit: 'cover'
  },
  story: { 
    aspectRatio: '9/16',
    objectFit: 'cover'
  }
};

export function applyPresets(attributes) {
  const processed = { ...attributes };
  
  if (attributes.size && sizePresets[attributes.size]) {
    Object.assign(processed, sizePresets[attributes.size]);
    delete processed.size;
  }
  
  if (attributes.ratio && ratioPresets[attributes.ratio]) {
    Object.assign(processed, ratioPresets[attributes.ratio]);
    delete processed.ratio;
  }
  
  return processed;
}
```

### 5. API 설계

```javascript
// src/index.js
import { Tokenizer } from './tokenizer';
import { Parser } from './parser';
import { HTMLRenderer } from './renderer/html';

export class MDMParser {
  constructor(options = {}) {
    this.options = {
      mediaRoot: './',
      enablePresets: true,
      ...options
    };
  }

  parse(markdown) {
    // 토큰화
    const tokenizer = new Tokenizer(markdown);
    const tokens = tokenizer.tokenize();
    
    // 파싱
    const parser = new Parser(tokens);
    const ast = parser.parse();
    
    // 렌더링
    const renderer = new HTMLRenderer(this.options);
    return renderer.render(ast);
  }

  parseToAST(markdown) {
    const tokenizer = new Tokenizer(markdown);
    const tokens = tokenizer.tokenize();
    const parser = new Parser(tokens);
    return parser.parse();
  }
}

// 편의 함수
export function parse(markdown, options) {
  const parser = new MDMParser(options);
  return parser.parse(markdown);
}
```

## 🧪 테스트 계획

### 단위 테스트

```javascript
// test/tokenizer.test.js
describe('Tokenizer', () => {
  test('should tokenize simple MDM block', () => {
    const input = '![[image.jpg]]';
    const tokenizer = new Tokenizer(input);
    const tokens = tokenizer.tokenize();
    
    expect(tokens).toEqual([
      { type: TokenType.MDM_OPEN, value: '![[' },
      { type: TokenType.FILENAME, value: 'image.jpg' },
      { type: TokenType.MDM_CLOSE, value: ']]' },
      { type: TokenType.EOF }
    ]);
  });

  test('should tokenize MDM with attributes', () => {
    const input = '![[photo.png]{width=500 align=center}]]';
    // 테스트 구현
  });
});
```

### 통합 테스트

```javascript
// test/integration.test.js
describe('MDM Parser Integration', () => {
  test('should parse and render image with caption', () => {
    const input = '![[hero.jpg]{alt="Hero Image" caption="Welcome"}]]';
    const expected = '<figure><img src="hero.jpg" alt="Hero Image"><figcaption>Welcome</figcaption></figure>';
    
    const result = parse(input);
    expect(result).toBe(expected);
  });
});
```

## 📦 패키지 설정

```json
// package.json
{
  "name": "@mdm/parser",
  "version": "0.1.0",
  "description": "MDM (Markdown+Media) parser for JavaScript",
  "main": "dist/index.js",
  "module": "dist/index.esm.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "rollup -c",
    "test": "jest",
    "lint": "eslint src/",
    "prepublishOnly": "npm run build && npm test"
  },
  "keywords": ["markdown", "parser", "media", "mdm"],
  "license": "MIT",
  "devDependencies": {
    "@rollup/plugin-node-resolve": "^15.0.0",
    "@rollup/plugin-commonjs": "^25.0.0",
    "eslint": "^8.0.0",
    "jest": "^29.0.0",
    "rollup": "^3.0.0"
  }
}
```

## 🚀 배포 단계

### v0.1.0 (MVP)
- [x] 기본 `![[]]` 문법 파싱
- [x] 이미지 속성 지원 (width, height, align, alt, caption)
- [x] 기본 이미지 포맷 지원 (jpg, jpeg, png, gif)
- [ ] 기본 테스트 커버리지 80% 이상

### v0.2.0
- [ ] 프리셋 시스템 구현
- [ ] 현대적 이미지 포맷 지원 (webp, svg)
- [ ] Sidecar 파일 (.mdm) 지원
- [ ] CommonMark 호환성 테스트

### v0.3.0
- [ ] 플러그인 시스템
- [ ] 성능 최적화
- [ ] 스트리밍 파서 모드

## 🔍 성능 고려사항

1. **메모리 효율성**
   - 스트리밍 파싱 옵션 제공
   - 큰 문서를 위한 청크 단위 처리

2. **파싱 속도**
   - 정규식 최적화
   - 캐싱 메커니즘

3. **번들 크기**
   - Tree-shaking 지원
   - 최소 의존성