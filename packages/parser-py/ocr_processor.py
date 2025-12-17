#!/usr/bin/env python3
"""
OCR Processor
Extracts text from images using OCR (Optical Character Recognition)
"""
import sys
import os
import json
from pathlib import Path

# Try to import OCR libraries
try:
    import pytesseract
    from PIL import Image
    HAS_TESSERACT = True
except ImportError:
    HAS_TESSERACT = False

try:
    import easyocr
    HAS_EASYOCR = True
except ImportError:
    HAS_EASYOCR = False

class OcrProcessor:
    def __init__(self, engine='auto', lang='kor+eng'):
        """
        Initialize OCR processor
        
        Args:
            engine: 'tesseract', 'easyocr', or 'auto'
            lang: Language code (default: Korean + English)
        """
        self.lang = lang
        
        if engine == 'auto':
            if HAS_EASYOCR:
                self.engine = 'easyocr'
            elif HAS_TESSERACT:
                self.engine = 'tesseract'
            else:
                raise ImportError(
                    "No OCR engine available. Install: "
                    "pip install pytesseract pillow  OR  pip install easyocr"
                )
        else:
            self.engine = engine
        
        # Initialize EasyOCR reader if using it
        if self.engine == 'easyocr' and HAS_EASYOCR:
            # Parse languages
            langs = ['ko', 'en'] if 'kor' in lang else ['en']
            self.reader = easyocr.Reader(langs)
    
    def extract_text(self, image_path):
        """
        Extract text from image
        
        Args:
            image_path: Path to image file
            
        Returns:
            Extracted text as string
        """
        image_path = Path(image_path)
        
        if not image_path.exists():
            raise FileNotFoundError(f"Image not found: {image_path}")
        
        if self.engine == 'tesseract':
            return self._extract_tesseract(image_path)
        elif self.engine == 'easyocr':
            return self._extract_easyocr(image_path)
        else:
            raise ValueError(f"Unknown engine: {self.engine}")
    
    def _extract_tesseract(self, image_path):
        """Extract text using Tesseract"""
        if not HAS_TESSERACT:
            raise ImportError("pytesseract not installed")
        
        image = Image.open(image_path)
        text = pytesseract.image_to_string(image, lang=self.lang)
        return text.strip()
    
    def _extract_easyocr(self, image_path):
        """Extract text using EasyOCR"""
        if not HAS_EASYOCR:
            raise ImportError("easyocr not installed")
        
        results = self.reader.readtext(str(image_path))
        texts = [result[1] for result in results]
        return '\n'.join(texts)
    
    def process_directory(self, input_dir, output_file=None):
        """
        Process all images in a directory
        
        Args:
            input_dir: Directory containing images
            output_file: Optional output file for combined text
            
        Returns:
            Dict mapping image names to extracted text
        """
        input_path = Path(input_dir)
        results = {}
        
        image_extensions = {'.png', '.jpg', '.jpeg', '.gif', '.bmp', '.tiff'}
        
        for image_file in input_path.iterdir():
            if image_file.suffix.lower() in image_extensions:
                print(f"Processing: {image_file.name}")
                try:
                    text = self.extract_text(image_file)
                    results[image_file.name] = text
                    print(f"  ✓ Extracted {len(text)} characters")
                except Exception as e:
                    print(f"  ✗ Error: {e}")
                    results[image_file.name] = f"Error: {e}"
        
        if output_file:
            with open(output_file, 'w', encoding='utf-8') as f:
                for name, text in results.items():
                    f.write(f"=== {name} ===\n{text}\n\n")
            print(f"✓ Saved combined output to: {output_file}")
        
        return results

def main():
    if len(sys.argv) < 2:
        print("Usage:")
        print("  python ocr_processor.py <image.png>")
        print("  python ocr_processor.py --dir <directory> [output.txt]")
        print("  python ocr_processor.py --engine tesseract <image.png>")
        print("  python ocr_processor.py --engine easyocr <image.png>")
        print("\nSupported engines: tesseract, easyocr, auto")
        print("\nInstall:")
        print("  pip install pytesseract pillow   # For Tesseract")
        print("  pip install easyocr              # For EasyOCR")
        sys.exit(1)
    
    # Parse args
    engine = 'auto'
    args = sys.argv[1:]
    
    if '--engine' in args:
        idx = args.index('--engine')
        engine = args[idx + 1]
        args = args[:idx] + args[idx+2:]
    
    try:
        processor = OcrProcessor(engine=engine)
        
        if args[0] == '--dir':
            input_dir = args[1]
            output_file = args[2] if len(args) > 2 else None
            results = processor.process_directory(input_dir, output_file)
            print(f"\n✅ Processed {len(results)} images")
        else:
            image_path = args[0]
            text = processor.extract_text(image_path)
            print("\n=== Extracted Text ===")
            print(text)
    except ImportError as e:
        print(f"❌ {e}")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
