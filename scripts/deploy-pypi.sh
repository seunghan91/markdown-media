#!/bin/bash
# PyPI Deployment Script

set -e

echo "🐍 MDM Python Package Deployment"
echo "================================="
echo ""

# Check if twine is installed
if ! command -v twine &> /dev/null; then
    echo "⚠️  twine not installed"
    echo "Installing: pip install twine build"
    pip install twine build
fi

# Build package
echo "📦 Building Python package..."
cd packages/parser-py

# Clean previous builds
rm -rf dist/ build/ *.egg-info

# Build
python -m build

echo "✓ Package built"
echo ""

# Check package
echo "🔍 Checking package..."
twine check dist/*

echo ""
echo "Ready to upload to PyPI"
echo ""
echo "Test PyPI (recommended first):"
echo "  twine upload --repository testpypi dist/*"
echo ""
echo "Production PyPI:"
echo "  twine upload dist/*"
echo ""

read -p "Upload to PyPI now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    twine upload dist/*
    echo "✓ Published to PyPI"
fi

cd ../..
