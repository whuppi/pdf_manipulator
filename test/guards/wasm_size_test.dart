// The walls around the web wasm. Every module the package ships is cut into
// parts listed in pdf_oxide_manifest.json, because static hosts cap each
// uploaded file (Cloudflare Pages and Workers static assets: 25 MiB), not the
// module a browser compiles. A file over the cap would deploy as a broken
// site, so the walls fail here and in the release compile (tool/compile.dart),
// not at a customer's deploy:
//
//   - every file in web_assets/ is under the cap;
//   - each manifest module is, byte for byte, its parts joined, and each part
//     is its file;
//   - a visitor's download, the gzip of the whole module, is inside its budget.
//
// The walls themselves are tested on small synthetic folders: each is shown to
// fail on the break it guards. The built web_assets/ then has to pass them.
//
// io-exempt: reads web_assets/, build.json and the release workflow from disk.

@TestOn('vm')
library;

import 'dart:io';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/wasm_parts.dart';
import 'package:test/test.dart';

final _built = File('web_assets/$wasmManifestFile').existsSync()
    ? false
    : 'web_assets/ holds no wasm build (make build-wasm)';

void main() {
  group('the walls fail on what they guard', () {
    late Directory dir;
    setUp(() {
      dir = Directory.systemTemp.createTempSync('wasm_wall_test_');
      final wasm = File('${dir.path}/whole.wasm')
        ..writeAsBytesSync(
          Uint8List.fromList([for (var i = 0; i < 3000; i++) i * 7 & 0xff]),
        );
      writeWasmParts(wasm: wasm, module: normalWasmModule, outDir: dir);
      wasm.deleteSync();
    });
    tearDown(() => dir.deleteSync(recursive: true));

    test('a clean folder passes', () {
      expect(wasmWallProblems(dir), isEmpty);
    });

    test('a file at the cap fails, naming it', () {
      expect(
        wasmWallProblems(dir, maxFileBytes: 3000),
        contains(contains('$normalWasmModule.0.wasm')),
      );
    });

    test('a file under the cap passes', () {
      expect(wasmWallProblems(dir, maxFileBytes: 3001), isEmpty);
    });

    test('a changed part byte fails, naming the part', () {
      final part = File('${dir.path}/$normalWasmModule.0.wasm');
      final bytes = part.readAsBytesSync();
      bytes[10] ^= 1;
      part.writeAsBytesSync(bytes);
      expect(
        wasmWallProblems(dir),
        contains(contains('$normalWasmModule.0.wasm')),
      );
    });

    test('a missing part fails, naming it', () {
      File('${dir.path}/$normalWasmModule.0.wasm').deleteSync();
      expect(
        wasmWallProblems(dir),
        contains(contains('$normalWasmModule.0.wasm')),
      );
    });

    test('a budget below the module fails, naming the module', () {
      expect(
        wasmWallProblems(dir, gzipBudgets: {normalWasmModule: 10}),
        contains(allOf(contains(normalWasmModule), contains('budget'))),
      );
    });

    test('a budget above the module passes', () {
      expect(
        wasmWallProblems(dir, gzipBudgets: {normalWasmModule: 1 << 20}),
        isEmpty,
      );
    });
  });

  group('the built web_assets/', () {
    test('every file is under the cap and every module is its parts', () {
      expect(wasmWallProblems(Directory('web_assets')), isEmpty);
    }, skip: _built);

    test('every part of the manifest is a part build.json ships', () {
      expect(
        buildJsonPartProblems(
          Directory('web_assets'),
          BuildConstants.load(Directory.current.uri),
        ),
        isEmpty,
      );
    }, skip: _built);

    test('the budgets are the ones the plan sets', () {
      expect(normalWasmGzipBudget, 8912896);
      expect(asyncifyWasmGzipBudget, 11010048);
      expect(maxWebAssetBytes, 26214400);
    });
  });

  test('build.json, the build outputs and the release workflow agree', () {
    final constants = BuildConstants.load(Directory.current.uri);
    final release = File('.github/workflows/release.yml').readAsStringSync();
    final shipped = [
      ...constants.webAssets.keys.where((n) => n.endsWith('.wasm')),
      wasmManifestFile,
    ];
    for (final local in shipped) {
      final asset = constants.webAssets[local];
      expect(asset, isNotNull, reason: 'build.json lists $local');
      expect(
        constants.wasmBuildOutputs,
        contains(local),
        reason: '$local is a file the build produces',
      );
      expect(
        release,
        contains('            $asset\n'),
        reason: 'the release verifies $asset is present',
      );
    }
    expect(
      constants.webAssets.keys,
      isNot(
        anyOf(
          contains('pdf_oxide_bg.wasm'),
          contains('pdf_oxide_asyncify_bg.wasm'),
        ),
      ),
      reason: 'no whole wasm is a web asset: every module ships as parts',
    );
    expect(
      constants.webAssets.keys.where((n) => n.startsWith('$normalWasmModule.')),
      isNotEmpty,
    );
    expect(
      constants.webAssets.keys.where(
        (n) => n.startsWith('$asyncifyWasmModule.'),
      ),
      isNotEmpty,
    );
  });
}
