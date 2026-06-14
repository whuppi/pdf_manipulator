// CHARTER — scale only: OUR builder (creation is the subject, so it
// rightly feeds itself) must build many-page and image-heavy
// documents inline within time budgets, and the result must re-open
// with the right shape.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerBuilderStressTests(Pdf Function() createPdf) {
  group('stress builder', tags: 'stress', () {
    test('build and open a 10-page image PDF inline', () async {
      // Creation-as-subject: OUR builder constructs the document here,
      // in the test, with its own API — never via fixtures: creation
      // is this battery's subject, so the builder feeds itself.
      final pdf = createPdf();
      final builder = await pdf.build();
      for (var i = 0; i < 10; i++) {
        final page = await builder.addA4Page();
        await page.font('Helvetica', 12);
        await page.heading(1, 'Image page $i');
        await page.image(
          src(photoPng),
          const PdfRect(x: 72, y: 500, width: 128, height: 128),
        );
        await page.done();
      }
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 10);
      var images = 0;
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        expect(img.width, greaterThan(0));
        expect(img.data.length, greaterThan(0));
        images++;
      }
      expect(
        images,
        10,
        reason:
            'one embedded image per page — fewer means pages '
            'were built without their image',
      );
      await doc.dispose();
    }, timeout: t(1));
  });
}
