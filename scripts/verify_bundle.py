#!/usr/bin/env python3
"""Verify a converted MDM bundle's internal consistency.

Usage: python3 scripts/verify_bundle.py [--strict] <mdx_path>

Given a .mdx file, locates the sibling .mdm (same stem) and assets/
directory, then checks:
  (a) every ![...](...) image reference in the .mdx resolves to a real file
  (b) leftover legacy syntax markers ([이미지: / @[[ / ![[) — warning by
      default, FAIL under --strict (use --strict for formats whose reference
      rewrite has already been migrated; without it a bundle whose references
      are still in legacy syntax reports a hollow OK on check (a))
  (c) .mdm (JSON) asset count matches the number of files on disk
  (d) every asset filename extension is lowercase

Checks (a), (c), (d) always affect the exit code. Under --strict, (b) does
too, and a bundle with assets but zero ![...](...)  references also fails.
"""

import json
import re
import sys
from pathlib import Path

IMAGE_REF_RE = re.compile(r"!\[[^\]]*\]\(([^)]+)\)")
LEGACY_MARKERS = ("[이미지:", "@[[", "![[")


def main() -> int:
    args = sys.argv[1:]
    strict = "--strict" in args
    args = [a for a in args if a != "--strict"]
    if len(args) != 1:
        print(f"usage: {sys.argv[0]} [--strict] <mdx_path>", file=sys.stderr)
        return 2

    mdx_path = Path(args[0]).resolve()
    if not mdx_path.is_file():
        print(f"FAIL: mdx file not found: {mdx_path}")
        return 1

    base_dir = mdx_path.parent
    stem = mdx_path.stem
    mdm_path = base_dir / f"{stem}.mdm"
    assets_dir = base_dir / "assets"

    text = mdx_path.read_text(encoding="utf-8")
    exit_code = 0

    # (a) image references resolve to real files
    refs = IMAGE_REF_RE.findall(text)
    broken = []
    for ref in refs:
        ref = ref.strip()
        if ref.startswith(("http://", "https://", "data:")):
            continue
        target = (base_dir / ref).resolve()
        if not target.is_file():
            broken.append(ref)

    if broken:
        print(f"FAIL: {len(broken)}/{len(refs)} image reference(s) broken:")
        for ref in broken:
            print(f"       - {ref}")
        exit_code = 1
    else:
        print(f"OK: image references ({len(refs)} found, all resolve)")

    # (b) leftover legacy syntax markers — warning by default, FAIL in strict
    marker_counts = {m: text.count(m) for m in LEGACY_MARKERS}
    total_markers = sum(marker_counts.values())
    if total_markers:
        detail = ", ".join(f"{m!r}={c}" for m, c in marker_counts.items() if c)
        if strict:
            print(f"FAIL: {total_markers} legacy syntax marker(s) remaining ({detail})")
            exit_code = 1
        else:
            print(f"WARN: {total_markers} legacy syntax marker(s) remaining ({detail})")
    else:
        print("OK: no legacy syntax markers ([이미지: / @[[ / ![[)")

    # (c) manifest asset count vs disk file count
    if not mdm_path.is_file():
        print(f"FAIL: manifest not found: {mdm_path}")
        exit_code = 1
        manifest_assets = []
    else:
        try:
            manifest = json.loads(mdm_path.read_text(encoding="utf-8"))
            manifest_assets = manifest.get("assets", [])
        except json.JSONDecodeError as e:
            print(f"FAIL: manifest is not valid JSON: {e}")
            exit_code = 1
            manifest_assets = []

    disk_files = [p for p in assets_dir.rglob("*") if p.is_file()] if assets_dir.is_dir() else []

    if len(manifest_assets) == len(disk_files):
        print(f"OK: manifest assets ({len(manifest_assets)}) == disk files ({len(disk_files)})")
    else:
        print(f"FAIL: manifest assets ({len(manifest_assets)}) != disk files ({len(disk_files)})")
        exit_code = 1

    # (d) asset extensions are lowercase
    non_lowercase = [p.name for p in disk_files if p.suffix != p.suffix.lower()]
    if non_lowercase:
        print(f"FAIL: {len(non_lowercase)} asset(s) with non-lowercase extension: {non_lowercase}")
        exit_code = 1
    else:
        print(f"OK: all {len(disk_files)} asset extension(s) lowercase")

    # strict: a bundle with assets but no standard references is hollow
    if strict and manifest_assets and not refs:
        print("FAIL: bundle has assets but zero ![...](...)  references (--strict)")
        exit_code = 1

    return exit_code


if __name__ == "__main__":
    sys.exit(main())
