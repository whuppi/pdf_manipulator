#!/bin/bash
# Build pdf_oxide WASM binary for web targets.
#
# Produces ONE binary: pdf_oxide_bg.wasm
# Used by all three I/O modes (OPFS, Atomics, JSPI).
# JSPI uses the normal binary — no binary modification needed.
#
# Run manually when vendor/pdf_oxide is bumped or patches change.
# Output goes to web_assets/ and is committed in git.
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli
#   cargo install wasm-opt   (or install binaryen via brew/apt)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
VENDOR="$PKG_ROOT/vendor/pdf_oxide"
OUT="$PKG_ROOT/web_assets"

# Rust's wasm32-unknown-unknown target enables these features by default.
# wasm-opt must be told about them or it rejects the binary.
WASM_FEATURES=(
  --enable-bulk-memory
  --enable-multivalue
  --enable-mutable-globals
  --enable-nontrapping-float-to-int
  --enable-reference-types
  --enable-sign-ext
  --enable-simd
)

# ── Step 1: Compile Rust to WASM ────────────────────────────────────

echo "=== Building pdf_oxide WASM ==="
cd "$VENDOR"

cargo build --lib \
  --target wasm32-unknown-unknown \
  --features wasm,rendering \
  --no-default-features \
  --release

# ── Step 2: wasm-bindgen (generate JS glue) ─────────────────────────

echo "=== Running wasm-bindgen ==="
wasm-bindgen \
  --target web \
  --out-dir "$OUT" \
  --out-name pdf_oxide \
  target/wasm32-unknown-unknown/release/pdf_oxide.wasm

# Remove TypeScript type definitions (wasm-bindgen emits them, we don't use them)
rm -f "$OUT"/*.d.ts

# ── Step 3: Optimize binary ─────────────────────────────────────────

echo "=== Optimizing WASM binary ==="
wasm-opt -O2 \
  "${WASM_FEATURES[@]}" \
  "$OUT/pdf_oxide_bg.wasm" \
  -o "$OUT/pdf_oxide_bg.wasm"

# ── Done ────────────────────────────────────────────────────────────

echo ""
echo "=== Output ==="
ls -lh "$OUT"/pdf_oxide*
echo ""
echo "Binary: $(wc -c < "$OUT/pdf_oxide_bg.wasm") bytes"
echo ""
echo "Done. Commit web_assets/ to ship the WASM binary."
