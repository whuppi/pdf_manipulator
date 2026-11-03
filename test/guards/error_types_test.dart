// Everything a caller can catch is a PdfError. A `throw` of a core error
// type under lib/src reaches an app as a type it cannot handle by name, so
// the library throws a PdfError subtype, or the file is listed in
// `_allowed` with the reason no caller sees the throw.
//
// io-exempt: the source-scan wall reads lib/src from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _coreThrow = RegExp(
  r'\bthrow\s+(?:const\s+)?'
  r'(?:ArgumentError|StateError|UnsupportedError|Exception|FormatException|'
  r'RangeError)\b',
);

/// Files (or folders, ending in `/`) under lib/src that may throw a core
/// error type, each with the reason it never reaches a caller.
const _allowed = <String, String>{
  'lib/src/hook/':
      'runs in the build hook, at build time; an app never loads it',
  'lib/src/runtime/router.dart':
      'a debug-only programmer error: it fires under `assert` for an op the '
      'creator map misses, and release builds never throw it',
};

bool _isAllowed(String path) =>
    _allowed.keys.any((a) => a.endsWith('/') ? path.startsWith(a) : path == a);

/// `file:line: text` for each throw of a core error type in [files] (name to
/// source) that is not on the allow-list.
List<String> coreThrows(Map<String, String> files) {
  final found = <String>[];
  for (final MapEntry(key: name, value: raw) in files.entries) {
    if (_isAllowed(name)) continue;
    final text = stripComments(raw);
    final lines = text.split('\n');
    for (final m in _coreThrow.allMatches(text)) {
      final line = lineOf(text, m.start);
      found.add('$name:$line: ${lines[line - 1].trim()}');
    }
  }
  return found;
}

Map<String, String> _libSources() => {
  for (final f in Directory(
    'lib/src',
  ).listSync(recursive: true).whereType<File>())
    if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
};

void main() {
  test('no file under lib/src throws a core error type', () {
    expect(coreThrows(_libSources()), isEmpty);
  });

  test('every allow-list entry names a file that still throws one', () {
    final sources = _libSources();
    for (final entry in _allowed.keys) {
      final hits = sources.entries.where(
        (s) =>
            (entry.endsWith('/') ? s.key.startsWith(entry) : s.key == entry) &&
            _coreThrow.hasMatch(stripComments(s.value)),
      );
      expect(hits, isNotEmpty, reason: 'stale allow-list entry $entry');
    }
  });

  group('the wall sees a core throw', () {
    for (final type in const [
      'ArgumentError',
      'StateError',
      'UnsupportedError',
      'Exception',
      'FormatException',
      'RangeError',
    ]) {
      test(type, () {
        expect(
          coreThrows({'lib/src/ops/x.dart': "f() => throw $type('x');\n"}),
          hasLength(1),
        );
      });
    }

    test('a named constructor and a const throw', () {
      expect(
        coreThrows({
          'lib/src/ops/x.dart':
              'f() => throw RangeError.range(1, 2, 3);\n'
              "g() => throw const FormatException('x');\n",
        }),
        hasLength(2),
      );
    });

    test('a PdfError, a comment and an allow-listed file pass', () {
      expect(
        coreThrows({
          'lib/src/ops/x.dart':
              "f() => throw const PdfInvalidArgument('x');\n"
              "// throw StateError('x') in a comment\n",
          'lib/src/hook/x.dart': "f() => throw StateError('x');\n",
        }),
        isEmpty,
      );
    });
  });
}
