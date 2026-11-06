// The release wall: every wasm the package ships stays under the 25 MiB that
// Cloudflare Pages and Workers static assets serve per file. A wasm over it
// would deploy as a broken site, so the wall fails here and in the release
// compile (tool/compile.dart), not at a customer's deploy.
//
// It judges the wasm files built into web_assets/ (the normal one, the
// Asyncify one) and checks that build.json and the release workflow agree on
// which wasm files exist.
//
// io-exempt: reads web_assets/, build.json and the release workflow from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:test/test.dart';

void main() {
  test('no wasm in web_assets/ reaches the 25 MiB static-asset limit', () {
    final wasm = [
      for (final f in Directory('web_assets').listSync().whereType<File>())
        if (f.path.endsWith('.wasm')) f,
    ];
    final over = {
      for (final f in wasm)
        if (f.lengthSync() >= maxWebAssetBytes)
          f.path: '${f.lengthSync()} bytes',
    };
    expect(
      over,
      isEmpty,
      reason:
          'limit is $maxWebAssetBytes bytes; built: '
          '${{for (final f in wasm) f.path: f.lengthSync()}}',
    );
  });

  test('build.json, the build outputs and the release workflow agree', () {
    final constants = BuildConstants.load(Directory.current.uri);
    final release = File('.github/workflows/release.yml').readAsStringSync();
    for (final MapEntry(key: local, value: asset)
        in constants.webAssets.entries) {
      if (!local.endsWith('.wasm')) continue;
      expect(
        constants.wasmBuildOutputs,
        contains(local),
        reason: '$local is a wasm the build produces',
      );
      expect(
        release,
        contains('            $asset\n'),
        reason: 'the release verifies $asset is present',
      );
    }
    expect(constants.webAssets, contains(asyncifyWebAsset));
  });
}
