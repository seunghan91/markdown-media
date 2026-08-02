"""Identify fixtures where MDM and external parsers diverge the most.

Output: a prioritized review queue (`review_queue.md`) listing fixtures
where adapters disagree on any metric by more than a threshold. These are
the cases worth hand-curating — ground truth seeded from MDM is unreliable
exactly in these spots.

Usage:
    python find_disagreements.py results/<run-id>/matrix.json [--threshold 0.3]
    python find_disagreements.py --latest
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path


def find_latest_run() -> Path:
    runs = sorted(Path("results").glob("*/matrix.json"))
    if not runs:
        raise SystemExit("no runs in results/ — run bench first")
    return runs[-1]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("matrix", nargs="?", type=Path)
    ap.add_argument("--latest", action="store_true")
    ap.add_argument("--threshold", type=float, default=0.3,
                    help="Max inter-adapter metric spread considered agreement")
    ap.add_argument("--reference", default="mdm",
                    help="Adapter whose output we currently trust as the GT seed")
    args = ap.parse_args()

    matrix_path = find_latest_run() if args.latest or not args.matrix else args.matrix
    data = json.loads(matrix_path.read_text())

    # Pivot: fixture → {adapter → metrics}
    by_fixture: dict[str, dict[str, dict[str, float]]] = {}
    for r in data:
        fx = Path(r["fixture"]).stem
        by_fixture.setdefault(fx, {})[r["adapter"]] = r["metrics"]

    # Score each fixture by "max spread across adapters on metrics that
    # measure similarity to the GT" — BLEU, ROUGE-L, edit_ratio, TSED are
    # higher=better; CER is lower=better, so invert for uniform direction.
    similarity_metrics = ["bleu", "rouge_l", "edit_ratio", "tsed"]
    disagreements = []
    for fx, adapters in by_fixture.items():
        if len(adapters) < 2:
            continue
        spreads = {}
        for m in similarity_metrics:
            values = [a.get(m) for a in adapters.values() if isinstance(a.get(m), (int, float))]
            if len(values) >= 2:
                spreads[m] = max(values) - min(values)
        if not spreads:
            continue
        max_spread = max(spreads.values())
        disagreements.append({
            "fixture": fx,
            "max_spread": max_spread,
            "spreads": spreads,
            "adapters": {a: {m: adapters[a].get(m) for m in similarity_metrics}
                         for a in adapters},
        })

    disagreements.sort(key=lambda d: d["max_spread"], reverse=True)
    flagged = [d for d in disagreements if d["max_spread"] >= args.threshold]

    out = Path(matrix_path.parent) / "review_queue.md"
    lines = [
        "# Review Queue — Adapter Disagreements",
        f"",
        f"Threshold: max metric spread ≥ {args.threshold}",
        f"Reference adapter: `{args.reference}` (current GT seed)",
        f"Flagged: {len(flagged)}/{len(disagreements)} fixtures",
        "",
        "Rank by max_spread — review top items first. When adapters disagree, the",
        "GT seeded from the reference adapter may be wrong; inspect the source PDF",
        "and choose a correct Markdown to replace `ground_truth/<stem>/document.md`.",
        "",
    ]
    for i, d in enumerate(flagged, 1):
        lines.append(f"## {i}. `{d['fixture']}` — spread {d['max_spread']:.3f}")
        lines.append("")
        lines.append("| adapter | bleu | rouge_l | edit_ratio | tsed |")
        lines.append("|---|---|---|---|---|")
        for a, ms in d["adapters"].items():
            def fmt(v): return f"{v:.3f}" if isinstance(v, (int, float)) else "-"
            lines.append(f"| {a} | {fmt(ms['bleu'])} | {fmt(ms['rouge_l'])} "
                         f"| {fmt(ms['edit_ratio'])} | {fmt(ms['tsed'])} |")
        lines.append("")
        worst_metric = max(d["spreads"], key=d["spreads"].get)
        lines.append(f"Worst-spread metric: **{worst_metric}** ({d['spreads'][worst_metric]:.3f})")
        lines.append("")

    if not flagged:
        lines.append("_No disagreements above threshold. Golden set may be reliable "
                     "— or adapters may agree on both being wrong._")

    out.write_text("\n".join(lines))
    print(f"flagged {len(flagged)} fixtures → {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
