#!/bin/bash
# macOS codesigning + notarization build script for MDM Desktop
# Prerequisites:
#   1. "Developer ID Application: seunghan kim (AVMJBATWAT)" certificate in local keychain
#   2. App-specific password stored via:
#      xcrun notarytool store-credentials "mdm-notarize" \
#        --apple-id "theqwe2000@naver.com" \
#        --team-id "AVMJBATWAT" \
#        --password "<app-specific-password>"
#   3. Or set APPLE_PASSWORD env var before running this script

set -euo pipefail

# --- Codesigning ---
export APPLE_SIGNING_IDENTITY="Developer ID Application: seunghan kim (AVMJBATWAT)"

# --- Notarization ---
export APPLE_ID="theqwe2000@naver.com"
export APPLE_TEAM_ID="AVMJBATWAT"
# APPLE_PASSWORD must be set externally:
#   export APPLE_PASSWORD="your-app-specific-password"
# Or use keychain profile: xcrun notarytool store-credentials "mdm-notarize"

if [ -z "${APPLE_PASSWORD:-}" ]; then
  # Try to extract from keychain profile
  APPLE_PASSWORD=$(security find-generic-password -s "notarytool-profile:mdm-notarize" -w 2>/dev/null || true)
  if [ -z "${APPLE_PASSWORD:-}" ]; then
    echo "WARNING: APPLE_PASSWORD not set and keychain profile not found."
    echo "Build will be signed but NOT notarized."
    echo "Set it via: export APPLE_PASSWORD=\"your-app-specific-password\""
    echo ""
  else
    export APPLE_PASSWORD
    echo "Using keychain profile for notarization."
  fi
fi

cd "$(dirname "$0")/.."

echo "=== Building MDM Desktop (signed) ==="
echo "Identity: ${APPLE_SIGNING_IDENTITY}"
echo "Target:   native ($(uname -m))"
echo ""

npx tauri build

echo ""
echo "=== Build complete ==="
echo "Output: src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/"
