// Editor stress — mutations on 1000-page PDF.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerEditorStressTests(Pdf Function() createPdf) {
  group('stress editor', tags: 'stress', () {
    late Uint8List largePdf;

    test('build 1000-page PDF', () async {
      largePdf = await buildLargePdf(createPdf, pageCount: 1000);
      expect(largePdf.length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 10)));

    test('watermark 1000 pages', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.watermark(src(largePdf), sink, text: 'CONFIDENTIAL');
      expect(sink.takeBytes().length, greaterThan(largePdf.length));
    }, timeout: Timeout(Duration(seconds: 10)));

    test('encrypt then decrypt 1000-page PDF', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.encrypt(src(largePdf), encSink,
          encryption: const PdfEncryptionConfig(
              ownerPassword: 'owner', userPassword: 'user'));
      final decSink = TestSink();
      await pdf.decrypt(src(encSink.takeBytes()), decSink, password: 'owner');
      final doc = await pdf.open(src(decSink.takeBytes()));
      expect(doc.pageCount, 1000);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 10)));

    test('editor watermark first 10 pages', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(largePdf));
      for (var i = 0; i < 10; i++) {
        await editor.addWatermark(i, 'EDITED');
      }
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1000);
    }, timeout: Timeout(Duration(seconds: 10)));

    test('compress 1000-page PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.compress(src(largePdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 10)));

    test('flattenForms on form PDF', () async {
      final pdf = createPdf();
      final formPdf = await buildFormPdf(createPdf);
      final sink = TestSink();
      await pdf.flattenForms(src(formPdf), sink);
      expect(sink.takeBytes().length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 10)));
  });
}
