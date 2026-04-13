#!/bin/bash
# DOCX Parser Benchmark: MDM vs Pandoc vs mammoth.js
set -e

BENCH_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="$BENCH_DIR/output"
rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"/{pandoc,mammoth,mdm}

MDM_BIN="$BENCH_DIR/../../core/target/debug/hwp2mdm"

# Build MDM if needed
echo "=== Building MDM core ==="
(cd "$BENCH_DIR/../../core" && cargo build 2>/dev/null)

for docx in "$BENCH_DIR"/test_*.docx; do
    base=$(basename "$docx" .docx)
    echo ""
    echo "=== Processing: $base ==="

    # --- Pandoc ---
    echo -n "  Pandoc:  "
    START=$(python3 -c "import time; print(time.time())")
    pandoc "$docx" -t markdown -o "$OUT_DIR/pandoc/${base}.md" 2>/dev/null
    END=$(python3 -c "import time; print(time.time())")
    echo "$(python3 -c "print(f'{($END - $START)*1000:.0f}ms')")"

    # --- mammoth.js ---
    echo -n "  mammoth: "
    START=$(python3 -c "import time; print(time.time())")
    mammoth "$docx" --output-format=markdown --output-dir="$OUT_DIR/mammoth" 2>/dev/null || \
    mammoth "$docx" --output-format=markdown > "$OUT_DIR/mammoth/${base}.md" 2>/dev/null || true
    END=$(python3 -c "import time; print(time.time())")
    echo "$(python3 -c "print(f'{($END - $START)*1000:.0f}ms')")"

    # --- MDM (Rust) ---
    echo -n "  MDM:     "
    START=$(python3 -c "import time; print(time.time())")
    if [ -f "$MDM_BIN" ]; then
        "$MDM_BIN" "$docx" "$OUT_DIR/mdm/${base}" 2>/dev/null || true
    else
        echo "MDM binary not found, skipping"
    fi
    END=$(python3 -c "import time; print(time.time())")
    echo "$(python3 -c "print(f'{($END - $START)*1000:.0f}ms')")"
done

echo ""
echo "=== Output comparison ==="
echo ""

for docx in "$BENCH_DIR"/test_*.docx; do
    base=$(basename "$docx" .docx)
    echo "--- $base ---"

    PANDOC_FILE="$OUT_DIR/pandoc/${base}.md"
    MAMMOTH_FILE="$OUT_DIR/mammoth/${base}.md"
    MDM_FILE=$(find "$OUT_DIR/mdm/${base}" -name "*.mdx" 2>/dev/null | head -1)

    if [ -f "$PANDOC_FILE" ]; then
        echo "  Pandoc:  $(wc -l < "$PANDOC_FILE") lines, $(wc -c < "$PANDOC_FILE") bytes"
    fi
    if [ -f "$MAMMOTH_FILE" ]; then
        echo "  mammoth: $(wc -l < "$MAMMOTH_FILE") lines, $(wc -c < "$MAMMOTH_FILE") bytes"
    fi
    if [ -n "$MDM_FILE" ] && [ -f "$MDM_FILE" ]; then
        echo "  MDM:     $(wc -l < "$MDM_FILE") lines, $(wc -c < "$MDM_FILE") bytes"
    else
        echo "  MDM:     (no output)"
    fi
    echo ""
done
