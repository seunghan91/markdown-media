"""
MDM Python Parser Package
Provides OCR processing, PDF handling, and document conversion utilities.
"""
__version__ = '0.1.0'

try:
    from .ocr_processor import OcrProcessor
    from .ocr_bridge import RustOcrBridge, OcrResult, RustOutput, OpenRouterOcrEngine
    from .pdf_processor import PdfProcessor
    from .hwp_to_svg import HwpToSvgConverter

    __all__ = [
        'OcrProcessor',
        'RustOcrBridge',
        'OcrResult',
        'RustOutput',
        'OpenRouterOcrEngine',
        'PdfProcessor',
        'HwpToSvgConverter',
    ]
except ImportError:
    # 패키지 외부에서 단독 임포트 시 (예: pytest 환경) 무시
    __all__ = []
