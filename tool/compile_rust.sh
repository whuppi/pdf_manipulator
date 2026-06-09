#!/bin/bash
# ────────────────────────────────────────────────────────────────────
# compile_rust.sh — Compile pdf_oxide for any target.
#
# Usage:
#   ./tool/compile_rust.sh macos        macOS arm64 + x64
#   ./tool/compile_rust.sh ios          iOS device + simulators
#   ./tool/compile_rust.sh linux        Linux x64 + arm64 (cross-compile)
#   ./tool/compile_rust.sh android      Android arm64 + arm + x64 + x86
#   ./tool/compile_rust.sh windows      Windows x64 (+ arm64 on MSVC)
#   ./tool/compile_rust.sh wasm         WASM + wasm-bindgen + wasm-opt
#   ./tool/compile_rust.sh native       Auto-detect what this host can build
#   ./tool/compile_rust.sh all          native + wasm
#   ./tool/compile_rust.sh --features   Print feature flags (for Makefile + build.dart)
#
# CI calls the specific target command. Local dev calls native or all.
#
# Prerequisites:
#   Rust toolchain with targets installed (rustup target add ...)
#   WASM: cargo install wasm-bindgen-cli; brew/apt install binaryen
#   Android: ANDROID_NDK_HOME set
#   Linux arm64 cross: apt install gcc-aarch64-linux-gnu
#
# Called by:  Makefile, build.dart, CI compile jobs
# Run from:   package root
# ────────────────────────────────────────────────────────────────────
set -euo pipefail


# ═══════════════════════════════════════════════════════════════════
# Paths
# ═══════════════════════════════════════════════════════════════════

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
MANIFEST="$PKG_ROOT/vendor/pdf_oxide/Cargo.toml"
VENDOR="$PKG_ROOT/vendor/pdf_oxide"

# ═══════════════════════════════════════════════════════════════════
# Feature flags — read from build.json (single source of truth)
# ═══════════════════════════════════════════════════════════════════

# Rust check — hard error if not installed
if ! command -v cargo &>/dev/null; then
  echo "Error: Rust not installed. Get it at https://rustup.rs"
  exit 1
fi

source "$SCRIPT_DIR/lib.sh"

NATIVE_FEATURES=$(_json_get 'native')
WASM_FEATURES=$(_json_get 'wasm')

if [[ "${1:-}" == "--features" ]]; then
  case "${2:-all}" in
    native) echo "$NATIVE_FEATURES" ;;
    wasm)   echo "$WASM_FEATURES" ;;
    *)      echo "NATIVE=$NATIVE_FEATURES"; echo "WASM=$WASM_FEATURES" ;;
  esac
  exit 0
fi

if [ ! -f "$MANIFEST" ]; then
  echo "Error: run from package root (vendor/pdf_oxide/Cargo.toml not found)"
  exit 1
fi

MODE="${1:-native}"


# ═══════════════════════════════════════════════════════════════════
# Native — shared helpers
# ═══════════════════════════════════════════════════════════════════


# Compile one Rust target and copy the library to the output directory.
#   $1 = Rust target triple
#   $2 = output subdirectory name
#   $3 = library filename
compile_one() {
  local target="$1" outdir="$2" libname="$3"
  local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"

  echo "=== Native: $target ==="
  ensure_target "$target"
  cargo build --manifest-path "$MANIFEST" --lib --release \
    --target "$target" --features "$NATIVE_FEATURES"

  mkdir -p "$out/$outdir"
  cp "$VENDOR/target/$target/release/$libname" "$out/$outdir/"
  echo "  → $out/$outdir/$libname ($(du -h "$out/$outdir/$libname" | cut -f1))"
}

# Compile one Android target using the NDK toolchain.
#   $1 = Rust target triple
#   $2 = output subdirectory name
#   $3 = NDK clang prefix (e.g. "aarch64-linux-android")
compile_android_target() {
  local target="$1" outdir="$2" clang_prefix="$3"
  local ndk="${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/28.0.12433566}"
  local toolchain="$ndk/toolchains/llvm/prebuilt"

  local host_dir
  if [ -d "$toolchain/darwin-x86_64" ]; then
    host_dir="$toolchain/darwin-x86_64"
  elif [ -d "$toolchain/linux-x86_64" ]; then
    host_dir="$toolchain/linux-x86_64"
  else
    # In CI, fail hard — silent skip would produce zero Android binaries
    # while the job exits 0, leaving consumers with download 404s.
    if [[ -n "${CI:-}" ]]; then
      echo "ERROR: NDK toolchain not found at $toolchain (darwin-x86_64 or linux-x86_64)"
      exit 1
    fi
    echo "  ⚠ Skipping $target (no NDK toolchain found)"
    return
  fi

  local linker_var="CARGO_TARGET_$(echo "$target" | tr '[:lower:]-' '[:upper:]_')_LINKER"
  export "$linker_var=$host_dir/bin/${clang_prefix}21-clang"
  compile_one "$target" "$outdir" "libpdf_oxide.so"
  unset "$linker_var"
}

native_summary() {
  echo ""
  local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"
  echo "=== Native summary ==="
  du -sh "$out"/*/ 2>/dev/null || echo "(no outputs)"
}


# ═══════════════════════════════════════════════════════════════════
# Target commands — CI calls these directly
# ═══════════════════════════════════════════════════════════════════

do_macos() {
  compile_one "aarch64-apple-darwin" "macos-arm64" "libpdf_oxide.dylib"
  compile_one "x86_64-apple-darwin"  "macos-x64"   "libpdf_oxide.dylib"
  native_summary
}

do_ios() {
  compile_one "aarch64-apple-ios"     "ios-arm64"     "libpdf_oxide.a"
  compile_one "aarch64-apple-ios-sim" "ios-sim-arm64" "libpdf_oxide.a"
  compile_one "x86_64-apple-ios"      "ios-sim-x64"   "libpdf_oxide.a"

  local out="${COMPILE_OUTPUT_DIR:-$PKG_ROOT/build_output}"
  strip -S "$out/ios-arm64/libpdf_oxide.a"     2>/dev/null || true
  strip -S "$out/ios-sim-arm64/libpdf_oxide.a" 2>/dev/null || true
  strip -S "$out/ios-sim-x64/libpdf_oxide.a"   2>/dev/null || true
  native_summary
}

do_linux() {
  compile_one "x86_64-unknown-linux-gnu" "linux-x64" "libpdf_oxide.so"

  # Cross-compiler for arm64
  if ! command -v aarch64-linux-gnu-gcc &>/dev/null; then
    if [ -n "${CI:-}" ]; then
      sudo apt-get update -qq && sudo apt-get install -y -qq gcc-aarch64-linux-gnu
    else
      echo "Error: aarch64-linux-gnu-gcc not found. Run: sudo apt-get install -y gcc-aarch64-linux-gnu"
      exit 1
    fi
  fi

  export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc"
  compile_one "aarch64-unknown-linux-gnu" "linux-arm64" "libpdf_oxide.so"
  unset CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER

  native_summary
}

do_android() {
  compile_android_target "aarch64-linux-android"  "android-arm64" "aarch64-linux-android"
  compile_android_target "armv7-linux-androideabi" "android-arm"   "armv7a-linux-androideabi"
  compile_android_target "x86_64-linux-android"    "android-x64"   "x86_64-linux-android"
  compile_android_target "i686-linux-android"      "android-x86"   "i686-linux-android"
  native_summary
}

do_windows() {
  if [[ "$(uname -s)" == MINGW* ]] || [[ "$(uname -s)" == MSYS* ]]; then
    # MSVC (native Windows) — builds both x64 and arm64
    compile_one "x86_64-pc-windows-msvc"  "windows-x64"   "pdf_oxide.dll"
    compile_one "aarch64-pc-windows-msvc" "windows-arm64"  "pdf_oxide.dll"
  elif command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    # MinGW cross-compile from Linux (x64 only)
    compile_one "x86_64-pc-windows-gnu" "windows-x64" "pdf_oxide.dll"
  else
    echo "⚠ Windows cross-compile not available on this host"
    return
  fi
  native_summary
}


# ═══════════════════════════════════════════════════════════════════
# Auto-detect — local dev convenience
# ═══════════════════════════════════════════════════════════════════
# Builds whatever this host supports. CI never calls this — it uses
# the explicit target commands above.

do_native() {
  if [[ "$(uname)" == "Darwin" ]]; then
    do_macos
    do_ios
  fi

  if [[ "$(uname)" == "Linux" ]]; then
    compile_one "x86_64-unknown-linux-gnu" "linux-x64" "libpdf_oxide.so"
    if command -v aarch64-linux-gnu-gcc &>/dev/null; then
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="aarch64-linux-gnu-gcc"
      compile_one "aarch64-unknown-linux-gnu" "linux-arm64" "libpdf_oxide.so"
      unset CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER
    fi
  fi

  if command -v x86_64-w64-mingw32-gcc &>/dev/null; then
    compile_one "x86_64-pc-windows-gnu" "windows-x64" "pdf_oxide.dll"
  elif [[ "$(uname -s)" == MINGW* ]] || [[ "$(uname -s)" == MSYS* ]]; then
    do_windows
  fi

  if [ -d "${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk}" ]; then
    do_android
  fi

  native_summary
}


# ═══════════════════════════════════════════════════════════════════
# WASM
# ═══════════════════════════════════════════════════════════════════

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

  # Ensure wasm-bindgen-cli matches the Cargo.lock version exactly.
  local wb_required wb_installed
  wb_required=$(grep -A1 'name = "wasm-bindgen"' "$VENDOR/Cargo.lock" \
    | grep version | head -1 | sed 's/.*"\(.*\)"/\1/')
  wb_installed=$(wasm-bindgen --version 2>/dev/null | sed 's/wasm-bindgen //' || echo "none")

  if [[ "$wb_installed" != "$wb_required" ]]; then
    echo "=== WASM: installing wasm-bindgen-cli $wb_required (have: $wb_installed) ==="
    cargo install wasm-bindgen-cli --version "$wb_required"
  fi

  # Ensure wasm-opt (binaryen) is installed.
  if ! command -v wasm-opt &>/dev/null; then
    if [ -n "${CI:-}" ]; then
      echo "=== WASM: installing binaryen (wasm-opt) ==="
      BINARYEN_VER="version_130"
      case "$(uname -s)" in
        Linux*)
          BINARYEN_URL="https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VER/binaryen-$BINARYEN_VER-x86_64-linux.tar.gz"
          ;;
        Darwin*)
          BINARYEN_URL="https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VER/binaryen-$BINARYEN_VER-arm64-macos.tar.gz"
          ;;
        MINGW*|MSYS*)
          BINARYEN_URL="https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VER/binaryen-$BINARYEN_VER-x86_64-windows.tar.gz"
          ;;
      esac
      echo "  $BINARYEN_VER"
      local tmp="${RUNNER_TEMP:-/tmp}"
      # Convert Windows paths (D:\...) to Unix (/d/...) so tar doesn't
      # interpret the colon as a remote host.
      if command -v cygpath &>/dev/null; then
        tmp=$(cygpath -u "$tmp")
      fi
      curl -sSL --fail "$BINARYEN_URL" -o "$tmp/binaryen.tar.gz" \
        || { echo "Error: failed to download binaryen $BINARYEN_VER"; exit 1; }
      tar xzf "$tmp/binaryen.tar.gz" -C "$tmp"
      local cargo_bin="$HOME/.cargo/bin"
      local cargo_lib="$HOME/.cargo/lib"
      mkdir -p "$cargo_bin" "$cargo_lib"
      cp "$tmp/binaryen-$BINARYEN_VER/bin/wasm-opt"* "$cargo_bin/" \
        || { echo "Error: could not copy wasm-opt to $cargo_bin"; exit 1; }
      cp "$tmp/binaryen-$BINARYEN_VER/lib/"* "$cargo_lib/" 2>/dev/null || true
      rm -rf "$tmp/binaryen.tar.gz" "$tmp/binaryen-$BINARYEN_VER"
    else
      echo "Error: wasm-opt not found. Install binaryen:"
      echo "  macOS:   brew install binaryen"
      echo "  Linux:   sudo apt-get install binaryen"
      echo "  Windows: https://github.com/WebAssembly/binaryen/releases"
      exit 1
    fi
  fi

  # Resolve COMPILE_OUTPUT_DIR before cd — relative paths would
  # break after changing to the vendor directory.
  local release_out="${COMPILE_OUTPUT_DIR:+$(cd "$PKG_ROOT" && mkdir -p "$COMPILE_OUTPUT_DIR/wasm" && cd "$COMPILE_OUTPUT_DIR/wasm" && pwd)}"

  echo "=== WASM: compile ==="
  ensure_target wasm32-unknown-unknown
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
  echo "Binary: $(wc -c < "$out/pdf_oxide_bg.wasm") bytes"
  echo ""
  # Copy to COMPILE_OUTPUT_DIR if set (release pipeline uses this).
  # release_out was resolved to absolute path before cd above.
  if [ -n "${release_out:-}" ]; then
    cp "$out/pdf_oxide.js" "$out/pdf_oxide_bg.wasm" "$release_out/"
    echo "Copied to $release_out/"
  fi

  echo "Done. Commit web_assets/ to ship the WASM binary."
}


# ═══════════════════════════════════════════════════════════════════
# Dispatch
# ═══════════════════════════════════════════════════════════════════

case "$MODE" in
  macos)    do_macos ;;
  ios)      do_ios ;;
  linux)    do_linux ;;
  android)  do_android ;;
  windows)  do_windows ;;
  wasm)     do_wasm ;;
  native)   do_native ;;
  all)      do_native; do_wasm ;;
  *)
    echo "Usage: $0 {macos|ios|linux|android|windows|wasm|native|all|--features [native|wasm]}"
    exit 1
    ;;
esac
