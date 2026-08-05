"""Bench runner — matrix over (adapter × fixture), emits per-sample metrics.

Usage:
    python runner.py --config config.toml
    python runner.py --config config.toml --config-local config.local.toml
"""
from __future__ import annotations

import argparse
import importlib
import json
import math
import shutil
import sys
import tomllib
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path

from metrics import score_all


@dataclass
class SampleResult:
    adapter: str
    fixture: str
    category: str
    elapsed_ms: float
    exit_code: int
    metrics: dict
    error: str | None = None


def discover_fixtures(root: Path) -> list[tuple[str, Path]]:
    """Return [(category, pdf_path)] for every PDF under fixtures/."""
    out = []
    for cat_dir in sorted(root.iterdir()):
        if not cat_dir.is_dir():
            continue
        for pdf in sorted(cat_dir.rglob("*.pdf")):
            out.append((cat_dir.name, pdf))
    return out


def load_ground_truth(gt_root: Path, pdf_path: Path) -> str | None:
    gt = gt_root / pdf_path.stem / "document.md"
    return gt.read_text(encoding="utf-8") if gt.exists() else None


def missing_binaries(adapters: dict) -> list[str]:
    """Adapter binaries that are not there.

    A binary that does not exist is not a slow run or a weak score — every
    fixture comes back empty, every metric prints `-`, and the run still exits
    0. The shipped `config.toml` points at `../core/target/release/hwp2mdm`,
    which is wrong on any machine that sets `CARGO_TARGET_DIR` elsewhere, so
    this is the default experience rather than an edge case.
    """
    out = []
    for name, cfg in adapters.items():
        binary = cfg.get("binary")
        if not binary:
            continue
        if not Path(binary).exists() and shutil.which(binary) is None:
            out.append(f"{name}: {binary}")
    return out


def run_one(adapter_name: str, adapter_cfg: dict, pdf: Path, category: str,
            gt: str | None, metric_names: list[str]) -> SampleResult:
    mod = importlib.import_module(adapter_cfg["module"])
    try:
        ar = mod.convert(pdf, **{k: v for k, v in adapter_cfg.items() if k != "module"})
        metrics = score_all(ar.markdown, gt, metric_names) if gt else {}
        return SampleResult(
            adapter=adapter_name,
            fixture=str(pdf),
            category=category,
            elapsed_ms=ar.elapsed_ms,
            exit_code=ar.exit_code,
            metrics=metrics,
        )
    except Exception as e:
        return SampleResult(
            adapter=adapter_name, fixture=str(pdf), category=category,
            elapsed_ms=0.0, exit_code=-1, metrics={}, error=str(e),
        )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", default="config.toml")
    ap.add_argument("--config-local", default=None,
                    help="Overrides merged on top of --config (for local adapter configs)")
    args = ap.parse_args()

    cfg = tomllib.loads(Path(args.config).read_text())
    # Adapters the shipped config declares are the ones the gate judges.
    # Anything `--config-local` adds is a comparison parser a developer
    # installed locally, so its absence is their business, not a failed run.
    gated_adapters = set(cfg.get("adapters", {}))
    if args.config_local and Path(args.config_local).exists():
        local = tomllib.loads(Path(args.config_local).read_text())
        for k, v in local.items():
            if isinstance(v, dict) and k in cfg:
                cfg[k].update(v)
            else:
                cfg[k] = v

    fx_root = Path(cfg["run"]["fixtures_root"])
    gt_root = Path(cfg["run"]["ground_truth_root"])
    adapters = cfg.get("adapters", {})
    metric_names = cfg.get("metrics", {}).get("enabled", [])

    # Checked before a results directory exists — an empty run left on disk
    # becomes the "latest" one the gate reads.
    if not metric_names:
        print("[metrics].enabled is empty — the run would score nothing.",
              file=sys.stderr)
        return 2

    absent = missing_binaries({k: v for k, v in adapters.items() if k in gated_adapters})
    if absent:
        print("adapter binary not found — nothing would be measured:", file=sys.stderr)
        for a in absent:
            print(f"  {a}", file=sys.stderr)
        print("Set it in config.local.toml ([adapters.<name>] binary = ...); "
              "cargo puts the build wherever CARGO_TARGET_DIR points.", file=sys.stderr)
        return 2
    for a in missing_binaries({k: v for k, v in adapters.items() if k not in gated_adapters}):
        print(f"note: local adapter binary not found, it will score nothing — {a}",
              file=sys.stderr)

    out_root = Path(cfg["run"]["results_root"]) / datetime.now().strftime("%Y-%m-%d_%H%M%S")
    out_root.mkdir(parents=True, exist_ok=True)

    fixtures = discover_fixtures(fx_root)

    jobs = []
    with ThreadPoolExecutor(max_workers=cfg["run"]["parallel_workers"]) as ex:
        for adapter_name, adapter_cfg in adapters.items():
            for category, pdf in fixtures:
                gt = load_ground_truth(gt_root, pdf)
                jobs.append(ex.submit(run_one, adapter_name, adapter_cfg,
                                      pdf, category, gt, metric_names))

        results = [j.result() for j in as_completed(jobs)]

    matrix_path = out_root / "matrix.json"
    matrix_path.write_text(json.dumps([asdict(r) for r in results], indent=2, ensure_ascii=False))

    # Per-adapter × per-metric scoreboard
    scoreboard: dict[str, dict[str, list[float]]] = {}
    for r in results:
        bucket = scoreboard.setdefault(r.adapter, {})
        for k, v in r.metrics.items():
            if k.endswith("_error") or not isinstance(v, (int, float)):
                continue
            bucket.setdefault(k, []).append(float(v))

    lines = ["# Bench Results", ""]
    lines.append(f"Samples: {len(results)}    Adapters: {', '.join(scoreboard)}")
    lines.append("")
    lines.append("| adapter | " + " | ".join(metric_names) + " |")
    lines.append("|" + "---|" * (len(metric_names) + 1))
    for adapter, metrics in scoreboard.items():
        row = [adapter]
        for m in metric_names:
            vals = metrics.get(m, [])
            row.append(f"{sum(vals) / len(vals):.3f}" if vals else "-")
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")

    # Per-fixture breakdown — useful for spotting weak cases
    lines.append("## Per-fixture scores")
    lines.append("")
    lines.append("| fixture | adapter | " + " | ".join(metric_names) + " | ms |")
    lines.append("|" + "---|" * (len(metric_names) + 3))
    for r in sorted(results, key=lambda x: (x.fixture, x.adapter)):
        row = [Path(r.fixture).stem, r.adapter]
        for m in metric_names:
            v = r.metrics.get(m)
            row.append(f"{v:.3f}" if isinstance(v, (int, float)) else "-")
        row.append(f"{r.elapsed_ms:.0f}")
        lines.append("| " + " | ".join(row) + " |")

    (out_root / "scoreboard.md").write_text("\n".join(lines))
    print(f"{len(results)} samples → {matrix_path}")
    print(f"scoreboard → {out_root / 'scoreboard.md'}")

    # The results are still written — they are what you read to find out why —
    # but the exit code has to say when nothing was measured, because the
    # scoreboard says it with a dash and says it next to real numbers.
    def unmeasured(r: SampleResult) -> bool:
        if r.adapter not in gated_adapters:
            return False
        if r.exit_code != 0 or r.error:
            # Checked before the reference, because a conversion that failed is
            # a broken build or a broken PDF either way. With a reference it
            # would score anyway — empty output is a finite BLEU 0 and CER 1,
            # which reads as a very bad document rather than as no document.
            return True
        if load_ground_truth(gt_root, Path(r.fixture)) is None:
            return False
        # Every enabled metric, not merely some number: a metric that is absent
        # drops out of the gate's comparison silently, since the gate can only
        # compare what both sides have.
        return any(
            not isinstance(r.metrics.get(m), (int, float))
            or not math.isfinite(float(r.metrics[m]))
            for m in metric_names
        )

    if not results:
        print("\nno samples — fixtures_root held no PDF, or no adapter was "
              "configured. Nothing was measured.", file=sys.stderr)
        return 2

    unscored = [r for r in results if unmeasured(r)]
    if unscored:
        print(f"\n{len(unscored)} of {len(results)} samples were not fully "
              f"measured:", file=sys.stderr)
        for r in unscored[:5]:
            bad = [m for m in metric_names
                   if not isinstance(r.metrics.get(m), (int, float))]
            nan = [m for m in metric_names
                   if isinstance(r.metrics.get(m), (int, float))
                   and not math.isfinite(float(r.metrics[m]))]
            if r.error or r.exit_code != 0:
                detail = r.error or f"exit {r.exit_code}"
            elif nan:
                detail = f"NaN: {', '.join(nan)}"
            else:
                detail = f"missing: {', '.join(bad)}"
            print(f"  {r.adapter} {Path(r.fixture).stem}: {detail}", file=sys.stderr)
        if len(unscored) > 5:
            print(f"  … and {len(unscored) - 5} more", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
