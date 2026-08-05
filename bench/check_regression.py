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
import math
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


def unmeasured(matrix: list[dict], adapter: str) -> list[str]:
    """Rows that hold no verdict-bearing number.

    Three shapes, all of which the gate used to read as passing:

    - A failed conversion. Empty output against a reference still scores —
      BLEU 0, CER 1 — so the numbers look like a very bad document rather than
      like no document.
    - A NaN metric. NaN loses every comparison, so `regressed()` reads it as
      "no drop". `metrics.py` returns NaN whenever a scoring library is
      missing, which is one bad import away at any time.
    A fixture with no `ground_truth/<stem>/document.md` is not one of these:
    the runner writes empty metrics for it on purpose. It converted, it just
    has nothing to be scored against, and it is told apart here by having
    exited cleanly with no error. The case this cannot see — a clean conversion
    that had a reference and still scored nothing — is the runner's to catch,
    since only the runner knows which fixtures have references.
    """
    out = []
    for row in matrix:
        if row["adapter"] != adapter:
            continue
        if row.get("exit_code", 0) != 0 or row.get("error"):
            detail = row.get("error") or f"exit {row['exit_code']}"
            out.append(f"{row['fixture']}: conversion failed ({detail})")
            continue
        values = [v for k, v in row["metrics"].items()
                  if not k.endswith("_error") and isinstance(v, (int, float))]
        if not values:
            continue  # no reference for this fixture — nothing to gate
        for k, v in row["metrics"].items():
            if k.endswith("_error"):
                continue
            if isinstance(v, float) and not math.isfinite(v):
                out.append(f"{row['fixture']}: {k}")
    return out


def compare_groups(
    label: str,
    current: dict[str, dict[str, float]],
    baseline: dict[str, dict[str, float]],
    tolerance: float,
    failures: list[str],
    blocked: list[str],
) -> None:
    """Gate each group separately so a strong group cannot mask a weak one."""
    if not baseline:
        return
    print(f"\n{label} (tolerance {tolerance})")
    for group in sorted(set(current) & set(baseline)):
        cur, ref = current[group], baseline[group]
        shared = sorted(set(cur) & set(ref))
        absent = sorted(set(ref) - set(cur))
        if absent:
            # A metric the baseline gates and the run does not carry drops out
            # of the intersection without a word. Partial coverage reads
            # exactly like full coverage once the missing column is gone, so
            # the group is blocked rather than judged on what survived.
            what = "no metric in common" if not shared else f"missing {', '.join(absent)}"
            print(f"  {group:<34} BLOCKED  {what}")
            blocked.append(f"{label} [{group}]: {what} — the baseline gates it, this run does not measure it")
            continue
        drops = []
        incomparable = []
        for m in shared:
            delta = cur[m] - ref[m]
            if not math.isfinite(delta):
                incomparable.append(m)
                blocked.append(
                    f"{label} [{group}]: {m} has no finite value on one side "
                    f"(run {cur[m]}, baseline {ref[m]})"
                )
                continue
            if regressed(m, delta, tolerance):
                drops.append(f"{m} {cur[m]:.3f} vs {ref[m]:.3f} (Δ={delta:+.3f})")
        if incomparable:
            print(f"  {group:<34} BLOCKED  not comparable: {', '.join(incomparable)}")
            continue
        if drops:
            for d in drops:
                failures.append(f"{label} [{group}]: {d}")
            print(f"  {group:<34} FAIL  {'; '.join(drops)}")
        else:
            print(f"  {group:<34} ok")
    missing = sorted(set(baseline) - set(current))
    if missing:
        print(f"  (not in this run: {', '.join(missing)})")
        # A baseline group with no counterpart is coverage the gate silently
        # lost, not a group that passed. It happens for a mundane reason:
        # `fixture` is whatever path the run recorded, so pointing
        # `fixtures_root` at an absolute path renames every key and this whole
        # level compares nothing while still printing "all gates passed".
        blocked.append(
            f"{label}: {len(missing)} baseline group(s) absent from this run "
            f"({', '.join(missing[:3])}{', …' if len(missing) > 3 else ''}). "
            f"Either the run did not cover them or the fixture keys do not "
            f"match the baseline's — re-run over the same fixture roots, or "
            f"re-record the baseline if the set really changed."
        )
    added = sorted(set(current) - set(baseline))
    if added:
        print(f"  (no baseline: {', '.join(added)})")
        # The other direction, and the same hole: a group the baseline has never
        # seen is dropped from the intersection, so this level never judges it.
        # Only the overall mean covers it, and that is the level built to hide
        # single fixtures. Adding a fixture means re-recording the baseline —
        # that is the documented workflow, not an extra cost imposed here.
        blocked.append(
            f"{label}: {len(added)} group(s) have no baseline entry "
            f"({', '.join(added[:3])}{', …' if len(added) > 3 else ''}) and are "
            f"gated by nothing at this level. Re-record the baseline to include "
            f"them."
        )


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
    """Mean of the last `window` runs, excluding the current one.

    A run that failed to measure is skipped whole, not averaged in. `results/`
    keeps every run including the broken ones, and a broken run does not
    announce itself in the mean: a failed conversion scores a finite BLEU 0,
    and a NaN turns the whole reference NaN, which then passes every
    comparison. Either way the reference stops describing a working parser.
    """
    runs = sorted(Path("results").glob("*/matrix.json"))
    if len(runs) < 2:
        return None
    history: dict[str, list[float]] = {}
    for r in runs[-(window + 1):-1]:  # exclude the current run
        try:
            matrix = load_matrix(r)
        except Exception:
            continue
        if unmeasured(matrix, adapter):
            continue
        agg = aggregate(matrix, adapter)
        if not agg or any(not math.isfinite(v) for v in agg.values()):
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
        return 2
    latest = load_matrix(runs[-1])
    current = aggregate(latest, args.adapter)

    # Everything that stops a verdict from being possible. Kept apart from
    # `failures`: a blocked gate is not a passing gate and not a regression
    # either, and reporting it as either one is how a broken measurement gets
    # mistaken for a green run.
    blocked: list[str] = []
    if not current:
        blocked.append(
            f"adapter {args.adapter} produced no measurable results — the run "
            f"scored nothing. Check the adapter's binary path before reading "
            f"anything into this."
        )
    gaps = unmeasured(latest, args.adapter)
    if gaps:
        shown = ", ".join(gaps[:4])
        blocked.append(
            f"{len(gaps)} result(s) in this run hold no measurement "
            f"({shown}{', …' if len(gaps) > 4 else ''}). Each line says which "
            f"kind: a failed conversion is the adapter's, a NaN metric is a "
            f"scoring dependency — see bench/README.md."
        )

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

    # A reference value has to be a number for the comparison against it to
    # mean anything. Same rule as the current run, applied to whichever side
    # the baseline came from.
    if not base:
        blocked.append(
            f"no baseline to compare against ({baseline_source}). With nothing "
            f"on the other side every comparison is skipped, which is not a "
            f"pass — pin one with --baseline, or build history by running the "
            f"bench more than once."
        )
    base_nans = sorted(m for m, v in base.items()
                       if isinstance(v, float) and not math.isfinite(v))
    if base_nans:
        blocked.append(
            f"baseline holds no usable value for {', '.join(base_nans)} "
            f"({baseline_source}). Nothing can be compared against it."
        )
    if scale_mismatch:
        blocked.append(scale_mismatch)
    if blocked:
        # Before the table, not after: none of these leave a comparison worth
        # printing.
        print(f"Adapter: {args.adapter}")
        print(f"Baseline: {baseline_source}")
        print("\nGATE BLOCKED — no verdict is possible, and none was reached:",
              file=sys.stderr)
        for b in blocked:
            print(f"  - {b}", file=sys.stderr)
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

    # Groups that scored nothing are dropped rather than compared. By this
    # point `unmeasured` has already blocked every failure, so what is left is
    # a fixture with no reference — it cannot be gated and cannot be baselined
    # either, so treating it as missing coverage would block the gate for good.
    def scored(groups: dict[str, dict[str, float]]) -> dict[str, dict[str, float]]:
        return {g: m for g, m in groups.items() if m}

    unreferenced = sorted(
        row["fixture"] for row in latest
        if row["adapter"] == args.adapter and not [
            v for k, v in row["metrics"].items()
            if not k.endswith("_error") and isinstance(v, (int, float))
        ]
    )
    if unreferenced:
        print(f"\nnot gated — no reference document: {', '.join(unreferenced)}")

    compare_groups(
        "per-slice",
        scored(aggregate_by(latest, args.adapter, lambda r: slice_of(r["fixture"]))),
        base_slices,
        slice_drop_max,
        failures,
        blocked,
    )
    compare_groups(
        "per-fixture",
        scored(aggregate_by(latest, args.adapter, lambda r: r["fixture"])),
        base_fixtures,
        fixture_drop_max,
        failures,
        blocked,
    )

    if args.strict:
        bleu = current.get("bleu")
        cer = current.get("cer")
        if bleu is not None and bleu < bleu_min:
            failures.append(f"absolute: bleu {bleu:.3f} < {bleu_min}")
        if cer is not None and cer > cer_max:
            failures.append(f"absolute: cer {cer:.3f} > {cer_max}")

    if blocked:
        # Coverage loss surfaces here, after the group levels have run. It
        # outranks `failures`: a level that compared nothing cannot certify the
        # levels that did.
        print("\nGATE BLOCKED — a comparison level covered nothing:", file=sys.stderr)
        for b in blocked:
            print(f"  - {b}", file=sys.stderr)
        return 2
    if failures:
        print("\nREGRESSION GATE FAILED:", file=sys.stderr)
        for f in failures:
            print(f"  - {f}", file=sys.stderr)
        return 1
    print("\nall gates passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
