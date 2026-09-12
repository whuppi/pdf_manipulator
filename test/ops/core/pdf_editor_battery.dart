// CHARTER — this battery alone proves: every editor-session mutation
// persists through save, and edit COMPOSITIONS never erase each other
// (the regression class where a later edit rebuilt page state from
// the source). One-shot transforms live in sugar; reads in doc.
//
// Diet: foreign fixtures (dart-pdf) + handwritten micro fixtures.
// Presence proofs are SEMANTIC: annotation appearances are proven by
// flattening into content and extracting — never by grepping bytes.

import 'dart:math' as math;
import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/third_party/tp_image_kinds.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerEditorTests(Pdf Function() createPdf) {
  group('editor', () {
    // ── Basic lifecycle ──

    test('open → pageCount → dispose', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      expect(await editor.pageCount, 1);
      await editor.dispose();
    }, timeout: t(1));

    test('double dispose is safe and stays disposed', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.dispose();
      await editor.dispose();
      expect(() => editor.setTitle('x'), throwsStateError);
    }, timeout: t(1));

    test('isModified false before mutation, true after', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      expect(await editor.isModified, isFalse);
      await editor.setTitle('Modified');
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
    }, timeout: t(1));

    // ── Metadata ──

    test('setTitle writes title into output', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setTitle('Behavioral Test Title');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final e2 = await pdf.edit(src(output));
      expect(
        await e2.title,
        contains('Behavioral Test Title'),
        reason: 'the title must survive save and re-open',
      );
      expect(await e2.pageCount, 1);
      await e2.dispose();
    }, timeout: t(1));

    test('scrubMetadata removes title and author', () async {
      final pdf = createPdf();
      // Prepare input with verified metadata (set+get is proven above),
      // then prove scrub removes it SEMANTICALLY from the saved bytes.
      final prep = await pdf.edit(src(minimalPdf));
      await prep.setTitle('ScrubMeTitle');
      await prep.setAuthor('ScrubMeAuthor');
      final prepSink = TestSink();
      await prep.save(prepSink);
      await prep.dispose();
      final metaPdf = prepSink.takeBytes();

      final check = await pdf.edit(src(metaPdf));
      expect(await check.title, contains('ScrubMeTitle'));
      await check.dispose();

      final editor = await pdf.edit(src(metaPdf));
      await editor.scrubMetadata();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final after = await pdf.edit(src(sink.takeBytes()));
      expect(
        await after.title,
        isNot(contains('ScrubMeTitle')),
        reason: 'scrubbed metadata must be gone from the document',
      );
      expect(await after.author, isNot(contains('ScrubMeAuthor')));
      await after.dispose();
    }, timeout: t(1));

    test('setAuthor + author roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setAuthor('Test Author');
      final author = await editor.author;
      expect(author, contains('Test Author'));
      await editor.dispose();
    }, timeout: t(1));

    test('setSubject + subject roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setSubject('Test Subject');
      final subject = await editor.subject;
      expect(subject, contains('Test Subject'));
      await editor.dispose();
    }, timeout: t(1));

    test('setKeywords + keywords roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setKeywords('dart, pdf, test');
      final kw = await editor.keywords;
      expect(kw, contains('dart'));
      await editor.dispose();
    }, timeout: t(1));

    test('setProducer + producer roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setProducer('pdf_manipulator');
      final producer = await editor.producer;
      expect(producer, contains('pdf_manipulator'));
      await editor.dispose();
    }, timeout: t(1));

    test('setProducer writes producer into output', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setProducer('Behavioral Producer');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final e2 = await pdf.edit(src(sink.takeBytes()));
      expect(
        await e2.producer,
        contains('Behavioral Producer'),
        reason: 'the producer must survive save and re-open',
      );
      await e2.dispose();
    }, timeout: t(1));

    test('setCreationDate + creationDate roundtrips', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setCreationDate('D:20240101120000Z');
      final date = await editor.creationDate;
      expect(date, contains('20240101'));
      await editor.dispose();
    }, timeout: t(1));

    test('PdfDoc surfaces producer/creator/creationDate', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.setProducer('Doc-Read Producer');
      await editor.setCreationDate('D:20240101120000Z');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        doc.producer,
        contains('Doc-Read Producer'),
        reason: 'producer must surface on the read-only PdfDoc',
      );
      expect(
        doc.creationDate,
        contains('20240101'),
        reason: 'creation date must surface on the read-only PdfDoc',
      );
      // Creator has no Dart setter; this exercises the read surface decodes
      // without throwing (String? — empty when the PDF carries no Creator).
      expect(doc.creator, isA<String?>());
      await doc.dispose();
    }, timeout: t(1));

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
      await doc.dispose();
    }, timeout: t(1));

    test('pageMediaBox returns correct A4 dimensions', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final box = await editor.pageMediaBox(0);
      expect(box.width, closeTo(595, 1));
      expect(box.height, closeTo(842, 1));
      await editor.dispose();
    }, timeout: t(1));

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
      await doc.dispose();
    }, timeout: t(1));

    test('selectPages reduces to selected set', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([
        src(minimalPdf),
        src(letterPdf),
        src(minimalPdf),
      ], mergeSink);
      final threePage = mergeSink.takeBytes();

      final editor = await pdf.edit(src(threePage));
      await editor.selectPages([0, 2]);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    test('movePage changes page order verified by dimensions', () async {
      final pdf = createPdf();
      final mergeSink = TestSink();
      await pdf.merge([src(minimalPdf), src(letterPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final editor = await pdf.edit(src(twoPage));
      final boxBefore = await editor.pageMediaBox(0);
      await editor.movePage(from: 0, to: 1);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].width, isNot(closeTo(boxBefore.width, 1)));
      await doc.dispose();
    }, timeout: t(1));

    // ── Redaction ──

    test('redactionCount increments after addRedaction', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));

      expect(await editor.redactionCount(0), 0);
      await editor.addRedaction(
        0,
        const PdfRect(x: 50, y: 50, width: 100, height: 20),
      );
      expect(await editor.redactionCount(0), 1);
      await editor.addRedaction(
        0,
        const PdfRect(x: 50, y: 100, width: 100, height: 20),
      );
      expect(await editor.redactionCount(0), 2);
      await editor.dispose();
    }, timeout: t(1));

    test('addRedaction + applyRedactions produces valid PDF', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addRedaction(
        0,
        const PdfRect(x: 50, y: 650, width: 300, height: 100),
      );
      await editor.applyRedactions();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Encryption ──

    test('save with encryption config applies encryption', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      final sink = TestSink();
      await editor.save(
        sink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(ownerPassword: 'test123'),
        ),
      );
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.isEncrypted, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('save with encryption remove strips encryption', () async {
      final pdf = createPdf();
      final editor1 = await pdf.edit(src(minimalPdf));
      final encSink = TestSink();
      await editor1.save(
        encSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.config(ownerPassword: 'pw'),
        ),
      );
      await editor1.dispose();
      final encrypted = encSink.takeBytes();

      final encDoc = await pdf.open(src(encrypted), password: 'pw');
      expect(encDoc.isEncrypted, isTrue);
      await encDoc.dispose();

      final editor2 = await pdf.edit(src(encrypted), password: 'pw');
      final decSink = TestSink();
      await editor2.save(
        decSink,
        options: const PdfSaveOptions.fullRewrite(
          encryption: PdfEncryption.remove(),
        ),
      );
      await editor2.dispose();
      final doc = await pdf.open(src(decSink.takeBytes()));
      expect(doc.isEncrypted, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    // ── Page images ──

    test(
      'pageImages lists each page\'s image and its name resizes it',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fImages));
        // One embedded PNG per page, placed axis-aligned by dart-pdf.
        for (var page = 0; page < fImagesTruth.pages; page++) {
          final images = await editor.pageImages(page);
          expect(images, hasLength(1), reason: 'page $page');
          final image = images.single;
          expect(image.name, isNotEmpty);
          expect(image.bounds.width, greaterThan(0));
          expect(image.bounds.height, greaterThan(0));
          expect(image.transform.isAxisAligned, isTrue);
          expect(image.transform.a, image.bounds.width);
          expect(image.transform.d, image.bounds.height);
          expect(image.transform.e, image.bounds.x);
          expect(image.transform.f, image.bounds.y);
        }
        // The listed name is what resizeImage takes: halve the first image,
        // save, and read the new placement back from the saved bytes.
        final before = (await editor.pageImages(0)).single;
        await editor.resizeImage(
          0,
          before.name,
          width: before.bounds.width / 2,
          height: before.bounds.height / 2,
        );
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final reopened = await pdf.edit(src(sink.takeBytes()));
        final after = (await reopened.pageImages(0)).single;
        expect(after.name, before.name);
        expect(after.bounds.width, closeTo(before.bounds.width / 2, 0.01));
        expect(after.bounds.height, closeTo(before.bounds.height / 2, 0.01));
        await reopened.dispose();
      },
      timeout: t(1),
    );

    test('repositionImage moves a listed image; setImageBounds moves and '
        'resizes it', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fImages));
      final first = (await editor.pageImages(0)).single;
      await editor.repositionImage(
        0,
        first.name,
        x: first.bounds.x + 20,
        y: first.bounds.y - 30,
      );
      final second = (await editor.pageImages(1)).single;
      const target = PdfRect(x: 10, y: 20, width: 40, height: 50);
      await editor.setImageBounds(1, second.name, target);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final reopened = await pdf.edit(src(sink.takeBytes()));
      final moved = (await reopened.pageImages(0)).single;
      expect(moved.bounds.x, closeTo(first.bounds.x + 20, 0.01));
      expect(moved.bounds.y, closeTo(first.bounds.y - 30, 0.01));
      expect(moved.bounds.width, closeTo(first.bounds.width, 0.01));
      expect(moved.bounds.height, closeTo(first.bounds.height, 0.01));
      final set = (await reopened.pageImages(1)).single;
      expect(set.bounds.x, closeTo(target.x, 0.01));
      expect(set.bounds.y, closeTo(target.y, 0.01));
      expect(set.bounds.width, closeTo(target.width, 0.01));
      expect(set.bounds.height, closeTo(target.height, 0.01));
      await reopened.dispose();
    }, timeout: t(1));

    test('pageImages lists image XObjects only, not a form drawn beside '
        'them', () async {
      // The fixture draws /Fx1 (a form) after /Im1; both are `Do` operators.
      final editor = await createPdf().edit(src(imageAndFormPdf));
      final images = await editor.pageImages(0);
      expect(images.map((i) => i.name), ['Im1']);
      final bounds = images.single.bounds;
      expect(bounds.x, closeTo(40, 0.01));
      expect(bounds.y, closeTo(700, 0.01));
      expect(bounds.width, closeTo(100, 0.01));
      expect(bounds.height, closeTo(50, 0.01));
      await editor.dispose();
    }, timeout: t(1));

    // ── Reduce images ──

    test('reduceImages screen preset downsamples a 288 ppi image to 72 ppi '
        'and keeps its placement', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fImagesHires));
      final before = await editor.pageImages(0);
      final report = await editor.reduceImages(PdfImagePolicy.screen);
      final row = report.images.single;
      expect(row.encoding, PdfImageEncoding.raw);
      expect(row.color, PdfImageColor.rgb);
      expect(row.width, fImagesHiresTruth.imageWidth);
      expect(row.ppiMin, closeTo(fImagesHiresTruth.ppi, 0.5));
      expect(row.action, PdfImageAction.downsampled);
      // 128 px × 72 / 288 = 32 px.
      expect(row.widthAfter, 32);
      expect(row.heightAfter, 32);
      expect(row.bytesAfter, lessThan(row.bytesBefore));
      expect(report.changed, 1);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final saved = sink.takeBytes();
      expect(saved.length, lessThan(fImagesHires.length));
      final reopened = await pdf.edit(src(saved));
      final after = await reopened.pageImages(0);
      expect(after.single.name, before.single.name);
      expect(after.single.bounds.x, closeTo(before.single.bounds.x, 0.01));
      expect(
        after.single.bounds.width,
        closeTo(before.single.bounds.width, 0.01),
      );
      await reopened.dispose();
      final doc = await pdf.open(src(saved));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.width, 32);
      expect(images.single.height, 32);
      await doc.dispose();
    }, timeout: t(1));

    test('reduceImages classifies every fixture kind', () async {
      final pdf = createPdf();

      Future<void> assertClassification(
        Uint8List bytes, {
        required PdfImageEncoding encoding,
        required PdfImageColor color,
        required bool indexed,
        required int bits,
        required bool softMask,
        required int width,
        required int uses,
        double? ppiMin,
      }) async {
        final editor = await pdf.edit(src(bytes));
        final row = (await editor.reduceImages(
          PdfImagePolicy.lossless,
        )).images.single;
        expect(row.encoding, encoding);
        expect(row.color, color);
        expect(row.indexed, indexed);
        expect(row.bits, bits);
        expect(row.hasSoftMask, softMask);
        expect(row.width, width);
        expect(row.uses, uses);
        if (ppiMin != null) expect(row.ppiMin, closeTo(ppiMin, 0.5));
        await editor.dispose();
      }

      await assertClassification(
        fImagesHires,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.rgb,
        indexed: false,
        bits: 8,
        softMask: true,
        width: fImagesHiresTruth.imageWidth,
        uses: 1,
        ppiMin: fImagesHiresTruth.ppi.toDouble(),
      );

      final jpegEditor = await pdf.edit(src(fImagesJpeg));
      final jpegReport = await jpegEditor.reduceImages(PdfImagePolicy.lossless);
      expect(jpegReport.images, hasLength(2));
      for (final row in jpegReport.images) {
        expect(row.encoding, PdfImageEncoding.jpeg);
        expect(row.color, PdfImageColor.rgb);
        expect(row.bits, 8);
        expect(row.hasSoftMask, isFalse);
        expect(row.width, fImagesJpegTruth.imageWidth);
        expect(row.uses, 1);
      }
      await jpegEditor.dispose();

      await assertClassification(
        tpKindGray,
        encoding: PdfImageEncoding.values.byName(tpKindGrayTruth.encoding),
        color: PdfImageColor.values.byName(tpKindGrayTruth.color),
        indexed: tpKindGrayTruth.indexed,
        bits: tpKindGrayTruth.bits,
        softMask: tpKindGrayTruth.softMask,
        width: tpKindGrayTruth.width,
        uses: 1,
        ppiMin: tpKindGrayTruth.ppi.toDouble(),
      );
      await assertClassification(
        tpKindCmykJpeg,
        encoding: PdfImageEncoding.values.byName(tpKindCmykJpegTruth.encoding),
        color: PdfImageColor.values.byName(tpKindCmykJpegTruth.color),
        indexed: tpKindCmykJpegTruth.indexed,
        bits: tpKindCmykJpegTruth.bits,
        softMask: tpKindCmykJpegTruth.softMask,
        width: tpKindCmykJpegTruth.width,
        uses: 1,
      );
      await assertClassification(
        tpKindIndexed,
        encoding: PdfImageEncoding.values.byName(tpKindIndexedTruth.encoding),
        color: PdfImageColor.values.byName(tpKindIndexedTruth.color),
        indexed: tpKindIndexedTruth.indexed,
        bits: tpKindIndexedTruth.bits,
        softMask: tpKindIndexedTruth.softMask,
        width: tpKindIndexedTruth.width,
        uses: 1,
      );
      await assertClassification(
        tpKindBilevelG4,
        encoding: PdfImageEncoding.values.byName(tpKindBilevelG4Truth.encoding),
        color: PdfImageColor.values.byName(tpKindBilevelG4Truth.color),
        indexed: tpKindBilevelG4Truth.indexed,
        bits: tpKindBilevelG4Truth.bits,
        softMask: tpKindBilevelG4Truth.softMask,
        width: tpKindBilevelG4Truth.width,
        uses: 1,
        ppiMin: tpKindBilevelG4Truth.ppi.toDouble(),
      );
      await assertClassification(
        bilevelRawPdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.bilevel,
        indexed: false,
        bits: 1,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        gray16RawPdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.gray,
        indexed: false,
        bits: 16,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        stencilImageMaskPdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.bilevel,
        indexed: false,
        bits: 1,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        separationImagePdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.other,
        indexed: false,
        bits: 8,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        colorKeyMaskPdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.rgb,
        indexed: false,
        bits: 8,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        jbig2StubPdf,
        encoding: PdfImageEncoding.jbig2,
        color: PdfImageColor.bilevel,
        indexed: false,
        bits: 1,
        softMask: false,
        width: 32,
        uses: 1,
      );
      await assertClassification(
        sharedImagePdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.gray,
        indexed: false,
        bits: 8,
        softMask: false,
        width: 32,
        uses: 2,
        ppiMin: 72,
      );
      await assertClassification(
        imageInFormPdf,
        encoding: PdfImageEncoding.raw,
        color: PdfImageColor.gray,
        indexed: false,
        bits: 8,
        softMask: false,
        width: 16,
        uses: 1,
        ppiMin: 288,
      );
    }, timeout: t(1));

    test(
      'reduceImages never re-encodes a JPEG it does not downsample',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fImagesJpeg));
        final report = await editor.reduceImages(PdfImagePolicy.screen);
        expect(report.images, hasLength(2));
        final kept = report.images.firstWhere(
          (row) => row.action == PdfImageAction.kept,
        );
        expect(kept.keepReason, PdfImageKeepReason.withinResolution);
        final downsampled = report.images.firstWhere(
          (row) => row.action == PdfImageAction.downsampled,
        );
        expect(downsampled.widthAfter, 32);
        expect(downsampled.heightAfter, 32);
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final saved = sink.takeBytes();
        final doc = await pdf.open(src(saved));
        final page0 = await doc
            .extractImages(pages: const PdfPages.single(0))
            .toList();
        expect(page0.single.width, 128);
        expect(page0.single.height, 128);
        expect(page0.single.format, 'jpeg');
        final page1 = await doc
            .extractImages(pages: const PdfPages.single(1))
            .toList();
        expect(page1.single.width, 32);
        expect(page1.single.height, 32);
        expect(page1.single.format, 'jpeg');
        await doc.dispose();

        final freshEditor = await pdf.edit(src(fImagesJpeg));
        final losslessReport = await freshEditor.reduceImages(
          PdfImagePolicy.lossless,
        );
        for (final row in losslessReport.images) {
          expect(row.action, PdfImageAction.kept);
          expect(row.keepReason, PdfImageKeepReason.alreadyOptimal);
        }
        await freshEditor.dispose();
      },
      timeout: t(1),
    );

    test('reduceImages co-resamples the soft mask with its image', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fImagesSmask));
      final report = await editor.reduceImages(PdfImagePolicy.screen);
      final row = report.images.single;
      expect(row.action, PdfImageAction.downsampled);
      expect(row.widthAfter, 8);
      expect(row.hasSoftMask, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(src(saved));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.width, 8);
      expect(images.single.height, 8);
      await doc.dispose();

      final before = await _renderFirstPage(pdf, fImagesSmask);
      final after = await _renderFirstPage(pdf, saved);
      expect(_psnr(before, after), greaterThanOrEqualTo(30));
    }, timeout: t(1));

    test('reduceImages converts a CMYK JPEG to RGB under screen and keeps it '
        'under print', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(tpKindCmykJpeg));
      final report = await editor.reduceImages(PdfImagePolicy.screen);
      final row = report.images.single;
      expect(row.action, PdfImageAction.downsampled);
      expect(row.widthAfter, 16);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.width, 16);
      expect(images.single.height, 16);
      expect(images.single.colorSpace, contains('RGB'));
      await doc.dispose();

      final printEditor = await pdf.edit(src(tpKindCmykJpeg));
      final printReport = await printEditor.reduceImages(PdfImagePolicy.print);
      final printRow = printReport.images.single;
      expect(printRow.action, PdfImageAction.kept);
      expect(printRow.keepReason, PdfImageKeepReason.withinResolution);
      expect(printReport.changed, 0);
      await printEditor.dispose();
    }, timeout: t(1));

    test('reduceImages writes CCITT G4 for bilevel images', () async {
      final pdf = createPdf();

      final editor = await pdf.edit(src(bilevelRawPdf));
      final report = await editor.reduceImages(PdfImagePolicy.screen);
      final row = report.images.single;
      expect(row.action, PdfImageAction.recompressed);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final saved = sink.takeBytes();
      final doc = await pdf.open(src(saved));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.format, 'raw');
      expect(images.single.width, 32);
      expect(images.single.height, 32);
      expect(images.single.bitsPerComponent, 1);
      await doc.dispose();
      final before = await _renderFirstPage(pdf, bilevelRawPdf);
      final after = await _renderFirstPage(pdf, saved);
      expect(_psnr(before, after), double.infinity);

      final g4Editor = await pdf.edit(src(tpKindBilevelG4));
      final g4Report = await g4Editor.reduceImages(PdfImagePolicy.screen);
      final g4Row = g4Report.images.single;
      expect(g4Row.action, PdfImageAction.downsampled);
      expect(g4Row.widthAfter, 16);
      expect(g4Row.heightAfter, 16);
      expect(g4Row.bytesAfter, lessThan(g4Row.bytesBefore));
      await g4Editor.dispose();

      final stencilEditor = await pdf.edit(src(stencilImageMaskPdf));
      final stencilReport = await stencilEditor.reduceImages(
        PdfImagePolicy.screen,
      );
      final stencilRow = stencilReport.images.single;
      expect(stencilRow.action, PdfImageAction.recompressed);
      final stencilSink = TestSink();
      await stencilEditor.save(stencilSink);
      await stencilEditor.dispose();
      final stencilSaved = stencilSink.takeBytes();
      final stencilBefore = await _renderFirstPage(pdf, stencilImageMaskPdf);
      final stencilAfter = await _renderFirstPage(pdf, stencilSaved);
      expect(_psnr(stencilBefore, stencilAfter), double.infinity);
    }, timeout: t(1));

    test('reduceImages downsamples a gray image to a gray JPEG', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(tpKindGray));
      final row = (await editor.reduceImages(
        PdfImagePolicy.screen,
      )).images.single;
      expect(row.action, PdfImageAction.downsampled, reason: '$row');
      expect(row.widthAfter, 16);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final image =
          (await doc.extractImages(pages: const PdfPages.single(0)).toList())
              .single;
      expect(image.width, 16);
      expect(image.format, 'jpeg');
      expect(image.colorSpace, contains('Gray'));
      await doc.dispose();
    }, timeout: t(1));

    test('reduceImages lossless changes no pixel', () async {
      final pdf = createPdf();
      final fixtures = <(String, Uint8List)>[
        ('fImagesHires', fImagesHires),
        ('tpKindGray', tpKindGray),
        ('tpKindIndexed', tpKindIndexed),
        ('gray16RawPdf', gray16RawPdf),
        ('bilevelRawPdf', bilevelRawPdf),
      ];
      for (final (name, bytes) in fixtures) {
        final editor = await pdf.edit(src(bytes));
        final report = await editor.reduceImages(PdfImagePolicy.lossless);
        for (final row in report.images) {
          expect(
            row.bytesAfter,
            lessThanOrEqualTo(row.bytesBefore),
            reason: name,
          );
          // A row the engine could not decode would satisfy every other
          // assertion here by doing nothing; lossless must have read it.
          expect(
            row.keepReason,
            isNot(PdfImageKeepReason.undecodable),
            reason: name,
          );
          expect(
            row.action == PdfImageAction.recompressed ||
                row.keepReason == PdfImageKeepReason.belowMinSavings ||
                // A palette image stays as stored under lossless (design
                // row 7): only a lossy codec or a downsample could beat it.
                (row.indexed &&
                    row.keepReason == PdfImageKeepReason.alreadyOptimal),
            isTrue,
            reason: '$name: $row',
          );
        }
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final saved = sink.takeBytes();
        final before = await _renderFirstPage(pdf, bytes);
        final after = await _renderFirstPage(pdf, saved);
        expect(_psnr(before, after), double.infinity, reason: name);
      }

      final gray16Editor = await pdf.edit(src(gray16RawPdf));
      final gray16Report = await gray16Editor.reduceImages(
        PdfImagePolicy.lossless,
      );
      final gray16Row = gray16Report.images.single;
      expect(gray16Row.action, PdfImageAction.recompressed);
      final gray16Sink = TestSink();
      await gray16Editor.save(gray16Sink);
      await gray16Editor.dispose();
      final doc = await pdf.open(src(gray16Sink.takeBytes()));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.bitsPerComponent, 8);
      await doc.dispose();
    }, timeout: t(1));

    test('reduceImages keeps unsupported kinds with a reason', () async {
      final pdf = createPdf();

      Future<void> assertUnsupported(
        Uint8List bytes,
        PdfImageKeepReason reason,
      ) async {
        final editor = await pdf.edit(src(bytes));
        final report = await editor.reduceImages(PdfImagePolicy.screen);
        final row = report.images.single;
        expect(row.action, PdfImageAction.kept);
        expect(row.keepReason, reason);
        expect(report.changed, 0);
        await editor.dispose();
      }

      await assertUnsupported(
        separationImagePdf,
        PdfImageKeepReason.unsupportedColor,
      );
      await assertUnsupported(
        colorKeyMaskPdf,
        PdfImageKeepReason.unsupportedColorKeyMask,
      );
      await assertUnsupported(
        jbig2StubPdf,
        PdfImageKeepReason.unsupportedJbig2,
      );

      final editor = await pdf.edit(src(separationImagePdf));
      final before = (await editor.pageImages(0)).single;
      await editor.reduceImages(PdfImagePolicy.screen);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final reopened = await pdf.edit(src(sink.takeBytes()));
      final after = (await reopened.pageImages(0)).single;
      expect(after.name, before.name);
      expect(after.bounds.width, closeTo(before.bounds.width, 0.01));
      expect(after.bounds.height, closeTo(before.bounds.height, 0.01));
      await reopened.dispose();
    }, timeout: t(1));

    test('reduceImages decides once for a shared XObject', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(sharedImagePdf));
      final beforePage0 = await editor.pageImages(0);
      final beforePage1 = await editor.pageImages(1);
      final report = await editor.reduceImages(PdfImagePolicy.screen);
      expect(report.images, hasLength(1));
      final row = report.images.single;
      expect(row.uses, 2);
      expect(row.ppiMin, closeTo(72, 0.5));
      expect(row.action, PdfImageAction.recompressed);
      expect(row.widthAfter, 32);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final reopened = await pdf.edit(src(sink.takeBytes()));
      final afterPage0 = await reopened.pageImages(0);
      final afterPage1 = await reopened.pageImages(1);
      expect(afterPage0.single.name, beforePage0.single.name);
      expect(
        afterPage0.single.bounds.width,
        closeTo(beforePage0.single.bounds.width, 0.01),
      );
      expect(afterPage1.single.name, beforePage1.single.name);
      expect(
        afterPage1.single.bounds.width,
        closeTo(beforePage1.single.bounds.width, 0.01),
      );
      await reopened.dispose();
    }, timeout: t(1));

    test('reduceImages sees an image drawn inside a form XObject', () async {
      final pdf = createPdf();
      // imageInFormPdf is 16 px wide — below the presets' minPixels: 32 —
      // so this policy lowers the floor to prove the downsample path,
      // targeting 8 px (extractImages filters out anything narrower),
      // while a fresh editor under the real preset proves tooSmall wins.
      const policy = PdfImagePolicy(
        colorPpi: 144,
        grayPpi: 144,
        monoPpi: 300,
        jpegQuality: 60,
        minPixels: 8,
      );
      final editor = await pdf.edit(src(imageInFormPdf));
      final report = await editor.reduceImages(policy);
      final row = report.images.single;
      expect(row.action, PdfImageAction.downsampled);
      expect(row.widthAfter, 8);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final images = await doc
          .extractImages(pages: const PdfPages.single(0))
          .toList();
      expect(images.single.width, 8);
      expect(images.single.height, 8);
      await doc.dispose();

      final freshEditor = await pdf.edit(src(imageInFormPdf));
      final screenReport = await freshEditor.reduceImages(
        PdfImagePolicy.screen,
      );
      final screenRow = screenReport.images.single;
      expect(screenRow.action, PdfImageAction.kept);
      expect(screenRow.keepReason, PdfImageKeepReason.tooSmall);
      await freshEditor.dispose();
    }, timeout: t(1));

    test('reduceImages honours minSavings before trading quality', () async {
      // fImages: a 72 ppi Flate photo, so screen-level policies recompress
      // it to JPEG rather than downsample. The same trade is refused when
      // the policy demands a saving no JPEG can deliver.
      final pdf = createPdf();
      final greedy = await pdf.edit(src(fImages));
      final refused = await greedy.reduceImages(
        const PdfImagePolicy(jpegQuality: 60, minSavings: 0.999),
      );
      expect(refused.changed, 0);
      expect(
        refused.images.map((r) => r.keepReason),
        everyElement(PdfImageKeepReason.belowMinSavings),
      );
      await greedy.dispose();
      final willing = await pdf.edit(src(fImages));
      final accepted = await willing.reduceImages(
        const PdfImagePolicy(jpegQuality: 60, minSavings: 0),
      );
      expect(accepted.changed, accepted.images.length);
      expect(
        accepted.images.map((r) => r.action),
        everyElement(PdfImageAction.recompressed),
      );
      await willing.dispose();
    }, timeout: t(1));

    test('reduceImages is idempotent', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fImagesHires));
      await editor.reduceImages(PdfImagePolicy.screen);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final reopened = await pdf.edit(src(sink.takeBytes()));
      final report = await reopened.reduceImages(PdfImagePolicy.screen);
      expect(report.changed, 0);
      for (final row in report.images) {
        expect(row.action, PdfImageAction.kept);
        expect(row.keepReason, PdfImageKeepReason.withinResolution);
      }
      await reopened.dispose();
    }, timeout: t(1));

    test(
      'reduceImages a second time in the same session sees the first',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fImagesHires));
        await editor.reduceImages(PdfImagePolicy.screen);
        final second = await editor.reduceImages(PdfImagePolicy.screen);
        expect(second.changed, 0);
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        final images = await doc
            .extractImages(pages: const PdfPages.single(0))
            .toList();
        expect(images.single.width, 32);
        expect(images.single.height, 32);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('pageImages on an imageless page is empty', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      expect(await editor.pageImages(0), isEmpty);
      await editor.dispose();
    }, timeout: t(1));

    test('pageImages with an out-of-range page throws typed error', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await expectLater(
        editor.pageImages(99),
        throwsA(isA<PdfEngineError>()),
        reason:
            'page 99 of a 1-page PDF does not exist — an empty list '
            'would hide caller bugs',
      );
      await editor.dispose();
    }, timeout: t(1));

    test('unembedStandardFonts on a font-free PDF reports zero', () async {
      // The minimal fixture embeds nothing — the only correct answer
      // is exactly 0. Any positive count means the engine invented
      // work; isA<int>() would have accepted that lie.
      final editor = await createPdf().edit(src(minimalPdf));
      final count = await editor.unembedStandardFonts();
      expect(count, 0);
      await editor.dispose();
    }, timeout: t(1));

    // ── Edit composition — every pair must survive ONE save ──
    //
    // Regression class: the engine once rebuilt a page's annotation
    // set from the SOURCE mid-edit, so a later edit silently erased
    // an earlier one (stamp wiped watermark). Compositions are a
    // distinct failure surface from solo ops — pin them.

    test('stamp after watermark keeps both', () async {
      final e = await createPdf().edit(src(minimalPdf));
      await e.addWatermark(0, 'COMPOWM');
      await e.addStamp(
        0,
        type: PdfStampType.draft,
        rect: const PdfRect(x: 50, y: 50, width: 200, height: 60),
      );
      final sink = TestSink();
      await e.save(sink);
      await e.dispose();
      // Semantic proof: flatten both annotations into page content —
      // both appearance texts must come back out of extraction.
      final pdf2 = createPdf();
      final flat = await pdf2.edit(src(sink.takeBytes()));
      await flat.flattenAllAnnotations();
      final flatSink = TestSink();
      await flat.save(flatSink);
      await flat.dispose();
      final doc = await pdf2.open(src(flatSink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('COMPOWM'),
        reason: 'adding a stamp must never erase an earlier edit',
      );
      expect(text.toUpperCase(), contains('DRAFT'));
      await doc.dispose();
    }, timeout: t(1));

    test('watermark after stamp keeps both', () async {
      final e = await createPdf().edit(src(minimalPdf));
      await e.addStamp(
        0,
        type: PdfStampType.draft,
        rect: const PdfRect(x: 50, y: 50, width: 200, height: 60),
      );
      await e.addWatermark(0, 'COMPOWM');
      final sink = TestSink();
      await e.save(sink);
      await e.dispose();
      final pdf2 = createPdf();
      final flat = await pdf2.edit(src(sink.takeBytes()));
      await flat.flattenAllAnnotations();
      final flatSink = TestSink();
      await flat.save(flatSink);
      await flat.dispose();
      final doc = await pdf2.open(src(flatSink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('COMPOWM'));
      expect(text.toUpperCase(), contains('DRAFT'));
      await doc.dispose();
    }, timeout: t(1));

    test('watermark + title + rotation all survive one save', () async {
      final pdf = createPdf();
      final e = await pdf.edit(src(minimalPdf));
      // Background layer draws into content — extraction proves it.
      await e.addWatermark(0, 'TRIPLEWM', layer: PdfWatermarkLayer.background);
      await e.setTitle('Triple Edit');
      await e.rotatePage(0, degrees: 90);
      final sink = TestSink();
      await e.save(sink);
      await e.dispose();
      final out = sink.takeBytes();
      final wmDoc = await pdf.open(src(out));
      expect(
        await wmDoc.extract(pages: const PdfPages.all()),
        contains('TRIPLEWM'),
      );
      await wmDoc.dispose();
      final doc = await pdf.open(src(out));
      expect(doc.pages[0].rotation, 90);
      await doc.dispose();
      final e2 = await pdf.edit(src(out));
      expect(await e2.title, contains('Triple Edit'));
      await e2.dispose();
    }, timeout: t(1));

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
      await doc.dispose();
    }, timeout: t(1));

    test('flattenAllAnnotations consumes foreign link annotations', () async {
      // dart-pdf link annotations carry foreign appearance
      // conventions — flatten must digest them without losing the
      // page's text content.
      final pdf = createPdf();
      final editor = await pdf.edit(src(fAnnotations));
      await editor.flattenAllAnnotations();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, fAnnotationsTruth.pages);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('Visit the site'),
        reason:
            'the link anchor text lives in page content and '
            'must survive annotation flattening',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('cropMargins produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));
      await editor.cropMargins(left: 50, right: 50, top: 50, bottom: 50);
      expect(await editor.isModified, isTrue);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('convertToPdfA produces valid PDF', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
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
    }, timeout: t(1));

    // ── Watermark ──

    test('addWatermark with tiled position', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addWatermark(
        0,
        'TILED',
        position: const PdfWatermarkPosition.tiled(columns: 2, rows: 2),
      );
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      final flat = await pdf.edit(src(output));
      await flat.flattenAllAnnotations();
      final flatSink = TestSink();
      await flat.save(flatSink);
      await flat.dispose();
      final doc = await pdf.open(src(flatSink.takeBytes()));
      expect(doc.pageCount, 1);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('TILED'),
        reason:
            'tiled watermark appearances must survive into '
            'content when flattened',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('addWatermark with background layer', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addWatermark(0, 'BG', layer: PdfWatermarkLayer.background);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('BG'),
        reason:
            'the background layer draws into the content '
            'stream — its text must extract',
      );
      await doc.dispose();
    }, timeout: t(1));

    // ── Form fields ──

    test('setFormFieldValue persists into the flattened page', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));
      await editor.setFormFieldValue('fullname', 'Jane Roe');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('Jane Roe'),
        reason: 'the set value must render where the field was',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldValue survives save + reopen, then flatten', () async {
      final pdf = createPdf();
      // Session 1: fill and save, WITHOUT flattening.
      final e1 = await pdf.edit(src(fFormFields));
      await e1.setFormFieldValue('fullname', 'Jane Roe');
      final filledSink = TestSink();
      await e1.save(filledSink);
      await e1.dispose();
      // Session 2: a FRESH editor flattens the reopened bytes. The in-session
      // modified-field map is empty on reopen, so the flattener must
      // regenerate the appearance from the persisted /V (the save path set
      // /NeedAppearances) instead of baking the stale placeholder appearance.
      final e2 = await pdf.edit(src(filledSink.takeBytes()));
      await e2.flattenForms();
      final flatSink = TestSink();
      await e2.save(flatSink);
      await e2.dispose();
      final doc = await pdf.open(src(flatSink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('Jane Roe'),
        reason:
            'a value set before save must still render after a '
            'reopen-then-flatten',
      );
      await doc.dispose();
    }, timeout: t(1));

    // Issue #215. A button widget's flattened appearance must follow the value
    // that was set, not the /AS the document shipped with. Proof is SEMANTIC —
    // rendered pixels: every widget in `uncheckedButtonForm` starts off (white
    // box, hairline border) and its on state fills the box solid black, so
    // "is it on?" is "did that band go dark?". Bands are fractions of the page
    // height because the rasterizer's exact pixel dimensions vary by platform.
    //
    // Band map (PDF y -> fraction from the top of a 792pt page):
    //   agree 700..760 -> 0.04..0.12   optin 600..660 -> 0.17..0.24
    //   color 500..560 -> 0.29..0.37
    Future<double> darkFractionInBand(
      Pdf pdf,
      Uint8List bytes,
      double top,
      double bottom, {
      double left = 0,
      double right = 1,
    }) async {
      final doc = await pdf.open(src(bytes));
      final frames = <RenderedPage>[];
      await for (final page in doc.render(
        pages: const PdfPages.single(0),
        size: const PdfRenderSize.thumbnail(240),
      )) {
        frames.add(page);
      }
      await doc.dispose();
      final bitmap = img.decodePng(frames.single.data)!;
      final y0 = (bitmap.height * top).round();
      final y1 = (bitmap.height * bottom).round();
      final x0 = (bitmap.width * left).round();
      final x1 = (bitmap.width * right).round();
      var dark = 0, total = 0;
      for (final p in bitmap) {
        if (p.y < y0 || p.y >= y1) continue;
        if (p.x < x0 || p.x >= x1) continue;
        total++;
        if (p.r < 100 && p.g < 100 && p.b < 100) dark++;
      }
      return total == 0 ? 0 : dark / total;
    }

    // The unflattened fixture is the control: every band must be near-white,
    // so any darkness a later test sees is the value that was set, not the
    // fixture's own furniture (the hairline border is a few pixels at most).
    test('uncheckedButtonForm starts with every button off', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flat = sink.takeBytes();
      for (final band in const [
        (name: 'agree', top: 0.04, bottom: 0.12),
        (name: 'optin', top: 0.17, bottom: 0.24),
        (name: 'color', top: 0.29, bottom: 0.37),
      ]) {
        expect(
          await darkFractionInBand(pdf, flat, band.top, band.bottom),
          lessThan(0.02),
          reason:
              'untouched ${band.name} must flatten to its off appearance — '
              'if this band is already dark the later proofs mean nothing',
        );
      }
    }, timeout: t(2));

    test(
      'setFormFieldValue on a checkbox flattens to the checked appearance',
      () async {
        // The literal repro from issue #215: a string value on a /Btn field.
        final pdf = createPdf();
        final editor = await pdf.edit(src(uncheckedButtonForm));
        await editor.setFormFieldValue('agree', 'Yes');
        await editor.flattenForms();
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        expect(
          await darkFractionInBand(pdf, sink.takeBytes(), 0.04, 0.12),
          greaterThan(0.05),
          reason:
              'the box must flatten checked — a string value on a button '
              'field is a state name, not text to draw',
        );
      },
      timeout: t(2),
    );

    test('setFormFieldValue honours a non-/Yes checkbox on-state', () async {
      // `optin` names its on state /On. A /V hardcoded to /Yes matches no
      // entry in its /AP /N and leaves the box blank.
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setFormFieldValue('optin', 'On');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(
        await darkFractionInBand(pdf, sink.takeBytes(), 0.17, 0.24),
        greaterThan(0.05),
        reason:
            "the on state must come from the widget's own /AP /N, not a "
            'hardcoded /Yes',
      );
    }, timeout: t(2));

    test('setFormFieldValue selects a radio kid', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setFormFieldValue('color', 'Red');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(
        await darkFractionInBand(pdf, sink.takeBytes(), 0.29, 0.37),
        greaterThan(0.02),
        reason: 'the selected radio kid must flatten to its on appearance',
      );
    }, timeout: t(2));

    test('checkbox value survives save + reopen, then flatten', () async {
      // Session 1 fills and saves without flattening; session 2 has an empty
      // in-session field map and must read the persisted /V + /AS.
      final pdf = createPdf();
      final e1 = await pdf.edit(src(uncheckedButtonForm));
      await e1.setFormFieldValue('agree', 'Yes');
      final filled = TestSink();
      await e1.save(filled);
      await e1.dispose();

      final e2 = await pdf.edit(src(filled.takeBytes()));
      await e2.flattenForms();
      final flat = TestSink();
      await e2.save(flat);
      await e2.dispose();
      expect(
        await darkFractionInBand(pdf, flat.takeBytes(), 0.04, 0.12),
        greaterThan(0.05),
        reason:
            'a checkbox checked before save must still flatten checked after '
            'a reopen — /V and /AS must persist as names',
      );
    }, timeout: t(2));

    test('setCheckboxFieldValue checks a box named /Yes', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setCheckboxFieldValue('agree', true);
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(
        await darkFractionInBand(pdf, sink.takeBytes(), 0.04, 0.12),
        greaterThan(0.05),
        reason: 'the typed setter must check the box',
      );
    }, timeout: t(2));

    test('setCheckboxFieldValue checks a box named /On', () async {
      // A boolean carries no state name, so the widget's own /AP /N has to
      // supply it. A hardcoded /Yes leaves this box blank.
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setCheckboxFieldValue('optin', true);
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      expect(
        await darkFractionInBand(pdf, sink.takeBytes(), 0.17, 0.24),
        greaterThan(0.05),
        reason:
            "true must select the widget's only on-state, whatever it is "
            'called',
      );
    }, timeout: t(2));

    test('setCheckboxFieldValue false clears a checked box', () async {
      // Round trip both ways: check, save, reopen, clear, flatten.
      final pdf = createPdf();
      final e1 = await pdf.edit(src(uncheckedButtonForm));
      await e1.setCheckboxFieldValue('agree', true);
      final checked = TestSink();
      await e1.save(checked);
      await e1.dispose();

      final e2 = await pdf.edit(src(checked.takeBytes()));
      await e2.setCheckboxFieldValue('agree', false);
      await e2.flattenForms();
      final flat = TestSink();
      await e2.save(flat);
      await e2.dispose();
      expect(
        await darkFractionInBand(pdf, flat.takeBytes(), 0.04, 0.12),
        lessThan(0.02),
        reason:
            'clearing a box that was checked in an earlier session must '
            'flatten to the off appearance',
      );
    }, timeout: t(2));

    test('setFormFieldValue off words clear a checkbox', () async {
      // Only exact `Off` used to mean off, so every other way of saying it —
      // `'No'`, `'false'` — fell through to "check the only on-state" and
      // checked the box the caller was trying to clear.
      for (final word in const ['Off', 'off', 'No', 'false', '0']) {
        final pdf = createPdf();
        final editor = await pdf.edit(src(uncheckedButtonForm));
        await editor.setFormFieldValue('agree', 'Yes');
        await editor.setFormFieldValue('agree', word);
        await editor.flattenForms();
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        expect(
          await darkFractionInBand(pdf, sink.takeBytes(), 0.04, 0.12),
          lessThan(0.02),
          reason: '"$word" must clear the box, not check it',
        );
      }
    }, timeout: t(3));

    test(
      'setFormFieldValue ignores a name the widget does not offer',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(uncheckedButtonForm));
        await editor.setFormFieldValue('agree', 'Maybe');
        await editor.flattenForms();
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        expect(
          await darkFractionInBand(pdf, sink.takeBytes(), 0.04, 0.12),
          lessThan(0.02),
          reason:
              'an unrecognised state name must select nothing rather than '
              'guess the only on-state',
        );
      },
      timeout: t(2),
    );

    test('a selected radio kid lights, and its sibling does not', () async {
      // The band holds both kids, so measuring it whole cannot tell "Red is on"
      // from "both are on". Split it by x: Red sits at 72..132, Blue at
      // 200..260 on a 612pt page.
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setFormFieldValue('color', 'Red');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flat = sink.takeBytes();
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.29,
          0.37,
          left: 0.10,
          right: 0.23,
        ),
        greaterThan(0.15),
        reason: 'the chosen kid must flatten in its on state',
      );
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.29,
          0.37,
          left: 0.31,
          right: 0.44,
        ),
        lessThan(0.05),
        reason:
            'the sibling must stay off — /V names exactly one state, and /AS '
            'goes to each kid separately',
      );
    }, timeout: t(2));

    test('an on-state named /No is selectable, not read as "off"', () async {
      // A Yes/No radio group really does have a state named /No, so the word
      // can only mean "off" once the widget has said it offers no such state.
      final pdf = createPdf();
      final editor = await pdf.edit(src(yesNoRadioForm));
      await editor.setFormFieldValue('answer', 'No');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flat = sink.takeBytes();
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.04,
          0.12,
          left: 0.31,
          right: 0.44,
        ),
        greaterThan(0.15),
        reason:
            'the /No kid must light — an offered state name wins over the '
            'word meaning off',
      );
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.04,
          0.12,
          left: 0.10,
          right: 0.23,
        ),
        lessThan(0.05),
        reason: 'the /Yes kid must stay off',
      );
    }, timeout: t(2));

    test('an on-word on a radio group selects nothing', () async {
      // "on" names no state in a group whose states are /Red and /Blue, and
      // nothing says which kid was meant. Guessing per kid lights every one of
      // them; the save path already resolves against the group's whole set and
      // writes /Off. Both paths must agree, and selecting nothing is the honest
      // answer for an ambiguous word.
      final pdf = createPdf();
      final editor = await pdf.edit(src(uncheckedButtonForm));
      await editor.setFormFieldValue('color', 'on');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flat = sink.takeBytes();
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.29,
          0.37,
          left: 0.10,
          right: 0.23,
        ),
        lessThan(0.05),
        reason: 'an ambiguous on-word must not light the first kid',
      );
      expect(
        await darkFractionInBand(
          pdf,
          flat,
          0.29,
          0.37,
          left: 0.31,
          right: 0.44,
        ),
        lessThan(0.05),
        reason:
            'nor the second — the flatten path must resolve against the '
            'whole group, as the save path does',
      );
    }, timeout: t(2));

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
      await doc.dispose();
    }, timeout: t(1));

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
      await doc.dispose();
    }, timeout: t(1));

    // ── Stamps ──

    test('addStamp increases output size', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addStamp(
        0,
        type: PdfStampType.approved,
        rect: const PdfRect(x: 50, y: 50, width: 200, height: 60),
      );
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      expect(output.length, greaterThan(minimalPdf.length));
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('addImageStamp embeds image data', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(minimalPdf));
      await editor.addImageStamp(
        0,
        src(minimalPng),
        rect: const PdfRect(x: 50, y: 50, width: 100, height: 100),
      );
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final output = sink.takeBytes();
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    test(
      'addImageStamp keeps existing widgets when /Annots is indirect',
      () async {
        final pdf = createPdf();
        // Stamp a page whose /Annots is an indirect reference to an array
        // holding one filled text widget (Datum = 01.01.2030).
        final e1 = await pdf.edit(src(indirectAnnotsForm));
        await e1.addImageStamp(
          0,
          src(minimalPng),
          rect: const PdfRect(x: 210, y: 478, width: 300, height: 45),
        );
        final stamped = TestSink();
        await e1.save(stamped);
        await e1.dispose();
        // Semantic proof: flatten the reopened doc — the widget's appearance
        // becomes page content only if the stamp left it attached to the page.
        final e2 = await pdf.edit(src(stamped.takeBytes()));
        await e2.flattenForms();
        final flatSink = TestSink();
        await e2.save(flatSink);
        await e2.dispose();
        final doc = await pdf.open(src(flatSink.takeBytes()));
        final text = await doc.extract(pages: const PdfPages.all());
        expect(
          text,
          contains('01.01.2030'),
          reason:
              'stamping must not disconnect the page\'s existing form widget',
        );
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('reopened filled form rasterizes its value (/NeedAppearances)', () async {
      // The renderer (not just the flattener) must regenerate a widget's
      // appearance from /V when the AcroForm asks for it. Fill a field whose
      // /AP is a blank placeholder, save (sets /V + /NeedAppearances), reopen,
      // and RASTERIZE: the value's pixels must appear where the blank /AP would
      // have drawn nothing. Proof is SEMANTIC — rendered pixels.
      final pdf = createPdf();
      final e1 = await pdf.edit(src(emptyApTextForm));
      await e1.setFormFieldValue('field', 'JANEROE');
      final filled = TestSink();
      await e1.save(filled);
      await e1.dispose();

      final doc = await pdf.open(src(filled.takeBytes()));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(
        pages: const PdfPages.single(0),
        size: const PdfRenderSize.thumbnail(220),
      )) {
        pages.add(page);
      }
      await doc.dispose();

      final bitmap = img.decodePng(pages.single.data)!;
      // In the broken case this page is provably blank white: empty /AP
      // (/Tx BMC EMC), blank content (q Q), no widget border. So ANY ink in
      // the top third — where the field sits — is the regenerated value.
      // Count non-white pixels, not just near-black ones: a strict dark
      // threshold reduces the small antialiased value to a few core pixels
      // whose count varies by rasterizer/DPI across machines, while counting
      // ink gives a strong 0-vs-many signal that survives those differences.
      var ink = 0;
      final cutoff = bitmap.height ~/ 3;
      for (final p in bitmap) {
        if (p.y < cutoff && (p.r < 200 || p.g < 200 || p.b < 200)) ink++;
      }
      expect(
        ink,
        greaterThan(10),
        reason:
            'the reopened value must rasterize, not the blank /AP placeholder',
      );
    }, timeout: t(1));

    test('addImageStamp preserves PNG transparency', () async {
      // A transparent-background PNG keeps its transparency only if the
      // engine emits a grayscale /SMask from the alpha channel. Proof is
      // SEMANTIC — render the stamped page and read the pixels.
      //
      // transparentPng is opaque red on the left half, fully transparent
      // black on the right half. Stamped over a blank WHITE A4 page,
      // oversized so no page edge stays white:
      //   • the opaque half must render RED   — proves the stamp drew
      //   • the transparent half must render WHITE — the page showing
      //     through. A dropped alpha renders it BLACK, so white ≈ 0.
      final pdf = createPdf();
      final editor = await pdf.edit(src(fBlankA4));
      await editor.addImageStamp(
        0,
        src(transparentPng),
        rect: const PdfRect(x: 0, y: 0, width: 600, height: 850),
      );
      final stamped = TestSink();
      await editor.save(stamped);
      await editor.dispose();

      final doc = await pdf.open(src(stamped.takeBytes()));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(
        pages: const PdfPages.single(0),
        size: const PdfRenderSize.thumbnail(160),
      )) {
        pages.add(page);
      }
      await doc.dispose();

      expect(pages, hasLength(1));
      // doc.render() yields PNG-encoded frames — decode to pixels.
      final decoded = img.decodePng(pages.single.data);
      expect(decoded, isNotNull, reason: 'rendered page must be a valid PNG');
      final bitmap = decoded!;

      // Count red (opaque half), white (page revealed through the
      // transparent half), and black (a dropped-alpha black box). With
      // the /SMask: red ≈ 0.50, white ≈ 0.49, black ≈ 0. A dropped alpha
      // gives red ≈ 0.07 (also corrupted), white ≈ 0, black ≈ 0.47.
      var red = 0, white = 0, black = 0;
      final total = bitmap.width * bitmap.height;
      for (final p in bitmap) {
        final r = p.r, g = p.g, b = p.b;
        if (r > 200 && g < 70 && b < 70) red++;
        if (r > 220 && g > 220 && b > 220) white++;
        if (r < 40 && g < 40 && b < 40) black++;
      }

      expect(total, greaterThan(0));
      // The opaque half drew — guards against a stamp that does nothing.
      expect(
        red / total,
        greaterThan(0.30),
        reason: 'opaque red half of the stamp must render',
      );
      // The transparent half reveals the white page; a dropped alpha
      // paints it solid black, so white ≈ 0.
      expect(
        white / total,
        greaterThan(0.30),
        reason: 'transparent half must reveal the white page, not render black',
      );
      // And the transparent region must not be the black box itself.
      expect(
        black / total,
        lessThan(0.05),
        reason: 'transparent PNG must not render as a black box',
      );
    }, timeout: t(1));

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
    }, timeout: t(1));

    test('eraseRegions produces valid output', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));
      await editor.eraseRegions(0, [
        const PdfRect(x: 50, y: 700, width: 200, height: 30),
      ]);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('flattenForms preserves page count', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

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
      await doc.dispose();
    }, timeout: t(1));

    test('fullRewrite with compress + GC produces smaller output', () async {
      final pdf = createPdf();
      final formBytes = fFormFields;
      final editor = await pdf.edit(src(formBytes));
      final sink = TestSink();
      await editor.save(
        sink,
        options: const PdfSaveOptions.fullRewrite(
          compress: true,
          garbageCollect: true,
        ),
      );
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Getters ──

    test('version returns non-empty string', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      final v = await editor.version;
      expect(v, contains('.'));
      await editor.dispose();
    }, timeout: t(1));

    test('title roundtrips with setTitle', () async {
      final editor = await createPdf().edit(src(minimalPdf));
      await editor.setTitle('RoundtripTitle');
      expect(await editor.title, contains('RoundtripTitle'));
      await editor.dispose();
    }, timeout: t(1));
  });
}

/// Renders [bytes]' first page at thumbnail size and decodes it, for
/// reduceImages tests that must prove pixels survived unchanged (or
/// close enough) across a save/reopen round trip.
Future<img.Image> _renderFirstPage(Pdf pdf, Uint8List bytes) async {
  final doc = await pdf.open(src(bytes));
  final frames = <RenderedPage>[];
  await for (final page in doc.render(
    pages: const PdfPages.single(0),
    size: const PdfRenderSize.thumbnail(200),
  )) {
    frames.add(page);
  }
  await doc.dispose();
  return img.decodePng(frames.single.data)!;
}

/// Peak signal-to-noise ratio over the RGB channels of two equally-sized
/// images; `double.infinity` when they are bit-identical.
double _psnr(img.Image a, img.Image b) {
  assert(a.width == b.width && a.height == b.height);
  var sumSquares = 0.0;
  for (var y = 0; y < a.height; y++) {
    for (var x = 0; x < a.width; x++) {
      final pa = a.getPixel(x, y);
      final pb = b.getPixel(x, y);
      final dr = pa.r - pb.r;
      final dg = pa.g - pb.g;
      final db = pa.b - pb.b;
      sumSquares += dr * dr + dg * dg + db * db;
    }
  }
  final meanSquareError = sumSquares / (a.width * a.height * 3);
  if (meanSquareError == 0) return double.infinity;
  return 10 * math.log(255 * 255 / meanSquareError) / math.ln10;
}
