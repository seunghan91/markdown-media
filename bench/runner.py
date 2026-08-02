"""Bench runner — matrix over (adapter × fixture), emits per-sample metrics.

Usage:
    python runner.py --config config.toml
    python runner.py --config config.toml --config-local config.local.toml
"""
from __future__ import annotations

import argparse
import importlib
import json
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
    if args.config_local and Path(args.config_local).exists():
        local = tomllib.loads(Path(args.config_local).read_text())
        for k, v in local.items():
            if isinstance(v, dict) and k in cfg:
                cfg[k].update(v)
            else:
                cfg[k] = v

    fx_root = Path(cfg["run"]["fixtures_root"])
    gt_root = Path(cfg["run"]["ground_truth_root"])
    out_root = Path(cfg["run"]["results_root"]) / datetime.now().strftime("%Y-%m-%d_%H%M%S")
    out_root.mkdir(parents=True, exist_ok=True)

    fixtures = discover_fixtures(fx_root)
    adapters = cfg.get("adapters", {})
    metric_names = cfg.get("metrics", {}).get("enabled", [])

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
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
