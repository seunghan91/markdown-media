#!/usr/bin/env bash
# Download + verify PP-OCRv5 korean OCR models for the `ocr` feature.
#
# Cache: $MDM_MODEL_CACHE/ppocr  (default: ~/.cache/mdm/models/ppocr)
# Total ~18MB. Models are NOT committed. Idempotent: existing valid files are kept.
#
# Provenance: PaddlePaddle official HuggingFace ONNX conversions (Apache-2.0).
# SHA-256 pins mirror core/src/ocr/models.rs — keep both in sync.
set -euo pipefail

CACHE_ROOT="${MDM_MODEL_CACHE:-$HOME/.cache/mdm/models}"
DIR="$CACHE_ROOT/ppocr"
mkdir -p "$DIR"

# filename|url|sha256
MODELS=(
  "det.onnx|https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/main/inference.onnx|a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d"
  "rec_korean.onnx|https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_onnx/resolve/main/inference.onnx|92f0b7785e64fc9090106a241cf4c1eb97472824558272751b88a2a4476d3a08"
  "rec_korean.yml|https://huggingface.co/PaddlePaddle/korean_PP-OCRv5_mobile_rec_onnx/resolve/main/inference.yml|f757fa1c40e99edcf27e9cce879b93eb2a51fa46f5ef39095689b8c37dd75998"
)

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

for entry in "${MODELS[@]}"; do
  IFS='|' read -r fname url want <<<"$entry"
  dest="$DIR/$fname"
  if [[ -f "$dest" ]] && [[ "$(sha256_of "$dest")" == "$want" ]]; then
    echo "[ok]   $fname (cached, verified)"
    continue
  fi
  echo "[get]  $fname <- $url"
  tmp="$dest.part"
  curl -fSL --retry 3 -o "$tmp" "$url"
  got="$(sha256_of "$tmp")"
  if [[ "$got" != "$want" ]]; then
    rm -f "$tmp"
    echo "[FAIL] $fname sha mismatch: got $got want $want" >&2
    exit 1
  fi
  mv "$tmp" "$dest"
  echo "[done] $fname (verified)"
done

echo "OCR models ready in $DIR"
