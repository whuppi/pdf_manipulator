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
  'android-x64-libpdf_oxide.so': '74c22849697b73c19793edd72140093fa6fab0380c6acd59caf76efd3e6d7e40',
  'android-x86-libpdf_oxide.so': 'b194361f3a23ebe0b97fff84e128930d74c7cb90f3c0e255433f7b0240473233',
  'ios-arm64-libpdf_oxide.a': 'b6e2dd5fbea91d96f3ad14f2782fb346173f808056ce0807373ce94f3c8bbea9',
  'ios-sim-arm64-libpdf_oxide.a': '5cff9d157d4bcf03e96a04c229e9c6908ba337fa75aafe4f02acea893d3c7027',
  'ios-sim-x64-libpdf_oxide.a': '9ec45c5c91fd51fb3e2b61220476b9e091caf2f2a48627062376c07b85e3ff37',
  'linux-arm64-libpdf_oxide.so': 'abbb189aa397f58b0bcd32921cd16515298bd66ca362fcbe198cf3a5d5f2dfa7',
  'linux-x64-libpdf_oxide.so': '33b78c4248adc8629d9714f684ded253a73bcf8e9d0dba1ed159f5c02864ea04',
  'macos-arm64-libpdf_oxide.dylib': '6ccef9908eb5d45fb0292393c6515467d450d6036bde2f58e7940131a5f6a954',
  'macos-x64-libpdf_oxide.dylib': '9acef6bd980b29a0a0ca687f87667b8f59b7bf966785d9963a145cf40bd6f47a',
  'windows-arm64-pdf_oxide.dll': '4d0fe545b436a82d924d20f756cdfc3a050e1271d44da62b06706b587d8b153f',
  'windows-x64-pdf_oxide.dll': 'e5d9d8b54d9b3982a08472a4213edfee5bc1ac2fea56b3ccd9508dd8733d4735',
  // --- GENERATED HASHES END ---
};
