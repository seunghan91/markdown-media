#!/usr/bin/env bash
#
# download-fonts.sh — fetch an open-license Korean CJK font for the
# `print-pdf` renderer to embed (see core/src/print/pdf.rs).
#
# The font binary is written to core/assets/fonts/, which is gitignored:
# we never commit font binaries to the repo. The renderer auto-discovers
# fonts placed here (highest search priority), so after running this the
# `print-pdf` path renders Korean with no further configuration.
#
# License: Nanum Gothic is published by Naver under the SIL Open Font
# License 1.1 (OFL-1.1) — https://scripts.sil.org/OFL. Redistribution and
# embedding in documents is permitted under the OFL. This script downloads
# it at build/setup time rather than vendoring it.
#
# Usage:  bash core/scripts/download-fonts.sh
set -euo pipefail

DEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/assets/fonts"
mkdir -p "$DEST_DIR"

# Nanum Gothic (OFL-1.1), served from Google Fonts' font-files mirror.
FONT_URL="https://github.com/google/fonts/raw/main/ofl/nanumgothic/NanumGothic-Regular.ttf"
DEST="$DEST_DIR/NanumGothic-Regular.ttf"

echo "[download-fonts] fetching Nanum Gothic (OFL-1.1) -> $DEST"
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$FONT_URL" -o "$DEST"
elif command -v wget >/dev/null 2>&1; then
  wget -O "$DEST" "$FONT_URL"
else
  echo "[download-fonts] need curl or wget" >&2
  exit 1
fi

echo "[download-fonts] done. The print-pdf renderer will now embed this font."
echo "[download-fonts] License: SIL Open Font License 1.1 (see script header)."
