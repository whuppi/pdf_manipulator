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
  'android-arm-libpdf_oxide.so': '0bb6a4df66716c5d74a345bf0218e3a7383cbaeda8851d3a8fa5347ffce5ac50',
  'android-arm64-libpdf_oxide.so': '102039d63732ec09a7263a47136d09ebc85931a344e33b794ecf6af09a0d0c7d',
  'android-x64-libpdf_oxide.so': '705837937d4716373f24c34efdaf44a681ffe0351263d12be9f7a417cc65e228',
  'android-x86-libpdf_oxide.so': 'b194361f3a23ebe0b97fff84e128930d74c7cb90f3c0e255433f7b0240473233',
  'ios-arm64-libpdf_oxide.a': '69601a7004ecefcdd40440873ba156512ae404d34f81e25fda68bb3f8eb2a7a8',
  'ios-sim-arm64-libpdf_oxide.a': '98cf8a792c47aa5f596d66824df8c9d151bb0aa8bdffb20cc7e908c6aa3dd307',
  'ios-sim-x64-libpdf_oxide.a': '32f8da6bac5b15be28234d9d5e7bbd520a1418f8eacb5c38456ba9bc4ebf5c33',
  'linux-arm64-libpdf_oxide.so': 'abbb189aa397f58b0bcd32921cd16515298bd66ca362fcbe198cf3a5d5f2dfa7',
  'linux-x64-libpdf_oxide.so': 'e60155068a2613b4b79ebb96d780bdd12c3ca91dba09dbc6a6ddff3a45f42392',
  'macos-arm64-libpdf_oxide.dylib': '6ccef9908eb5d45fb0292393c6515467d450d6036bde2f58e7940131a5f6a954',
  'macos-x64-libpdf_oxide.dylib': 'b3a6c5b7f1104348ec20bd6de990699bee6f339b7b9b5d1600871216db7b10cd',
  'windows-arm64-pdf_oxide.dll': 'c68a49c9a38ffb9328658e95c9f640757b949bc463600fe83d6e909ee7803d6b',
  'windows-x64-pdf_oxide.dll': 'f58ca071ae7686b62d8bb172ffacd1acb17a2b6a7c6da25950ba5274f476b6d6',
  // --- GENERATED HASHES END ---
};
