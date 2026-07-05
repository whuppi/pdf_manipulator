#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# platforms_gate.sh — the ONE pub.dev platform-support gate, shared verbatim.
#
# Canonical: whuppi/ci/tool/platforms_gate.sh; the workspace stamper copies it
# verbatim into each consumer's tool/. Edit the canonical + re-stamp — never a
# stamped copy. pr-checks fails a consumer PR whose stamped copy drifted.
#
# Runs pana (pinned to PANA_VERSION, the exact analyzer pub.dev runs) on a lean
# snapshot of the working tree and fails if the package no longer resolves to
# every expected platform — catching a regression like an unconditional
# dart:io/ffi import in a shared file silently dropping web, before it ships.
#
# PANA_VERSION tracks pub.dev's LATEST via each repo's upgrade radar, so the
# gate runs the same pana pub.dev runs — otherwise it drifts from the verdict
# it exists to predict. pana needs a recent STABLE sdk: a pre-release dart
# can't satisfy pana's floor, so `activate` fails loudly rather than silently
# resolving an OLDER pana (a false green that hides the regression).
#
# Env:
#   DART                 SDK command       (default: fvm dart) — a STABLE sdk
#   PANA_VERSION         required — read from the caller's tool/versions.env
#   EXPECTED_PLATFORMS   space-separated   (default: android ios linux macos
#                                            windows web)
# Run from the package root. Needs jq + rsync (both preinstalled on CI runners).
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PKG_ROOT"

DART="${DART:-fvm dart}"
EXPECTED_PLATFORMS="${EXPECTED_PLATFORMS:-android ios linux macos windows web}"

command -v jq >/dev/null 2>&1 || {
  echo "platforms_gate: jq not found (needed to parse pana output)" >&2
  exit 2
}

# PANA_VERSION comes from the caller's own tool/versions.env, stamped next to
# this script. Pinned so the gate runs the same pana pub.dev runs.
if [ -f "$SCRIPT_DIR/versions.env" ]; then
  # shellcheck source=/dev/null
  . "$SCRIPT_DIR/versions.env"
fi
: "${PANA_VERSION:?platforms_gate: PANA_VERSION must be set (tool/versions.env or env)}"

$DART pub global activate pana "$PANA_VERSION" >/dev/null

# pana copies the whole package to a temp dir, so snapshot a lean rsync of the
# working tree first — excludes the gitignored build caches + any vendored
# source (each exclude is a no-op when the package has no such dir). A copy, so
# it also reflects uncommitted changes.
snap="$(mktemp -d)"
out="$(mktemp)"
trap 'rm -rf "$snap" "$out"' EXIT

rsync -a \
  --exclude='/.git' \
  --exclude='/vendor' \
  --exclude='.dart_tool' \
  --exclude='/build' \
  --exclude='/build_output' \
  --exclude='/test-results' \
  --exclude='/example/build' \
  ./ "$snap/"

( cd "$snap" && $DART pub global run pana --json . ) > "$out" 2>/dev/null || true

if ! jq -e '.tags' "$out" >/dev/null 2>&1; then
  echo "platforms_gate: pana produced no tags — run '$DART pub global run pana .' to see why"
  exit 1
fi

pana_used="$(jq -r '.runtimeInfo.panaVersion // "?"' "$out")"

missing=""
# EXPECTED_PLATFORMS is a space-separated list meant to word-split.
# shellcheck disable=SC2086
set -- $EXPECTED_PLATFORMS
for platform in "$@"; do
  if ! jq -e --arg t "platform:$platform" '.tags | index($t)' "$out" >/dev/null 2>&1; then
    missing="$missing $platform"
  fi
done

if [ -n "$missing" ]; then
  echo "platforms_gate: FAIL (pana $pana_used) — package no longer supports:$missing"
  echo "  pana detected:"
  jq -r '.tags[] | select(startswith("platform:"))' "$out" | sed 's/^/    /'
  exit 1
fi

echo "platforms_gate: OK (pana $pana_used) — all expected supported ($EXPECTED_PLATFORMS)"
