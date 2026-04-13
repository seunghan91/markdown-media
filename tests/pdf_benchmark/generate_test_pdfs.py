#!/usr/bin/env python3
"""Generate comprehensive test PDF files for parser benchmarking.
Uses reportlab for precise PDF control."""

import subprocess, os, sys

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))

def check_reportlab():
    try:
        import reportlab
        return True
    except ImportError:
        subprocess.run([sys.executable, "-m", "pip", "install", "reportlab", "--quiet"], check=True)
        return True

check_reportlab()

from reportlab.lib.pagesizes import A4
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib.units import mm, cm
from reportlab.lib.enums import TA_LEFT, TA_CENTER, TA_JUSTIFY
from reportlab.lib.colors import black, blue, red, grey
from reportlab.platypus import (
    SimpleDocTemplate, Paragraph, Spacer, Table, TableStyle,
    PageBreak, ListFlowable, ListItem, HRFlowable
)
from reportlab.lib import colors


def create_comprehensive_test():
    """Test 1: Comprehensive document with headings, lists, tables, formatting."""
    path = os.path.join(BENCH_DIR, "test_comprehensive.pdf")
    doc = SimpleDocTemplate(path, pagesize=A4,
                           title="MDM PDF Parser Benchmark",
                           author="Seunghan Kim")
    styles = getSampleStyleSheet()

    # Custom styles
    h1 = ParagraphStyle('H1', parent=styles['Heading1'], fontSize=24, spaceAfter=12)
    h2 = ParagraphStyle('H2', parent=styles['Heading2'], fontSize=18, spaceAfter=10)
    h3 = ParagraphStyle('H3', parent=styles['Heading3'], fontSize=14, spaceAfter=8)
    body = ParagraphStyle('Body', parent=styles['Normal'], fontSize=11, spaceAfter=6,
                          leading=16)
    bold_style = ParagraphStyle('Bold', parent=body, fontName='Helvetica-Bold')

    story = []

    # Title
    story.append(Paragraph("Document Title", h1))
    story.append(Paragraph(
        "This is body text under the main heading. It tests basic paragraph extraction "
        "from PDF documents. The MDM parser should preserve this text accurately.", body))
    story.append(Spacer(1, 12))

    # Section with formatting
    story.append(Paragraph("Section One", h2))
    story.append(Paragraph("Body text in section one with normal formatting.", body))
    story.append(Spacer(1, 8))

    story.append(Paragraph("Subsection 1.1", h3))
    story.append(Paragraph(
        "This paragraph contains <b>bold text</b>, <i>italic text</i>, "
        "and <b><i>bold italic</i></b> formatting.", body))
    story.append(Spacer(1, 12))

    # Bullet list
    story.append(Paragraph("List Tests", h2))
    bullet_items = [
        ListItem(Paragraph("First bullet item", body)),
        ListItem(Paragraph("Second bullet item", body)),
        ListItem(Paragraph("Third bullet item", body)),
    ]
    story.append(ListFlowable(bullet_items, bulletType='bullet', start=''))
    story.append(Spacer(1, 8))

    # Numbered list
    num_items = [
        ListItem(Paragraph("First numbered item", body)),
        ListItem(Paragraph("Second numbered item", body)),
        ListItem(Paragraph("Third numbered item", body)),
    ]
    story.append(ListFlowable(num_items, bulletType='1'))
    story.append(Spacer(1, 12))

    # Simple table
    story.append(Paragraph("Table Tests", h2))
    table_data = [
        ['Name', 'Age', 'City'],
        ['Alice', '30', 'Seoul'],
        ['Bob', '25', 'Busan'],
        ['Charlie', '35', 'Daejeon'],
    ]
    t = Table(table_data, colWidths=[120, 60, 120])
    t.setStyle(TableStyle([
        ('BACKGROUND', (0, 0), (-1, 0), colors.grey),
        ('TEXTCOLOR', (0, 0), (-1, 0), colors.white),
        ('FONTNAME', (0, 0), (-1, 0), 'Helvetica-Bold'),
        ('ALIGN', (0, 0), (-1, -1), 'LEFT'),
        ('GRID', (0, 0), (-1, -1), 1, colors.black),
        ('FONTSIZE', (0, 0), (-1, -1), 10),
    ]))
    story.append(t)
    story.append(Spacer(1, 12))

    # Horizontal rule
    story.append(HRFlowable(width="80%", thickness=1, color=black))
    story.append(Spacer(1, 12))

    # Korean content
    try:
        from reportlab.pdfbase import pdfmetrics
        from reportlab.pdfbase.ttfonts import TTFont
        # Try common Korean font paths
        korean_fonts = [
            '/System/Library/Fonts/AppleSDGothicNeo.ttc',
            '/System/Library/Fonts/Supplemental/AppleGothic.ttf',
            '/Library/Fonts/NanumGothic.ttf',
        ]
        for font_path in korean_fonts:
            if os.path.exists(font_path):
                pdfmetrics.registerFont(TTFont('Korean', font_path))
                break

        kr_style = ParagraphStyle('Korean', parent=body, fontName='Korean', fontSize=11)
        kr_h2 = ParagraphStyle('KoreanH2', parent=h2, fontName='Korean')

        story.append(Paragraph("한국어 콘텐츠 테스트", kr_h2))
        story.append(Paragraph(
            "이것은 한국어 텍스트입니다. MDM 파서의 한국어 처리 능력을 테스트합니다.", kr_style))
        story.append(Spacer(1, 8))

        # Korean table
        kr_table_data = [
            ['형식', '개발사', '특징'],
            ['HWP', '한컴', '한국 정부 표준'],
            ['PDF', 'Adobe', '범용 문서'],
            ['DOCX', 'Microsoft', '국제 표준'],
        ]
        t2 = Table(kr_table_data, colWidths=[80, 80, 140])
        t2.setStyle(TableStyle([
            ('BACKGROUND', (0, 0), (-1, 0), colors.HexColor('#2C3E50')),
            ('TEXTCOLOR', (0, 0), (-1, 0), colors.white),
            ('FONTNAME', (0, 0), (-1, -1), 'Korean'),
            ('GRID', (0, 0), (-1, -1), 1, colors.black),
            ('FONTSIZE', (0, 0), (-1, -1), 10),
        ]))
        story.append(t2)
    except Exception as e:
        story.append(Paragraph(f"(Korean font not available: {e})", body))

    doc.build(story)
    print(f"Created: {path}")
    return path


def create_multicolumn_test():
    """Test 2: Two-column layout document."""
    path = os.path.join(BENCH_DIR, "test_twocolumn.pdf")
    doc = SimpleDocTemplate(path, pagesize=A4,
                           title="Two-Column Layout Test")
    styles = getSampleStyleSheet()
    h1 = ParagraphStyle('H1', parent=styles['Heading1'], fontSize=22)
    h2 = ParagraphStyle('H2', parent=styles['Heading2'], fontSize=16)
    body = ParagraphStyle('Body', parent=styles['Normal'], fontSize=10, leading=14)

    story = []
    story.append(Paragraph("Two-Column Document", h1))
    story.append(Spacer(1, 12))

    # Simulate two-column using a table
    col1_text = (
        "Left column paragraph one. This text should appear in the left column "
        "of the document. Proper reading order detection should read this column "
        "completely before moving to the right column."
    )
    col2_text = (
        "Right column paragraph one. This text should appear in the right column. "
        "If the parser reads left-to-right line by line instead of column by column, "
        "the text will be garbled."
    )
    col1_para2 = "Left column paragraph two continues with more content."
    col2_para2 = "Right column paragraph two has additional information."

    col_table = Table(
        [[Paragraph(col1_text + "<br/><br/>" + col1_para2, body),
          Paragraph(col2_text + "<br/><br/>" + col2_para2, body)]],
        colWidths=[250, 250],
        spaceBefore=10,
    )
    col_table.setStyle(TableStyle([
        ('VALIGN', (0, 0), (-1, -1), 'TOP'),
        ('LEFTPADDING', (0, 0), (-1, -1), 8),
        ('RIGHTPADDING', (0, 0), (-1, -1), 8),
    ]))
    story.append(col_table)
    story.append(Spacer(1, 20))

    # Section after columns
    story.append(Paragraph("After Columns", h2))
    story.append(Paragraph(
        "This section appears after the two-column layout and should be detected "
        "as full-width content.", body))

    doc.build(story)
    print(f"Created: {path}")
    return path


def create_header_footer_test():
    """Test 3: Document with headers, footers, page numbers."""
    from reportlab.platypus import Frame, PageTemplate, BaseDocTemplate
    from reportlab.lib.units import inch
    from functools import partial

    path = os.path.join(BENCH_DIR, "test_headers_footers.pdf")

    def header_footer(canvas, doc, title="Header Footer Test"):
        canvas.saveState()
        # Header
        canvas.setFont('Helvetica', 9)
        canvas.drawString(72, A4[1] - 40, title)
        canvas.drawRightString(A4[0] - 72, A4[1] - 40, "Confidential")
        canvas.line(72, A4[1] - 45, A4[0] - 72, A4[1] - 45)
        # Footer
        canvas.line(72, 50, A4[0] - 72, 50)
        canvas.drawString(72, 38, "MDM Parser Benchmark")
        canvas.drawRightString(A4[0] - 72, 38, f"Page {doc.page}")
        canvas.restoreState()

    doc = BaseDocTemplate(path, pagesize=A4,
                         title="Header Footer Test",
                         author="Seunghan Kim")
    frame = Frame(72, 60, A4[0] - 144, A4[1] - 120, id='normal')
    template = PageTemplate(id='main', frames=frame,
                           onPage=partial(header_footer, title="Header Footer Test"))
    doc.addPageTemplates([template])

    styles = getSampleStyleSheet()
    h1 = ParagraphStyle('H1', parent=styles['Heading1'], fontSize=22)
    h2 = ParagraphStyle('H2', parent=styles['Heading2'], fontSize=16)
    body = ParagraphStyle('Body', parent=styles['Normal'], fontSize=11, leading=16)

    story = []
    story.append(Paragraph("Main Document Title", h1))
    story.append(Paragraph(
        "This document tests header and footer detection. The header contains "
        "'Header Footer Test' and 'Confidential'. The footer contains "
        "'MDM Parser Benchmark' and page numbers. These should be stripped from "
        "the main content.", body))
    story.append(Spacer(1, 12))

    story.append(Paragraph("Section One", h2))
    for i in range(8):
        story.append(Paragraph(
            f"Paragraph {i+1} of section one. This content should be preserved while "
            f"headers and footers are removed from the output.", body))
        story.append(Spacer(1, 6))

    story.append(PageBreak())
    story.append(Paragraph("Section Two (Page 2)", h2))
    for i in range(5):
        story.append(Paragraph(
            f"Paragraph {i+1} of section two on page two. The same header and footer "
            f"should repeat on this page.", body))
        story.append(Spacer(1, 6))

    doc.build(story)
    print(f"Created: {path}")
    return path


def create_heading_hierarchy_test():
    """Test 4: Document with clear heading size hierarchy for heading detection."""
    path = os.path.join(BENCH_DIR, "test_headings.pdf")
    doc = SimpleDocTemplate(path, pagesize=A4,
                           title="Heading Hierarchy Test")
    styles = getSampleStyleSheet()

    h1 = ParagraphStyle('H1', parent=styles['Normal'],
                        fontName='Helvetica-Bold', fontSize=24, spaceAfter=12, spaceBefore=20)
    h2 = ParagraphStyle('H2', parent=styles['Normal'],
                        fontName='Helvetica-Bold', fontSize=18, spaceAfter=10, spaceBefore=16)
    h3 = ParagraphStyle('H3', parent=styles['Normal'],
                        fontName='Helvetica-Bold', fontSize=14, spaceAfter=8, spaceBefore=12)
    h4 = ParagraphStyle('H4', parent=styles['Normal'],
                        fontName='Helvetica-Bold', fontSize=12, spaceAfter=6, spaceBefore=10)
    body = ParagraphStyle('Body', parent=styles['Normal'], fontSize=11, leading=16)

    story = []
    story.append(Paragraph("Main Title (Should be H1)", h1))
    story.append(Paragraph("Introduction paragraph under the main title.", body))

    story.append(Paragraph("Chapter One (Should be H2)", h2))
    story.append(Paragraph("Content of chapter one.", body))

    story.append(Paragraph("Section 1.1 (Should be H3)", h3))
    story.append(Paragraph("Content of section 1.1.", body))

    story.append(Paragraph("Detail 1.1.1 (Should be H4)", h4))
    story.append(Paragraph("Content of detail 1.1.1.", body))

    story.append(Paragraph("Section 1.2 (Should be H3)", h3))
    story.append(Paragraph("Content of section 1.2.", body))

    story.append(Paragraph("Chapter Two (Should be H2)", h2))
    story.append(Paragraph("Content of chapter two.", body))

    story.append(Paragraph("Section 2.1 (Should be H3)", h3))
    story.append(Paragraph("Content of section 2.1.", body))

    doc.build(story)
    print(f"Created: {path}")
    return path


if __name__ == "__main__":
    create_comprehensive_test()
    create_multicolumn_test()
    create_header_footer_test()
    create_heading_hierarchy_test()
    print("\nAll test PDF files created!")
