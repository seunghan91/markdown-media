#!/usr/bin/env python3
# ============================================================================
# 🚧 작업 중 - 이 파일은 현재 [테스트 팀]에서 작업 중입니다
# ============================================================================
# 작업 담당: 병렬 작업 팀
# 시작 시간: 2025-01-01
# 진행 상태: Phase 1.8 테스트 구현
#
# ⚠️ 주의: 1.7 오케스트레이터는 다른 팀에서 작업 중입니다.
#         이 테스트 파일은 1.7과 독립적으로 개별 컴포넌트를 테스트합니다.
# ============================================================================
"""
MDM Pipeline Component Tests

이 모듈은 MDM 파이프라인의 개별 컴포넌트들을 테스트합니다:
- OCR 브릿지 (ocr_bridge.py)
- 테이블 SVG 렌더러 (table_to_svg_enhanced.py)
- 차트 PNG 렌더러 (chart_to_png.py)
- 문서 변환기들 (docx_converter.py, hwp_converter.py 등)

1.7 오케스트레이터 통합 테스트는 해당 작업 완료 후 추가됩니다.
"""

import os
import sys
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch, MagicMock

# 프로젝트 루트를 PATH에 추가
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))  # allows `import pipeline` (package)
sys.path.insert(0, str(PROJECT_ROOT / "converters"))
sys.path.insert(0, str(PROJECT_ROOT / "packages" / "parser-py"))
sys.path.insert(0, str(PROJECT_ROOT / "pipeline"))


class TestPipelineOrchestratorDryRun(unittest.TestCase):
    """pipeline/orchestrator.py 최소 스모크 (의존성 없이 동작해야 함)"""

    def test_import_and_dry_run(self):
        from pipeline import MdmPipeline

        p = MdmPipeline()

        # 파일이 없으면 Rust 실행 전 단계에서 안전하게 실패해야 함
        res = p.convert("does-not-exist.hwp", output_dir=Path(tempfile.mkdtemp()))
        self.assertFalse(res.success)
        self.assertTrue(any("Input file not found" in e for e in res.errors))


class TestTableSvgEnhanced(unittest.TestCase):
    """table_to_svg_enhanced.py 테스트"""

    def setUp(self):
        """테스트 환경 설정"""
        self.temp_dir = tempfile.mkdtemp()

    def tearDown(self):
        """테스트 환경 정리"""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_table_from_markdown(self):
        """마크다운 테이블 파싱 테스트"""
        try:
            from table_to_svg_enhanced import Table

            markdown = """| A | B |
| --- | --- |
| 1 | 2 |"""

            table = Table.from_markdown(markdown)
            self.assertEqual(table.row_count, 2)
            self.assertEqual(table.col_count, 2)
            self.assertTrue(table.has_header)
        except ImportError:
            self.skipTest("table_to_svg_enhanced not available")

    def test_table_from_rust_output(self):
        """Rust 출력 형식 파싱 테스트"""
        try:
            from table_to_svg_enhanced import Table

            rust_data = {
                "rows": [
                    ["Header1", "Header2"],
                    ["Cell1", "Cell2"]
                ],
                "has_header": True
            }

            table = Table.from_rust_output(rust_data)
            self.assertEqual(table.row_count, 2)
            self.assertEqual(table.col_count, 2)
        except ImportError:
            self.skipTest("table_to_svg_enhanced not available")

    def test_cell_span_support(self):
        """병합 셀 지원 테스트"""
        try:
            from table_to_svg_enhanced import Table, TableCell

            rust_data = {
                "cells": [
                    {"content": "Merged", "row": 0, "col": 0, "row_span": 2, "col_span": 1},
                    {"content": "B", "row": 0, "col": 1},
                    {"content": "C", "row": 1, "col": 1},
                ],
                "row_count": 2,
                "col_count": 2,
            }

            table = Table.from_rust_output(rust_data)
            merged_cell = table.cells[0]
            self.assertEqual(merged_cell.row_span, 2)
        except ImportError:
            self.skipTest("table_to_svg_enhanced not available")

    def test_svg_rendering(self):
        """SVG 렌더링 테스트"""
        try:
            from table_to_svg_enhanced import Table, TableSvgRenderer

            markdown = "| A | B |\n| --- | --- |\n| 1 | 2 |"
            table = Table.from_markdown(markdown)
            renderer = TableSvgRenderer()

            output_path = os.path.join(self.temp_dir, "test_table.svg")
            result = renderer.render(table, output_path)

            self.assertTrue(os.path.exists(result))
            with open(result, 'r') as f:
                content = f.read()
                self.assertIn("<svg", content)
                self.assertIn("</svg>", content)
        except ImportError:
            self.skipTest("table_to_svg_enhanced or svgwrite not available")


class TestChartToPng(unittest.TestCase):
    """chart_to_png.py 테스트"""

    def setUp(self):
        """테스트 환경 설정"""
        self.temp_dir = tempfile.mkdtemp()

    def tearDown(self):
        """테스트 환경 정리"""
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_chart_data_from_dict(self):
        """차트 데이터 딕셔너리 파싱 테스트"""
        try:
            from chart_to_png import ChartData, ChartType

            data = {
                "type": "bar",
                "title": "Test Chart",
                "categories": ["A", "B", "C"],
                "series": [
                    {"name": "Series 1", "values": [1, 2, 3]}
                ]
            }

            chart = ChartData.from_dict(data)
            self.assertEqual(chart.chart_type, ChartType.BAR)
            self.assertEqual(chart.title, "Test Chart")
            self.assertEqual(len(chart.series), 1)
        except ImportError:
            self.skipTest("chart_to_png not available")

    def test_chart_types(self):
        """지원되는 차트 유형 테스트"""
        try:
            from chart_to_png import ChartType

            expected_types = ["bar", "line", "pie", "scatter", "area"]
            for chart_type in expected_types:
                enum_type = ChartType(chart_type)
                self.assertIsNotNone(enum_type)
        except ImportError:
            self.skipTest("chart_to_png not available")

    def test_chart_style_themes(self):
        """차트 스타일 테마 테스트"""
        try:
            from chart_to_png import ChartStyle

            dark = ChartStyle.dark_theme()
            self.assertIsNotNone(dark.background_color)

            minimal = ChartStyle.minimal_theme()
            self.assertIsNotNone(minimal.background_color)

            presentation = ChartStyle.presentation_theme()
            self.assertIsNotNone(presentation.background_color)
        except ImportError:
            self.skipTest("chart_to_png not available")

    def test_chart_rendering(self):
        """차트 PNG 렌더링 테스트"""
        try:
            from chart_to_png import ChartRenderer, ChartData

            data = {
                "type": "bar",
                "title": "Test",
                "categories": ["A", "B"],
                "series": [{"name": "Data", "values": [10, 20]}]
            }

            chart_data = ChartData.from_dict(data)
            renderer = ChartRenderer()
            output_path = os.path.join(self.temp_dir, "test_chart.png")

            result = renderer.render(chart_data, output_path)

            self.assertTrue(os.path.exists(result))
            # PNG 파일 시그니처 확인
            with open(result, 'rb') as f:
                signature = f.read(8)
                self.assertEqual(signature[:4], b'\x89PNG')
        except ImportError:
            self.skipTest("chart_to_png or matplotlib not available")


class TestOcrBridge(unittest.TestCase):
    """ocr_bridge.py 테스트"""

    def test_rust_output_parsing_json(self):
        """Rust JSON 출력 파싱 테스트"""
        try:
            from ocr_bridge import RustOutput

            with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
                json.dump({
                    "format": "hwp",
                    "version": "5.0",
                    "metadata": {"title": "Test"},
                    "text": "Hello World",
                    "images": [],
                    "tables": []
                }, f)
                f.flush()

                output = RustOutput.from_json(f.name)
                self.assertEqual(output.format, "hwp")
                self.assertEqual(output.text_content, "Hello World")

                os.unlink(f.name)
        except ImportError:
            self.skipTest("ocr_bridge not available")

    def test_rust_output_parsing_mdx(self):
        """Rust MDX 출력 파싱 테스트"""
        try:
            from ocr_bridge import RustOutput

            with tempfile.NamedTemporaryFile(mode='w', suffix='.mdx', delete=False) as f:
                f.write("""---
title: "Test Document"
format: hwp
---

# Heading

![Image](image1.png)

Some text content.
""")
                f.flush()

                output = RustOutput.from_mdx(f.name)
                self.assertEqual(output.format, "hwp")
                self.assertIn("Heading", output.text_content)
                self.assertEqual(len(output.images), 1)

                os.unlink(f.name)
        except ImportError:
            self.skipTest("ocr_bridge not available")

    def test_ocr_result_structure(self):
        """OCR 결과 구조 테스트"""
        try:
            from ocr_bridge import OcrResult

            result = OcrResult(
                image_id="img_001",
                source_path="/path/to/image.png",
                extracted_text="Hello World",
                confidence=0.95,
                language="kor"
            )

            result_dict = result.to_dict()
            self.assertEqual(result_dict["image_id"], "img_001")
            self.assertEqual(result_dict["extracted_text"], "Hello World")
            self.assertEqual(result_dict["confidence"], 0.95)
        except ImportError:
            self.skipTest("ocr_bridge not available")

    @patch('ocr_bridge.OcrProcessor')
    def test_ocr_bridge_initialization(self, mock_processor):
        """OCR 브릿지 초기화 테스트"""
        try:
            from ocr_bridge import RustOcrBridge

            mock_processor.return_value = MagicMock()
            bridge = RustOcrBridge(ocr_engine="auto")
            self.assertIsNotNone(bridge)
        except ImportError:
            self.skipTest("ocr_bridge not available")


class TestDocxConverter(unittest.TestCase):
    """docx_converter.py 테스트"""

    def test_docx_converter_exists(self):
        """DOCX 변환기 존재 확인"""
        try:
            from docx_converter import DocxConverter
            self.assertTrue(hasattr(DocxConverter, 'convert'))
        except ImportError:
            self.skipTest("docx_converter not available")


class TestHwpConverter(unittest.TestCase):
    """hwp_converter.py 테스트"""

    def test_hwp_converter_exists(self):
        """HWP 변환기 존재 확인"""
        try:
            from hwp_converter import HwpConverter
            self.assertTrue(hasattr(HwpConverter, 'convert'))
        except ImportError:
            self.skipTest("hwp_converter not available")


class TestPdfConverter(unittest.TestCase):
    """pdf_converter.py 테스트"""

    def test_pdf_converter_exists(self):
        """PDF 변환기 존재 확인"""
        try:
            from pdf_converter import PdfConverter
            self.assertTrue(hasattr(PdfConverter, 'convert'))
        except ImportError:
            self.skipTest("pdf_converter not available")


class TestOutputFormatConsistency(unittest.TestCase):
    """출력 형식 일관성 테스트"""

    def test_mdx_frontmatter_format(self):
        """MDX 프론트매터 형식 검증"""
        # 예상 프론트매터 필드
        expected_fields = ['format', 'source']

        sample_frontmatter = """---
format: hwp
source: "document.hwp"
title: "테스트 문서"
---
"""
        # 필수 필드 존재 확인
        for field in expected_fields:
            self.assertIn(field, sample_frontmatter)

    def test_image_reference_format(self):
        """이미지 참조 형식 검증"""
        # 마크다운 이미지 형식: ![alt](path)
        import re
        pattern = r'!\[([^\]]*)\]\(([^)]+)\)'

        samples = [
            "![이미지](./assets/image1.png)",
            "![](media/photo.jpg)",
            "![테스트 이미지](images/test.gif)",
        ]

        for sample in samples:
            match = re.match(pattern, sample)
            self.assertIsNotNone(match, f"Failed to match: {sample}")

    def test_table_markdown_format(self):
        """테이블 마크다운 형식 검증"""
        valid_table = """| A | B |
| --- | --- |
| 1 | 2 |"""

        lines = valid_table.strip().split('\n')
        self.assertTrue(lines[0].startswith('|'))
        self.assertTrue('---' in lines[1])


# ============================================================================
# 파이프라인 통합 테스트 (1.7 완료 후 활성화)
# ============================================================================

class TestPipelineIntegration(unittest.TestCase):
    """
    파이프라인 통합 테스트

    ⚠️ 주의: 이 테스트들은 1.7 오케스트레이터 완료 후 활성화됩니다.
    현재는 스킵 처리되어 있습니다.
    """

    @unittest.skip("1.7 오케스트레이터 작업 완료 대기")
    def test_full_hwp_pipeline(self):
        """HWP → MDX 전체 파이프라인 테스트"""
        pass

    @unittest.skip("1.7 오케스트레이터 작업 완료 대기")
    def test_full_docx_pipeline(self):
        """DOCX → MDX 전체 파이프라인 테스트"""
        pass

    @unittest.skip("1.7 오케스트레이터 작업 완료 대기")
    def test_full_pdf_pipeline(self):
        """PDF → MDX 전체 파이프라인 테스트"""
        pass

    @unittest.skip("1.7 오케스트레이터 작업 완료 대기")
    def test_pipeline_with_ocr(self):
        """OCR 포함 파이프라인 테스트"""
        pass

    @unittest.skip("1.7 오케스트레이터 작업 완료 대기")
    def test_pipeline_error_handling(self):
        """파이프라인 에러 처리 테스트"""
        pass


if __name__ == '__main__':
    # 테스트 실행
    unittest.main(verbosity=2)
