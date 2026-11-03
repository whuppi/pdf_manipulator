#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# prune_target.sh — bounds the cargo target dirs the gate builds into.
#
#   bash tool/prune_target.sh
#
# Removes, from each target dir below:
#   - every `incremental` folder (a rebuild cache, never needed to run);
#   - every file not modified for PRUNE_DAYS days, then the folders that
#     emptied. A reused artifact that is older than that is rebuilt on the
#     next build, so the cost is time, never a wrong result.
# The target dirs are the three vendored crates' `target/`, which holds the
# test build and keep-sets' own `target/keep-sets/`, and the memory probe's
# `target/memprobe` and `tool/memprobe/target`.
#
# Run it only when no build is in flight: `make test-rust` and
# `make check-keep-sets` call it when they end, pass or fail.
#
# Env:
#   PRUNE_DAYS  age in days past which an untouched file goes; default 2,
#               0 removes everything older than a minute.
# Run from the package root.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PKG_ROOT"

DAYS="${PRUNE_DAYS:-2}"
case "$DAYS" in
  '' | *[!0-9]*)
    echo "PRUNE_DAYS must be a whole number of days, got '$DAYS'" >&2
    exit 64
    ;;
esac
MINUTES=$((DAYS * 1440))

TARGETS=(
  vendor/pdf_oxide/target
  vendor/office_oxide/target
  vendor/tiny_skia/target
  target/memprobe
  tool/memprobe/target
)

# Size of a folder in MiB.
size_mib() {
  echo $(($(du -sk "$1" | cut -f1) / 1024))
}

total_before=0
total_after=0
for rel in "${TARGETS[@]}"; do
  dir="${PKG_ROOT:?}/${rel:?}"
  [ -d "$dir" ] || continue
  before=$(size_mib "$dir")
  find "${dir:?}" -type d -name incremental -prune -exec rm -rf {} +
  find "${dir:?}" -type f -mmin "+$MINUTES" \
    ! -name CACHEDIR.TAG ! -name .rustc_info.json -delete
  find "${dir:?}" -mindepth 1 -type d -empty -delete
  after=$(size_mib "$dir")
  total_before=$((total_before + before))
  total_after=$((total_after + after))
  echo "prune-target: $rel ${before} MiB -> ${after} MiB"
done
echo "prune-target: total ${total_before} MiB -> ${total_after} MiB (files untouched for ${DAYS} days, every incremental folder)"
