# MDM 테스트 전략 및 크로스 언어 호환성

## 🎯 목표

모든 MDM 파서 구현체(JavaScript, Python, Rust)가 동일한 출력을 생성하도록 보장하는 포괄적인 테스트 프레임워크를 구축합니다.

## 🏗️ 테스트 구조

```
tests/
├── spec/                    # 언어 중립적 스펙 테스트
│   ├── basic/              # 기본 기능 테스트
│   │   ├── 001-simple-image.md
│   │   ├── 001-simple-image.html
│   │   ├── 002-image-with-attrs.md
│   │   └── 002-image-with-attrs.html
│   ├── advanced/           # 고급 기능 테스트
│   │   ├── 101-presets.md
│   │   └── 101-presets.html
│   ├── edge-cases/         # 엣지 케이스
│   │   ├── 201-special-chars.md
│   │   └── 201-special-chars.html
│   └── compatibility/      # CommonMark 호환성
│       ├── 301-mixed-content.md
│       └── 301-mixed-content.html
├── fixtures/               # 테스트용 미디어 파일
│   ├── images/
│   ├── videos/
│   └── audio/
├── runners/                # 언어별 테스트 러너
│   ├── run-js.js
│   ├── run-py.py
│   └── run-rs.sh
└── results/               # 테스트 결과 저장
    └── compatibility-matrix.json
```

## 📝 스펙 테스트 형식

### 입력 파일 (`.md`)

```markdown
<!-- tests/spec/basic/001-simple-image.md -->
# Test: Simple Image

This is a paragraph with an image:

![[test-image.jpg]]

End of test.
```

### 예상 출력 파일 (`.html`)

```html
<!-- tests/spec/basic/001-simple-image.html -->
<h1>Test: Simple Image</h1>
<p>This is a paragraph with an image:</p>
<p><img src="test-image.jpg"></p>
<p>End of test.</p>
```

### 메타데이터 파일 (`.json`)

```json
{
  "test_id": "001-simple-image",
  "category": "basic",
  "description": "Tests simple image embedding without attributes",
  "features": ["image", "basic-syntax"],
  "priority": "high"
}
```

## 🧪 테스트 케이스 카테고리

### 1. 기본 기능 테스트 (Basic)

```yaml
001-simple-image:
  input: "![[image.jpg]]"
  output: '<img src="image.jpg">'

002-image-with-width:
  input: "![[image.jpg]{width=500}]]"
  output: '<img src="image.jpg" width="500">'

003-image-with-caption:
  input: "![[image.jpg]{caption=\"My Photo\"}]]"
  output: '<figure><img src="image.jpg"><figcaption>My Photo</figcaption></figure>'

004-image-with-align:
  input: "![[image.jpg]{align=center}]]"
  output: '<img src="image.jpg" class="align-center">'

005-multiple-attributes:
  input: "![[photo.png]{width=300 height=200 alt=\"Profile\"}]]"
  output: '<img src="photo.png" width="300" height="200" alt="Profile">'
```

### 2. 프리셋 테스트 (Presets)

```yaml
101-size-preset-thumb:
  input: "![[image.jpg]{size=thumb}]]"
  output: '<img src="image.jpg" width="150px">'

102-size-preset-medium:
  input: "![[image.jpg]{size=medium}]]"
  output: '<img src="image.jpg" width="768px">'

103-ratio-preset-widescreen:
  input: "![[image.jpg]{ratio=widescreen}]]"
  output: '<img src="image.jpg" style="aspect-ratio: 16/9; object-fit: cover;">'

104-combined-presets:
  input: "![[image.jpg]{size=small ratio=square}]]"
  output: '<img src="image.jpg" width="480px" style="aspect-ratio: 1/1; object-fit: cover;">'
```

### 3. 엣지 케이스 (Edge Cases)

```yaml
201-special-characters-in-filename:
  input: "![[my-file (1).jpg]]"
  output: '<img src="my-file (1).jpg">'

202-unicode-in-attributes:
  input: "![[image.jpg]{alt=\"한글 테스트\"}]]"
  output: '<img src="image.jpg" alt="한글 테스트">'

203-empty-attributes:
  input: "![[image.jpg]{}]]"
  output: '<img src="image.jpg">'

204-malformed-syntax:
  input: "![[image.jpg}"
  output: '<p>![[image.jpg}</p>'

205-nested-brackets:
  input: "![[folder/[special]/image.jpg]]"
  output: '<img src="folder/[special]/image.jpg">'
```

### 4. CommonMark 호환성 (Compatibility)

```yaml
301-inline-with-text:
  input: "Text before ![[image.jpg]] text after"
  output: '<p>Text before <img src="image.jpg"> text after</p>'

302-inside-lists:
  input: |
    - Item 1
    - ![[image.jpg]]
    - Item 3
  output: |
    <ul>
    <li>Item 1</li>
    <li><img src="image.jpg"></li>
    <li>Item 3</li>
    </ul>

303-inside-blockquote:
  input: |
    > Quote text
    > ![[image.jpg]]
  output: |
    <blockquote>
    <p>Quote text</p>
    <p><img src="image.jpg"></p>
    </blockquote>
```

## 🔄 크로스 언어 테스트 러너

### JavaScript 테스트 러너

```javascript
// tests/runners/run-js.js
const { MDMParser } = require('@mdm/parser');
const fs = require('fs');
const path = require('path');

async function runTests() {
  const parser = new MDMParser();
  const specDir = path.join(__dirname, '..', 'spec');
  const results = [];

  // 모든 .md 파일 찾기
  const testFiles = findTestFiles(specDir);

  for (const testFile of testFiles) {
    const input = fs.readFileSync(testFile, 'utf8');
    const expectedFile = testFile.replace('.md', '.html');
    const expected = fs.readFileSync(expectedFile, 'utf8').trim();

    try {
      const actual = parser.parse(input).trim();
      const passed = actual === expected;
      
      results.push({
        test: path.basename(testFile),
        passed,
        actual,
        expected
      });
    } catch (error) {
      results.push({
        test: path.basename(testFile),
        passed: false,
        error: error.message
      });
    }
  }

  // 결과 저장
  saveResults('javascript', results);
  return results;
}
```

### Python 테스트 러너

```python
# tests/runners/run-py.py
import os
import json
from pathlib import Path
from mdm import MDMParser

def run_tests():
    parser = MDMParser()
    spec_dir = Path(__file__).parent.parent / 'spec'
    results = []

    # 모든 .md 파일 찾기
    for test_file in spec_dir.rglob('*.md'):
        with open(test_file, 'r', encoding='utf-8') as f:
            input_text = f.read()
        
        expected_file = test_file.with_suffix('.html')
        with open(expected_file, 'r', encoding='utf-8') as f:
            expected = f.read().strip()
        
        try:
            actual = parser.parse(input_text).strip()
            passed = actual == expected
            
            results.append({
                'test': test_file.name,
                'passed': passed,
                'actual': actual,
                'expected': expected
            })
        except Exception as e:
            results.append({
                'test': test_file.name,
                'passed': False,
                'error': str(e)
            })
    
    # 결과 저장
    save_results('python', results)
    return results
```

### 통합 테스트 실행 스크립트

```bash
#!/bin/bash
# tests/run-all.sh

echo "Running MDM Cross-Language Tests..."
echo "=================================="

# JavaScript 테스트
echo "Running JavaScript tests..."
node runners/run-js.js

# Python 테스트
echo "Running Python tests..."
python runners/run-py.py

# Rust 테스트
echo "Running Rust tests..."
./runners/run-rs.sh

# 결과 비교
echo "Comparing results..."
node tools/compare-results.js

echo "=================================="
echo "Test run complete!"
```

## 📊 호환성 매트릭스

테스트 결과를 시각적으로 보여주는 호환성 매트릭스:

```javascript
// tools/generate-compatibility-matrix.js
function generateMatrix(results) {
  const matrix = {
    timestamp: new Date().toISOString(),
    languages: ['javascript', 'python', 'rust'],
    tests: {},
    summary: {
      total: 0,
      passed: 0,
      failed: 0,
      compatibility: 0
    }
  };

  // 각 테스트에 대해 언어별 결과 수집
  for (const testName of getAllTestNames()) {
    matrix.tests[testName] = {
      javascript: getTestResult('javascript', testName),
      python: getTestResult('python', testName),
      rust: getTestResult('rust', testName),
      compatible: areAllPassing(testName)
    };
  }

  // 요약 통계 계산
  calculateSummary(matrix);
  
  return matrix;
}
```

### 호환성 리포트 예시

```markdown
# MDM Cross-Language Compatibility Report

Generated: 2025-01-21T10:00:00Z

## Summary
- Total Tests: 45
- All Passing: 42 (93.3%)
- Incompatible: 3 (6.7%)

## Detailed Results

| Test | JavaScript | Python | Rust | Compatible |
|------|------------|--------|------|------------|
| 001-simple-image | ✅ | ✅ | ✅ | ✅ |
| 002-image-with-attrs | ✅ | ✅ | ✅ | ✅ |
| 201-special-chars | ✅ | ❌ | ✅ | ❌ |
| 202-unicode-attrs | ✅ | ✅ | ❌ | ❌ |

## Failed Tests Details

### Test: 201-special-chars
- **JavaScript**: PASS
- **Python**: FAIL - Incorrect escaping of parentheses
- **Rust**: PASS

### Test: 202-unicode-attrs
- **JavaScript**: PASS
- **Python**: PASS
- **Rust**: FAIL - UTF-8 encoding issue
```

## 🚀 CI/CD 통합

### GitHub Actions Workflow

```yaml
# .github/workflows/cross-language-tests.yml
name: Cross-Language Compatibility Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Setup Node.js
      uses: actions/setup-node@v3
      with:
        node-version: '18'
    
    - name: Setup Python
      uses: actions/setup-python@v4
      with:
        python-version: '3.10'
    
    - name: Setup Rust
      uses: actions-rust-lang/setup-rust-toolchain@v1
    
    - name: Install dependencies
      run: |
        npm install
        pip install -e packages/parser-py
        cd packages/parser-rs && cargo build
    
    - name: Run tests
      run: ./tests/run-all.sh
    
    - name: Upload compatibility matrix
      uses: actions/upload-artifact@v3
      with:
        name: compatibility-matrix
        path: tests/results/compatibility-matrix.json
    
    - name: Comment PR with results
      if: github.event_name == 'pull_request'
      uses: actions/github-script@v6
      with:
        script: |
          const results = require('./tests/results/summary.json');
          const comment = generatePRComment(results);
          github.rest.issues.createComment({
            issue_number: context.issue.number,
            owner: context.repo.owner,
            repo: context.repo.repo,
            body: comment
          });
```

## 🔍 디버깅 도구

### 테스트 실패 분석기

```javascript
// tools/analyze-failures.js
function analyzeFailure(testName, language) {
  const result = getTestResult(language, testName);
  
  console.log(`\n=== Analyzing ${testName} for ${language} ===`);
  console.log('Expected:', result.expected);
  console.log('Actual:', result.actual);
  
  // 차이점 하이라이트
  const diff = generateDiff(result.expected, result.actual);
  console.log('\nDifferences:');
  console.log(diff);
  
  // AST 비교 (디버깅용)
  if (language === 'javascript') {
    const expectedAST = parseToAST(getTestInput(testName));
    console.log('\nAST:', JSON.stringify(expectedAST, null, 2));
  }
}
```

## 📈 성능 벤치마크

### 벤치마크 테스트 세트

```javascript
// tests/benchmarks/performance.js
const benchmarks = [
  {
    name: 'Small document (1KB)',
    file: 'small.md',
    iterations: 10000
  },
  {
    name: 'Medium document (100KB)',
    file: 'medium.md',
    iterations: 100
  },
  {
    name: 'Large document (1MB)',
    file: 'large.md',
    iterations: 10
  }
];

async function runBenchmarks() {
  for (const benchmark of benchmarks) {
    console.log(`Running: ${benchmark.name}`);
    
    const results = {
      javascript: await benchmarkJS(benchmark),
      python: await benchmarkPython(benchmark),
      rust: await benchmarkRust(benchmark)
    };
    
    displayResults(results);
  }
}
```