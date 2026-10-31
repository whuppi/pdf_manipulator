// No handle outside `_own`. A handle the library opens is closed in one
// place, the `finally` of `_own`; a door call (`open(`, `edit(`, `build(`)
// anywhere else in lib/src/ops is a handle that can leak on error or cancel.
// Allowed: the first argument of `_own(`, and the three door bodies in
// Pdf, which create the handles for callers who own them.
//
// io-exempt: the source-scan wall reads lib/src/ops from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'source_scan.dart';

final _creator = RegExp(
  r'(?<![\w$.])(?:open|edit|build|openEditor|createBuilder)\('
  r'|\b(?:_?pdf|_bridge|this)\.(?:open|edit|build|openEditor|createBuilder)\(',
);
final _own = RegExp(r'\b_own\s*(?:<[^(]*>)?\(');
final _door = RegExp(
  r'PdfTask<(?:PdfDoc|PdfEditor|PdfBuilder)> (?:open|edit|build)\(',
);

/// `file:line: text` for each handle creator in [files] (name to source)
/// that is neither the opening argument of `_own` nor inside a door body.
List<String> handleCreatorViolations(Map<String, String> files) {
  final found = <String>[];
  for (final MapEntry(key: name, value: raw) in files.entries) {
    final text = stripComments(raw);
    final allowed = <(int, int)>[];
    for (final m in _own.allMatches(text)) {
      final open = m.end - 1;
      final close = matchingClose(text, open);
      var depth = 0;
      var end = close;
      for (var i = open; i < close; i++) {
        final c = text[i];
        if ('([{'.contains(c)) depth++;
        if (')]}'.contains(c)) depth--;
        if (c == ',' && depth == 1) {
          end = i;
          break;
        }
      }
      allowed.add((open, end));
    }
    for (final m in _door.allMatches(text)) {
      final paramsOpen = m.end - 1;
      final bodyOpen = text.indexOf('{', matchingClose(text, paramsOpen));
      allowed.add((m.start, matchingClose(text, bodyOpen)));
    }
    for (final m in _creator.allMatches(text)) {
      final inside = allowed.any((a) => m.start >= a.$1 && m.start <= a.$2);
      if (!inside) {
        final line = text.split('\n')[lineOf(text, m.start) - 1].trim();
        found.add('$name:${lineOf(text, m.start)}: $line');
      }
    }
  }
  return found;
}

Map<String, String> _opsSources() => {
  for (final f in Directory('lib/src/ops').listSync().whereType<File>())
    if (f.path.endsWith('.dart') && !f.path.endsWith('handle_backstop.dart'))
      f.path: f.readAsStringSync(),
};

void main() {
  test('no handle is opened outside _own and the doors', () {
    expect(handleCreatorViolations(_opsSources()), isEmpty);
  });

  group('the wall sees a leak', () {
    test('a bare edit call', () {
      expect(
        handleCreatorViolations({
          'x.dart':
              'Future<void> f(Pdf pdf) async {\n'
              '  final e = await pdf.edit(s);\n'
              '}\n',
        }),
        hasLength(1),
      );
    });

    test('a creator in the body of _own, not its opening argument', () {
      expect(
        handleCreatorViolations({
          'x.dart':
              'x() => _own(edit(a), (e, h) async {\n'
              '  await edit(b);\n'
              '});\n',
        }),
        hasLength(1),
      );
    });

    test('a creator as the opening argument of _own is fine', () {
      expect(
        handleCreatorViolations({
          'x.dart': 'x() => _own(open(a, password: p), (d, h) => d);\n',
        }),
        isEmpty,
      );
    });

    test('a comment is not a call', () {
      expect(
        handleCreatorViolations({'x.dart': '// pdf.edit(source) opens one\n'}),
        isEmpty,
      );
    });
  });
}
