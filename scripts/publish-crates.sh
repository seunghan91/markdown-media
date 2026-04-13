#!/bin/bash
# Publish mdm-core to crates.io
# Prerequisites: cargo login <token>
set -e

cd "$(dirname "$0")/../core"

echo "Running tests..."
cargo test --lib

echo ""
echo "Publishing to crates.io..."
cargo publish --dry-run
echo ""
echo "Dry run passed. Run 'cargo publish' to actually publish."
