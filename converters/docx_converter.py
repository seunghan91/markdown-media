#!/usr/bin/env python3
"""
DOCX to MDX Converter
Converts Microsoft Word DOCX files to MDX format
"""
import sys
import os
import json
from pathlib import Path
import zipfile
import xml.etree.ElementTree as ET

class DocxToMdxConverter:
    WORD_NS = '{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
    
    def __init__(self, file_path):
        self.file_path = Path(file_path)
        self.validate_file()
        
    def validate_file(self):
        if not self.file_path.exists():
            raise FileNotFoundError(f"File not found: {self.file_path}")
        if not self.file_path.suffix.lower() == '.docx':
            raise ValueError("Not a valid DOCX file")
    
    def convert(self, output_dir):
        """Convert DOCX to MDX format"""
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
        """Extract text from DOCX"""
        paragraphs = []
        
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            try:
                with zip_file.open('word/document.xml') as doc_file:
                    tree = ET.parse(doc_file)
                    root = tree.getroot()
                    
                    for para in root.iter(f'{self.WORD_NS}p'):
                        texts = []
                        for text in para.iter(f'{self.WORD_NS}t'):
                            if text.text:
                                texts.append(text.text)
                        
                        if texts:
                            para_text = ''.join(texts)
                            
                            # Check for headings
                            style = para.find(f'.//{self.WORD_NS}pStyle')
                            if style is not None:
                                style_val = style.get(f'{self.WORD_NS}val', '')
                                if 'Heading1' in style_val:
                                    paragraphs.append(f"# {para_text}")
                                elif 'Heading2' in style_val:
                                    paragraphs.append(f"## {para_text}")
                                elif 'Heading3' in style_val:
                                    paragraphs.append(f"### {para_text}")
                                else:
                                    paragraphs.append(para_text)
                            else:
                                paragraphs.append(para_text)
            except KeyError:
                return "Error: Could not read document content"
        
        return '\n\n'.join(paragraphs)
    
    def extract_images(self, assets_dir):
        """Extract images from DOCX"""
        assets_dir = Path(assets_dir)
        assets_dir.mkdir(parents=True, exist_ok=True)
        
        images = []
        
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            for name in zip_file.namelist():
                if name.startswith('word/media/'):
                    img_name = os.path.basename(name)
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
        """Extract metadata from DOCX"""
        metadata = {
            'source': self.file_path.name,
            'converter': 'docx_converter.py',
            'format': 'docx'
        }
        
        with zipfile.ZipFile(self.file_path, 'r') as zip_file:
            try:
                with zip_file.open('docProps/core.xml') as core_file:
                    tree = ET.parse(core_file)
                    root = tree.getroot()
                    
                    # Extract Dublin Core metadata
                    dc_ns = '{http://purl.org/dc/elements/1.1/}'
                    cp_ns = '{http://schemas.openxmlformats.org/package/2006/metadata/core-properties}'
                    
                    title = root.find(f'{dc_ns}title')
                    if title is not None and title.text:
                        metadata['title'] = title.text
                    
                    creator = root.find(f'{dc_ns}creator')
                    if creator is not None and creator.text:
                        metadata['author'] = creator.text
            except KeyError:
                pass
        
        return metadata
    
    def generate_mdx(self, content, metadata):
        """Generate MDX file content"""
        title = metadata.get('title', self.file_path.stem)
        
        return f"""---
title: {title}
source: {self.file_path.name}
converted: true
---

# {title}

{content}
"""

def main():
    if len(sys.argv) < 3:
        print("Usage: python docx_converter.py <input.docx> <output_dir>")
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    try:
        converter = DocxToMdxConverter(input_path)
        result = converter.convert(output_dir)
        
        print("\n✅ Conversion complete!")
        print(json.dumps(result, indent=2))
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
