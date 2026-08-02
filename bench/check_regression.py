"""Regression gate for CI.

Compares the latest bench run against a baseline (rolling average of the
last N runs OR a pinned baseline-metrics.json on main). Fails CI when any
metric drops by more than the configured tolerance.

Usage:
    python check_regression.py --adapter mdm
    python check_regression.py --adapter mdm --baseline results/baseline.json
    python check_regression.py --adapter mdm --strict     # absolute thresholds too
"""
from __future__ import annotations

import argparse
import json
import sys
import tomllib
from pathlib import Path


HIGHER_IS_BETTER = {"bleu", "rouge_l", "edit_ratio", "tsed"}
LOWER_IS_BETTER = {"cer"}


def load_matrix(path: Path) -> list[dict]:
    return json.loads(path.read_text())


def slice_of(fixture: str) -> str:
    """Fixture family, taken from its directory under `fixtures/`."""
    parts = Path(fixture).parts
    if len(parts) >= 3 and parts[0] == "fixtures":
        return parts[1]
    return parts[-2] if len(parts) >= 2 else "other"


def aggregate_by(matrix: list[dict], adapter: str, key) -> dict[str, dict[str, float]]:
    """Return {group → {metric → mean}}, grouping rows with `key(row)`."""
    buckets: dict[str, dict[str, list[float]]] = {}
    for row in matrix:
        if row["adapter"] != adapter:
            continue
        group = buckets.setdefault(key(row), {})
        for k, v in row["metrics"].items():
            if k.endswith("_error") or not isinstance(v, (int, float)):
                continue
            group.setdefault(k, []).append(float(v))
    return {
        g: {m: sum(vs) / len(vs) for m, vs in ms.items() if vs}
        for g, ms in buckets.items()
    }


def regressed(metric: str, delta: float, tolerance: float) -> bool:
    if metric in HIGHER_IS_BETTER:
        return delta < -tolerance
    if metric in LOWER_IS_BETTER:
        return delta > tolerance
    return False


def compare_groups(
    label: str,
    current: dict[str, dict[str, float]],
    baseline: dict[str, dict[str, float]],
    tolerance: float,
    failures: list[str],
) -> None:
    """Gate each group separately so a strong group cannot mask a weak one."""
    if not baseline:
        return
    print(f"\n{label} (tolerance {tolerance})")
    for group in sorted(set(current) & set(baseline)):
        cur, ref = current[group], baseline[group]
        drops = []
        for m in sorted(set(cur) & set(ref)):
            delta = cur[m] - ref[m]
            if regressed(m, delta, tolerance):
                drops.append(f"{m} {cur[m]:.3f} vs {ref[m]:.3f} (Δ={delta:+.3f})")
        if drops:
            for d in drops:
                failures.append(f"{label} [{group}]: {d}")
            print(f"  {group:<34} FAIL  {'; '.join(drops)}")
        else:
            print(f"  {group:<34} ok")
    missing = sorted(set(baseline) - set(current))
    if missing:
        print(f"  (not in this run: {', '.join(missing)})")


def aggregate(matrix: list[dict], adapter: str) -> dict[str, float]:
    """Return {metric → mean} for the given adapter across all fixtures."""
    buckets: dict[str, list[float]] = {}
    for row in matrix:
        if row["adapter"] != adapter:
            continue
        for k, v in row["metrics"].items():
            if k.endswith("_error") or not isinstance(v, (int, float)):
                continue
            buckets.setdefault(k, []).append(float(v))
    return {m: sum(vs) / len(vs) for m, vs in buckets.items() if vs}


def rolling_baseline(adapter: str, window: int) -> dict[str, float] | None:
    runs = sorted(Path("results").glob("*/matrix.json"))
    if len(runs) < 2:
        return None
    history: dict[str, list[float]] = {}
    for r in runs[-(window + 1):-1]:  # exclude the current run
        try:
            agg = aggregate(load_matrix(r), adapter)
        except Exception:
            continue
        for m, v in agg.items():
            history.setdefault(m, []).append(v)
    if not history:
        return None
    return {m: sum(vs) / len(vs) for m, vs in history.items()}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--adapter", default="mdm")
    ap.add_argument("--baseline", type=Path,
                    help="Pinned baseline JSON (overrides rolling average)")
    ap.add_argument("--config", default="config.toml")
    ap.add_argument("--strict", action="store_true",
                    help="Also enforce absolute bleu_min / cer_max gates")
    args = ap.parse_args()

    cfg = tomllib.loads(Path(args.config).read_text())
    gate = cfg.get("gate", {})
    relative_drop_max = gate.get("relative_drop_max", 0.05)
    window = gate.get("rolling_window", 10)
    bleu_min = gate.get("bleu_min", 0.80)
    cer_max = gate.get("cer_max", 0.10)
    slice_drop_max = gate.get("slice_drop_max", relative_drop_max)
    fixture_drop_max = gate.get("fixture_drop_max", relative_drop_max * 2)

    runs = sorted(Path("results").glob("*/matrix.json"))
    if not runs:
        print("no runs in results/ — run bench first", file=sys.stderr)
        return 1
    latest = load_matrix(runs[-1])
    current = aggregate(latest, args.adapter)
    if not current:
        print(f"adapter {args.adapter} produced no measurable results", file=sys.stderr)
        return 1

    base_slices: dict[str, dict[str, float]] = {}
    base_fixtures: dict[str, dict[str, float]] = {}
    scale_mismatch: str | None = None

    if args.baseline and args.baseline.exists():
        raw = json.loads(args.baseline.read_text())
        baseline_source = f"pinned {args.baseline}"
        if "overall" in raw:
            # Baseline recorded with provenance: overall + per-slice + per-fixture.
            base = raw["overall"]
            base_slices = {k: {m: v for m, v in d.items() if m != "n"}
                           for k, d in raw.get("by_slice", {}).items()}
            base_fixtures = raw.get("per_fixture", {})
            pinned_sha = raw.get("metrics_py_sha256_16")
            if pinned_sha:
                import hashlib
                actual = hashlib.sha256(Path("metrics.py").read_bytes()).hexdigest()[:16]
                if actual != pinned_sha:
                    scale_mismatch = (
                        f"metrics.py changed since this baseline was recorded "
                        f"({pinned_sha} → {actual}). Scores are measured on a "
                        f"different scale — re-record the baseline before gating."
                    )
        else:
            # Legacy flat {metric: value}: no provenance, so no scale check.
            base = raw
    else:
        base = rolling_baseline(args.adapter, window) or {}
        baseline_source = f"rolling avg of last {window} runs"

    failures: list[str] = []

    if scale_mismatch:
        print(f"Adapter: {args.adapter}")
        print(f"Baseline: {baseline_source}")
        print(f"\nGATE BLOCKED — not a regression, a scale change:\n  {scale_mismatch}",
              file=sys.stderr)
        return 2
    print(f"Adapter: {args.adapter}")
    print(f"Baseline: {baseline_source}")
    print(f"{'metric':<14} {'current':>10} {'baseline':>10} {'Δ':>10}   gate")
    for m in sorted(set(current) | set(base)):
        cur = current.get(m)
        ref = base.get(m)
        delta = (cur - ref) if (cur is not None and ref is not None) else None
        gate_result = "skip"
        if ref is not None and cur is not None:
            if m in HIGHER_IS_BETTER and delta < -relative_drop_max:
                failures.append(f"{m}: {cur:.3f} vs {ref:.3f} (Δ={delta:+.3f})")
                gate_result = "FAIL"
            elif m in LOWER_IS_BETTER and delta > relative_drop_max:
                failures.append(f"{m}: {cur:.3f} vs {ref:.3f} (Δ={delta:+.3f})")
                gate_result = "FAIL"
            else:
                gate_result = "ok"
        cur_str = f"{cur:.3f}" if cur is not None else "-"
        ref_str = f"{ref:.3f}" if ref is not None else "-"
        delta_str = f"{delta:+.3f}" if delta is not None else "-"
        print(f"{m:<14} {cur_str:>10} {ref_str:>10} {delta_str:>10}   {gate_result}")

    compare_groups(
        "per-slice",
        aggregate_by(latest, args.adapter, lambda r: slice_of(r["fixture"])),
        base_slices,
        slice_drop_max,
        failures,
    )
    compare_groups(
        "per-fixture",
        aggregate_by(latest, args.adapter, lambda r: r["fixture"]),
        base_fixtures,
        fixture_drop_max,
        failures,
    )

    if args.strict:
        bleu = current.get("bleu")
        cer = current.get("cer")
        if bleu is not None and bleu < bleu_min:
            failures.append(f"absolute: bleu {bleu:.3f} < {bleu_min}")
        if cer is not None and cer > cer_max:
            failures.append(f"absolute: cer {cer:.3f} > {cer_max}")

    if failures:
        print("\nREGRESSION GATE FAILED:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    print("\nall gates passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
