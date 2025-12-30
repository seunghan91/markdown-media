"""
MDM Pipeline Module
====================
E2E 문서 변환 파이프라인.

사용법:
    from pipeline import MdmPipeline, ConversionOptions
    
    pipeline = MdmPipeline()
    result = pipeline.convert("document.hwp", output_dir="./output")
"""

from .orchestrator import (
    MdmPipeline,
    ConversionOptions,
    ConversionResult,
    OutputFormat,
    DocumentType,
)

__all__ = [
    "MdmPipeline",
    "ConversionOptions", 
    "ConversionResult",
    "OutputFormat",
    "DocumentType",
]

__version__ = "0.1.0"
