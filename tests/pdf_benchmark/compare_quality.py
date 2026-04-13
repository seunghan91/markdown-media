#!/usr/bin/env python3
"""Compare MDM vs Marker vs pdftotext output quality for PDF parsing."""
import os, re

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))

def read_file(path):
    try:
        with open(path) as f:
            return f.read()
    except:
        return ""

def check(text, pattern, is_regex=False):
    if is_regex:
        return bool(re.search(pattern, text, re.MULTILINE))
    return pattern in text

def compare(test_name, checks):
    marker = read_file(f"{BENCH_DIR}/output/marker/{test_name}.md")
    pdftotext_f = read_file(f"{BENCH_DIR}/output/pdftotext/{test_name}.txt")
    mdm_dir = f"{BENCH_DIR}/output/mdm/{test_name}"
    mdm = ""
    if os.path.isdir(mdm_dir):
        for f in os.listdir(mdm_dir):
            if f.endswith(".mdx"):
                mdm = read_file(os.path.join(mdm_dir, f))
                break

    print(f"\n{'='*70}")
    print(f"  {test_name}")
    print(f"{'='*70}")
    print(f"  {'Feature':<40} {'Marker':>8} {'pdftext':>8} {'MDM':>8}")
    print(f"  {'-'*40} {'-'*8} {'-'*8} {'-'*8}")

    scores = [0, 0, 0]
    total = len(checks)

    for name, pattern, *opts in checks:
        is_regex = opts[0] if opts else False
        results = [
            check(marker, pattern, is_regex),
            check(pdftotext_f, pattern, is_regex),
            check(mdm, pattern, is_regex),
        ]
        for i, r in enumerate(results):
            scores[i] += r
        syms = ["✅" if r else "❌" for r in results]
        print(f"  {name:<40} {syms[0]:>8} {syms[1]:>8} {syms[2]:>8}")

    print(f"  {'-'*40} {'-'*8} {'-'*8} {'-'*8}")
    for name, score in zip(["Marker", "pdftext", "MDM"], scores):
        pct = score/total*100 if total else 0
        print(f"  {name + ' SCORE':<40} {score:>5}/{total}  ({pct:.0f}%)")

    return scores, total


if __name__ == "__main__":
    totals = [0, 0, 0]
    grand_total = 0

    # Test 1: Comprehensive
    checks = [
        ("H1 heading (# Document Title)", r"^#\s+.*Document Title", True),
        ("H2 heading (## Section One)", r"^#{1,3}\s+.*Section One", True),
        ("H3 heading (### Subsection)", r"^#{1,4}\s+.*Subsection", True),
        ("Bold text (**bold**)", "**bold"),
        ("Italic text (*italic*)", "*italic"),
        ("Bullet list marker", r"^\s*[-•]\s+.*bullet item", True),
        ("Numbered list (1. or 1)", r"\d\s+.*numbered item", True),
        ("Table pipe format", "| Name"),
        ("Table separator (---)", "| --- |"),
        ("Table data (Alice)", "Alice"),
        ("Body text preserved", "basic paragraph extraction"),
        ("Metadata title", "MDM PDF Parser Benchmark"),
    ]
    s, t = compare("test_comprehensive", checks)
    for i in range(3): totals[i] += s[i]
    grand_total += t

    # Test 2: Heading hierarchy
    checks = [
        ("H1 for main title", r"^#\s+.*Main Title", True),
        ("H2 for chapter (not H1)", r"^##\s+.*Chapter One", True),
        ("H3 for section", r"^###\s+.*Section 1\.1", True),
        ("H4 for detail", r"^####\s+.*Detail 1\.1\.1", True),
        ("Consistent H2 for Ch2", r"^##\s+.*Chapter Two", True),
        ("Body text preserved", "Introduction paragraph"),
        ("All headings detected", r"^#+\s+", True),
    ]
    s, t = compare("test_headings", checks)
    for i in range(3): totals[i] += s[i]
    grand_total += t

    # Test 3: Two-column layout
    checks = [
        ("Left col text intact", "Left column paragraph one"),
        ("Right col text intact", "Right column paragraph one"),
        ("Reading order (left before right)", r"Left column.*Right column", True),
        ("No garbled text mixing", "Proper reading order"),
        ("After-columns section", "After Columns"),
        ("Full-width content detected", "full-width content"),
    ]
    s, t = compare("test_twocolumn", checks)
    for i in range(3): totals[i] += s[i]
    grand_total += t

    # Test 4: Headers/footers
    checks = [
        ("Main content preserved", "Main Document Title"),
        ("Section headings", "Section One"),
        ("Page 2 content", "Section Two"),
        ("Header stripped (no 'Confidential' in body)", True),  # special
        ("Footer stripped (no 'MDM Parser Benchmark' as content)", True),  # special
        ("Page numbers stripped", True),  # special
    ]
    # Manual checks for header/footer
    marker = read_file(f"{BENCH_DIR}/output/marker/test_headers_footers.md")
    pdftotext_f = read_file(f"{BENCH_DIR}/output/pdftotext/test_headers_footers.txt")
    mdm_dir = f"{BENCH_DIR}/output/mdm/test_headers_footers"
    mdm = ""
    if os.path.isdir(mdm_dir):
        for f in os.listdir(mdm_dir):
            if f.endswith(".mdx"):
                mdm = read_file(os.path.join(mdm_dir, f))
                break

    hf_checks = [
        ("Main content preserved", "Main Document Title"),
        ("Section headings", "Section One"),
        ("Page 2 content", "Section Two"),
        ("Body paragraphs preserved", "should be preserved"),
    ]
    # Header/footer should NOT appear in body content
    hf_strip_checks = [
        # For these, presence is BAD (means headers/footers weren't stripped)
    ]

    s, t = compare("test_headers_footers", hf_checks)
    for i in range(3): totals[i] += s[i]
    grand_total += t

    # Overall
    print(f"\n{'='*70}")
    print(f"  OVERALL SCORE")
    print(f"{'='*70}")
    for name, score in zip(["Marker", "pdftotext", "MDM"], totals):
        pct = score/grand_total*100 if grand_total else 0
        print(f"  {name:<15} {score}/{grand_total} ({pct:.0f}%)")

    gap = totals[0] - totals[2]
    if gap > 0:
        print(f"\n  Gap: MDM is {gap} features behind Marker")
    elif gap < 0:
        print(f"\n  MDM LEADS Marker by {-gap} features!")
    else:
        print(f"\n  PARITY with Marker!")
