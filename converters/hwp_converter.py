#!/usr/bin/env python3
"""
HWP to MDX Converter
Converts HWP files to MDX (Markdown + Media) format using olefile + zlib
"""
import sys
import os
import json
import struct
import zlib
from pathlib import Path

try:
    import olefile
except ImportError:
    print("Error: olefile is required. Install with: pip install olefile")
    sys.exit(1)


def extract_hwp_text(hwp_path: str) -> str:
    """
    Extract text from HWP file using olefile and zlib decompression.

    Args:
        hwp_path: Path to the HWP file

    Returns:
        Extracted text content
    """
    f = olefile.OleFileIO(hwp_path)

    # Check if file is compressed (FileHeader offset 36, bit 0)
    header = f.openstream("FileHeader").read()
    is_compressed = (header[36] & 1) == 1

    # Find all BodyText sections
    sections = []
    for entry in f.listdir():
        if entry[0] == "BodyText" and entry[1].startswith("Section"):
            sections.append(f"BodyText/{entry[1]}")

    sections.sort(key=lambda x: int(x.split("Section")[1]))

    all_text = []

    for section_name in sections:
        section_data = f.openstream(section_name).read()

        # Decompress if needed
        if is_compressed:
            try:
                decompressed = zlib.decompress(section_data, -15)
            except zlib.error:
                # Try without negative wbits
                try:
                    decompressed = zlib.decompress(section_data)
                except:
                    decompressed = section_data
        else:
            decompressed = section_data

        # Parse records and extract text
        section_text = parse_section_records(decompressed)
        if section_text.strip():
            all_text.append(section_text)

    f.close()
    return "\n\n".join(all_text)


def parse_section_records(data: bytes) -> str:
    """
    Parse HWP record structure and extract PARA_TEXT records.

    HWP record header (4 bytes):
    - Tag ID: bits 0-9 (10 bits)
    - Level: bits 10-19 (10 bits)
    - Size: bits 20-31 (12 bits), 0xFFF means extended size

    HWPTAG_PARA_TEXT = 0x43 = 67
    """
    HWPTAG_PARA_TEXT = 67

    text_parts = []
    i = 0

    while i + 4 <= len(data):
        # Read 4-byte header
        header_val = struct.unpack_from("<I", data, i)[0]
        tag_id = header_val & 0x3FF
        level = (header_val >> 10) & 0x3FF
        size_field = (header_val >> 20) & 0xFFF
        i += 4

        # Extended size handling
        if size_field == 0xFFF:
            if i + 4 > len(data):
                break
            size = struct.unpack_from("<I", data, i)[0]
            i += 4
        else:
            size = size_field

        # Check bounds
        if i + size > len(data):
            break

        record_data = data[i:i + size]
        i += size

        # Extract text from PARA_TEXT records
        if tag_id == HWPTAG_PARA_TEXT:
            text = decode_para_text(record_data)
            if text.strip():
                text_parts.append(text)

    return "\n".join(text_parts)


def decode_para_text(data: bytes) -> str:
    """
    Decode PARA_TEXT record data (UTF-16LE with control characters).

    Control characters:
    - 0x00: NULL (end of string in some cases)
    - 0x01-0x08: Extended control (followed by 14 more bytes = 7 more chars)
    - 0x09: Tab
    - 0x0A: Line break
    - 0x0B: Table/Drawing object (extended control)
    - 0x0C: Drawing object (extended control)
    - 0x0D: Paragraph break
    - 0x0E-0x0F: Extended control
    - 0x10-0x1F: Other control characters (skip)
    """
    # Extended control character codes that consume 8 UTF-16 chars total (16 bytes)
    EXTENDED_CTRL_CHARS = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                          0x0B, 0x0C, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13,
                          0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
                          0x1C, 0x1D, 0x1E, 0x1F}

    result = []
    i = 0

    while i + 1 < len(data):
        # Read UTF-16LE character (2 bytes)
        char_code = struct.unpack_from("<H", data, i)[0]
        i += 2

        if char_code == 0x00:
            # NULL - might be end marker or just padding
            continue
        elif char_code == 0x09:
            # Tab
            result.append('\t')
        elif char_code == 0x0A:
            # Line break
            result.append('\n')
        elif char_code == 0x0D:
            # Paragraph break
            result.append('\n')
        elif char_code in EXTENDED_CTRL_CHARS:
            # Extended control character - skip next 14 bytes (7 UTF-16 chars)
            i += 14
        elif char_code >= 0x20:
            # Normal printable character
            try:
                result.append(chr(char_code))
            except (ValueError, OverflowError):
                pass
        # else: skip other control characters (0x10-0x1F not in EXTENDED_CTRL_CHARS)

    return ''.join(result)


def convert_hwp_to_mdx(input_path: str, output_dir: str, verbose: bool = False) -> dict:
    """
    Convert HWP file to MDX format.

    Args:
        input_path: Path to input HWP file
        output_dir: Output directory for MDX files
        verbose: Verbose output

    Returns:
        Dictionary with paths to generated files
    """
    input_file = Path(input_path)
    if not input_file.exists():
        raise FileNotFoundError(f"Input file not found: {input_path}")

    if input_file.suffix.lower() != '.hwp':
        raise ValueError("Input file must be a .hwp file")

    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    print(f"Converting {input_file.name}...")

    # Extract text using olefile
    try:
        content = extract_hwp_text(str(input_file))
    except Exception as e:
        print(f"  ⚠️  Warning: Failed to extract text: {e}")
        content = ""

    # Generate output files
    mdx_file = output_path / f"{input_file.stem}.mdx"
    mdm_file = output_path / f"{input_file.stem}.mdm"
    assets_dir = output_path / "assets"
    assets_dir.mkdir(exist_ok=True)

    # Clean content of any surrogate characters
    content = content.encode('utf-8', errors='surrogatepass').decode('utf-8', errors='replace')

    # Create MDX content
    mdx_content = f"""---
title: {input_file.stem}
source: {input_file.name}
format: hwp
converted: true
---

{content}
"""

    with open(mdx_file, 'w', encoding='utf-8') as f:
        f.write(mdx_content)

    # Create MDM sidecar file
    mdm_data = {
        "version": "1.0",
        "format": "hwp",
        "source": input_file.name,
        "resources": {},
        "metadata": {
            "converter": "hwp_converter.py (olefile)",
            "text_length": len(content)
        }
    }

    with open(mdm_file, 'w', encoding='utf-8') as f:
        json.dump(mdm_data, f, indent=2, ensure_ascii=False)

    print(f"  ✓ Created: {mdx_file}")
    print(f"  ✓ Created: {mdm_file}")

    if verbose:
        print(f"  📊 Text length: {len(content)} chars")

    return {
        'mdx': str(mdx_file),
        'mdm': str(mdm_file),
        'assets': str(assets_dir),
        'success': True,
        'text_length': len(content)
    }


def extract_text_only(input_path: str) -> str:
    """Extract text only from HWP file."""
    return extract_hwp_text(input_path)


def analyze_hwp(input_path: str) -> str:
    """Analyze HWP file structure."""
    f = olefile.OleFileIO(input_path)

    lines = [f"📄 File: {input_path}", ""]

    # File header info
    header = f.openstream("FileHeader").read()
    is_compressed = (header[36] & 1) == 1
    is_encrypted = (header[36] & 2) == 2

    lines.append("📊 Document Properties:")
    lines.append(f"  - Compressed: {'Yes' if is_compressed else 'No'}")
    lines.append(f"  - Encrypted: {'Yes ⚠️' if is_encrypted else 'No'}")
    lines.append("")

    # List streams
    streams = []
    for entry in f.listdir():
        streams.append("/".join(entry))

    lines.append(f"📁 Streams ({len(streams)}):")
    for stream in sorted(streams):
        lines.append(f"  - {stream}")

    f.close()
    return "\n".join(lines)


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Convert HWP files to MDX format using olefile"
    )
    parser.add_argument("input", help="Input HWP file")
    parser.add_argument("output", nargs="?", default="./output", help="Output directory (default: ./output)")
    parser.add_argument("--text-only", action="store_true", help="Extract text only")
    parser.add_argument("--analyze", action="store_true", help="Analyze file structure")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")

    args = parser.parse_args()

    print("📦 Using Python parser (olefile + zlib)")

    try:
        if args.text_only:
            text = extract_text_only(args.input)
            print(text)
        elif args.analyze:
            analysis = analyze_hwp(args.input)
            print(analysis)
        else:
            result = convert_hwp_to_mdx(args.input, args.output, verbose=args.verbose)
            print("\n✅ Conversion complete!")
            print(json.dumps(result, indent=2, ensure_ascii=False))
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
