import sys
import os
try:
    import pdfplumber
except ImportError:
    pdfplumber = None

class PdfProcessor:
    def __init__(self, file_path):
        self.file_path = file_path
        self.validate_file()

    def validate_file(self):
        if not os.path.exists(self.file_path):
            raise FileNotFoundError(f"File not found: {self.file_path}")
        if not self.file_path.lower().endswith('.pdf'):
             raise ValueError("Not a valid PDF file")

    def extract_text(self):
        """
        Extract text from PDF
        """
        if not pdfplumber:
            print("pdfplumber not installed")
            return

        with pdfplumber.open(self.file_path) as pdf:
            text = []
            for page in pdf.pages:
                text.append(page.extract_text())
            return "\n".join(text)

    def extract_images(self, output_dir):
        """
        Extract images from PDF
        """
        os.makedirs(output_dir, exist_ok=True)
        
        if not pdfplumber:
            print("pdfplumber not installed - cannot extract images")
            return []
        
        images = []
        
        with pdfplumber.open(self.file_path) as pdf:
            for page_num, page in enumerate(pdf.pages):
                if hasattr(page, 'images'):
                    for img_num, img in enumerate(page.images):
                        # Save image info
                        img_filename = f"page{page_num+1}_img{img_num+1}.png"
                        img_path = os.path.join(output_dir, img_filename)
                        
                        images.append({
                            'page': page_num + 1,
                            'filename': img_filename,
                            'path': img_path,
                            'bbox': (img.get('x0'), img.get('top'), 
                                   img.get('x1'), img.get('bottom'))
                        })
        
        return images
    
    def extract_metadata(self):
        """
        Extract PDF metadata
        """
        if not pdfplumber:
            return {}
        
        metadata = {}
        with pdfplumber.open(self.file_path) as pdf:
            metadata = {
                'pages': len(pdf.pages),
                'metadata': pdf.metadata if hasattr(pdf, 'metadata') else {}
            }
        
        return metadata

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python pdf_processor.py <input.pdf> [--extract-images output_dir]")
        sys.exit(1)
    
    processor = PdfProcessor(sys.argv[1])
    
    if len(sys.argv) >= 4 and sys.argv[2] == '--extract-images':
        images = processor.extract_images(sys.argv[3])
        print(f"Extracted {len(images)} images to {sys.argv[3]}")
    else:
        print("=== Text Content ===")
        print(processor.extract_text())
        print("\n=== Metadata ===")
        import json
        print(json.dumps(processor.extract_metadata(), indent=2))
