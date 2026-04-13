#!/usr/bin/env python3
"""Compare MDM vs Pandoc output quality feature by feature."""
import os, re

BENCH_DIR = os.path.dirname(os.path.abspath(__file__))

def read_file(path):
    try:
        with open(path) as f:
            return f.read()
    except:
        return ""

def check_feature(text, feature_name, pattern, is_regex=False):
    if is_regex:
        found = bool(re.search(pattern, text, re.MULTILINE))
    else:
        found = pattern in text
    return found

def compare(test_name):
    pandoc = read_file(f"{BENCH_DIR}/output/pandoc/{test_name}.md")
    mdm_dir = f"{BENCH_DIR}/output/mdm/{test_name}"
    mdm = ""
    for f in os.listdir(mdm_dir) if os.path.isdir(mdm_dir) else []:
        if f.endswith(".mdx"):
            mdm = read_file(os.path.join(mdm_dir, f))
            break

    print(f"\n{'='*60}")
    print(f"  {test_name}")
    print(f"{'='*60}")
    print(f"  {'Feature':<35} {'Pandoc':>8} {'MDM':>8}")
    print(f"  {'-'*35} {'-'*8} {'-'*8}")

    checks = []

    if test_name == "test_comprehensive":
        checks = [
            ("H1 heading", "# Document Title (H1)"),
            ("H2 heading", "## Section One (H2)"),
            ("H3 heading", "### Subsection 1.1 (H3)"),
            ("H4 heading", "#### Sub-subsection 1.1.1 (H4)"),
            ("Bold", "**Bold text**"),
            ("Italic", "*italic text*"),
            ("Bold+Italic", "***bold italic***"),
            ("Strikethrough", "~~strikethrough~~"),
            ("Bullet list marker", r"^- Bullet item 1", True),
            ("Numbered list marker", r"^\d+\.\s+Numbered item 1", True),
            ("Nested list indent", r"  +- Nested bullet", True),
            ("Table pipe format", "| Name |"),
            ("Table separator", "| --- |"),
            ("Table data row", "| Alice |"),
            ("Merged cell table", "Merged A+B"),
            ("Hyperlink [text](url)", r"\[MDM GitHub Repository\]\(https://", True),
            ("Space before hyperlink", r"Visit \[MDM", True),
            ("Footnote reference [^1]", "[^1]"),
            ("Korean bold", "**굵은 한글**"),
            ("Korean italic", "*기울임 한글*"),
            ("Korean bold italic", "***굵은 기울임 한글***"),
            ("Blockquote >", "> This is a quote"),
            ("Mixed bold spacing", r"The \*\*ActionCable\*\*", True),
            ("Spacing around formatting", r",\s?\*italic text\*", True),
        ]
    elif test_name == "test_korean_gov":
        checks = [
            ("Title heading", "# 대한민국 정부 공문서"),
            ("Article heading", "## 제1조 (목적)"),
            ("Korean body text", "MDM 파서의 한국어"),
            ("Numbered definitions", '"파서"란'),
            ("Table header bold", "형식"),
            ("Table Korean data", "한컴"),
            ("Emoji in table", "✅"),
            ("Appendix heading", "## 부칙"),
        ]
    elif test_name == "test_tables":
        checks = [
            ("Simple table", "| Header A |"),
            ("Wide table 7 cols", "| Col 1 |"),
            ("Formatted cell bold", "Bold header"),
            ("Pipe escape in cell", r"pipe \\?\|", True),
            ("Multi-paragraph cell", "First paragraph"),
            ("Complex merge", "Group A"),
            ("Vertical merge", "Vert Merged"),
        ]

    score_p = 0
    score_m = 0
    total = len(checks)

    for name, pattern, *opts in checks:
        is_regex = opts[0] if opts else False
        p = check_feature(pandoc, name, pattern, is_regex)
        m = check_feature(mdm, name, pattern, is_regex)
        score_p += p
        score_m += m
        p_sym = "✅" if p else "❌"
        m_sym = "✅" if m else "❌"
        print(f"  {name:<35} {p_sym:>8} {m_sym:>8}")

    print(f"  {'-'*35} {'-'*8} {'-'*8}")
    print(f"  {'SCORE':<35} {score_p:>5}/{total}  {score_m:>5}/{total}")
    pct_p = score_p/total*100 if total else 0
    pct_m = score_m/total*100 if total else 0
    print(f"  {'PERCENTAGE':<35} {pct_p:>7.0f}%  {pct_m:>7.0f}%")

    return score_p, score_m, total


if __name__ == "__main__":
    tp, tm, tt = 0, 0, 0
    for test in ["test_comprehensive", "test_korean_gov", "test_tables"]:
        p, m, t = compare(test)
        tp += p; tm += m; tt += t

    print(f"\n{'='*60}")
    print(f"  OVERALL SCORE")
    print(f"{'='*60}")
    print(f"  Pandoc:  {tp}/{tt} ({tp/tt*100:.0f}%)")
    print(f"  MDM:     {tm}/{tt} ({tm/tt*100:.0f}%)")
    gap = tp - tm
    if gap > 0:
        print(f"\n  Gap: MDM is {gap} features behind Pandoc")
    elif gap < 0:
        print(f"\n  MDM LEADS Pandoc by {-gap} features!")
    else:
        print(f"\n  PARITY achieved!")
