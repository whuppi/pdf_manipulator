#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# check_keep_sets.sh — compiles every valid keep-set against the vendored
# engine, for one target or both.
#
#   bash tool/check_keep_sets.sh [native|wasm|all]
#
# Every keep-list a user can write must compile, not only the full default
# set the other build checks use. `tool/keep_sets.dart` is the one home of the valid keep-set list
# (the requires graph lives in capabilities.dart); this script feeds each
# line to `cargo check`.
#
# Native checks `--profile test` so `#[cfg(test)]` code (unit tests and the
# trim probes) is checked too, not just the shipped lib. Wasm checks the
# library alone: upstream's own unit tests call its path-based `open`, which
# is compiled out on wasm32, so they never compile — or run — there.
# `--no-default-features` because the feature string from keep_sets.dart is
# authoritative — same as the real compile in engine_compiler.dart, so a set
# that passes here compiles for a consumer the same way.
#
# Env:
#   DART   SDK command — defaults to "fvm dart", like the Makefile.
# The 48 builds go to their own target dir, `vendor/pdf_oxide/target/keep-sets`,
# so they never mix with the test build's artifacts. When the run ends, pass or
# fail, the script removes that dir's `incremental` folders and every file
# older than the run's start, so the dir holds only this run's output.
# Run from the package root.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PKG_ROOT"

# shellcheck source=tool/build_lib.sh
source "$SCRIPT_DIR/build_lib.sh"

KEEP_SETS_TARGET="${PKG_ROOT:?}/vendor/pdf_oxide/target/keep-sets"
export CARGO_TARGET_DIR="$KEEP_SETS_TARGET"

DART="${DART:-fvm dart}"
TARGET="${1:-all}"

case "$TARGET" in
  native|wasm|all) ;;
  *)
    echo "usage: $0 [native|wasm|all]" >&2
    exit 64
    ;;
esac

WORKDIR="$(mktemp -d)"
RUN_START="$WORKDIR/run-start"
touch "$RUN_START"

# Runs on every exit, so a failed or interrupted run leaves no more behind than
# a passing one: the log dir goes, and the keep-sets target keeps only the
# files this run wrote, without the incremental caches.
cleanup() {
  if [ -d "$KEEP_SETS_TARGET" ]; then
    find "${KEEP_SETS_TARGET:?}" -type d -name incremental -prune -exec rm -rf {} +
    find "${KEEP_SETS_TARGET:?}" -type f ! -newer "$RUN_START" \
      ! -name CACHEDIR.TAG ! -name .rustc_info.json -delete
    find "${KEEP_SETS_TARGET:?}" -mindepth 1 -type d -empty -delete
    echo "keep-sets target: $(($(du -sk "$KEEP_SETS_TARGET" | cut -f1) / 1024)) MiB after pruning"
  fi
  rm -rf "${WORKDIR:?}"
}
trap cleanup EXIT

FAILED=0
TOTAL=0
FAIL_LOGS=()

# One `cargo check` of the vendored crate: target, features, log file.
cargo_check() {
  local manifest="$PKG_ROOT/vendor/pdf_oxide/Cargo.toml"
  if [ "$1" = wasm ]; then
    cargo check --lib --manifest-path "$manifest" --target wasm32-unknown-unknown \
      --no-default-features --features "$2" > "$3" 2>&1
  else
    cargo check --lib --manifest-path "$manifest" --profile test \
      --no-default-features --features "$2" > "$3" 2>&1
  fi
}

# Runs one target's whole lattice. Reads the line list from
# tool/keep_sets.dart, checks each against the vendored crate.
check_target() {
  local target="$1"
  if [ "$target" = wasm ]; then
    ensure_target wasm32-unknown-unknown
  fi

  # The list is captured first, so a failing generator stops the run
  # instead of feeding the loop nothing and passing with zero sets.
  local sets
  sets=$($DART tool/keep_sets.dart "$target")
  if [ -z "$sets" ]; then
    echo "tool/keep_sets.dart printed no keep-sets for $target" >&2
    exit 1
  fi

  local line keep features start end elapsed log status
  while IFS= read -r line; do
    keep="${line%%$'\t'*}"
    features="${line#*$'\t'}"
    TOTAL=$((TOTAL + 1))
    log="$WORKDIR/$target-$TOTAL.log"
    start=$(date +%s)
    if cargo_check "$target" "$features" "$log"; then
      status=ok
    else
      status=FAIL
      FAILED=$((FAILED + 1))
      FAIL_LOGS+=("$log|$target|$keep")
    fi
    end=$(date +%s)
    elapsed=$((end - start))
    echo "$status  $target  $keep  ${elapsed}s"
  done <<< "$sets"
}

if [ "$TARGET" = native ] || [ "$TARGET" = all ]; then
  check_target native
fi
if [ "$TARGET" = wasm ] || [ "$TARGET" = all ]; then
  check_target wasm
fi

if [ "$FAILED" -gt 0 ]; then
  echo ""
  for entry in "${FAIL_LOGS[@]}"; do
    log="${entry%%|*}"
    rest="${entry#*|}"
    target="${rest%%|*}"
    keep="${rest#*|}"
    echo "── FAIL $target $keep ──"
    grep -A1 '^error' "$log" || true
  done
  echo ""
  echo "$FAILED of $TOTAL keep-sets failed"
  exit 1
fi

echo "all $TOTAL keep-sets compile"
