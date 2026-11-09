// R11: points and rectangles are PdfPoint and PdfRect. No `(double, double)`
// record and no `double x, double y` pair in a public signature of
// lib/src/ops or lib/src/types.
//
// io-exempt: the source scan reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _pair = RegExp(
  r'\(\s*double\s*,\s*double\s*\)|double\s+x\s*,\s*(?:required\s+)?double\s+y\b',
);

/// `file:line` of each coordinate pair in [files].
List<String> coordinatePairs(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final m in _pair.allMatches(stripComments(raw)))
      '$name:${lineOf(raw, m.start)}',
];

void main() {
  test('no coordinate pair under lib/src/ops and lib/src/types', () {
    final files = {
      for (final dir in const ['lib/src/ops', 'lib/src/types'])
        for (final f in Directory(dir).listSync().whereType<File>())
          if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
    };
    expect(coordinatePairs(files), isEmpty);
  });

  test('the wall sees a record and a pair', () {
    expect(coordinatePairs({'x.dart': 'void f((double, double) p) {}\n'}), [
      'x.dart:1',
    ]);
    expect(coordinatePairs({'x.dart': 'void f(double x, double y) {}\n'}), [
      'x.dart:1',
    ]);
    expect(coordinatePairs({'x.dart': 'void f(PdfPoint p) {}\n'}), isEmpty);
  });
}
