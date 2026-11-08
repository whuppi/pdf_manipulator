// The cutter every producer of web wasm calls: it cuts a built module into
// parts of exactly wasmPartBytes and merges the module's entry into the
// manifest beside them.
//
// io-exempt: writes parts and a manifest into a temp folder.

@TestOn('vm')
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:pdf_manipulator/src/hook/build_constants.dart';
import 'package:pdf_manipulator/src/hook/wasm_parts.dart';
import 'package:test/test.dart';

const _mib = 1024 * 1024;

Uint8List _bytes(int length) =>
    Uint8List.fromList([for (var i = 0; i < length; i++) (i * 31 + 7) & 0xff]);

String _hex(List<int> bytes) => sha256.convert(bytes).toString();

void main() {
  late Directory dir;
  late Directory out;

  setUp(() {
    dir = Directory.systemTemp.createTempSync('wasm_parts_test_');
    out = Directory('${dir.path}/out')..createSync();
  });
  tearDown(() => dir.deleteSync(recursive: true));

  File wasmOf(Uint8List bytes, [String name = 'whole.wasm']) =>
      File('${dir.path}/$name')..writeAsBytesSync(bytes);

  test('a part is 16 MiB', () => expect(wasmPartBytes, 16 * _mib));

  test('40 MiB and one byte is parts of 16, 16 and 8 MiB and one byte', () {
    final whole = _bytes(40 * _mib + 1);
    writeWasmParts(wasm: wasmOf(whole), module: 'pdf_oxide_bg', outDir: out);

    final names = out.listSync().map((e) => e.uri.pathSegments.last).toList()
      ..sort();
    expect(names, [
      'pdf_oxide_bg.0.wasm',
      'pdf_oxide_bg.1.wasm',
      'pdf_oxide_bg.2.wasm',
      'pdf_oxide_manifest.json',
    ]);

    final sizes = [
      for (var i = 0; i < 3; i++)
        File('${out.path}/pdf_oxide_bg.$i.wasm').lengthSync(),
    ];
    expect(sizes, [16 * _mib, 16 * _mib, 8 * _mib + 1]);

    final manifest =
        jsonDecode(
              File('${out.path}/pdf_oxide_manifest.json').readAsStringSync(),
            )
            as Map<String, dynamic>;
    final entry = manifest['pdf_oxide_bg'] as Map<String, dynamic>;
    expect(entry['bytes'], whole.length);
    expect(entry['sha256'], _hex(whole));
    final parts = (entry['parts'] as List).cast<Map<String, dynamic>>();
    expect(
      [for (final p in parts) p['file']],
      ['pdf_oxide_bg.0.wasm', 'pdf_oxide_bg.1.wasm', 'pdf_oxide_bg.2.wasm'],
    );
    expect([for (final p in parts) p['bytes']], sizes);
    var offset = 0;
    for (final (i, p) in parts.indexed) {
      expect(p['sha256'], _hex(whole.sublist(offset, offset + sizes[i])));
      offset += sizes[i];
    }
  });

  test('exactly 16 MiB is one part', () {
    final whole = _bytes(16 * _mib);
    final manifest = writeWasmParts(
      wasm: wasmOf(whole),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    expect(manifest.modules['pdf_oxide_bg']!.parts, hasLength(1));
    expect(
      File('${out.path}/pdf_oxide_bg.0.wasm').readAsBytesSync(),
      orderedEquals(whole),
    );
    expect(File('${out.path}/pdf_oxide_bg.1.wasm').existsSync(), isFalse);
  });

  test('a module of one byte more than a part is two parts', () {
    final manifest = writeWasmParts(
      wasm: wasmOf(_bytes(16 * _mib + 1)),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    expect(
      [for (final p in manifest.modules['pdf_oxide_bg']!.parts) p.bytes],
      [16 * _mib, 1],
    );
  });

  test("merging a module keeps the other module's entry", () {
    final normal = _bytes(3 * _mib);
    final asyncify = _bytes(17 * _mib);
    writeWasmParts(wasm: wasmOf(normal), module: 'pdf_oxide_bg', outDir: out);
    writeWasmParts(
      wasm: wasmOf(asyncify, 'asyncify.wasm'),
      module: asyncifyWasmModule,
      outDir: out,
    );

    final manifest = WasmManifest.load(out);
    expect(manifest.modules.keys, ['pdf_oxide_bg', asyncifyWasmModule]);
    expect(manifest.modules['pdf_oxide_bg']!.sha256, _hex(normal));
    expect(manifest.modules[asyncifyWasmModule]!.sha256, _hex(asyncify));
    expect(manifest.modules[asyncifyWasmModule]!.parts, hasLength(2));
  });

  test('cutting a module again replaces its entry and its stale parts', () {
    writeWasmParts(
      wasm: wasmOf(_bytes(33 * _mib)),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    expect(File('${out.path}/pdf_oxide_bg.2.wasm').existsSync(), isTrue);

    final smaller = _bytes(2 * _mib);
    writeWasmParts(wasm: wasmOf(smaller), module: 'pdf_oxide_bg', outDir: out);
    expect(File('${out.path}/pdf_oxide_bg.1.wasm').existsSync(), isFalse);
    expect(File('${out.path}/pdf_oxide_bg.2.wasm').existsSync(), isFalse);
    expect(
      WasmManifest.load(out).modules['pdf_oxide_bg']!.sha256,
      _hex(smaller),
    );
  });

  test('joining the parts gives back the module', () {
    final whole = _bytes(20 * _mib + 5);
    final manifest = writeWasmParts(
      wasm: wasmOf(whole),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    expect(
      joinWasmParts(out, manifest.modules['pdf_oxide_bg']!),
      orderedEquals(whole),
    );
  });

  test('removing a module takes its entry and its parts', () {
    writeWasmParts(
      wasm: wasmOf(_bytes(1 * _mib)),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    writeWasmParts(
      wasm: wasmOf(_bytes(17 * _mib), 'a.wasm'),
      module: asyncifyWasmModule,
      outDir: out,
    );
    removeWasmModule(out, asyncifyWasmModule);
    expect(WasmManifest.load(out).modules.keys, ['pdf_oxide_bg']);
    expect(
      out.listSync().map((e) => e.uri.pathSegments.last).toList()..sort(),
      ['pdf_oxide_bg.0.wasm', 'pdf_oxide_manifest.json'],
    );
  });

  test('verification names a part whose byte changed', () {
    final manifest = writeWasmParts(
      wasm: wasmOf(_bytes(20 * _mib)),
      module: 'pdf_oxide_bg',
      outDir: out,
    );
    expect(verifyWasmParts(out, manifest), isEmpty);

    final part = File('${out.path}/pdf_oxide_bg.1.wasm');
    final bytes = part.readAsBytesSync();
    bytes[100] ^= 0xff;
    part.writeAsBytesSync(bytes);
    expect(
      verifyWasmParts(out, manifest),
      contains(contains('pdf_oxide_bg.1.wasm')),
    );
  });
}
