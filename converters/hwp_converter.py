#!/usr/bin/env python3
"""
HWP to MDX Converter
Converts HWP files to MDX (Markdown + Media) format
"""
import sys
import os
import json
from pathlib import Path

def convert_hwp_to_mdx(input_path, output_dir):
    """
    Convert HWP file to MDX format
    
    Args:
        input_path: Path to input HWP file
        output_dir: Output directory for MDX files
    """
    input_file = Path(input_path)
    if not input_file.exists():
        raise FileNotFoundError(f"Input file not found: {input_path}")
    
    if not input_file.suffix.lower() == '.hwp':
        raise ValueError("Input file must be a .hwp file")
    
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)
    
    # Output files
    mdx_file = output_path / f"{input_file.stem}.mdx"
    mdm_file = output_path / f"{input_file.stem}.mdm"
    assets_dir = output_path / "assets"
    assets_dir.mkdir(exist_ok=True)
    
    print(f"Converting {input_file.name}...")
    
    # TODO: Call Rust core parser
    # For now, create a placeholder
    
    # Create MDX file
    mdx_content = f"""---
title: {input_file.stem}
source: {input_file.name}
converted: true
---

# {input_file.stem}

<!-- Converted from HWP -->

![hero](assets/hero.png)

## Content

This is a placeholder. Integration with Rust parser pending.

"""
    
    with open(mdx_file, 'w', encoding='utf-8') as f:
        f.write(mdx_content)
    
    # Create MDM sidecar file
    mdm_data = {
        "version": "1.0",
        "resources": {
            "hero": {
                "type": "image",
                "src": "assets/hero.png",
                "alt": "Hero image"
            }
        },
        "presets": {},
        "metadata": {
            "source": input_file.name,
            "converter": "hwp_converter.py",
            "format": "hwp"
        }
    }
    
    with open(mdm_file, 'w', encoding='utf-8') as f:
        json.dump(mdm_data, f, indent=2)
    
    print(f"✓ Created: {mdx_file}")
    print(f"✓ Created: {mdm_file}")
    print(f"✓ Assets directory: {assets_dir}")
    
    return {
        'mdx': str(mdx_file),
        'mdm': str(mdm_file),
        'assets': str(assets_dir)
    }

def main():
    if len(sys.argv) < 3:
        print("Usage: python hwp_converter.py <input.hwp> <output_dir>")
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    try:
        result = convert_hwp_to_mdx(input_path, output_dir)
        print("\n✅ Conversion complete!")
        print(json.dumps(result, indent=2))
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
