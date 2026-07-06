#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# verify_web_gate.sh — compile the example under BOTH web compilers, shared verbatim.
#
# Canonical: whuppi/ci/tool/verify_web_gate.sh; the workspace stamper copies it
# verbatim into each consumer's tool/. Edit the canonical + re-stamp — never a
# stamped copy. pr-checks fails a consumer PR whose stamped copy drifted.
#
# "Web support" is two compilers, not one. dart2js (the default JS build) and
# dart2wasm (`--wasm`) have different type models: a js-interop switch or cast
# that dart2js accepts, dart2wasm can reject (a non-exhaustive JSAny switch,
# an unsound interop `as`, etc.). Neither the analyzer nor pana's wasm heuristic
# actually compiles wasm, so a JS-only build is a false green — the package
# advertises web, but a wasm-mode Flutter app can't compile it. This gate
# compiles the example under both compilers so that gap fails at PR time, not
# in a user's build.
#
# Env:
#   FLUTTER   Flutter command — REQUIRED, no fallback. The caller (the Makefile
#             target) passes it; a missing one fails loud, never guesses.
# The example dir is fixed at "example" (the Flutter package convention).
# Run from the package root; builds example/ under both compilers.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

: "${FLUTTER:?verify_web_gate: FLUTTER must be set by the caller, e.g. fvm flutter}"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PKG_ROOT/example"

echo "verify_web_gate: dart2js   — $FLUTTER build web"
# FLUTTER is intentionally word-split (e.g. "fvm flutter").
# shellcheck disable=SC2086
$FLUTTER build web --release

echo "verify_web_gate: dart2wasm — $FLUTTER build web --wasm"
# shellcheck disable=SC2086
$FLUTTER build web --wasm --release

echo "verify_web_gate: OK — example compiles under dart2js and dart2wasm"
