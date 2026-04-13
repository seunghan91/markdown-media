#!/usr/bin/env python3
"""
MDM Benchmark Engine — quantitative comparison with BLEU, edit distance, structure score.
Compares MDM output against reference parsers (Pandoc, Marker, pdftotext).
"""

import os, re, sys, json, subprocess, time, difflib
from pathlib import Path
from collections import Counter

BENCH_DIR = Path(__file__).parent
PROJECT_ROOT = BENCH_DIR.parent
MDM_BIN = PROJECT_ROOT / "core" / "target" / "debug" / "hwp2mdm"


# ─── Metrics ───

def normalized_edit_distance(ref: str, hyp: str) -> float:
    """Normalized Levenshtein edit distance (0=identical, 1=completely different)."""
    if not ref and not hyp:
        return 0.0
    ref_words = ref.split()
    hyp_words = hyp.split()
    sm = difflib.SequenceMatcher(None, ref_words, hyp_words)
    return 1.0 - sm.ratio()


def bleu_score(ref: str, hyp: str, max_n: int = 4) -> float:
    """Simple BLEU score (unigram to max_n-gram)."""
    ref_tokens = ref.split()
    hyp_tokens = hyp.split()

    if not ref_tokens or not hyp_tokens:
        return 0.0

    scores = []
    for n in range(1, max_n + 1):
        ref_ngrams = Counter(tuple(ref_tokens[i:i+n]) for i in range(len(ref_tokens) - n + 1))
        hyp_ngrams = Counter(tuple(hyp_tokens[i:i+n]) for i in range(len(hyp_tokens) - n + 1))

        overlap = sum((hyp_ngrams & ref_ngrams).values())
        total = sum(hyp_ngrams.values())

        if total == 0:
            scores.append(0.0)
        else:
            scores.append(overlap / total)

    # Geometric mean
    import math
    product = 1.0
    for s in scores:
        product *= max(s, 1e-10)
    bleu = product ** (1.0 / len(scores))

    # Brevity penalty
    bp = min(1.0, len(hyp_tokens) / max(len(ref_tokens), 1))
    return bleu * bp


def structure_score(text: str) -> dict:
    """Analyze markdown structural elements."""
    lines = text.split('\n')
    return {
        'headings': sum(1 for l in lines if re.match(r'^#{1,6}\s', l)),
        'h1': sum(1 for l in lines if re.match(r'^#\s', l)),
        'h2': sum(1 for l in lines if re.match(r'^##\s', l)),
        'h3': sum(1 for l in lines if re.match(r'^###\s', l)),
        'h4': sum(1 for l in lines if re.match(r'^####\s', l)),
        'bold': len(re.findall(r'\*\*[^*]+\*\*', text)),
        'italic': len(re.findall(r'(?<!\*)\*[^*]+\*(?!\*)', text)),
        'bullet_lists': sum(1 for l in lines if re.match(r'^\s*[-•*]\s', l)),
        'numbered_lists': sum(1 for l in lines if re.match(r'^\s*\d+[.)]\s', l)),
        'tables': sum(1 for l in lines if l.strip().startswith('|') and '|' in l[1:]),
        'links': len(re.findall(r'\[([^\]]+)\]\(([^)]+)\)', text)),
        'footnotes': len(re.findall(r'\[\^[^\]]+\]', text)),
        'blockquotes': sum(1 for l in lines if l.strip().startswith('>')),
        'images': len(re.findall(r'!\[', text)),
        'code_blocks': text.count('```'),
        'total_words': len(text.split()),
        'total_lines': len(lines),
    }


def text_completeness(ref: str, hyp: str) -> float:
    """What fraction of reference words appear in hypothesis."""
    ref_words = set(ref.lower().split())
    hyp_words = set(hyp.lower().split())
    if not ref_words:
        return 1.0
    return len(ref_words & hyp_words) / len(ref_words)


# ─── Runners ───

def run_mdm(input_file: str, output_dir: str) -> tuple:
    """Run MDM parser. Returns (output_text, elapsed_ms)."""
    os.makedirs(output_dir, exist_ok=True)
    start = time.time()
    result = subprocess.run(
        [str(MDM_BIN), input_file, "-o", output_dir],
        capture_output=True, text=True, timeout=30
    )
    elapsed = (time.time() - start) * 1000

    # Find output file
    for f in Path(output_dir).glob("*.mdx"):
        return f.read_text(), elapsed
    return "", elapsed


def run_pandoc(input_file: str, output_file: str) -> tuple:
    """Run Pandoc. Returns (output_text, elapsed_ms)."""
    start = time.time()
    subprocess.run(
        ["pandoc", input_file, "-t", "markdown", "-o", output_file],
        capture_output=True, timeout=30
    )
    elapsed = (time.time() - start) * 1000

    if os.path.exists(output_file):
        return Path(output_file).read_text(), elapsed
    return "", elapsed


def run_pdftotext(input_file: str, output_file: str) -> tuple:
    """Run pdftotext. Returns (output_text, elapsed_ms)."""
    start = time.time()
    subprocess.run(
        ["pdftotext", "-layout", input_file, output_file],
        capture_output=True, timeout=30
    )
    elapsed = (time.time() - start) * 1000

    if os.path.exists(output_file):
        return Path(output_file).read_text(), elapsed
    return "", elapsed


def run_marker(input_file: str) -> tuple:
    """Run Marker via Python API. Returns (output_text, elapsed_ms)."""
    try:
        start = time.time()
        from marker.converters.pdf import PdfConverter
        from marker.models import create_model_dict
        models = create_model_dict()
        converter = PdfConverter(artifact_dict=models)
        rendered = converter(input_file)
        elapsed = (time.time() - start) * 1000
        return rendered.markdown, elapsed
    except Exception as e:
        return f"(Marker error: {e})", 0


# ─── Normalize text for comparison ───

def normalize_for_comparison(text: str) -> str:
    """Strip frontmatter, metadata sections, normalize whitespace."""
    # Remove YAML frontmatter
    text = re.sub(r'^---\n.*?---\n', '', text, flags=re.DOTALL)
    # Remove HTML comments
    text = re.sub(r'<!--.*?-->', '', text, flags=re.DOTALL)
    # Remove "## Font Styles" appended sections
    text = re.sub(r'\n## Font Styles\n.*$', '', text, flags=re.DOTALL)
    # Remove "## Images" appended sections
    text = re.sub(r'\n## Images\n.*$', '', text, flags=re.DOTALL)
    # Remove "## Tables" appended sections
    text = re.sub(r'\n## Tables\n.*$', '', text, flags=re.DOTALL)
    # Normalize whitespace
    text = re.sub(r'\n{3,}', '\n\n', text)
    return text.strip()


# ─── Main benchmark ───

def benchmark_file(input_file: str, file_type: str, output_base: str):
    """Benchmark a single file across all available parsers."""
    basename = Path(input_file).stem
    results = {}

    # MDM
    mdm_dir = os.path.join(output_base, "mdm", basename)
    mdm_text, mdm_ms = run_mdm(input_file, mdm_dir)
    results['MDM'] = {'text': mdm_text, 'time_ms': mdm_ms}

    if file_type == 'pdf':
        # pdftotext
        pdftotext_file = os.path.join(output_base, "pdftotext", f"{basename}.txt")
        os.makedirs(os.path.dirname(pdftotext_file), exist_ok=True)
        pt_text, pt_ms = run_pdftotext(input_file, pdftotext_file)
        results['pdftotext'] = {'text': pt_text, 'time_ms': pt_ms}

    if file_type in ('pdf', 'docx'):
        # Pandoc
        pandoc_file = os.path.join(output_base, "pandoc", f"{basename}.md")
        os.makedirs(os.path.dirname(pandoc_file), exist_ok=True)
        pd_text, pd_ms = run_pandoc(input_file, pandoc_file)
        results['Pandoc'] = {'text': pd_text, 'time_ms': pd_ms}

    return results


def compare_results(results: dict, basename: str):
    """Compare all parser results quantitatively."""
    print(f"\n{'='*70}")
    print(f"  {basename}")
    print(f"{'='*70}")

    # Use longest output as pseudo-reference for cross-comparison
    texts = {k: normalize_for_comparison(v['text']) for k, v in results.items()}

    # Structure analysis
    print(f"\n  {'Structure':<20}", end="")
    for name in results:
        print(f" {name:>12}", end="")
    print()
    print(f"  {'-'*20}", end="")
    for _ in results:
        print(f" {'-'*12}", end="")
    print()

    metrics = ['headings', 'bold', 'italic', 'bullet_lists', 'numbered_lists',
               'tables', 'links', 'footnotes', 'blockquotes', 'total_words']
    structures = {k: structure_score(v) for k, v in texts.items()}

    for metric in metrics:
        print(f"  {metric:<20}", end="")
        for name in results:
            val = structures[name].get(metric, 0)
            print(f" {val:>12}", end="")
        print()

    # Speed comparison
    print(f"\n  {'Speed (ms)':<20}", end="")
    for name, data in results.items():
        print(f" {data['time_ms']:>11.0f}ms", end="")
    print()

    # Cross-BLEU (each parser vs each other)
    names = list(results.keys())
    if len(names) >= 2:
        print(f"\n  Cross-BLEU scores:")
        for i, n1 in enumerate(names):
            for j, n2 in enumerate(names):
                if i < j:
                    b = bleu_score(texts[n1], texts[n2])
                    ed = normalized_edit_distance(texts[n1], texts[n2])
                    tc = text_completeness(texts[n1], texts[n2])
                    print(f"    {n1} vs {n2}: BLEU={b:.3f}  EditDist={ed:.3f}  Completeness={tc:.3f}")

    return structures


def run_full_benchmark():
    """Run complete benchmark across all test files."""
    output_base = str(BENCH_DIR / "benchmark_output")
    os.makedirs(output_base, exist_ok=True)

    all_results = {}

    # DOCX tests
    docx_dir = BENCH_DIR / "docx_benchmark"
    for f in sorted(docx_dir.glob("test_*.docx")):
        results = benchmark_file(str(f), 'docx', output_base)
        structures = compare_results(results, f"DOCX: {f.stem}")
        all_results[f"docx:{f.stem}"] = structures

    # PDF tests
    pdf_dir = BENCH_DIR / "pdf_benchmark"
    for f in sorted(pdf_dir.glob("test_*.pdf")):
        results = benchmark_file(str(f), 'pdf', output_base)
        structures = compare_results(results, f"PDF: {f.stem}")
        all_results[f"pdf:{f.stem}"] = structures

    # HWP tests
    hwp_dir = PROJECT_ROOT / "samples" / "input"
    hwp_files = sorted(hwp_dir.glob("*.hwp"))[:5]  # First 5 for now
    for f in hwp_files:
        results = benchmark_file(str(f), 'hwp', output_base)
        structures = compare_results(results, f"HWP: {f.stem}")
        all_results[f"hwp:{f.stem}"] = structures

    # Summary
    print(f"\n{'='*70}")
    print(f"  OVERALL SUMMARY")
    print(f"{'='*70}")

    # Aggregate MDM structure scores
    mdm_totals = Counter()
    file_count = 0
    for key, structures in all_results.items():
        if 'MDM' in structures:
            for metric, val in structures['MDM'].items():
                mdm_totals[metric] += val
            file_count += 1

    if file_count > 0:
        print(f"\n  MDM aggregate across {file_count} files:")
        for metric in ['headings', 'bold', 'italic', 'bullet_lists', 'tables',
                       'links', 'footnotes', 'total_words']:
            print(f"    {metric}: {mdm_totals[metric]}")


if __name__ == "__main__":
    # Build MDM first
    print("Building MDM core...")
    subprocess.run(
        ["cargo", "build", "--manifest-path", str(PROJECT_ROOT / "core" / "Cargo.toml")],
        capture_output=True
    )

    run_full_benchmark()
