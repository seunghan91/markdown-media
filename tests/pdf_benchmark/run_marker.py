#!/usr/bin/env python3
"""Run Marker on test PDFs for benchmarking."""
import os, sys, glob

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))
OUT_DIR = os.path.join(BENCH_DIR, "output", "marker")
os.makedirs(OUT_DIR, exist_ok=True)

from marker.converters.pdf import PdfConverter
from marker.models import create_model_dict

print("Loading Marker models (this may take a moment)...")
models = create_model_dict()
converter = PdfConverter(artifact_dict=models)

for pdf_path in sorted(glob.glob(os.path.join(BENCH_DIR, "test_*.pdf"))):
    base = os.path.splitext(os.path.basename(pdf_path))[0]
    print(f"  Converting: {base}...")
    try:
        rendered = converter(pdf_path)
        md_text = rendered.markdown
        out_path = os.path.join(OUT_DIR, f"{base}.md")
        with open(out_path, "w") as f:
            f.write(md_text)
        print(f"    -> {len(md_text)} chars, {md_text.count(chr(10))} lines")
    except Exception as e:
        print(f"    ERROR: {e}")

print("\nMarker conversion complete!")
