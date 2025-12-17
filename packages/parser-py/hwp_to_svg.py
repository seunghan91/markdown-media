import sys
import os

class HwpToSvgConverter:
    def __init__(self, file_path):
        self.file_path = file_path
        self.validate_file()

    def validate_file(self):
        if not os.path.exists(self.file_path):
            raise FileNotFoundError(f"File not found: {self.file_path}")
        if not self.file_path.lower().endswith('.hwp'):
             raise ValueError("Not a valid HWP file")

    def convert(self, output_path):
        """
        Convert HWP tables and charts to SVG
        """
        print(f"Converting {self.file_path} to {output_path}...")
        # TODO: Implement connection with pyhwp
        pass

    def extract_tables(self):
        """
        Extract tables from HWP file
        """
        pass

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python hwp_to_svg.py <input.hwp> <output.svg>")
        sys.exit(1)
    
    converter = HwpToSvgConverter(sys.argv[1])
    converter.convert(sys.argv[2])
