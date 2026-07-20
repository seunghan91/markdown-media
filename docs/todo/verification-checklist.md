# MDM 검증 체크리스트 — 다른 AI 에이전트 지시용

> Date: 2026-07-19
> 상태: 구현 완료, 검증 필요
> 대상: /Users/seunghan/markdown-media/

---

## 1. 빌드 검증

```bash
cd /Users/seunghan/markdown-media/core

# 기본 빌드 (default features)
cargo build --release

# 전체 feature 빌드
cargo build --release --features full,watch,ocr,url-fetch,docx-out,pdf-out

# WASM 빌드
cargo build --release --target wasm32-unknown-unknown
```

---

## 2. 신규 CLI 서브커맨드 동작 검증 (11개)

각 명령을 실제 파일로 실행해보고 출력을 확인한다.

테스트 파일 위치: `core/../tests/fixtures/` 또는 `core/../samples/`

### 2.1 `generate` — Markdown → HWPX/DOCX/PDF
```bash
echo "# 제목\n\n본문 내용입니다.\n\n- 항목1\n- 항목2" > /tmp/test.md

# HWPX 출력
hwp2mdm generate /tmp/test.md -o /tmp/test.hwpx
# → /tmp/test.hwpx 생성, ZIP 파일 (PK 시그니처)

# 공문서 프리셋
hwp2mdm generate /tmp/test.md -o /tmp/report.hwpx -p 보고서

# DOCX 출력 (--features docx-out 필요)
hwp2mdm generate /tmp/test.md -o /tmp/test.docx -F docx

# PDF 출력 (--features pdf-out 필요)
hwp2mdm generate /tmp/test.md -o /tmp/test.pdf -F pdf
```

### 2.2 `redact` — PII 마스킹
```bash
echo "주민번호: 900101-1234567\n전화: 010-1234-5678\n이메일: hong@example.com" > /tmp/pii.md
hwp2mdm redact /tmp/pii.md
# → 마스킹된 텍스트 (●●●)

# 특정 룰만
hwp2mdm redact /tmp/pii.md -r rrn,phone -o /tmp/redacted.md
```

### 2.3 `diff` — 문서 비교
```bash
echo "# 문서1\n\n내용A\n\n내용B" > /tmp/doc_a.md
echo "# 문서1\n\n내용A\n\n내용C\n\n내용D" > /tmp/doc_b.md
hwp2mdm diff /tmp/doc_a.md /tmp/doc_b.md
# → 추가/삭제/변경/이동 블록 diff 출력

hwp2mdm diff /tmp/doc_a.md /tmp/doc_b.md --format json
# → JSON 구조적 diff
```

### 2.4 `fill` — 양식 채움
```bash
# HWPX 파일이 있으면:
echo '{"성명": "홍길동", "생년월일": "1990-01-01"}' > /tmp/values.json
hwp2mdm fill template.hwpx -j /tmp/values.json -o /tmp/filled.hwpx

# Dry run (스키마 확인)
hwp2mdm fill template.hwpx --dry-run
```

### 2.5 `lint` — 공문서 표기법
```bash
hwp2mdm lint /tmp/test.md
# → 13개 규칙 위반 목록 (또는 "No issues found")
```

### 2.6 `chunks` — RAG 청킹
```bash
hwp2mdm chunks /tmp/test.md
# → JSON 청크 배열 출력

hwp2mdm chunks /tmp/test.md -g block --max-chars 500 --overlap 50
# → 문자수 제한 + 중첩
```

### 2.7 `watch` — 디렉토리 감시 (--features watch)
```bash
mkdir -p /tmp/watch_in /tmp/watch_out
hwp2mdm watch /tmp/watch_in -o /tmp/watch_out
# → 감시 시작. 다른 터미널에서 /tmp/watch_in/*.hwp 복사 시 자동 변환
```

### 2.8 `legal` — 법령문서 파싱
```bash
echo "제1조 (목적) 이 법은..." > /tmp/law.md
hwp2mdm legal /tmp/law.md
# → LegalChunk JSON

hwp2mdm legal /tmp/law.md --format json
```

### 2.9 `url` — URL→MD (--features url-fetch)
```bash
hwp2mdm url https://example.com -o /tmp/url_out
# → /tmp/url_out/example_com.mdx 생성
```

### 2.10 `validate` — HWPX 검증
```bash
# HWPX 파일이 있으면:
hwp2mdm validate document.hwpx
# → "Validation passed" 또는 오류 목록
```

### 2.11 `equation` — 수식 변환
```bash
echo "sum from {i=1} to n i" > /tmp/eq.hulk
hwp2mdm equation /tmp/eq.hulk
# → LaTeX 출력

hwp2mdm equation /tmp/eq.hulk -d latex2hulk
# → HULK 출력
```

---

## 3. 신규 포맷 파서 검증 (5종)

### 3.1 RTF
```bash
# RTF 샘플 준비 (최소)
printf '{\rtf1\ansi Hello world\par Goodbye.}' > /tmp/test.rtf
hwp2mdm convert /tmp/test.rtf -o /tmp/rtf_out
# → /tmp/rtf_out/test.mdx 생성, "Hello world\nGoodbye" 포함
```

### 3.2 EPUB
```bash
# EPUB 파일이 있으면:
hwp2mdm convert book.epub -o /tmp/epub_out
# → 챕터별 마크다운 추출
```

### 3.3 DOC (97-2003)
```bash
# .doc 파일이 있으면:
hwp2mdm convert legacy.doc -o /tmp/doc_out
# → 텍스트 추출 (best-effort)
```

### 3.4 XLS (97-2003) — calamine 업그레이드 검증
```bash
# .xls 파일이 있으면:
hwp2mdm convert spreadsheet.xls -o /tmp/xls_out
# → 시트별 마크다운 테이블 추출
```

### 3.5 HWP3 (1996-2002)
```bash
# HWP3 파일이 있으면:
hwp2mdm convert old.hwp -o /tmp/hwp3_out
# → "Format: HWP 3.0" 메시지와 함께 텍스트 추출
```

---

## 4. OCR 검증 (--features ocr)

```bash
hwp2mdm convert scanned.pdf --ocr -o /tmp/ocr_out
# → OCR 엔진 로드 후 텍스트 추출
# 참고: 첫 실행 시 PP-OCRv5 모델 다운로드 (~18MB)
```

---

## 5. MCP 서버 검증

```bash
cd /Users/seunghan/markdown-media/packages/parser-py

# 의존성 설치
pip install -e ".[mcp]"

# 서버 시작 (stdio)
python -m mdm.mcp_server

# Claude Desktop 설정 (~/Library/Application Support/Claude/claude_desktop_config.json):
{
  "mcpServers": {
    "mdm-parser": {
      "command": "python",
      "args": ["-m", "mdm.mcp_server"],
      "cwd": "/Users/seunghan/markdown-media/packages/parser-py"
    }
  }
}
```

### 동작 확인할 MCP 도구 (17개)
1. `parse_document` — 문서 → 마크다운 + 메타
2. `convert_to_markdown` — 문서 → 본문만
3. `detect_format` — 포맷 감지
4. `get_document_info` — 파일 정보
5. `parse_metadata` — 메타데이터
6. `extract_media` — 이미지 추출
7. `compare_documents` — 문서 비교
8. `redact_document` — PII 마스킹
9. `lint_document` — 공문서 린트
10. `parse_chunks` — RAG 청킹
11. `generate_document` — Markdown→HWPX
12. `fill_form` — 양식 채움
13. `parse_form` — 양식 스키마 추출
14. **`parse_table`** — 표 추출 (신규 연결)
15. **`validate_hwpx`** — HWPX 검증 (신규 연결)
16. **`hulk_to_latex`** — 수식 변환 (신규 연결)
17. **`ocr_document`** — OCR (신규 연결)

### 아직 스텁인 MCP 도구 (5개)
| 도구 | 사유 |
|------|------|
| `parse_pages` | 페이지 범위 API 미구현 |
| `place_seal` | 도장 배치 모듈 미구현 |
| `patch_document` | Roundtrip patch 미구현 |
| `render_document` | HWPX→PNG/SVG 렌더링 미구현 |
| `extract_profile` | 표 서식 프로필 추출 미구현 |

---

## 6. Feature Flag 빌드 검증

```bash
cd /Users/seunghan/markdown-media/core

# 각 feature 조합별 빌드 확인
cargo check --no-default-features --features hwp
cargo check --no-default-features --features hwp,xls,rtf,epub
cargo check --features watch
cargo check --features ocr
cargo check --features url-fetch
cargo check --features docx-out
cargo check --features pdf-out
cargo check --features heic
cargo check --features full,watch,ocr,url-fetch,docx-out,pdf-out,heic
```

---

## 7. 유닛 테스트 작성이 시급한 모듈

현재 테스트 0개인 핵심 파서들. 회귀 방지를 위해 최소 스모크 테스트 필요.

| 모듈 | 파일 | 라인 | 우선순위 |
|------|------|------|---------|
| HWP5 | `core/src/hwp/parser.rs` | 2,837 | P0 |
| HWPX | `core/src/hwpx/parser.rs` | 2,867 | P0 |
| PDF | `core/src/pdf/parser.rs` | 3,689 | P0 |
| DOCX | `core/src/docx/parser.rs` | 2,084 | P0 |
| HWP3 | `core/src/hwp3/parser.rs` | 2,509 | P1 |
| OCR | `core/src/ocr/engine.rs` | 178 | P1 |
| DOC97 | `core/src/doc97.rs` | 187 | P2 |
| EPUB | `core/src/epub.rs` | 428 | P2 |
| RTF | `core/src/rtf.rs` | 103 | P2 |

### 추천: 스모크 테스트 추가
`samples/` 디렉토리에 있는 `.hwp`/`.hwpx`/`.pdf`/`.docx` 파일을 fixture로 사용:
```rust
#[test]
fn smoke_test_hwp5() {
    let data = include_bytes!("../../tests/fixtures/sample.hwp");
    let parser = HwpParser::from_bytes(data.to_vec()).unwrap();
    let doc = parser.to_mdm().unwrap();
    assert!(!doc.content.is_empty());
}
```

---

## 8. 문서화 갱신 필요

| 문서 | 현재 상태 | 필요 작업 |
|------|----------|----------|
| `README.md` | 21개 CLI 언급 없음 | CLI 목록 + 예제 추가 |
| `start.md` | 구버전 로드맵 | 현재 구현 상태 반영 |
| `GETTING-STARTED.md` | CLI 사용법 없음 | `hwp2mdm --help` 기준 업데이트 |
| `docs/MCP_USAGE.md` | 3개 도구만 문서화 | 17개 도구로 갱신 |
| `packages/parser-py/docs/mcp.md` | 13개 도구 문서화 | 17개로 갱신 |
| `CHANGELOG.md` | v0.3.0 이후 없음 | 오늘 변경사항 기록 |

---

## 9. 남은 기능 구현 (P3)

| 항목 | 파일 | 난이도 | 예상 |
|------|------|--------|------|
| 차트 생성 | `hwpx_gen/section.rs:169 TODO(chart)` | 중 | 1주 |
| 5개 MCP 스텁 구현 | 각 모듈 신규 작성 | 상 | 2-3주 |
| 기밀 등급 분류 | ML 모델 필요 | 상 | 3-4주 |
| 유해발화 감지 | regex + 모델 | 중 | 1주 |

---

## 10. 파일 구조 최종 확인

```bash
# 신규 파일 존재 확인
ls -la /Users/seunghan/markdown-media/core/src/rtf.rs
ls -la /Users/seunghan/markdown-media/core/src/epub.rs
ls -la /Users/seunghan/markdown-media/core/src/doc97.rs
ls -la /Users/seunghan/markdown-media/core/src/url_fetch.rs
ls -la /Users/seunghan/markdown-media/core/src/heic.rs
ls -la /Users/seunghan/markdown-media/core/src/gen_docx.rs
ls -la /Users/seunghan/markdown-media/core/src/gen_pdf.rs

# 문서
ls -la /Users/seunghan/markdown-media/docs/todo/final-gap.md
ls -la /Users/seunghan/markdown-media/docs/todo/remaining-roadmap.md
ls -la /Users/seunghan/markdown-media/docs/todo/gap-analysis.md
ls -la /Users/seunghan/markdown-media/docs/todo/competitive-gap.md
ls -la /Users/seunghan/markdown-media/docs/todo/long-term.md
```

---

## 지시 요약

다른 AI에게 위임할 때:
```
다음 체크리스트를 순서대로 검증하고 결과를 보고해줘:
1. cargo build --release 통과 확인
2. 신규 CLI 11개 각각 실행 테스트 (섹션 2)
3. 신규 포맷 5종 변환 테스트 (섹션 3)
4. MCP 서버 17개 도구 동작 확인 (섹션 5)
5. Feature flag 9개 조합 빌드 확인 (섹션 6)
6. 실패한 항목은 원인 분석 후 수정
```
