import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;
  late Uint8List fivePagePdf;

  setUp(() async {
    pdf = Pdf();
    fivePagePdf = await buildFivePagePdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.splitBySize', () {
    test('splits into chunks that each fit under maxBytes', () async {
      // Each minimal page is roughly 200-400 bytes in the output.
      // Set a small maxBytes to force splitting.
      final sinks = <TestPdfSink>[];
      await pdf.splitBySize(sourceOf(fivePagePdf), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, maxBytes: 500);
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      // Should produce multiple chunks since 5 pages won't fit in 500 bytes
      expect(chunks.length, greaterThan(1));
      // Each chunk should be a valid PDF
      for (final chunk in chunks) {
        final doc = await pdf.open(sourceOf(chunk));
        expect(doc.pageCount, greaterThan(0));
      }
    });

    test('large maxBytes returns single chunk', () async {
      final sinks = <TestPdfSink>[];
      await pdf.splitBySize(
        sourceOf(fivePagePdf),
        (i) {
          final s = TestPdfSink();
          sinks.add(s);
          return s;
        },
        maxBytes: 1024 * 1024, // 1MB — way more than our tiny PDF
      );
      final chunks = sinks.map((s) => s.takeBytes()).toList();
      expect(chunks, hasLength(1));
      final doc = await pdf.open(sourceOf(chunks[0]));
      expect(doc.pageCount, equals(5));
    });

    test('total pages across chunks equals original', () async {
      final sinks = <TestPdfSink>[];
      await pdf.splitBySize(sourceOf(fivePagePdf), (i) {
        final s = TestPdfSink();
        sinks.add(s);
        return s;
      }, maxBytes: 600);
      var totalPages = 0;
      for (final s in sinks) {
        final doc = await pdf.open(sourceOf(s.takeBytes()));
        totalPages += doc.pageCount;
      }
      expect(totalPages, equals(5));
    });

    test('throws on maxBytes < 1', () {
      expect(
        () => pdf.splitBySize(sourceOf(minimalPdf), (_) => TestPdfSink(),
            maxBytes: 0),
        throwsArgumentError,
      );
    });
  });
}
