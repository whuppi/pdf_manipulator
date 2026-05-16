import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;
  late Uint8List fivePagePdf;

  setUp(() async {
    pdf = Pdf();
    fivePagePdf = await buildFivePagePdf();
  });

  tearDown(() {
    pdf.kill();
  });

  group('Pdf.splitBySize', () {
    test('splits into chunks that each fit under maxBytes', () async {
      // Each minimal page is roughly 200-400 bytes in the output.
      // Set a small maxBytes to force splitting.
      final chunks = await pdf.splitBySize(fivePagePdf, maxBytes: 500);
      // Should produce multiple chunks since 5 pages won't fit in 500 bytes
      expect(chunks.length, greaterThan(1));
      // Each chunk should be a valid PDF
      for (final chunk in chunks) {
        final doc = await pdf.open(chunk);
        expect(doc.pageCount, greaterThan(0));
      }
    });

    test('large maxBytes returns single chunk', () async {
      final chunks = await pdf.splitBySize(
        fivePagePdf,
        maxBytes: 1024 * 1024, // 1MB — way more than our tiny PDF
      );
      expect(chunks, hasLength(1));
      final doc = await pdf.open(chunks[0]);
      expect(doc.pageCount, equals(5));
    });

    test('total pages across chunks equals original', () async {
      final chunks = await pdf.splitBySize(fivePagePdf, maxBytes: 600);
      var totalPages = 0;
      for (final chunk in chunks) {
        final doc = await pdf.open(chunk);
        totalPages += doc.pageCount;
      }
      expect(totalPages, equals(5));
    });

    test('throws on maxBytes < 1', () {
      expect(
        () => pdf.splitBySize(minimalPdf, maxBytes: 0),
        throwsArgumentError,
      );
    });
  });
}
