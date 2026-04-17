#!/bin/bash
# Evaluate each HWP corpus source: run parser, collect stats.
# Usage: ./scripts/eval-hwp-corpus.sh [source_name]

set -e
cd "$(dirname "$0")/.."

CORPUS=assets/conformance/hwp-corpus
OUT=target/eval-hwp
BIN=core/target/release/hwp2mdm
mkdir -p "$OUT"

# Build once
(cd core && cargo build --release --bin hwp2mdm --quiet)

run_source() {
  local src="$1"
  local src_dir="$CORPUS/$src"
  [ -d "$src_dir" ] || return
  echo ""
  echo "===== $src ====="
  local outdir="$OUT/$src"
  rm -rf "$outdir"
  mkdir -p "$outdir"

  local total=0 ok=0 fail=0 total_md_bytes=0 total_in_bytes=0
  # Write a per-file log
  local log="$OUT/$src.tsv"
  echo -e "file\tstatus\tinput_bytes\tmd_bytes\tms\terror" > "$log"

  while IFS= read -r f; do
    total=$((total + 1))
    local in_sz=$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f")
    local start=$(python3 -c "import time; print(int(time.time()*1000))")
    local stem=$(basename "$f")
    stem="${stem%.*}"
    local md_out="$outdir/${total}_${stem}.md"

    if "$BIN" convert "$f" -o "$outdir" --format mdx > /tmp/hwp_run.log 2>&1; then
      # Find generated file
      local found=$(/usr/bin/find "$outdir" -name "*.md*" -newer /tmp/hwp_run.log 2>/dev/null | head -1)
      if [ -z "$found" ]; then
        found=$(/usr/bin/find "$outdir" -name "*.md*" | tail -1)
      fi
      if [ -n "$found" ] && [ -s "$found" ]; then
        local md_sz=$(stat -f%z "$found" 2>/dev/null || stat -c%s "$found")
        local end=$(python3 -c "import time; print(int(time.time()*1000))")
        local ms=$((end - start))
        ok=$((ok + 1))
        total_md_bytes=$((total_md_bytes + md_sz))
        total_in_bytes=$((total_in_bytes + in_sz))
        echo -e "$(basename "$f")\tOK\t$in_sz\t$md_sz\t$ms\t" >> "$log"
      else
        fail=$((fail + 1))
        local end=$(python3 -c "import time; print(int(time.time()*1000))")
        local ms=$((end - start))
        echo -e "$(basename "$f")\tEMPTY\t$in_sz\t0\t$ms\tempty output" >> "$log"
      fi
    else
      fail=$((fail + 1))
      local end=$(python3 -c "import time; print(int(time.time()*1000))")
      local ms=$((end - start))
      local err=$(tail -1 /tmp/hwp_run.log | head -c 200 | tr '\t\n' ' ')
      echo -e "$(basename "$f")\tFAIL\t$in_sz\t0\t$ms\t$err" >> "$log"
    fi
  done < <(/usr/bin/find "$src_dir" -type f \( -name '*.hwp' -o -name '*.hwpx' \))

  local ratio="0"
  if [ $total_in_bytes -gt 0 ]; then
    ratio=$(python3 -c "print(f'{$total_md_bytes/$total_in_bytes:.3f}')")
  fi
  echo "  total: $total"
  echo "  ok:    $ok"
  echo "  fail:  $fail"
  echo "  md/in ratio: $ratio"
  echo "  log:   $log"
}

if [ -n "$1" ]; then
  run_source "$1"
else
  for src in open_samples law_go_kr assembly_go_kr gov_press gov_kr; do
    run_source "$src"
  done
fi

echo ""
echo "===== aggregate ====="
python3 << 'PYEOF'
from pathlib import Path
logs = sorted(Path('target/eval-hwp').glob('*.tsv'))
print(f"{'source':<20} {'total':>6} {'ok':>6} {'fail':>6} {'success%':>9} {'avg_ms':>8}")
print('-' * 60)
for log in logs:
    lines = log.read_text().strip().split('\n')[1:]
    if not lines:
        continue
    total = len(lines)
    ok = sum(1 for l in lines if l.split('\t')[1] == 'OK')
    fail = total - ok
    mss = [int(l.split('\t')[4]) for l in lines if l.split('\t')[4].isdigit()]
    avg_ms = sum(mss) // len(mss) if mss else 0
    pct = 100 * ok / total if total else 0
    print(f"{log.stem:<20} {total:>6} {ok:>6} {fail:>6} {pct:>8.1f}% {avg_ms:>8}")
PYEOF
