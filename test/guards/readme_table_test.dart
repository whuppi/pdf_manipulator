// R17: the README's "I want to..." table covers the surface. Every door on
// Pdf and every member of PdfReads and PdfEdits has a row, and every member a
// row names exists.
//
// io-exempt: the analyzer and the README are read from disk.

@TestOn('vm')
library;

import 'dart:io';

import 'package:test/test.dart';

import 'api_model.dart';

final _heading = RegExp(
  r'^#+ .*I want to',
  multiLine: true,
  caseSensitive: false,
);
// A call on a value (`e.rotatePages(`), never a factory on a type
// (`PdfPages.single(`) or a dot shorthand (`.r90`, `.all()`).
final _call = RegExp(r'\b[a-z]\w*\.(\w+)\(');

/// Members of Pdf that are not doors, each with the reason.
const notDoors = <String, String>{
  'ioMode': 'a fact of the instance',
  'ensureInitialized': 'starts the instance',
  'registerFallbackFont': 'set-up of the instance, not an operation on a PDF',
  'dispose': 'ends the instance',
};

/// The rows of the "I want to..." table in [readme], or null without one.
List<String>? readmeRows(String readme) {
  final h = _heading.firstMatch(readme);
  if (h == null) return null;
  final rows = <String>[];
  for (final line in readme.substring(h.end).split('\n').skip(1)) {
    final t = line.trim();
    if (t.startsWith('|')) {
      rows.add(t);
    } else if (rows.isNotEmpty) {
      break;
    }
  }
  return rows.skip(2).toList(); // header and separator
}

/// Problems with the table in [readme]: a member in a row that is not in
/// [known], or a member of [required] that no row mentions.
List<String> readmeProblems(
  String readme, {
  required Set<String> known,
  required Set<String> required,
}) {
  final rows = readmeRows(readme);
  if (rows == null) return ['README has no "I want to..." table'];
  final mentioned = <String>{};
  final problems = <String>[];
  for (final row in rows) {
    for (final m in _call.allMatches(row)) {
      final name = m.group(1)!;
      mentioned.add(name);
      if (!known.contains(name)) problems.add('row names unknown $name: $row');
    }
  }
  for (final member in required.difference(mentioned)) {
    problems.add('no row mentions $member');
  }
  return problems;
}

void main() {
  test('the README table names members that exist and misses none', () async {
    final model = await ApiModel.load();
    Set<String> names(String type) => {
      for (final m in model.membersOf(model.type(type)))
        if (!m.isConstructor) m.name,
    };
    final doors = names('Pdf').difference(notDoors.keys.toSet());
    final known = {
      ...names('Pdf'),
      ...names('PdfEdits'),
      ...names('PdfReads'),
      ...names('PdfEditor'),
      ...names('PdfDocument'),
      ...names('PdfBuilder'),
      ...names('PdfPageBuilder'),
      ...names('PdfEditorHandle'),
      ...names('PdfBuilderHandle'),
      ...names('PdfDocumentHandle'),
      ...names('PdfFormFieldEditor'),
    };
    expect(
      readmeProblems(
        File('README.md').readAsStringSync(),
        known: known,
        required: {...doors, ...names('PdfEdits'), ...names('PdfReads')},
      ),
      isEmpty,
    );
  });

  test('every non-door names a member of Pdf and has a reason', () async {
    final model = await ApiModel.load();
    final members = {
      for (final m in model.membersOf(model.type('Pdf'))) m.name,
    };
    for (final MapEntry(key: name, value: reason) in notDoors.entries) {
      expect(members, contains(name), reason: 'stale entry');
      expect(reason.trim(), isNotEmpty);
    }
  });

  group('the wall sees a violation', () {
    const table =
        '## I want to...\n\n'
        '| I want to | Do this |\n|---|---|\n'
        '| watermark | `pdf.rewrite(a, b, (e) => e.addWatermark("x"))` |\n'
        '| search | `r.search("q")` |\n\nnext\n';

    test('a complete table passes', () {
      expect(
        readmeProblems(
          table,
          known: {'rewrite', 'addWatermark', 'search'},
          required: {'addWatermark', 'search'},
        ),
        isEmpty,
      );
    });

    test('a row naming a member that does not exist', () {
      expect(
        readmeProblems(
          table,
          known: {'rewrite', 'search'},
          required: {'search'},
        ),
        hasLength(1),
      );
    });

    test('a member no row mentions', () {
      expect(
        readmeProblems(
          table,
          known: {'rewrite', 'addWatermark', 'search'},
          required: {'addWatermark', 'search', 'flatten'},
        ),
        ['no row mentions flatten'],
      );
    });

    test('no table', () {
      expect(readmeProblems('# R\n', known: {}, required: {}), hasLength(1));
    });
  });
}
