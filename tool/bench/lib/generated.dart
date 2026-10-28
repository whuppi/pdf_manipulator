import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';

import 'engines.dart';

/// A case the bench builds itself from a corpus file.
class GeneratedCase {
  const GeneratedCase(this.id, this.kind, this.baseId, this.build);

  final String id;
  final String kind;

  /// Corpus entry the case is built from.
  final String baseId;
  final Future<Uint8List> Function(Pdf pdf, Uint8List base) build;
}

final generatedCases = <GeneratedCase>[
  const GeneratedCase('merge-x5', 'merged', 'irs-f1040', _mergeFiveTimes),
  const GeneratedCase(
    'recompressed',
    'recompressed',
    'plos-0000308',
    _recompress,
  ),
  const GeneratedCase(
    'incremental-x2',
    'incremental',
    'irs-f1040',
    _saveIncrementallyTwice,
  ),
];

Future<Uint8List> _mergeFiveTimes(Pdf pdf, Uint8List base) async {
  final sink = MemorySink();
  await pdf.merge(List.filled(5, MemorySource(base)), sink);
  return sink.takeBytes();
}

/// Our own lossless output, compressed once more.
Future<Uint8List> _recompress(Pdf pdf, Uint8List base) =>
    compressWith(pdf, base, PdfImagePolicy.lossless);

Future<Uint8List> _saveIncrementallyTwice(Pdf pdf, Uint8List base) async {
  var current = base;
  for (final title in const ['first edit', 'second edit']) {
    final editor = await pdf.edit(MemorySource(current));
    await editor.setTitle(title);
    final sink = MemorySink();
    await editor.save(sink, options: const PdfSaveOptions.incremental());
    await editor.dispose();
    current = sink.takeBytes();
  }
  return current;
}
