# MDM 시작 가이드

HWP, PDF, DOCX 파일을 깨끗한 Markdown으로 변환하는 가장 쉬운 방법입니다.

---

## 1분 요약 — 상황별 최단 경로

| 상황 | 방법 | 소요 시간 |
|------|------|---------|
| 파일 몇 개만 빠르게 변환 | 데스크톱 앱 | 1분 |
| Python 스크립트/자동화 | `pip install mdm-parser` | 1분 |
| Claude/Cursor 등 AI agent에서 호출 | MCP 서버 (Korea Law Hub Gateway) | 2분 |
| 서버 stdin/stdout 파이프 | Rust 바이너리 `stream` 서브커맨드 | 1분 |

---

## 방법 1: 데스크톱 앱 (설치만 하면 끝)

코딩 몰라도 됩니다. 다운로드 → 설치 → 파일 끌어다 놓기. 끝.

### Step 1. 다운로드

| OS | 다운로드 |
|----|---------|
| **macOS** (M1/M2/M3/M4) | [MDM-Desktop-0.1.0-macOS-AppleSilicon.dmg](https://github.com/seunghan91/mdm-desktop/releases/download/v0.1.0/MDM-Desktop-0.1.0-macOS-AppleSilicon.dmg) |
| **Windows** (64bit) | [MDM-Desktop-0.1.0-Windows-x64.exe](https://github.com/seunghan91/mdm-desktop/releases/download/v0.1.0/MDM-Desktop-0.1.0-Windows-x64.exe) |

### Step 2. 설치

**macOS:**
1. `.dmg` 파일 더블클릭
2. `MDM Desktop` 아이콘을 `Applications` 폴더로 드래그
3. 처음 실행 시 "확인되지 않은 개발자" 경고가 뜨면:
   - `시스템 설정` → `개인정보 보호 및 보안` → 하단의 `확인 없이 열기` 클릭

**Windows:**
1. `.exe` 파일 더블클릭
2. "Windows의 PC 보호" 경고가 뜨면: `추가 정보` → `실행` 클릭
3. 설치 마법사 따라가기 (다음 → 다음 → 완료)

### Step 3. 사용

1. **MDM Desktop** 실행
2. 변환할 파일을 화면에 **끌어다 놓기** (또는 클릭해서 파일 선택)
3. 자동으로 Markdown 변환 → 결과가 뷰어에 표시됨
4. **내보내기** 버튼으로 `.md` 파일 저장

**지원 파일**: HWP, HWPX, PDF, DOCX, PPTX, XLSX, CSV, HTML, TXT

### 여러 파일 한번에 변환하기

1. 왼쪽 메뉴에서 `일괄 변환` 클릭
2. 폴더째로 끌어다 놓기
3. `모두 변환` 클릭
4. `내보내기`에서 결과물 다운로드

---

## 방법 2: Python (개발자용)

터미널에서 한 줄이면 됩니다.

### Step 1. 설치

```bash
pip install mdm-parser
```

### Step 2. 사용

```python
from mdm_parser import PdfProcessor, HwpToSvgConverter, OcrProcessor

# PDF → 텍스트 추출
processor = PdfProcessor("보고서.pdf")
text = processor.extract_text()
print(text)

# HWP → SVG (표 변환)
converter = HwpToSvgConverter("공문서.hwp")
svg_files = converter.convert("output/")

# 스캔 이미지 → 텍스트 (OCR)
ocr = OcrProcessor(engine="auto", lang="kor+eng")
text = ocr.extract_text("스캔문서.png")
```

### Step 3. CLI로 사용

```bash
# PDF 텍스트 추출
mdm-pdf 보고서.pdf

# HWP 표를 SVG로
mdm-hwp-svg 공문서.hwp output/

# OCR
mdm-ocr 스캔이미지.png
```

---

## 방법 3: MCP 서버 — Claude/Cursor/LLM에서 직접 호출 (2분)

MDM은 **Korea Law Hub MCP Gateway**(`https://law-check.com/api/mcp`)를 통해 MCP tool 3개로 노출됩니다. Claude Code, Cursor, Continue.dev 등 MCP를 지원하는 모든 AI 클라이언트에서 바로 호출 가능합니다.

### 제공 Tool

| Tool | 용도 | cost |
|------|------|------|
| `mdm_convert_document` | 문서 → Markdown 변환 (표·서식·이미지 보존) | 6 |
| `mdm_extract_text` | 평문 텍스트만 빠르게 추출 (검색·임베딩용) | 3 |
| `mdm_detect_format` | 포맷 감지 + 지원 여부 확인 | 1 |

### Step 1. MCP 키 발급

1. https://law-check.com 접속 → 로그인
2. 설정 → MCP 키 생성
3. 발급된 키 복사 (`mcpk_...`)

### Step 2. Claude Code 설정

`~/.claude.json`에 추가:

```json
{
  "mcpServers": {
    "korea-law-hub": {
      "url": "https://law-check.com/api/mcp",
      "headers": {
        "Authorization": "McpKey YOUR_KEY_HERE"
      }
    }
  }
}
```

Claude Code 재시작하면 MDM tool 3개가 자동 로드됩니다.

### Step 3. 사용 예시

```
사용자: 이 계약서.hwp 파일 내용 정리해줘 (파일 URL: https://example.com/contract.hwp)

Claude: mdm_convert_document 호출
  → HWP 파싱 → 표·서식 보존 Markdown 반환
  → LLM이 내용 분석 후 응답
```

### 파일 전달 방식

```jsonc
// 방법 A: 원격 URL (권장)
{
  "tool": "mdm_convert_document",
  "arguments": {
    "file_url": "https://example.com/contract.hwp",
    "mode": "body"
  }
}

// 방법 B: base64 (5MB 이하)
{
  "tool": "mdm_convert_document",
  "arguments": {
    "file_base64": "UEsDBBQA...",
    "filename": "contract.hwp"
  }
}
```

### 지원 포맷
HWP · HWPX · PDF · DOCX · PPTX · XLSX · HTML · CSV · TSV · TXT

### 왜 Korea Law Hub Gateway를 통하나?

MDM은 독립 MCP 서버를 만들지 않고 이미 운영 중인 Korea Law Hub Gateway(법제처 API 44개 tool)에 3개를 추가하는 방식을 택했습니다. 이유:

- **단일 인증** — 법률 검색 + 문서 변환을 하나의 MCP 키로 사용
- **기존 rate limit · 로깅 재사용** — `cost_units` 기반 과금 인프라 그대로 활용
- **자연스러운 워크플로** — 계약서.hwp → `mdm_convert` → `search_legal_es` → LLM 종합

---

## 방법 4: Rust 바이너리 — stdin/stdout 파이프 (1분)

서버 사이드 변환이나 쉘 파이프라인에 쓸 때.

```bash
# 빌드 (최초 1회)
cd core && cargo build --release

# stdin에서 읽어서 stdout으로 출력
cat 계약서.hwp | ./target/release/hwp2mdm stream --ext hwp > 계약서.md

# 본문만 (frontmatter 제외)
cat 보고서.pdf | ./target/release/hwp2mdm stream --ext pdf --mode body
```

`stream` 서브커맨드는 파일 I/O 없이 파이프로 동작합니다. MCP 서버, Docker 컨테이너, CI 파이프라인 등에서 임시 파일 경로 고민 없이 바로 연결 가능합니다.

---

## 자주 묻는 질문

### macOS에서 "손상된 파일" 경고가 떠요
앱이 Apple 공증(Notarization)을 받았지만, 간혹 경고가 뜰 수 있습니다.
터미널에서 아래 명령어를 한 번만 실행하세요:
```bash
xattr -cr /Applications/MDM\ Desktop.app
```
그 다음 다시 실행하면 정상 작동합니다.

### Windows에서 바이러스 경고가 떠요
오픈소스 앱이라 Microsoft 인증서가 없어서 뜨는 경고입니다.
`추가 정보` → `실행`을 클릭하면 정상 설치됩니다.
소스코드: https://github.com/seunghan91/markdown-media

### HWP 파일이 안 열려요
암호가 걸린 HWP는 아직 지원하지 않습니다. 한글에서 암호를 해제한 후 다시 시도하세요.

### pip install이 안 돼요
Python 3.8 이상이 필요합니다. 버전 확인:
```bash
python3 --version
```

---

## 링크 모음

| 항목 | 링크 |
|------|------|
| 릴리즈 페이지 | https://github.com/seunghan91/mdm-desktop/releases |
| 소스코드 | https://github.com/seunghan91/markdown-media |
| PyPI | https://pypi.org/project/mdm-parser/ |
| MCP Gateway | https://law-check.com/api/mcp |
| MCP 키 발급 | https://law-check.com |
| 버그 신고 | https://github.com/seunghan91/markdown-media/issues |
