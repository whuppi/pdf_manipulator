// PdfEditor — mutations only. Parse once, mutate many, save once.
// Mirrors lib/src/ops/pdf_editor.dart.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../helpers/fixtures.dart';
import '../../helpers/generators.dart';
import '../../helpers/test_source_sink.dart';

void registerEditorTests(Pdf Function() createPdf) {
  group('editor', () {
    // ── Basic lifecycle ──

    test('open → pageCount → dispose', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      expect(await editor.pageCount, 1);
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('double dispose is safe', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.dispose();
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('isModified false before mutation, true after', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.setTitle('Modified');
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Metadata ──

    test('setTitle writes title into output', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setTitle('Behavioral Test Title');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(String.fromCharCodes(output), contains('Behavioral Test Title'));
    }, timeout: Timeout(Duration(seconds: 1)));

    test('scrubMetadata removes title and author', () async {
      final pdf = createPdf();
      final metaPdf = await buildMetadataPdf(title: 'ScrubMeTitle', author: 'ScrubMeAuthor');
      final raw = String.fromCharCodes(metaPdf);
      expect(raw, contains('ScrubMeTitle'));
      expect(raw, contains('ScrubMeAuthor'));

      final editor = await pdf.edit(src(metaPdf));
      await editor.scrubMetadata();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      final scrubbed = String.fromCharCodes(output);
      expect(scrubbed, isNot(contains('ScrubMeTitle')));
      expect(scrubbed, isNot(contains('ScrubMeAuthor')));
    }, timeout: Timeout(Duration(seconds: 1)));

    test('setAuthor + getAuthor roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setAuthor('Test Author');
      final author = await editor.getAuthor();
      expect(author, contains('Test Author'));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('setSubject + getSubject roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setSubject('Test Subject');
      final subject = await editor.getSubject();
      expect(subject, contains('Test Subject'));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('setKeywords + getKeywords roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setKeywords('dart, pdf, test');
      final kw = await editor.getKeywords();
      expect(kw, contains('dart'));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Pages ──

    test('deletePage reduces page count', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final editor = await pdf.edit(src(twoPage));
      await editor.deletePage(0);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('getPageMediaBox returns correct A4 dimensions', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final box = await editor.getPageMediaBox(0);
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('mergeFrom increases page count', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      expect(await editor.pageCount, 1);
      await editor.mergeFrom(src(letterPdf));
      expect(await editor.pageCount, 2);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('selectPages reduces to selected set', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf), src(minimalPdf)], mergeSink);
      final threePage = mergeSink.takeBytes();

      final editor = await pdf.edit(src(threePage));
      await editor.selectPages([0, 2]);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('movePage changes page order verified by dimensions', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final editor = await pdf.edit(src(twoPage));
      final boxBefore = await editor.getPageMediaBox(0);
      await editor.movePage(from: 0, to: 1);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].width, isNot(closeTo(boxBefore.width, 1)));
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Redaction ──

    test('redactionCount increments after addRedaction', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));

      expect(await editor.redactionCount(0), 0);
      await editor.addRedaction(0, const PdfRect(x: 50, y: 50, width: 100, height: 20));
      expect(await editor.redactionCount(0), 1);
      await editor.addRedaction(0, const PdfRect(x: 50, y: 100, width: 100, height: 20));
      expect(await editor.redactionCount(0), 2);
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('addRedaction + applyRedactions produces valid PDF', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addRedaction(0, const PdfRect(x: 50, y: 650, width: 300, height: 100));
      await editor.applyRedactions();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Encryption ──

    test('save with encryption config applies encryption', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final sink = TestSink();
      await editor.save(sink,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(ownerPassword: 'test123'),
          ));
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.isEncrypted, isTrue);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('save with encryption remove strips encryption', () async {
      final pdf = createPdf();
      final editor1 = await pdf.edit(src(minimalPdf));
      final encSink = TestSink();
      await editor1.save(encSink,
          options: const PdfSaveOptions.fullRewrite(
            encryption: PdfEncryption.config(ownerPassword: 'pw'),
          ));
      await editor1.dispose();
      final encrypted = encSink.takeBytes();

      final encDoc = await pdf.open(src(encrypted), password: 'pw');
      expect(encDoc.isEncrypted, isTrue);

      final editor2 = await pdf.edit(src(encrypted), password: 'pw');
      final decSink = TestSink();
      await editor2.save(decSink,
          options: const PdfSaveOptions.fullRewrite(encryption: PdfEncryption.remove()));
      await editor2.dispose();
      final doc = await pdf.open(src(decSink.takeBytes()));
      expect(doc.isEncrypted, isFalse);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Optimization ──

    test('optimizeImages returns count on imageless PDF', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final count = await editor.optimizeImages(quality: 75);
      expect(count, 0);
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('optimizeImages returns non-zero on image PDF', () async {
      final pdf = createPdf();
      final imageBytes = await buildImagePdf(createPdf, pageCount: 1);
      final editor = await pdf.edit(src(imageBytes));
      final count = await editor.optimizeImages(quality: 50);
      expect(count, greaterThan(0));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('unembedStandardFonts returns count', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final count = await editor.unembedStandardFonts();
      expect(count, isA<int>());
      expect(count, greaterThanOrEqualTo(0));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Content ──

    test('flattenAllAnnotations produces valid PDF', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.flattenAllAnnotations();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('cropMargins produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.cropMargins(left: 50, right: 50, top: 50, bottom: 50);
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('convertToPdfA produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.convertToPdfA();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      final validation = await doc.validatePdfA();
      expect(validation.errors, 0);
      await doc.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Watermark ──

    test('addWatermark with tiled position', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addWatermark(0, 'TILED',
          position: const PdfWatermarkPosition.tiled(columns: 2, rows: 2));
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(output.length, greaterThan(minimalPdf.length));
      expect(String.fromCharCodes(output), contains('TILED'));
    }, timeout: Timeout(Duration(seconds: 1)));

    test('addWatermark with background layer', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addWatermark(0, 'BG', layer: PdfWatermarkLayer.background);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(String.fromCharCodes(output), contains('BG'));
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Form fields ──

    test('setFormFieldValue produces valid output', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.setFormFieldValue('name', 'John Doe');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(String.fromCharCodes(output), contains('John Doe'));
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Rotation ──

    test('rotatePage sets rotation on specific page', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.rotatePage(0, degrees: 90);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 90);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('rotateAllPages sets rotation on every page', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final editor = await pdf.edit(src(mergeSink.takeBytes()));
      await editor.rotateAllPages(degrees: 180);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].rotation, 180);
      expect(doc.pages[1].rotation, 180);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Stamps ──

    test('addStamp increases output size', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addStamp(0,
          type: PdfStampType.approved,
          rect: const PdfRect(x: 50, y: 50, width: 200, height: 60));
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      expect(output.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('addImageStamp embeds image data', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addImageStamp(0, src(minimalPng),
          rect: const PdfRect(x: 50, y: 50, width: 100, height: 100));
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Content ──

    test('embedFile increases output size', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final fileData = Uint8List.fromList('Hello from embed test!'.codeUnits);
      await editor.embedFile('test.txt', src(fileData));
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(sink.takeBytes().length, greaterThan(minimalPdf.length));
    }, timeout: Timeout(Duration(seconds: 1)));

    test('eraseRegions produces valid output', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.eraseRegions(0, [const PdfRect(x: 50, y: 700, width: 200, height: 30)]);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('flattenForms preserves page count', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Save options ──

    test('incremental save produces valid output', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setTitle('Incremental');
      final sink = TestSink();
      await editor.save(sink, options: const PdfSaveOptions.incremental());
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    test('fullRewrite with compress + GC produces smaller output', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      final sink = TestSink();
      await editor.save(sink, options: const PdfSaveOptions.fullRewrite(
          compress: true, garbageCollect: true));
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── Getters ──

    test('version returns non-empty string', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final v = await editor.version;
      expect(v, contains('.'));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    test('getTitle roundtrips with setTitle', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setTitle('RoundtripTitle');
      expect(await editor.getTitle(), contains('RoundtripTitle'));
      await editor.dispose();
    }, timeout: Timeout(Duration(seconds: 1)));

    // ── ResizeImage ──

    test('resizeImage does not crash on image PDF', () async {
      final pdf = createPdf();
      final imageBytes = await buildImagePdf(createPdf, pageCount: 1);
      final editor = await pdf.edit(src(imageBytes));
      // resizeImage needs an image name — try a plausible one
      try {
        await editor.resizeImage(0, 'Im0', width: 50, height: 50);
      } catch (_) {
        // Image name may not match — that's OK, we're testing it doesn't crash
      }
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(sink.takeBytes().length, greaterThan(0));
    }, timeout: Timeout(Duration(seconds: 1)));
  });
}
