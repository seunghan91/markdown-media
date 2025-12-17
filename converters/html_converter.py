#!/usr/bin/env python3
"""
HTML to MDX Converter
Converts HTML files (especially blog posts) to MDX format
"""
import sys
import os
import json
import re
from pathlib import Path
from urllib.parse import urlparse, urljoin

try:
    from bs4 import BeautifulSoup
    import requests
    HAS_DEPS = True
except ImportError:
    HAS_DEPS = False
    print("Warning: Install dependencies: pip install beautifulsoup4 requests")

class HtmlToMdxConverter:
    def __init__(self, html_source, is_url=False):
        """
        Initialize converter
        
        Args:
            html_source: HTML string or URL
            is_url: True if html_source is a URL, False if it's HTML content
        """
        self.is_url = is_url
        self.base_url = None
        
        if is_url:
            self.base_url = html_source
            self.html = self.fetch_html(html_source)
        else:
            self.html = html_source if isinstance(html_source, str) else open(html_source, 'r', encoding='utf-8').read()
        
        if not HAS_DEPS:
            raise ImportError("BeautifulSoup4 is required. Install: pip install beautifulsoup4")
        
        self.soup = BeautifulSoup(self.html, 'html.parser')
        self.platform = self.detect_platform()
        
    def fetch_html(self, url):
        """Fetch HTML from URL"""
        if not HAS_DEPS:
            raise ImportError("requests is required for URL fetching")
        
        response = requests.get(url, headers={
            'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36'
        })
        response.raise_for_status()
        return response.text
    
    def detect_platform(self):
        """Detect blog platform"""
        html_lower = self.html.lower()
        
        if 'blog.naver.com' in html_lower or 'naver' in html_lower:
            return 'naver'
        elif 'tistory.com' in html_lower or 'tistory' in html_lower:
            return 'tistory'
        elif 'wordpress' in html_lower:
            return 'wordpress'
        else:
            return 'generic'
    
    def convert(self, output_dir):
        """
        Convert HTML to MDX format
        
        Returns:
            dict with paths to generated files
        """
        print(f"Converting HTML ({self.platform} platform)...")
        
        output_path = Path(output_dir)
        output_path.mkdir(parents=True, exist_ok=True)
        
        # Extract content based on platform
        content = self.extract_content()
        images = self.extract_images(output_path / 'assets')
        
        # Generate MDX
        mdx_file = output_path / 'index.mdx'
        mdm_file = output_path / 'index.mdm'
        
        self.write_mdx(content, images, mdx_file)
        self.write_mdm(images, mdm_file)
        
        print(f"✓ Created: {mdx_file}")
        print(f"✓ Created: {mdm_file}")
        
        return {
            'mdx': str(mdx_file),
            'mdm': str(mdm_file),
            'images': [str(img['local_path']) for img in images]
        }
    
    def extract_content(self):
        """Extract main content based on platform"""
        if self.platform == 'naver':
            return self.extract_naver_content()
        elif self.platform == 'tistory':
            return self.extract_tistory_content()
        else:
            return self.extract_generic_content()
    
    def extract_naver_content(self):
        """Extract content from Naver blog"""
        # Naver blog specific selectors
        main_content = (
            self.soup.find('div', class_='se-main-container') or
            self.soup.find('div', {'id': 'postViewArea'}) or
            self.soup.find('div', class_='post-view')
        )
        
        if main_content:
            return self.html_to_markdown(main_content)
        return self.extract_generic_content()
    
    def extract_tistory_content(self):
        """Extract content from Tistory blog"""
        main_content = (
            self.soup.find('div', class_='entry-content') or
            self.soup.find('article') or
            self.soup.find('div', class_='article')
        )
        
        if main_content:
            return self.html_to_markdown(main_content)
        return self.extract_generic_content()
    
    def extract_generic_content(self):
        """Extract content from generic HTML"""
        # Try to find main content area
        main = (
            self.soup.find('main') or
            self.soup.find('article') or
            self.soup.find('div', {'id': 'content'}) or
            self.soup.find('div', class_='content') or
            self.soup.body
        )
        
        if main:
            return self.html_to_markdown(main)
        return ""
    
    def html_to_markdown(self, element):
        """Convert HTML element to Markdown"""
        markdown = []
        
        for child in element.children:
            if child.name is None:
                # Text node
                text = str(child).strip()
                if text:
                    markdown.append(text)
            elif child.name == 'h1':
                markdown.append(f"# {child.get_text().strip()}")
            elif child.name == 'h2':
                markdown.append(f"## {child.get_text().strip()}")
            elif child.name == 'h3':
                markdown.append(f"### {child.get_text().strip()}")
            elif child.name == 'h4':
                markdown.append(f"#### {child.get_text().strip()}")
            elif child.name == 'p':
                markdown.append(child.get_text().strip())
            elif child.name == 'a':
                text = child.get_text().strip()
                href = child.get('href', '')
                markdown.append(f"[{text}]({href})")
            elif child.name == 'img':
                # Images will be handled separately
                alt = child.get('alt', 'image')
                src = child.get('src', '')
                markdown.append(f"![[{os.path.basename(src)} | alt=\"{alt}\"]]")
            elif child.name == 'ul':
                for li in child.find_all('li', recursive=False):
                    markdown.append(f"- {li.get_text().strip()}")
            elif child.name == 'ol':
                for i, li in enumerate(child.find_all('li', recursive=False), 1):
                    markdown.append(f"{i}. {li.get_text().strip()}")
            elif child.name in ['strong', 'b']:
                markdown.append(f"**{child.get_text().strip()}**")
            elif child.name in ['em', 'i']:
                markdown.append(f"*{child.get_text().strip()}*")
            elif child.name == 'code':
                markdown.append(f"`{child.get_text()}`")
            elif child.name == 'pre':
                code = child.find('code')
                if code:
                    markdown.append(f"```\n{code.get_text()}\n```")
                else:
                    markdown.append(f"```\n{child.get_text()}\n```")
            elif child.name in ['div', 'section', 'article']:
                # Recursively process
                markdown.append(self.html_to_markdown(child))
        
        return '\n\n'.join(filter(None, markdown))
    
    def extract_images(self, assets_dir):
        """Extract and download images"""
        assets_dir = Path(assets_dir)
        assets_dir.mkdir(parents=True, exist_ok=True)
        
        images = []
        img_tags = self.soup.find_all('img')
        
        for i, img in enumerate(img_tags, 1):
            src = img.get('src', '')
            if not src:
                continue
            
            # Make absolute URL if relative
            if self.base_url and not src.startswith('http'):
                src = urljoin(self.base_url, src)
            
            # Download image if URL
            filename = f"image_{i}{Path(urlparse(src).path).suffix or '.jpg'}"
            local_path = assets_dir / filename
            
            if src.startswith('http') and HAS_DEPS:
                try:
                    img_data = requests.get(src).content
                    with open(local_path, 'wb') as f:
                        f.write(img_data)
                    print(f"  Downloaded: {filename}")
                except Exception as e:
                    print(f"  Failed to download {src}: {e}")
                    continue
            
            images.append({
                'name': filename,
                'src': src,
                'local_path': local_path,
                'alt': img.get('alt', '')
            })
        
        return images
    
    def write_mdx(self, content, images, output_file):
        """Write MDX file"""
        # Extract title
        title_tag = self.soup.find('title')
        title = title_tag.get_text().strip() if title_tag else 'Untitled'
        
        mdx_content = f"""---
title: {title}
source: {self.base_url if self.is_url else 'local'}
platform: {self.platform}
converted: true
---

# {title}

{content}
"""
        
        with open(output_file, 'w', encoding='utf-8') as f:
            f.write(mdx_content)
    
    def write_mdm(self, images, output_file):
        """Write MDM metadata file"""
        resources = {}
        for img in images:
            resources[img['name']] = {
                'type': 'image',
                'src': f"assets/{img['name']}",
                'alt': img['alt']
            }
        
        mdm_data = {
            'version': '1.0',
            'resources': resources,
            'presets': {},
            'metadata': {
                'platform': self.platform,
                'converter': 'html_converter.py'
            }
        }
        
        with open(output_file, 'w', encoding='utf-8') as f:
            json.dump(mdm_data, f, indent=2, ensure_ascii=False)

def main():
    if len(sys.argv) < 3:
        print("Usage:")
        print("  python html_converter.py <input.html> <output_dir>")
        print("  python html_converter.py --url <blog_url> <output_dir>")
        sys.exit(1)
    
    if sys.argv[1] == '--url':
        html_source = sys.argv[2]
        output_dir = sys.argv[3] if len(sys.argv) > 3 else './output'
        is_url = True
    else:
        html_source = sys.argv[1]
        output_dir = sys.argv[2]
        is_url = False
    
    try:
        converter = HtmlToMdxConverter(html_source, is_url=is_url)
        result = converter.convert(output_dir)
        
        print("\n✅ Conversion complete!")
        print(json.dumps(result, indent=2, ensure_ascii=False))
    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()
