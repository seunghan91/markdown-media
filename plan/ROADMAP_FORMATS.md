# MDM Format Support Roadmap — Universal Document-to-Markdown

## Current Status (v0.1)

| Format | Extension | Status | Parser | LOC |
|--------|-----------|:------:|--------|:---:|
| HWP | `.hwp` | ✅ Production | Rust native (OLE/CFB) | 4,800+ |
| HWPX | `.hwpx` | ✅ Production | Rust native (quick-xml) | 1,200+ |
| PDF | `.pdf` | ✅ Production | Rust (lopdf + pdf-extract) | 2,800+ |
| DOCX | `.docx` | ✅ Production | Rust (quick-xml + zip) | 1,800+ |

Benchmark: DOCX 100% vs Pandoc, PDF 93% vs Marker, HWP no competitor

---

## Phase 1: Essential Foundation (85% of real-world requests)

Target: 5 most-used formats worldwide

| Format | Extension | Global Share | Approach | Reference Library | Difficulty |
|--------|-----------|:----------:|---------|-------------------|:----------:|
| **PDF** | `.pdf` | ~30% | ✅ Done | lopdf, pdf-extract | - |
| **DOCX** | `.docx` | ~25% | ✅ Done | quick-xml, zip | - |
| **XLSX** | `.xlsx` | ~15% | 🆕 Add | calamine (Rust) / openpyxl (Python) | Medium |
| **PPTX** | `.pptx` | ~10% | 🆕 Add | quick-xml + zip (same as DOCX) | Medium |
| **HTML** | `.html` | ~10% | 🆕 Add | html5ever (Rust) / scraper | Easy |

### XLSX Strategy
- Extract cell values as Markdown tables (sheet by sheet)
- Handle merged cells, multi-sheet workbooks
- Ignore formulas (output computed values)
- Rust: `calamine` crate (fast Excel reader, pure Rust)
- Fallback: wrap openpyxl via Python FFI

### PPTX Strategy
- Same OOXML ZIP structure as DOCX
- Extract slide text + speaker notes
- Each slide → Markdown section (`## Slide N`)
- Images → extract to assets/
- Reuse existing DOCX ZIP/XML parsing code

### HTML Strategy
- Semantic tag → Markdown mapping (`<h1>`→`#`, `<table>`→pipe table)
- Strip scripts/styles/nav
- Preserve images (`<img>` → `@[[image]]`)
- Rust: `scraper` + `html5ever` for parsing

---

## Phase 2: Enterprise Format Coverage (95% of requests)

| Format | Extension | Use Case | Approach | Reference Library |
|--------|-----------|----------|---------|-------------------|
| **RTF** | `.rtf` | Legal/gov legacy | Rust parser or wrap rtfparse | rtfparse (Python) |
| **ODT** | `.odt` | LibreOffice docs | ZIP + XML (similar to DOCX) | odfdo (Python) |
| **ODS** | `.ods` | LibreOffice sheets | ZIP + XML | calamine (Rust) |
| **ODP** | `.odp` | LibreOffice slides | ZIP + XML | odfdo (Python) |
| **EPUB** | `.epub` | Ebooks/publishing | ZIP + HTML chapters | epub-rs (Rust) |
| **CSV/TSV** | `.csv .tsv` | Tabular data | Native Rust (trivial) | csv crate (Rust) |
| **TXT** | `.txt` | Plain text | Pass-through | std::fs (Rust) |
| **EML/MSG** | `.eml .msg` | Email archives | Python email.parser / msg-parser | mail-parser (Rust) |

### ODT/ODS/ODP Strategy
- Same approach as DOCX: ZIP → unpack → parse XML
- ODF XML is simpler than OOXML
- Can share 80% of DOCX parser infrastructure

### EPUB Strategy
- EPUB = ZIP of HTML chapters + metadata
- Reuse HTML parser for each chapter
- Extract cover image, TOC from metadata

---

## Phase 3: Specialized & Regional Formats

| Format | Extension | Region/Domain | Approach | Reference Library |
|--------|-----------|---------------|---------|-------------------|
| **LaTeX** | `.tex` | Academic/research | latex-parser → AST → MD | pulldown-latex (Rust) |
| **RST** | `.rst` | Python docs | rst-parser → MD | rstparse (Python) |
| **AsciiDoc** | `.adoc` | Technical docs | asciidoc-parser | asciidoctor (Ruby) |
| **DOC** | `.doc` | Legacy Word | Binary format, complex | antiword/catdoc (C) |
| **XLS** | `.xls` | Legacy Excel | BIFF format | calamine (Rust) |
| **PPT** | `.ppt` | Legacy PowerPoint | Binary format | python-pptx partial |
| **WPS** | `.wps` | Kingsoft (China) | Reverse-engineered | Limited support |
| **DjVu** | `.djvu` | Scanned docs | djvulibre → text | djvulibre (C) |

### LaTeX Strategy
- Parse `.tex` → extract document structure
- Convert `\section{}` → `##`, `\begin{table}` → pipe table
- Preserve equations as `$[[...]]` MDM syntax
- Handle `\includegraphics{}` → `@[[image]]`

### Legacy Binary Formats (DOC/XLS/PPT)
- Very complex binary formats (OLE Compound Document)
- Recommendation: wrap LibreOffice headless for conversion
- `libreoffice --headless --convert-to docx file.doc`
- Then parse the DOCX output with MDM

---

## Phase 4: Emerging Platform Formats

| Format | Extension | Platform | Approach |
|--------|-----------|----------|---------|
| **Notion Export** | `.md` + assets | Notion | Parse Notion-flavored MD + remap images |
| **Obsidian Vault** | `.md` + `![[]]` | Obsidian | Convert `![[]]` → MDM `@[[]]` syntax |
| **Google Docs** | `.gdoc` → DOCX | Google Workspace | Export as DOCX → MDM pipeline |
| **Apple Pages** | `.pages` | macOS/iOS | ZIP + protobuf (complex) |
| **Figma** | API export | Design tools | API → extract frames as images |
| **Markdown variants** | `.md .mdx` | Various | Normalize to CommonMark + MDM |

### Obsidian Vault Strategy
- Read `![[wikilink]]` → convert to `@[[image]]` or standard links
- Handle `[[note]]` cross-references
- Preserve frontmatter YAML
- Convert Obsidian callouts to blockquotes

### Notion Export Strategy
- Notion exports as Markdown + media folder
- Remap image paths to MDM `assets/` structure
- Convert Notion-specific blocks (toggles, databases) to tables/lists

---

## Competitor Format Coverage Comparison

| Format | MDM (Current) | MDM (Planned) | Pandoc | MarkItDown | Docling |
|--------|:---:|:---:|:---:|:---:|:---:|
| PDF | ✅ | ✅ | ✅ | ✅ | ✅ |
| DOCX | ✅ | ✅ | ✅ | ✅ | ✅ |
| HWP | ✅ | ✅ | ❌ | ❌ | ❌ |
| HWPX | ✅ | ✅ | ❌ | ❌ | ❌ |
| XLSX | ❌ | ✅ | ❌ | ✅ | ✅ |
| PPTX | ❌ | ✅ | ✅ | ✅ | ✅ |
| HTML | ❌ | ✅ | ✅ | ✅ | ✅ |
| RTF | ❌ | ✅ | ✅ | ❌ | ❌ |
| ODT | ❌ | ✅ | ✅ | ❌ | ❌ |
| EPUB | ❌ | ✅ | ✅ | ❌ | ❌ |
| CSV | ❌ | ✅ | ✅ | ✅ | ❌ |
| LaTeX | ❌ | ✅ | ✅ | ❌ | ❌ |
| EML/MSG | ❌ | ✅ | ❌ | ✅ | ❌ |
| DOC (legacy) | ❌ | ✅* | ✅ | ✅ | ❌ |
| XLS (legacy) | ❌ | ✅* | ❌ | ✅ | ❌ |
| PPT (legacy) | ❌ | ✅* | ✅ | ✅ | ❌ |
| Notion | ❌ | ✅ | ❌ | ❌ | ❌ |
| Obsidian | ❌ | ✅ | ❌ | ❌ | ❌ |
| Images (OCR) | ❌ | ✅ | ❌ | ✅ | ✅ |

*via LibreOffice headless conversion

### MDM Unique Advantage
- **HWP/HWPX**: Only MDM
- **Media Manifest**: Only MDM
- **Type-specific syntax**: Only MDM (`@[[]] ~[[]] %[[]]`)
- **Content-addressable storage**: Only MDM

---

## Implementation Priority Matrix

| Priority | Format | Users | Parser Exists? | Effort | ROI |
|:--------:|--------|:-----:|:--------------:|:------:|:---:|
| P0 | PDF | Billions | ✅ Done | - | - |
| P0 | DOCX | Billions | ✅ Done | - | - |
| P0 | HWP/HWPX | 50M+ | ✅ Done | - | - |
| **P1** | **XLSX** | **Billions** | calamine (Rust) | **2 days** | **Very High** |
| **P1** | **PPTX** | **Billions** | reuse DOCX code | **2 days** | **Very High** |
| **P1** | **HTML** | **Billions** | scraper (Rust) | **1 day** | **Very High** |
| **P1** | **CSV/TXT** | **Billions** | csv crate | **0.5 day** | **High** |
| P2 | EPUB | 500M+ | epub-rs | 1 day | Medium |
| P2 | RTF | 100M+ | rtfparse | 2 days | Medium |
| P2 | ODT/ODS/ODP | 100M+ | reuse DOCX | 3 days | Medium |
| P2 | EML/MSG | 100M+ | mail-parser | 2 days | Medium |
| P3 | LaTeX | 10M+ | latex-parser | 3 days | Niche |
| P3 | DOC/XLS/PPT | Legacy | LibreOffice wrap | 1 day | Low |
| P4 | Notion/Obsidian | 10M+ | MD normalize | 1 day | Growing |
| P4 | Images (OCR) | Special | Tesseract | 3 days | Medium |

---

## Plugin Architecture (Future)

```rust
// core/src/plugin.rs

pub trait FormatPlugin: Send + Sync {
    /// File extensions this plugin handles
    fn extensions(&self) -> &[&str];
    
    /// Convert bytes to Markdown + Media bundle
    fn convert(&self, data: &[u8], filename: &str) -> Result<ConversionResult, ConversionError>;
    
    /// Plugin metadata
    fn name(&self) -> &str;
    fn version(&self) -> &str;
}

pub struct ConversionResult {
    pub markdown: String,
    pub assets: Vec<ExtractedAsset>,
    pub metadata: serde_json::Value,
}

pub struct ExtractedAsset {
    pub data: Vec<u8>,
    pub media_type: MediaType,
    pub extension: String,
    pub metadata: AssetMetadata,
}
```

This enables community-contributed format plugins without modifying core.

---

## Timeline Summary

```
v0.1 (Current):  HWP, HWPX, PDF, DOCX           — 4 formats
v0.2 (Next):     + XLSX, PPTX, HTML, CSV, TXT    — 9 formats  (85% coverage)
v0.3:            + EPUB, RTF, ODT, ODS, ODP, EML  — 15 formats (95% coverage)
v0.4:            + LaTeX, DOC, XLS, PPT, DjVu     — 20 formats
v0.5:            + Notion, Obsidian, OCR           — 23+ formats
v1.0:            Plugin architecture + community    — Extensible
```
