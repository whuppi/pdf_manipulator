#!/usr/bin/env bash
# shellcheck shell=bash
# ────────────────────────────────────────────────────────────────────
# check_bindings.sh — compiles the language bindings the fork carries from
# upstream (src/python.rs, src/ffi.rs, src/wasm.rs) with `cargo check --lib`,
# so an edit to one of them cannot go unchecked.
#
#   bash tool/check_bindings.sh
#
# Binding sets (each must compile with exactly the features it needs):
#   python  native  --features python
#   ffi     native  --features public-api,native-bridge
#   wasm    wasm32  --features wasm,public-api   (target wasm32-unknown-unknown)
# native-bridge is listed for ffi because the host layer, built on every
# non-wasm32 target, needs its crates; public-api has no wasm32 need of it.
#
# Environment needs:
#   - cargo, with the vendor/ submodules initialised
#   - python: a Python 3 interpreter and its headers (pyo3 builds against
#     them; python3-dev / python3-devel), `python3` on PATH or PYO3_PYTHON set
#   - wasm: `rustup target add wasm32-unknown-unknown`
# The script refuses with a message when one of these is missing.
# Run from anywhere; exits non-zero on the first missing need or any failure.
# ────────────────────────────────────────────────────────────────────
set -euo pipefail

cd "$(dirname "$0")/.."

manifest=vendor/pdf_oxide/Cargo.toml
[ -f "$manifest" ] || { echo "check_bindings: $manifest missing — run: git submodule update --init vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia" >&2; exit 1; }
command -v cargo >/dev/null || { echo "check_bindings: cargo not found (https://rustup.rs)" >&2; exit 1; }

py=${PYO3_PYTHON:-python3}
command -v "$py" >/dev/null || { echo "check_bindings: no Python interpreter ($py) — pyo3 needs one; install python3 or set PYO3_PYTHON" >&2; exit 1; }
header=$("$py" -c 'import os,sysconfig;print(os.path.join(sysconfig.get_paths()["include"],"Python.h"))')
[ -f "$header" ] || { echo "check_bindings: $header missing — install the Python development headers (python3-dev / python3-devel)" >&2; exit 1; }

if ! { rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; }; then
  echo "check_bindings: wasm32-unknown-unknown target missing — run: rustup target add wasm32-unknown-unknown" >&2
  exit 1
fi

export PYO3_PYTHON="$py"

echo "== python =="
cargo check --manifest-path "$manifest" --lib --features python
echo "== ffi =="
cargo check --manifest-path "$manifest" --lib --features public-api,native-bridge
echo "== wasm =="
cargo check --manifest-path "$manifest" --lib --target wasm32-unknown-unknown --features wasm,public-api
echo "bindings: all compile"
