"""Cross-validate MDM output against external parsers, without relying on
ground truth. Surfaces content-level issues by finding tokens/lines that
external parsers agree on but MDM does not — strong signal that MDM may
be dropping real content (or hallucinating, if the pattern is inverted).

Usage:
    python cross_validate.py <fixture-stem>         # one fixture, detailed
    python cross_validate.py --all                  # every fixture, summary
"""
from __future__ import annotations

import argparse
import importlib
import tomllib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


@dataclass
class OutputSet:
    adapter: str
    text: str
    tokens: set[str]
    lines: set[str]
    dense: str = ""
    """Whole output, whitespace stripped and lowercased — see `_dense`."""


# Below this length a whitespace-free needle matches by coincidence.
_MIN_DENSE_LEN = 12


def _dense(s: str) -> str:
    """Collapse a string to comparable form: no whitespace, lowercased.

    Line and token equality both fall apart on Korean PDFs because parsers
    disagree about where spaces go inside a phrase — MDM writes `투표연령 하향
    (19 → 18 세)` where the external ones write `투표연령 하향(19→18세)`. Nothing
    is missing, yet every such line counts as a miss. Comparing whitespace-free
    substrings asks the question that actually matters: is this content present
    at all?
    """
    return "".join(s.split()).lower()


_MD_STRIP = ("# ", "## ", "### ", "#### ", "##### ", "###### ",
             "- ", "* ", "> ", "+ ")


def _strip_md(line: str) -> str:
    """Strip markdown structural prefixes so 'Foo' and '### Foo' compare equal.
    Also drops MDX frontmatter lines (key: value at the top of the doc)."""
    s = line.strip()
    # Bold/italic wrappers
    if s.startswith("**") and s.endswith("**"):
        s = s[2:-2]
    if s.startswith("*") and s.endswith("*") and len(s) > 2:
        s = s[1:-1]
    for prefix in _MD_STRIP:
        if s.startswith(prefix):
            s = s[len(prefix):].lstrip()
            break
    return s.strip()


def _is_frontmatter_line(line: str) -> bool:
    stripped = line.strip()
    if stripped in {"---", "```"}:
        return True
    # Simple "key: value" — treat as metadata, not content
    if ":" in stripped and not stripped.startswith("#"):
        left = stripped.split(":", 1)[0]
        if left and all(c.isalnum() or c == "_" for c in left) and len(left) < 20:
            return True
    return False


def _tokenize(text: str) -> tuple[set[str], set[str]]:
    # Skip frontmatter block at the top: everything between first pair of `---`
    lines = text.splitlines()
    if lines and lines[0].strip() == "---":
        try:
            end = lines.index("---", 1)
            lines = lines[end + 1:]
        except ValueError:
            pass
    cleaned = []
    for l in lines:
        if _is_frontmatter_line(l):
            continue
        s = _strip_md(l)
        if s and len(s) > 3:
            cleaned.append(s)
    tokens = set()
    for l in cleaned:
        for t in l.split():
            if len(t) > 1:
                tokens.add(t.lower().strip(".,;:!?()[]{}\"'"))
    line_set = {l.lower() for l in cleaned}
    return tokens, line_set


def run_all_adapters(pdf: Path, adapters: dict) -> list[OutputSet]:
    out = []
    for name, cfg in adapters.items():
        mod = importlib.import_module(cfg["module"])
        kwargs = {k: v for k, v in cfg.items() if k != "module"}
        res = mod.convert(pdf, **kwargs)
        toks, lines = _tokenize(res.markdown)
        out.append(OutputSet(adapter=name, text=res.markdown, tokens=toks,
                             lines=lines, dense=_dense(res.markdown)))
    return out


def analyze(outputs: list[OutputSet], primary: str = "mdm") -> dict:
    others = [o for o in outputs if o.adapter != primary]
    mdm = next((o for o in outputs if o.adapter == primary), None)
    if mdm is None or not others:
        return {}

    # Consensus lines — lines present in ALL external parsers
    consensus_lines = set.intersection(*(o.lines for o in others)) if others else set()
    consensus_tokens = set.intersection(*(o.tokens for o in others)) if others else set()

    # Lines external parsers agree on but MDM lacks → MDM may be dropping
    missing_from_mdm = consensus_lines - mdm.lines
    # Tokens in consensus but not in MDM
    missing_tokens = consensus_tokens - mdm.tokens
    # Lines only MDM has — possible hallucination (or MDM is better)
    mdm_only_lines = mdm.lines - set.union(*(o.lines for o in others))
    mdm_only_tokens = mdm.tokens - set.union(*(o.tokens for o in others))

    # The signal that survives whitespace disagreement: consensus content that
    # is nowhere in MDM's output, not merely spaced or line-broken differently.
    # Measured on changes_brochure, this cuts 32 "missing" lines down to the
    # handful that are genuinely absent.
    absent = sorted(
        (l for l in consensus_lines
         if len(_dense(l)) >= _MIN_DENSE_LEN and _dense(l) not in mdm.dense),
        key=len,
        reverse=True,
    )

    return {
        "primary_lines": len(mdm.lines),
        "primary_tokens": len(mdm.tokens),
        "consensus_lines": len(consensus_lines),
        "consensus_tokens": len(consensus_tokens),
        "missing_from_primary_lines": len(missing_from_mdm),
        "missing_from_primary_tokens": len(missing_tokens),
        "primary_only_lines": len(mdm_only_lines),
        "primary_only_tokens": len(mdm_only_tokens),
        "absent_from_primary": len(absent),
        # Sample a few for inspection
        "missing_samples": sorted(missing_from_mdm, key=len, reverse=True)[:5],
        "primary_only_samples": sorted(mdm_only_lines, key=len, reverse=True)[:5],
        "absent_samples": absent[:8],
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture", nargs="?")
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--config", default="config.toml")
    ap.add_argument("--config-local", default="config.local.toml")
    args = ap.parse_args()

    cfg = tomllib.loads(Path(args.config).read_text())
    if Path(args.config_local).exists():
        local = tomllib.loads(Path(args.config_local).read_text())
        for k, v in local.items():
            if isinstance(v, dict) and k in cfg:
                cfg[k].update(v)
            else:
                cfg[k] = v
    adapters = cfg["adapters"]
    fx_root = Path(cfg["run"]["fixtures_root"])

    fixtures = {p.stem: p for p in fx_root.rglob("*.pdf")}

    if args.all:
        # `absent` is the column to read. `miss` counts line-equality failures,
        # which whitespace differences dominate on Korean text.
        print(f"{'fixture':<28} {'lines':>8} {'miss':>6} {'only':>6} {'absent':>7}")
        totals = 0
        for stem, pdf in sorted(fixtures.items()):
            outs = run_all_adapters(pdf, adapters)
            a = analyze(outs)
            if not a:
                continue
            totals += a["absent_from_primary"]
            print(f"{stem:<28} {a['primary_lines']:>8} "
                  f"{a['missing_from_primary_lines']:>6} {a['primary_only_lines']:>6} "
                  f"{a['absent_from_primary']:>7}")
        print(f"\ncontent absent from MDM across all fixtures: {totals}")
        return 0

    if not args.fixture:
        print("specify a fixture stem or use --all", file=__import__("sys").stderr)
        return 1

    pdf = fixtures.get(args.fixture)
    if pdf is None:
        print(f"fixture {args.fixture} not found", file=__import__("sys").stderr)
        return 1

    outs = run_all_adapters(pdf, adapters)
    a = analyze(outs)
    print(f"# Cross-validation: {args.fixture}\n")
    print(f"MDM lines: {a['primary_lines']}, tokens: {a['primary_tokens']}")
    print(f"External consensus lines: {a['consensus_lines']}, tokens: {a['consensus_tokens']}")
    print(f"\nAbsent from MDM (consensus content nowhere in the output, "
          f"whitespace ignored): {a['absent_from_primary']}")
    for s in a["absent_samples"]:
        print(f"    ! {s[:120]}")
    print(f"\nMissing from MDM by line equality (whitespace-sensitive — mostly "
          f"formatting, read `absent` above instead):")
    print(f"  lines: {a['missing_from_primary_lines']}, tokens: {a['missing_from_primary_tokens']}")
    for s in a["missing_samples"]:
        print(f"    - {s[:120]}")
    print(f"\nMDM-only (possibly hallucinated or MDM is better):")
    print(f"  lines: {a['primary_only_lines']}, tokens: {a['primary_only_tokens']}")
    for s in a["primary_only_samples"]:
        print(f"    - {s[:120]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
