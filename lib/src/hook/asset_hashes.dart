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
  'android-x64-libpdf_oxide.so': 'fdbbe79a960775fcd9784be31378b1853b601b92a6bc739e302d23ae1d2949b7',
  'android-x86-libpdf_oxide.so': 'b194361f3a23ebe0b97fff84e128930d74c7cb90f3c0e255433f7b0240473233',
  'ios-arm64-libpdf_oxide.a': '954fdb20951e99aea0a7d9d340e43be9c4d250c7f86fb5e92fd2236066bcf011',
  'ios-sim-arm64-libpdf_oxide.a': 'c5dd94d253258d03df4606737e3ba7fcdd6f11085adce6a3d7a7ef403e2ab530',
  'ios-sim-x64-libpdf_oxide.a': '4d1496fc8ab6d9247641b8cd848942e4db7f97b826d1e9ff24e812dd1ff986f2',
  'linux-arm64-libpdf_oxide.so': 'abbb189aa397f58b0bcd32921cd16515298bd66ca362fcbe198cf3a5d5f2dfa7',
  'linux-x64-libpdf_oxide.so': '9831f6a3db97c39b53a7418bf123b47c62682ff6630d12d42ec4dd16a8bd5d06',
  'macos-arm64-libpdf_oxide.dylib': '6ccef9908eb5d45fb0292393c6515467d450d6036bde2f58e7940131a5f6a954',
  'macos-x64-libpdf_oxide.dylib': '0cd7f1b9ecaf2911318ed0e263d8c012e677035bc041d0f8efeacbdee126827a',
  'windows-arm64-pdf_oxide.dll': 'a564b21b0c94e263ca719893ac100824dec72a39a6ca5c182f3ff42fe3bbfadd',
  'windows-x64-pdf_oxide.dll': 'f7a653ddec1670a67db37957cd4086ec1d51744e8ea6c828a3269da1cc68c990',
  // --- GENERATED HASHES END ---
};
