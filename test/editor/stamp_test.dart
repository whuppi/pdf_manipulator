import 'package:test/test.dart';
import 'package:pdf_manipulator/pdf_manipulator.dart';

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

  group('PdfEditorHandle.addStamp', () {
    test('adds stamp — output is larger', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      await handle.addStamp(
        0,
        stampType: 0, // Approved
        x: 100, y: 100, width: 200, height: 60,
      );
      final sink = TestPdfSink();
      await handle.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      handle.dispose();
    });

    test('stamp marks document as modified', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      expect(await handle.isModified, isFalse);
      await handle.addStamp(
        0,
        stampType: 1, // Experimental
        x: 50, y: 50, width: 150, height: 50,
      );
      expect(await handle.isModified, isTrue);
      handle.dispose();
    });

    test('stamp produces valid PDF', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      await handle.addStamp(
        0,
        stampType: 2, // NotApproved
        x: 200, y: 400, width: 200, height: 80,
      );
      final sink = TestPdfSink();
      await handle.save(sink);
      handle.dispose();

      final saved = sink.takeBytes();
      final doc = await pdf.open(sourceOf(saved));
      expect(doc.pageCount, equals(1));
    });

    test('custom stamp with name', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      await handle.addStamp(
        0,
        stampType: 14, // Custom
        customName: 'MyStamp',
        x: 100, y: 100, width: 200, height: 60,
      );
      final sink = TestPdfSink();
      await handle.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      handle.dispose();
    });

    test('stamp with custom opacity', () async {
      final handle = await Pdf.edit(sourceOf(minimalPdf));
      await handle.addStamp(
        0,
        stampType: 0,
        x: 100, y: 100, width: 200, height: 60,
        opacity: 0.5,
      );
      final sink = TestPdfSink();
      await handle.save(sink);
      final saved = sink.takeBytes();
      expect(saved.length, greaterThan(minimalPdf.length));
      handle.dispose();
    });
  });
}
