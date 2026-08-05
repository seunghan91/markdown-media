# MDM Benchmark Harness

Matrix runner that scores MDM against reference documents, and optionally against
external parsers, on a fixed set of PDFs.

## What the reference documents actually are

**`ground_truth/` is not hand-curated.** Each `document.md` is an *older output of
this same parser*, kept as a fixed point of comparison. The giveaway is its
frontmatter — `format` / `version` / `pages` / `images` / `fonts` / `tables` /
`author` — byte-identical in shape to what `hwp2mdm` emits today.

That has consequences you must hold onto when reading a score:

- **The numbers measure drift, not quality.** A low score means "differs from the
  old output", not "wrong". Verified example: `korean_press2` scores worst in the
  set, yet the current output renders the dateline as one clean paragraph while
  the reference scatters it across nine table cells. The current output is better.
- **A real improvement can lower the score, and a regression can raise it.**
  Whenever a metric moves, read the actual diff before deciding which way it went.
- **Cross-parser comparison is skewed** for the same reason — an external parser is
  being scored against MDM's own past behaviour.

Known staleness in the references, all traced to the parser version that produced
them: pre-CommonMark image syntax (`@[[image_N]]`, 685 occurrences — `metrics.py`
normalizes both sides so this no longer costs points), spurious tables from the
table detector of the day (empty-cell ratio reaches 49% in `korean_press2`), and
image counts that include `/ImageMask` stencils the extractor now deliberately
skips.

Use the harness to catch regressions between commits. Do not use it to claim
absolute quality.

## Layout

- `fixtures/` — input PDFs, grouped by kind (`cjk/`, `mixed/`, `text-native/`,
  `edge/`, `scanned/`). Gitignored: large and freely regenerable.
- `ground_truth/<pdf-stem>/document.md` — the reference text (see above). Tracked.
  Image and `.mdm` byproducts sitting beside it are gitignored.
- `adapters/` — one conversion wrapper per parser. Only `mdm.py` ships; external
  ones are developer-local (see below).
- `runner.py` — orchestrator (parallel matrix: parser × fixture)
- `metrics.py` — BLEU / ROUGE-L / CER / edit ratio / TSED
- `check_regression.py` — the gate
- `config.toml` — active adapters, fixtures and gate thresholds
- `results/<timestamp>/` — per-run artifacts + aggregated matrix. Gitignored.
- `regressions/` — PDFs where MDM scored lowest, auto-copied for triage. Gitignored.

## Running

The metric libraries are not in the repo environment; install them into a venv
that `.gitignore` already expects at `bench/.venv/`:

```bash
cd bench
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
```

Then set the binary path. `config.toml` ships
`binary = "../core/target/release/hwp2mdm"`, which is relative to the working
directory and therefore wrong for any run started outside `bench/` — and wrong
inside it too if `CARGO_TARGET_DIR` sends builds elsewhere. Put an absolute path
in `config.local.toml`, which is gitignored:

```toml
[adapters.mdm]
module = "adapters.mdm"
binary = "/absolute/path/to/hwp2mdm"
```

Now run it from a directory that is **not** `bench/` and not above it. `nltk`
refuses to import anything that resolves inside the working directory, `.venv`
lives inside `bench/`, so a run started from `bench/` cannot import `regex` and
every `bleu`/`rouge_l` comes back NaN. `PYTHONSAFEPATH` does not help — the
package really is under the cwd, which is the thing being refused.

The roots have to stay relative even so, or the fixture keys stop matching the
baseline and the per-fixture level of the gate compares nothing. A directory of
symlinks satisfies both. Set `BENCH` to this directory's absolute path:

```bash
BENCH=$PWD                       # from bench/
mkdir -p "$BENCH/results" /tmp/mdmbench && cd /tmp/mdmbench
for p in fixtures ground_truth results metrics.py; do ln -sfn "$BENCH/$p" .; done

PYTHONPATH=$BENCH "$BENCH/.venv/bin/python" "$BENCH/runner.py" \
  --config "$BENCH/config.toml" --config-local "$BENCH/config.local.toml"
```

`runner.py` takes only `--config` and `--config-local`; fixture and output roots
come from the config file. It exits `2` instead of writing a green-looking run
when an adapter binary is missing, when a conversion fails, or when a fixture
that has a reference is not fully scored.

## The gate

Run it from the same directory as the bench run — it reads `results/` and
`metrics.py` relative to the cwd, not from the config:

```bash
PYTHONPATH=$BENCH "$BENCH/.venv/bin/python" "$BENCH/check_regression.py" \
  --baseline "$BENCH/baseline-metrics.2026-08-03.json" --config "$BENCH/config.toml"
```

Three levels, because an aggregate hides things:

- **overall** — mean across every fixture
- **per-slice** — `cjk` / `mixed` / `text-native` separately
- **per-fixture** — each PDF on its own

The slice and fixture levels exist because the easy English and synthetic fixtures
score around 0.95 while `cjk`/`mixed` sit near 0.7, so a genuine Korean regression
can surface as an overall *improvement*. Measured: `korean_directive` lost 0.026
BLEU in a change whose mean went **up** 0.001. Tolerances are `slice_drop_max` and
`fixture_drop_max` in `config.toml`.

Exit codes: `0` pass, `1` regression, `2` gate blocked.

`2` is the one to read carefully. It means no verdict was reached, which is not
the same as passing, and every one of these used to print `all gates passed`:

- a metric came back `nan`, so it was never measured — `nan` loses every
  comparison, which the gate read as "no drop"
- the run scored nothing at all
- a comparison level had no baseline group in common with the run, so it
  compared nothing while printing one `ok` per group it did have
- `metrics.py` changed since the baseline was recorded, so the two are on
  different scales

### Baselines

`baseline-metrics.2026-08-03.json` records overall, per-slice and per-fixture
scores together with the commit SHA, the normalization rule, dependency pins and a
hash of `metrics.py`. If `metrics.py` changes, the gate exits `2` — the scores were
measured on a different scale, which is not a regression and must not be reported
as one. Re-record the baseline instead of forcing a comparison.

`baseline-metrics.json` is the older flat `{metric: value}` pin. It has no record
of its scale or fixture set, so it is not comparable to current runs; it is kept
only for backwards compatibility and still works with the legacy code path.

To re-record after a deliberate behaviour change, regenerate the dated file with
the same fields (see its `_comment`) and commit it alongside the change, with the
before/after numbers in the commit message. Raising the floor is safe; lowering it
lets a future regression hide inside the baseline.

## Reference parsers

External parsers are cloned into `../reference/` (gitignored). Each needs an
adapter in `adapters/` — the repo ships none for any specific external parser, so
those are developer-local, as is the `config.local.toml` that enables them. See
`adapters/mdm.py` for the adapter contract.

`cross_validate.py` and `find_disagreements.py` compare parsers against each other
rather than against `ground_truth`. Given how the references were made, these are
the more trustworthy signal for "where is MDM alone in being wrong".

## CI

There is no bench workflow. `.github/workflows/` builds and tests the crate; the
harness is run by hand.

See `plan/pdf-triage.md` §Benchmark Harness for the metric table.
