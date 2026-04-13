"""LangChain Document Loader for MDM Core.

Usage:
    from mdm_core.langchain import MDMLoader
    loader = MDMLoader("contract.hwp")
    docs = loader.load()
"""
from __future__ import annotations
import json
from pathlib import Path
from typing import Iterator, Optional, Union

try:
    from langchain_core.document_loaders import BaseLoader
    from langchain_core.documents import Document
except ImportError:
    raise ImportError("Install: pip install mdm-core[langchain]")

from mdm_core._mdm_native import convert_file, convert_file_to_json

SUPPORTED = {".hwp", ".hwpx", ".pdf", ".docx"}

class MDMLoader(BaseLoader):
    """Load HWP/HWPX/PDF/DOCX via MDM Rust engine. 10-100x faster than Python."""

    def __init__(self, file_path: Union[str, Path], extract_metadata: bool = True):
        self.file_path = Path(file_path)
        self.extract_metadata = extract_metadata

    def lazy_load(self) -> Iterator[Document]:
        if self.file_path.is_file():
            yield from self._load_file(self.file_path)
        elif self.file_path.is_dir():
            for ext in SUPPORTED:
                for p in sorted(self.file_path.glob(f"**/*{ext}")):
                    yield from self._load_file(p)

    def _load_file(self, path: Path) -> Iterator[Document]:
        if path.suffix.lower() not in SUPPORTED:
            return
        try:
            md = convert_file(str(path))
        except Exception as e:
            import warnings; warnings.warn(f"MDMLoader: {path}: {e}")
            return
        meta = {"source": str(path), "format": path.suffix.lstrip("."), "filename": path.name}
        if self.extract_metadata:
            try:
                data = json.loads(convert_file_to_json(str(path)))
                if isinstance(data.get("metadata"), dict):
                    meta.update(data["metadata"])
            except Exception:
                pass
        yield Document(page_content=md, metadata=meta)
