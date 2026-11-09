#!/usr/bin/env bash
# Shake audit: proves the trim pipeline actually deletes what it promises,
# so the guarantee survives upstream rebases. Four checks on a core-only
# native build (every droppable capability dropped):
#   1. the C API surface (public-api feature) is absent from the dylib
#   2. the lane bridge exports are present (the binary still works)
#   3. the trimmed dylib is materially smaller than the full one + under a ceiling
#   4. the runtime probe: excluded ops answer with the typed not-enabled error
# Wasm is opt-in (SHAKE_AUDIT_WASM=1) — it needs the full wasm toolchain and
# ~10 minutes.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR="$ROOT/vendor/pdf_oxide"
# shellcheck source=tool/build_lib.sh
source "$ROOT/tool/build_lib.sh"

fail() { echo "SHAKE-AUDIT FAIL: $1" >&2; exit 1; }

# Measure on the Rust CI ships with: another release inlines differently, and
# the README quotes these numbers.
# shellcheck source=tool/versions.env
source "$ROOT/tool/versions.env"
export RUSTUP_TOOLCHAIN="$RUST_VERSION"
rustup run "$RUST_VERSION" rustc --version >/dev/null 2>&1 \
  || fail "Rust $RUST_VERSION is not installed: rustup toolchain install $RUST_VERSION"

# The audit's own baseline: engine internals with every droppable capability
# dropped — the `[]` keep-set tool/keep_sets.dart prints for native, never a
# hand copy that silently goes stale when a capability is added.
: "${DART:?shake_audit: DART must be set by the caller (the Makefile passes it)}"
# The features of the first line keep_sets.dart prints for $1. The output is
# captured whole before it is cut: piping it into `head` closes the pipe
# early, and Dart throws on a write to a closed pipe.
empty_keep_set() {
  local sets
  sets=$(cd "$ROOT" && $DART tool/keep_sets.dart "$1") \
    || fail "tool/keep_sets.dart $1 failed"
  printf '%s\n' "$sets" | sed -n '1p' | cut -f2
}
CORE_FEATURES=$(empty_keep_set native)
[ -n "$CORE_FEATURES" ] || fail "tool/keep_sets.dart native produced no [] features line"
# The shipped full native set — the audit compares core against THIS and feeds
# the README size numbers, so it must be build.json's actual features, never a
# hand-copied duplicate that silently goes stale.
FULL_FEATURES=$(json_get '.features.native' "$ROOT/build.json")
# Ceiling with headroom over the measured core-only size; a breach means a
# heavy module leaked back into the core build.
CORE_CEILING_BYTES=$((9 * 1024 * 1024))

# File size in bytes — wc -c is POSIX; BSD wc pads with spaces, so trim.
fsize() {
  wc -c < "$1" | tr -d ' '
}

# Exported defined symbols — Apple nm flags on macOS, GNU nm elsewhere.
defined_syms() {
  case "$(uname -s)" in
    Darwin*) nm -gU "$1" ;;
    *)       nm -g --defined-only "$1" ;;
  esac
}

dylib_for() {
  # cargo puts the host cdylib at target/release; feature sets share the dir,
  # so build order matters — we capture sizes immediately after each build.
  case "$(uname -s)" in
    Darwin*)      echo "$VENDOR/target/release/libpdf_oxide.dylib" ;;
    Linux*)       echo "$VENDOR/target/release/libpdf_oxide.so" ;;
    MINGW*|MSYS*) echo "$VENDOR/target/release/pdf_oxide.dll" ;;
    *)            fail "unsupported host: $(uname -s)" ;;
  esac
}

# Merge key/value size numbers into .shake_sizes.json (create if absent).
# MERGE, never overwrite — a native-only run would otherwise wipe the
# numbers a heavier run recorded, and `make verify-readme-sizes` would then
# fail.
merge_sizes() {
  python3 - "$ROOT/tool/.shake_sizes.json" "$@" <<'PY'
import json, os, sys
p = sys.argv[1]
d = json.load(open(p)) if os.path.exists(p) else {}
a = sys.argv[2:]
for i in range(0, len(a), 2):
    d[a[i]] = int(a[i + 1])
json.dump(d, open(p, "w"))
PY
}

# Build one wasm variant to a TEMP DIR (never touching web_assets/, the
# committed default artifact) and echo "<raw> <gz>" bytes — the same temp-dir
# staging a trimmed / non-default consumer build uses, so no backup-restore.
# $1 = features, $2 = the build mode ("" = speed, "size").
measure_wasm() {
  : "${DART:?shake_audit: DART must be set by the caller (the Makefile passes it)}"
  local line
  line=$(cd "$ROOT" && $DART tool/measure_wasm.dart "$1" "${2:-}" \
    | grep -oE 'raw=[0-9]+ gz=[0-9]+' | tail -1)
  [ -n "$line" ] || fail "measure_wasm produced no size for features='$1' build='${2:-}'"
  line=${line#raw=}
  echo "${line/ gz=/ }"
}

command -v python3 >/dev/null 2>&1 || fail "python3 is required for the measurement record"

# One release build of the host engine with features $1; prints its size.
# Extra cargo env (a profile override) comes from the caller's environment.
native_size() {
  (cd "$VENDOR" && cargo build --release --features "$1" -q)
  fsize "$(dylib_for)"
}

# The keep guarantee: the core-only build drops the dead C API, keeps the lane
# bridge, is materially smaller than the full build and under its ceiling,
# and an excluded op answers the typed not-enabled error.
part_guarantee() {
  echo "== [1/4] full-profile build (reference) =="
  FULL_SIZE=$(native_size "$FULL_FEATURES")
  echo "full dylib: $FULL_SIZE bytes"

  echo "== [2/4] core-only trim build =="
  CORE_SIZE=$(native_size "$CORE_FEATURES")
  DYLIB="$(dylib_for)"
  echo "core dylib: $CORE_SIZE bytes"

  echo "== [3/4] symbol + size assertions =="
  SYMS=$(defined_syms "$DYLIB")
  # Dead public C API must be gone (representative no-mangle exports).
  # Mach-O prefixes C symbols with _; ELF does not — accept both.
  for banned in pdf_document_builder_create pdf_document_load pdf_render_page; do
    echo "$SYMS" | grep -Eq "_?$banned\$" && fail "banned symbol survived: $banned"
  done
  # The lane bridge must be alive (its C surface is lane_* + channel_*).
  for required in executor_job_cancel channel_init_read; do
    echo "$SYMS" | grep -Eq "_?$required\$" || fail "lane bridge export missing: $required"
  done
  # Trim must actually delete code: core-only materially smaller than full.
  if [ $((FULL_SIZE - CORE_SIZE)) -lt $((2 * 1024 * 1024)) ]; then
    fail "core-only is <2MB smaller than full ($CORE_SIZE vs $FULL_SIZE) — trim deleted nothing"
  fi
  if [ "$CORE_SIZE" -gt "$CORE_CEILING_BYTES" ]; then
    fail "core-only dylib $CORE_SIZE exceeds ceiling $CORE_CEILING_BYTES"
  fi
  echo "symbols + sizes OK (full=$FULL_SIZE core=$CORE_SIZE saved=$((FULL_SIZE - CORE_SIZE)))"
  # Record the measurements for tool/verify_readme_sizes.dart — the audit is
  # the only builder/measurer; the verifier only formats + asserts.
  merge_sizes nativeFull "$FULL_SIZE" nativeCore "$CORE_SIZE"

  echo "== [4/4] runtime probe: excluded op answers typed error =="
  (cd "$VENDOR" && cargo test --lib --release -q \
    --features "$CORE_FEATURES,test-support" trim_probe 2>&1 | tail -2)
}

# The `build: size` column of the README table, native: opt-level z.
part_size_full() {
  echo "== [size] native opt-level z, full =="
  local size
  size=$(CARGO_PROFILE_RELEASE_OPT_LEVEL=z native_size "$FULL_FEATURES")
  echo "native size full: $size"
  merge_sizes nativeFullSize "$size"
}
part_size_core() {
  echo "== [size] native opt-level z, core =="
  local size
  size=$(CARGO_PROFILE_RELEASE_OPT_LEVEL=z native_size "$CORE_FEATURES")
  echo "native size core: $size"
  merge_sizes nativeCoreSize "$size"
}

# The wasm cells of the README table the default artifact does not give:
# core-only (speed), and full and core at `build: size`.
part_wasm() {
  echo "== [wasm] core (speed) + full/core (size), each staged to a temp dir =="
  WASM_FULL_FEATURES=$(json_get '.features.wasm' "$ROOT/build.json")
  WASM_CORE_FEATURES=$(empty_keep_set wasm)
  [ -n "$WASM_CORE_FEATURES" ] \
    || fail "tool/keep_sets.dart wasm produced no [] features line"
  read -r WASM_CORE_RAW WASM_CORE_GZ <<< "$(measure_wasm "$WASM_CORE_FEATURES")"
  [ -f "$ROOT/web_assets/pdf_oxide_manifest.json" ] \
    || fail "web_assets/pdf_oxide_manifest.json missing: run make web_assets/pdf_oxide_bg.0.wasm first"
  DEFAULT_RAW=$(json_get '[.pdf_oxide_bg.parts[].bytes] | add' "$ROOT/web_assets/pdf_oxide_manifest.json")
  [ "$WASM_CORE_RAW" -lt "$DEFAULT_RAW" ] \
    || fail "core-only wasm not smaller than the full default"
  read -r WASM_FULL_SIZE_RAW WASM_FULL_SIZE_GZ <<< "$(measure_wasm "$WASM_FULL_FEATURES" size)"
  read -r WASM_CORE_SIZE_RAW WASM_CORE_SIZE_GZ <<< "$(measure_wasm "$WASM_CORE_FEATURES" size)"
  echo "wasm core=$WASM_CORE_RAW/$WASM_CORE_GZ full-size=$WASM_FULL_SIZE_RAW/$WASM_FULL_SIZE_GZ core-size=$WASM_CORE_SIZE_RAW/$WASM_CORE_SIZE_GZ"
  merge_sizes \
    wasmCoreRaw "$WASM_CORE_RAW" wasmCoreGz "$WASM_CORE_GZ" \
    wasmFullSizeRaw "$WASM_FULL_SIZE_RAW" wasmFullSizeGz "$WASM_FULL_SIZE_GZ" \
    wasmCoreSizeRaw "$WASM_CORE_SIZE_RAW" wasmCoreSizeGz "$WASM_CORE_SIZE_GZ"
}

# A capability's native build: core plus its feature, recorded as a total.
# tool/verify_readme_sizes.dart takes its cost as total − core; office is
# measured over core+extract, since the office feature requires extract.
part_cap() {
  local key features
  case "$1" in
    render)     key=nativeCoreRender;        features="$CORE_FEATURES,rendering" ;;
    signatures) key=nativeCoreSignatures;    features="$CORE_FEATURES,signatures" ;;
    pdfa)       key=nativeCorePdfa;          features="$CORE_FEATURES,pdfa" ;;
    extract)    key=nativeCoreExtract;       features="$CORE_FEATURES,extract" ;;
    office)     key=nativeCoreExtractOffice; features="$CORE_FEATURES,extract,office" ;;
    *)          fail "unknown capability: $1" ;;
  esac
  echo "== [caps] native core + $1 =="
  local size
  size=$(native_size "$features")
  echo "core + $1: $size"
  merge_sizes "$key" "$size"
}

# SHAKE_AUDIT_PART runs one part, so CI measures them on parallel runners:
# guarantee, size-full, size-core, wasm, cap-<capability>. Without it the
# audit runs the guarantee, then the opt-in modes.
case "${SHAKE_AUDIT_PART:-}" in
  "")
    part_guarantee
    if [ "${SHAKE_AUDIT_WASM:-0}" = "1" ]; then
      part_size_full
      part_size_core
      part_wasm
    fi
    if [ "${SHAKE_AUDIT_CAPS:-0}" = "1" ]; then
      for cap in render signatures pdfa extract office; do part_cap "$cap"; done
    fi
    ;;
  guarantee) part_guarantee ;;
  size-full) part_size_full ;;
  size-core) part_size_core ;;
  wasm)      part_wasm ;;
  cap-*)     part_cap "${SHAKE_AUDIT_PART#cap-}" ;;
  *)         fail "unknown SHAKE_AUDIT_PART: $SHAKE_AUDIT_PART" ;;
esac

echo "SHAKE-AUDIT PASS"
