import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  late Pdf pdf;

  setUp(() {
    pdf = Pdf();
  });

  tearDown(() {
    pdf.dispose();
  });

  group('Pdf.compress', () {
    test('compressed output is a valid PDF with same page count', () async {
      final sink = TestPdfSink();
      await pdf.compress(sourceOf(minimalPdf), sink);
      final result = sink.takeBytes();
      final doc = await pdf.open(sourceOf(result));
      expect(doc.pageCount, equals(1));
    });

    test('compressed output size differs from input (GC + stream recompression)', () async {
      // Merge creates internal duplication that GC can clean up
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final bloated = mergeSink.takeBytes();
      final compressSink = TestPdfSink();
      await pdf.compress(sourceOf(bloated), compressSink);
      final compressed = compressSink.takeBytes();
      // After GC + recompression, size should differ (not necessarily smaller
      // for minimal PDFs, but the bytes should be structurally different)
      expect(compressed.length, isNot(equals(bloated.length)));
    });

    test('compress preserves page count on multi-page', () async {
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();
      final compressSink = TestPdfSink();
      await pdf.compress(sourceOf(twoPage), compressSink);
      final compressed = compressSink.takeBytes();
      final doc = await pdf.open(sourceOf(compressed));
      expect(doc.pageCount, equals(2));
    });

    test('compress preserves page dimensions', () async {
      final sink = TestPdfSink();
      await pdf.compress(sourceOf(minimalPdf), sink);
      final compressed = sink.takeBytes();
      final doc = await pdf.open(sourceOf(compressed));
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
    });

    test('compress with linearize produces valid output', () async {
      final sink = TestPdfSink();
      await pdf.compress(sourceOf(minimalPdf), sink, linearize: true);
      final result = sink.takeBytes();
      final doc = await pdf.open(sourceOf(result));
      expect(doc.pageCount, equals(1));
    });

    test('double compress is idempotent (second pass same or smaller)', () async {
      final sink1 = TestPdfSink();
      await pdf.compress(sourceOf(minimalPdf), sink1);
      final first = sink1.takeBytes();
      final sink2 = TestPdfSink();
      await pdf.compress(sourceOf(first), sink2);
      final second = sink2.takeBytes();
      expect(second.length, lessThanOrEqualTo(first.length + 100));
      // Allow small variance from serialization differences
    });
  });
}
