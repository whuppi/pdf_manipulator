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

unzip -o "$APK" 'lib/*' -d "$TMP" >/dev/null 2>&1

FAIL=0
CHECKED=0

for so in $(find "$TMP/lib" \( -path '*/arm64-v8a/*.so' -o -path '*/x86_64/*.so' \) 2>/dev/null); do
  align=$(objdump -p "$so" 2>/dev/null | grep LOAD | awk '{ print $NF }' | head -1)
  name=$(basename "$so")
  arch=$(basename "$(dirname "$so")")
  CHECKED=$((CHECKED + 1))

  if echo "$align" | grep -qE '2\*\*(1[4-9]|[2-9][0-9])'; then
    echo "  ALIGNED: $arch/$name ($align)"
  else
    echo "  UNALIGNED: $arch/$name ($align)"
    FAIL=1
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
