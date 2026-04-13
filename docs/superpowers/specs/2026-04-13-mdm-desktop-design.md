# MDM Desktop — Design Specification

> **Date:** 2026-04-13
> **Author:** seunghan
> **Status:** Draft → Pending Review
> **Design System:** ios26-design-system (`~/ios26-design-system`)

---

## 1. Product Overview

### What
MDM Desktop은 HWP/HWPX/PDF/DOCX 문서를 Markdown으로 변환하고, 역으로 Markdown을 DOCX/HWPX/PDF로 내보내는 **양방향 문서 변환 데스크톱 앱**이다.

### Who
기술 지식이 없는 **일반 사무직 사용자** (공무원, 직장인 등). HWP 파일을 많이 다루지만 한글 오피스가 없거나 마크다운으로 관리하고 싶은 사람.

### Why
- 한글(HWP) 파일은 한국에서 광범위하게 사용되지만 열람/변환 도구가 부족
- 기존 `hwp2mdm` CLI는 개발자 전용 — 일반 유저가 사용 불가
- 양방향 변환으로 마크다운 기반 문서 관리와 기존 포맷 호환을 동시 해결

### Platform
- macOS + Windows 동시 지원 (Tauri 2.0 크로스 플랫폼)
- Apple HIG + ios26 디자인 시스템 기반 UI (Windows에서도 동일 CSS 적용)

---

## 2. Architecture

```
┌──────────────────────────────────────────────────┐
│  Frontend (Svelte 5 + Vite + Tailwind CSS)       │
│  ├─ 모드 탭: 변환 / 뷰어 / 배치 / 내보내기        │
│  ├─ 아이콘 사이드바 (최근/폴더/즐겨찾기/내보내기)   │
│  ├─ ios26 디자인 토큰 (CSS custom properties)     │
│  └─ Dark/Light 모드 자동 감지                     │
├──────────── Tauri IPC (invoke) ───────────────────┤
│  Backend (Rust — Tauri 2.0)                      │
│  ├─ mdm_core (기존 파서 — workspace dependency)   │
│  │   ├─ HwpParser / HwpxParser                   │
│  │   ├─ PdfParser / DocxParser                   │
│  │   ├─ IR (Intermediate Representation)         │
│  │   └─ Markdown Renderer                        │
│  ├─ 역변환 모듈 (신규)                             │
│  │   ├─ md_to_docx.rs (quick-xml → OOXML)        │
│  │   ├─ md_to_hwpx.rs (quick-xml → OWPML)        │
│  │   └─ md_to_pdf.rs (HTML render → PDF)          │
│  ├─ commands/ (Tauri IPC handlers)               │
│  │   ├─ convert.rs   — 문서→MD                    │
│  │   ├─ export.rs    — MD→문서                    │
│  │   ├─ batch.rs     — 배치 변환                   │
│  │   └─ viewer.rs    — 파일 열기/렌더링             │
│  └─ history.rs (SQLite — 변환 히스토리)            │
└──────────────────────────────────────────────────┘
```

### Key Decisions

| 결정 | 선택 | 이유 |
|------|------|------|
| 프레임워크 | Tauri 2.0 | ~10MB 바이너리, ~40MB RAM, Rust 코어 직접 연동 |
| 프론트엔드 | Svelte 5 | 기존 Group A 스택, 경량, 반응형 |
| 파서 연동 | workspace dependency | FFI/WASM 불필요, `use mdm_core::*` 직접 호출 |
| 히스토리 | SQLite (rusqlite) | 변환 이력 로컬 저장, 경량 |
| 디자인 시스템 | ios26-design-system 토큰 | Apple HIG Liquid Glass 공식 스펙 |

---

## 3. UI Layout — Hybrid Mode

### 3.1 전체 구조

```
┌─ Title Bar (unified toolbar) ──────────────────────┐
│ 🔴🟡🟢  [변환 | 뷰어 | 배치 | 내보내기]        ⚙ │
├─────┬──────────────────────────────────────────────┤
│ 📄  │                                              │
│ 📁  │          Main Content Area                   │
│ ⭐  │     (모드별 다른 컨텐츠 표시)                  │
│ 🕐  │                                              │
│ ─── │                                              │
│ 📤  │                                              │
├─────┴──────────────────────────────────────────────┤
```

- **Unified Toolbar**: 타이틀바 + 툴바 통합, Segmented Control로 모드 전환
- **Icon Sidebar**: 56px 폭 아이콘 사이드바 (접기/펼치기 가능)
- **Main Content**: 모드별 다른 뷰 렌더링

### 3.2 모드별 상세

#### 변환 모드 (Convert)
- 중앙 드래그앤드롭 존 (파일 놓으면 즉시 변환)
- Quick Action 카드 3개: 문서→MD, MD→문서, 배치 변환
- 하단 최근 변환 목록
- 변환 진행 시 인라인 프로그레스 표시

#### 뷰어 모드 (Viewer)
- 파일 더블클릭 또는 드래그로 열기
- **기본**: 단일 렌더링 뷰 (Preview.app 스타일)
- **토글**: `렌더 | 나란히 | 소스` Segmented Control
  - 렌더: 마크다운 → HTML 렌더링 (표/이미지 포함)
  - 나란히: 좌측 렌더링 + 우측 마크다운 소스 (편집 가능)
  - 소스: 마크다운 원문만 (코드 에디터 스타일)
- 마지막 선택한 뷰 모드 기억 (localStorage)
- 툴바에 `변환 ▾` 버튼 (드롭다운: MD로 저장, DOCX로 내보내기 등)

#### 배치 모드 (Batch)
- 폴더 드롭 → 파일 목록 자동 스캔
- 체크박스로 개별 선택/전체 선택
- 출력 포맷 선택 (MD / DOCX / HWPX)
- 진행률 바 + 결과 테이블 (성공/실패/건수)
- 완료 후 출력 폴더 Finder에서 열기

#### 내보내기 모드 (Export)
- MD 파일 드롭 또는 선택
- 출력 포맷: DOCX / HWPX / PDF
- 템플릿 선택 (기본, 공문서, 보고서)
- 미리보기 → 변환 → 저장

---

## 4. Design System — ios26 Tokens

> **Source:** `~/ios26-design-system/packages/tokens/src/`
> Apple Figma iOS & iPadOS 26 Community Kit에서 추출한 공식 디자인 토큰.

### 4.0 Dark/Light 모드 필수 규칙

> **⚠️ CRITICAL: 색상값 하드코딩 절대 금지**

- 모든 색상은 반드시 **CSS custom property** (`--color-*`)를 통해 참조한다.
- 컴포넌트, 스타일에서 `#1a1a1a`, `rgba(0,0,0,...)` 등 리터럴 색상값을 직접 사용하지 않는다.
- Light/Dark 전환은 `prefers-color-scheme` 미디어 쿼리로 `:root` 토큰 값만 교체하여 처리한다.
- 색상 토큰은 `tokens.css` 한 곳에서만 정의하고, 나머지 파일은 모두 `var(--color-*)` 로만 참조한다.
- Liquid Glass 배경, 그림자, 오버레이도 토큰 변수를 사용한다 (하드코딩된 rgba 금지).
- **검증 기준**: `tokens.css`를 제외한 모든 `.svelte`, `.css` 파일에서 `#` hex 색상이나 `rgba(` 리터럴이 0건이어야 한다.
- Tailwind 사용 시에도 `bg-[#xxx]` 같은 arbitrary value가 아닌 `bg-[var(--color-bg-primary)]` 형태로만 사용한다.

### 4.1 Colors

`tokens.css` 한 파일에서 CSS custom properties 정의. `prefers-color-scheme`로 자동 전환.
ios26 `colors.json`에서 4가지 모드 (Light, Dark, IC Light, IC Dark) 토큰을 그대로 매핑.

```css
/* tokens.css — 유일한 색상 정의 파일 */
:root {
  /* Accent (from colors.json accents.blue) */
  --color-accent: #0088ff;
  --color-accent-hover: #0091ff;

  /* Labels (from colors.json labels) */
  --color-label-primary: rgba(0,0,0,1);
  --color-label-secondary: rgba(60,60,67,0.6);
  --color-label-tertiary: rgba(60,60,67,0.3);
  --color-label-quaternary: rgba(60,60,67,0.18);

  /* Backgrounds (from colors.json backgrounds) */
  --color-bg-primary: #ffffff;
  --color-bg-secondary: #f2f2f7;
  --color-bg-tertiary: #ffffff;
  --color-bg-elevated: #ffffff;
  --color-bg-secondary-elevated: #f2f2f7;

  /* Fills (from colors.json fills) */
  --color-fill-primary: rgba(120,120,128,0.2);
  --color-fill-secondary: rgba(120,120,128,0.16);
  --color-fill-tertiary: rgba(118,118,128,0.12);

  /* Separators */
  --color-separator: #c6c6c8;
  --color-separator-non-opaque: rgba(0,0,0,0.12);

  /* Grays */
  --color-gray: #8e8e93;
  --color-gray4: #d1d1d6;
  --color-gray5: #e5e5ea;
  --color-gray6: #f2f2f7;

  /* Status */
  --color-success: #34c759;
  --color-warning: #ff9f0a;
  --color-error: #ff383c;

  /* Liquid Glass (from materials.json) */
  --glass-sidebar-bg: rgba(250,250,250,0.7);
  --glass-sidebar-shadow: rgba(0,0,0,0.08);
  --glass-toolbar-bg: rgba(245,245,245,0.6);
  --glass-segment-bg: rgba(247,247,247,1);
}

@media (prefers-color-scheme: dark) {
  :root {
    --color-accent: #0091ff;
    --color-label-primary: rgba(255,255,255,1);
    --color-label-secondary: rgba(235,235,245,0.7);
    --color-label-tertiary: rgba(235,235,245,0.3);
    --color-label-quaternary: rgba(235,235,245,0.16);
    --color-bg-primary: #000000;
    --color-bg-secondary: #1c1c1e;
    --color-bg-tertiary: #2c2c2e;
    --color-bg-elevated: #1c1c1e;
    --color-bg-secondary-elevated: #2c2c2e;
    --color-fill-primary: rgba(120,120,128,0.36);
    --color-fill-secondary: rgba(120,120,128,0.32);
    --color-fill-tertiary: rgba(118,118,128,0.24);
    --color-separator: #38383a;
    --color-separator-non-opaque: rgba(255,255,255,0.17);
    --color-gray: #8e8e93;
    --color-gray4: #3a3a3c;
    --color-gray5: #2c2c2e;
    --color-gray6: #1c1c1e;
    --color-success: #30d158;
    --color-warning: #ff9f0a;
    --color-error: #ff4245;
    --glass-sidebar-bg: rgba(0,0,0,0.8);
    --glass-sidebar-shadow: rgba(0,0,0,0.3);
    --glass-toolbar-bg: rgba(0,0,0,0.6);
    --glass-segment-bg: rgba(0,0,0,0.6);
  }
}
```

### 4.2 Typography

SF Pro 폰트 패밀리 + 시스템 폰트 폴백.

```css
:root {
  --font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", system-ui, sans-serif;
  --font-mono: "SF Mono", SFMono-Regular, Menlo, Consolas, monospace;

  /* Type Scale (from typography.json) */
  --text-large-title: 34px/41px;   /* weight: Regular, Bold */
  --text-title1: 28px/34px;
  --text-title2: 22px/28px;
  --text-title3: 20px/25px;
  --text-headline: 17px/22px;      /* weight: Semibold */
  --text-body: 17px/22px;
  --text-callout: 16px/21px;
  --text-subheadline: 15px/20px;
  --text-footnote: 13px/18px;
  --text-caption1: 12px/16px;
  --text-caption2: 11px/13px;
}
```

### 4.3 Spacing & Radius

8pt 그리드 시스템.

```css
:root {
  /* Spacing (from spacing.json) */
  --space-0: 0px;
  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;

  /* Inset */
  --inset-xs: 8px;
  --inset-sm: 12px;
  --inset-md: 16px;
  --inset-lg: 20px;

  /* Radius */
  --radius-xs: 4px;
  --radius-sm: 8px;
  --radius-md: 10px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-full: 9999px;   /* pill */

  /* Semantic Radius */
  --radius-card: 12px;
  --radius-button: 12px;
  --radius-menu: 14px;
  --radius-popover: 14px;
}
```

### 4.4 Liquid Glass Materials

사이드바, 툴바, 모드 탭에 적용. **모든 색상은 `var(--glass-*)` 토큰 참조 — rgba 하드코딩 금지.**

```css
/* Liquid Glass — Sidebar (large, frost 14px) */
.liquid-glass-sidebar {
  background: var(--glass-sidebar-bg);
  backdrop-filter: blur(14px) saturate(180%);
  -webkit-backdrop-filter: blur(14px) saturate(180%);
  box-shadow: 0 0 40px var(--glass-sidebar-shadow);
}

/* Liquid Glass — Toolbar (medium, frost 12px) */
.liquid-glass-toolbar {
  background: var(--glass-toolbar-bg);
  backdrop-filter: blur(12px) saturate(180%);
  -webkit-backdrop-filter: blur(12px) saturate(180%);
}

/* Liquid Glass — Segmented Control (small, pill) */
.liquid-glass-segment {
  background: var(--glass-segment-bg);
  border-radius: var(--radius-full);
}
.liquid-glass-segment .active {
  background: var(--color-accent);
  border-radius: var(--radius-full);
}
```

> **Note:** Light↔Dark 전환 시 `.liquid-glass-*` 클래스는 코드 변경 없이 `tokens.css`의 `--glass-*` 변수값만 바뀌어 자동 적용된다.

### 4.5 Animations

```css
:root {
  /* Duration (from animations.json) */
  --duration-micro: 0.1s;
  --duration-fast: 0.2s;
  --duration-normal: 0.3s;
  --duration-slow: 0.4s;

  /* Easing */
  --ease-default: cubic-bezier(0.25, 0.46, 0.45, 0.94);
  --ease-spring-snappy: cubic-bezier(0.34, 1.56, 0.64, 1.0);
  --ease-spring-gentle: cubic-bezier(0.25, 0.46, 0.45, 0.94);

  /* Semantic */
  --duration-tab-transition: 0.3s;
  --duration-liquid-glass-morph: 0.35s;
  --duration-page-transition: 0.35s;
}
```

---

## 5. Component Specifications

### 5.1 Unified Toolbar (38px)
- Traffic lights 통합 (macOS), 빈 영역 (Windows)
- 중앙: Segmented Control (변환/뷰어/배치/내보내기) — Liquid Glass small pill
- 우측: 설정 아이콘 버튼
- 전체 영역 드래그 가능 (data-tauri-drag-region)

### 5.2 Icon Sidebar (56px)
- Liquid Glass large 배경
- 아이콘 버튼: 36×36px, 8px radius
- 선택 상태: accent color 배경 + 0.33 opacity
- 구분선으로 네비게이션 / 액션 영역 분리
- 접기: 0px (사이드바 숨김), 토글 단축키 ⌘\

### 5.3 Drop Zone
- `border: 2px dashed var(--color-gray4)`, radius 16px
- 드래그 hover 시: `border-color: var(--color-accent)`, 배경 accent 10% opacity
- 파일 아이콘 + 안내 텍스트 + "파일 선택" 버튼
- 지원 포맷 레이블: `HWP · PDF · DOCX ↔ Markdown`

### 5.4 Quick Action Cards
- 3개 카드: 문서→MD (blue), MD→문서 (green), 배치 (orange)
- Gradient 배경 + 1px 테두리 (accent 색상 30% opacity)
- Radius: 10px (card semantic)
- Hover: scale(1.02) + shadow 강화

### 5.5 File List Row
- Height: 44px (HIG minimum)
- 파일명 (body, semibold) + 날짜/크기 (caption1, secondary)
- 선택 상태: accent 배경 20% opacity, radius 6px
- 우클릭 컨텍스트 메뉴: 열기, 변환, 내보내기, 삭제

### 5.6 Viewer Toolbar
- 뒤로/앞으로 버튼 + 파일명 (center) + 검색 + 변환 드롭다운
- `렌더 | 나란히 | 소스` segmented control
- Liquid Glass medium 배경

### 5.7 Progress Indicator
- 배치 모드: 전체 진행률 바 (accent color, rounded)
- 개별 파일: 인라인 상태 아이콘 (⏳ 진행중 / ✅ 완료 / ❌ 실패)

---

## 6. Tauri IPC Commands

```rust
// convert.rs
#[tauri::command]
async fn convert_file(path: String, format: String) -> Result<ConvertResult, String>

#[tauri::command]
async fn convert_text(content: String, from_format: String) -> Result<String, String>

// export.rs
#[tauri::command]
async fn export_to_docx(markdown: String, template: String, output: String) -> Result<(), String>

#[tauri::command]
async fn export_to_hwpx(markdown: String, template: String, output: String) -> Result<(), String>

#[tauri::command]
async fn export_to_pdf(markdown: String, output: String) -> Result<(), String>

// batch.rs
#[tauri::command]
async fn batch_convert(paths: Vec<String>, format: String, output_dir: String) -> Result<BatchResult, String>

// viewer.rs
#[tauri::command]
async fn open_file(path: String) -> Result<ViewerData, String>

#[tauri::command]
async fn get_markdown_source(path: String) -> Result<String, String>

// history.rs
#[tauri::command]
async fn get_history(limit: usize) -> Result<Vec<HistoryEntry>, String>
```

### Data Types

```rust
struct ConvertResult {
    markdown: String,
    images: Vec<ExtractedImage>,
    metadata: DocumentMetadata,
}

struct ViewerData {
    html: String,           // 렌더링된 HTML
    markdown: String,       // 마크다운 소스
    metadata: DocumentMetadata,
}

struct BatchResult {
    total: usize,
    success: usize,
    failed: usize,
    results: Vec<BatchItemResult>,
}

struct HistoryEntry {
    id: i64,
    file_name: String,
    file_path: String,
    direction: String,      // "to_md" | "from_md"
    output_format: String,
    created_at: String,
    status: String,
}
```

---

## 7. File Structure

```
markdown-media/
├─ desktop/                         # Tauri 앱 (신규 디렉토리)
│  ├─ src/                          # Svelte 5 프론트엔드
│  │  ├─ lib/
│  │  │  ├─ components/
│  │  │  │  ├─ Sidebar.svelte       # 아이콘 사이드바
│  │  │  │  ├─ Toolbar.svelte       # Unified 툴바 + 모드 탭
│  │  │  │  ├─ DropZone.svelte      # 드래그앤드롭 존
│  │  │  │  ├─ QuickActions.svelte  # Quick Action 카드
│  │  │  │  ├─ FileList.svelte      # 파일 목록
│  │  │  │  ├─ ProgressBar.svelte   # 진행률 바
│  │  │  │  └─ ViewerToggle.svelte  # 렌더/나란히/소스 토글
│  │  │  ├─ stores/
│  │  │  │  ├─ app.ts               # 현재 모드, 사이드바 상태
│  │  │  │  ├─ history.ts           # 변환 히스토리
│  │  │  │  └─ viewer.ts            # 뷰어 상태 (뷰 모드, 파일)
│  │  │  └─ styles/
│  │  │     ├─ tokens.css           # ios26 디자인 토큰
│  │  │     ├─ liquid-glass.css     # Liquid Glass 유틸리티
│  │  │     └─ global.css           # 리셋, 폰트, 기본 스타일
│  │  ├─ routes/
│  │  │  ├─ +layout.svelte          # 사이드바 + 툴바 레이아웃
│  │  │  ├─ convert/+page.svelte    # 변환 모드
│  │  │  ├─ viewer/+page.svelte     # 뷰어 모드
│  │  │  ├─ batch/+page.svelte      # 배치 모드
│  │  │  └─ export/+page.svelte     # 내보내기 모드
│  │  └─ app.html
│  ├─ src-tauri/
│  │  ├─ src/
│  │  │  ├─ main.rs                 # Tauri entry point
│  │  │  ├─ commands/
│  │  │  │  ├─ mod.rs
│  │  │  │  ├─ convert.rs
│  │  │  │  ├─ export.rs
│  │  │  │  ├─ batch.rs
│  │  │  │  └─ viewer.rs
│  │  │  ├─ history.rs              # SQLite 히스토리
│  │  │  └─ export/                 # 역변환 모듈
│  │  │     ├─ mod.rs
│  │  │     ├─ md_to_docx.rs
│  │  │     ├─ md_to_hwpx.rs
│  │  │     └─ md_to_pdf.rs
│  │  ├─ Cargo.toml                 # mdm-core = { path = "../../core" }
│  │  ├─ tauri.conf.json
│  │  └─ capabilities/
│  │     └─ default.json            # fs, dialog, shell 권한
│  ├─ package.json
│  ├─ vite.config.ts
│  ├─ svelte.config.js
│  └─ tailwind.config.js
├─ core/                            # 기존 Rust 파서 (변경 없음)
│  ├─ Cargo.toml
│  └─ src/
└─ ...
```

---

## 8. Reverse Conversion (Export) Scope

### Phase 1 (MVP)

| Input | Output | 구현 방식 | 난이도 |
|-------|--------|-----------|--------|
| Markdown | DOCX | Rust: MD 파싱 → quick-xml로 OOXML 생성 | Medium |
| Markdown | PDF | HTML 렌더링 → Tauri webview print-to-PDF | Low |

### Phase 2

| Input | Output | 구현 방식 | 난이도 |
|-------|--------|-----------|--------|
| Markdown | HWPX | Rust: MD 파싱 → quick-xml로 OWPML XML 생성 | High |
| Markdown | PPTX | 향후: 슬라이드 분할 로직 + OOXML Presentation | High |

### 템플릿 시스템
- `기본` — 깔끔한 문서 스타일
- `공문서` — 한국 정부 공문서 양식 (제목, 수신, 참조 등)
- `보고서` — 비즈니스 보고서 (목차, 표지 포함)
- 사용자 정의 템플릿 (향후)

---

## 9. Keyboard Shortcuts

| 단축키 | 기능 |
|--------|------|
| ⌘O / Ctrl+O | 파일 열기 |
| ⌘E / Ctrl+E | 내보내기 |
| ⌘1~4 / Ctrl+1~4 | 모드 전환 (변환/뷰어/배치/내보내기) |
| ⌘\ / Ctrl+\ | 사이드바 토글 |
| ⌘, / Ctrl+, | 설정 |
| ⌘D / Ctrl+D | 뷰어 모드 전환 (렌더→나란히→소스) |
| Space | 뷰어에서 선택 파일 미리보기 |

---

## 10. Build & Distribution

```bash
# Development
cd desktop
pnpm install
pnpm tauri dev

# Build
pnpm tauri build
# → macOS: .dmg + .app
# → Windows: .exe + .msi
```

### Binary Size Target
- macOS: ~15MB (.dmg)
- Windows: ~20MB (.msi)
- RAM: ~50MB 이하

### Code Signing
- macOS: Apple Developer ID (`74PTNNLD4P` — iyu974895)
- Windows: Self-signed (추후 EV 인증서)

---

## 11. Out of Scope (YAGNI)

- 클라우드 동기화 / 계정 시스템
- 실시간 협업 편집
- OCR (이미지 내 텍스트 인식)
- 플러그인 시스템
- 모바일 지원 (iOS/Android)
- 자동 업데이트 (Phase 2에서 Tauri updater 추가 가능)

---

## 12. Success Criteria

- [ ] HWP/HWPX/PDF/DOCX → Markdown 변환 성공률 ≥95%
- [ ] Markdown → DOCX 역변환 동작
- [ ] 4개 모드 전환 동작 (변환/뷰어/배치/내보내기)
- [ ] 뷰어 3가지 토글 (렌더/나란히/소스) 동작
- [ ] Dark/Light 모드 자동 전환
- [ ] macOS + Windows 빌드 성공
- [ ] 앱 시작 → 파일 드롭 → 변환 완료 3클릭 이내

---

## 13. Implementation Plan — 작업 분담 & 진행 상태

> **Codex (GPT-5.4)** 와 **Claude Opus** 가 병렬로 작업 진행.
> 디렉토리 구조는 Codex가 생성 완료 (파일 미생성 상태).

### Wave 1: 프로젝트 초기화 + 디자인 토큰

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 1.1 | `desktop/` 디렉토리 구조 생성 | Codex | [완료] | |
| 1.2 | `package.json`, `vite.config.ts`, `svelte.config.js`, `tailwind.config.js` | Codex | [완료] | Tauri + Svelte 5 빌드 설정 |
| 1.3 | `src-tauri/` Rust 백엔드 파일 | Codex | [대기] | **아직 미생성** — Cargo.toml, main.rs, commands 필요 |
| 1.4 | `src/lib/styles/tokens.css` — ios26 디자인 토큰 | Codex | [완료] | Codex가 간소화 버전으로 덮어씀 |
| 1.5 | `src/lib/styles/liquid-glass.css` — Liquid Glass | Codex | [완료] | Codex가 gradient 기반으로 덮어씀 |
| 1.6 | `src/lib/styles/global.css` — 리셋, 유틸리티 | Codex | [완료] | Tailwind base 포함 |
| 1.7 | `src/app.html` + `app.d.ts` + `types.ts` + `utils/ipc.ts` | Codex | [완료] | 타입 정의 + IPC 래퍼 포함 |

### Wave 2: 레이아웃 쉘 + 라우팅

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 2.1 | `src/routes/+layout.svelte` — 앱 쉘 레이아웃 | Codex | [완료] | grid 기반 sidebar + content |
| 2.2 | `src/lib/components/Sidebar.svelte` — 아이콘 사이드바 | Codex | [완료] | 접기/펼치기, 모드 네비게이션 |
| 2.3 | `src/lib/components/Toolbar.svelte` — Unified 툴바 | Codex | [완료] | Segmented Control + settings |
| 2.4 | `src/lib/stores/app.ts` — 앱 상태 | Codex | [완료] | appMode, sidebarCollapsed |
| 2.5 | 4개 모드 라우트 페이지 | Codex | [완료] | convert, viewer, batch, export |

### Wave 3: 변환 모드 (핵심 기능)

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 3.1 | `src/lib/components/DropZone.svelte` | Codex | [완료] | 드래그앤드롭 UI |
| 3.2 | `src/lib/components/QuickActions.svelte` | Codex | [완료] | 3개 액션 카드 |
| 3.3 | `src-tauri/src/commands/convert.rs` — IPC 변환 커맨드 | Codex | [완료] | mdm_core API 정확히 호출 |
| 3.4 | `src-tauri/src/main.rs` — Tauri entry + command 등록 | Codex | [완료] | HistoryStore 관리 포함 |
| 3.5 | `src/routes/convert/+page.svelte` — 변환 모드 UI | Codex | [완료] | |

### Wave 4: 뷰어 모드

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 4.1 | `src/lib/components/ViewerToggle.svelte` — 렌더/나란히/소스 | Codex | [완료] | |
| 4.2 | `src/lib/stores/viewer.ts` — 뷰어 상태 | Codex | [완료] | localStorage 기억 |
| 4.3 | `src-tauri/src/commands/viewer.rs` — 파일 열기/렌더링 IPC | Codex | [완료] | open_file + get_markdown_source |
| 4.4 | `src/routes/viewer/+page.svelte` — 뷰어 모드 UI | Codex | [완료] | |

### Wave 5: 배치 + 내보내기 모드

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 5.1 | `src/lib/components/FileList.svelte` + `ProgressBar.svelte` | Codex | [완료] | |
| 5.2 | `src-tauri/src/commands/batch.rs` — 배치 IPC | Codex | [완료] | walkdir 기반 폴더 스캔 |
| 5.3 | `src/routes/batch/+page.svelte` — 배치 모드 UI | Codex | [완료] | |
| 5.4 | `src-tauri/src/commands/export.rs` — 역변환 IPC | Codex | [완료] | |
| 5.5 | `src-tauri/src/export/md_to_docx.rs` + hwpx + pdf | Codex | [완료] | quick-xml, printpdf, zip |
| 5.6 | `src/routes/export/+page.svelte` — 내보내기 모드 UI | Codex | [완료] | |

### Wave 6: 히스토리 + 키보드 단축키 + 빌드 검증

| # | 작업 | 담당 | 상태 | 비고 |
|---|------|------|------|------|
| 6.1 | `src-tauri/src/history.rs` — SQLite 히스토리 | Codex | [완료] | HistoryStore + rusqlite |
| 6.2 | `src/lib/stores/history.ts` — 프론트엔드 히스토리 store | Codex | [완료] | IPC refreshHistory |
| 6.3 | 키보드 단축키 바인딩 (⌘O, ⌘E, ⌘1~4 등) | - | [대기] | 아직 미구현 |
| 6.4 | 빌드 검증 | Claude | [완료] | Rust `cargo check` 0 errors, Svelte 0 errors |
| 6.5 | Dark/Light 모드 토큰 하드코딩 검증 | - | [대기] | Codex 버전에 일부 하드코딩 있음 — 후속 정리 필요 |

### 작업 분담 원칙

- **Codex**: Rust 백엔드 (Tauri commands, mdm-core 연동, 역변환 모듈), 빌드 설정
- **Claude**: Svelte 프론트엔드 (컴포넌트, 스타일, 라우트), ios26 디자인 토큰, Apple HIG UI
- **공동**: 빌드 검증, 통합 테스트, 하드코딩 검증
