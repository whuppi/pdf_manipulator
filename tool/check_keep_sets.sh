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
# Run from the package root.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PKG_ROOT"

# shellcheck source=tool/build_lib.sh
source "$SCRIPT_DIR/build_lib.sh"

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
trap 'rm -rf "$WORKDIR"' EXIT

FAILED=0
TOTAL=0
FAIL_LOGS=()

# Runs one target's whole lattice. Reads the line list from `dart run
# tool/keep_sets.dart`, checks each against the vendored crate.
check_target() {
  local target="$1"
  local wasm_flag="" profile_flag="--profile test"
  if [ "$target" = wasm ]; then
    ensure_target wasm32-unknown-unknown
    wasm_flag="--target wasm32-unknown-unknown"
    profile_flag=""
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
    # shellcheck disable=SC2086 # wasm_flag/profile_flag are caller-controlled flags, split on purpose.
    if cargo check --lib \
        --manifest-path "$PKG_ROOT/vendor/pdf_oxide/Cargo.toml" \
        --no-default-features --features "$features" \
        $profile_flag $wasm_flag > "$log" 2>&1; then
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
