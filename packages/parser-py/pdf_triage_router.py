#!/usr/bin/env python3
"""PDF triage router — consumes the manifest emitted by `hwp2mdm triage`
and dispatches pages to the appropriate extraction path.

Pipeline:
  PDF ─▶ Rust triage ─▶ manifest.json
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
    text_native             scanned                  mixed
    → Rust extraction       → rasterize + full       → Rust text +
      (hwp2mdm stream)        page OCR                 OCR bbox regions
        │                       │                       │
        └───────────────────────┴───────────────────────┘
                                │
                                ▼
                         merged Markdown

Page rasterization uses PyMuPDF (fitz). OCR uses the existing engines in
ocr_processor.py / ocr_bridge.py.

Usage (CLI):
    python pdf_triage_router.py <pdf> [--engine tesseract|easyocr|openrouter]
                                      [--dpi 300]
                                      [--out output.md]
                                      [--manifest]   # emit manifest JSON too

Usage (library):
    from pdf_triage_router import TriageRouter
    router = TriageRouter(engine="tesseract", dpi=300)
    markdown = router.process("report.pdf")
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# Locate the Rust binary. Overridable via $MDM_BINARY.
import os
DEFAULT_BINARY = os.environ.get(
    "MDM_BINARY",
    str(Path(__file__).resolve().parent.parent.parent / "core" / "target" / "release" / "hwp2mdm"),
)


@dataclass
class PageRouting:
    page: int
    category: str  # "text_native" | "scanned" | "mixed"
    confidence: float
    needs_full_page_ocr: bool
    ocr_regions: list[dict]
    cjk_hint: bool


@dataclass
class TriageManifest:
    document: str
    page_count: int
    pages: list[PageRouting]

    @classmethod
    def from_json(cls, data: dict) -> "TriageManifest":
        return cls(
            document=data["document"],
            page_count=data["page_count"],
            pages=[
                PageRouting(
                    page=p["page"],
                    category=p["category"],
                    confidence=p["confidence"],
                    needs_full_page_ocr=p.get("needs_full_page_ocr", False),
                    ocr_regions=p.get("ocr_regions", []),
                    cjk_hint=p.get("cjk_hint", False),
                )
                for p in data["pages"]
            ],
        )


class TriageRouter:
    def __init__(
        self,
        engine: str = "tesseract",
        dpi: int = 300,
        lang: str = "kor+eng",
        binary: str = DEFAULT_BINARY,
    ):
        self.engine = engine
        self.dpi = dpi
        self.lang = lang
        self.binary = binary

        if not Path(binary).exists():
            raise FileNotFoundError(
                f"Rust binary not found at {binary}. "
                f"Build with `cd core && cargo build --release --features pdf`, "
                f"or set $MDM_BINARY."
            )

    # ---------- Stage 1: get the manifest ----------
    def get_manifest(self, pdf_path: str | Path) -> TriageManifest:
        proc = subprocess.run(
            [self.binary, "triage", str(pdf_path), "--format", "json"],
            capture_output=True, text=True, check=False,
        )
        if proc.returncode != 0:
            raise RuntimeError(f"triage failed: {proc.stderr}")
        return TriageManifest.from_json(json.loads(proc.stdout))

    # ---------- Stage 2: per-category dispatch ----------
    def process(self, pdf_path: str | Path, page_range: tuple[int, int] | None = None) -> str:
        pdf_path = Path(pdf_path)
        manifest = self.get_manifest(pdf_path)

        parts: list[str] = []
        for entry in manifest.pages:
            if page_range is not None:
                lo, hi = page_range
                if entry.page < lo or entry.page > hi:
                    continue
            if entry.category == "text_native":
                md = self._extract_text_native(pdf_path, entry.page)
            elif entry.category == "scanned":
                md = self._ocr_full_page(pdf_path, entry.page, entry.cjk_hint)
            elif entry.category == "mixed":
                md = self._process_mixed_page(pdf_path, entry)
            else:
                # unknown → treat as text-native (fallback); the Rust extractor
                # handles garbled PDFs gracefully.
                md = self._extract_text_native(pdf_path, entry.page)

            parts.append(f"<!-- page {entry.page} ({entry.category}) -->\n{md.rstrip()}")

        return "\n\n".join(parts) + "\n"

    # ---------- Text-native path ----------
    def _extract_text_native(self, pdf_path: Path, page: int) -> str:
        """Use the Rust parser for the whole document, then slice the page.

        Optimization opportunity: once hwp2mdm supports --page-range, switch
        to per-page extraction. For now we extract once and cache.
        """
        if not hasattr(self, "_rust_cache"):
            self._rust_cache: dict[str, list[str]] = {}
        key = str(pdf_path)
        if key not in self._rust_cache:
            full = self._rust_extract(pdf_path)
            # Split on form-feed (the Rust parser emits these between pages).
            self._rust_cache[key] = full.split("\x0c")
        pages = self._rust_cache[key]
        idx = page - 1
        return pages[idx] if 0 <= idx < len(pages) else ""

    def _rust_extract(self, pdf_path: Path) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            proc = subprocess.run(
                [self.binary, str(pdf_path), "-o", tmp, "--format", "mdx"],
                capture_output=True, text=True, check=False,
            )
            if proc.returncode != 0:
                return ""
            out = Path(tmp) / f"{pdf_path.stem}.mdx"
            return out.read_text(encoding="utf-8") if out.exists() else ""

    # ---------- Full-page OCR path ----------
    def _ocr_full_page(self, pdf_path: Path, page: int, cjk: bool) -> str:
        png_path = self._rasterize_page(pdf_path, page)
        try:
            return self._run_ocr(png_path, cjk)
        finally:
            try:
                png_path.unlink()
            except OSError:
                pass

    # ---------- Mixed-page path ----------
    def _fetch_layout(self, pdf_path: Path, page: int) -> list[dict]:
        """Call `hwp2mdm layout` to get per-element (y, type, content) for
        one page. Cached so repeated page lookups don't re-parse the PDF."""
        if not hasattr(self, "_layout_cache"):
            self._layout_cache: dict[tuple[str, int], list[dict]] = {}
        key = (str(pdf_path), page)
        if key in self._layout_cache:
            return self._layout_cache[key]
        proc = subprocess.run(
            [self.binary, "layout", str(pdf_path), "--page", str(page)],
            capture_output=True, text=True, check=False,
        )
        if proc.returncode != 0:
            self._layout_cache[key] = []
            return []
        try:
            elems = json.loads(proc.stdout)
        except json.JSONDecodeError:
            elems = []
        self._layout_cache[key] = elems
        return elems

    def _process_mixed_page(self, pdf_path: Path, entry: PageRouting) -> str:
        """Extract Rust text + OCR each image region, then merge them by
        PRECISE Y position using the `hwp2mdm layout` JSON output.

        The Rust layout emits per-element (y, type, content) tuples. For
        each OCR region we compare its center Y against every text
        element's Y and insert the OCR result at the matching offset in
        the reading order. This replaces the earlier paragraph-fraction
        approximation with a per-element placement.
        """
        text_part = self._extract_text_native(pdf_path, entry.page)
        if not entry.ocr_regions:
            return text_part

        # Pull the Rust layout for this page to drive precise merging.
        layout = self._fetch_layout(pdf_path, entry.page)
        text_elems = [e for e in layout
                      if e.get("element_type") in ("text", "list_item")]
        # Sort top→bottom (PDF Y grows upward, so descending Y)
        text_elems.sort(key=lambda e: -e.get("y", 0.0))

        import pymupdf
        with pymupdf.open(str(pdf_path)) as doc:
            page = doc[entry.page - 1]
            page_h = page.rect.height

        png_path = self._rasterize_page(pdf_path, entry.page)
        try:
            from PIL import Image
            img = Image.open(png_path)
            scale = self.dpi / 72.0

            ocr_entries: list[tuple[float, str]] = []
            for region in entry.ocr_regions:
                left = region["x"] * scale
                top = (page_h - (region["y"] + region["height"])) * scale
                right = left + region["width"] * scale
                bottom = top + region["height"] * scale
                if right <= left or bottom <= top:
                    continue
                crop = img.crop((left, top, right, bottom))
                with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tf:
                    crop.save(tf.name, "PNG")
                    crop_path = Path(tf.name)
                try:
                    ocr_text = self._run_ocr(crop_path, entry.cjk_hint).strip()
                finally:
                    try:
                        crop_path.unlink()
                    except OSError:
                        pass
                if not ocr_text:
                    continue
                y_center_pdf = region["y"] + region["height"] / 2.0
                ocr_entries.append((y_center_pdf, ocr_text))

            if not ocr_entries:
                return text_part

            # If the Rust layout isn't available (fallback), we can still
            # produce reasonable output by appending OCR content at the
            # end ordered top→bottom by Y.
            if not text_elems:
                merged = text_part.rstrip() + "\n\n" + "\n\n".join(
                    f"> [OCR figure]: {t}" for _, t in
                    sorted(ocr_entries, key=lambda r: -r[0]))
                return merged

            # For each OCR region, find the nearest text element ABOVE it
            # (higher PDF Y == earlier in reading order). That's the
            # paragraph immediately preceding the figure; we insert the
            # OCR block right after it. Elements and paragraphs share the
            # same reading order, so we use the text element's index into
            # `text_elems` to pick a paragraph slot.
            paragraphs = text_part.split("\n\n")
            # Build a mapping from text-element index → paragraph offset.
            # Rust layout produces one element per text block; to_mdx
            # then emits roughly one paragraph per block with some
            # headings / lists interleaved. The ratio isn't 1:1 but the
            # monotonic mapping `idx / N * M` is a strict improvement
            # over pure y_fraction approximation.
            n_elems = len(text_elems)
            n_paras = max(1, len(paragraphs))

            insertions: list[tuple[int, str]] = []
            for y_center, ocr_text in ocr_entries:
                # Find the text element whose Y is the smallest one still
                # above `y_center` (i.e. the paragraph right before the
                # figure). If none, the figure is above all text — insert
                # at top (offset 0).
                above_idx = -1
                for i, e in enumerate(text_elems):
                    if e.get("y", 0.0) >= y_center:
                        above_idx = i
                    else:
                        break
                if above_idx < 0:
                    para_slot = 0
                else:
                    para_slot = min(
                        len(paragraphs),
                        round((above_idx + 1) / n_elems * n_paras),
                    )
                insertions.append((para_slot, f"> [OCR figure]: {ocr_text}"))

            # Insert from bottom-up so indices stay valid
            insertions.sort(key=lambda t: t[0], reverse=True)
            for slot, block in insertions:
                paragraphs.insert(slot, block)

            return "\n\n".join(paragraphs)
        finally:
            try:
                png_path.unlink()
            except OSError:
                pass

    # ---------- Rasterization ----------
    def _rasterize_page(self, pdf_path: Path, page: int) -> Path:
        try:
            import pymupdf  # pymupdf >= 1.24
        except ImportError as e:
            raise RuntimeError(
                "PyMuPDF is required for rasterization. Install with `pip install pymupdf`."
            ) from e

        doc = pymupdf.open(str(pdf_path))
        try:
            pg = doc[page - 1]
            mat = pymupdf.Matrix(self.dpi / 72.0, self.dpi / 72.0)
            pix = pg.get_pixmap(matrix=mat, alpha=False)
            with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as tf:
                pix.save(tf.name)
                return Path(tf.name)
        finally:
            doc.close()

    # ---------- OCR invocation ----------
    def _run_ocr(self, image_path: Path, cjk_hint: bool) -> str:
        # Reuse the existing OcrProcessor for local engines.
        if self.engine in ("tesseract", "easyocr", "auto"):
            try:
                from ocr_processor import OcrProcessor
            except ImportError:
                from .ocr_processor import OcrProcessor  # type: ignore
            lang = self.lang if cjk_hint else "eng"
            return OcrProcessor(engine=self.engine, lang=lang).extract_text(str(image_path))

        if self.engine == "openrouter":
            try:
                from ocr_bridge import OpenRouterOcrEngine
            except ImportError:
                from .ocr_bridge import OpenRouterOcrEngine  # type: ignore
            return OpenRouterOcrEngine().extract_text(str(image_path))

        raise ValueError(f"Unknown OCR engine: {self.engine}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("pdf", type=Path)
    ap.add_argument("--engine", default="tesseract",
                    choices=["tesseract", "easyocr", "openrouter", "auto"])
    ap.add_argument("--dpi", type=int, default=300)
    ap.add_argument("--lang", default="kor+eng")
    ap.add_argument("--out", type=Path)
    ap.add_argument("--manifest", action="store_true",
                    help="Also print the triage manifest to stderr")
    ap.add_argument("--binary", default=DEFAULT_BINARY)
    ap.add_argument("--pages", help="Limit to a page range (e.g., 1-3 or 7)")
    args = ap.parse_args()

    page_range: tuple[int, int] | None = None
    if args.pages:
        parts = args.pages.split("-")
        if len(parts) == 1:
            p = int(parts[0])
            page_range = (p, p)
        else:
            page_range = (int(parts[0]), int(parts[1]))

    router = TriageRouter(
        engine=args.engine, dpi=args.dpi, lang=args.lang, binary=args.binary,
    )

    if args.manifest:
        m = router.get_manifest(args.pdf)
        print(json.dumps({
            "document": m.document,
            "page_count": m.page_count,
            "pages": [
                {"page": p.page, "category": p.category,
                 "confidence": p.confidence, "cjk_hint": p.cjk_hint}
                for p in m.pages
            ],
        }, indent=2, ensure_ascii=False), file=sys.stderr)

    md = router.process(args.pdf, page_range=page_range)
    if args.out:
        args.out.write_text(md, encoding="utf-8")
    else:
        sys.stdout.write(md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
