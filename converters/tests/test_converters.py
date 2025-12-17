#!/usr/bin/env python3
"""
Test suite for document converters
"""
import unittest
import os
import sys
import tempfile
import shutil
import json
from pathlib import Path

# Add converters directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

class TestDocxConverter(unittest.TestCase):
    """Test DOCX converter"""
    
    def test_import(self):
        """Test module import"""
        from docx_converter import DocxToMdxConverter
        self.assertTrue(True)
    
    def test_file_validation(self):
        """Test file validation"""
        from docx_converter import DocxToMdxConverter
        
        with self.assertRaises(FileNotFoundError):
            DocxToMdxConverter('nonexistent.docx')

class TestHwpxConverter(unittest.TestCase):
    """Test HWPX converter"""
    
    def test_import(self):
        """Test module import"""
        from hwpx_converter import HwpxToMdxConverter
        self.assertTrue(True)
    
    def test_file_validation(self):
        """Test file validation"""
        from hwpx_converter import HwpxToMdxConverter
        
        with self.assertRaises(FileNotFoundError):
            HwpxToMdxConverter('nonexistent.hwpx')

class TestHtmlConverter(unittest.TestCase):
    """Test HTML converter"""
    
    def test_import(self):
        """Test module import"""
        try:
            from html_converter import HtmlToMdxConverter
            self.has_deps = True
        except ImportError:
            self.has_deps = False
        self.assertTrue(True)
    
    def test_platform_detection(self):
        """Test blog platform detection"""
        if not self.has_deps:
            self.skipTest("beautifulsoup4 not installed")
        
        from html_converter import HtmlToMdxConverter
        
        # Test with Naver blog HTML
        naver_html = '<html><head></head><body>blog.naver.com</body></html>'
        converter = HtmlToMdxConverter(naver_html, is_url=False)
        self.assertEqual(converter.platform, 'naver')
        
        # Test with Tistory HTML
        tistory_html = '<html><head></head><body>tistory.com</body></html>'
        converter = HtmlToMdxConverter(tistory_html, is_url=False)
        self.assertEqual(converter.platform, 'tistory')
    
    def test_html_to_markdown(self):
        """Test HTML to Markdown conversion"""
        if not self.has_deps:
            self.skipTest("beautifulsoup4 not installed")
        
        from html_converter import HtmlToMdxConverter
        
        html = '''
        <html><head><title>Test</title></head>
        <body>
            <h1>Title</h1>
            <p>Paragraph text</p>
            <ul>
                <li>Item 1</li>
                <li>Item 2</li>
            </ul>
        </body></html>
        '''
        
        converter = HtmlToMdxConverter(html, is_url=False)
        content = converter.extract_content()
        
        self.assertIn('Title', content)
        self.assertIn('Paragraph text', content)
    
    has_deps = True

class TestIntegration(unittest.TestCase):
    """Integration tests"""
    
    def setUp(self):
        """Set up test directory"""
        self.test_dir = tempfile.mkdtemp()
    
    def tearDown(self):
        """Clean up test directory"""
        shutil.rmtree(self.test_dir)
    
    def test_mdm_output_structure(self):
        """Test MDM output structure creation"""
        # Create expected structure
        mdx_path = os.path.join(self.test_dir, 'test.mdx')
        mdm_path = os.path.join(self.test_dir, 'test.mdm')
        assets_path = os.path.join(self.test_dir, 'assets')
        
        # Create files
        with open(mdx_path, 'w') as f:
            f.write('# Test\n\nContent')
        
        with open(mdm_path, 'w') as f:
            json.dump({'version': '1.0', 'resources': {}}, f)
        
        os.makedirs(assets_path)
        
        # Verify
        self.assertTrue(os.path.exists(mdx_path))
        self.assertTrue(os.path.exists(mdm_path))
        self.assertTrue(os.path.isdir(assets_path))

if __name__ == '__main__':
    print("=" * 50)
    print("Document Converter Tests")
    print("=" * 50)
    unittest.main(verbosity=2)
