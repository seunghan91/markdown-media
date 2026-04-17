# Changelog

All notable changes to MDM are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and semantic
versioning applies to `mdm-core` (Rust crate) and `mdm-desktop` (Tauri
app); sub-packages under `packages/` version independently.

## [0.3.1] — 2026-04-17

### 핵심 주제
마크다운 뷰어·내보내기에 **LaTeX 수식 렌더링** 추가. Rust 한 곳만
수정하는 최소 의존성 경로 (`pulldown-latex` → MathML → WebView
네이티브). JS/CSS/폰트 번들 증가 0.

### 데스크톱 뷰어 (mdm-desktop)

- **LaTeX 수식 지원** — `$a^2+b^2=c^2$` / `$$…$$` 를 `pulldown-latex`
  로 MathML 변환. Tauri WebKit/WebView2 가 네이티브 렌더. KaTeX/MathJax
  의존성 **없음**. 분수·합·적분·행렬·그리스 문자·첨자 전부 지원 (≈95%
  KaTeX 호환).
- `<annotation encoding="application/x-tex">` 로 원본 LaTeX 보존 —
  마크다운 재추출·복사 시 라운드트립 가능.
- 파싱 실패 시 `<code class="math-error">` 로 폴백해 내용 유실 방지.
- **HTML 내보내기 자동 지원** — `<math>` 태그가 HTML 에 직접 들어 있어
  오프라인 브라우저에서도 JS 없이 렌더됨.

### 내부

- `desktop/src-tauri/src/markdown.rs` — `Event::InlineMath/DisplayMath`
  인터셉트 + `pulldown-latex::mathml::push_mathml` 위임.
- 유닛 테스트 4건 추가 (inline/block/invalid/plain).

### 알려진 한계

- DOCX/PDF/HWPX 내보내기 경로는 아직 수식 미지원 (별도 RFC).
- Fidelity 뷰(rhwp iframe)는 HWP native equation 을 자체 SVG 로 렌더 —
  본 변경과 무관.

---

## [0.3.0] — 2026-04-16

### 핵심 주제
HWPX 추출 품질 한 단계 상향 (문자 스타일 · 주석 · 수식 · 머리말/꼬리말) ·
뷰어에 6개 액션 버튼 + 원본 충실도 뷰 · rhwp 상호기여 체계 정립.

### HWPX 파서 (mdm-core)

- **문자 스타일 정규화** — `<hh:strikeout>` · `<hh:underline>` 판정을
  블랙리스트 → **화이트리스트** 전환. 한컴 내보내기의 `shape="3D"` 등
  placeholder 값을 본문 전체 취소선/밑줄로 오해석하던 버그 제거.
  forward-compat: 미래 placeholder도 fail-closed.
- **강조점(`<mark>`)** — OWPML `symMark` 속성(`DOT`/`CIRCLE`/`TICK`/
  `TILDE`/`MIDDLE_DOT`/`COLON`)을 `<mark>…</mark>` 래핑으로 보존. 공공
  문서 핵심 용어 신호가 마크다운에 살아남음.
- **루비 (덧말, `<hp:dutmal>`)** — subText를 `한자(hanja)` 형태의
  괄호 주석으로 보존. 한자·일본어 발음 표기가 드롭되지 않음.
- **각주 / 미주 / 머리말 / 꼬리말** — paragraph-level 컨트롤 4종을
  각각 `[각주: …]` · `[미주: …]` · `[머리말: …]` · `[꼬리말: …]`로 인라인
  확장. 기존 `[이미지: …]` placeholder 그래머와 일관.
- **수식 → LaTeX** — `<hp:equation>` 의 `<hp:script>` 내용을 단일 라인은
  `$…$`, 다중 라인은 `$$ … $$` 블록으로 출력. Hancom script 는 LaTeX와
  거의 호환되므로 대부분의 수식이 GitHub/Obsidian/LLM에서 바로 렌더/해독
  가능.
- **Depth-aware paragraph scanner** — `<hp:footNote>` / `<hp:endNote>` /
  `<hp:header>` / `<hp:footer>` 내부의 중첩 `<hp:p>` 때문에 기존 substring
  기반 `</hp:p>` 탐색이 본문을 조기 종료시키던 버그 수정. 새 헬퍼
  `find_matching_close_para()` 가 깊이 카운터로 쌍을 맞춘다.

### 보안 (mdm-core)

- 이전 릴리즈에서 추가한 ZIP 폭탄 방어(`MAX_HWPX_XML` · `MAX_HWPX_BINDATA`)
  + HWP/PDF 디컴프레션 상한이 rhwp 측에도 이식되었다 (rhwp PR #153, merged).

### 테스트 (mdm-core)

- Golden-file 회귀 테스트 스캐폴드 (`core/tests/golden_hwpx.rs`).
  `UPDATE_GOLDEN=1` 환경변수로 재생성, diff 리포트.
  10개 초기 고정점: strikeout/underline/emphasis/ruby/footnote/endnote/
  equation(inline+block)/header+footer.
- `cargo test --lib` 242 passed (기존 237 + 신규 5).
- 총 4개 스위트 합산 260 passed, 회귀 0.

### 데스크톱 뷰어 (mdm-desktop)

뷰어 헤더에 **액션 버튼 바 6종** 추가:

| 버튼 | 동작 |
|---|---|
| 📋 복사 | 마크다운 클립보드 복사 |
| 📊 통계 | 9개 지표 모달 (문자·어절·문단·헤딩·표·이미지·강조·취소선·체크리스트) |
| 🔀 비교 | 두 번째 파일 선택 → 신구대조표 side-by-side |
| 📝 메모 | 사이드카(.mdm.json) 메모 — 원본 HWP 불변 |
| ✨ AI에 묻기 | 4개 프리셋 × 4개 프로바이더 (Claude/ChatGPT/Gemini/Perplexity) |
| 💾 내보내기 | JSON · HTML · TXT |

**뷰어 모드 추가**:

- **원본** 모드 — `@rhwp/editor` iframe 임베드로 rhwp 의 픽셀-충실 렌더링
  결과를 MDM 안에서 바로 확인. 상호보완 서비스의 UI 완결.
- **스크롤 동기화** — 나란히 모드에서 렌더 ↔ 소스 판이 비율 기반으로 함께
  스크롤.

### 문서

- **RFC 001** (`docs/rfcs/001-rhwp-bridge.md`) — MDM ↔ rhwp 공식 통합
  계약 초안 (Draft). 공유 Document 모델, `BridgeBlockId`, 편집 위임
  프로토콜, 라운드트립 CI 제안. 수용 여부는 edwardkim 과의 후속 토론.

### rhwp 에 역기여 (external)

- [#153](https://github.com/edwardkim/rhwp/pull/153) — HWPX ZIP 엔트리
  디컴프레션 상한 (merged 2026-04-16).
- [#154](https://github.com/edwardkim/rhwp/pull/154) — Strikeout shape
  화이트리스트 (merged 2026-04-16).

### 내부

- `CharStyle` 테스트의 구조체 리터럴을 `..Default::default()` 로 전환,
  향후 필드 추가 시 downstream 변경 최소화.

---

## [0.1.x] — 이전

`0.1.0` / `0.1.1` 릴리즈의 CHANGELOG 항목은 `README.md` 와 커밋 로그에
분산되어 있습니다. 이 CHANGELOG.md 는 0.3.0 릴리즈부터 시작합니다.

[0.3.0]: https://github.com/seunghan91/markdown-media/releases/tag/v0.3.0
