#!/bin/bash
# Run all static analysis: banned ignores check + Dart + Rust warnings.
# Called by: make analyze
# Run from package root.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PKG_ROOT="$(dirname "$SCRIPT_DIR")"

# Pinned versions live in ONE file (tool/versions.env), never inline.
source "$SCRIPT_DIR/versions.env"

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

# ── Resolve BEFORE formatting ───────────────────────────────────────
# `dart format`'s output depends on the file's resolved LANGUAGE
# VERSION (new "every-arg-on-its-line" style at >=3.7, old wrap style
# below). The language version comes from each package's pubspec SDK
# floor, read via .dart_tool/package_config.json. With no
# package_config (unresolved deps), the formatter falls back to the
# SDK's latest — a DIFFERENT style than a resolved package produces.
#
# So both packages MUST be resolved before either is formatted, and
# each package MUST be formatted from its own directory so the
# formatter reads ITS package_config. Resolving here also means the
# analyze steps below need no second pub get.
echo "=== Dart: pub get (root + example) ==="
$DART pub get --no-example
( cd "$PKG_ROOT/example" && $FLUTTER pub get )

# ── Dart formatting ─────────────────────────────────────────────────
# Locally: format in place (the gate fixes what it finds).
# CI: fail on any diff — unformatted code never lands unnoticed.
#
# Each package formatted from its own root so the resolved language
# version is the one its pubspec declares — identical output local and
# CI, regardless of either machine's default SDK.
echo "=== Dart: format ==="
format_pkg() {
  local pkg_dir="$1"; shift
  local targets=()
  for d in "$@"; do
    [ -e "$pkg_dir/$d" ] && targets+=("$d")
  done
  [ ${#targets[@]} -eq 0 ] && return 0
  if [ -n "${CI:-}" ]; then
    ( cd "$pkg_dir" && $DART format --set-exit-if-changed "${targets[@]}" )
  else
    ( cd "$pkg_dir" && $DART format "${targets[@]}" )
  fi
}
format_pkg "$PKG_ROOT" lib bin test tool hook
format_pkg "$PKG_ROOT/example" lib integration_test

# ── Dart analysis ───────────────────────────────────────────────────

echo "=== Dart: analyze lib/ bin/ test/ hook/ ==="
$DART analyze --fatal-infos lib/ bin/ test/ hook/

echo "=== Dart: analyze example/ ==="
( cd "$PKG_ROOT/example" && $FLUTTER analyze --fatal-infos )

# ── Rust analysis (warnings in our patched lines only) ──────────────

source "$SCRIPT_DIR/lib.sh"

NATIVE_FEATURES=$(bash "$SCRIPT_DIR/compile_rust.sh" --features native)
WASM_FEATURES=$(bash "$SCRIPT_DIR/compile_rust.sh" --features wasm)

# Filter cargo warnings to only lines we changed vs upstream base tag.
_filter_warnings() {
  local diff_text="$1"
  # Membership via temp file, not `declare -A` — macOS ships bash 3.2,
  # which has no associative arrays.
  local changed_keys
  changed_keys=$(mktemp)
  local cur_file=""
  while IFS= read -r line; do
    case "$line" in
      "+++ b/"*)
        cur_file="${line#'+++ b/'}"
        ;;
      "@@"*)
        local start count
        start=$(echo "$line" | sed -n 's/.*+\([0-9][0-9]*\).*/\1/p')
        count=$(echo "$line" | sed -n 's/.*+[0-9][0-9]*,\([0-9][0-9]*\).*/\1/p')
        count=${count:-1}
        [ "$count" -eq 0 ] && count=1
        for (( i=start; i<start+count; i++ )); do
          echo "${cur_file}:${i}" >> "$changed_keys"
        done
        ;;
    esac
  done <<< "$diff_text"

  local warns=0 skipped=0
  while IFS= read -r json_line; do
    echo "$json_line" | grep -q '"reason":"compiler-message"' || continue
    echo "$json_line" | grep -q '"level":"warning"' || continue
    local span_file span_line msg_text
    span_file=$(echo "$json_line" | grep -oE '"file_name":"[^"]*"' | head -1 | sed 's/"file_name":"//;s/"//')
    span_line=$(echo "$json_line" | grep -oE '"line_start":[0-9]+' | head -1 | sed 's/"line_start"://')
    msg_text=$(echo "$json_line" | sed -n 's/.*"message":"\([^"]*\)".*/\1/p' | head -1)
    if [ -z "$span_file" ] || [ -z "$span_line" ]; then
      skipped=$((skipped + 1))
      continue
    fi
    if grep -qxF "${span_file}:${span_line}" "$changed_keys"; then
      echo "  ${span_file}:${span_line}: ${msg_text}"
      warns=$((warns + 1))
    fi
  done

  rm -f "$changed_keys"
  if [ "$skipped" -gt 0 ]; then
    echo "  ($skipped warning(s) skipped — could not parse span)" >&2
  fi
  if [ "$warns" -gt 0 ]; then
    echo ""
    echo "${warns} warning(s) in our changed lines"
    return 1
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
    base_tag="$BASE_TAG"
  fi

  local diff_output
  diff_output=$(git diff --unified=0 "$base_tag..HEAD" -- src/ 2>/dev/null || true)

  local feature_flag=""
  if [ -n "$features" ]; then
    feature_flag="--features $features"
  fi

  cargo check --lib $extra_args $feature_flag \
    --message-format=json 2>/dev/null \
  | _filter_warnings "$diff_output"

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
