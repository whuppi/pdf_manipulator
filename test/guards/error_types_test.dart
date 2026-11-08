// Two kinds of failure, two kinds of type. A runtime failure is a PdfError,
// delivered through the task, and a `switch` over the sealed class is
// exhaustive. A programmer mistake is an ArgumentError or a StateError,
// thrown synchronously before any I/O, and never a PdfError.
//
// 1. Under lib/src/ops a `throw` of a core type names only ArgumentError or
//    StateError.
// 2. Under lib/src/bridge no `throw` names a core type: the bridge reports
//    through PdfError.
// 3. Elsewhere under lib/src a core throw is on the allow-list with the
//    reason it never reaches a caller as a runtime failure.
// 4. The subtypes of PdfError are exactly the eleven below, and none is named
//    for a kind of object ending in `Error`.
//
// io-exempt: the source-scan wall reads lib/src from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _coreThrow = RegExp(
  r'\bthrow\s+(?:const\s+)?'
  r'(ArgumentError|StateError|UnsupportedError|Exception|FormatException|'
  r'RangeError)\b',
);

/// The runtime failures a caller can switch over.
const _subtypes = <String>{
  'PdfIoFailed',
  'PdfCorrupted',
  'PdfEngineFailed',
  'PdfPageOutOfRange',
  'PdfNotFound',
  'PdfUnsupported',
  'PdfPasswordRequired',
  'PdfWrongPassword',
  'PdfCryptoFailed',
  'PdfIncrementalRefused',
  'PdfCancelled',
};

/// Files (or folders, ending in `/`) outside ops and bridge that may throw a
/// core error type, each with the reason.
const _allowed = <String, String>{
  'lib/src/hook/':
      'runs in the build hook, at build time; an app never loads it',
  'lib/src/runtime/router.dart':
      'a programmer error: the router is used after its Pdf was disposed, or '
      'an op the creator map misses under `assert`',
  'lib/src/types/pdf_pages.dart':
      'a value type refuses an invalid selection at construction, before any '
      'I/O: the design puts that check in the type',
  'lib/src/io/file_source.dart':
      'a programmer error: a read after the source was closed',
};

bool _isAllowed(String path) =>
    _allowed.keys.any((a) => a.endsWith('/') ? path.startsWith(a) : path == a);

/// `file:line: text` for each throw in [files] (name to source) that breaks
/// rules 1 to 3.
List<String> coreThrows(Map<String, String> files) {
  final found = <String>[];
  for (final MapEntry(key: name, value: raw) in files.entries) {
    final text = stripComments(raw);
    final lines = text.split('\n');
    final inOps = name.startsWith('lib/src/ops/');
    if (!inOps && _isAllowed(name)) continue;
    for (final m in _coreThrow.allMatches(text)) {
      if (inOps && const {'ArgumentError', 'StateError'}.contains(m.group(1))) {
        continue;
      }
      final line = lineOf(text, m.start);
      found.add('$name:$line: ${lines[line - 1].trim()}');
    }
  }
  return found;
}

final _extends = RegExp(r'class (\w+) extends PdfError\b');

/// The names of the classes in [source] that extend PdfError.
Set<String> errorSubtypes(String source) => {
  for (final m in _extends.allMatches(stripComments(source))) m.group(1)!,
};

Map<String, String> _libSources() => {
  for (final f in Directory(
    'lib/src',
  ).listSync(recursive: true).whereType<File>())
    if (f.path.endsWith('.dart')) f.path: f.readAsStringSync(),
};

void main() {
  test('no throw under lib/src breaks the two-kinds rule', () {
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

  test('the subtypes of PdfError are exactly the eleven', () {
    final found = errorSubtypes(
      File('lib/src/types/errors.dart').readAsStringSync(),
    );
    expect(found, _subtypes);
  });

  test('no subtype names itself an Error', () {
    expect(_subtypes.where((name) => name.endsWith('Error')), isEmpty);
  });

  group('the wall sees a violation', () {
    test('a core throw in the bridge, even a StateError', () {
      expect(
        coreThrows({
          'lib/src/bridge/x.dart': "f() => throw StateError('x');\n",
        }),
        hasLength(1),
      );
    });

    test('a core throw in ops other than the two', () {
      expect(
        coreThrows({
          'lib/src/ops/x.dart': "f() => throw UnsupportedError('x');\n",
        }),
        hasLength(1),
      );
    });

    test(
      'ArgumentError and StateError in ops, a comment and an allowed file pass',
      () {
        expect(
          coreThrows({
            'lib/src/ops/x.dart':
                "f() => throw ArgumentError('x');\n"
                "g() => throw StateError('x');\n"
                "// throw RangeError('x') in a comment\n",
            'lib/src/hook/x.dart': "f() => throw StateError('x');\n",
          }),
          isEmpty,
        );
      },
    );

    test('an extra subtype is seen', () {
      expect(
        errorSubtypes('class PdfOops extends PdfError {}\n'),
        isNot(_subtypes),
      );
    });
  });
}
