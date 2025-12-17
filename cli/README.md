# MDM CLI 사용 가이드

## 설치

```bash
npm install -g @mdm/cli
```

## 명령어 목록

```bash
mdm --help
```

### 1. convert - 문서 변환

단일 파일을 MDM 번들로 변환합니다.

**사용법:**

```bash
mdm convert <input> [options]
```

**옵션:**

- `-o, --output <dir>` - 출력 디렉토리 (기본값: `./output`)
- `-f, --format <type>` - 출력 포맷 (기본값: `mdx`)

**지원 포맷:**

- `.hwp` - 한글 문서
- `.hwpx` - 한글 문서 (오픈 포맷)
- `.pdf` - PDF 문서
- `.docx` - Microsoft Word
- `.html`, `.htm` - 웹페이지

**예시:**

```bash
# HWP 파일 변환
mdm convert document.hwp -o ./converted/

# PDF 파일 변환
mdm convert report.pdf -o ./reports/

# HTML 파일 변환 (블로그 포스트)
mdm convert blog_post.html -o ./posts/

# DOCX 파일 변환
mdm convert letter.docx -o ./letters/
```

---

### 2. validate - 번들 검증

MDM 번들의 구조를 검증합니다.

**사용법:**

```bash
mdm validate <path> [options]
```

**옵션:**

- `-v, --verbose` - 상세 출력

**예시:**

```bash
# 번들 검증
mdm validate ./output/my-document/

# 상세 검증
mdm validate ./output/my-document/ --verbose
```

**검증 항목:**

- ✅ .mdx 파일 존재
- ✅ .mdm 메타데이터 파일 검증
- ✅ 참조된 이미지 파일 존재 확인
- ✅ MDM 구문 (`![[]]`) 유효성

---

### 3. serve - 로컬 서버

MDM 번들을 미리보기할 수 있는 로컬 서버를 실행합니다.

**사용법:**

```bash
mdm serve [path] [options]
```

**옵션:**

- `-p, --port <number>` - 포트 번호 (기본값: `3000`)
- `--open` - 브라우저 자동 열기

**예시:**

```bash
# 현재 디렉토리 서빙
mdm serve

# 특정 디렉토리 서빙
mdm serve ./output/

# 포트 지정 + 브라우저 열기
mdm serve ./output/ --port 8080 --open
```

---

### 4. watch - 실시간 변환

파일이나 디렉토리를 감시하고 변경 시 자동 변환합니다.

**사용법:**

```bash
mdm watch <path> [options]
```

**옵션:**

- `-o, --output <dir>` - 출력 디렉토리 (기본값: `./output`)

**예시:**

```bash
# 단일 파일 감시
mdm watch document.hwp -o ./output/

# 디렉토리 감시
mdm watch ./docs/ -o ./converted/
```

**동작:**

1. 지정된 경로의 파일 변경을 감시
2. 지원되는 포맷의 파일이 변경되면 자동 변환
3. Ctrl+C로 종료

---

### 5. batch - 일괄 변환

여러 파일을 한 번에 변환합니다.

**사용법:**

```bash
mdm batch <pattern> [options]
```

**옵션:**

- `-o, --output <dir>` - 출력 디렉토리 (기본값: `./output`)

**예시:**

```bash
# 모든 HWP 파일 변환
mdm batch "*.hwp" -o ./converted/

# 특정 디렉토리의 모든 PDF
mdm batch "reports/*.pdf" -o ./pdf-converted/

# 중첩 디렉토리 포함
mdm batch "docs/**/*.hwp" -o ./all-docs/

# 여러 포맷
mdm batch "*.{hwp,pdf,docx}" -o ./output/
```

---

## 실제 사용 시나리오

### 1. 네이버 블로그 백업

```bash
# 1. 브라우저에서 블로그 포스트 HTML 저장 (Ctrl+S)
# 2. 변환
mdm convert my_blog_post.html -o ./blog-backup/

# 3. 결과 확인
ls ./blog-backup/
# index.mdx  index.mdm  assets/
```

### 2. 정부 문서 디지털화

```bash
# HWP 문서 일괄 변환
mdm batch "government-docs/*.hwp" -o ./digitized/

# 변환된 문서 확인
mdm validate ./digitized/document1/
```

### 3. PDF 보고서 아카이빙

```bash
# 실시간 변환 모드
mdm watch ./incoming-reports/ -o ./archive/

# 새 PDF가 추가될 때마다 자동 변환됨
```

### 4. 워드 문서 마이그레이션

```bash
# DOCX를 Markdown으로 변환
mdm batch "**/*.docx" -o ./markdown-docs/

# 로컬에서 미리보기
mdm serve ./markdown-docs/ --port 8080 --open
```

---

## 출력 구조

모든 변환 결과는 동일한 구조로 생성됩니다:

```
output/
├── document.mdx          # Markdown 본문
│   ├── 제목 (YAML front matter)
│   ├── 메타데이터
│   ├── 본문 텍스트
│   └── ![[]] 미디어 참조
│
├── document.mdm          # 리소스 메타데이터 (JSON)
│   ├── version
│   ├── resources (이미지/표 목록)
│   └── metadata (출처, 원본 포맷 등)
│
└── assets/               # 미디어 파일
    ├── image_1.jpg       # 추출/다운로드된 이미지
    ├── image_2.png
    └── table_1.svg       # SVG로 변환된 표
```

---

## 문제 해결

### Python 관련 오류

```bash
# Python 종속성 설치
pip install -r packages/parser-py/requirements.txt
```

### 변환 실패

```bash
# 상세 로그 확인
mdm convert document.hwp -o ./output/ 2>&1 | tee log.txt
```

### 권한 오류

```bash
# CLI 실행 권한 부여
chmod +x ./cli/index.js
```

---

## 다음 단계

변환이 완료되면:

1. **편집**: `.mdx` 파일을 수정하여 내용 정리
2. **미리보기**: `mdm serve`로 결과 확인
3. **배포**: 정적 사이트로 게시 또는 Obsidian에 추가

---

**Author**: seunghan91
**Version**: 0.1.0
