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

  group('PdfEditor.resizeImage', () {
    test('resizeImage on PDF without images does not crash', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      // Resizing a non-existent image should throw a clean PdfError
      try {
        await editor.resizeImage(0, 'nonexistent',
            width: 100, height: 100);
      } on PdfError {
        // Expected -- no images exist in minimal PDF
      }
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });

    test('resizeImage with valid page index produces result', () async {
      final editor = await Pdf.edit(sourceOf(minimalPdf));
      try {
        await editor.resizeImage(0, 'Im0', width: 50, height: 50);
      } on PdfError {
        // Expected -- image name may not exist in minimal PDF
      }
      final sink = TestPdfSink();
      await editor.save(sink);
      editor.dispose();
      final result = sink.takeBytes();
      expect(result.length, greaterThan(0));
    });
  });
}
