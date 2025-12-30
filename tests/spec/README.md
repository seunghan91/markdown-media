# MDM 통합 테스트 스펙 (Cross-Language Spec Tests)

> 🚧 작업 중 - 병렬 작업 팀 (Phase 3.7)

이 디렉토리는 JavaScript, Python, Rust 모든 파서에서 동일한 결과를 보장하기 위한 **스펙 테스트**를 포함합니다.

## 디렉토리 구조

```
tests/spec/
├── basic/           # 기본 이미지 파싱 테스트
│   ├── 001-simple-image.md
│   ├── 001-simple-image.expected.json
│   └── ...
├── presets/         # 프리셋 테스트
│   ├── 001-size-presets.md
│   └── ...
├── sidecar/         # MDM 사이드카 파일 테스트
│   ├── 001-basic-mdm.md
│   ├── 001-basic-mdm.mdm
│   └── ...
└── README.md
```

## 테스트 형식

각 테스트는 다음 파일들로 구성됩니다:

1. **`{name}.md`** - 입력 마크다운 파일
2. **`{name}.expected.json`** - 예상 파싱 결과 (JSON)
3. **`{name}.mdm`** (선택) - 사이드카 파일 (sidecar 테스트용)
4. **`{name}.assets/`** (선택) - 테스트용 미디어 파일

## 테스트 실행

```bash
# JavaScript 테스트
node tests/runners/run-js.js

# Python 테스트
python tests/runners/run-py.py

# Rust 테스트
cargo test --manifest-path core/Cargo.toml spec_tests

# 전체 테스트
./tests/e2e_test.sh
```

## 테스트 작성 가이드

### 기본 테스트 예시

**`001-simple-image.md`**:
```markdown
# Hello

![alt text](./image.png)
```

**`001-simple-image.expected.json`**:
```json
{
  "resources": {
    "image.png": {
      "type": "image",
      "src": "./image.png",
      "alt": "alt text"
    }
  }
}
```

### 프리셋 테스트 예시

**`001-size-presets.md`**:
```markdown
![thumb](./photo.jpg){preset=thumb}
![large](./photo.jpg){preset=large}
```

### MDM 사이드카 테스트 예시

**`001-basic-mdm.mdm`**:
```yaml
version: "1.0"
media_root: ./assets
resources:
  hero:
    src: hero.jpg
    type: image
    width: 1200
```

## 검증 기준

1. **리소스 파싱** - 모든 미디어 리소스가 올바르게 감지됨
2. **속성 추출** - alt, title, width, height 등 속성이 정확함
3. **프리셋 적용** - 프리셋이 올바르게 병합됨
4. **경로 정규화** - 상대/절대 경로가 일관되게 처리됨
5. **에러 처리** - 잘못된 입력에 대해 적절한 에러 반환
