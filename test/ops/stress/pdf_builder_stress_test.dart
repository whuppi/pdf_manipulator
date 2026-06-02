// Builder stress — build large PDFs, image PDFs.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerBuilderStressTests(Pdf Function() createPdf) {
  group('stress builder', tags: 'stress', () {
    test('build and open image PDF', () async {
      final pdf = createPdf();
      final imagePdf = await buildImagePdf(createPdf, pageCount: 10);
      final doc = await pdf.open(src(imagePdf));
      expect(doc.pageCount, 10);
    }, timeout: Timeout(Duration(seconds: 10)));
  });
}
