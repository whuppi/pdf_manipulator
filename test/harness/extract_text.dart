// Extracts text into a MemorySink and hands back what it received.
//
// The library writes text to a sink, never to a string it builds itself, so
// a test that wants the text writes it into a MemorySink and takes it.

import 'package:pdf_manipulator/pdf_manipulator.dart';

/// The text of the pages named, as the engine wrote it; every page when
/// [pages] is all.
Future<String> extractTextToString(
  PdfReads reads, {
  PdfPages pages = const PdfPages.all(),
  PdfExtractionFormat format = PdfExtractionFormat.auto,
}) async {
  final sink = MemorySink();
  await reads.extractText(sink, pages: pages, format: format);
  return sink.takeString();
}
