# shellcheck shell=bash
# Consumer extension for the shared whuppi/ci release.sh (sourced by its
# stamp_asset_hashes — see the shared script's header). Source, don't execute.
#
# pdf_manipulator ships hand-written web assets (e.g. lane_worker.js) that the
# GitHub Release API carries no digest for — the release only uploads the
# COMPILED binaries + wasm outputs, which get API digests. This hook hashes the
# hand-written files from the tag's web_assets/ tree so they land in
# lib/src/hook/asset_hashes.dart alongside the API digests.
#
# Contract: print one "asset_name<TAB>sha256" line per asset. Runs inside the
# shared release.sh process, so build_lib.sh helpers (sha256_file, ensure_jq) are
# available. Reads local filenames + asset names from build.json, skipping the
# wasmBuildOutputs (those get hashes from the Release API).
release_extra_asset_hashes() {
  local wasm_outputs local_name asset_name src
  ensure_jq
  wasm_outputs=$(jq -r '.wasmBuildOutputs | join(" ")' build.json)
  while IFS=$'\t' read -r local_name asset_name; do
    [ -n "$local_name" ] || continue
    grep -qw "$local_name" <<< "$wasm_outputs" && continue
    src="web_assets/$local_name"
    [ -f "$src" ] || continue
    printf '%s\t%s\n' "$asset_name" "$(sha256_file "$src")"
  done < <(jq -r '.web | to_entries[] | "\(.key)\t\(.value)"' build.json)
}
