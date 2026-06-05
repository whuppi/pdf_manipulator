// Hashes of pre-built native binaries downloaded by the build hook.
//
// The build hook verifies downloaded files against these hashes.
// Hash mismatch → re-download. Correct → use cached. Missing → download.
//
// Entries between the markers are replaced by tool/release.sh
// --update-tag-hashes. Everything outside the markers is stable.

/// SHA-256 hashes for pre-built native binaries, verified by the build hook.
const Map<String, String> assetHashesSha256 = {
  // --- GENERATED HASHES START ---
  // --- GENERATED HASHES END ---
};
