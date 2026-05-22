// Smoke test for the new Layer 1 Pdf class using the new bridge.
// Proves B20: Pdf → PdfBridge → NativeBridge → Rust thread pool → engine.

import 'dart:typed_data';

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/api/pdf.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/api/types/pdf_params.dart';

import '../bridge/test_helpers.dart';

// Minimal valid PDF
final _minimalPdf = _buildMinimalPdf();

Uint8List _buildMinimalPdf() {
  final s = '%PDF-1.4\n'
      '1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n'
      '2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n'
      '3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n'
      'xref\n0 4\n'
      '0000000000 65535 f \n'
      '0000000009 00000 n \n'
      '0000000058 00000 n \n'
      '0000000115 00000 n \n'
      'trailer<</Size 4/Root 1 0 R>>\nstartxref\n183\n%%EOF';
  return Uint8List.fromList(s.codeUnits);
}

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() async {
    await pdf.dispose();
  });

  group('Pdf (new bridge)', () {
    test('open reads page count', () async {
      final doc = await pdf.open(src(_minimalPdf));
      expect(doc.pageCount, 1);
    });

    test('open reads version', () async {
      final doc = await pdf.open(src(_minimalPdf));
      expect(doc.version, contains('.'));
    });

    test('merge two PDFs produces valid output', () async {
      final sink = TestSink();
      await pdf.merge([src(_minimalPdf), src(_minimalPdf)], sink);
      final merged = sink.takeBytes();
      expect(merged.length, greaterThan(_minimalPdf.length));

      // Re-open the merged output
      final doc = await pdf.open(src(merged));
      expect(doc.pageCount, 2);
    });

    test('extract text returns string', () async {
      final text = await pdf.extract(src(_minimalPdf),
          pages: const PdfPages.all(),
          format: PdfExtractionFormat.text);
      expect(text, isA<String>());
    });

    test('deletePages removes a page', () async {
      // Create a 2-page PDF by merging
      final mergeSink = TestSink();
      await pdf.merge([src(_minimalPdf), src(_minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      // Delete page 0
      final deleteSink = TestSink();
      await pdf.deletePages(src(twoPage), deleteSink, pages: [0]);
      final result = deleteSink.takeBytes();

      final doc = await pdf.open(src(result));
      expect(doc.pageCount, 1);
    });

    test('render yields RenderedPage stream', () async {
      final pages = await pdf
          .render(src(_minimalPdf),
              pages: const PdfPages.single(0),
              size: const PdfRenderSize.thumbnail(100))
          .toList();
      expect(pages, hasLength(1));
      expect(pages.first.width, greaterThan(0));
      expect(pages.first.height, greaterThan(0));
    });

    test('dispose prevents further operations', () async {
      await pdf.dispose();
      expect(() => pdf.open(src(_minimalPdf)), throwsStateError);
    });
  });
}
