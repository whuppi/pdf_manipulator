#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# prune_target.sh — bounds the cargo target dirs the gate builds into.
#
#   bash tool/prune_target.sh
#
# Removes, from each target dir below:
#   - every `incremental` folder (a rebuild cache, never needed to run);
#   - every cargo unit none of whose files changed for PRUNE_DAYS days.
#     A unit is everything one build step left under one hash in a
#     profile's `.fingerprint`, `build`, `deps` and `examples`, and it goes
#     whole or not at all: a fingerprint left behind without its library
#     tells cargo the crate is built, and rustc then fails with "can't
#     find crate". A pruned unit is rebuilt on the next build, so the
#     cost is time, never a wrong result;
#   - every other file not modified for PRUNE_DAYS days, then the folders
#     that emptied.
# The target dirs are the three vendored crates' `target/`, which holds the
# test build and keep-sets' own `target/keep-sets/`, and the memory probe's
# `target/memprobe` and `tool/memprobe/target`.
#
# Run it only when no build is in flight: `make test-rust` and
# `make check-keep-sets` call it when they end, pass or fail. In CI it
# does nothing: the runner is thrown away, the target dir is saved as the
# next run's cache, and a restored cache carries the dates of the run that
# saved it, so pruning by age there throws away a warm cache.
#
# Env:
#   PRUNE_DAYS  age in days past which an untouched file goes; default 2,
#               0 removes everything older than a minute.
# Run from the package root.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

if [ -n "${CI:-}" ]; then
  echo "prune-target: skipped in CI"
  exit 0
fi

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

# The folders of a cargo profile that hold units, each entry named
# `<name>-<16 hex hash>[.<ext>]`.
UNIT_DIRS=(.fingerprint build deps examples)

# Size of a folder in MiB.
size_mib() {
  echo $(($(du -sk "$1" | cut -f1) / 1024))
}

# Removes the units of one profile folder that have no file newer than the
# cutoff. Lists the fresh files and the top-level entries in one pass each,
# then deletes every entry whose hash no fresh file carries.
prune_units() {
  local profile=$1
  (
    cd "$profile"
    find "${UNIT_DIRS[@]}" -mindepth 1 -mmin "-$MINUTES" 2>/dev/null | sed 's/^/F /' || true
    find "${UNIT_DIRS[@]}" -mindepth 1 -maxdepth 1 2>/dev/null | sed 's/^/E /' || true
  ) | awk '
    function unit(path,   parts, name) {
      split(path, parts, "/")
      name = parts[2]
      if (match(name, /-[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f](\.|$)/))
        return substr(name, RSTART + 1, 16)
      return ""
    }
    /^F / { h = unit(substr($0, 3)); if (h != "") fresh[h] = 1; next }
    /^E / { path = substr($0, 3); h = unit(path); if (h != "" && !(h in fresh)) print path }
  ' | while IFS= read -r entry; do
    rm -rf "${profile:?}/${entry:?}"
  done
}

total_before=0
total_after=0
for rel in "${TARGETS[@]}"; do
  dir="${PKG_ROOT:?}/${rel:?}"
  [ -d "$dir" ] || continue
  before=$(size_mib "$dir")
  find "${dir:?}" -type d -name incremental -prune -exec rm -rf {} +
  while IFS= read -r fingerprints; do
    prune_units "$(dirname "$fingerprints")"
  done < <(find "${dir:?}" -type d -name .fingerprint -prune)
  # A unit's files are left to prune_units above, whatever their age.
  find "${dir:?}" -type f -mmin "+$MINUTES" \
    ! -path '*/.fingerprint/*' ! -path '*/build/*' \
    ! -path '*/deps/*' ! -path '*/examples/*' \
    ! -name CACHEDIR.TAG ! -name .rustc_info.json -delete
  find "${dir:?}" -mindepth 1 -type d -empty -delete
  after=$(size_mib "$dir")
  total_before=$((total_before + before))
  total_after=$((total_after + after))
  echo "prune-target: $rel ${before} MiB -> ${after} MiB"
done
echo "prune-target: total ${total_before} MiB -> ${total_after} MiB (units and files untouched for ${DAYS} days, every incremental folder)"
