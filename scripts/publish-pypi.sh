#!/bin/bash
# Publish mdm-core to PyPI
# Prerequisites: MATURIN_PYPI_TOKEN env var set, or ~/.pypirc configured
set -e

cd "$(dirname "$0")/../packages/python"

echo "Building wheel..."
python3 -m maturin build --release

echo ""
echo "Publishing to PyPI..."
python3 -m maturin publish --skip-existing

echo ""
echo "Done! Install with: pip install mdm-core"
