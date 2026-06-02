#!/bin/bash
# Compile pdf_oxide for any target: native platforms, WASM, or both.
#
# Usage:
#   ./tool/compile_rust.sh native       Compile all native targets for this host
#   ./tool/compile_rust.sh wasm         Compile WASM + wasm-bindgen + wasm-opt
#   ./tool/compile_rust.sh all          Both
#   ./tool/compile_rust.sh --features   Print feature flags (used by Makefile)
#
# Run from package root. Auto-detects what native targets the host can build.
#
# Prerequisites:
#   - Rust toolchain with targets installed
#   - WASM: rustup target add wasm32-unknown-unknown
#   - WASM: cargo install wasm-bindgen-cli; brew/apt install binaryen
#   - Android: ANDROID_NDK_HOME set

set -euo pipefail

# ── Feature flags (single source of truth) ──────────────────────────

NATIVE_FEATURES="icc,legacy-crypto,rendering,signatures,native-bridge"
WASM_FEATURES="wasm,rendering"

# Query mode — Makefile calls this to get features without building.
if [[ "${1:-}" == "--features" ]]; then
  case "${2:-all}" in
    native) echo "$NATIVE_FEATURES" ;;
    wasm)   echo "$WASM_FEATURES" ;;
    *)      echo "NATIVE=$NATIVE_FEATURES"; echo "WASM=$WASM_FEATURES" ;;
  esac
  exit 0
fi

# ── Paths ───────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
MANIFEST="$PKG_ROOT/vendor/pdf_oxide/Cargo.toml"
VENDOR="$PKG_ROOT/vendor/pdf_oxide"

if [ ! -f "$MANIFEST" ]; then
  echo "Error: run from package root (vendor/pdf_oxide/Cargo.toml not found)"
  exit 1
fi

MODE="${1:-all}"

# ═════════════════════════════════════════════════════════════════════
# NATIVE
# ═════════════════════════════════════════════════════════════════════

compile_native() {
  local target=$1 outdir=$2 libname=$3
  local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"

  echo "=== Native: $target ==="
  cargo build --manifest-path "$MANIFEST" --lib --release \
    --target "$target" --features "$NATIVE_FEATURES"

  mkdir -p "$out/$outdir"
  cp "$VENDOR/target/$target/release/$libname" "$out/$outdir/"
  echo "  → $out/$outdir/$libname ($(du -h "$out/$outdir/$libname" | cut -f1))"
}

compile_android() {
  local target=$1 outdir=$2 clang_prefix=$3
  local ndk="${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/28.0.12433566}"
  local toolchain="$ndk/toolchains/llvm/prebuilt"

  local host_dir
  if [ -d "$toolchain/darwin-x86_64" ]; then
    host_dir="$toolchain/darwin-x86_64"
  elif [ -d "$toolchain/linux-x86_64" ]; then
    host_dir="$toolchain/linux-x86_64"
  else
    echo "  ⚠ Skipping $target (no NDK toolchain found)"
    return
  fi

  local linker_var="CARGO_TARGET_$(echo "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  export "$linker_var=$host_dir/bin/${clang_prefix}21-clang"
  compile_native "$target" "$outdir" "libpdf_oxide.so"
  unset "$linker_var"
}

do_native() {
  # macOS + iOS
  if [[ "$(uname)" == "Darwin" ]]; then
    compile_native "aarch64-apple-darwin" "macos-arm64" "libpdf_oxide.dylib"
    compile_native "x86_64-apple-darwin"  "macos-x64"   "libpdf_oxide.dylib"
    compile_native "aarch64-apple-ios"     "ios-arm64"     "libpdf_oxide.a"
    compile_native "aarch64-apple-ios-sim" "ios-sim-arm64" "libpdf_oxide.a"
    compile_native "x86_64-apple-ios"      "ios-sim-x64"   "libpdf_oxide.a"
    local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"
    strip -S "$out/ios-arm64/libpdf_oxide.a" 2>/dev/null || true
    strip -S "$out/ios-sim-arm64/libpdf_oxide.a" 2>/dev/null || true
    strip -S "$out/ios-sim-x64/libpdf_oxide.a" 2>/dev/null || true
  fi

  # Linux
  if [[ "$(uname)" == "Linux" ]]; then
    compile_native "x86_64-unknown-linux-gnu"  "linux-x64"  "libpdf_oxide.so"
    compile_native "aarch64-unknown-linux-gnu" "linux-arm64" "libpdf_oxide.so"
  fi

  # Windows
  if command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    compile_native "x86_64-pc-windows-gnu" "windows-x64" "pdf_oxide.dll"
  elif [[ "$(uname -s)" == MINGW* ]] || [[ "$(uname -s)" == MSYS* ]]; then
    compile_native "x86_64-pc-windows-msvc" "windows-x64" "pdf_oxide.dll"
    compile_native "aarch64-pc-windows-msvc" "windows-arm64" "pdf_oxide.dll"
  fi

  # Android
  if [ -d "${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk}" ]; then
    compile_android "aarch64-linux-android"   "android-arm64" "aarch64-linux-android"
    compile_android "armv7-linux-androideabi" "android-arm"   "armv7a-linux-androideabi"
    compile_android "x86_64-linux-android"    "android-x64"   "x86_64-linux-android"
    compile_android "i686-linux-android"      "android-x86"   "i686-linux-android"
  fi

  echo ""
  local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"
  echo "=== Native summary ==="
  du -sh "$out"/*/ 2>/dev/null || echo "(no outputs)"
}

# ═════════════════════════════════════════════════════════════════════
# WASM
# ═════════════════════════════════════════════════════════════════════

# wasm-opt feature flags (Rust's wasm32 target enables these by default;
# wasm-opt must be told about them or it rejects the binary).
WASM_OPT_FLAGS=(
  --enable-bulk-memory
  --enable-multivalue
  --enable-mutable-globals
  --enable-nontrapping-float-to-int
  --enable-reference-types
  --enable-sign-ext
  --enable-simd
)

do_wasm() {
  local out="$PKG_ROOT/web_assets"

  echo "=== WASM: compile ==="
  cd "$VENDOR"
  cargo build --lib \
    --target wasm32-unknown-unknown \
    --features "$WASM_FEATURES" \
    --no-default-features \
    --release

  echo "=== WASM: wasm-bindgen ==="
  wasm-bindgen \
    --target web \
    --out-dir "$out" \
    --out-name pdf_oxide \
    target/wasm32-unknown-unknown/release/pdf_oxide.wasm
  rm -f "$out"/*.d.ts

  echo "=== WASM: optimize ==="
  wasm-opt -O2 \
    "${WASM_OPT_FLAGS[@]}" \
    "$out/pdf_oxide_bg.wasm" \
    -o "$out/pdf_oxide_bg.wasm"

  echo ""
  echo "=== WASM summary ==="
  ls -lh "$out"/pdf_oxide*
  echo ""
  echo "Binary: $(wc -c < "$out/pdf_oxide_bg.wasm") bytes"
  echo ""
  echo "Done. Commit web_assets/ to ship the WASM binary."
}

# ═════════════════════════════════════════════════════════════════════
# Dispatch
# ═════════════════════════════════════════════════════════════════════

case "$MODE" in
  native) do_native ;;
  wasm)   do_wasm ;;
  all)    do_native; do_wasm ;;
  *)      echo "Usage: $0 {native|wasm|all|--features [native|wasm]}"; exit 1 ;;
esac
