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

  group('Pdf.watermarkPositioned', () {
    test('watermarked PDF is larger than original', () async {
      final sink = TestPdfSink();
      await pdf.watermarkPositioned(
        sourceOf(minimalPdf), sink,
        text: 'DRAFT',
        x: 100, y: 100, width: 400, height: 200,
      );
      final result = sink.takeBytes();
      expect(result.length, greaterThan(minimalPdf.length));
    });

    test('watermarked PDF preserves page count', () async {
      final sink = TestPdfSink();
      await pdf.watermarkPositioned(
        sourceOf(minimalPdf), sink,
        text: 'CONFIDENTIAL',
        x: 0, y: 0, width: 595, height: 842,
      );
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(1));
    });

    test('watermark on specific pages only', () async {
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();
      final sink0 = TestPdfSink();
      await pdf.watermarkPositioned(
        sourceOf(twoPage), sink0,
        text: 'FIRST', x: 50, y: 50, width: 200, height: 100, pages: [0],
      );
      final onlyPage0 = sink0.takeBytes();
      final sinkAll = TestPdfSink();
      await pdf.watermarkPositioned(
        sourceOf(twoPage), sinkAll,
        text: 'FIRST', x: 50, y: 50, width: 200, height: 100,
      );
      final bothPages = sinkAll.takeBytes();
      expect(bothPages.length, greaterThan(onlyPage0.length));
    });

    test('watermark with custom font size', () async {
      final sink = TestPdfSink();
      await pdf.watermarkPositioned(
        sourceOf(minimalPdf), sink,
        text: 'TEST', x: 100, y: 400, width: 300, height: 200, fontSize: 72,
      );
      final doc = await pdf.open(sourceOf(sink.takeBytes()));
      expect(doc.pageCount, equals(1));
    });
  });

  group('PdfEditorHandle.addWatermarkPositioned', () {
    test('adds positioned watermark — output is larger', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      await handle.addWatermarkPositioned(
        0, 'EDITED', x: 100, y: 100, width: 300, height: 200,
      );
      final sink = TestPdfSink();
      await handle.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      handle.dispose();
    });

    test('positioned watermark marks document as modified', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      expect(await handle.isModified, isFalse);
      await handle.addWatermarkPositioned(
        0, 'MOD', x: 0, y: 0, width: 200, height: 100,
      );
      expect(await handle.isModified, isTrue);
      handle.dispose();
    });
  });
}
