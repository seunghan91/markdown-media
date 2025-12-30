"""
MDM E2E Pipeline Orchestrator
=============================
작업 담당: E팀
작업 상태: 진행 중
시작 시간: 2025-12-31

전체 변환 파이프라인을 조율하는 오케스트레이터:
1. Rust 파서로 문서 파싱
2. OCR 처리 (필요시)
3. 테이블 → SVG 변환
4. 차트 → PNG 변환
5. MDX/JSON 생성

사용법:
    from pipeline.orchestrator import MdmPipeline
    
    pipeline = MdmPipeline()
    result = pipeline.convert("document.hwp", output_dir="./output")
"""

import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Union


class OutputFormat(Enum):
    """출력 포맷."""
    MDX = "mdx"
    JSON = "json"
    HTML = "html"


class DocumentType(Enum):
    """지원하는 문서 타입."""
    HWP = "hwp"
    HWPX = "hwpx"
    PDF = "pdf"
    DOCX = "docx"


@dataclass
class ConversionOptions:
    """변환 옵션 설정."""
    output_format: OutputFormat = OutputFormat.MDX
    extract_images: bool = True
    convert_tables_to_svg: bool = True
    convert_charts_to_png: bool = True
    enable_ocr: bool = False
    image_quality: int = 85
    svg_theme: str = "default"  # default, dark, minimal
    chart_theme: str = "default"  # default, dark, minimal, presentation
    verbose: bool = False


@dataclass
class ConversionResult:
    """변환 결과."""
    success: bool
    output_path: Optional[str] = None
    mdx_path: Optional[str] = None
    json_path: Optional[str] = None
    assets_dir: Optional[str] = None
    tables: List[str] = field(default_factory=list)
    charts: List[str] = field(default_factory=list)
    images: List[str] = field(default_factory=list)
    errors: List[str] = field(default_factory=list)
    warnings: List[str] = field(default_factory=list)
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """결과를 딕셔너리로 변환."""
        return {
            "success": self.success,
            "output_path": self.output_path,
            "mdx_path": self.mdx_path,
            "json_path": self.json_path,
            "assets_dir": self.assets_dir,
            "tables": self.tables,
            "charts": self.charts,
            "images": self.images,
            "errors": self.errors,
            "warnings": self.warnings,
            "metadata": self.metadata,
        }


class MdmPipeline:
    """
    MDM 문서 변환 파이프라인.
    
    Rust 파서와 Python 변환기들을 조율하여 문서를 MDX/JSON으로 변환합니다.
    """
    
    # Rust CLI 바이너리 경로 (빌드된 위치)
    RUST_CLI_PATHS = [
        Path(__file__).parent.parent / "core" / "target" / "release" / "hwp2mdm",
        Path(__file__).parent.parent / "core" / "target" / "debug" / "hwp2mdm",
        Path("/usr/local/bin/hwp2mdm"),
        Path("hwp2mdm"),  # PATH에 있는 경우
    ]
    
    def __init__(self, rust_cli_path: Optional[str] = None):
        """
        파이프라인 초기화.
        
        Args:
            rust_cli_path: Rust CLI 바이너리 경로 (None이면 자동 탐색)
        """
        self.rust_cli = self._find_rust_cli(rust_cli_path)
        self.converters_dir = Path(__file__).parent.parent / "converters"
        
    def _find_rust_cli(self, custom_path: Optional[str] = None) -> Optional[Path]:
        """Rust CLI 바이너리를 찾습니다."""
        if custom_path:
            path = Path(custom_path)
            if path.exists() and path.is_file():
                return path
        
        for path in self.RUST_CLI_PATHS:
            if path.exists() and path.is_file():
                return path
        
        # which/where로 찾기
        try:
            result = subprocess.run(
                ["which", "hwp2mdm"],
                capture_output=True,
                text=True,
            )
            if result.returncode == 0:
                return Path(result.stdout.strip())
        except Exception:
            pass
        
        return None
    
    def convert(
        self,
        input_path: Union[str, Path],
        output_dir: Union[str, Path] = "./output",
        options: Optional[ConversionOptions] = None,
    ) -> ConversionResult:
        """
        문서를 변환합니다.
        
        Args:
            input_path: 입력 파일 경로 (HWP, HWPX, PDF, DOCX)
            output_dir: 출력 디렉토리
            options: 변환 옵션
            
        Returns:
            ConversionResult: 변환 결과
        """
        options = options or ConversionOptions()
        input_path = Path(input_path)
        output_dir = Path(output_dir)
        
        result = ConversionResult(success=False)
        result.output_path = str(output_dir)
        
        # 입력 파일 검증
        if not input_path.exists():
            result.errors.append(f"Input file not found: {input_path}")
            return result
        
        # 문서 타입 확인
        doc_type = self._detect_document_type(input_path)
        if not doc_type:
            result.errors.append(f"Unsupported file format: {input_path.suffix}")
            return result
        
        result.metadata["document_type"] = doc_type.value
        result.metadata["input_file"] = str(input_path)
        
        # 출력 디렉토리 생성
        output_dir.mkdir(parents=True, exist_ok=True)
        assets_dir = output_dir / "assets"
        assets_dir.mkdir(exist_ok=True)
        result.assets_dir = str(assets_dir)
        
        try:
            # Step 1: Rust 파서로 기본 변환
            if options.verbose:
                print(f"📄 Step 1: Parsing {input_path.name} with Rust parser...")
            
            rust_result = self._run_rust_parser(input_path, output_dir, options)
            if not rust_result["success"]:
                result.errors.extend(rust_result.get("errors", []))
                return result
            
            result.metadata.update(rust_result.get("metadata", {}))
            
            # Step 2: 테이블 → SVG 변환
            if options.convert_tables_to_svg and rust_result.get("tables"):
                if options.verbose:
                    print(f"📊 Step 2: Converting {len(rust_result['tables'])} tables to SVG...")
                
                table_results = self._convert_tables_to_svg(
                    rust_result["tables"],
                    assets_dir,
                    options.svg_theme,
                )
                result.tables = table_results
            
            # Step 3: 차트 → PNG 변환
            if options.convert_charts_to_png and rust_result.get("charts"):
                if options.verbose:
                    print(f"📈 Step 3: Converting {len(rust_result['charts'])} charts to PNG...")
                
                chart_results = self._convert_charts_to_png(
                    rust_result["charts"],
                    assets_dir,
                    options.chart_theme,
                )
                result.charts = chart_results
            
            # Step 4: OCR 처리 (필요시)
            if options.enable_ocr and rust_result.get("images_for_ocr"):
                if options.verbose:
                    print(f"🔍 Step 4: Running OCR on images...")
                
                ocr_results = self._run_ocr(rust_result["images_for_ocr"], options)
                result.metadata["ocr_results"] = ocr_results
            
            # Step 5: 이미지 목록 수집
            result.images = self._collect_images(assets_dir)
            
            # Step 6: 최종 출력 파일 경로 설정
            stem = input_path.stem
            if options.output_format == OutputFormat.MDX:
                result.mdx_path = str(output_dir / f"{stem}.mdx")
            elif options.output_format == OutputFormat.JSON:
                result.json_path = str(output_dir / f"{stem}.json")
            
            result.success = True
            
            if options.verbose:
                print(f"✅ Conversion complete!")
                print(f"   Output: {output_dir}")
                print(f"   Images: {len(result.images)}")
                print(f"   Tables: {len(result.tables)}")
                print(f"   Charts: {len(result.charts)}")
            
        except Exception as e:
            result.errors.append(f"Pipeline error: {str(e)}")
        
        return result
    
    def _detect_document_type(self, path: Path) -> Optional[DocumentType]:
        """문서 타입을 감지합니다."""
        ext = path.suffix.lower()
        type_map = {
            ".hwp": DocumentType.HWP,
            ".hwpx": DocumentType.HWPX,
            ".pdf": DocumentType.PDF,
            ".docx": DocumentType.DOCX,
        }
        return type_map.get(ext)
    
    def _run_rust_parser(
        self,
        input_path: Path,
        output_dir: Path,
        options: ConversionOptions,
    ) -> Dict[str, Any]:
        """Rust 파서를 실행합니다."""
        result = {
            "success": False,
            "tables": [],
            "charts": [],
            "images_for_ocr": [],
            "metadata": {},
            "errors": [],
        }
        
        if not self.rust_cli:
            result["errors"].append("Rust CLI not found. Please build the core package first.")
            return result
        
        try:
            # Rust CLI 실행
            cmd = [
                str(self.rust_cli),
                "convert",
                str(input_path),
                "-o", str(output_dir),
                "-f", "json" if options.output_format == OutputFormat.JSON else "mdx",
            ]
            
            if options.extract_images:
                cmd.append("--extract-images")
            
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                timeout=300,  # 5분 타임아웃
            )
            
            if proc.returncode != 0:
                result["errors"].append(f"Rust parser failed: {proc.stderr}")
                return result
            
            # JSON 출력 파싱 (있는 경우)
            json_path = output_dir / f"{input_path.stem}.json"
            if json_path.exists():
                with open(json_path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                    result["tables"] = data.get("tables", [])
                    result["charts"] = data.get("charts", [])
                    result["metadata"] = data.get("metadata", {})
            
            result["success"] = True
            
        except subprocess.TimeoutExpired:
            result["errors"].append("Rust parser timed out")
        except Exception as e:
            result["errors"].append(f"Rust parser error: {str(e)}")
        
        return result
    
    def _convert_tables_to_svg(
        self,
        tables: List[Dict[str, Any]],
        output_dir: Path,
        theme: str,
    ) -> List[str]:
        """테이블을 SVG로 변환합니다."""
        converted = []
        
        try:
            # table_to_svg_enhanced 모듈 동적 임포트
            import sys
            sys.path.insert(0, str(self.converters_dir))
            from table_to_svg_enhanced import TableSvgRenderer, Table, TableStyle, CellStyle
            
            # 테마 설정
            if theme == "dark":
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
            elif theme == "minimal":
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
            
            renderer = TableSvgRenderer(style)
            
            for i, table_data in enumerate(tables):
                try:
                    table = Table.from_rust_output(table_data)
                    output_path = output_dir / f"table_{i+1}.svg"
                    renderer.render(table, str(output_path))
                    converted.append(str(output_path))
                except Exception as e:
                    print(f"Warning: Failed to convert table {i+1}: {e}")
            
        except ImportError as e:
            print(f"Warning: table_to_svg_enhanced not available: {e}")
        
        return converted
    
    def _convert_charts_to_png(
        self,
        charts: List[Dict[str, Any]],
        output_dir: Path,
        theme: str,
    ) -> List[str]:
        """차트를 PNG로 변환합니다."""
        converted = []
        
        try:
            import sys
            sys.path.insert(0, str(self.converters_dir))
            from chart_to_png import ChartRenderer, ChartStyle
            
            # 테마 설정
            if theme == "dark":
                style = ChartStyle.dark_theme()
            elif theme == "minimal":
                style = ChartStyle.minimal_theme()
            elif theme == "presentation":
                style = ChartStyle.presentation_theme()
            else:
                style = ChartStyle()
            
            renderer = ChartRenderer(style)
            
            for i, chart_data in enumerate(charts):
                try:
                    output_path = output_dir / f"chart_{i+1}.png"
                    renderer.render(chart_data, str(output_path))
                    converted.append(str(output_path))
                except Exception as e:
                    print(f"Warning: Failed to convert chart {i+1}: {e}")
            
        except ImportError as e:
            print(f"Warning: chart_to_png not available: {e}")
        
        return converted
    
    def _run_ocr(
        self,
        images: List[str],
        options: ConversionOptions,
    ) -> Dict[str, str]:
        """이미지에 OCR을 실행합니다."""
        results = {}
        
        try:
            # OCR 브릿지 임포트 시도
            from packages.parser_py.ocr_bridge import RustOcrBridge
            
            bridge = RustOcrBridge()
            for img_path in images:
                try:
                    text = bridge.process_image(img_path)
                    results[img_path] = text
                except Exception as e:
                    results[img_path] = f"OCR failed: {e}"
                    
        except ImportError:
            # OCR 미설치 시 스킵
            for img_path in images:
                results[img_path] = "OCR not available"
        
        return results
    
    def _collect_images(self, assets_dir: Path) -> List[str]:
        """assets 디렉토리의 이미지 목록을 수집합니다."""
        images = []
        image_extensions = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg", ".bmp"}
        
        if assets_dir.exists():
            for f in assets_dir.iterdir():
                if f.suffix.lower() in image_extensions:
                    images.append(str(f))
        
        return sorted(images)
    
    def batch_convert(
        self,
        input_pattern: str,
        output_dir: Union[str, Path] = "./output",
        options: Optional[ConversionOptions] = None,
    ) -> List[ConversionResult]:
        """
        여러 파일을 일괄 변환합니다.
        
        Args:
            input_pattern: glob 패턴 (예: "*.hwp", "docs/**/*.hwp")
            output_dir: 출력 디렉토리
            options: 변환 옵션
            
        Returns:
            List[ConversionResult]: 변환 결과 목록
        """
        import glob
        
        results = []
        output_dir = Path(output_dir)
        
        files = glob.glob(input_pattern, recursive=True)
        
        for file_path in files:
            file_path = Path(file_path)
            file_output_dir = output_dir / file_path.stem
            
            print(f"Converting: {file_path}")
            result = self.convert(file_path, file_output_dir, options)
            results.append(result)
        
        return results


# CLI 인터페이스
def main():
    """CLI 엔트리 포인트."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="MDM E2E Pipeline Orchestrator",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s document.hwp -o ./output
  %(prog)s document.hwp -o ./output --format json
  %(prog)s "docs/*.hwp" -o ./converted --batch
  %(prog)s document.hwp -o ./output --ocr --svg-theme dark
        """,
    )
    
    parser.add_argument("input", help="Input file or glob pattern")
    parser.add_argument("-o", "--output", default="./output", help="Output directory")
    parser.add_argument("-f", "--format", choices=["mdx", "json", "html"],
                       default="mdx", help="Output format")
    parser.add_argument("--batch", action="store_true", help="Batch convert mode")
    parser.add_argument("--ocr", action="store_true", help="Enable OCR")
    parser.add_argument("--no-tables", action="store_true", help="Skip table conversion")
    parser.add_argument("--no-charts", action="store_true", help="Skip chart conversion")
    parser.add_argument("--svg-theme", choices=["default", "dark", "minimal"],
                       default="default", help="SVG theme for tables")
    parser.add_argument("--chart-theme", choices=["default", "dark", "minimal", "presentation"],
                       default="default", help="Chart theme")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    
    args = parser.parse_args()
    
    # 옵션 설정
    options = ConversionOptions(
        output_format=OutputFormat(args.format),
        convert_tables_to_svg=not args.no_tables,
        convert_charts_to_png=not args.no_charts,
        enable_ocr=args.ocr,
        svg_theme=args.svg_theme,
        chart_theme=args.chart_theme,
        verbose=args.verbose,
    )
    
    pipeline = MdmPipeline()
    
    if args.batch:
        results = pipeline.batch_convert(args.input, args.output, options)
        success_count = sum(1 for r in results if r.success)
        print(f"\n📊 Batch complete: {success_count}/{len(results)} succeeded")
    else:
        result = pipeline.convert(args.input, args.output, options)
        if result.success:
            print(f"\n✅ Conversion successful!")
            print(f"   Output: {result.output_path}")
        else:
            print(f"\n❌ Conversion failed:")
            for error in result.errors:
                print(f"   - {error}")


if __name__ == "__main__":
    main()
