// The one-shot surface stays whole.
//
// 1. Build pairing: every `addXPage(DataSource, {Options page})` on the
//    builder has `xsToPdf(List<DataSource>, DataSink, {Options page,
//    bool deflate})` on Pdf with the same options type.
// 2. README table: the "I want to..." table names only members that exist,
//    and lists every member of PdfEdits and PdfReads once there is more
//    than the few the foundation and the render task declare.
//
// io-exempt: the source-scan wall reads lib and README.md from disk.
// VM-only: it reflects over the classes with dart:mirrors.

@TestOn('vm')
library;

import 'dart:io';
import 'dart:mirrors';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import 'source_scan.dart';

// ── Build pairing ──

final _addPage = RegExp(
  r'PdfTask<\w+> add(\w+)Page\(\s*DataSource \w+,\s*\{\s*(\w+) \w+[^}]*\}',
);

/// Problems with the `addXPage` / `xsToPdf` pairs of [builder] and [pdf]
/// sources: a missing partner, or a partner with other options.
List<String> pairingProblems(String builder, String pdf) {
  final b = stripComments(builder);
  final p = stripComments(pdf);
  final problems = <String>[];
  for (final m in _addPage.allMatches(b)) {
    final x = m.group(1)!;
    final options = m.group(2)!;
    final plural = '${x[0].toLowerCase()}${x.substring(1)}sToPdf';
    final partner = RegExp(
      '\\b$plural\\(\\s*List<DataSource> \\w+,\\s*DataSink \\w+,\\s*'
      '\\{\\s*($options) \\w+,\\s*bool deflate[^}]*\\}',
    );
    if (!partner.hasMatch(p)) {
      problems.add(
        'add${x}Page takes $options but Pdf has no '
        '$plural(List<DataSource>, DataSink, {$options page, bool deflate})',
      );
    }
  }
  return problems;
}

// ── README table ──

final _heading = RegExp(
  r'^#+ .*I want to',
  multiLine: true,
  caseSensitive: false,
);
final _call = RegExp(r'\.(\w+)\(');

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
/// [known], or a one-shot member that no row mentions. [foundation] is the
/// member count under which the table may still be missing: the members the
/// foundation and the render task declare, until the sweep writes the table.
List<String> readmeProblems(
  String readme, {
  required Set<String> known,
  required Set<String> oneShot,
  int foundation = 4,
}) {
  final rows = readmeRows(readme);
  if (rows == null) {
    return oneShot.length <= foundation
        ? []
        : [
            'README has no "I want to..." table, and there are ${oneShot.length} one-shot members',
          ];
  }
  final mentioned = <String>{};
  final problems = <String>[];
  for (final row in rows) {
    for (final m in _call.allMatches(row)) {
      final name = m.group(1)!;
      mentioned.add(name);
      if (!known.contains(name)) problems.add('row names unknown $name: $row');
    }
  }
  for (final member in oneShot.difference(mentioned)) {
    problems.add('no row mentions $member');
  }
  return problems;
}

Set<String> _public(Type type) => {
  for (final d in reflectClass(type).declarations.values)
    if (!d.isPrivate && !(d is MethodMirror && d.isConstructor))
      MirrorSystem.getName(d.simpleName).replaceAll('=', ''),
};

/// Names declared by `extension ... on Pdf` in [sources].
Set<String> _extensionMembers(Iterable<String> sources) => {
  for (final s in sources)
    for (final m in RegExp(
      r'^  (?:PdfTask<[^>]*(?:<[^>]*>)?[^>]*>|Future<[^>]*>|void) (\w+)[(<]',
      multiLine: true,
    ).allMatches(s))
      m.group(1)!,
};

void main() {
  group('build pairing', () {
    test('every addXPage has its xsToPdf', () {
      final builder = File('lib/src/ops/pdf_builder.dart').readAsStringSync();
      final sugar = File('lib/src/ops/pdf_sugar.dart').readAsStringSync();
      expect(pairingProblems(builder, sugar), isEmpty);
    });

    test('a page builder with no plural is a problem', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePageOptions page}) => x;',
          'PdfTask<void> merge(List<DataSource> a, DataSink o) => x;',
        ),
        hasLength(1),
      );
    });

    test('a plural with other options is a problem', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePageOptions page}) => x;',
          'PdfTask<void> imagesToPdf(List<DataSource> i, DataSink o, {Other page, bool deflate}) => x;',
        ),
        hasLength(1),
      );
    });

    test('a matching plural passes', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePageOptions page}) => x;',
          'PdfTask<void> imagesToPdf(List<DataSource> i, DataSink o, {PdfImagePageOptions page, bool deflate}) => x;',
        ),
        isEmpty,
      );
    });
  });

  group('README table', () {
    final known = {'rewrite', 'addWatermark', 'rotatePage', 'search'};
    const table =
        '## I want to...\n\n'
        '| I want to | Do this |\n|---|---|\n'
        '| watermark | `pdf.rewrite(a, b, (e) => e.addWatermark(0, "x"))` |\n'
        '| rotate | `pdf.rewrite(a, b, (e) => e.rotatePage(0, degrees: 90))` |\n'
        '| search | `r.search(query: "q")` |\n\nnext\n';

    test('the real README names only members that exist', () {
      final sources = [
        File('lib/src/ops/pdf_sugar.dart').readAsStringSync(),
        File('lib/src/ops/pdf_standalone.dart').readAsStringSync(),
      ];
      final oneShot = _public(PdfEdits).union(_public(PdfReads));
      final names = {
        ..._public(Pdf),
        ..._public(PdfEdits),
        ..._public(PdfReads),
        ..._public(PdfEditor),
        ..._public(PdfDoc),
        ..._public(PdfBuilder),
        ..._extensionMembers(sources),
      };
      expect(
        readmeProblems(
          File('README.md').readAsStringSync(),
          known: names,
          oneShot: oneShot,
        ),
        isEmpty,
      );
    });

    test('a complete table passes', () {
      expect(
        readmeProblems(
          table,
          known: known,
          oneShot: {'addWatermark', 'rotatePage', 'search'},
        ),
        isEmpty,
      );
    });

    test('a row naming a member that does not exist fails', () {
      expect(
        readmeProblems(
          table.replaceAll('rotatePage', 'rotatePages'),
          known: known,
          oneShot: {'addWatermark', 'search'},
        ),
        hasLength(1),
      );
    });

    test('a one-shot member no row mentions fails', () {
      expect(
        readmeProblems(
          table,
          known: known,
          oneShot: {'addWatermark', 'rotatePage', 'search', 'flatten'},
        ),
        ['no row mentions flatten'],
      );
    });

    test('no table is fine while the surface is the foundation', () {
      expect(
        readmeProblems('# R\n', known: known, oneShot: {'a', 'b', 'c', 'd'}),
        isEmpty,
      );
    });

    test('no table fails once the surface grows', () {
      expect(
        readmeProblems(
          '# R\n',
          known: known,
          oneShot: {'a', 'b', 'c', 'd', 'e'},
        ),
        hasLength(1),
      );
    });
  });
}
