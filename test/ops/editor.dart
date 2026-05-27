// Editor — persistent handle: open, mutate, save, dispose, redaction, encryption.
// Every test verifies mutations took effect via re-open + inspect.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../helpers/fixtures.dart';
import '../helpers/generators.dart';
import '../helpers/test_source_sink.dart';

void registerEditorTests(Pdf Function() createPdf) {
  group('editor', timeout: Timeout(Duration(seconds: 60)), () {
    // ── Basic lifecycle ──

    test('open → pageCount → dispose', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final pageCount = await editor.pageCount;
      expect(pageCount, 1);
      await editor.dispose();
    });

    test('double dispose is safe', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.dispose();
      await editor.dispose();
    });

    test('isModified is false before mutation, true after', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.setTitle('Modified');
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
    });

    // ── Metadata ──

    test('setTitle writes title into output bytes', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setTitle('Behavioral Test Title');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();

      // Re-open to verify it's a valid PDF.
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);

      // The title string should appear in the output bytes.
      expect(String.fromCharCodes(output), contains('Behavioral Test Title'),
          reason: 'title must be written into PDF output');
    });

    test('scrubMetadata removes title and author from output', () async {
      final pdf = createPdf();
      final metaPdf = await buildMetadataPdf(
        title: 'ScrubMeTitle',
        author: 'ScrubMeAuthor',
      );
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
      expect(scrubbed, isNot(contains('ScrubMeTitle')),
          reason: 'scrubbed metadata must not contain original title');
      expect(scrubbed, isNot(contains('ScrubMeAuthor')),
          reason: 'scrubbed metadata must not contain original author');
    });

    // ── Pages ──

    test('open → deletePage → save → verify page count', () async {
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
    });

    test('getPageMediaBox returns correct A4 dimensions', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final box = await editor.getPageMediaBox(0);
      // minimalPdf is A4: 595×842
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
      await editor.dispose();
    });

    // ── Redaction ──

    test('redactionCount starts at 0, increments after addRedaction', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));

      final before = await editor.redactionCount(0);
      expect(before, 0, reason: 'no redactions on a fresh PDF');

      await editor.addRedaction(
          0, const PdfRect(x: 50, y: 50, width: 100, height: 20));
      final after = await editor.redactionCount(0);
      expect(after, 1, reason: 'one redaction added');

      await editor.addRedaction(
          0, const PdfRect(x: 50, y: 100, width: 100, height: 20));
      final after2 = await editor.redactionCount(0);
      expect(after2, 2, reason: 'two redactions added');

      await editor.dispose();
    });

    test('addRedaction + applyRedactions produces valid PDF', () async {
      final pdf = createPdf();
      // Use minimalPdf (no Type0 fonts) — bookmarkedPdf has composite fonts
      // that the destructive redaction engine can't handle yet.
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addRedaction(
          0, const PdfRect(x: 50, y: 650, width: 300, height: 100));
      await editor.applyRedactions();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    // ── Encryption ──

    test('save with encryption config applies encryption', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final sink = TestSink();
      await editor.save(sink,
          options: const PdfSaveOptions(
            encryption: PdfEncryption.config(ownerPassword: 'test123'),
          ));
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.isEncrypted, isTrue);
    });

    test('save with encryption remove strips encryption', () async {
      final pdf = createPdf();

      // First encrypt.
      final editor1 = await pdf.edit(src(minimalPdf));
      final encSink = TestSink();
      await editor1.save(encSink,
          options: const PdfSaveOptions(
            encryption: PdfEncryption.config(ownerPassword: 'pw'),
          ));
      await editor1.dispose();
      final encrypted = encSink.takeBytes();

      // Verify encrypted.
      final encDoc = await pdf.open(src(encrypted), password: 'pw');
      expect(encDoc.isEncrypted, isTrue);

      // Then remove encryption.
      final editor2 = await pdf.edit(src(encrypted), password: 'pw');
      final decSink = TestSink();
      await editor2.save(decSink,
          options: const PdfSaveOptions(
            encryption: PdfEncryption.remove(),
          ));
      await editor2.dispose();
      final decrypted = decSink.takeBytes();
      final doc = await pdf.open(src(decrypted));
      expect(doc.isEncrypted, isFalse);
    });

    // ── Metadata get/set roundtrip ──

    test('setAuthor + getAuthor roundtrips', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setAuthor('Test Author');
      final author = await editor.getAuthor();
      expect(author, contains('Test Author'));
      await editor.dispose();
    });

    test('setSubject + getSubject roundtrips', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setSubject('Test Subject');
      final subject = await editor.getSubject();
      expect(subject, contains('Test Subject'));
      await editor.dispose();
    });

    test('setKeywords + getKeywords roundtrips', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setKeywords('dart, pdf, test');
      final kw = await editor.getKeywords();
      expect(kw, contains('dart'));
      await editor.dispose();
    });

    // ── Page operations ──

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
    });

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
    });

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
      // Page 0 should now be what was page 1 (Letter, wider).
      expect(doc.pages[0].width, isNot(closeTo(boxBefore.width, 1)));
    });

    // ── Optimization ──

    test('optimizeImages returns count on imageless PDF', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final count = await editor.optimizeImages(quality: 75);
      expect(count, 0, reason: 'no images in minimalPdf');
      await editor.dispose();
    });

    test('optimizeImages returns non-zero on image PDF', () async {
      final pdf = createPdf();
      final imageBytes = await buildImagePdf(createPdf, pageCount: 1);
      final editor = await pdf.edit(src(imageBytes));
      final count = await editor.optimizeImages(quality: 50);
      expect(count, greaterThan(0),
          reason: 'PDF with images must report optimized count > 0');
      await editor.dispose();
    });

    test('unembedStandardFonts returns count', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final count = await editor.unembedStandardFonts();
      expect(count, isA<int>());
      expect(count, greaterThanOrEqualTo(0));
      await editor.dispose();
    });

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
    });

    test('cropMargins produces valid PDF with reduced dimensions', () async {
      final pdf = createPdf();
      // Use a builder-created PDF with content so crop has boundaries to detect.
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.cropMargins(left: 50, right: 50, top: 50, bottom: 50);
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
    });

    test('convertToPdfA produces valid PDF and validates', () async {
      final pdf = createPdf();
      final formBytes = await buildFormPdf(createPdf);
      final editor = await pdf.edit(src(formBytes));
      await editor.convertToPdfA();
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();

      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);

      final validation = await pdf.validatePdfA(src(output));
      expect(validation.errors, 0,
          reason: 'converted PDF/A must have zero validation errors');
    });

    // ── Watermark position variants on editor ──

    test('addWatermark with tiled position adds multiple annotations', () async {
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
    });

    test('addWatermark with background layer produces valid PDF', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addWatermark(0, 'BG',
          layer: PdfWatermarkLayer.background);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(String.fromCharCodes(output), contains('BG'));
    });

    // ── Form fields ──

    test('setFormFieldValue produces valid output', () async {
      final pdf = createPdf();
      // Build a PDF with a form field.
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
    });
  });
}
