# MDM Converters

Document conversion tools for MDM format.

## Available Converters

### HWP Converter

Convert HWP (Hancom Office) files to MDX.

```bash
python hwp_converter.py input.hwp output/
```

### PDF Converter

Convert PDF files to MDX.

```bash
python pdf_converter.py input.pdf output/
```

### HTML Converter

Convert HTML files (including blog posts) to MDX.

**From file:**

```bash
python html_converter.py saved_blog.html output/
```

**From URL:**

```bash
python html_converter.py --url https://blog.naver.com/post/123 output/
```

**Supported platforms:**

- Naver Blog (blog.naver.com)
- Tistory (tistory.com)
- WordPress
- Generic HTML

### Table to SVG

Render table data as SVG image.

```bash
python table_to_svg.py table.json output.svg
```

## Requirements

```bash
pip install -r ../packages/parser-py/requirements.txt
```

Or install individually:

```bash
pip install pyhwp pdfplumber pillow svgwrite beautifulsoup4 requests
```

## CLI Usage

Use the MDM CLI for easier conversion:

```bash
# Convert any supported format
mdm convert document.hwp -o output/
mdm convert report.pdf -o output/
mdm convert blog.html -o output/

# From URL (HTML only)
python converters/html_converter.py --url <blog_url> output/
```

## Examples

### Convert Naver Blog Post

**Save HTML first:**

1. Visit blog post in browser
2. Save as HTML (Ctrl+S / Cmd+S)
3. Convert:

```bash
mdm convert saved_blog.html -o ./my-posts/
```

**Or directly from URL:**

```bash
python converters/html_converter.py --url https://blog.naver.com/user/123 ./output/
```

### Convert Tistory Post

```bash
python converters/html_converter.py --url https://user.tistory.com/123 ./output/
```

### Batch Convert Blog Archive

```bash
# Convert all saved HTML files
for file in *.html; do
    mdm convert "$file" -o "./converted/$(basename "$file" .html)/"
done
```

## Output Structure

All converters create the same MDM bundle structure:

```
output/
├── index.mdx           # Markdown content
├── index.mdm           # Resource metadata
└── assets/
    ├── image_1.jpg     # Downloaded/extracted images
    ├── image_2.png
    └── table_1.svg     # Rendered tables
```
