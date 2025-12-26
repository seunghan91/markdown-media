# Format Support Plan

## Current Status (2024-12, Updated 2024-12-26)

### HWPX (ZIP-based XML)
**Status: ✅ Full Support**

| Feature | Status | Details |
|---------|--------|---------|
| File parsing | ✅ | ZIP archive + XML parsing |
| Text extraction | ✅ | Section-based text extraction |
| Table extraction | ✅ | Markdown table conversion |
| **Character formatting** | ✅ | Bold, italic, underline, strikeout |
| Image listing | ✅ | BinData extraction |
| MDX output | ✅ | With frontmatter |

**Key files:**
- `src/hwpx/parser.rs` - Main parser with CharStyle support

---

### HWP 5.0 (OLE Compound File)
**Status: ✅ Full Support**

| Feature | Status | Details |
|---------|--------|---------|
| File parsing | ✅ | OLE reader with zlib/deflate |
| Text extraction | ✅ | HWPTAG_PARA_TEXT record parsing |
| Table extraction | 🔶 | Basic structure, needs improvement |
| **Character formatting** | ✅ | Bold, italic, underline, strikeout |
| Image extraction | ✅ | BinData streams |
| MDX output | ✅ | With formatting |

**Implemented (2024-12-26):**
1. [x] Parse HWPTAG_CHAR_SHAPE records from DocInfo stream
2. [x] Build char_shape_map in HwpParser (HashMap<u32, CharShape>)
3. [x] Parse HWPTAG_PARA_CHAR_SHAPE for text position → style mapping
4. [x] Apply Markdown formatting (bold, italic, underline, strikeout)

**TODO:**
1. [ ] Improve table cell content extraction

**Technical Implementation:**
- `record.rs`: `parse_char_shape()` - Parses HWPTAG_CHAR_SHAPE records
  - Attr field at offset 46-49: bit 0=italic, bit 1=bold, bits 2-3=underline, bits 18-21=strikeout
- `record.rs`: `parse_para_char_shape()` - Parses position→style ID mappings
- `record.rs`: `extract_para_text_formatted()` - Applies styles to text runs
- `parser.rs`: `parse_doc_info()` - Builds char_shapes HashMap from DocInfo
- `parser.rs`: `parse_section_records_formatted()` - Extracts text with formatting

---

### PDF
**Status: 🟡 Basic Support**

| Feature | Status | Details |
|---------|--------|---------|
| File parsing | ✅ | lopdf + pdf-extract |
| Text extraction | ✅ | pdf-extract with page support |
| Page organization | ✅ | Form feed / line-based splitting |
| Metadata extraction | ✅ | Title, Author, Subject, Creator, Producer |
| **Image extraction** | ✅ | XObject extraction (JPEG, FlateDecode) |
| Table extraction | ❌ | Not implemented |
| Character formatting | ❌ | Not implemented |
| MDX output | ✅ | With frontmatter, page markers, image list |

**Implemented (2024-12-26):**
1. [x] Use `pdf-extract` for text extraction
2. [x] Use `lopdf` for page count and metadata
3. [x] Page-by-page text organization (form feed / line heuristics)
4. [x] MDX output with page markers (`<!-- Page N -->`)
5. [x] Metadata in frontmatter (title, author, version, pages)

**Technical Implementation:**
- `parser.rs`: `PdfParser::parse()` - Main parsing using pdf-extract
- `parser.rs`: `split_into_pages()` - Form feed or line-based page splitting
- `parser.rs`: `extract_metadata()` - lopdf for PDF Info dictionary
- `parser.rs`: `PdfDocument::to_mdx()` - MDX generation with frontmatter

**Implemented (2024-12-26 - Phase 3):**
6. [x] Image extraction from XObjects (Subtype=Image)
7. [x] DCTDecode (JPEG) and FlateDecode (compressed) format support
8. [x] Image metadata (width, height, format) extraction
9. [x] Image list in MDX output

**TODO (Phase 3 - Advanced):**
1. [ ] Table detection using text positioning heuristics
2. [ ] Font-based formatting detection (bold/italic)
3. [ ] Handle encrypted PDFs

**Used Crates:**
- `lopdf` - Page count, metadata extraction, image XObject parsing
- `pdf-extract` - Text extraction
- `flate2` - FlateDecode (zlib) decompression for images

---

## Implementation Priority

### Phase 1: HWP Formatting ✅ COMPLETED (2024-12-26)
**Goal:** Match HWPX feature parity

1. ✅ Study HWP 5.0 spec for HWPTAG_CHAR_SHAPE
2. ✅ Parse DocInfo stream for char shape definitions
3. ✅ Build char_shape_map in HwpParser
4. ✅ Apply formatting during text extraction
5. ✅ Update MDX output with Markdown formatting

### Phase 2: PDF Basic Support ✅ COMPLETED (2024-12-26)
**Goal:** Reliable text extraction

1. ✅ Add `lopdf` and `pdf-extract` dependencies
2. ✅ Implement proper PDF structure parsing
3. ✅ Extract text with page-by-page organization
4. ✅ Generate MDX with frontmatter and page markers
5. ✅ Extract metadata (title, author, etc.)

### Phase 3: PDF Advanced (Next)
**Goal:** Tables, images, and formatting

1. [ ] Implement table detection algorithm (text positioning heuristics)
2. [ ] Extract embedded images
3. [ ] Detect formatting from font info (bold/italic)
4. [ ] Handle encrypted PDFs

---

## File Structure

```
src/
├── hwp/
│   ├── mod.rs
│   ├── ole.rs         # OLE compound file reader
│   ├── parser.rs      # HWP document parser with char_shapes map
│   └── record.rs      # HWP record parsing (CharShape, ParaCharShapeMapping)
├── hwpx/
│   ├── mod.rs
│   └── parser.rs      # HWPX parser (complete)
├── pdf/
│   ├── mod.rs
│   └── parser.rs      # PDF parser (pdf-extract + lopdf)
└── main.rs            # CLI tool
```

## Key Data Structures

### HWP Character Formatting
```rust
// record.rs
pub struct CharShape {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikeout: bool,
}

pub struct ParaCharShapeMapping {
    pub mappings: Vec<(u32, u32)>, // (text_position, char_shape_id)
}

// parser.rs
pub struct HwpParser {
    ole_reader: OleReader,
    char_shapes: HashMap<u32, CharShape>, // Parsed from DocInfo
}
```

### PDF Document Structure
```rust
// pdf/parser.rs
pub struct PdfParser {
    path: PathBuf,
    data: Vec<u8>,
}

pub struct PdfDocument {
    pub version: String,
    pub page_count: usize,
    pub pages: Vec<PageContent>,
    pub metadata: PdfMetadata,
    pub images: Vec<PdfImage>,  // Phase 3: Image extraction
}

pub struct PageContent {
    pub page_number: usize,
    pub text: String,
}

pub struct PdfMetadata {
    pub title: String,
    pub author: String,
    pub subject: String,
    pub creator: String,
    pub producer: String,
}

// Phase 3: Image extraction
pub struct PdfImage {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,  // Jpeg, Png, Raw
    pub data: Vec<u8>,
    pub page: Option<usize>,
}

pub enum ImageFormat {
    Jpeg,      // DCTDecode filter
    Png,       // Future support
    Raw,       // Uncompressed or unknown
}
```

---

## References

- [HWP 5.0 Format Spec](https://www.hancom.com/etc/hwpDownload.do)
- [HWPX/OWPML Standard](https://tech.hancom.com/hwpxformat/)
- [PDF Reference](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)
