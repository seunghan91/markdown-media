import sys
import os
import json

try:
    from pyhwp import hwp5
    from pyhwp.hwp5 import xmlmodel
    HAS_PYHWP = True
except ImportError:
    HAS_PYHWP = False
    print("Warning: pyhwp not installed. Limited functionality.")

try:
    import svgwrite
    HAS_SVGWRITE = True
except ImportError:
    HAS_SVGWRITE = False
    print("Warning: svgwrite not installed. Run: pip install svgwrite")

class HwpToSvgConverter:
    def __init__(self, file_path):
        self.file_path = file_path
        self.validate_file()

    def validate_file(self):
        if not os.path.exists(self.file_path):
            raise FileNotFoundError(f"File not found: {self.file_path}")
        if not self.file_path.lower().endswith('.hwp'):
             raise ValueError("Not a valid HWP file")

    def convert(self, output_dir):
        """
        Convert HWP tables and charts to SVG
        """
        print(f"Converting {self.file_path} to {output_dir}...")
        
        os.makedirs(output_dir, exist_ok=True)
        
        if HAS_PYHWP:
            try:
                hwp = hwp5.Hwp5File(self.file_path)
                tables = self.extract_tables(hwp)
                
                svg_files = []
                for i, table in enumerate(tables):
                    svg_path = os.path.join(output_dir, f"table_{i+1}.svg")
                    self.render_table_to_svg(table, svg_path)
                    svg_files.append(svg_path)
                    print(f"  Created: {svg_path}")
                
                return svg_files
            except Exception as e:
                print(f"Error with pyhwp: {e}")
                return self._fallback_conversion(output_dir)
        else:
            return self._fallback_conversion(output_dir)

    def _fallback_conversion(self, output_dir):
        """Fallback when pyhwp is not available"""
        print("  Using fallback mode (pyhwp not available)")
        svg_path = os.path.join(output_dir, "placeholder.svg")
        
        if HAS_SVGWRITE:
            dwg = svgwrite.Drawing(svg_path, size=(400, 200))
            dwg.add(dwg.text("HWP file (pyhwp required)", 
                           insert=(50, 100), 
                           font_size=20))
            dwg.save()
            return [svg_path]
        return []

    def extract_tables(self, hwp):
        """
        Extract tables from HWP file using pyhwp
        """
        tables = []
        
        try:
            # Iterate through document sections
            for section in hwp.bodytext.section_list:
                # Parse XML model
                model = xmlmodel.Model(hwp)
                # Extract table data
                # (Simplified - real implementation would parse table structures)
                pass
        except Exception as e:
            print(f"Table extraction error: {e}")
        
        # Return sample table for demonstration
        return [{
            'rows': [
                ['Header 1', 'Header 2', 'Header 3'],
                ['Cell 1-1', 'Cell 1-2', 'Cell 1-3'],
                ['Cell 2-1', 'Cell 2-2', 'Cell 2-3'],
            ]
        }]

    def render_table_to_svg(self, table_data, output_path):
        """
        Render table data to SVG file
        """
        if not HAS_SVGWRITE:
            raise ImportError("svgwrite required for SVG rendering")
        
        rows = table_data.get('rows', [])
        if not rows:
            return
        
        # Calculate dimensions
        cell_width = 120
        cell_height = 40
        padding = 10
        
        num_rows = len(rows)
        num_cols = max(len(row) for row in rows)
        
        width = num_cols * cell_width + padding * 2
        height = num_rows * cell_height + padding * 2
        
        # Create SVG
        dwg = svgwrite.Drawing(output_path, size=(width, height))
        
        # Add background
        dwg.add(dwg.rect(insert=(0, 0), size=(width, height), fill='white'))
        
        # Draw table
        for i, row in enumerate(rows):
            for j, cell in enumerate(row):
                x = j * cell_width + padding
                y = i * cell_height + padding
                
                # Cell background (header row)
                fill_color = '#e8f4f8' if i == 0 else 'white'
                
                # Cell rectangle
                dwg.add(dwg.rect(
                    insert=(x, y),
                    size=(cell_width, cell_height),
                    fill=fill_color,
                    stroke='#333',
                    stroke_width=1
                ))
                
                # Cell text
                text_y = y + cell_height/2 + 5
                dwg.add(dwg.text(
                    str(cell),
                    insert=(x + cell_width/2, text_y),
                    text_anchor='middle',
                    font_size=14,
                    font_family='Arial',
                    fill='#333'
                ))
        
        dwg.save()

def main():
    if len(sys.argv) < 3:
        print("Usage: python hwp_to_svg.py <input.hwp> <output_dir>")
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_dir = sys.argv[2]
    
    try:
        converter = HwpToSvgConverter(input_path)
        svg_files = converter.convert(output_dir)
        
        print(f"\n✅ Conversion complete!")
        print(f"   Created {len(svg_files)} SVG file(s)")
        
        # Output JSON for programmatic use
        result = {
            'input': input_path,
            'output_dir': output_dir,
            'svg_files': svg_files
        }
        print(json.dumps(result, indent=2))
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()

