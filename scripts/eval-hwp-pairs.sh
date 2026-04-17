#!/bin/bash
# Convert HWP↔HWPX pairs, normalize, diff. Expectation: same semantic content.
set -e
cd "$(dirname "$0")/.."

BIN=core/target/release/hwp2mdm
PAIR_DIR=assets/conformance/hwp-corpus/open_samples/neolord0_hwp2hwpx
OUT=target/eval-pairs
mkdir -p "$OUT"

(cd core && cargo build --release --bin hwp2mdm --quiet)

normalize() {
  # Strip YAML frontmatter, blank lines, trim whitespace
  awk 'BEGIN{fm=0} /^---$/{fm++; next} fm==1{next} {print}' "$1" \
    | sed 's/[[:space:]]*$//' \
    | awk 'NF'
}

echo "name                hwp_bytes  hwpx_bytes  md_hwp_bytes  md_hwpx_bytes  diff_lines  verdict"
printf '%.0s-' {1..95}; echo

for hwp in "$PAIR_DIR"/*.hwp; do
  stem=$(basename "$hwp" .hwp)
  hwpx="$PAIR_DIR/$stem.hwpx"
  [ -f "$hwpx" ] || continue

  rm -rf "$OUT/$stem"; mkdir -p "$OUT/$stem"
  "$BIN" convert "$hwp"  -o "$OUT/$stem/from_hwp"  --format mdx > /dev/null 2>&1 || true
  "$BIN" convert "$hwpx" -o "$OUT/$stem/from_hwpx" --format mdx > /dev/null 2>&1 || true

  md_hwp=$(/usr/bin/find "$OUT/$stem/from_hwp"  -name '*.mdx' | head -1)
  md_hwpx=$(/usr/bin/find "$OUT/$stem/from_hwpx" -name '*.mdx' | head -1)

  if [ -z "$md_hwp" ] || [ -z "$md_hwpx" ]; then
    printf "%-20s %10s %11s %13s %14s  %10s  FAIL (missing output)\n" \
      "$stem" "$(stat -f%z "$hwp")" "$(stat -f%z "$hwpx")" "-" "-" "-"
    continue
  fi

  hsz=$(stat -f%z "$md_hwp")
  xsz=$(stat -f%z "$md_hwpx")
  normalize "$md_hwp"  > "$OUT/$stem/normalized_hwp.md"
  normalize "$md_hwpx" > "$OUT/$stem/normalized_hwpx.md"
  diff_lines=$(diff "$OUT/$stem/normalized_hwp.md" "$OUT/$stem/normalized_hwpx.md" | wc -l | tr -d ' ')

  if [ "$diff_lines" = "0" ]; then
    verdict="IDENTICAL"
  else
    lines_hwp=$(wc -l < "$OUT/$stem/normalized_hwp.md" | tr -d ' ')
    lines_hwpx=$(wc -l < "$OUT/$stem/normalized_hwpx.md" | tr -d ' ')
    pct=$(python3 -c "print(f'{$diff_lines*100/max($lines_hwp,$lines_hwpx,1):.1f}')")
    verdict="DIFF ${diff_lines}L (${pct}% of ${lines_hwp}/${lines_hwpx})"
  fi

  printf "%-20s %10s %11s %13s %14s  %10s  %s\n" \
    "$stem" "$(stat -f%z "$hwp")" "$(stat -f%z "$hwpx")" "$hsz" "$xsz" "$diff_lines" "$verdict"
done

echo ""
echo "Normalized outputs: $OUT/*/normalized_*.md"
echo "To inspect a diff:  diff $OUT/equation/normalized_hwp.md $OUT/equation/normalized_hwpx.md"
