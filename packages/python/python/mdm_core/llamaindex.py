"""LlamaIndex Reader for MDM Core.

Usage:
    from mdm_core.llamaindex import MDMReader
    reader = MDMReader()
    docs = reader.load_data("contract.hwp")
"""
from __future__ import annotations
from pathlib import Path
from typing import List, Optional, Union

try:
    from llama_index.core.readers.base import BaseReader
    from llama_index.core.schema import Document
except ImportError:
    raise ImportError("Install: pip install mdm-core[llamaindex]")

from mdm_core._mdm_native import convert_file

SUPPORTED = {".hwp", ".hwpx", ".pdf", ".docx"}

class MDMReader(BaseReader):
    """Read HWP/HWPX/PDF/DOCX via MDM Rust engine. 10-100x faster than Python."""

    def load_data(self, file: Union[str, Path, List], extra_info: Optional[dict] = None) -> List[Document]:
        files = [Path(file)] if isinstance(file, (str, Path)) else [Path(f) for f in file]
        docs = []
        for f in files:
            if f.is_dir():
                for ext in SUPPORTED:
                    for c in sorted(f.glob(f"**/*{ext}")):
                        d = self._load(c, extra_info)
                        if d: docs.append(d)
            elif f.is_file():
                d = self._load(f, extra_info)
                if d: docs.append(d)
        return docs

    def _load(self, path: Path, extra_info=None) -> Optional[Document]:
        if path.suffix.lower() not in SUPPORTED:
            return None
        try:
            md = convert_file(str(path))
        except Exception:
            return None
        meta = {"file_path": str(path), "file_name": path.name, "file_type": path.suffix.lstrip(".")}
        if extra_info: meta.update(extra_info)
        return Document(text=md, metadata=meta)
