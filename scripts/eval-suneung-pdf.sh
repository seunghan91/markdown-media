#!/bin/bash
# Batch convert all 수능 PDFs, collect success/fail + content stats.
set -e
cd "$(dirname "$0")/.."

BIN=core/target/release/hwp2mdm
CORPUS=assets/conformance/suneung/pdf
OUT=target/eval-suneung
LOG=$OUT/results.tsv
mkdir -p "$OUT"

(cd core && cargo build --release --bin hwp2mdm --quiet)

echo -e "file\tstatus\tpages\tchars\tmd_bytes\tms" > "$LOG"

count_pdfs=$(/usr/bin/find "$CORPUS" -name '*.pdf' | wc -l | tr -d ' ')
echo "[plan] $count_pdfs PDFs to process"

export BIN OUT LOG

process() {
  local f="$1"
  local rel="${f#$CORPUS/}"
  local outdir="$OUT/out/$(dirname "$rel")"
  mkdir -p "$outdir"

  local start=$(python3 -c "import time; print(int(time.time()*1000))")
  local log_out
  log_out=$("$BIN" convert "$f" -o "$outdir" --format mdx 2>&1 || echo "__FAIL__")
  local end=$(python3 -c "import time; print(int(time.time()*1000))")
  local ms=$((end - start))

  if [[ "$log_out" == *"__FAIL__"* ]] || [[ "$log_out" == *"Error"* && "$log_out" != *"✅"* ]]; then
    echo -e "$rel\tFAIL\t0\t0\t0\t$ms" >> "$LOG"
    return
  fi

  local stem=$(basename "$f" .pdf)
  local md="$outdir/$stem.mdx"
  if [ ! -s "$md" ]; then
    echo -e "$rel\tEMPTY\t0\t0\t0\t$ms" >> "$LOG"
    return
  fi

  local pages=$(echo "$log_out" | grep -oE 'Pages: [0-9]+' | head -1 | grep -oE '[0-9]+' || echo 0)
  local chars=$(echo "$log_out" | grep -oE 'Text length: [0-9]+' | head -1 | grep -oE '[0-9]+' || echo 0)
  local md_bytes=$(stat -f%z "$md")
  echo -e "$rel\tOK\t$pages\t$chars\t$md_bytes\t$ms" >> "$LOG"
}
export -f process

# Parallel processing — xargs with 4 workers (PDF converter uses multi-threading internally)
/usr/bin/find "$CORPUS" -name '*.pdf' -print0 \
  | xargs -0 -P 4 -I{} bash -c 'process "{}"'

echo ""
echo "===== aggregate ====="
python3 << 'PYEOF'
from pathlib import Path
log = Path('target/eval-suneung/results.tsv')
lines = log.read_text().strip().split('\n')[1:]
total = len(lines)
ok = sum(1 for l in lines if l.split('\t')[1] == 'OK')
fail = sum(1 for l in lines if l.split('\t')[1] == 'FAIL')
empty = sum(1 for l in lines if l.split('\t')[1] == 'EMPTY')
pages = sum(int(l.split('\t')[2]) for l in lines if l.split('\t')[1] == 'OK')
chars = sum(int(l.split('\t')[3]) for l in lines if l.split('\t')[1] == 'OK')
md_bytes = sum(int(l.split('\t')[4]) for l in lines if l.split('\t')[1] == 'OK')
mss = [int(l.split('\t')[5]) for l in lines]

print(f"total:    {total}")
print(f"ok:       {ok} ({100*ok/total:.1f}%)")
print(f"fail:     {fail}")
print(f"empty:    {empty}")
print(f"pages:    {pages:,}")
print(f"chars:    {chars:,}")
print(f"md MB:    {md_bytes/1024/1024:.1f}")
print(f"avg ms:   {sum(mss)//len(mss) if mss else 0:,}")
print(f"total min: {sum(mss)/60000:.1f}")

# By subject
from collections import defaultdict
by_subj = defaultdict(lambda: {'ok': 0, 'fail': 0, 'empty': 0, 'chars': 0})
for l in lines:
    parts = l.split('\t')
    # rel path: suneung/2026/seq_X_과목/파일.pdf → extract 과목 from dir
    path_parts = parts[0].split('/')
    if len(path_parts) >= 3:
        subj_dir = path_parts[2]
        subj = subj_dir.split('_', 2)[2] if '_' in subj_dir else subj_dir
    else:
        subj = '?'
    status = parts[1]
    by_subj[subj][status.lower()] = by_subj[subj].get(status.lower(), 0) + 1
    if status == 'OK':
        by_subj[subj]['chars'] += int(parts[3])

print()
print(f"{'subject':<20} {'ok':>5} {'fail':>5} {'empty':>5} {'chars':>12}")
print('-' * 55)
for subj, s in sorted(by_subj.items()):
    print(f"{subj:<20} {s.get('ok',0):>5} {s.get('fail',0):>5} {s.get('empty',0):>5} {s.get('chars',0):>12,}")
PYEOF
