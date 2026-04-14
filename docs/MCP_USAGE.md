# MDM MCP Usage Guide

**MDM** (Markdown-Media) can be consumed by AI agents through two distinct integration paths. This document covers both.

| 경로 | 언제 사용 | 인증 | 배포 |
|-----|---------|-----|-----|
| **1. Local CLI (`stream`)** | Claude Code / Cursor / Continue.dev 등 stdio MCP 어댑터에서 직접 호출, 로컬 파일 변환 | 불필요 (로컬) | 바이너리 1개 |
| **2. Korea Law Hub MCP Gateway** | 원격 클라이언트(웹, Claude Desktop, ChatGPT Apps 등)에서 JSON-RPC로 호출, 공유 인프라 | UserMcpKey (Bearer) | `www.law-check.com/api/mcp` |

---

## 1. Local CLI — `hwp2mdm stream`

stdin으로 문서 바이트를 받아 stdout으로 마크다운을 출력하는 1-shot 모드. MCP 어댑터가 subprocess로 돌릴 때 임시파일 관리 없이 파이프만으로 동작.

### 설치

```bash
cd core && cargo build --release
# 바이너리 위치: core/target/release/hwp2mdm
```

### 사용법

```
hwp2mdm stream --ext <format> [--mode mdx|body]
```

| 옵션 | 필수 | 값 | 설명 |
|-----|:---:|----|-----|
| `--ext` | ✅ | `hwp` / `hwpx` / `pdf` / `docx` / `pptx` / `xlsx` / `html` / `csv` / `tsv` / `txt` | 확장자 힌트 (stdin엔 파일명이 없음) |
| `--mode` |  | `mdx` (기본) / `body` | `mdx`는 YAML 프론트매터 포함, `body`는 본문만 |

### 예제

```bash
# 계약서.hwp → 마크다운
cat 계약서.hwp | hwp2mdm stream --ext hwp > contract.md

# 엑셀 → 본문만
cat data.xlsx | hwp2mdm stream --ext xlsx --mode body

# DOCX with equations → LaTeX 포함 마크다운
cat paper.docx | hwp2mdm stream --ext docx | head -20
```

### Claude Code / Cursor 통합 (예시)

`~/.claude.json` (Claude Code) 또는 `~/.cursor/mcp_servers.json` (Cursor)에서 MCP 어댑터를 직접 만들 때:

```json
{
  "mcpServers": {
    "mdm": {
      "command": "sh",
      "args": [
        "-c",
        "cat \"$FILE\" | /usr/local/bin/hwp2mdm stream --ext \"${FILE##*.}\""
      ]
    }
  }
}
```

> **참고**: 이 방식은 어댑터 레이어가 필요합니다. 표준 JSON-RPC MCP 서버로 공개하려면 **경로 2 (게이트웨이)** 를 쓰는 것이 더 간단합니다.

---

## 2. Korea Law Hub MCP Gateway

원격 JSON-RPC 2.0 엔드포인트로 MDM을 호출하는 방식. 인증·rate limit·로깅이 포함되며 Claude Desktop이나 웹 에이전트에서 별도 로컬 바이너리 없이 사용 가능.

**엔드포인트**: `POST https://www.law-check.com/api/mcp`
**인증**: `Authorization: Bearer <64-hex UserMcpKey>`
**프로토콜**: MCP JSON-RPC 2.0 (`protocolVersion: "2024-11-05"`)

MDM 툴은 Gateway의 47개 툴 중 3개 (`mdm_*` prefix). 나머지 44개는 법령/판례 검색 계열 (본 문서 범위 밖).

### 2.1. 툴 레퍼런스

#### `mdm_convert_document`

풀 마크다운 변환. 구조·헤딩·표·볼드/이탤릭·이미지 참조 보존.

**입력 스키마**:
```json
{
  "type": "object",
  "properties": {
    "file_url":    { "type": "string", "description": "원격 파일 URL (HTTP/HTTPS)" },
    "file_base64": { "type": "string", "description": "base64 인코딩된 바이트 (5MB 이하)" },
    "filename":    { "type": "string", "description": "확장자 판별용 (file_base64 사용 시 필수)" },
    "mode":        { "type": "string", "enum": ["mdx", "body"], "description": "출력 형식" }
  },
  "oneOf": [
    { "required": ["file_url"] },
    { "required": ["file_base64", "filename"] }
  ]
}
```

**출력**:
```json
{
  "status": "OK",
  "format": "hwp",
  "elapsed_ms": 23,
  "meta": { "author": "...", "title": "..." },
  "markdown": "# 근로계약서\n\n갑과 을은...",
  "powered_by": "MDM (Markdown-Media) — Rust, HWP-native",
  "data_source": "user_provided_document (MDM Rust engine)"
}
```

**Cost**: 6 units.

#### `mdm_extract_text`

빠른 텍스트만 추출 (마크다운 마커 제거). 임베딩 인덱싱·검색 파이프라인용.

**입력**: `file_url` / `file_base64` + `filename` (모드 옵션 없음)

**출력**:
```json
{
  "status": "OK",
  "format": "docx",
  "text": "근로계약서 갑과 을은 다음과 같이...",
  "powered_by": "MDM (Markdown-Media)"
}
```

**Cost**: 3 units.

#### `mdm_detect_format`

변환 없이 포맷 판정. 지원 여부와 확장자/매직바이트만 반환.

**입력**:
```json
{
  "filename": "contract.hwp",
  "file_base64": "<optional, first 16 bytes>"
}
```

**출력**:
```json
{
  "status": "OK",
  "filename": "contract.hwp",
  "extension": "hwp",
  "supported": true,
  "magic_bytes": "D0 CF 11 E0 A1 B1 1A E1 ...",
  "powered_by": "MDM (Markdown-Media)"
}
```

**Cost**: 1 unit.

### 2.2. 예제 — curl

#### 툴 목록 조회 (`tools/list`)

```bash
curl -s https://www.law-check.com/api/mcp \
  -H "Authorization: Bearer $MCP_KEY" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' \
| jq '.result.tools[] | select(.name | startswith("mdm_"))'
```

#### 문서 변환 (URL)

```bash
curl -s https://www.law-check.com/api/mcp \
  -H "Authorization: Bearer $MCP_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 42,
    "method": "tools/call",
    "params": {
      "name": "mdm_convert_document",
      "arguments": {
        "file_url": "https://example.org/contract.hwp",
        "mode": "body"
      }
    }
  }' \
| jq -r '.result.content[0].text | fromjson | .markdown' \
| head -20
```

#### 문서 변환 (base64)

```bash
FILE=contract.hwp
curl -s https://www.law-check.com/api/mcp \
  -H "Authorization: Bearer $MCP_KEY" \
  -H "Content-Type: application/json" \
  -d "$(jq -n --arg b64 "$(base64 < $FILE)" --arg fn "$FILE" '{
    jsonrpc: "2.0",
    id: 43,
    method: "tools/call",
    params: {
      name: "mdm_convert_document",
      arguments: { file_base64: $b64, filename: $fn, mode: "mdx" }
    }
  }')"
```

#### Claude Desktop 설정

```json
{
  "mcpServers": {
    "korea-law-hub": {
      "url": "https://www.law-check.com/api/mcp",
      "headers": {
        "Authorization": "Bearer <YOUR_MCP_KEY>"
      }
    }
  }
}
```

→ Claude Desktop에서 바로 `mdm_convert_document`, `mdm_extract_text`, `mdm_detect_format` 뿐 아니라 법령·판례 검색 44개 툴까지 통째로 사용 가능.

### 2.3. 권한 (Permissions)

MDM 툴 3개 모두 `read` 스코프. UserMcpKey에 `read` 권한이 있으면 호출 가능. 별도 "convert" 스코프는 만들지 않음 — 법령 조회(`read`)와 동일 보안 티어로 취급.

### 2.4. Rate Limit (가중치)

| 툴 | Cost Units | 설명 |
|---|:----:|-----|
| `mdm_detect_format` | 1 | 매직바이트/확장자 검사만 |
| `mdm_extract_text` | 3 | 텍스트 추출 (변환 후 마커 제거) |
| `mdm_convert_document` | 6 | 풀 구조화 변환 |

`mcp_usage_logs.cost_units` 합계가 키별 월 한도를 초과하면 `-32004 Rate Limited` 반환.

### 2.5. 보안 / 데이터 처리

- 업로드된 파일은 Rails Gateway 내부 Tempfile로 처리. Rust 바이너리(`bin/hwp2mdm-linux-x64`) subprocess에서만 읽힘.
- **변환 완료 후 즉시 삭제** (Tempfile 블록 종료 → auto-unlink).
- 변환 결과(markdown)는 `mcp_usage_logs`에 **저장되지 않음** (로그는 tool_name/cost/error만 기록).
- `file_url`은 HTTP(S) 스킴만 허용. `javascript:`, `data:`, 로컬 `file:` 차단.
- `file_base64` 상한 5MB. 초과 시 `file_url`로 유도.

---

## 3. 지원 포맷

| 포맷 | 확장자 | HWP 네이티브 | 비고 |
|-----|-------|:-----------:|-----|
| HWP | `.hwp` | ✅ | 한글 OLE CFB 컨테이너. 암호화, 법률 문서 구조 지원 |
| HWPX | `.hwpx` | ✅ | 한글 XML 기반 (ZIP 컨테이너) |
| PDF | `.pdf` | ✅ | 자체 파서. H1-H4 헤딩, 볼드/이탤릭, 2열 레이아웃, 헤더/푸터 제거 |
| DOCX | `.docx` | ✅ | OMML 수식 → LaTeX 자동 변환 |
| PPTX | `.pptx` | ✅ | 슬라이드 표, 이미지 플레이스홀더, 노트 정확 매핑 |
| XLSX | `.xlsx` `.xls` | ✅ | 다중 시트, 파이프 이스케이프, ODS 지원 |
| HTML | `.html` `.htm` | ✅ | alt 보존, XSS URL 스트립, 체크박스 `[x]`/`[ ]` |
| CSV | `.csv` | ✅ | 따옴표·개행 평탄화, 파이프 이스케이프 |
| TSV | `.tsv` | ✅ | 자동 탭 감지 |
| TXT | `.txt` `.log` | ✅ | UTF-8/UTF-16 BOM / EUC-KR 자동 감지 |

**MarkItDown 대비** (Microsoft): DOCX/PPTX/XLSX/HTML/CSV/TXT/PDF 7개 포맷에서 MDM이 상대적 우위. 상세: [docs/markitdown-compare/README.md](markitdown-compare/README.md).

---

## 4. 에러 핸들링

| 상황 | 응답 |
|-----|-----|
| 알 수 없는 툴 이름 | JSON-RPC `-32601 Method not found` |
| 필수 인자 누락 | `{ status: "ERROR", message: "either file_url or file_base64 is required" }` |
| 지원 안 되는 포맷 | `{ status: "ERROR", message: "no .mdx output produced" }` |
| `file_url` 비-HTTP 스킴 | `{ status: "ERROR", message: "file_url must be http(s)" }` |
| `file_base64` 5MB 초과 | `{ status: "ERROR", message: "file_base64 payload exceeds 5 MB ..." }` |
| Rate limit 초과 | JSON-RPC `-32004 Rate limited` |
| 권한 부족 | JSON-RPC `-32003 Unauthorized tool` |
| 변환 타임아웃 | `{ status: "ERROR", message: "conversion timed out (60s)" }` |

`isError: true` 필드가 MCP 응답 최상위에 세팅됨.

---

## 5. FAQ

**Q. 왜 `file_url`과 `file_base64` 두 가지 입력 방식?**
A. MCP 툴 설계 베스트 프랙티스 (2025-2026). 대용량은 signed URL이 표준이나, 소형/로컬 파일은 base64가 편리. 둘 중 하나 선택. 동시 지정 시 `file_url`이 우선.

**Q. HWPX와 HWP 중 어떤 게 나은가요?**
A. 입력 파일 포맷에 따라 다름 — 둘 다 MDM 네이티브 지원. 양쪽 모두 한글 Office에서 저장한 원본이면 품질 동일.

**Q. DRM 걸린 HWP 파일은?**
A. Fasoo DRMONE 등 기업 DRM은 OS 레벨 AES 암호화 + 라이선스 서버 필요. 오픈소스 파서로 해독 불가. 한글 Office에서 DRM 해제 후 재저장 필요. MDM은 친절한 에러 메시지로 안내.

**Q. 변환 결과에 YAML 프론트매터가 왜 있나요? 없앨 수 있나요?**
A. `mode: "body"` 옵션 사용. 스트림 모드에서는 `--mode body`. 프론트매터는 LLM 컨텍스트에 유용한 메타데이터(작성자, 페이지 수, 시트 수 등)라 기본은 포함.

**Q. 이미지는 어떻게 처리되나요?**
A. Gateway의 `mdm_convert_document`는 현재 **플레이스홀더만** 반환 (`![alt](filename.ext)`). 실제 이미지 바이트 추출은 Phase 2에서 제공 예정 (현재는 미디어 번들이 CLI-only).

**Q. PDF 100페이지가 넘는 큰 파일도 되나요?**
A. Gateway는 60초 타임아웃. 보통 383페이지를 5.6초에 처리하므로 1000페이지 이내는 안전. 초과 시 로컬 CLI(`stream` 모드) 권장.

**Q. 생성된 마크다운은 저장되나요?**
A. 아니요. `mcp_usage_logs`에는 tool_name / cost_units / response_time / error만 기록. markdown 본문은 응답 즉시 GC.

**Q. MCP 키 발급은 어디서?**
A. `www.law-check.com/settings/mcp-keys` (로그인 필요).

---

## 6. 관련 문서

- [README.md](../README.md) — MDM 프로젝트 개요
- [docs/MDM_SYNTAX_SPEC.md](MDM_SYNTAX_SPEC.md) — 출력 마크다운의 MDM 참조 문법 (`@[[image]]` 등)
- [docs/markitdown-compare/README.md](markitdown-compare/README.md) — Microsoft MarkItDown 대비 품질 비교
- [docs/mcp-integration-assessment.md](mcp-integration-assessment.md) — MCP 통합 설계 의사결정 기록

---

**Last Updated**: 2026-04-14
**Gateway Version**: 2.0.0 (Korea Law Hub MCP)
**MDM Version**: latest (master)
