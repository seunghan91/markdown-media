# MDM User Guide

## What is MDM?

**MDM (Markdown+Media)** is a format and toolset for converting document files (HWP, PDF, DOCX) into a structured Markdown bundle with separated media assets.

### The Problem

Korean government and organizations use HWP (Hancom Office) files extensively. These files:

- Mix text and complex media (tables, charts, images)
- Are difficult to parse programmatically
- Don't work well with modern web tools
- Lock content in proprietary formats

### The Solution

MDM converts documents into:

1. **Clean Markdown (.mdx)** - Pure text content, machine-readable
2. **Media Assets** - Tables as SVG, images extracted separately
3. **Metadata (.mdm)** - JSON file with resource definitions

## Installation

### npm (JavaScript/CLI)

```bash
npm install -g @mdm/cli @mdm/parser
```

### PyPI (Python)

```bash
pip install mdm-parser-py
```

### Rust (from source)

```bash
cd core
cargo build --release
```

## Quick Start

### Convert HWP to MDX

```bash
# Using CLI
mdm convert document.hwp -o output/

# Result:
# output/
#   ├── document.mdx      # Markdown content
#   ├── document.mdm      # Resource metadata
#   └── assets/
#       ├── table_1.svg   # Extracted tables
#       └── image_1.png   # Extracted images
```

### Convert PDF to MDX

```bash
mdm convert report.pdf -o output/
```

### Preview Locally

```bash
mdm serve output/ --port 3000 --open
```

## Usage Examples

### Example 1: Government Report Conversion

```bash
# Convert HWP government report
mdm convert 2024_report.hwp -o ./converted/

# Validate output
mdm validate ./converted/

# Serve for preview
mdm serve ./converted/
```

### Example 2: Batch Conversion

```bash
# Convert all HWP files in a directory
for file in *.hwp; do
    mdm convert "$file" -o "./output/$(basename "$file" .hwp)/"
done
```

### Example 3: Python Usage

```python
from mdm_parser_py import HwpToSvgConverter, PdfProcessor

# Convert HWP table to SVG
converter = HwpToSvgConverter('document.hwp')
svg_files = converter.convert('output_dir/')

# Extract PDF text
processor = PdfProcessor('report.pdf')
text = processor.extract_text()
metadata = processor.extract_metadata()
```

## MDM Syntax

### Image References

```markdown
<!-- Simple image -->

![[photo.jpg]]

<!-- With attributes -->

![[logo.png | width=300 align=center alt="Company Logo"]]

<!-- Using presets -->

![[thumbnail.jpg | size=thumb]]
![[banner.jpg | ratio=widescreen]]
```

### Available Presets

**Size Presets:**

- `thumb` - 150px
- `small` - 300px
- `medium` - 640px
- `large` - 1024px

**Ratio Presets:**

- `square` - 1:1
- `standard` - 4:3
- `widescreen` - 16:9
- `portrait` - 3:4
- `story` - 9:16

### Sidecar File (.mdm)

```json
{
  "version": "1.0",
  "resources": {
    "logo": {
      "type": "image",
      "src": "assets/logo.png",
      "alt": "Company Logo",
      "width": 300
    }
  },
  "presets": {
    "hero": {
      "width": 1200,
      "ratio": "16/9"
    }
  }
}
```

## Workflow

### 1. Convert

```bash
mdm convert input.hwp -o output/
```

### 2. Edit

Edit the generated `.mdx` file:

- Clean up extracted text
- Add proper Markdown formatting
- Insert media references using `![[]]` syntax

### 3. Validate

```bash
mdm validate output/
```

### 4. Preview

```bash
mdm serve output/ --open
```

## Advanced Features

### Watch Mode

```bash
# Auto-convert on file change (planned feature)
mdm convert input.hwp -o output/ --watch
```

### Custom Presets

Create custom presets in your `.mdm` file:

```json
{
  "presets": {
    "my-custom": {
      "width": 800,
      "height": 600,
      "object-fit": "contain"
    }
  }
}
```

## Troubleshooting

### HWP Files Not Converting

**Issue**: Text extraction fails
**Solution**: HWP files use compression. Install Python dependencies:

```bash
pip install pyhwp
```

### PDF Images Not Extracting

**Issue**: Images not found
**Solution**: Install pdfplumber:

```bash
pip install pdfplumber
```

### CLI Not Found

**Issue**: `mdm` command not recognized
**Solution**: Install globally:

```bash
npm install -g @mdm/cli
```

## API Reference

See [API Documentation](./api/) for detailed API reference.

## Contributing

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](../LICENSE)
