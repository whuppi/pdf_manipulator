#!/bin/bash
# Gate pub.dev platform support: run pana — the exact analyzer pub.dev runs,
# pinned to PANA_VERSION — and fail if the package no longer resolves to all 6
# platforms. Catches a regression like an unconditional dart:io/ffi import in a
# shared file silently dropping web, before it ships.
# Called by: make platforms (and the CI platforms job).
# Run from package root.
#
# PANA_VERSION tracks pub.dev's LATEST via the upgrade radar, so the gate runs
# the same pana pub.dev runs — otherwise it drifts from the verdict it predicts.
#
# pana needs a recent STABLE sdk: a pre-release dart can't satisfy pana's floor,
# so `activate` fails loudly here rather than silently resolving an OLDER pana,
# a false green that hides the very regression this gate exists to catch.
#
# pana copies the whole package to a temp dir, so this runs on a lean rsync
# snapshot of the working tree with the gitignored build caches excluded — what
# gets published, never the tens of GB of cargo target/. A copy, so it also
# reflects uncommitted changes.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PKG_ROOT"

# SDK command (overridable via env for non-FVM setups). Must be a STABLE sdk.
DART="${DART:-fvm dart}"

. "$SCRIPT_DIR/versions.env"

$DART pub global activate pana "$PANA_VERSION" >/dev/null

snap="$(mktemp -d)"
out="$(mktemp)"
trap 'rm -rf "$snap" "$out"' EXIT

rsync -a \
  --exclude='/.git' \
  --exclude='/vendor' \
  --exclude='.dart_tool' \
  --exclude='build_output' \
  --exclude='test-results' \
  --exclude='/example/build' \
  ./ "$snap/"

( cd "$snap" && $DART pub global run pana --json . ) > "$out" 2>/dev/null || true

if ! jq -e '.tags' "$out" >/dev/null 2>&1; then
  echo "platforms: pana produced no tags — run '$DART pub global run pana .' to see why"
  exit 1
fi

pana_used="$(jq -r '.runtimeInfo.panaVersion // "?"' "$out")"

missing=""
for platform in android ios linux macos windows web; do
  if ! jq -e --arg t "platform:$platform" '.tags | index($t)' "$out" >/dev/null 2>&1; then
    missing="$missing $platform"
  fi
done

if [ -n "$missing" ]; then
  echo "platforms: FAIL (pana $pana_used) — package no longer supports:$missing"
  echo "  pana detected:"
  jq -r '.tags[] | select(startswith("platform:"))' "$out" | sed 's/^/    /'
  exit 1
fi

echo "platforms: OK (pana $pana_used) — all 6 supported (android ios linux macos windows web)"
