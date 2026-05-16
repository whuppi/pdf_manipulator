#!/bin/bash
# Build pdf_oxide WASM binary for web targets.
#
# Run manually when vendor/pdf_oxide is bumped or patches change.
# Output goes to web_assets/ and is committed in git.
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.121

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
VENDOR="$PKG_ROOT/vendor/pdf_oxide"
OUT="$PKG_ROOT/web_assets"

echo "Building pdf_oxide WASM..."
cd "$VENDOR"

cargo build --lib \
  --target wasm32-unknown-unknown \
  --features wasm \
  --no-default-features \
  --release

echo "Running wasm-bindgen..."
wasm-bindgen \
  --target web \
  --out-dir "$OUT" \
  --out-name pdf_oxide \
  target/wasm32-unknown-unknown/release/pdf_oxide.wasm

echo ""
echo "Output:"
ls -lh "$OUT"/pdf_oxide*
echo ""
echo "Done. Commit web_assets/ to ship the WASM binary."
