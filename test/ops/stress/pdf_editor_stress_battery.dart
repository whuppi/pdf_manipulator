// CHARTER — scale only: mutations must survive a 1000-page foreign
// document within their time budgets. Mutation correctness lives in
// the core editor/sugar batteries (no duplicate claims here).

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/perf.dart';

void registerEditorStressTests(Pdf Function() createPdf) {
  group('stress editor', tags: 'stress', () {
    final largePdf = fThousandPage;

    // Observed op/calibration: 3.29, 3.36, 3.66.
    stressTest(
      'pageImages across the first 100 pages of a 1000-page PDF',
      createPdf,
      ratio: 11,
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(largePdf));
        for (var page = 0; page < 100; page++) {
          expect(await editor.pageImages(page), isEmpty, reason: 'page $page');
        }
        await editor.dispose();
      },
    );

    // Observed op/calibration: 142.74, 144.49, 134.63.
    stressTest('watermark 1000 pages', createPdf, ratio: 434, () async {
      final pdf = createPdf();
      final sink = TestSink();
      // Background layer draws into page content, so extraction can
      // prove the text landed — output size alone proves nothing.
      await pdf.watermark(
        src(largePdf),
        sink,
        text: 'CONFIDENTIAL',
        layer: PdfWatermarkLayer.background,
      );
      final out = sink.takeBytes();
      expect(out.length, greaterThan(largePdf.length));
      final doc = await pdf.open(src(out));
      expect(doc.pageCount, 1000);
      final midPage = await doc.extract(pages: const PdfPages.single(500));
      expect(
        midPage,
        contains('CONFIDENTIAL'),
        reason: 'a mid-document page must carry the watermark text',
      );
      await doc.dispose();
    });

    // Observed op/calibration: 19.14, 19.61, 19.66.
    stressTest(
      'encrypt then decrypt 1000-page PDF',
      createPdf,
      ratio: 59,
      () async {
        final pdf = createPdf();
        final encSink = TestSink();
        await pdf.encrypt(
          src(largePdf),
          encSink,
          encryption: const PdfEncryptionConfig(
            ownerPassword: 'owner',
            userPassword: 'user',
          ),
        );
        final decSink = TestSink();
        await pdf.decrypt(src(encSink.takeBytes()), decSink, password: 'owner');
        final doc = await pdf.open(src(decSink.takeBytes()));
        expect(doc.pageCount, 1000);
        await doc.dispose();
      },
    );

    // Observed op/calibration: 35.25, 38.36, 44.23.
    stressTest(
      'editor watermark first 10 pages',
      createPdf,
      ratio: 133,
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(largePdf));
        // Background layer draws into page content, so extraction can
        // prove every page took its edit.
        for (var i = 0; i < 10; i++) {
          await editor.addWatermark(
            i,
            'EDITED',
            layer: PdfWatermarkLayer.background,
          );
        }
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final out = sink.takeBytes();
        final doc = await pdf.open(src(out));
        expect(doc.pageCount, 1000);
        final firstTen = await doc.extract(pages: const PdfPages.range(0, 10));
        expect(
          firstTen,
          contains('EDITED'),
          reason: 'the edited pages must carry the watermark text',
        );
        await doc.dispose();
      },
    );

    // Observed op/calibration: 8.81, 9.28, 10.13.
    stressTest(
      'reduceImages over a 1000-page PDF completes and keeps placement',
      createPdf,
      ratio: 31,
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(largePdf));
        final before = await editor.pageImages(0);
        await editor.reduceImages(PdfImagePolicy.screen);
        final after = await editor.pageImages(0);
        expect(after, before);
        await editor.dispose();
      },
    );

    // Observed op/calibration: 16.96, 23.03, 26.16.
    stressTest('compress 1000-page PDF', createPdf, ratio: 79, () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.compress(src(largePdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1000, reason: 'compression must never cost a page');
      await doc.dispose();
    });

    // Observed op/calibration: 0.33, 0.34, 0.29.
    stressTest('flattenForms on form PDF', createPdf, ratio: 2, () async {
      final pdf = createPdf();
      final formPdf = fFormFields;
      final sink = TestSink();
      await pdf.flattenForms(src(formPdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    });
  });
}
