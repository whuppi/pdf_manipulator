#!/usr/bin/env bash
# Verify 16 KB ELF page alignment in an Android APK.
# Google Play requires arm64-v8a and x86_64 .so files to have
# PT_LOAD segments aligned to 2**14 (16384) or higher.
# https://developer.android.com/guide/practices/page-sizes
#
# Uses NDK's llvm-objdump — Google's recommended cross-platform
# tool for this check. Works on Linux, macOS, and Windows.
#
# Usage: check_alignment.sh <path-to-apk>

set -euo pipefail

# shellcheck source=tool/lib.sh
source "$(cd "$(dirname "$0")" && pwd)/lib.sh"

APK="${1:?Usage: check_alignment.sh <path-to-apk>}"

if [ ! -f "$APK" ]; then
  echo "Error: $APK not found" >&2
  exit 1
fi

# Locate NDK's llvm-objdump.
ANDROID_NDK_DIR="${ANDROID_NDK:-${ANDROID_NDK_HOME:-}}"
if [ -z "$ANDROID_NDK_DIR" ] && [ -n "${ANDROID_HOME:-}" ]; then
  ANDROID_NDK_DIR=$(latest_version_subdir "$ANDROID_HOME/ndk")
fi

OBJDUMP=""
if [ -n "$ANDROID_NDK_DIR" ]; then
  OBJDUMP=$(find "$ANDROID_NDK_DIR/toolchains/llvm/prebuilt" \
    -name 'llvm-objdump' -o -name 'llvm-objdump.exe' 2>/dev/null | head -1)
fi

if [ -z "$OBJDUMP" ]; then
  echo "Error: NDK llvm-objdump not found. Set ANDROID_NDK or install the NDK." >&2
  exit 1
fi

echo "=== Verify: 16 KB ELF alignment ==="
echo "  tool: $OBJDUMP"

# Extract native libs from APK.
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

if command -v unzip &>/dev/null; then
  unzip -o "$APK" 'lib/*' -d "$TMP" >/dev/null 2>&1 || true
elif command -v 7z &>/dev/null; then
  7z x "-o$TMP" "$APK" lib/ -y >/dev/null 2>&1 || true
else
  echo "Error: no unzip or 7z available to extract APK" >&2
  exit 1
fi

FAIL=0
CHECKED=0

# Read via process substitution, not `for $(find)`: keeps the loop in this
# shell so FAIL/CHECKED survive, and handles odd filenames safely.
while IFS= read -r so; do
  name=$(basename "$so")
  arch=$(basename "$(dirname "$so")")
  CHECKED=$((CHECKED + 1))

  # Google's recommended check: llvm-objdump -p | grep LOAD
  # Alignment shown as 2**N. 2**14 = 16384 (16 KB) or higher passes.
  align=$("$OBJDUMP" -p "$so" 2>/dev/null | grep LOAD | awk '{ print $NF }' | head -1)

  if grep -qE '2\*\*(1[4-9]|[2-9][0-9])' <<< "$align"; then
    echo "  ALIGNED: $arch/$name ($align)"
  else
    echo "  UNALIGNED: $arch/$name ($align)"
    FAIL=1
  fi
done < <(find "$TMP/lib" \( -path '*/arm64-v8a/*.so' -o -path '*/x86_64/*.so' \) 2>/dev/null)

if [ "$CHECKED" -eq 0 ]; then
  echo "  No arm64-v8a or x86_64 .so files found in APK"
  exit 0
fi

if [ "$FAIL" -eq 1 ]; then
  echo "Error: 16 KB alignment check failed — Google Play rejects < 16 KB"
  exit 1
fi

echo "All $CHECKED libraries aligned"
