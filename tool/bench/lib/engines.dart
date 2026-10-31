import 'dart:typed_data';

import 'package:pdf_document/pdf_document.dart' as theirs;
import 'package:pdf_manipulator/pdf_manipulator.dart';

/// One compression run: an engine and the preset it ran with.
class Config {
  const Config(this.engine, this.preset, this._run);

  final String engine;
  final String preset;
  final Future<Uint8List> Function(Uint8List source) _run;

  Future<Uint8List> run(Uint8List source) => _run(source);
}

/// Every config the bench measures: pdf_manipulator's compress with each
/// image policy, and pdf_document's optimize with each preset.
List<Config> configs(Pdf pdf) => [
  _ours(pdf, 'lossless', PdfImagePolicy.lossless),
  _ours(pdf, 'screen', PdfImagePolicy.screen),
  _ours(pdf, 'ebook', PdfImagePolicy.ebook),
  _ours(pdf, 'printer', PdfImagePolicy.print),
  for (final preset in theirs.PdfCompressionPreset.values) _theirs(preset),
];

Config _ours(Pdf pdf, String name, PdfImagePolicy policy) => Config(
  'pdf_manipulator',
  name,
  (source) => compressWith(pdf, source, policy),
);

Config _theirs(theirs.PdfCompressionPreset preset) =>
    Config('pdf_document', preset.name, (source) async {
      final document = theirs.PdfDocument.open(source);
      return Uint8List.fromList(
        theirs.PdfCompressor.optimize(document, options: preset.options).bytes,
      );
    });

/// pdf_manipulator's compress under [policy], in memory.
Future<Uint8List> compressWith(
  Pdf pdf,
  Uint8List source,
  PdfImagePolicy policy,
) async {
  final sink = MemorySink();
  await pdf.compress(MemorySource(source), sink, images: policy);
  return sink.takeBytes();
}
