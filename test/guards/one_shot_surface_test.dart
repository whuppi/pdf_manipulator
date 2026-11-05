// The one-shot surface stays whole.
//
// Build pairing (R10): every `addXPage(DataSource, {Options page})` on the
//    builder has `xsToPdf(List<DataSource>, DataSink, {Options page,
//    bool deflate})` on Pdf with the same options type.
//
// io-exempt: the source-scan wall reads lib from disk.

@TestOn('vm')
library;

import 'dart:io';

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
    // The options parameter may carry a default value; the pair must
    // name the same options type and the same deflate flag.
    final partner = RegExp(
      '\\b$plural\\(\\s*List<DataSource> \\w+,\\s*DataSink \\w+,\\s*'
      '\\{\\s*($options) \\w+\\s*(?:=[^,}]*)?,\\s*bool deflate[^}]*\\}',
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

void main() {
  group('build pairing', () {
    test('every addXPage has its xsToPdf', () {
      final builder = File('lib/src/ops/pdf_builder.dart').readAsStringSync();
      final pdf = File('lib/src/ops/pdf.dart').readAsStringSync();
      expect(pairingProblems(builder, pdf), isEmpty);
    });

    test('a page builder with no plural is a problem', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePage page}) => x;',
          'PdfTask<void> merge(List<DataSource> a, DataSink o) => x;',
        ),
        hasLength(1),
      );
    });

    test('a plural with other options is a problem', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePage page}) => x;',
          'PdfTask<void> imagesToPdf(List<DataSource> i, DataSink o, {Other page, bool deflate}) => x;',
        ),
        hasLength(1),
      );
    });

    test('a matching plural passes', () {
      expect(
        pairingProblems(
          'PdfTask<void> addImagePage(DataSource image, {PdfImagePage page}) => x;',
          'PdfTask<void> imagesToPdf(List<DataSource> i, DataSink o, {PdfImagePage page, bool deflate}) => x;',
        ),
        isEmpty,
      );
    });
  });
}
