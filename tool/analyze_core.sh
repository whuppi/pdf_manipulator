#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# analyze_core.sh — the ONE Dart static-analysis gate, shared verbatim.
#
# Canonical copy: whuppi/ci/tool/analyze_core.sh. Stamped into each
# package as tool/analyze_core.sh by .claude/scripts/stamp-analyze.sh —
# DO NOT edit the stamped copy; edit the canonical one and re-stamp.
#
# What it enforces, identically in every package, locally and in CI:
#   1. No suppression comments in the analyzed source ("// " then
#      "ignore:" / "ignore_for_file:") — every lint is fixed for real.
#   2. `dart analyze --fatal-infos` over the package's root dirs — an
#      INFO (deprecated_member_use, anything) fails, same as an error.
#   3. `flutter analyze --fatal-infos` over example/ when present.
#
# It deliberately does NOT format (packages own their format step) and
# does NOT hard-code package layout: the analyzable root dirs are
# auto-detected, example is auto-detected, and package-only steps
# (Rust, fixtures) live in the package's own wrapper AROUND this core.
#
# Env (all optional):
#   DART / FLUTTER   SDK commands            (default: fvm dart / fvm flutter)
#   ANALYZE_DIRS     root dirs to analyze    (default: the ones that exist
#                    among lib bin test tool hook)
#   EXAMPLE_DIR      example app dir          (default: example if present;
#                    set to "" to skip)
# Run from the package root.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

DART="${DART:-fvm dart}"
FLUTTER="${FLUTTER:-fvm flutter}"

# Auto-detect the analyzable root dirs so one script fits every package
# shape. example/ is deliberately NOT in this list: it is a separate
# Flutter package and gets its own `flutter analyze` pass below (using
# its own resolution), never a root-package `dart analyze`.
if [ -z "${ANALYZE_DIRS:-}" ]; then
  ANALYZE_DIRS=""
  for d in lib bin test tool hook; do
    [ -d "$d" ] && ANALYZE_DIRS="${ANALYZE_DIRS:+$ANALYZE_DIRS }$d"
  done
fi
# ANALYZE_DIRS is a space-separated dir list meant to word-split into args.
# shellcheck disable=SC2086
set -- $ANALYZE_DIRS
if [ "$#" -eq 0 ]; then
  echo "analyze_core: no analyzable dirs found — run from the package root." >&2
  exit 2
fi

if [ -z "${EXAMPLE_DIR+x}" ] && [ -d example ]; then
  EXAMPLE_DIR="example"
fi
EXAMPLE_DIR="${EXAMPLE_DIR:-}"

# ── 1. Ban suppression comments ─────────────────────────────────────
# Dart has no built-in way to forbid suppressions; enforce via grep.
# Silencing one lint is how strictness drifts, so none are allowed.
# The pattern is split so this script never matches itself.
echo "=== analyze_core: banned suppression comments ==="
BANNED=$(grep -rnE "// ""ignore:|// ""ignore_for_file:" "$@" 2>/dev/null \
  | grep -v '\.g\.dart' || true)
if [ -n "$BANNED" ]; then
  echo "BANNED: suppression comments found. Fix the lint, don't silence it."
  echo "$BANNED"
  exit 1
fi
echo "  clean"

# ── 2. Resolve, then analyze — infos are fatal ──────────────────────
echo "=== analyze_core: pub get ==="
$DART pub get --no-example
if [ -n "$EXAMPLE_DIR" ]; then
  ( cd "$EXAMPLE_DIR" && $FLUTTER pub get )
fi

echo "=== analyze_core: dart analyze --fatal-infos $* ==="
$DART analyze --fatal-infos "$@"

if [ -n "$EXAMPLE_DIR" ]; then
  echo "=== analyze_core: example ($EXAMPLE_DIR) analyze --fatal-infos ==="
  ( cd "$EXAMPLE_DIR" && $FLUTTER analyze --fatal-infos )
fi

echo "=== analyze_core: passed ==="
