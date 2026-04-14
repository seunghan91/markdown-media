# MDM × Korea Law Hub — MCP 서버 제공 및 통합 점검

**Date**: 2026-04-14
**Scope**: MDM을 MCP 서버로 제공하는 방안 + law 프로젝트(Korea Law Hub)와의 통합 가능성 점검

## TL;DR

- **law 프로젝트는 이미 MDM을 vendor 바이너리로 운영 중** — `/Users/seunghan/law/vendor/markdown-media/`에 전체 소스, `bin/hwp2mdm-linux-x64`에 프로덕션 바이너리, `DocumentParserService`가 subprocess로 호출, `/hwp` 페이지에서 `kordoc vs mdm` A/B 변환 UI 운영 중.
- **MCP 층만 없음** — 현재는 Rails 프로세스 내부 호출. 외부 agent(Claude Code/Cursor/LLM)가 MDM을 MCP tool로 호출하는 경로가 없음.
- **Korea Law Hub MCP Gateway(44 tools)에 `mdm_convert_*`를 추가하는 것이 가장 빠른 통합.** Rails `api/mcp_controller.rb:477 law_verification_tools`에 2-3개 tool 추가만으로 공개 가능. 별도 서버 구축 불필요.
- **독립 MCP 서버(Phase 2)**: `packages/mcp-server` Node.js 신설로 Claude Code/Cursor용 stdio MCP 제공 — law 종속 없이 재사용 가능.

## 1. 현재 통합 현황 (실측)

| 항목 | 상태 | 위치 |
|-----|-----|-----|
| MDM 소스 vendor 포함 | ✅ | `/Users/seunghan/law/vendor/markdown-media/` |
| 리눅스 프로덕션 바이너리 | ✅ 2.1MB | `apps/legal_audit_web/bin/hwp2mdm-linux-x64` |
| 로컬 빌드 참조 | ✅ | `vendor/markdown-media/core/target/release/hwp2mdm` |
| DocumentParserService | ✅ 137 LOC | `app/services/document_parser_service.rb` |
| A/B 비교 UI | ✅ | `HwpController` → `Hwp/Index` Inertia 페이지 |
| DocumentConversion 모델 | ✅ | `PREFERRED_ENGINES = %w[kordoc mdm tie]` |
| MCP Gateway | ✅ 44 tools | `api/mcp_controller.rb`, `www.law-check.com/api/mcp` |
| **MDM을 MCP tool로 노출** | ❌ | 현재 Gateway의 `law_verification_tools`에 등록 안 됨 |
| 독립 MCP 서버 | ❌ | MDM 리포에 MCP scaffolding 없음 |

**결론**: 데이터 평면(바이너리 통합)은 이미 완성. 제어 평면(MCP 노출)만 빠짐.

## 2. Korea Law Hub 아키텍처와의 적합성

law 프로젝트 CLAUDE.md에서 정의된 4계층:

```
(1) Korea Law Hub MCP Gateway     www.law-check.com/api/mcp     ← 외부 공개
(2a) korea-law-mcp                korea-law-mcp.onrender.com    ← REST 래퍼
(3a) korean-law-mcp v3.0.0        fly.dev                       ← 비교 검증용
(4) Korea Law Hub Engine          npm @korea-law                ← 코어
```

**MDM의 자연스러운 포지션**: 계층 (1) Gateway의 새 tool 카테고리.

기존 44 tools은 전부 "**외부 데이터 조회**" 성격 (법령/판례/행정규칙 검색, 해석 비교 등). MDM은 "**사용자 제공 문서 정규화**"로 카테고리가 겹치지 않음 — 오히려 상호 보완:

```
사용자 흐름:
  1. 계약서.hwp 업로드
  2. mdm_convert  → 마크다운 + 구조 IR  ← MDM이 제공
  3. search_legal_es(해당 조항)         ← 기존 Hub tool
  4. compare_amendment(관련 법 개정)    ← 기존 Hub tool
  5. LLM이 분석 종합
```

## 3. MCP 통합 옵션 비교

### 옵션 A: Hub Gateway에 tool 추가 (Rails)

**작업**: `api/mcp_controller.rb:477 law_verification_tools` 메서드에 tool 정의 3개 추가, `proxy_to_mcp_server` 또는 새 `convert_document_via_mdm` 메서드로 `DocumentParserService`(이미 존재) 호출.

**장점**:
- 기존 인증(UserMcpKey), rate limit(cost_units), 사용량 로깅(mcp_usage_logs) 즉시 재활용
- `www.law-check.com/api/mcp` 한 엔드포인트로 44→47 tools 확장
- 코드 변경 ~150 LOC, 1일 이내

**단점**:
- MDM이 law에 종속 — 다른 프로젝트가 사용하려면 별도 Gateway 필요
- Rails 프로세스 메모리/CPU에서 변환 실행 (대용량 PDF 시 워커 블록)
- 프로덕션 바이너리를 Render 컨테이너에 수동 업데이트 중 (CI 부재)

### 옵션 B: 독립 Node.js MCP 서버 (`packages/mcp-server`)

**작업**: MDM 리포에 신규 패키지 추가, `@modelcontextprotocol/sdk-typescript` 사용, Rust 바이너리를 subprocess로 spawn.

**장점**:
- stdio MCP로 Claude Code / Cursor / Continue.dev에서 직접 사용 가능
- HTTP MCP 모드도 지원하여 독립 배포 가능
- npm publish `@mdm/mcp-server` — 누구나 설치해서 자체 agent에 연결
- law 프로젝트와 무관하게 활용 범위 확장

**단점**:
- Node.js 추가 런타임 의존 (현재 MDM은 순수 Rust + 선택적 Python)
- 초기 개발 ~3일 (MCP SDK 학습 + 패키징)

### 옵션 C: Rust native MCP 서버

**작업**: `core`에 `mdm-mcp-server` 바이너리 추가, JSON-RPC 2.0 직접 구현 또는 `rust-mcp-sdk` 사용.

**장점**:
- 단일 Rust 바이너리, 런타임 의존 없음
- 최고 성능 (subprocess 오버헤드 없음)

**단점**:
- Rust MCP 생태계 미성숙 (SDK 불안정)
- Node/Python MCP 표준 patterns와 다른 스택이라 사용자 친숙도 낮음

### 옵션 D: Python MCP 서버 (mdm-parser 확장)

**작업**: `packages/parser-py`에 MCP 서버 모듈 추가, `mcp` pip 패키지 사용, PyO3 바인딩으로 Rust 코어 호출.

**장점**:
- 이미 Python 바인딩 존재
- MCP Python SDK는 가장 성숙

**단점**:
- Python 런타임 의존
- 이미 mdm-parser는 "라이브러리"로 포지셔닝됨 — MCP 서버는 다른 사용 패턴

## 4. 추천 로드맵

### Phase 1: Hub Gateway에 tool 등록 (옵션 A, 1일)

목표: 즉시 `law-check.com` 사용자가 MCP로 MDM 호출 가능하게.

**추가할 tool 정의 (3개)**:

```ruby
# mcp_controller.rb law_verification_tools 추가

{
  name: "mdm_convert",
  description: "HWP/HWPX/PDF/DOCX/PPTX/XLSX/HTML 문서를 Markdown으로 변환. " \
               "업로드된 문서의 URL 또는 base64를 받아 구조화된 마크다운과 메타데이터를 반환.",
  inputSchema: {
    type: "object",
    properties: {
      file_url:    { type: "string", description: "업로드된 파일 URL (ActiveStorage 또는 S3)" },
      file_base64: { type: "string", description: "파일 본문 base64 (file_url 대신)" },
      filename:    { type: "string", description: "확장자 판별용 (예: contract.hwp)" },
      preserve_media: { type: "boolean", default: false, description: "이미지/표 asset 번들 포함 여부" }
    },
    oneOf: [{ required: ["file_url"] }, { required: ["file_base64", "filename"] }]
  }
},
{
  name: "mdm_extract_text",
  description: "문서에서 텍스트만 빠르게 추출 (구조 정보 없음, 빠른 경로).",
  inputSchema: { /* file_url 또는 file_base64 + filename */ }
},
{
  name: "mdm_detect",
  description: "파일 타입 감지 및 MDM 지원 여부 확인. 변환 없이 가능 여부만 판단.",
  inputSchema: { /* filename 또는 magic bytes */ }
}
```

**cost_units 제안**:
- `mdm_detect`: 1 유닛 (메타데이터만)
- `mdm_extract_text`: 3 유닛 (가벼움)
- `mdm_convert`: 6 유닛 (기본값), `preserve_media: true`면 10 유닛

**구현 단계**:
1. `api/mcp_controller.rb:477` `law_verification_tools` 배열에 3개 tool 추가
2. `handle_tools_call` 분기에 `mdm_*` 라우팅 추가
3. `DocumentParserService.run_mdm` 재사용 — file_url 다운로드 → 임시파일 → 기존 메서드 호출
4. 응답 sanitize (`sanitize_response`에 `mdm_*` 엔트리 추가 — markdown 본문은 통과)
5. `mcp_capabilities` 에 tool 카테고리 "document_conversion" 추가

### Phase 2: 독립 MCP 서버 (옵션 B, 3일)

목표: law 종속 없이 Claude Code/Cursor에서 직접 MDM 호출.

**패키지 구조**:
```
packages/mcp-server/
├── package.json        # @mdm/mcp-server
├── src/
│   ├── index.ts        # stdio MCP 엔트리
│   ├── http.ts         # HTTP MCP 엔트리 (옵셔널)
│   ├── tools/
│   │   ├── convert.ts
│   │   ├── extract_text.ts
│   │   └── detect.ts
│   └── binary.ts       # hwp2mdm subprocess 래퍼
├── README.md
└── bin/
    └── mdm-mcp         # 실행파일
```

**Claude Code 설정 예**:
```json
{
  "mcpServers": {
    "mdm": {
      "command": "npx",
      "args": ["-y", "@mdm/mcp-server@latest"],
      "env": {
        "MDM_BINARY_PATH": "/usr/local/bin/hwp2mdm"
      }
    }
  }
}
```

**배포**:
- npm publish `@mdm/mcp-server`
- Docker image `mdm/mcp-server:latest` (바이너리 동봉)
- Homebrew formula 선택적

### Phase 3: Hub Gateway가 독립 서버를 호출하도록 전환

Phase 1에서는 Rails 내부에서 subprocess 호출. Phase 2 서버가 안정되면 Gateway의 `mdm_*` tool 구현을 **MCP-to-MCP 프록시**로 교체 → Render 컨테이너에서 MDM을 분리해 별도 워커로 운영 (대용량 PDF 처리 시 Rails 메인 프로세스 보호).

## 5. 위험 요소 & 결정 사항

| 이슈 | 현황 | 결정 |
|-----|-----|-----|
| law의 vendor 바이너리 수동 업데이트 | 동기화 없음 | Phase 1 시 `bin/sync-mdm-binary.sh` 스크립트 추가 |
| HWP 암호화/패스워드 파일 처리 | MDM core 이미 지원 (`hwp::`에 로직 있음) | tool 입력에 `password` optional 필드 |
| 대용량 PDF(수백 페이지) Rails 프로세스 블록 | 현재 60s 타임아웃만 설정 | Phase 1에선 tool 자체에 `max_pages` 옵션. Phase 3에서 분리 |
| 프로덕션 Render 컨테이너에 Rust 바이너리 배포 | 수동 커밋 | CI(GitHub Actions)에서 MDM Release 아티팩트 → law 리포 `bin/` 자동 커밋 |
| 라이선스 | MDM MIT, law Proprietary | 문제없음 (vendor는 MIT 고지 필요, 이미 준수) |
| 브랜딩 | "Korea Law Hub MCP"와 "MDM" 명칭 혼동 | Hub 문서에서 tool 설명에 "powered by MDM (Markdown-Media)" 명시 |

## 6. MDM 리포에 필요한 최소 변경 (Phase 1에서도)

| 파일 | 변경 | 이유 |
|-----|-----|-----|
| `core/src/main.rs` | `--emit-json` 플래그 추가 | MCP tool 응답으로 구조화 IR 전달 시 유용 (현재는 mdx + 파일출력만) |
| `core/src/main.rs` | stdin 입력 + stdout 출력 모드 (`--stdin --stdout`) | 임시파일 I/O 스킵 가능, MCP 서버에서 성능 이득 |
| 없음 (Phase 1) | 나머지 변경 불필요 — 기존 `hwp2mdm convert FILE -o DIR`로 충분 | |

## 7. 질문 (사용자 확인 필요)

1. **Phase 1부터 할지, Phase 2 먼저 할지**: 빠른 통합(Hub 내부)은 Phase 1, 재사용성(외부 agent)은 Phase 2. 두 Phase 동시 진행도 가능(독립적).
2. **독립 MCP 서버의 이름/패키지명**: `@mdm/mcp-server`, `@markdown-media/mcp`, `hwp2mdm-mcp` 중 선호.
3. **Hub Gateway에 추가할 때 tool 이름 prefix**: `mdm_convert` 또는 `document_convert` 또는 `hwp_convert` 중.
4. **stdin/stdout 모드** MDM 바이너리에 추가할 의향 — MCP 성능을 위한 투자(반나절 정도), Phase 1에도 도움.
5. **Korea Law Hub 공개 로드맵에 "powered by MDM" 명시 의향** — MDM 노출도 상승, law 공공성 강조에도 부합.

## 결론

- **통합은 이미 80% 완료된 상태**. MCP 표면만 얹으면 됨.
- **Phase 1 (Hub tool 등록)을 1일 내 완료 가능** — 즉시 외부 agent가 `law-check.com/api/mcp`로 `mdm_convert` 호출 가능.
- **Phase 2 (독립 MCP 서버)는 MDM 단독 가치를 극대화** — Claude Code 등에서 모두가 "한글 문서 → 마크다운" 툴로 사용.
- MDM의 HWP 네이티브 파싱이 Korea Law Hub의 핵심 차별화(한국 문서 생태계)와 완벽히 정렬됨. 두 프로젝트의 결합은 MDM README의 "Document-to-AI 인프라 레이어" 포지셔닝을 실증적으로 증명함.
