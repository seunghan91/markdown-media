#!/bin/bash
# MDM vs unhwp 품질 비교 스크립트
# Usage: ./compare.sh [mdm_dir] [unhwp_dir]

MDM_DIR="${1:-mdm_before}"
UNHWP_DIR="${2:-unhwp_baseline}"
BASE="/Users/seunghan/markdown-media/tests/benchmark"

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║  MDM vs unhwp 품질 비교 ($MDM_DIR vs $UNHWP_DIR)"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

DIRS=(table lists charshape footnote-endnote sample-5017 sample-5017-pics hwpx_test)
TOTAL_MDM_SCORE=0
TOTAL_UNHWP_SCORE=0

printf "%-22s %6s %6s %6s %6s %6s | %6s %6s %6s %6s %6s\n" \
  "File" "Lines" "Chars" "Tbl" "Head" "Bold" "Lines" "Chars" "Tbl" "Head" "Bold"
printf "%-22s %6s %6s %6s %6s %6s | %6s %6s %6s %6s %6s\n" \
  "" "---MDM---" "" "" "" "" "---unhwp---" "" "" "" ""
echo "─────────────────────────────────────────────────────────────────────────────────"

for dir in "${DIRS[@]}"; do
  MDM_FILE=$(find "$BASE/$MDM_DIR/$dir" -name "*.mdx" 2>/dev/null | head -1)
  UNHWP_FILE=$(find "$BASE/$UNHWP_DIR/$dir" -name "*.md" 2>/dev/null | head -1)

  if [ -n "$MDM_FILE" ]; then
    ML=$(wc -l < "$MDM_FILE" | tr -d ' ')
    MC=$(wc -c < "$MDM_FILE" | tr -d ' ')
    MT=$(grep -c '|.*|.*|' "$MDM_FILE" 2>/dev/null || echo 0)
    MH=$(grep -c '^#' "$MDM_FILE" 2>/dev/null || echo 0)
    MB=$(grep -co '\*\*[^*]*\*\*' "$MDM_FILE" 2>/dev/null || echo 0)
  else
    ML=0; MC=0; MT=0; MH=0; MB=0
  fi

  if [ -n "$UNHWP_FILE" ]; then
    UL=$(wc -l < "$UNHWP_FILE" | tr -d ' ')
    UC=$(wc -c < "$UNHWP_FILE" | tr -d ' ')
    UT=$(grep -c '|.*|.*|' "$UNHWP_FILE" 2>/dev/null || echo 0)
    UH=$(grep -c '^#' "$UNHWP_FILE" 2>/dev/null || echo 0)
    UB=$(grep -co '\*\*[^*]*\*\*' "$UNHWP_FILE" 2>/dev/null || echo 0)
  else
    UL=0; UC=0; UT=0; UH=0; UB=0
  fi

  printf "%-22s %6d %6d %6d %6d %6d | %6d %6d %6d %6d %6d\n" \
    "$dir" "$ML" "$MC" "$MT" "$MH" "$MB" "$UL" "$UC" "$UT" "$UH" "$UB"
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  품질 기준 체크리스트"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

check() {
  local file="$1" pattern="$2" label="$3"
  if [ -n "$file" ] && grep -q "$pattern" "$file" 2>/dev/null; then
    echo "  ✅ $label"
  else
    echo "  ❌ $label"
  fi
}

for tool in "$MDM_DIR" "$UNHWP_DIR"; do
  echo ""
  echo "[$tool]"

  # charshape: bold/italic detection
  F=$(find "$BASE/$tool/charshape" -name "*.mdx" -o -name "*.md" 2>/dev/null | head -1)
  check "$F" '\*\*' "charshape: Bold 감지 (**text**)"
  check "$F" '\*[^*]' "charshape: Italic 감지 (*text*)"

  # sample-5017: heading detection
  F=$(find "$BASE/$tool/sample-5017" -name "*.mdx" -o -name "*.md" 2>/dev/null | head -1)
  check "$F" '^# ' "sample-5017: Heading 감지 (# text)"
  check "$F" '|.*|.*|' "sample-5017: Table 렌더링"

  # sample-5017-pics: image extraction
  F=$(find "$BASE/$tool/sample-5017-pics" -name "*.mdx" -o -name "*.md" 2>/dev/null | head -1)
  check "$F" '!\[' "sample-5017-pics: Image 참조 (![alt](path))"

  # footnote: footnote detection
  F=$(find "$BASE/$tool/footnote-endnote" -name "*.mdx" -o -name "*.md" 2>/dev/null | head -1)
  check "$F" '각주\|footnote\|\[\^' "footnote-endnote: 각주 감지"

  # HWPX: bold in table headers
  F=$(find "$BASE/$tool/hwpx_test" -name "*.mdx" -o -name "*.md" 2>/dev/null | head -1)
  check "$F" '|\s*\*\*' "hwpx: Table header Bold 감지"
done
