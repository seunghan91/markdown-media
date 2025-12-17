#!/usr/bin/env python3
"""
Test suite for Python converters
"""
import unittest
import os
import sys
import tempfile
import shutil
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

class TestHwpToSvg(unittest.TestCase):
    """Test HWP to SVG converter"""
    
    def test_import(self):
        """Test module import"""
        from hwp_to_svg import HwpToSvgConverter
        self.assertTrue(True)
    
    def test_file_validation(self):
        """Test file validation"""
        from hwp_to_svg import HwpToSvgConverter
        
        with self.assertRaises(FileNotFoundError):
            HwpToSvgConverter('nonexistent.hwp')
        
        # Create temp file with wrong extension
        with tempfile.NamedTemporaryFile(suffix='.txt', delete=False) as f:
            temp_path = f.name
        
        try:
            with self.assertRaises(ValueError):
                HwpToSvgConverter(temp_path)
        finally:
            os.unlink(temp_path)

class TestPdfProcessor(unittest.TestCase):
    """Test PDF processor"""
    
    def test_import(self):
        """Test module import"""
        from pdf_processor import PdfProcessor
        self.assertTrue(True)
    
    def test_file_validation(self):
        """Test file validation"""
        from pdf_processor import PdfProcessor
        
        with self.assertRaises(FileNotFoundError):
            PdfProcessor('nonexistent.pdf')

class TestOcrProcessor(unittest.TestCase):
    """Test OCR processor"""
    
    def test_import(self):
        """Test module import"""
        from ocr_processor import OcrProcessor, HAS_TESSERACT, HAS_EASYOCR
        self.assertTrue(True)
    
    def test_engine_detection(self):
        """Test OCR engine detection"""
        from ocr_processor import HAS_TESSERACT, HAS_EASYOCR
        
        # At least report available engines
        print(f"\nOCR Engines available:")
        print(f"  Tesseract: {HAS_TESSERACT}")
        print(f"  EasyOCR: {HAS_EASYOCR}")

class TestIntegration(unittest.TestCase):
    """Integration tests"""
    
    def setUp(self):
        """Set up test directory"""
        self.test_dir = tempfile.mkdtemp()
    
    def tearDown(self):
        """Clean up test directory"""
        shutil.rmtree(self.test_dir)
    
    def test_output_directory_creation(self):
        """Test that output directories are created"""
        output_dir = os.path.join(self.test_dir, 'output', 'nested')
        os.makedirs(output_dir, exist_ok=True)
        self.assertTrue(os.path.exists(output_dir))

if __name__ == '__main__':
    print("=" * 50)
    print("Python Converter Tests")
    print("=" * 50)
    unittest.main(verbosity=2)
