// R6: one `end` meaning. Every public `end` is exclusive, as in
// `List.sublist(start, end)`, so no dartdoc line in lib/src/types says `end`
// and `inclusive` together.
//
// io-exempt: the source scan reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

/// `file:line` of each line in [files] that has `end` and `inclusive`.
List<String> inclusiveEnds(Map<String, String> files) => [
  for (final MapEntry(key: name, value: raw) in files.entries)
    for (final (i, line) in raw.split('\n').indexed)
      if (RegExp(r'\bend\b', caseSensitive: false).hasMatch(line) &&
          RegExp(r'\binclusive\b', caseSensitive: false).hasMatch(line) &&
          !RegExp(r'\bexclusive\b', caseSensitive: false).hasMatch(line))
        '$name:${i + 1}',
];

void main() {
  test('no dartdoc in lib/src/types calls an end inclusive', () {
    final files = {
      for (final f in Directory('lib/src/types').listSync().whereType<File>())
        if (f.path.endsWith('.dart'))
          f.path: [
            for (final l in f.readAsLinesSync())
              if (l.trimLeft().startsWith('///')) l else '',
          ].join('\n'),
    };
    expect(inclusiveEnds(files), isEmpty);
  });

  test('the wall sees an inclusive end', () {
    expect(inclusiveEnds({'x.dart': '/// Last page (inclusive); end.\n'}), [
      'x.dart:1',
    ]);
    expect(inclusiveEnds({'x.dart': '/// The end (exclusive).\n'}), isEmpty);
  });
}
