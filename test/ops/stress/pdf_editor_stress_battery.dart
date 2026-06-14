// CHARTER — scale only: mutations must survive a 1000-page foreign
// document within their time budgets. Mutation correctness lives in
// the core editor/sugar batteries (no duplicate claims here).

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerEditorStressTests(Pdf Function() createPdf) {
  group('stress editor', tags: 'stress', () {
    final largePdf = fThousandPage;

    test('watermark 1000 pages', () async {
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
    }, timeout: t(2));

    test('encrypt then decrypt 1000-page PDF', () async {
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
    }, timeout: t(1));

    test('editor watermark first 10 pages', () async {
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
    }, timeout: t(1));

    test('compress 1000-page PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.compress(src(largePdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1000, reason: 'compression must never cost a page');
      await doc.dispose();
    }, timeout: t(1));

    test('flattenForms on form PDF', () async {
      final pdf = createPdf();
      final formPdf = fFormFields;
      final sink = TestSink();
      await pdf.flattenForms(src(formPdf), sink);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));
  });
}
