"""Smoke tests for pdf_triage_router.

The full pipeline (rasterize + OCR) needs PyMuPDF + Tesseract installed, so
these tests focus on the parts that don't: manifest parsing + text-native
routing. End-to-end OCR coverage is handled by the bench harness.
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

# Put parser-py on sys.path (no package install needed for CI smoke tests).
REPO_ROOT = Path(__file__).resolve().parent.parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "packages" / "parser-py"))
BINARY = REPO_ROOT / "core" / "target" / "release" / "hwp2mdm"


@pytest.fixture(scope="module")
def sample_pdf() -> Path:
    p = REPO_ROOT / "tests" / "pdf_benchmark" / "test_comprehensive.pdf"
    if not p.exists():
        pytest.skip(f"missing fixture {p}")
    return p


@pytest.fixture(scope="module")
def scanned_pdf() -> Path:
    p = REPO_ROOT / "tests" / "realworld" / "mois" / "간행_121669_0.pdf"
    if not p.exists():
        pytest.skip(f"missing fixture {p}")
    return p


@pytest.fixture(scope="module", autouse=True)
def ensure_binary():
    if not BINARY.exists():
        pytest.skip(f"hwp2mdm binary missing: {BINARY} — run `cargo build --release`")


def test_manifest_roundtrip_via_cli(sample_pdf):
    """hwp2mdm triage --format json produces JSON we can parse."""
    proc = subprocess.run(
        [str(BINARY), "triage", str(sample_pdf), "--format", "json"],
        capture_output=True, text=True, check=True,
    )
    data = json.loads(proc.stdout)
    assert "document" in data
    assert "pages" in data
    assert data["page_count"] == len(data["pages"])
    for p in data["pages"]:
        assert p["category"] in {"text_native", "scanned", "mixed", "unknown"}
        assert 0.0 <= p["confidence"] <= 1.0


def test_manifest_parses_into_dataclass(sample_pdf):
    from pdf_triage_router import TriageRouter
    router = TriageRouter(binary=str(BINARY))
    m = router.get_manifest(sample_pdf)
    assert m.page_count == len(m.pages)
    for p in m.pages:
        assert p.page >= 1
        assert isinstance(p.ocr_regions, list)
        assert isinstance(p.cjk_hint, bool)


def test_scanned_pdf_classified_as_scanned(scanned_pdf):
    """간행_121669_0.pdf is a full-scan doc with OCR underlay — must route to OCR."""
    from pdf_triage_router import TriageRouter
    router = TriageRouter(binary=str(BINARY))
    m = router.get_manifest(scanned_pdf)
    categories = [p.category for p in m.pages]
    assert "scanned" in categories, f"expected at least one scanned page, got {categories}"
    # All scanned pages must flag full_page_ocr.
    for p in m.pages:
        if p.category == "scanned":
            assert p.needs_full_page_ocr is True


def test_text_native_digital_pdfs(sample_pdf):
    """Digital-born PDFs must NEVER be classified as scanned."""
    from pdf_triage_router import TriageRouter
    router = TriageRouter(binary=str(BINARY))
    m = router.get_manifest(sample_pdf)
    for p in m.pages:
        assert p.category != "scanned", \
            f"digital PDF page {p.page} wrongly classified as scanned"
