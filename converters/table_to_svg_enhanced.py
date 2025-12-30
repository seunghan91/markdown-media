#!/usr/bin/env python3
"""
Enhanced Table to SVG Renderer

Features:
- Merged cell support (rowspan, colspan)
- Customizable styling (colors, fonts, borders)
- Multiple output formats (SVG, PNG via cairosvg)
- Integration with Rust mdm-core table output
- Markdown table parsing
- Korean text support with proper font handling
"""
import os
import sys
import json
import re
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any, Union
from dataclasses import dataclass, field

try:
    import svgwrite
    HAS_SVGWRITE = True
except ImportError:
    HAS_SVGWRITE = False

try:
    import cairosvg
    HAS_CAIROSVG = True
except ImportError:
    HAS_CAIROSVG = False


@dataclass
class CellStyle:
    """Cell styling configuration"""
    background_color: str = "#FFFFFF"
    text_color: str = "#333333"
    border_color: str = "#CCCCCC"
    border_width: float = 1.0
    font_family: str = "Noto Sans KR, Arial, sans-serif"
    font_size: int = 14
    font_weight: str = "normal"  # normal, bold
    text_align: str = "center"  # left, center, right
    vertical_align: str = "middle"  # top, middle, bottom
    padding: int = 8


@dataclass
class TableStyle:
    """Table styling configuration"""
    header_style: CellStyle = field(default_factory=lambda: CellStyle(
        background_color="#4A90D9",
        text_color="#FFFFFF",
        font_weight="bold",
    ))
    cell_style: CellStyle = field(default_factory=CellStyle)
    alt_row_color: Optional[str] = "#F8F9FA"  # None to disable alternating
    min_cell_width: int = 80
    min_cell_height: int = 40
    table_padding: int = 10
    border_collapse: bool = True
    shadow: bool = False
    rounded_corners: int = 0  # 0 for sharp corners


@dataclass
class TableCell:
    """Table cell with content and span information"""
    content: str
    row: int
    col: int
    row_span: int = 1
    col_span: int = 1
    is_header: bool = False
    style: Optional[CellStyle] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "content": self.content,
            "row": self.row,
            "col": self.col,
            "row_span": self.row_span,
            "col_span": self.col_span,
            "is_header": self.is_header,
        }


@dataclass
class Table:
    """Table structure with cells and dimensions"""
    cells: List[TableCell]
    row_count: int
    col_count: int
    has_header: bool = True
    col_widths: Optional[List[int]] = None
    row_heights: Optional[List[int]] = None

    @classmethod
    def from_markdown(cls, markdown: str) -> "Table":
        """Parse markdown table to Table structure"""
        lines = [line.strip() for line in markdown.strip().split("\n") if line.strip()]

        if not lines:
            raise ValueError("Empty markdown table")

        cells = []
        row_count = 0
        col_count = 0

        for line_idx, line in enumerate(lines):
            # Skip separator row (|---|---|)
            if re.match(r"^\|[\s\-:|]+\|$", line):
                continue

            # Parse cells
            parts = [p.strip() for p in line.split("|")]
            # Remove empty parts from leading/trailing pipes
            # Use enumerate to avoid index() bug with duplicate values
            parts = [p for idx, p in enumerate(parts) if p or idx not in (0, len(parts) - 1)]

            if not parts:
                continue

            col_count = max(col_count, len(parts))
            is_header = row_count == 0

            for col_idx, content in enumerate(parts):
                cells.append(TableCell(
                    content=content,
                    row=row_count,
                    col=col_idx,
                    is_header=is_header,
                ))

            row_count += 1

        return cls(
            cells=cells,
            row_count=row_count,
            col_count=col_count,
            has_header=True,
        )

    @classmethod
    def from_rust_output(cls, data: Dict[str, Any]) -> "Table":
        """Parse Rust mdm-core table output to Table structure"""
        cells = []

        # Handle different formats
        if "cells" in data:
            # Full cell structure with spans
            for cell_data in data["cells"]:
                cells.append(TableCell(
                    content=cell_data.get("content", ""),
                    row=cell_data.get("row", 0),
                    col=cell_data.get("col", 0),
                    row_span=cell_data.get("row_span", 1),
                    col_span=cell_data.get("col_span", 1),
                    is_header=cell_data.get("is_header", False),
                ))

            row_count = data.get("row_count", max(c.row for c in cells) + 1 if cells else 0)
            col_count = data.get("col_count", max(c.col for c in cells) + 1 if cells else 0)

        elif "rows" in data:
            # Simple 2D array format
            rows = data["rows"]
            row_count = len(rows)
            col_count = max(len(row) for row in rows) if rows else 0

            for row_idx, row in enumerate(rows):
                for col_idx, content in enumerate(row):
                    cells.append(TableCell(
                        content=str(content),
                        row=row_idx,
                        col=col_idx,
                        is_header=(row_idx == 0 and data.get("has_header", True)),
                    ))

        else:
            raise ValueError("Unknown table format")

        return cls(
            cells=cells,
            row_count=row_count,
            col_count=col_count,
            has_header=data.get("has_header", True),
            col_widths=data.get("col_widths"),
            row_heights=data.get("row_heights"),
        )

    @classmethod
    def from_json(cls, json_path: str) -> "Table":
        """Load table from JSON file"""
        with open(json_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        return cls.from_rust_output(data)


class TableSvgRenderer:
    """
    Enhanced SVG renderer for tables with merged cell support.
    """

    def __init__(self, style: Optional[TableStyle] = None):
        """
        Initialize renderer with style configuration.

        Args:
            style: TableStyle configuration (uses defaults if None)
        """
        if not HAS_SVGWRITE:
            raise ImportError("svgwrite required: pip install svgwrite")

        self.style = style or TableStyle()
        self._occupied: Dict[Tuple[int, int], bool] = {}

    def _calculate_dimensions(
        self,
        table: Table,
    ) -> Tuple[List[int], List[int], int, int]:
        """Calculate cell dimensions based on content"""
        # Initialize with minimum sizes
        col_widths = table.col_widths or [self.style.min_cell_width] * table.col_count
        row_heights = table.row_heights or [self.style.min_cell_height] * table.row_count

        # Expand to fit provided widths/heights if smaller
        col_widths = [max(w, self.style.min_cell_width) for w in col_widths]
        row_heights = [max(h, self.style.min_cell_height) for h in row_heights]

        # Adjust for content (estimate based on character count)
        for cell in table.cells:
            content_len = len(cell.content)
            char_width = self.style.cell_style.font_size * 0.6

            # Estimated width needed
            cell_content_width = int(content_len * char_width + self.style.cell_style.padding * 2)
            cell_width = cell_content_width // cell.col_span

            for c in range(cell.col, cell.col + cell.col_span):
                if c < len(col_widths):
                    col_widths[c] = max(col_widths[c], cell_width)

        # Calculate total dimensions
        total_width = sum(col_widths) + self.style.table_padding * 2
        total_height = sum(row_heights) + self.style.table_padding * 2

        return col_widths, row_heights, total_width, total_height

    def _get_cell_rect(
        self,
        cell: TableCell,
        col_widths: List[int],
        row_heights: List[int],
    ) -> Tuple[int, int, int, int]:
        """Get cell rectangle coordinates (x, y, width, height)"""
        x = sum(col_widths[:cell.col]) + self.style.table_padding
        y = sum(row_heights[:cell.row]) + self.style.table_padding

        width = sum(col_widths[cell.col:cell.col + cell.col_span])
        height = sum(row_heights[cell.row:cell.row + cell.row_span])

        return x, y, width, height

    def _get_cell_style(self, cell: TableCell, row_idx: int) -> CellStyle:
        """Get style for a cell"""
        if cell.style:
            return cell.style

        if cell.is_header:
            return self.style.header_style

        style = CellStyle(
            background_color=self.style.cell_style.background_color,
            text_color=self.style.cell_style.text_color,
            border_color=self.style.cell_style.border_color,
            border_width=self.style.cell_style.border_width,
            font_family=self.style.cell_style.font_family,
            font_size=self.style.cell_style.font_size,
            font_weight=self.style.cell_style.font_weight,
            text_align=self.style.cell_style.text_align,
            vertical_align=self.style.cell_style.vertical_align,
            padding=self.style.cell_style.padding,
        )

        # Alternating row colors
        if self.style.alt_row_color and row_idx % 2 == 1 and not cell.is_header:
            style.background_color = self.style.alt_row_color

        return style

    def render(
        self,
        table: Table,
        output_path: str,
        title: Optional[str] = None,
    ) -> str:
        """
        Render table to SVG file.

        Args:
            table: Table structure to render
            output_path: Output SVG file path
            title: Optional title to display above table

        Returns:
            Path to generated SVG file
        """
        col_widths, row_heights, total_width, total_height = self._calculate_dimensions(table)

        # Add space for title if provided
        title_height = 40 if title else 0
        total_height += title_height

        # Create SVG drawing
        dwg = svgwrite.Drawing(
            output_path,
            size=(total_width, total_height),
            profile="full",
        )

        # Add styles for text
        dwg.defs.add(dwg.style(f"""
            text {{
                dominant-baseline: middle;
            }}
            .korean-text {{
                font-family: 'Noto Sans KR', 'Malgun Gothic', sans-serif;
            }}
        """))

        # Background
        if self.style.shadow:
            # Shadow effect
            dwg.add(dwg.rect(
                insert=(3, 3),
                size=(total_width - 3, total_height - 3),
                fill="#00000020",
                rx=self.style.rounded_corners,
                ry=self.style.rounded_corners,
            ))

        dwg.add(dwg.rect(
            insert=(0, 0),
            size=(total_width, total_height),
            fill="white",
            rx=self.style.rounded_corners,
            ry=self.style.rounded_corners,
        ))

        # Title
        if title:
            dwg.add(dwg.text(
                title,
                insert=(total_width / 2, 25),
                text_anchor="middle",
                font_size=18,
                font_weight="bold",
                font_family="Noto Sans KR, Arial, sans-serif",
                fill="#333333",
            ))

        # Track occupied cells for merged cells
        self._occupied.clear()

        # Sort cells by position for proper rendering order
        sorted_cells = sorted(table.cells, key=lambda c: (c.row, c.col))

        # Render cells
        for cell in sorted_cells:
            # Skip if this position is already occupied by a merged cell
            if (cell.row, cell.col) in self._occupied:
                continue

            # Mark occupied cells for spans
            for r in range(cell.row, cell.row + cell.row_span):
                for c in range(cell.col, cell.col + cell.col_span):
                    self._occupied[(r, c)] = True

            x, y, width, height = self._get_cell_rect(cell, col_widths, row_heights)
            y += title_height  # Offset for title

            style = self._get_cell_style(cell, cell.row)

            # Cell background
            dwg.add(dwg.rect(
                insert=(x, y),
                size=(width, height),
                fill=style.background_color,
                stroke=style.border_color,
                stroke_width=style.border_width,
            ))

            # Cell text
            text_x = x + width / 2 if style.text_align == "center" else \
                     x + style.padding if style.text_align == "left" else \
                     x + width - style.padding

            text_y = y + height / 2 if style.vertical_align == "middle" else \
                     y + style.padding + style.font_size if style.vertical_align == "top" else \
                     y + height - style.padding

            text_anchor = "middle" if style.text_align == "center" else \
                         "start" if style.text_align == "left" else "end"

            # Handle multi-line text
            lines = cell.content.split("\n")
            line_height = style.font_size * 1.2

            if len(lines) > 1:
                # Adjust starting y for multi-line
                text_y = y + (height - len(lines) * line_height) / 2 + style.font_size

            for i, line in enumerate(lines):
                dwg.add(dwg.text(
                    line,
                    insert=(text_x, text_y + i * line_height),
                    text_anchor=text_anchor,
                    font_size=style.font_size,
                    font_weight=style.font_weight,
                    font_family=style.font_family,
                    fill=style.text_color,
                    class_="korean-text" if self._has_korean(line) else None,
                ))

        dwg.save()
        return output_path

    def render_to_png(
        self,
        table: Table,
        output_path: str,
        scale: float = 2.0,
        title: Optional[str] = None,
    ) -> str:
        """
        Render table to PNG file (requires cairosvg).

        Args:
            table: Table structure to render
            output_path: Output PNG file path
            scale: Scale factor for PNG resolution
            title: Optional title

        Returns:
            Path to generated PNG file
        """
        if not HAS_CAIROSVG:
            raise ImportError("cairosvg required for PNG export: pip install cairosvg")

        # Render to temporary SVG first
        svg_path = output_path.replace(".png", ".tmp.svg")
        self.render(table, svg_path, title)

        # Convert to PNG
        cairosvg.svg2png(
            url=svg_path,
            write_to=output_path,
            scale=scale,
        )

        # Clean up temporary SVG
        os.remove(svg_path)

        return output_path

    @staticmethod
    def _has_korean(text: str) -> bool:
        """Check if text contains Korean characters"""
        return any("\uac00" <= char <= "\ud7a3" for char in text)


def render_markdown_table_to_svg(
    markdown: str,
    output_path: str,
    style: Optional[TableStyle] = None,
    title: Optional[str] = None,
) -> str:
    """
    Convenience function to render markdown table to SVG.

    Args:
        markdown: Markdown table string
        output_path: Output SVG file path
        style: Optional TableStyle
        title: Optional title

    Returns:
        Path to generated SVG file
    """
    table = Table.from_markdown(markdown)
    renderer = TableSvgRenderer(style)
    return renderer.render(table, output_path, title)


def render_json_table_to_svg(
    json_path: str,
    output_path: str,
    style: Optional[TableStyle] = None,
    title: Optional[str] = None,
) -> str:
    """
    Convenience function to render JSON table to SVG.

    Args:
        json_path: Path to JSON table file
        output_path: Output SVG file path
        style: Optional TableStyle
        title: Optional title

    Returns:
        Path to generated SVG file
    """
    table = Table.from_json(json_path)
    renderer = TableSvgRenderer(style)
    return renderer.render(table, output_path, title)


def main():
    """CLI entry point"""
    import argparse

    parser = argparse.ArgumentParser(
        description="Enhanced Table to SVG Renderer with merged cell support",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Render markdown table
  python table_to_svg_enhanced.py --markdown "| A | B |\\n|---|---|\\n| 1 | 2 |" -o table.svg

  # Render from JSON file
  python table_to_svg_enhanced.py --json table.json -o table.svg

  # Render to PNG with custom title
  python table_to_svg_enhanced.py --json table.json -o table.png --format png --title "My Table"

  # Use dark theme
  python table_to_svg_enhanced.py --json table.json -o table.svg --theme dark
        """,
    )

    input_group = parser.add_mutually_exclusive_group(required=True)
    input_group.add_argument(
        "--markdown", "-m",
        help="Markdown table string",
    )
    input_group.add_argument(
        "--json", "-j",
        help="Path to JSON table file",
    )
    input_group.add_argument(
        "--file", "-f",
        help="Path to file containing markdown table",
    )

    parser.add_argument(
        "-o", "--output",
        required=True,
        help="Output file path",
    )
    parser.add_argument(
        "--format",
        choices=["svg", "png"],
        default="svg",
        help="Output format (default: svg)",
    )
    parser.add_argument(
        "--title",
        help="Table title",
    )
    parser.add_argument(
        "--theme",
        choices=["default", "dark", "minimal"],
        default="default",
        help="Table theme (default: default)",
    )
    parser.add_argument(
        "--scale",
        type=float,
        default=2.0,
        help="PNG scale factor (default: 2.0)",
    )

    args = parser.parse_args()

    # Configure style based on theme
    if args.theme == "dark":
        style = TableStyle(
            header_style=CellStyle(
                background_color="#2C3E50",
                text_color="#FFFFFF",
                font_weight="bold",
            ),
            cell_style=CellStyle(
                background_color="#34495E",
                text_color="#ECF0F1",
                border_color="#2C3E50",
            ),
            alt_row_color="#3D566E",
        )
    elif args.theme == "minimal":
        style = TableStyle(
            header_style=CellStyle(
                background_color="#FFFFFF",
                text_color="#333333",
                font_weight="bold",
                border_color="#E0E0E0",
            ),
            cell_style=CellStyle(
                background_color="#FFFFFF",
                text_color="#666666",
                border_color="#E0E0E0",
            ),
            alt_row_color=None,
        )
    else:
        style = TableStyle()

    try:
        # Parse input
        if args.markdown:
            table = Table.from_markdown(args.markdown.replace("\\n", "\n"))
        elif args.json:
            table = Table.from_json(args.json)
        elif args.file:
            with open(args.file, "r", encoding="utf-8") as f:
                table = Table.from_markdown(f.read())

        renderer = TableSvgRenderer(style)

        # Render output
        if args.format == "png":
            output_path = renderer.render_to_png(table, args.output, args.scale, args.title)
        else:
            output_path = renderer.render(table, args.output, args.title)

        print(f"Created: {output_path}")

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
