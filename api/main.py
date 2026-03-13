#!/usr/bin/env python3
"""
Markdown Media API Server
FastAPI server that converts HWP / DOCX / PDF files to MDM markdown.

Endpoints:
  POST /api/convert  — multipart file upload, returns JSON with markdown + base64 images
  GET  /api/health   — health check
  GET  /             — redirect to /docs
"""
from __future__ import annotations

import base64
import logging
import os
import struct
import sys
import tempfile
import zipfile
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# Converter path resolution
# Render deploy: repo root is mounted as /app, converters live at /app/converters
# Local dev:     __file__ is <repo>/api/main.py, converters at <repo>/converters
# ---------------------------------------------------------------------------
_REPO_ROOT = Path(__file__).parent.parent
_CONVERTERS_LOCAL = _REPO_ROOT / "converters"
_CONVERTERS_RENDER = Path("/app/converters")

for _p in (_CONVERTERS_RENDER, _CONVERTERS_LOCAL):
    if _p.exists() and str(_p) not in sys.path:
        sys.path.insert(0, str(_p))

# ---------------------------------------------------------------------------
# FastAPI
# ---------------------------------------------------------------------------
from fastapi import FastAPI, File, HTTPException, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import RedirectResponse

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(name)s: %(message)s")
logger = logging.getLogger("markdown-media-api")

app = FastAPI(
    title="Markdown Media Converter API",
    description="Convert HWP / DOCX / PDF files to MDM markdown with embedded base64 images.",
    version="1.0.0",
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_origin_regex=r"https://seunghan91\.github\.io.*",
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
MAX_FILE_BYTES = 20 * 1024 * 1024  # 20 MB
SUPPORTED_EXTENSIONS = {".hwp", ".docx", ".pdf"}


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _read_b64(path: Path) -> str:
    """Read a file and return its content as a base64 string."""
    return base64.b64encode(path.read_bytes()).decode("ascii")


def _images_to_b64(assets_dir: Path) -> dict[str, str]:
    """
    Walk *assets_dir* and return a mapping of filename -> base64-encoded bytes
    for every image file found.
    """
    result: dict[str, str] = {}
    if not assets_dir.exists():
        return result
    image_suffixes = {".png", ".jpg", ".jpeg", ".gif", ".bmp", ".webp", ".tiff"}
    for img_path in assets_dir.iterdir():
        if img_path.suffix.lower() in image_suffixes:
            result[img_path.name] = _read_b64(img_path)
    return result


def _build_image_markdown(image_names: list[str]) -> str:
    """Return MDM-syntax image embeds for a list of filenames."""
    return "\n\n".join(f"![[{name} | width=auto]]" for name in image_names)


# ---------------------------------------------------------------------------
# HWP conversion
# ---------------------------------------------------------------------------

def _extract_hwp_bindata_images(hwp_path: Path) -> dict[str, str]:
    """
    Extract embedded images from the BinData OLE streams inside an HWP file.

    HWP stores images as streams named  BinData/BIN0001, BIN0002, …
    Each stream is either raw image bytes or zlib-compressed image bytes,
    depending on the global FileHeader compression flag.

    Returns a mapping of  image_NNNN.{ext} -> base64-encoded bytes.
    """
    import zlib

    try:
        import olefile
    except ImportError:
        logger.warning("olefile not available; skipping HWP image extraction")
        return {}

    images: dict[str, str] = {}

    try:
        with olefile.OleFileIO(str(hwp_path)) as ole:
            # Determine global compression flag from FileHeader
            header_data = ole.openstream("FileHeader").read()
            is_compressed = bool(header_data[36] & 1)

            for entry in ole.listdir():
                # BinData streams look like ['BinData', 'BIN0001']
                if len(entry) == 2 and entry[0] == "BinData":
                    stream_name = f"BinData/{entry[1]}"
                    raw = ole.openstream(stream_name).read()

                    if is_compressed:
                        try:
                            raw = zlib.decompress(raw, -15)
                        except zlib.error:
                            try:
                                raw = zlib.decompress(raw)
                            except zlib.error:
                                pass  # keep original bytes

                    # Detect image type by magic bytes
                    ext = _detect_image_ext(raw)
                    img_name = f"{entry[1].lower()}.{ext}"
                    images[img_name] = base64.b64encode(raw).decode("ascii")

    except Exception as exc:
        logger.warning("HWP image extraction failed: %s", exc)

    return images


def _detect_image_ext(data: bytes) -> str:
    """Return a file extension based on image magic bytes."""
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:3] == b"\xff\xd8\xff":
        return "jpg"
    if data[:6] in (b"GIF87a", b"GIF89a"):
        return "gif"
    if data[:2] == b"BM":
        return "bmp"
    if data[:4] in (b"RIFF",) and data[8:12] == b"WEBP":
        return "webp"
    return "bin"


def _convert_hwp(tmp_path: Path) -> tuple[str, dict[str, str]]:
    """
    Convert an HWP file to MDM markdown.

    Returns:
        (markdown_text, {filename: base64_str, ...})
    """
    try:
        from hwp_converter import extract_hwp_text
    except ImportError as exc:
        raise RuntimeError(f"hwp_converter module not found: {exc}") from exc

    # Extract text
    try:
        text = extract_hwp_text(str(tmp_path))
        # Sanitise surrogate characters that would break JSON serialisation
        text = text.encode("utf-8", errors="surrogatepass").decode("utf-8", errors="replace")
    except Exception as exc:
        logger.warning("HWP text extraction failed: %s", exc)
        text = ""

    # Extract images from BinData streams
    images = _extract_hwp_bindata_images(tmp_path)

    # Build MDM markdown
    lines: list[str] = [text.strip()]
    if images:
        lines.append("")
        lines.append(_build_image_markdown(list(images.keys())))

    markdown = "\n\n".join(filter(None, lines))
    return markdown, images


# ---------------------------------------------------------------------------
# DOCX conversion
# ---------------------------------------------------------------------------

def _convert_docx(tmp_path: Path) -> tuple[str, dict[str, str]]:
    """
    Convert a DOCX file to MDM markdown.

    Returns:
        (markdown_text, {filename: base64_str, ...})
    """
    try:
        from docx_converter import DocxToMdxConverter
    except ImportError as exc:
        raise RuntimeError(f"docx_converter module not found: {exc}") from exc

    converter = DocxToMdxConverter(str(tmp_path))

    # Extract text
    try:
        text = converter.extract_text()
    except Exception as exc:
        logger.warning("DOCX text extraction failed: %s", exc)
        text = ""

    # Extract images into a temporary assets directory
    images: dict[str, str] = {}
    with tempfile.TemporaryDirectory() as assets_tmp:
        assets_dir = Path(assets_tmp)
        try:
            converter.extract_images(assets_dir)
            images = _images_to_b64(assets_dir)
        except Exception as exc:
            logger.warning("DOCX image extraction failed: %s", exc)

    # Build MDM markdown
    lines: list[str] = [text.strip()]
    if images:
        lines.append("")
        lines.append(_build_image_markdown(list(images.keys())))

    markdown = "\n\n".join(filter(None, lines))
    return markdown, images


# ---------------------------------------------------------------------------
# PDF conversion
# ---------------------------------------------------------------------------

def _convert_pdf(tmp_path: Path) -> tuple[str, dict[str, str]]:
    """
    Convert a PDF file to MDM markdown (text only; pdfplumber does not
    expose raster images, so images dict will always be empty).

    Returns:
        (markdown_text, {})
    """
    try:
        from pdf_converter import convert_pdf_to_mdx
    except ImportError as exc:
        raise RuntimeError(f"pdf_converter module not found: {exc}") from exc

    with tempfile.TemporaryDirectory() as out_tmp:
        out_dir = Path(out_tmp)
        try:
            convert_pdf_to_mdx(str(tmp_path), str(out_dir))
        except Exception as exc:
            raise RuntimeError(f"PDF conversion failed: {exc}") from exc

        # Read back the generated .mdx file (the only .mdx in out_dir)
        mdx_files = list(out_dir.glob("*.mdx"))
        if not mdx_files:
            raise RuntimeError("PDF converter produced no .mdx output")

        raw_content = mdx_files[0].read_text(encoding="utf-8")

        # Strip the YAML front-matter block so we return clean body markdown
        markdown = _strip_front_matter(raw_content)

        # PDF assets (images) are not produced by the current converter
        images: dict[str, str] = {}
        assets_dir = out_dir / "assets"
        if assets_dir.exists():
            images = _images_to_b64(assets_dir)

    return markdown, images


def _strip_front_matter(content: str) -> str:
    """Remove YAML front-matter (--- ... ---) from the top of a markdown string."""
    if not content.startswith("---"):
        return content
    end = content.find("\n---", 3)
    if end == -1:
        return content
    # Skip past the closing ---\n
    body = content[end + 4:].lstrip("\n")
    return body


# ---------------------------------------------------------------------------
# Route handlers
# ---------------------------------------------------------------------------

@app.get("/", include_in_schema=False)
async def root() -> RedirectResponse:
    """Redirect bare root to the auto-generated API docs."""
    return RedirectResponse(url="/docs")


@app.get("/api/health", tags=["meta"])
async def health() -> dict[str, Any]:
    """Return server health status and available converter modules."""
    converters: dict[str, bool] = {}
    for module_name in ("hwp_converter", "docx_converter", "pdf_converter"):
        try:
            __import__(module_name)
            converters[module_name] = True
        except ImportError:
            converters[module_name] = False

    return {
        "status": "ok",
        "converters": converters,
        "max_file_bytes": MAX_FILE_BYTES,
        "supported_formats": sorted(SUPPORTED_EXTENSIONS),
    }


@app.post("/api/convert", tags=["convert"])
async def convert_file(file: UploadFile = File(...)) -> dict[str, Any]:
    """
    Convert an uploaded HWP / DOCX / PDF document to MDM markdown.

    The response includes:
    - **markdown**: full MDM-formatted markdown string
    - **images**: map of filename to base64-encoded image data
    - **stats**: character count and image count
    """
    # --- Validate file extension ---
    filename = file.filename or "upload"
    suffix = Path(filename).suffix.lower()
    if suffix not in SUPPORTED_EXTENSIONS:
        raise HTTPException(
            status_code=400,
            detail=(
                f"Unsupported file format '{suffix}'. "
                f"Supported formats: {', '.join(sorted(SUPPORTED_EXTENSIONS))}"
            ),
        )

    # --- Read file content with size guard ---
    content = await file.read()
    if len(content) > MAX_FILE_BYTES:
        raise HTTPException(
            status_code=400,
            detail=f"File too large ({len(content):,} bytes). Maximum is {MAX_FILE_BYTES:,} bytes.",
        )

    if len(content) == 0:
        raise HTTPException(status_code=400, detail="Uploaded file is empty.")

    logger.info("Converting '%s' (%d bytes, format=%s)", filename, len(content), suffix)

    # --- Write to a temp file so converters can open it by path ---
    try:
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as tmp_file:
            tmp_file.write(content)
            tmp_path = Path(tmp_file.name)
    except OSError as exc:
        raise HTTPException(status_code=500, detail=f"Failed to write temp file: {exc}") from exc

    try:
        # --- Dispatch to the appropriate converter ---
        try:
            if suffix == ".hwp":
                markdown, images = _convert_hwp(tmp_path)
            elif suffix == ".docx":
                markdown, images = _convert_docx(tmp_path)
            elif suffix == ".pdf":
                markdown, images = _convert_pdf(tmp_path)
            else:
                # Should not reach here due to earlier validation, but be safe
                raise HTTPException(status_code=400, detail=f"Unhandled format: {suffix}")
        except HTTPException:
            raise
        except RuntimeError as exc:
            logger.exception("Conversion error for '%s'", filename)
            raise HTTPException(status_code=500, detail=str(exc)) from exc
        except Exception as exc:
            logger.exception("Unexpected error converting '%s'", filename)
            raise HTTPException(
                status_code=500,
                detail=f"Conversion failed: {type(exc).__name__}: {exc}",
            ) from exc

    finally:
        # Always clean up the temporary input file
        try:
            tmp_path.unlink(missing_ok=True)
        except OSError:
            pass

    return {
        "filename": filename,
        "format": suffix.lstrip("."),
        "markdown": markdown,
        "images": images,
        "stats": {
            "chars": len(markdown),
            "images": len(images),
        },
    }
