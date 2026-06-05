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
  'android-x64-libpdf_oxide.so': 'db564c5ef6361a0021d1943f3493d10464c137ba1c02b27fee33debb2338c95c',
  'android-x86-libpdf_oxide.so': 'b194361f3a23ebe0b97fff84e128930d74c7cb90f3c0e255433f7b0240473233',
  'ios-arm64-libpdf_oxide.a': '6f3a74315adfef893a3ee201b7611a2e4341f08245228379e87936f4b6dc19ef',
  'ios-sim-arm64-libpdf_oxide.a': '999282e4491d03cb8760d9355f777791726b7afe9e7544149fb925082a7d4ea9',
  'ios-sim-x64-libpdf_oxide.a': '693a28746d43b33a02b22e5c3540a2e292975f1a900a4cc5bc62e52acd697b25',
  'linux-arm64-libpdf_oxide.so': 'abbb189aa397f58b0bcd32921cd16515298bd66ca362fcbe198cf3a5d5f2dfa7',
  'linux-x64-libpdf_oxide.so': 'd252c445532c24b5f6a1d6633c807b51caf35b9bd9248512c188ea406df6ea88',
  'macos-arm64-libpdf_oxide.dylib': '6ccef9908eb5d45fb0292393c6515467d450d6036bde2f58e7940131a5f6a954',
  'macos-x64-libpdf_oxide.dylib': '0d51fbca3321aeabfe96e23ad8d70783650e041eaeb315fdcd2453a40dfc2c3e',
  'windows-arm64-pdf_oxide.dll': '6ee968f6a954b0f0ebe1fb3ad9b3aa951a062598bf7e716e96eb2d98f2ee6436',
  'windows-x64-pdf_oxide.dll': '087e1a4918d16c383563bd9cf2aa0363b8c3bf3713147737a796aa18810d1c38',
  // --- GENERATED HASHES END ---
};
