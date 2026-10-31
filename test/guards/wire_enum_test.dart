// Enums cross the engine boundary by name, never by index. A reordered or
// inserted enum value would silently change what the engine receives.
//
// 1. Nothing under lib/src/bridge reads `.index` or `.values[`.
// 2. Every public enum under lib/src/types implements PdfWire.
//
// io-exempt: the source-scan wall reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:test/test.dart';

import 'source_scan.dart';

final _byIndex = RegExp(r'\.index\b|\.values\[');

/// `file:line: text` for each `.index` or `.values[` in [files].
List<String> indexUses(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in _byIndex.allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}',
];

final _enum = RegExp(r'^enum (\w+)( implements [^{]*)? \{', multiLine: true);

/// Names of the enums in [source] that do not implement PdfWire.
List<String> enumsWithoutWire(String source) => [
  for (final m in _enum.allMatches(source))
    if (!(m.group(2) ?? '').contains('PdfWire')) m.group(1)!,
];

Map<String, String> _dir(String dir) => {
  for (final f in Directory(dir).listSync(recursive: true).whereType<File>())
    if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
};

void main() {
  test('no .index or .values[ in lib/src/bridge', () {
    expect(indexUses(_dir('lib/src/bridge')), isEmpty);
  });

  test('every public enum implements PdfWire', () {
    final bad = <String>[];
    for (final MapEntry(key: path, value: source) in _dir(
      'lib/src/types',
    ).entries) {
      for (final name in enumsWithoutWire(source)) {
        bad.add('$path: $name');
      }
    }
    expect(bad, isEmpty);
  });

  test('the wall sees an index on the wire', () {
    expect(indexUses({'x.dart': "final m = {'layer': layer.index};\n"}), [
      'x.dart:1',
    ]);
    expect(indexUses({'x.dart': 'final v = Kind.values[3];\n'}), ['x.dart:1']);
    expect(indexUses({'x.dart': '// layer.index is gone\n'}), isEmpty);
  });

  test('the wall sees an enum without a wire name', () {
    expect(enumsWithoutWire('enum A {\n  x,\n}\n'), ['A']);
    expect(enumsWithoutWire('enum A implements PdfWire {\n  x;\n}\n'), isEmpty);
  });

  test('the byte codec refuses a bare enum', () {
    expect(
      () => encodeRequest('op', {'mode': PdfIoMode.native}),
      throwsA(isA<PdfInvalidArgument>()),
    );
  });

  test('the byte codec carries a wire name', () {
    final bytes = encodeRequest('op', {'mode': PdfIoMode.native.wire});
    expect(bytes, isNotEmpty);
  });
}
