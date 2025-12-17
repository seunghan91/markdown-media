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
        pass

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python pdf_processor.py <input.pdf>")
        sys.exit(1)
    
    processor = PdfProcessor(sys.argv[1])
    print(processor.extract_text())
