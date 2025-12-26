# Format Support Plan

## Current Status (2024-12)

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
**Status: 🔶 Partial Support**

| Feature | Status | Details |
|---------|--------|---------|
| File parsing | ✅ | OLE reader with zlib/deflate |
| Text extraction | ✅ | HWPTAG_PARA_TEXT record parsing |
| Table extraction | 🔶 | Basic structure, needs improvement |
| **Character formatting** | ❌ | Not implemented |
| Image extraction | ✅ | BinData streams |
| MDX output | ✅ | Basic conversion |

**TODO:**
1. [ ] Parse HWPTAG_CHAR_SHAPE records for formatting info
2. [ ] Map character shape IDs to text runs
3. [ ] Apply Markdown formatting (bold, italic, underline, strikeout)
4. [ ] Improve table cell content extraction

**Technical Notes:**
- Character shapes are defined in DocInfo stream
- Each paragraph has charShapeIDRef mapping
- Need to build char_shape_map similar to HWPX's CharStyle

---

### PDF
**Status: 🔴 Minimal Support**

| Feature | Status | Details |
|---------|--------|---------|
| File parsing | ✅ | Basic binary read |
| Text extraction | 🔴 | Very basic (BT/ET operators only) |
| Table extraction | ❌ | Not implemented |
| Character formatting | ❌ | Not implemented |
| Image extraction | ❌ | Not implemented |
| MDX output | ❌ | Not implemented |

**TODO (Phase 1 - Basic):**
1. [ ] Use `pdf-extract` or `lopdf` crate for proper PDF parsing
2. [ ] Implement proper text extraction with positioning
3. [ ] Add page-by-page text organization
4. [ ] Create MDX output with page markers

**TODO (Phase 2 - Advanced):**
1. [ ] Table detection using text positioning heuristics
2. [ ] Image extraction (embedded images)
3. [ ] Font-based formatting detection (bold/italic)
4. [ ] Handle encrypted PDFs

**Recommended Crates:**
- `lopdf` - Low-level PDF manipulation
- `pdf-extract` - Text extraction
- `pdfium-render` - High-fidelity rendering (if needed)

---

## Implementation Priority

### Phase 1: HWP Formatting (High Priority)
**Goal:** Match HWPX feature parity

1. Study HWP 5.0 spec for HWPTAG_CHAR_SHAPE
2. Parse DocInfo stream for char shape definitions
3. Build char_shape_map in HwpParser
4. Apply formatting during text extraction
5. Update MDX output

### Phase 2: PDF Basic Support
**Goal:** Reliable text extraction

1. Add `lopdf` dependency
2. Implement proper PDF structure parsing
3. Extract text with positioning info
4. Generate MDX with page breaks

### Phase 3: PDF Advanced
**Goal:** Tables and images

1. Implement table detection algorithm
2. Extract embedded images
3. Detect formatting from font info

---

## File Structure

```
src/
├── hwp/
│   ├── mod.rs
│   ├── ole.rs         # OLE compound file reader
│   ├── parser.rs      # HWP document parser
│   └── record.rs      # HWP record parsing
├── hwpx/
│   ├── mod.rs
│   └── parser.rs      # HWPX parser (complete)
├── pdf/
│   ├── mod.rs
│   └── parser.rs      # PDF parser (needs work)
└── main.rs            # CLI tool
```

---

## References

- [HWP 5.0 Format Spec](https://www.hancom.com/etc/hwpDownload.do)
- [HWPX/OWPML Standard](https://tech.hancom.com/hwpxformat/)
- [PDF Reference](https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf)
