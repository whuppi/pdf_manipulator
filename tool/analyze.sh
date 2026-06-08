#!/bin/bash
# Run all static analysis: banned ignores check + Dart + Rust warnings.
# Called by: make analyze
# Run from package root.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"

# SDK commands (overridable via env for non-FVM setups)
DART="${DART:-fvm dart}"
FLUTTER="${FLUTTER:-fvm flutter}"

# ── Ban // ignore: comments ─────────────────────────────────────────
# Dart has no built-in way to prevent ignore comments in analysis_options.
# Enforce via grep. Every lint must be fixed for real — no suppressions.

echo "=== Dart: check for banned // ignore: comments ==="
IGNORES=$(grep -rn '// ignore:\|// ignore_for_file:' "$PKG_ROOT/lib/" "$PKG_ROOT/bin/" "$PKG_ROOT/test/" "$PKG_ROOT/hook/" 2>/dev/null | grep -v '\.g\.dart' || true)
if [ -n "$IGNORES" ]; then
  echo "BANNED: // ignore: comments found. Fix the lint, don't suppress it."
  echo "$IGNORES"
  exit 1
fi
echo "  No // ignore: comments found. Clean."

# ── Dart analysis ───────────────────────────────────────────────────

echo "=== Dart: pub get ==="
$DART pub get --no-example

echo "=== Dart: analyze lib/ bin/ test/ hook/ ==="
$DART analyze --fatal-infos lib/ bin/ test/ hook/

echo "=== Dart: analyze example/ ==="
cd "$PKG_ROOT/example" && $FLUTTER pub get && $FLUTTER analyze --fatal-infos
cd "$PKG_ROOT"

# ── Rust analysis (warnings in our patched lines only) ──────────────

NATIVE_FEATURES=$(bash "$SCRIPT_DIR/compile_rust.sh" --features native)
WASM_FEATURES=$(bash "$SCRIPT_DIR/compile_rust.sh" --features wasm)

# Ensure wasm target is installed for cargo check
ensure_target() {
  if ! rustup target list --installed | grep -qx "$1"; then
    echo "  Installing Rust target: $1"
    rustup target add "$1"
  fi
}

check_rust_warnings() {
  local crate_dir="$1" features="$2" extra_args="${3:-}"

  cd "$crate_dir"

  local branch base_tag
  branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "")
  if [[ "$branch" == *"/"* ]]; then
    base_tag="v$(echo "$branch" | cut -d/ -f2 | sed 's/-patches$//')"
  else
    base_tag="v0.3.55"
  fi

  local diff_output
  diff_output=$(git diff --unified=0 "$base_tag..HEAD" -- src/ 2>/dev/null || true)

  local feature_flag=""
  if [ -n "$features" ]; then
    feature_flag="--features $features"
  fi

  cargo check --lib $extra_args $feature_flag \
    --message-format=json 2>/dev/null \
  | bash "$SCRIPT_DIR/check_rust_warnings.sh" "$diff_output"


  cd "$PKG_ROOT"
}

echo "=== Rust: check pdf_oxide warnings (native) ==="
check_rust_warnings "$PKG_ROOT/vendor/pdf_oxide" "$NATIVE_FEATURES" ""

echo "=== Rust: check pdf_oxide warnings (wasm) ==="
ensure_target wasm32-unknown-unknown
check_rust_warnings "$PKG_ROOT/vendor/pdf_oxide" "$WASM_FEATURES" \
  "--target wasm32-unknown-unknown --no-default-features"

echo "=== Rust: check office_oxide warnings ==="
check_rust_warnings "$PKG_ROOT/vendor/office_oxide" "" ""

echo ""
echo "=== All analysis passed ==="
