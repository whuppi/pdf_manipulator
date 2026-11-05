// N12: one word per idea. The retired twin of each kept word appears in no
// public declaration or dartdoc of lib/src/ops and lib/src/types, in the
// README, or in the example.
//
// io-exempt: the source scan reads lib, README.md and example from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

/// Retired token, then the kept word it gave way to.
const retired = <String, String>{
  r'\bdeletePages?\b': 'remove',
  r'\bembedFile\b|\bEmbeddedFile\b': 'attachment',
  r'\bmodDate\b': 'modificationDate',
  r'\bmonoPpi\b': 'bilevelPpi',
  r'\bPdfDoc\b|\bPdfDocHandle\b': 'document',
  r'\bfullRewrite\b|\bPdfSaveFullRewrite\b': 'full',
  r'\bPdf\w*Type\b': 'kind',
  r'\bmaxLanes\b|\bwebLaneWorkerUrl\b': 'concurrency',
  r'\bRenderedImage\b|\brenderedImage\b': 'bitmap',
  r'\bmergeFrom\b': 'appendPages',
};

/// `file:line: token` for each retired token in [files] (name to source).
List<String> retiredTokens(Map<String, String> files) => [
  for (final MapEntry(key: name, value: text) in files.entries)
    for (final (i, line) in text.split('\n').indexed)
      for (final pattern in retired.keys)
        if (RegExp(pattern).hasMatch(line)) '$name:${i + 1}: $pattern',
];

Map<String, String> _files() => {
  for (final dir in const ['lib/src/ops', 'lib/src/types', 'example/lib'])
    for (final f in Directory(dir).listSync(recursive: true).whereType<File>())
      if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
  'README.md': File('README.md').readAsStringSync(),
};

void main() {
  test('no retired word in lib/src/ops, lib/src/types, README or example', () {
    expect(retiredTokens(_files()), isEmpty);
  });

  test('the wall sees a retired word', () {
    expect(retiredTokens({'x.dart': 'void deletePages() {}\n'}), hasLength(1));
    expect(
      retiredTokens({'x.dart': 'final DateTime? modDate;\n'}),
      hasLength(1),
    );
    expect(retiredTokens({'x.dart': 'void removePages() {}\n'}), isEmpty);
  });
}
