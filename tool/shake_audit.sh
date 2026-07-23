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
# The audit's own baseline: engine internals with every droppable capability
# dropped (= the trim system's output for keep={}). Not a build.json concept.
CORE_FEATURES="icc,legacy-crypto,native-bridge"
# The shipped full native set — the audit compares core against THIS and feeds
# the README size numbers, so it must be build.json's actual features, never a
# hand-copied duplicate that silently goes stale.
FULL_FEATURES=$(json_get '.features.native' "$ROOT/build.json")
# Ceiling with headroom over the measured core-only size; a breach means a
# heavy module leaked back into the core build.
CORE_CEILING_BYTES=$((8 * 1024 * 1024))

fail() { echo "SHAKE-AUDIT FAIL: $1" >&2; exit 1; }

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

if [ "${SHAKE_AUDIT_WASM:-0}" = "1" ] || [ "${SHAKE_AUDIT_CAPS:-0}" = "1" ]; then
  command -v python3 >/dev/null 2>&1 \
    || fail "python3 is required for the WASM/CAPS measurement merges"
fi

echo "== [1/4] full-profile build (reference) =="
(cd "$VENDOR" && cargo build --release --features "$FULL_FEATURES" -q)
FULL_SIZE=$(fsize "$(dylib_for)")
echo "full dylib: $FULL_SIZE bytes"

echo "== [2/4] core-only trim build =="
(cd "$VENDOR" && cargo build --release --features "$CORE_FEATURES" -q)
DYLIB="$(dylib_for)"
CORE_SIZE=$(fsize "$DYLIB")
echo "core dylib: $CORE_SIZE bytes"

echo "== [3/4] symbol + size assertions =="
SYMS=$(defined_syms "$DYLIB")

# Dead public C API must be gone (representative no-mangle exports).
# Mach-O prefixes C symbols with _; ELF does not — accept both.
for banned in pdf_document_builder_create pdf_document_load pdf_render_page; do
  echo "$SYMS" | grep -Eq "_?$banned\$" && fail "banned symbol survived: $banned"
done
# The lane bridge must be alive (its C surface is lane_* + channel_*).
for required in lane_job_cancel channel_init_read; do
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
cat > "$ROOT/tool/.shake_sizes.json" <<JSON
{"nativeFull": $FULL_SIZE, "nativeCore": $CORE_SIZE}
JSON

echo "== [4/4] runtime probe: excluded op answers typed error =="
(cd "$VENDOR" && cargo test --lib --release -q \
  --features "$CORE_FEATURES,test-support" trim_probe 2>&1 | tail -2)

if [ "${SHAKE_AUDIT_WASM:-0}" = "1" ]; then
  echo "== [wasm] core-only wasm build + size check =="
  # the wasm build always writes web_assets/ — preserve the default artifact.
  DEFAULT_RAW=$(fsize "$ROOT/web_assets/pdf_oxide_bg.wasm")
  BAK=$(mktemp -d)
  cp "$ROOT/web_assets/pdf_oxide_bg.wasm" "$ROOT/web_assets/pdf_oxide.js" "$BAK/"
  : "${DART:?shake_audit: DART must be set by the caller (the Makefile passes it)}"
  ( cd "$ROOT" && PDF_FEATURES_WASM="wasm" $DART tool/compile.dart wasm )
  WASM_CORE_RAW=$(fsize "$ROOT/web_assets/pdf_oxide_bg.wasm")
  WASM_CORE_GZ=$(gzip -c "$ROOT/web_assets/pdf_oxide_bg.wasm" | wc -c | tr -d ' ')
  cp "$BAK/pdf_oxide_bg.wasm" "$BAK/pdf_oxide.js" "$ROOT/web_assets/"
  echo "core wasm: $WASM_CORE_RAW raw, $WASM_CORE_GZ gzipped"
  [ "$WASM_CORE_RAW" -lt "$DEFAULT_RAW" ] || fail "core-only wasm not smaller than the full default"
  python3 - "$ROOT/tool/.shake_sizes.json" <<PYEOF
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d.update({"wasmCoreRaw": $WASM_CORE_RAW, "wasmCoreGz": $WASM_CORE_GZ})
json.dump(d, open(p, "w"))
PYEOF
fi

# Per-capability cost measurement (opt-in — five extra release builds).
# cost(cap) = size(core+cap) − size(core); office is measured over
# core+extract because the office feature requires extract.
if [ "${SHAKE_AUDIT_CAPS:-0}" = "1" ]; then
  echo "== [caps] per-capability native costs =="
  measure() {
    (cd "$VENDOR" && cargo build --release --features "$1" -q)
    fsize "$(dylib_for)"
  }
  RENDER=$(( $(measure "$CORE_FEATURES,rendering") - CORE_SIZE ))
  SIGS=$(( $(measure "$CORE_FEATURES,signatures") - CORE_SIZE ))
  PDFA=$(( $(measure "$CORE_FEATURES,pdfa") - CORE_SIZE ))
  EXTRACT_TOTAL=$(measure "$CORE_FEATURES,extract")
  EXTRACT=$(( EXTRACT_TOTAL - CORE_SIZE ))
  OFFICE=$(( $(measure "$CORE_FEATURES,extract,office") - EXTRACT_TOTAL ))
  echo "render=+$RENDER signatures=+$SIGS pdfa=+$PDFA extract=+$EXTRACT office=+$OFFICE"
  python3 - "$ROOT/tool/.shake_sizes.json" <<PYEOF
import json, sys
p = sys.argv[1]
d = json.load(open(p))
d.update({"capRender": $RENDER, "capSignatures": $SIGS, "capPdfa": $PDFA,
          "capExtract": $EXTRACT, "capOffice": $OFFICE})
json.dump(d, open(p, "w"))
PYEOF
fi

echo "SHAKE-AUDIT PASS"
