# MDM 프로젝트 TODO 및 즉시 시작 가능한 작업

## 🚀 즉시 시작 가능한 작업들

### 1. JavaScript Parser MVP 구현 (Phase 1.1)

#### 📁 프로젝트 초기 설정
```bash
# 1. JavaScript 파서 디렉토리 설정
cd packages/parser-js
npm init -y

# 2. 필요한 디렉토리 생성
mkdir -p src/{tokenizer,parser,renderer,media,utils}
mkdir -p test/fixtures

# 3. 기본 파일 생성
touch src/index.js
touch src/tokenizer/{index.js,tokens.js,patterns.js}
touch src/parser/{index.js,ast.js,rules.js}
touch src/renderer/{index.js,html.js}
```

#### 🔧 첫 번째 구현 작업
1. **Token 타입 정의** (`src/tokenizer/tokens.js`)
   - 기본 토큰 타입 enum 생성
   - MDM_OPEN, MDM_CLOSE, FILENAME, ATTRIBUTE 등

2. **정규식 패턴 정의** (`src/tokenizer/patterns.js`)
   - `![[` 와 `]]` 패턴
   - 파일명 패턴
   - 속성 블록 패턴

3. **기본 Tokenizer 클래스** (`src/tokenizer/index.js`)
   - 입력 문자열을 토큰으로 분리하는 기본 로직

### 2. 테스트 환경 구축

#### 📝 첫 번째 테스트 케이스 작성
```javascript
// test/tokenizer.test.js
describe('Tokenizer', () => {
  test('should tokenize simple image syntax', () => {
    const input = '![[image.jpg]]';
    // 테스트 구현
  });
});
```

#### 🧪 테스트 스펙 파일 생성
```bash
# 기본 테스트 케이스 생성
mkdir -p tests/spec/basic
echo '![[test.jpg]]' > tests/spec/basic/001-simple-image.md
echo '<img src="test.jpg">' > tests/spec/basic/001-simple-image.html
```

### 3. 개발 환경 설정

#### 📦 package.json 설정
```json
{
  "name": "@mdm/parser",
  "version": "0.0.1",
  "scripts": {
    "test": "jest",
    "lint": "eslint src/",
    "dev": "nodemon src/index.js"
  },
  "devDependencies": {
    "jest": "^29.0.0",
    "eslint": "^8.0.0",
    "nodemon": "^3.0.0"
  }
}
```

#### 🔧 ESLint 설정
```javascript
// .eslintrc.js
module.exports = {
  env: {
    node: true,
    es2021: true,
    jest: true
  },
  extends: 'eslint:recommended',
  parserOptions: {
    ecmaVersion: 12,
    sourceType: 'module'
  }
};
```

## 📋 전체 TODO 리스트

### Phase 1: JavaScript Parser (MVP)

#### v0.1.0 - 기본 이미지 파싱
- [ ] **Tokenizer 구현**
  - [ ] Token 타입 정의
  - [ ] 정규식 패턴 작성
  - [ ] Tokenizer 클래스 구현
  - [ ] 기본 토큰화 테스트

- [ ] **Parser 구현**
  - [ ] AST 노드 타입 정의
  - [ ] Parser 클래스 구현
  - [ ] MDM 블록 파싱 로직
  - [ ] AST 생성 테스트

- [ ] **Renderer 구현**
  - [ ] HTML 렌더러 클래스
  - [ ] 이미지 렌더링 로직
  - [ ] 속성 처리 (width, height, alt, align, caption)
  - [ ] 렌더링 테스트

- [ ] **통합 테스트**
  - [ ] End-to-end 파싱 테스트
  - [ ] 다양한 속성 조합 테스트
  - [ ] 엣지 케이스 테스트

#### v0.2.0 - 향상된 기능
- [ ] **프리셋 시스템**
  - [ ] Size 프리셋 구현 (thumb, small, medium, large)
  - [ ] Ratio 프리셋 구현 (square, widescreen, portrait)
  - [ ] 프리셋 적용 로직
  - [ ] 프리셋 테스트

- [ ] **포맷 지원 확장**
  - [ ] WebP 지원
  - [ ] SVG 지원
  - [ ] 포맷별 검증 로직

- [ ] **Sidecar 파일 지원**
  - [ ] .mdm 파일 파서
  - [ ] media_root 경로 처리
  - [ ] 메타데이터 관리

### Phase 2: Python Implementation

- [ ] **Python 파서 포팅**
  - [ ] 프로젝트 구조 설정
  - [ ] JavaScript 코드를 Python으로 포팅
  - [ ] Python 특화 최적화

- [ ] **PyPI 패키지 준비**
  - [ ] setup.py 작성
  - [ ] 패키지 메타데이터 설정
  - [ ] 배포 스크립트 준비

### Phase 3: Rust Core

- [ ] **Rust 파서 구현**
  - [ ] Cargo 프로젝트 설정
  - [ ] 핵심 파싱 로직 구현
  - [ ] 성능 최적화

- [ ] **WASM 컴파일**
  - [ ] wasm-bindgen 설정
  - [ ] JavaScript 바인딩
  - [ ] 브라우저 호환성 테스트

### 공통 작업

- [ ] **문서화**
  - [ ] API 문서 작성
  - [ ] 사용 가이드 작성
  - [ ] 예제 코드 작성

- [ ] **CI/CD 설정**
  - [ ] GitHub Actions workflow 작성
  - [ ] 자동 테스트 설정
  - [ ] 자동 배포 설정

- [ ] **커뮤니티**
  - [ ] CONTRIBUTING.md 작성
  - [ ] Issue 템플릿 생성
  - [ ] PR 템플릿 생성

## 🎯 이번 주 목표

### Day 1-2: 기본 구조 설정
1. JavaScript 파서 프로젝트 구조 생성
2. 기본 Tokenizer 구현
3. 첫 번째 테스트 케이스 작성

### Day 3-4: Parser 구현
1. AST 구조 정의
2. 기본 파싱 로직 구현
3. Parser 테스트 작성

### Day 5-6: Renderer 구현
1. HTML 렌더링 로직
2. 속성 처리 구현
3. 통합 테스트

### Day 7: 문서화 및 정리
1. 코드 리팩토링
2. 문서 작성
3. v0.1.0 릴리스 준비

## 🔍 우선순위 작업

### 높음 (High Priority)
1. **기본 `![[]]` 문법 파싱** - MVP의 핵심 기능
2. **이미지 속성 지원** - width, height, alt, caption
3. **기본 테스트 케이스** - 품질 보증의 기초

### 중간 (Medium Priority)
1. **프리셋 시스템** - 사용성 향상
2. **CommonMark 호환성** - 기존 Markdown과의 통합
3. **성능 최적화** - 실사용을 위한 준비

### 낮음 (Low Priority)
1. **플러그인 시스템** - 확장성
2. **스트리밍 파서** - 대용량 문서 처리
3. **실시간 미리보기** - 개발자 경험 향상

## 💡 빠른 시작 스크립트

```bash
#!/bin/bash
# quick-start.sh

echo "Setting up MDM JavaScript Parser..."

# 1. 디렉토리 생성
cd packages/parser-js
mkdir -p src/{tokenizer,parser,renderer,media,utils}
mkdir -p test/fixtures

# 2. 기본 파일 생성
cat > src/index.js << 'EOF'
// MDM Parser Entry Point
export { MDMParser } from './parser';
export { parse } from './api';
EOF

cat > src/tokenizer/tokens.js << 'EOF'
// Token Type Definitions
export const TokenType = {
  TEXT: 'TEXT',
  MDM_OPEN: 'MDM_OPEN',
  MDM_CLOSE: 'MDM_CLOSE',
  FILENAME: 'FILENAME',
  EOF: 'EOF'
};
EOF

# 3. package.json 생성
cat > package.json << 'EOF'
{
  "name": "@mdm/parser",
  "version": "0.0.1",
  "description": "MDM (Markdown+Media) parser for JavaScript",
  "main": "src/index.js",
  "scripts": {
    "test": "jest",
    "dev": "node src/index.js"
  },
  "keywords": ["markdown", "parser", "media"],
  "license": "MIT"
}
EOF

# 4. 의존성 설치
npm install --save-dev jest eslint

echo "Setup complete! You can now start implementing the tokenizer."
```

## 📊 진행 상황 추적

### 완료된 작업 ✅
- [x] 프로젝트 계획 수립
- [x] 구현 가이드 작성
- [x] 테스트 전략 수립

### 진행 중 🔄
- [ ] JavaScript 파서 초기 구현

### 대기 중 ⏳
- [ ] Python 파서 포팅
- [ ] Rust 코어 구현
- [ ] Playground 개발

## 🤝 기여 가이드라인

1. **브랜치 전략**
   - `main`: 안정된 릴리스
   - `develop`: 개발 브랜치
   - `feature/*`: 기능 개발
   - `fix/*`: 버그 수정

2. **커밋 메시지 규칙**
   - `feat:` 새로운 기능
   - `fix:` 버그 수정
   - `docs:` 문서 수정
   - `test:` 테스트 추가/수정
   - `refactor:` 코드 리팩토링

3. **PR 체크리스트**
   - [ ] 테스트 통과
   - [ ] 문서 업데이트
   - [ ] 코드 리뷰 완료