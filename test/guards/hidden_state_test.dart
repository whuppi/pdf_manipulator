// R12: no hidden target. No public builder member's dartdoc says
// "last-written", "most recently" or "most-recently": what a link, a markup
// or a script attaches to is an argument of the call.
//
// io-exempt: the source scan reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

final _hidden = RegExp(
  r'last-written|most[- ]recently|last written',
  caseSensitive: false,
);

/// The 1-based lines of [source] that name a hidden target.
List<int> hiddenTargets(String source) => [
  for (final (i, line) in source.split('\n').indexed)
    if (_hidden.hasMatch(line)) i + 1,
];

void main() {
  test('lib/src/ops/pdf_builder.dart names no hidden target', () {
    expect(
      hiddenTargets(File('lib/src/ops/pdf_builder.dart').readAsStringSync()),
      isEmpty,
    );
  });

  test('the wall sees one', () {
    expect(hiddenTargets('/// On the last-written text.\n'), [1]);
    expect(hiddenTargets('/// Most recently added field.\n'), [1]);
    expect(hiddenTargets('/// On the text it writes.\n'), isEmpty);
  });
}
