#!/usr/bin/env python3
"""
PDF to MDX Converter
Converts PDF files to MDX (Markdown + Media) format
"""
import sys
import json
from pathlib import Path

try:
    import pdfplumber
    HAS_PDFPLUMBER = True
except ImportError:
    HAS_PDFPLUMBER = False
    print("Warning: pdfplumber not installed. Run: pip install pdfplumber")

def convert_pdf_to_mdx(input_path, output_dir):
    """
    Convert PDF file to MDX format
    
    Args:
        input_path: Path to input PDF file
        output_dir: Output directory for MDX files
    """
    input_file = Path(input_path)
    if not input_file.exists():
        raise FileNotFoundError(f"Input file not found: {input_path}")
    
    if not input_file.suffix.lower() == '.pdf':
        raise ValueError("Input file must be a .pdf file")
    
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    
    # Output files
    mdx_file = output_path / f"{input_file.stem}.mdx"
    mdm_file = output_path / f"{input_file.stem}.mdm"
    assets_dir = output_path / "assets"
    assets_dir.mkdir(exist_ok=True)
    
    print(f"Converting {input_file.name}...")
    
    # Extract text if pdfplumber is available
    text_content = ""
    if HAS_PDFPLUMBER:
        with pdfplumber.open(input_file) as pdf:
            pages = []
            for i, page in enumerate(pdf.pages):
                page_text = page.extract_text()
                if page_text:
                    pages.append(f"## Page {i+1}\n\n{page_text}\n")
            text_content = "\n".join(pages)
    else:
        text_content = "<!-- PDF text extraction requires pdfplumber -->\n\nPlaceholder content."
    
    # Create MDX file
    mdx_content = f"""---
title: {input_file.stem}
source: {input_file.name}
converted: true
---

# {input_file.stem}

<!-- Converted from PDF -->

{text_content}

"""
    
    with open(mdx_file, 'w', encoding='utf-8') as f:
        f.write(mdx_content)
    
    # Create MDM sidecar file
    mdm_data = {
        "version": "1.0",
        "resources": {},
        "presets": {},
        "metadata": {
            "source": input_file.name,
            "converter": "pdf_converter.py",
            "format": "pdf"
        }
    }
    
    with open(mdm_file, 'w', encoding='utf-8') as f:
        json.dump(mdm_data, f, indent=2)
    
    print(f"✓ Created: {mdx_file}")
    print(f"✓ Created: {mdm_file}")
    
    return {
        'mdx': str(mdx_file),
        'mdm': str(mdm_file),
        'assets': str(assets_dir)
    }

def main():
    if len(sys.argv) < 3:
        print("Usage: python pdf_converter.py <input.pdf> <output_dir>")
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    try:
        result = convert_pdf_to_mdx(input_path, output_dir)
        print("\n✅ Conversion complete!")
        print(json.dumps(result, indent=2))
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
