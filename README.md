# MDM (Markdown-Media) — The Fastest Document-to-AI Engine

[English](#why-mdm) | [한국어](#이-프로젝트는-왜-만들었나요) | [日本語](#日本語) | [中文](#中文)

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/Core-Rust_21K_LOC-orange)
![Python](https://img.shields.io/badge/pip_install-mdm--core-blue)
![Tests](https://img.shields.io/badge/tests-159_passed-green)

Converts **HWP, HWPX, PDF, DOCX** into clean **Markdown + Media Bundles** optimized for AI/LLM consumption.

**HWP, HWPX, PDF, DOCX** 파일을 깨끗한 **Markdown + 미디어 번들**로 변환합니다. AI/LLM 파이프라인에 최적화되어 있습니다.

Built in Rust — **23%p higher quality than Pandoc, 17%p higher than AI-based Marker**, and 10-100x faster.

Rust로 작성되어 **Pandoc보다 23%p, AI 기반 Marker보다 17%p 높은 품질**을 달성하면서도 10-100배 빠릅니다.

```
contract.hwp  ──┐
report.pdf    ──┼──▶  MDM Engine (Rust)  ──▶  Clean Markdown + Media Bundle
proposal.docx ──┘
```

> **Scope note**: MDM is a **one-way pipeline — existing documents → Markdown**. Converting Markdown back to HWP/DOCX is out of scope. The mission is to unlock the content already trapped in proprietary formats, not to author new documents in those formats.
>
> **방향 명시**: MDM은 **기존 문서 → Markdown 단방향 파이프라인**입니다. Markdown을 HWP/DOCX로 역변환하는 기능은 이 프로젝트의 범위 밖입니다. 독점 포맷에 갇혀 있는 콘텐츠를 해방하는 것이 목적이며, 그 포맷으로 새 문서를 저작하는 도구가 아닙니다.

---

## 5분 만에 시작하기 / Quick Start

### Method 1: Python (Easiest)

```bash
pip install mdm-core
```

```python
import mdm_core

# Convert a single file to Markdown
md = mdm_core.convert("document.hwp")
print(md)

# PDF, DOCX, HWPX all supported
md = mdm_core.convert("report.pdf")
md = mdm_core.convert("proposal.docx")
md = mdm_core.convert("notice.hwpx")
```

That's it. That's all you need.

### Method 2: Command Line (CLI)

```bash
# Build Rust core (one-time)
cd core && cargo build --release

# Convert
./target/release/hwp2mdm contract.hwp -o output/
./target/release/hwp2mdm report.pdf -o output/
./target/release/hwp2mdm proposal.docx -o output/
```

`output/` contains `.mdx` (Markdown) + `.mdm` (metadata JSON) files.

### Method 3: Web Viewer (No install)

Open `viewer/index.html` in a browser and drag-and-drop your files.

---

## Why Markdown?

**Markdown uses 34-38% fewer tokens than JSON and ~10% fewer than YAML across all major LLMs.**

Source: [improvingagents.com benchmark](https://improvingagents.com) (tested on GPT, Llama, Gemini)

GitHub SpecKit (2025) adopted Markdown as the foundation for AI-driven development ([dev.to](https://dev.to)).

**Why does this matter?**

- **Lower cost** — Fewer tokens per API call means lower LLM bills
- **Faster processing** — Less to parse, faster inference
- **Fits more in context window** — Same context window holds 34-38% more content in Markdown vs JSON

For RAG pipelines, document Q&A, and agent workflows, Markdown is the optimal serialization format.

마크다운은 JSON 대비 34-38% 적은 토큰을 사용하며, YAML 대비 약 10% 적습니다. 이는 LLM API 비용 절감, 더 빠른 처리, 그리고 동일한 컨텍스트 윈도우에 더 많은 콘텐츠를 담을 수 있음을 의미합니다.

---

## Supported Formats / 어떤 파일을 변환할 수 있나요?

MDM converts two categories of files: **documents** (meant for reading) and **data containers** (meant for structured information). Both are converted to Markdown so that AI/LLM can consume them uniformly.

MDM은 두 가지 종류의 파일을 변환합니다: **문서** (읽기 위한 것)와 **데이터 컨테이너** (구조화된 정보를 담는 것). 둘 다 마크다운으로 변환하여 AI/LLM이 동일한 형태로 소비할 수 있게 합니다.

### Documents / 문서 (preserving structure + formatting)

| Format | Extension | Description | Supported Features |
|--------|-----------|-------------|--------------------|
| **HWP** | `.hwp` | Korean word processor (Hangul) | Text, tables, bold/italic, footnotes, images, encryption, legal doc structure |
| **HWPX** | `.hwpx` | Hangul (XML-based) | Text, tables, formatting, outline headings |
| **PDF** | `.pdf` | Universal document | Text, heading hierarchy (H1-H4), tables, bold/italic, 2-column layout, header/footer removal |
| **DOCX** | `.docx` | Microsoft Word | Text, headings, lists, tables (merged cells), hyperlinks, footnotes, blockquotes, images |
| **PPTX** | `.pptx` | PowerPoint presentations | Slide text, titles, speaker notes, per-slide sections |
| **HTML** | `.html` `.htm` | Web pages | Headings, links, images, tables, lists, code blocks, strip scripts |

### Data Containers / 데이터 컨테이너 (extracting text + tables for AI)

These formats aren't traditional "documents" — they hold structured data (spreadsheets, tables, logs). MDM extracts their textual content as Markdown tables so AI can read, search, and reason over the data.

이 포맷들은 전통적인 "문서"가 아니라 구조화된 데이터(스프레드시트, 테이블, 로그)를 담는 컨테이너입니다. MDM은 이들의 텍스트 콘텐츠를 마크다운 테이블로 추출하여 AI가 데이터를 읽고 검색하고 추론할 수 있게 합니다.

| Format | Extension | Description | What MDM extracts |
|--------|-----------|-------------|-------------------|
| **XLSX** | `.xlsx` `.xls` | Excel spreadsheets | All sheets as Markdown tables, sheet names as headings |
| **CSV/TSV** | `.csv` `.tsv` | Tabular data | Pipe table with auto-detected delimiter |
| **TXT** | `.txt` `.log` | Plain text / logs | Text with encoding detection (UTF-8, EUC-KR) |

---

## Benchmarks / 다른 도구와 비교

### DOCX Conversion Quality (39 feature tests)

```
MDM (Rust)  ████████████████████████████████████████ 100% (39/39)
Pandoc      ██████████████████████████████           77% (30/39)
```

MDM-only features: GFM tables, hyperlinks, nested lists, Korean numbering (가나다)

### PDF Conversion Quality (29 feature tests)

```
MDM (Rust)  ███████████████████████████████████████  93% (27/29)
Marker (AI) ████████████████████████████             76% (22/29)
pdftotext   █████████████████                        45% (13/29)
```

Why MDM beats AI-based Marker: accurate H1-H4 heading detection, inline bold/italic, metadata preservation

### HWP Conversion

```
MDM (Rust)  ████████████████████████████████████████ No competition
(The world's only open-source HWP → Markdown converter)
(세계 유일의 오픈소스 HWP → Markdown 변환기)
```

### Speed

| Tool | DOCX | PDF |
|------|:----:|:---:|
| **MDM** | **14ms** | **20ms** |
| Pandoc | 64ms | - |
| Marker (AI+GPU) | - | ~7,000ms |

383-page PDF in 5.6 seconds (Rust + Rayon parallel processing).

---

## Why MDM?

### The Gap / 기존 도구와의 차이

```
Existing tools:  Document → Text only (media discarded)
기존 도구:        문서 → 텍스트만 추출 (미디어 폐기)

MDM:             Document → Markdown + Media Bundle (media preserved + indexed)
MDM:             문서 → 마크다운 + 미디어 번들 (미디어 보존 + 인덱싱)
```

### 5 Differentiators / 5가지 차별점

1. **Integrated Media Manifest** / 통합 미디어 매니페스트
   Asset index + deduplication + metadata. No competitor has this.
   에셋 인덱스 + 중복 제거 + 메타데이터. 어떤 경쟁 도구도 이 기능이 없습니다.

2. **Type-specific Media Syntax** / 타입별 미디어 참조 문법
   `@[[image]]` `~[[table]]` `%[[video]]` `$[[equation]]` `^[[audio]]` `&[[embed]]`
   6종의 미디어 타입에 대한 전용 참조 문법을 제공합니다.

3. **Content-addressable Storage** / 컨텐츠 주소 기반 저장
   Hash-based filenames for automatic deduplication.
   해시 기반 파일명으로 자동 중복 제거됩니다.

4. **AI Auto-classification** / AI 자동 분류
   Extracted images auto-tagged as chart/photo/scan/signature.
   추출된 이미지를 차트/사진/스캔/서명으로 자동 분류합니다.

5. **HWP Native + Global** / HWP 네이티브 + 글로벌
   Only tool that natively parses Korean HWP, plus PDF/DOCX.
   HWP를 네이티브로 파싱하는 유일한 도구이면서, PDF/DOCX도 지원합니다.

### Position / 포지셔닝

> MDM is a "Document-to-AI Infrastructure Layer" — not just a converter, but infrastructure that structures every component of a document for AI consumption.

> MDM은 "Document-to-AI 인프라 레이어" — 단순 변환기가 아니라, 문서의 모든 구성 요소를 AI가 소비할 수 있는 형태로 구조화하는 인프라입니다.

### Conversion Direction / 변환 방향

MDM deliberately covers **only one direction**:

```
HWP / HWPX / PDF / DOCX / PPTX / XLSX  ──▶  Markdown (+ Media Bundle)
```

Markdown → HWP or Markdown → DOCX is **not a goal**. Tools like Pandoc already handle document authoring workflows. MDM's focus is **reading and extracting** — liberating content locked inside proprietary formats so that AI and humans can work with it.

MDM은 의도적으로 **단방향만 지원**합니다:

```
HWP / HWPX / PDF / DOCX / PPTX / XLSX  ──▶  Markdown (+ 미디어 번들)
```

Markdown → HWP 또는 Markdown → DOCX 변환은 **이 프로젝트의 목표가 아닙니다**. 문서 저작 워크플로는 Pandoc 등 기존 도구가 이미 담당합니다. MDM의 집중 영역은 **읽기와 추출** — 독점 포맷 안에 갇힌 콘텐츠를 AI와 사람이 활용할 수 있도록 해방하는 것입니다.

---

## AI Pipeline Integration / AI 파이프라인에서 사용하기

### With LangChain

```bash
pip install mdm-core[langchain]
```

```python
from mdm_core.langchain import MDMLoader

# Load a single file
loader = MDMLoader("contract.hwp")
docs = loader.load()

# Load an entire folder (auto-detects HWP, PDF, DOCX)
loader = MDMLoader("./documents/")
docs = loader.load()

# Connect directly to LangChain RAG pipeline
from langchain_openai import ChatOpenAI
from langchain.chains import RetrievalQA
from langchain_community.vectorstores import FAISS
from langchain_openai import OpenAIEmbeddings

vectorstore = FAISS.from_documents(docs, OpenAIEmbeddings())
qa = RetrievalQA.from_chain_type(ChatOpenAI(), retriever=vectorstore.as_retriever())
answer = qa.invoke("What are the termination conditions in this contract?")
```

### With LlamaIndex

```bash
pip install mdm-core[llamaindex]
```

```python
from mdm_core.llamaindex import MDMReader
from llama_index.core import VectorStoreIndex

reader = MDMReader()
docs = reader.load_data(["notice.hwpx", "law.pdf", "contract.docx"])

index = VectorStoreIndex.from_documents(docs)
engine = index.as_query_engine()
response = engine.query("What are the eligibility requirements?")
```

---

## Conversion Output Example / 변환 결과는 어떻게 생겼나요?

### Input: Government Youth Intern Recruitment Notice (HWPX)

### Output:

```markdown
---
format: hwpx
version: "1.0"
sections: 2
---

**행정안전부 공고 제2025 – 2377호**

2026년 제1기 행정안전부 청년인턴 채용 공고

| **근무기관(지역)** | **지원코드** | **채용분야** | **선발인원** |
| --- | --- | --- | --- |
| 행정안전부 본부(세종) | **인턴01** | **행정** | **16** |
| | **인턴02** | **홍보** | **7** |
| 지방자치인재개발원(전북 완주) | **인턴06** | **행정** | **12** |
...
```

Tables, bold, and document structure are fully preserved.

---

## MDM Media Reference Syntax / MDM 미디어 참조 문법

MDM provides 6 dedicated syntax prefixes for embedding media in Markdown. The prefix symbol declares the media type.

`[[]]` double brackets mark "this is MDM media", and the preceding symbol declares the type.

```
@[[photo.jpg]]              Image
~[[table_01.svg]]           Table/Chart
&[[youtube:dQw4w9WgXcQ]]    Embed (external service)
%[[intro.mp4]]              Video
$[[E = mc^2]]               Equation (LaTeX)
^[[podcast.mp3]]            Audio
```

### Why These Symbols?

| Symbol | Type | Frequency | Keyboard | Mnemonic |
|:------:|------|:---------:|:--------:|----------|
| `@` | Image | 76% | Shift+2 | @=at (location) |
| `~` | Table/Chart | 39% | Shift+\` | ~=wave |
| `&` | Embed | 24% | Shift+7 | &=link |
| `%` | Video | 10% | Shift+5 | %=progress |
| `$` | Equation | 5% | Shift+4 | $=LaTeX convention |
| `^` | Audio | 2% | Shift+6 | ^=sound wave |

More frequent types are assigned to easier-to-reach keys (RISC-V encoding principle).

### Attributes

All types support `| attribute` options:

```markdown
@[[photo.jpg | w=800 center caption="Seoul night view"]]
%[[demo.mp4 | autoplay muted loop]]
&[[youtube:id | w=100%]]
```

### Sidecar Presets

Reference resources pre-defined in the `.mdm` manifest by name:

```markdown
@[[logo:header]]        logo resource's header preset from .mdm
#[[budget-table]]       budget-table defined in .mdm
```

### Conversion Output Bundle

When MDM converts HWP/PDF/DOCX:

```
output/
├── index.md              Body text (with MDM references)
│   @[[image_001]]        ← auto-numbered
│   ~[[table_001]]
├── manifest.mdm          Resource index (YAML)
│   image_001: assets/images/image_001.png
│   table_001: assets/tables/table_001.svg
└── assets/
    ├── images/
    │   ├── image_001.png
    │   └── image_002.jpg
    └── tables/
        └── table_001.svg
```

Auto-numbering rule: `{type}_{appearance_order:3digits}` (page order > top-to-bottom > left-to-right)

### No Conflict with Standard Markdown

`[[]]` double brackets are the MDM marker. Only the `symbol + [[` pattern is recognized as MDM, so standalone symbol usage (`~strikethrough~`, `$100`) is never misinterpreted.

```
~~strikethrough~~       ← Markdown strikethrough (~~+text+~~)
~[[table.svg]]          ← MDM table/chart        (~+[[, single tilde)

$x^2$                   ← LaTeX inline            ($+equation+$)
$[[E=mc^2]]             ← MDM equation            ($+[[, no closing $)

![alt](src)             ← Standard image          (![ prefix)
@[[image.jpg]]          ← MDM image               (@[[ prefix)

[^1]                    ← Footnote                ([^ prefix)
^[[audio.mp3]]          ← MDM audio               (^[[ prefix)
```

All 6 symbols verified conflict-free with CommonMark/GFM/LaTeX.

Full syntax spec: [docs/MDM_SYNTAX_SPEC.md](docs/MDM_SYNTAX_SPEC.md)

---

## Project Structure / 프로젝트 구조

```
markdown-media/
├── core/                    # [Rust] Core parser engine (21,000+ LOC)
│   └── src/
│       ├── hwp/             #   HWP parser (OLE, encryption, legal docs)
│       ├── hwpx/            #   HWPX parser (XML)
│       ├── pdf/             #   PDF parser (layout, heading detection)
│       ├── docx/            #   DOCX parser (hyperlinks, footnotes)
│       ├── wasm.rs          #   WASM bindings (browser)
│       └── main.rs          #   CLI tool
├── packages/
│   └── python/              # [Python] pip install mdm-core
│       └── python/mdm_core/ #   LangChain, LlamaIndex loaders
├── viewer/
│   └── index.html           # Web viewer (44KB, no install)
├── tests/
│   ├── docx_benchmark/      # DOCX benchmark (vs Pandoc)
│   ├── pdf_benchmark/       # PDF benchmark (vs Marker)
│   └── benchmark_engine.py  # Quantitative metrics (BLEU, edit distance)
└── samples/input/           # Test HWP/HWPX files
```

---

## Build from Source / 직접 빌드하기

### Requirements

- **Rust** 1.70+ ([install](https://rustup.rs/))
- **Python** 3.8+ (for Python package build)

### Rust Core Build

```bash
git clone https://github.com/seunghan91/markdown-media.git
cd markdown-media

# Build
cd core && cargo build --release

# Test (all 159 must pass)
cargo test
```

### Python Package Build (Development)

```bash
pip install maturin
cd packages/python
maturin build --release
pip install target/wheels/mdm_core-*.whl
```

### Desktop App Build (macOS / Windows)

데스크톱 앱은 **Tauri 2** (Rust + Svelte) 기반입니다.

**Prerequisites:**
- Node.js 20+
- Rust 1.70+ ([rustup.rs](https://rustup.rs/))
- **macOS 전용**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows 전용**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) (C++ 워크로드 포함)

#### macOS

```bash
cd desktop
npm install

# 개발 서버 (핫 리로드)
npm run tauri dev

# 릴리즈 빌드 (.dmg + .app)
npm run tauri build
```

출력 위치: `desktop/src-tauri/target/release/bundle/`
- `macos/MDM Desktop.app`
- `dmg/MDM Desktop_0.1.0_aarch64.dmg`

#### Windows

> **주의**: Windows 설치파일은 **Windows 환경에서만 빌드 가능**합니다.
> macOS에서 빌드하려면 아래 [GitHub Actions](#github-actions-cross-build) 방법을 사용하세요.

Windows 머신에서:

```bat
cd desktop
npm install

:: 개발 서버
npm run tauri dev

:: 릴리즈 빌드 (.msi + .exe)
npm run tauri build
```

출력 위치: `desktop\src-tauri\target\release\bundle\`
- `msi\MDM Desktop_0.1.0_x64_en-US.msi`
- `nsis\MDM Desktop_0.1.0_x64-setup.exe`

#### GitHub Actions Cross-Build

macOS에서 Windows 설치파일을 만들려면 CI를 사용합니다:

```yaml
# .github/workflows/release.yml
jobs:
  build-desktop:
    strategy:
      matrix:
        include:
          - os: macos-latest   # → .dmg
          - os: windows-latest # → .msi / .exe
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - uses: dtolnay/rust-toolchain@stable
      - run: cd desktop && npm ci
      - uses: tauri-apps/tauri-action@v0
        with:
          projectPath: desktop/
```

---

## Run Benchmarks / 벤치마크 직접 돌려보기

```bash
# Generate test files
python3 tests/docx_benchmark/generate_test_docx.py
python3 tests/pdf_benchmark/generate_test_pdfs.py

# DOCX: MDM vs Pandoc
python3 tests/docx_benchmark/compare_quality.py

# PDF: MDM vs Marker vs pdftotext
python3 tests/pdf_benchmark/compare_quality.py

# Quantitative metrics (BLEU, edit distance)
python3 tests/benchmark_engine.py
```

---

## 이 프로젝트는 왜 만들었나요? / Why Was This Built?

90% of information is trapped in unstructured documents (PDF, DOCX, legacy formats like HWP). AI needs structured text, but existing converters discard media assets.

90%의 정보가 비정형 문서(PDF, DOCX, HWP 등 레거시 포맷)에 갇혀 있습니다. AI는 구조화된 텍스트가 필요하지만, 기존 변환기들은 미디어 에셋을 폐기합니다.

The problems / 문제점:

- **No HWP support** — Existing tools (Marker, Docling, MinerU, MarkItDown) don't support HWP at all. 기존 도구들은 HWP를 전혀 지원하지 않습니다.
- **Media loss** — Current converters extract text only, discarding images, charts, and tables. 현재 변환기들은 텍스트만 추출하고 이미지, 차트, 표를 폐기합니다.
- **Quality gaps** — Pandoc breaks DOCX tables; PDF tools miss heading hierarchy. Pandoc은 DOCX 테이블을 망가뜨리고, PDF 도구는 제목 계층을 놓칩니다.
- **Speed** — Python-based tools are slow. Python 기반 도구는 느립니다.

MDM solves all of these / MDM은 이 모든 문제를 해결합니다:

- **HWP Native Parsing** — The only tool in the world. 세계 유일의 HWP 네이티브 파서.
- **Rust Performance** — 10-100x faster than Python-based tools. Python 대비 10-100배 빠름.
- **Media Preservation** — Every image, table, and chart extracted, indexed, and referenced. 모든 이미지, 표, 차트를 추출, 인덱싱, 참조.
- **AI-Ready** — LangChain/LlamaIndex integration out of the box. LangChain/LlamaIndex 즉시 연결.

---

## Contributing / 기여하기

All contributions are welcome! See `CONTRIBUTING.md`.

We especially need help with:
- HWP equation parsing
- PDF OCR (scanned documents)
- WASM build optimization (removing C dependencies)
- Real-world document test cases

---

## License / 라이선스

MIT License

---

## 日本語

MDM(Markdown-Media)は、HWP、PDF、DOCXなどの文書をMarkdown + メディアバンドルに変換する高速エンジンです。Rustで構築され、PandocやMarkerよりも高品質で10-100倍高速です。AI/LLMパイプラインに最適化されています。

主な特徴:
- **HWPネイティブ対応** — 世界唯一のオープンソースHWP→Markdownエンジン
- **メディア保存** — 画像、表、チャートをインデックス付きで抽出
- **型別メディア構文** — `@[[画像]]` `~[[表]]` `%[[動画]]` `$[[数式]]` `^[[音声]]` `&[[埋め込み]]`
- **Python統合** — `pip install mdm-core` でLangChain/LlamaIndexと即座に接続

## 中文

MDM(Markdown-Media)是一个将HWP、PDF、DOCX等文档转换为Markdown + 媒体包的高速引擎。基于Rust构建，比Pandoc和Marker质量更高、速度快10-100倍。专为AI/LLM管道优化。

主要特点:
- **HWP原生支持** — 全球唯一的开源HWP→Markdown引擎
- **媒体保留** — 提取图像、表格、图表并建立索引
- **类型化媒体语法** — `@[[图像]]` `~[[表格]]` `%[[视频]]` `$[[公式]]` `^[[音频]]` `&[[嵌入]]`
- **Python集成** — `pip install mdm-core` 即可连接LangChain/LlamaIndex

---

**Author**: [seunghan91](https://github.com/seunghan91)
**Last Updated**: 2026.04.13
