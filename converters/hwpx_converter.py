#!/usr/bin/env python3
"""
HWPX to MDX Converter
Converts HWPX (Hancom Office Open Format) files to MDX
HWPX is an XML-based format similar to DOCX
"""
import sys
import os
import json
from pathlib import Path
import zipfile
import xml.etree.ElementTree as ET

class HwpxToMdxConverter:
    def __init__(self, file_path):
        self.file_path = Path(file_path)
        self.validate_file()
        
    def validate_file(self):
        if not self.file_path.exists():
            raise FileNotFoundError(f"File not found: {self.file_path}")
        if not self.file_path.suffix.lower() == '.hwpx':
            raise ValueError("Not a valid HWPX file")
    
    def convert(self, output_dir):
        """Convert HWPX to MDX format"""
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        
        print(f"Converting {self.file_path.name}...")
        
        # Extract content
        text_content = self.extract_text()
        images = self.extract_images(output_path / 'assets')
        metadata = self.extract_metadata()
        
        # Create MDX
        mdx_file = output_path / f"{self.file_path.stem}.mdx"
        mdm_file = output_path / f"{self.file_path.stem}.mdm"
        
        mdx_content = self.generate_mdx(text_content, metadata)
        
        with open(mdx_file, 'w', encoding='utf-8') as f:
            f.write(mdx_content)
        
        # Create MDM
        mdm_data = {
            "version": "1.0",
            "resources": {img['name']: {'type': 'image', 'src': f"assets/{img['name']}"} 
                        for img in images},
            "metadata": metadata
        }
        
        with open(mdm_file, 'w', encoding='utf-8') as f:
            json.dump(mdm_data, f, indent=2, ensure_ascii=False)
        
        print(f"✓ Created: {mdx_file}")
        print(f"✓ Created: {mdm_file}")
        
        return {'mdx': str(mdx_file), 'mdm': str(mdm_file)}
    
    def extract_text(self):
        """Extract text from HWPX"""
        paragraphs = []
        
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            # HWPX structure: Contents/section0.xml, section1.xml, etc.
            for name in zip_file.namelist():
                if name.startswith('Contents/section') and name.endswith('.xml'):
                    print(f"  Reading: {name}")
                    try:
                        with zip_file.open(name) as section_file:
                            content = section_file.read().decode('utf-8')
                            paragraphs.extend(self._parse_section(content))
                    except Exception as e:
                        print(f"  Warning: Could not parse {name}: {e}")
        
        return '\n\n'.join(paragraphs)
    
    def _parse_section(self, xml_content):
        """Parse HWPX section XML"""
        paragraphs = []
        
        try:
            root = ET.fromstring(xml_content)
            
            # Find all paragraph elements
            for elem in root.iter():
                tag_name = elem.tag.split('}')[-1] if '}' in elem.tag else elem.tag
                
                if tag_name == 'p':  # Paragraph
                    texts = []
                    for child in elem.iter():
                        child_tag = child.tag.split('}')[-1] if '}' in child.tag else child.tag
                        if child_tag == 't' and child.text:  # Text
                            texts.append(child.text)
                    
                    if texts:
                        paragraphs.append(''.join(texts))
        except ET.ParseError as e:
            print(f"  XML parse error: {e}")
        
        return paragraphs
    
    def extract_images(self, assets_dir):
        """Extract images from HWPX"""
        assets_dir = Path(assets_dir)
        assets_dir.mkdir(parents=True, exist_ok=True)
        
        images = []
        
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            for name in zip_file.namelist():
                # HWPX stores images in BinData/
                if name.startswith('BinData/'):
                    img_name = os.path.basename(name)
                    if img_name and ('.' in img_name):
                        img_path = assets_dir / img_name
                        
                        with zip_file.open(name) as img_file:
                            with open(img_path, 'wb') as out_file:
                                out_file.write(img_file.read())
                        
                        images.append({
                            'name': img_name,
                            'path': str(img_path)
                        })
                        print(f"  Extracted: {img_name}")
        
        return images
    
    def extract_metadata(self):
        """Extract metadata from HWPX"""
        metadata = {
            'source': self.file_path.name,
            'converter': 'hwpx_converter.py',
            'format': 'hwpx'
        }
        
        # Try to read header
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            try:
                if 'META-INF/container.xml' in zip_file.namelist():
                    with zip_file.open('META-INF/container.xml') as meta_file:
                        content = meta_file.read().decode('utf-8')
                        # Parse metadata here if needed
            except Exception:
                pass
        
        return metadata
    
    def generate_mdx(self, content, metadata):
        """Generate MDX file content"""
        title = metadata.get('title', self.file_path.stem)
        
        return f"""---
title: {title}
source: {self.file_path.name}
format: hwpx
converted: true
---

# {title}

{content}
"""

def main():
    if len(sys.argv) < 3:
        print("Usage: python hwpx_converter.py <input.hwpx> <output_dir>")
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    try:
        converter = HwpxToMdxConverter(input_path)
        result = converter.convert(output_dir)
        
        print("\n✅ Conversion complete!")
        print(json.dumps(result, indent=2))
    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
