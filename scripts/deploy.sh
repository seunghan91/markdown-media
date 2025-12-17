#!/bin/bash
# MDM Project Deployment Script

set -e

echo "🚀 MDM Deployment Script"
echo "========================"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if logged in to npm
echo "Checking npm authentication..."
if ! npm whoami &> /dev/null; then
    echo -e "${YELLOW}⚠️  Not logged in to npm${NC}"
    echo "Run: npm login"
    echo "Use account: beasthan2025"
    exit 1
fi

CURRENT_USER=$(npm whoami)
echo -e "${GREEN}✓${NC} Logged in as: $CURRENT_USER"
echo ""

# Step 1: Test packages
echo "📦 Step 1: Running tests..."
echo ""

cd packages/parser-js
echo "Testing @mdm/parser..."
npm test
cd ../..

echo -e "${GREEN}✓${NC} All tests passed"
echo ""

# Step 2: Build Rust core
echo "🦀 Step 2: Building Rust core..."
cd core
cargo build --release
cd ..
echo -e "${GREEN}✓${NC} Rust core built"
echo ""

# Step 3: Build WASM parser (optional, if wasm-pack installed)
if command -v wasm-pack &> /dev/null; then
    echo "🌐 Step 3: Building WASM parser..."
    cd packages/parser-rs
    wasm-pack build --target web
    cd ../..
    echo -e "${GREEN}✓${NC} WASM built"
else
    echo -e "${YELLOW}⚠️  wasm-pack not installed, skipping WASM build${NC}"
    echo "Install: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
fi
echo ""

# Step 4: Publish npm packages
echo "📤 Step 4: Publishing npm packages..."
echo ""

read -p "Publish @mdm/parser? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cd packages/parser-js
    npm publish --access public
    echo -e "${GREEN}✓${NC} Published @mdm/parser"
    cd ../..
fi

read -p "Publish @mdm/cli? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    cd cli
    npm publish --access public
    echo -e "${GREEN}✓${NC} Published @mdm/cli"
    cd ..
fi

echo ""
echo -e "${GREEN}🎉 Deployment complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Update package versions if needed"
echo "2. Create GitHub release"
echo "3. Publish Python package to PyPI (see scripts/deploy-pypi.sh)"
