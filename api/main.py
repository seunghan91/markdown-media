#!/usr/bin/env python3
"""
Markdown Media API Server.

FastAPI server that converts HWP / HWPX / DOCX / PDF files to MDM markdown.
Backed by the Rust mdm-core engine (published on PyPI as `mdm-core`).

Endpoints:
  POST /api/convert  — multipart file upload, returns JSON with markdown + base64 images
  GET  /api/health   — health check
  GET  /             — redirect to /docs
"""
from __future__ import annotations

import base64
import logging
from pathlib import Path
from typing import Any

from fastapi import FastAPI, File, HTTPException, UploadFile
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import RedirectResponse

from mdm_core import _mdm_native

logging.basicConfig(level=logging.INFO, format="%(levelname)s %(name)s: %(message)s")
logger = logging.getLogger("markdown-media-api")

app = FastAPI(
    title="Markdown Media Converter API",
    description="Convert HWP / HWPX / DOCX / PDF files to MDM markdown with embedded base64 images.",
    version="2.0.0",
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
SUPPORTED_EXTENSIONS = {".hwp", ".hwpx", ".docx", ".pdf"}

# Rust core sometimes returns this sentinel for HWP files where text extraction
# failed (e.g. unusual compression). Frontend expects empty string rather than
# the diagnostic message, so strip it at the API boundary.
_HWP_NO_TEXT_SENTINEL = "No text extracted. File may be encrypted or have unsupported format."


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _build_image_markdown(image_names: list[str]) -> str:
    """Return MDM-syntax image embeds for a list of filenames."""
    return "\n\n".join(f"![[{name} | width=auto]]" for name in image_names)


def _convert(data: bytes, filename: str) -> tuple[str, dict[str, str]]:
    """
    Convert in-memory document bytes to MDM markdown + base64 image map.

    Returns:
        (markdown, {filename: base64_str, ...})
    """
    # Text extraction via Rust core.
    try:
        markdown = _mdm_native.convert_bytes(data, filename)
    except ValueError as exc:
        raise HTTPException(status_code=500, detail=f"Conversion failed: {exc}") from exc

    # HWP sentinel → treat as empty content so images still render.
    if markdown.strip() == _HWP_NO_TEXT_SENTINEL:
        markdown = ""

    # Image extraction via Rust core.
    try:
        raw_images = _mdm_native.extract_images(data, filename)
    except ValueError as exc:
        # Non-fatal: log and return markdown only.
        logger.warning("Image extraction failed for '%s': %s", filename, exc)
        raw_images = {}

    images: dict[str, str] = {
        name: base64.b64encode(bytes(payload)).decode("ascii")
        for name, payload in raw_images.items()
    }

    # Append MDM embed syntax for extracted images — preserves pre-2.0 behavior
    # where the markdown references images the client then resolves from the
    # `images` dict.
    if images:
        lines = [markdown.strip()] if markdown.strip() else []
        lines.append(_build_image_markdown(list(images.keys())))
        markdown = "\n\n".join(lines)

    return markdown, images


# ---------------------------------------------------------------------------
# Route handlers
# ---------------------------------------------------------------------------

@app.get("/", include_in_schema=False)
async def root() -> RedirectResponse:
    """Redirect bare root to the auto-generated API docs."""
    return RedirectResponse(url="/docs")


@app.get("/api/health", tags=["meta"])
async def health() -> dict[str, Any]:
    """Return server health status and engine info."""
    return {
        "status": "ok",
        "engine": "mdm-core",
        "engine_version": _mdm_native.version(),
        "max_file_bytes": MAX_FILE_BYTES,
        "supported_formats": sorted(SUPPORTED_EXTENSIONS),
    }


@app.post("/api/convert", tags=["convert"])
async def convert_file(file: UploadFile = File(...)) -> dict[str, Any]:
    """
    Convert an uploaded HWP / HWPX / DOCX / PDF document to MDM markdown.

    The response includes:
    - **markdown**: full MDM-formatted markdown string
    - **images**: map of filename to base64-encoded image data
    - **stats**: character count and image count
    """
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

    content = await file.read()
    if len(content) > MAX_FILE_BYTES:
        raise HTTPException(
            status_code=400,
            detail=f"File too large ({len(content):,} bytes). Maximum is {MAX_FILE_BYTES:,} bytes.",
        )
    if len(content) == 0:
        raise HTTPException(status_code=400, detail="Uploaded file is empty.")

    logger.info("Converting '%s' (%d bytes, format=%s)", filename, len(content), suffix)

    try:
        markdown, images = _convert(content, filename)
    except HTTPException:
        raise
    except Exception as exc:
        logger.exception("Unexpected error converting '%s'", filename)
        raise HTTPException(
            status_code=500,
            detail=f"Conversion failed: {type(exc).__name__}: {exc}",
        ) from exc

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
