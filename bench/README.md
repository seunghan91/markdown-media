# MDM Benchmark Harness

Matrix runner that evaluates MDM against external reference parsers on a curated golden set.

## Layout

- `fixtures/` — input PDFs, categorized by expected triage class
- `ground_truth/<pdf-stem>/document.md` + `tables.json` — hand-curated reference output
- `adapters/` — per-parser conversion wrappers (one per reference parser, configured locally)
- `runner.py` — orchestrator (parallel matrix: parser × fixture)
- `metrics.py` — BLEU / ROUGE-L / CER / TSED / TEDS
- `config.toml` — which adapters and fixtures are active
- `results/<date>/` — per-run artifacts + aggregated matrix
- `regressions/` — PDFs where MDM scored lowest, auto-copied for triage

## Reference Parsers

External parsers are cloned into `../reference/` (gitignored). Each has a corresponding adapter in `adapters/` that is configured locally — the MDM repo itself does not ship adapter code for any specific external parser. See `adapters/mdm.py` for the adapter contract.

## Running

```bash
cd bench
pip install -r requirements.txt
python runner.py --config config.toml --fixtures fixtures/ --out results/
```

## Metrics

See `plan/pdf-triage.md` §Benchmark Harness for the full metric table and regression gate logic.

## CI integration

`.github/workflows/bench.yml` runs this harness on every push / PR to
`master` that touches `core/` or `bench/`. The workflow:

1. Builds the `hwp2mdm` release binary
2. Runs `runner.py` with the committed `config.toml` only (external adapters are developer-only)
3. Compares the MDM aggregate against `baseline-metrics.json` via `check_regression.py --strict`
4. Fails the run on any metric drop >5% vs baseline, or if BLEU <0.80 / CER >0.10 absolute

### Updating the baseline

When a deliberate behavior change shifts metrics, regenerate the baseline:

```bash
cd bench
# After a successful run on master
python -c "
import json
from pathlib import Path
data = json.loads(sorted(Path('results').glob('*/matrix.json'))[-1].read_text())
buckets = {}
for row in data:
    if row['adapter'] != 'mdm': continue
    for k, v in row['metrics'].items():
        if isinstance(v, (int, float)):
            buckets.setdefault(k, []).append(v)
agg = {m: sum(vs)/len(vs) for m, vs in buckets.items()}
Path('baseline-metrics.json').write_text(json.dumps(agg, indent=2))
print(json.dumps(agg, indent=2))
"
```

Commit the updated `baseline-metrics.json` in the same PR that shifted metrics, with a rationale in the commit message.
