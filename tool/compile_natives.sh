#!/bin/bash
# Compile pdf_oxide for all native targets.
# Run from package root: ./tool/compile_natives.sh
#
# Requires:
#   - Rust toolchain with cross-compilation targets installed
#   - Android NDK (for Android targets)
#
# macOS can cross-compile for: macOS, iOS, Android
# Linux CI compiles for: Linux
# Windows CI compiles for: Windows
# This script auto-detects what it can build on the current host.

set -euo pipefail

MANIFEST="vendor/pdf_oxide/Cargo.toml"
OUT="${COMPILE_OUTPUT_DIR:-build_output}"

if [ ! -f "$MANIFEST" ]; then
  echo "Error: run from package root (vendor/pdf_oxide/Cargo.toml not found)"
  exit 1
fi

FEATURES="icc,legacy-crypto,rendering,signatures,native-bridge"

build() {
  local target=$1
  local outdir=$2
  local libname=$3

  echo "=== Building $target ==="
  cargo build --manifest-path "$MANIFEST" --lib --release --target "$target" --features "$FEATURES"

  mkdir -p "$OUT/$outdir"
  cp "vendor/pdf_oxide/target/$target/release/$libname" "$OUT/$outdir/"
  echo "  → $OUT/$outdir/$libname ($(du -h "$OUT/$outdir/$libname" | cut -f1))"
}

build_android() {
  local target=$1
  local outdir=$2
  local clang_prefix=$3

  local ndk="${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk/28.0.12433566}"
  local toolchain="$ndk/toolchains/llvm/prebuilt"

  # Auto-detect host prebuilt dir
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
  build "$target" "$outdir" "libpdf_oxide.so"
  unset "$linker_var"
}

# macOS
if [[ "$(uname)" == "Darwin" ]]; then
  build "aarch64-apple-darwin" "macos-arm64" "libpdf_oxide.dylib"
  build "x86_64-apple-darwin"  "macos-x64"   "libpdf_oxide.dylib"

  # iOS
  build "aarch64-apple-ios"     "ios-arm64"     "libpdf_oxide.a"
  build "aarch64-apple-ios-sim" "ios-sim-arm64" "libpdf_oxide.a"
  build "x86_64-apple-ios"      "ios-sim-x64"   "libpdf_oxide.a"

  # Strip iOS debug symbols
  strip -S "$OUT/ios-arm64/libpdf_oxide.a" 2>/dev/null || true
  strip -S "$OUT/ios-sim-arm64/libpdf_oxide.a" 2>/dev/null || true
  strip -S "$OUT/ios-sim-x64/libpdf_oxide.a" 2>/dev/null || true
fi

# Linux
if [[ "$(uname)" == "Linux" ]]; then
  build "x86_64-unknown-linux-gnu"  "linux-x64"    "libpdf_oxide.so"
  build "aarch64-unknown-linux-gnu" "linux-arm64"   "libpdf_oxide.so"
fi

# Windows (from Linux CI with mingw, or on Windows with MSVC)
if command -v x86_64-w64-mingw32-gcc &>/dev/null; then
  build "x86_64-pc-windows-gnu" "windows-x64" "pdf_oxide.dll"
elif [[ "$(uname -s)" == MINGW* ]] || [[ "$(uname -s)" == MSYS* ]]; then
  build "x86_64-pc-windows-msvc" "windows-x64" "pdf_oxide.dll"
  build "aarch64-pc-windows-msvc" "windows-arm64" "pdf_oxide.dll"
fi

# Android (needs NDK)
if [ -d "${ANDROID_NDK_HOME:-$HOME/Library/Android/sdk/ndk}" ]; then
  build_android "aarch64-linux-android"   "android-arm64" "aarch64-linux-android"
  build_android "armv7-linux-androideabi" "android-arm"   "armv7a-linux-androideabi"
  build_android "x86_64-linux-android"    "android-x64"   "x86_64-linux-android"
  build_android "i686-linux-android"      "android-x86"   "i686-linux-android"
fi

echo ""
echo "=== Summary ==="
du -sh "$OUT"/*/ 2>/dev/null || echo "(no outputs)"
echo ""
echo "Done. Push a version bump to main — release.yml uploads to GitHub Releases."
