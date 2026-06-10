#!/usr/bin/env bash
# Verify 16 KB ELF page alignment in an Android APK.
# Google Play requires arm64-v8a and x86_64 .so files to have
# PT_LOAD segments aligned to 2**14 (16384) or higher.
# https://developer.android.com/guide/practices/page-sizes
#
# Usage: check_alignment.sh <path-to-apk>

set -euo pipefail

APK="${1:?Usage: check_alignment.sh <path-to-apk>}"

if [ ! -f "$APK" ]; then
  echo "Error: $APK not found" >&2
  exit 1
fi

echo "=== Verify: 16 KB ELF alignment ==="

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Extract native libs from APK.
if command -v unzip &>/dev/null; then
  unzip -o "$APK" 'lib/*' -d "$TMP" >/dev/null 2>&1
elif command -v 7z &>/dev/null; then
  7z x -o"$TMP" "$APK" 'lib/*' >/dev/null 2>&1
else
  echo "Error: no unzip or 7z available to extract APK" >&2
  exit 1
fi

# Find a tool that can read ELF headers.
# NDK's llvm-readelf handles cross-arch; host objdump may not.
READELF=""
if command -v llvm-readelf &>/dev/null; then
  READELF="llvm-readelf"
elif [ -n "${ANDROID_NDK:-}" ]; then
  NDK_READELF=$(find "$ANDROID_NDK/toolchains/llvm/prebuilt" -name 'llvm-readelf' -o -name 'llvm-readelf.exe' 2>/dev/null | head -1)
  [ -n "$NDK_READELF" ] && READELF="$NDK_READELF"
fi

if [ -n "$READELF" ]; then
  # llvm-readelf: check p_align of LOAD segments directly.
  check_align() {
    "$READELF" -l "$1" 2>/dev/null | awk '/LOAD/{print $NF}' | while read -r align; do
      if [ "$((align))" -lt 16384 ] 2>/dev/null; then echo "BAD"; return; fi
    done
  }
elif command -v objdump &>/dev/null; then
  # GNU objdump: alignment shown as 2**N.
  check_align() {
    local align
    align=$(objdump -p "$1" 2>/dev/null | grep LOAD | awk '{ print $NF }' | head -1)
    if ! echo "$align" | grep -qE '2\*\*(1[4-9]|[2-9][0-9])'; then echo "BAD"; fi
  }
else
  echo "Error: no readelf or objdump available to check ELF alignment" >&2
  exit 1
fi

FAIL=0
CHECKED=0

for so in $(find "$TMP/lib" \( -path '*/arm64-v8a/*.so' -o -path '*/x86_64/*.so' \) 2>/dev/null); do
  name=$(basename "$so")
  arch=$(basename "$(dirname "$so")")
  CHECKED=$((CHECKED + 1))

  if [ -n "$(check_align "$so")" ]; then
    echo "  UNALIGNED: $arch/$name"
    FAIL=1
  else
    echo "  ALIGNED: $arch/$name"
  fi
done

if [ "$CHECKED" -eq 0 ]; then
  echo "  No arm64-v8a or x86_64 .so files found in APK"
  exit 0
fi

if [ "$FAIL" -eq 1 ]; then
  echo "Error: 16 KB alignment check failed — Google Play rejects < 16 KB"
  exit 1
fi

echo "All $CHECKED libraries aligned"
