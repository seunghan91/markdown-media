# mdm-parser MCP 서버

`mdm-parser` 패키지는 [Model Context Protocol](https://modelcontextprotocol.io) 서버를 내장합니다.
Claude Desktop, Claude Code 등 MCP 클라이언트에서 한국 문서(HWP, HWPX, PDF, XLSX, DOCX)를
파싱·변환·비교하는 도구로 사용할 수 있습니다.

레퍼런스: [kordoc](https://www.npmjs.com/package/kordoc) (`reference/kkdoc/src/mcp.ts`, TypeScript
구현, 15개 도구)와 도구 이름·스키마 호환을 목표로 합니다.

## 설치

```bash
pip install "mdm-parser[mcp]"
```

`mcp` extra는 공식 [`mcp` Python SDK](https://pypi.org/project/mcp/)를 추가로 설치합니다.

### 백엔드

이 서버는 실제 문서 파싱을 아래 우선순위로 위임합니다.

1. **`mdm-core`** (PyO3 네이티브 확장, PyPI) — 설치돼 있으면 in-process로 가장 빠르게 동작합니다.
   ```bash
   pip install mdm-core
   ```
2. **`hwp2mdm` Rust CLI 바이너리** — `mdm-core`가 없으면 `core/` 빌드 산출물을 서브프로세스로
   호출합니다. 저장소를 직접 빌드했다면 (`cargo build --release` in `core/`) 자동으로 탐지됩니다.
   바이너리 경로를 직접 지정하려면 환경변수를 쓰세요.
   ```bash
   export MDM_CORE_BIN=/path/to/hwp2mdm
   ```
3. 둘 다 없으면 도구 호출 시 어떤 바이너리/패키지가 필요한지 안내하는 오류가 반환됩니다.

## Claude Desktop 등록

`claude_desktop_config.json`에 추가:

```json
{
  "mcpServers": {
    "mdm-parser": {
      "command": "mdm-mcp"
    }
  }
}
```

가상환경에 설치했다면 절대 경로를 지정하세요:

```json
{
  "mcpServers": {
    "mdm-parser": {
      "command": "/path/to/venv/bin/mdm-mcp",
      "env": {
        "MDM_CORE_BIN": "/path/to/markdown-media/core/target/release/hwp2mdm"
      }
    }
  }
}
```

## Claude Code 등록

```bash
claude mcp add mdm-parser -- mdm-mcp
```

또는 `.mcp.json`:

```json
{
  "mcpServers": {
    "mdm-parser": {
      "command": "mdm-mcp"
    }
  }
}
```

## 모듈 직접 실행 (개발 중)

패키지를 설치하지 않고 저장소에서 바로 실행하려면:

```bash
cd packages/parser-py
python -m mdm.mcp_server
```

## 도구 목록 (22개)

### 즉시 사용 가능 (mdm-core / hwp2mdm CLI 백엔드로 실제 동작)

| 도구 | 설명 |
|------|------|
| `parse_document` | 문서를 마크다운으로 변환 (포맷/페이지/제목/작성자 메타 헤더 포함) |
| `convert_to_markdown` | 문서를 마크다운 본문만으로 변환 |
| `detect_format` | 파일 포맷 감지 (hwp/hwpx/pdf/xlsx/docx/unknown) |
| `get_document_info` | 파일 정보 + 메타데이터 JSON |
| `parse_metadata` | `get_document_info`와 동일 백엔드, kordoc 이름 호환 |
| `extract_media` | 문서에서 이미지 추출 → 지정 디렉토리에 저장 |
| `compare_documents` | 두 문서를 마크다운 변환 후 라인 단위 diff (신구대조표 근사치) |

### 스텁 (스키마만 정의 — 백엔드 연결 전까지 `NotYetAvailableError` 반환)

다른 에이전트가 병렬로 core(Rust)에 구현 중인 기능입니다. 각 도구 호출 시 어떤 작업에
연결 대기 중인지 오류 메시지에 명시됩니다.

| 도구 | 연결 대기 |
|------|-----------|
| `parse_pages` | core 페이지 범위 API (미배정) |
| `parse_table` | gap-pdftable / `hwp2mdm inspect` 연동 |
| `parse_form` | gap-form |
| `fill_form` | gap-form |
| `place_seal` | 미배정 (HWPX 생성 파이프라인 이후) |
| `patch_document` | 미배정 (gap-diff 구조적 diff 이후) |
| `render_document` | gap-hwpxgen |
| `redact_document` | gap-pii |
| `parse_chunks` | gap-chunker |
| `extract_profile` | gap-hwpxgen |
| `generate_document` | gap-hwpxgen |
| `validate_hwpx` | gap-hwpxgen |
| `hulk_to_latex` | gap-equation |
| `ocr_document` | gap-ocr |
| `lint_document` | 미배정 (gongmun-lint 포팅 이후) |

## 진단

백엔드 활성 상태를 확인하려면:

```python
from mdm import mcp_backend
print(mcp_backend.backend_status())
# {'mdm_core_native': True/False, 'hwp2mdm_cli': '/path/to/hwp2mdm' 또는 None}
```
