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
CORE_FEATURES="icc,legacy-crypto,native-bridge"
FULL_FEATURES="icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract"
# Ceiling with headroom over the measured core-only size; a breach means a
# heavy module leaked back into the core build.
CORE_CEILING_BYTES=$((8 * 1024 * 1024))

dylib_for() {
  # cargo puts the host cdylib at target/release; feature sets share the dir,
  # so build order matters — we capture sizes immediately after each build.
  echo "$VENDOR/target/release/libpdf_oxide.dylib"
}

echo "== [1/4] full-profile build (reference) =="
(cd "$VENDOR" && cargo build --release --features "$FULL_FEATURES" -q)
FULL_SIZE=$(stat -f%z "$(dylib_for)")
echo "full dylib: $FULL_SIZE bytes"

echo "== [2/4] core-only trim build =="
(cd "$VENDOR" && cargo build --release --features "$CORE_FEATURES" -q)
DYLIB="$(dylib_for)"
CORE_SIZE=$(stat -f%z "$DYLIB")
echo "core dylib: $CORE_SIZE bytes"

echo "== [3/4] symbol + size assertions =="
SYMS=$(nm -gU "$DYLIB")
fail() { echo "SHAKE-AUDIT FAIL: $1" >&2; exit 1; }

# Dead public C API must be gone (representative no-mangle exports).
for banned in pdf_document_builder_create pdf_document_load pdf_render_page; do
  echo "$SYMS" | grep -q "_$banned\$" && fail "banned symbol survived: $banned"
done
# The lane bridge must be alive (its C surface is lane_* + channel_*).
for required in lane_job_cancel channel_init_read; do
  echo "$SYMS" | grep -q "_$required\$" || fail "lane bridge export missing: $required"
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
  # compile_rust.sh always writes web_assets/ — preserve the default artifact.
  DEFAULT_RAW=$(stat -f%z "$ROOT/web_assets/pdf_oxide_bg.wasm")
  BAK=$(mktemp -d)
  cp "$ROOT/web_assets/pdf_oxide_bg.wasm" "$ROOT/web_assets/pdf_oxide.js" "$BAK/"
  PDF_FEATURES_WASM="wasm" bash "$ROOT/tool/compile_rust.sh" wasm
  WASM_CORE_RAW=$(stat -f%z "$ROOT/web_assets/pdf_oxide_bg.wasm")
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
  CORE="icc,legacy-crypto,native-bridge"
  measure() {
    (cd "$VENDOR" && cargo build --release --features "$1" -q)
    stat -f%z "$(dylib_for)"
  }
  RENDER=$(( $(measure "$CORE,rendering") - CORE_SIZE ))
  SIGS=$(( $(measure "$CORE,signatures") - CORE_SIZE ))
  PDFA=$(( $(measure "$CORE,pdfa") - CORE_SIZE ))
  EXTRACT_TOTAL=$(measure "$CORE,extract")
  EXTRACT=$(( EXTRACT_TOTAL - CORE_SIZE ))
  OFFICE=$(( $(measure "$CORE,extract,office") - EXTRACT_TOTAL ))
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
