#!/usr/bin/env python3
"""
Table to SVG Renderer
Converts complex table structures to SVG images
"""
import sys
import json
from pathlib import Path

try:
    import svgwrite
    HAS_SVGWRITE = True
except ImportError:
    HAS_SVGWRITE = False
    print("Warning: svgwrite not installed. Run: pip install svgwrite")

def render_table_to_svg(table_data, output_path):
    """
    Render a table structure to SVG
    
    Args:
        table_data: Dictionary with table structure
        output_path: Output SVG file path
    """
    if not HAS_SVGWRITE:
        raise ImportError("svgwrite is required. Install with: pip install svgwrite")
    
    rows = table_data.get('rows', [])
    if not rows:
        raise ValueError("No rows in table data")
    
    # Calculate dimensions
    num_rows = len(rows)
    num_cols = max(len(row) for row in rows)
    cell_width = 100
    cell_height = 40
    padding = 10
    
    width = num_cols * cell_width + padding * 2
    height = num_rows * cell_height + padding * 2
    
    # Create SVG
    dwg = svgwrite.Drawing(output_path, size=(width, height))
    
    # Draw table
    for i, row in enumerate(rows):
        for j, cell in enumerate(row):
            x = j * cell_width + padding
            y = i * cell_height + padding
            
            # Cell rectangle
            dwg.add(dwg.rect(
                insert=(x, y),
                size=(cell_width, cell_height),
                fill='white',
                stroke='black',
                stroke_width=1
            ))
            
            # Cell text
            dwg.add(dwg.text(
                str(cell),
                insert=(x + cell_width/2, y + cell_height/2),
                text_anchor='middle',
                dominant_baseline='middle',
                font_size=12,
                font_family='Arial'
            ))
    
    dwg.save()
    print(f"✓ Created SVG: {output_path}")

def main():
    if len(sys.argv) < 3:
        print("Usage: python table_to_svg.py <table.json> <output.svg>")
        print("\nTable JSON format:")
        print('  {"rows": [["A", "B"], ["C", "D"]]}')
        sys.exit(1)
    
    input_path = sys.argv[1]
    output_path = sys.argv[2]
    
    try:
        with open(input_path, 'r') as f:
            table_data = json.load(f)
        
        render_table_to_svg(table_data, output_path)
        print("✅ Table rendered successfully!")
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
