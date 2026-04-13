#!/bin/bash
# MDM Real-World Document Benchmark Suite
# Runs all documents in tests/realworld/ through MDM and reports results.
# Add new test files to any subdirectory — they'll be picked up automatically.
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MDM_BIN="$PROJECT_ROOT/core/target/debug/hwp2mdm"
RESULTS_FILE="$SCRIPT_DIR/results.csv"

# Build if needed
if [ ! -f "$MDM_BIN" ]; then
    echo "Building MDM..."
    (cd "$PROJECT_ROOT/core" && cargo build 2>/dev/null)
fi

echo "source,format,size_bytes,status,time_ms,lines,chars,headings,tables,bold" > "$RESULTS_FILE"

TOTAL=0
OK=0
FAIL=0

echo "=========================================="
echo "  MDM Real-World Benchmark"
echo "  $(date '+%Y-%m-%d %H:%M')"
echo "=========================================="

# Find all supported files recursively
find "$SCRIPT_DIR" -type f \( -name "*.hwp" -o -name "*.hwpx" -o -name "*.pdf" -o -name "*.docx" \) | sort | while read -r f; do
    base=$(basename "$f")
    rel=$(python3 -c "import os; print(os.path.relpath('$f', '$SCRIPT_DIR'))")
    ext="${base##*.}"
    name="${base%.*}"
    size=$(stat -f%z "$f" 2>/dev/null || stat -c%s "$f" 2>/dev/null)
    outdir="$SCRIPT_DIR/output/${rel%.*}"
    mkdir -p "$outdir"

    TOTAL=$((TOTAL + 1))

    # Run MDM
    START=$(python3 -c "import time; print(time.time())")
    "$MDM_BIN" "$f" -o "$outdir" 2>/dev/null
    END=$(python3 -c "import time; print(time.time())")
    MS=$(python3 -c "print(int(($END-$START)*1000))")

    # Check output
    mdx=$(find "$outdir" -name "*.mdx" 2>/dev/null | head -1)
    if [ -n "$mdx" ] && [ -f "$mdx" ]; then
        lines=$(wc -l < "$mdx" | tr -d ' ')
        chars=$(wc -c < "$mdx" | tr -d ' ')
        headings=$(grep -c "^#" "$mdx" 2>/dev/null || echo 0)
        tables=$(grep -c "^|" "$mdx" 2>/dev/null || echo 0)
        bold=$(grep -o "\*\*" "$mdx" 2>/dev/null | wc -l | tr -d ' ')
        status="OK"
        OK=$((OK + 1))
        echo "  ✅ ${rel} (${MS}ms) → ${lines}L ${headings}H ${tables}T"
    else
        lines=0; chars=0; headings=0; tables=0; bold=0
        status="FAIL"
        FAIL=$((FAIL + 1))
        echo "  ❌ ${rel} (${MS}ms)"
    fi

    echo "${rel},${ext},${size},${status},${MS},${lines},${chars},${headings},${tables},${bold}" >> "$RESULTS_FILE"
done

echo ""
echo "=========================================="
echo "  Results: $(grep -c "OK" "$RESULTS_FILE") OK / $(grep -c "FAIL" "$RESULTS_FILE") FAIL"
echo "  CSV: $RESULTS_FILE"
echo "=========================================="
