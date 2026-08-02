"""MDM adapter: invokes the hwp2mdm binary and returns the .mdx output."""
from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path
from dataclasses import dataclass


@dataclass
class AdapterResult:
    markdown: str
    tables_json: str | None  # optional table structure (for TEDS)
    elapsed_ms: float
    exit_code: int
    stderr: str


def convert(pdf_path: Path, binary: str) -> AdapterResult:
    """Contract shared by all adapters:
        input  = absolute PDF path + adapter-specific config
        output = AdapterResult
    """
    import time
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp)
        t0 = time.perf_counter()
        proc = subprocess.run(
            [binary, str(pdf_path), "-o", str(out_dir)],
            capture_output=True,
            text=True,
            timeout=300,
        )
        elapsed = (time.perf_counter() - t0) * 1000

        stem = pdf_path.stem
        mdx_path = out_dir / f"{stem}.mdx"
        md = mdx_path.read_text(encoding="utf-8") if mdx_path.exists() else ""

        tables_path = out_dir / f"{stem}.tables.json"
        tables = tables_path.read_text(encoding="utf-8") if tables_path.exists() else None

        return AdapterResult(
            markdown=md,
            tables_json=tables,
            elapsed_ms=elapsed,
            exit_code=proc.returncode,
            stderr=proc.stderr,
        )
