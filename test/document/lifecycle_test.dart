import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/memory_io.dart';
import '../helpers/pdf_fixtures.dart';

void main() {
  group('Pdf.kill', () {
    test('kill does not throw', () {
      final pdf = Pdf();
      pdf.dispose();
    });

    test('operations work on a new instance after killing old one', () async {
      final pdf1 = Pdf();
      pdf1.dispose();
      final pdf2 = Pdf();
      final doc = await pdf2.open(sourceOf(minimalPdf));
      expect(doc.pageCount, equals(1));
      pdf2.dispose();
    });

    test('double kill is safe', () {
      final pdf = Pdf();
      pdf.dispose();
      pdf.dispose();
    });

    test('heavy usage then kill then new instance works', () async {
      final pdf = Pdf();
      // Use the worker
      final mergeSink = TestPdfSink();
      await pdf.merge([sourceOf(minimalPdf), sourceOf(minimalPdf)], mergeSink);
      final rotSink = TestPdfSink();
      await pdf.rotatePages(sourceOf(minimalPdf), rotSink, pages: {0: 90});
      await pdf.extractText(sourceOf(minimalPdf));

      // Kill it
      pdf.dispose();

      // New instance should work
      final pdf2 = Pdf();
      final doc = await pdf2.open(sourceOf(minimalPdf));
      expect(doc.pageCount, equals(1));
      pdf2.dispose();
    });
  });
}
