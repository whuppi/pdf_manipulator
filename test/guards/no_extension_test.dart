// R8: every door is a member of Pdf. No public extension on Pdf, so a door
// never hides in a second file.
//
// io-exempt: the source scan reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _extension = RegExp(r'\bextension\s+\w+\s+on\s+Pdf\b');

/// `file:line` of each extension on Pdf in [files].
List<String> pdfExtensions(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in _extension.allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}',
];

void main() {
  test('no extension on Pdf under lib', () {
    final files = {
      for (final f in Directory(
        'lib',
      ).listSync(recursive: true).whereType<File>())
        if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
    };
    expect(pdfExtensions(files), isEmpty);
  });

  test('the wall sees an extension on Pdf', () {
    expect(pdfExtensions({'x.dart': 'extension PdfSugar on Pdf {\n}\n'}), [
      'x.dart:1',
    ]);
    expect(pdfExtensions({'x.dart': '// extension A on Pdf\n'}), isEmpty);
  });
}
