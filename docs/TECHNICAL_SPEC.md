# MDM Technical Specification

## Overview

MDM (Markdown+Media) is a **document conversion format** that separates text content from visual media.

## What MDM Does

### INPUT ➡️ OUTPUT

```
┌─────────────────┐
│   INPUT FILES   │
├─────────────────┤
│ • HWP           │  ──┐
│ • PDF           │    │
│ • DOCX (future) │    │
└─────────────────┘    │
                       │ MDM Conversion
                       │
                       ▼
┌─────────────────────────────────┐
│      MDM BUNDLE (OUTPUT)        │
├─────────────────────────────────┤
│ ✓ document.mdx                  │  ← Clean Markdown
│ ✓ document.mdm                  │  ← Metadata JSON
│ ✓ assets/                       │
│   ├── table_1.svg               │  ← Tables as SVG
│   ├── chart_2.png               │  ← Charts as images
│   └── photo_3.jpg               │  ← Extracted media
└─────────────────────────────────┘
```

## Architecture

### 3-Layer System

```
┌────────────────────────────────────┐
│     1. RUST CORE ENGINE            │
│  ┌──────────────────────────────┐  │
│  │ HWP Parser (OLE format)      │  │
│  │ PDF Parser (text extraction) │  │
│  │ Image detection              │  │
│  └──────────────────────────────┘  │
└────────────────────────────────────┘
              │
              ▼
┌────────────────────────────────────┐
│     2. PYTHON CONVERTER            │
│  ┌──────────────────────────────┐  │
│  │ Table → SVG rendering        │  │
│  │ Chart → PNG capture          │  │
│  │ OCR (optional)               │  │
│  └──────────────────────────────┘  │
└────────────────────────────────────┘
              │
              ▼
┌────────────────────────────────────┐
│     3. JAVASCRIPT RENDERER         │
│  ┌──────────────────────────────┐  │
│  │ MDM syntax parser            │  │
│  │ ![[]] reference handler      │  │
│  │ HTML output generator        │  │
│  └──────────────────────────────┘  │
└────────────────────────────────────┘
```

## Key Concepts

### 1. Separation of Concerns

**Text Stream (Data)**

- Extracted as pure Markdown
- Machine-readable
- SEO-friendly
- AI training ready

**Media Stream (Visual)**

- Complex tables → SVG
- Charts → PNG/SVG
- Images → Original format
- Preserves visual fidelity

### 2. MDM Reference Syntax

```markdown
![[resource-name | attributes]]
```

Examples:

```markdown
![[logo.png]]
![[table_1.svg | width=800]]
![[photo.jpg | size=medium ratio=widescreen]]
```

### 3. Bundle Structure

```
my-document/
├── index.mdx          # Markdown content with ![[]] refs
├── index.mdm          # JSON metadata
└── assets/            # Media files
    ├── image_1.png
    ├── table_1.svg
    └── chart_2.png
```

## Conversion Flow

```
HWP File
  │
  ├─→ Rust Core: Read OLE streams
  │     └─→ Extract text sections
  │     └─→ Find BinData (images)
  │
  ├─→ Python: Parse tables
  │     └─→ Render to SVG
  │
  └─→ Generators:
        ├─→ Create .mdx (Markdown)
        ├─→ Create .mdm (Metadata JSON)
        └─→ Save assets/
```

## Use Cases

### 1. Government Digital Transformation

Convert legacy HWP documents to web-friendly format:

- Archives → Markdown
- Tables → SVG (searchable, scalable)
- Maintains original layout

### 2. Personal Knowledge Management

- Extract content from PDFs
- Organize in Markdown-based systems (Obsidian, Logseq)
- Keep media separate but linked

### 3. Documentation

- Convert technical docs
- Preserve complex diagrams
- Version control friendly

## Technical Details

### HWP Parsing

```rust
// Rust Core
OleReader::open("file.hwp")
  → read_body_text(section_num)
  → extract_text() // ASCII/UTF-8 extraction
  → extract_images() // Magic byte detection
```

### PDF Parsing

```rust
// Rust Core
PdfParser::open("file.pdf")
  → extract_text() // BT...ET operators
  → get_page_count()
  → extract_metadata()
```

### Python Bridge

```python
# Python Converter
HwpToSvgConverter("file.hwp")
  .extract_tables()  # pyhwp integration
  .render_to_svg()   # svgwrite
```

## Comparison

### Before MDM

```
document.hwp → ❌ Locked in HWP format
              ❌ Can't extract programmatically
              ❌ Not web-compatible
```

### After MDM

```
document.hwp → ✅ document.mdx (Markdown)
              ✅ assets/tables.svg (Web-ready)
              ✅ API accessible
              ✅ Version controllable
```

## Browser Support

MDM files can be:

1. Rendered in any Markdown viewer
2. Processed by WASM parser (browser-based)
3. Served via `mdm serve` command

## Questions Answered

### "Can this convert HTML to Markdown?"

**No.** MDM converts:

- **HWP → MDX** (Markdown+Media bundle)
- **PDF → MDX** (Markdown+Media bundle)

It's not an HTML-to-Markdown converter. It's a **document-to-structured-content** converter.

### "Why not just extract to plain Markdown?"

Complex media (tables, charts) don't translate well to Markdown. MDM:

1. Extracts text → Markdown
2. Converts complex visuals → SVG/PNG
3. Links them together with `![[]]` syntax

### "What's the difference from regular Markdown?"

Regular Markdown:

```markdown
![image](./img.png)
```

MDM:

```markdown
![[image.png | size=large ratio=widescreen]]
```

Plus `.mdm` sidecar for metadata.

## Roadmap

- ✅ HWP text extraction
- ✅ PDF processing
- ✅ Table→SVG rendering
- ⏳ DOCX support
- ⏳ OCR integration
- ⏳ Advanced table parsing

## License

MIT
