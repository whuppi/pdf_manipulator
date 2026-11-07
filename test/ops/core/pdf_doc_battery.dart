// CHARTER — this battery alone proves: every read-only query answers
// correctly on foreign-produced PDFs (open, extract, search, render,
// images, signatures, validation, classification, outline plans, and
// encrypted-document refusal). Mutations live in editor/sugar; scale
// in stress.
//
// Diet: generated foreign fixtures (dart-pdf), the handwritten micro
// fixtures (a second independent producer), and the committed
// qpdf-encrypted fixture.

import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/third_party/tp_encrypted.dart';
import '../../fixtures/third_party/tp_encrypted_legacy.dart';
import '../../harness/extract_images.dart';
import '../../harness/extract_text.dart';
import '../../harness/render_pages.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerDocTests(Pdf Function() createPdf) {
  group('doc', () {
    // ── Open + page info ──────────────────────────────────────────

    test('open returns pageCount 1 for minimal PDF', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('open returns version containing dot', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(doc.version, contains('.'));
      await doc.dispose();
    }, timeout: t(1));

    test('A4 dimensions are 595×842', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final pages = await doc.pageInfo();
      expect(pages, hasLength(1));
      expect(pages[0].width, closeTo(595, 1));
      expect(pages[0].height, closeTo(842, 1));
      await doc.dispose();
    }, timeout: t(1));

    test('Letter dimensions are 612×792', () async {
      final doc = await createPdf().open(PdfInput(src(letterPdf)));
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(612, 1));
      expect(pages[0].height, closeTo(792, 1));
      await doc.dispose();
    }, timeout: t(1));

    test('isEncrypted false for unencrypted PDF', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(doc.encryption, isNull);
      await doc.dispose();
    }, timeout: t(1));

    test('multi-page PDF has correct page count', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    // ── Open with password ────────────────────────────────────────

    test('open encrypted PDF with correct password succeeds', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        PdfInput(src(minimalPdf)),
        encSink,
        save: const PdfSave.full(
          encryption: PdfEncryption.password(ownerPassword: 'testpw'),
        ),
      );
      final encBytes = encSink.takeBytes();

      final doc = await pdf.open(PdfInput(src(encBytes), password: 'testpw'));
      expect(doc.encryption, isNotNull);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('permissions read back exactly as they were written', () async {
      final pdf = createPdf();
      const written = PdfPermissions(
        canPrint: true,
        canPrintHighQuality: false,
        canModify: false,
        canCopy: false,
        canAnnotate: true,
        canFillForms: true,
        canExtractForAccessibility: true,
        canAssemble: false,
      );
      final encSink = TestSink();
      await pdf.resave(
        PdfInput(src(minimalPdf)),
        encSink,
        save: const PdfSave.full(
          encryption: PdfEncryption.password(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
            permissions: written,
          ),
        ),
      );
      final doc = await pdf.open(
        PdfInput(src(encSink.takeBytes()), password: 'user-pw'),
      );
      final read = doc.encryption!.permissions;
      await doc.dispose();
      expect(read.canPrint, isTrue);
      expect(read.canPrintHighQuality, isFalse);
      expect(read.canModify, isFalse);
      expect(read.canCopy, isFalse);
      expect(read.canAnnotate, isTrue);
      expect(read.canFillForms, isTrue);
      expect(read.canExtractForAccessibility, isTrue);
      expect(read.canAssemble, isFalse);
    }, timeout: t(1));

    test(
      'user-password PDF refuses a wrong password with typed error',
      () async {
        final pdf = createPdf();
        final encSink = TestSink();
        await pdf.resave(
          PdfInput(src(minimalPdf)),
          encSink,
          save: const PdfSave.full(
            encryption: PdfEncryption.password(
              ownerPassword: 'owner-pw',
              userPassword: 'user-pw',
            ),
          ),
        );
        final encBytes = encSink.takeBytes();

        await expectLater(
          pdf.open(PdfInput(src(encBytes), password: 'not-the-password')),
          throwsA(isA<PdfWrongPassword>()),
          reason:
              'a wrong password must refuse loudly — silently '
              'opening would defeat the encryption',
        );
      },
      timeout: t(1),
    );

    test('user-password PDF refuses opening with no password', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        PdfInput(src(minimalPdf)),
        encSink,
        save: const PdfSave.full(
          encryption: PdfEncryption.password(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
          ),
        ),
      );
      final encBytes = encSink.takeBytes();

      await expectLater(
        pdf.open(PdfInput(src(encBytes))),
        throwsA(isA<PdfPasswordRequired>()),
      );
    }, timeout: t(1));

    test('owner-only encryption opens without a password (PDF spec)', () async {
      // PDF semantics: the USER password gates opening; the OWNER
      // password gates permissions. Owner-only encryption leaves the
      // user password empty, so any reader may open the file. This
      // pin exists so nobody "fixes" that into a lockout.
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.resave(
        PdfInput(src(minimalPdf)),
        encSink,
        save: const PdfSave.full(
          encryption: PdfEncryption.password(ownerPassword: 'testpw'),
        ),
      );
      final doc = await pdf.open(PdfInput(src(encSink.takeBytes())));
      expect(doc.encryption, isNotNull);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Extract text ──────────────────────────────────────────────

    test(
      'extractText writes to the caller\'s sink and never closes it',
      () async {
        // The sink is the caller's: the library writes to it and leaves it
        // open, so the caller can write more or close it itself.
        final doc = await createPdf().open(PdfInput(src(fTwoPageMarkers)));
        final sink = _KeepsBytes();
        await doc.extractText(sink);
        expect(sink.closes, 0, reason: 'the library never closes the sink');
        expect(sink.bytes, greaterThan(0));
        expect(
          utf8.decode(sink.written),
          contains(fTwoPageMarkersTruth.page0Marker),
          reason: 'the text reaches the sink as UTF-8',
        );
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('extract from blank page returns empty', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final text = await extractTextToString(
        doc,
        pages: const PdfPages.single(0),
      );
      expect(text.trim(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('foreign blank page: empty extraction, declared dimensions', () async {
      // The handwritten blank above is OUR minimal skeleton; this one
      // is dart-pdf's idea of a blank page (it still carries fonts and
      // resources). Emptiness must hold for both producers.
      final doc = await createPdf().open(PdfInput(src(fBlankA4)));
      expect(doc.pageCount, fBlankA4Truth.pages);
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(fBlankA4Truth.width, 1));
      expect(pages[0].height, closeTo(fBlankA4Truth.height, 1));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(
        text.trim(),
        isEmpty,
        reason:
            'a producer-styled blank must still extract as '
            'emptiness, not as resource noise',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('extract all pages returns text from both pages', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains(fTwoPageMarkersTruth.page0Marker));
      expect(text, contains(fTwoPageMarkersTruth.page1Marker));
      await doc.dispose();
    }, timeout: t(1));

    test('text returns the same string extractText writes to a sink', () async {
      final doc = await createPdf().open(PdfInput(src(fTwoPageMarkers)));
      final viaSink = await extractTextToString(doc);
      final text = await doc.text();
      expect(text, viaSink);
      expect(text, contains(fTwoPageMarkersTruth.page0Marker));
      expect(text, contains(fTwoPageMarkersTruth.page1Marker));
      final first = await doc.text(pages: const PdfPages.single(0));
      expect(first, contains(fTwoPageMarkersTruth.page0Marker));
      expect(first, isNot(contains(fTwoPageMarkersTruth.page1Marker)));
      await doc.dispose();
    }, timeout: t(1));

    // All pages arrive as one frame per page; the joined result must be
    // exactly what the per-page ops give plus the page glue.
    for (final (format, glue) in [
      (PdfExtractionFormat.layout, '\n'),
      (PdfExtractionFormat.html, '\n'),
      (PdfExtractionFormat.markdown, '\n---\n\n'),
    ]) {
      test(
        'extract all ${format.name} equals the pages joined by the glue',
        () async {
          final doc = await createPdf().open(
            PdfInput(src(dashTextThreePagePdf)),
          );
          final all = await extractTextToString(
            doc,
            pages: const PdfPages.all(),
            format: format,
          );
          final pages = [
            for (var i = 0; i < 3; i++)
              await extractTextToString(
                doc,
                pages: PdfPages.single(i),
                format: format,
              ),
          ];
          expect(all, pages.join(glue));
          expect(pages[0], contains('FIRSTPAGE'));
          expect(pages[1], contains('SECONDPAGE'));
          expect(pages[2], contains('THIRDPAGE'));
          await doc.dispose();
        },
        timeout: t(1),
      );
    }

    test('extract all markdown holds exactly two page separators', () async {
      final doc = await createPdf().open(PdfInput(src(dashTextThreePagePdf)));
      final md = await extractTextToString(
        doc,
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect('\n---\n\n'.allMatches(md), hasLength(2));
      await doc.dispose();
    }, timeout: t(1));

    test('extract single page returns only that page text', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final page0 = await extractTextToString(
        doc,
        pages: const PdfPages.single(0),
      );
      final page1 = await extractTextToString(
        doc,
        pages: const PdfPages.single(1),
      );
      expect(page0, contains(fTwoPageMarkersTruth.page0Marker));
      expect(page0, isNot(contains(fTwoPageMarkersTruth.page1Marker)));
      expect(page1, contains(fTwoPageMarkersTruth.page1Marker));
      expect(page1, isNot(contains(fTwoPageMarkersTruth.page0Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('extract range returns subset', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final range = await extractTextToString(
        doc,
        pages: const PdfPages.range(0, 1),
      );
      expect(range, contains(fTwoPageMarkersTruth.page0Marker));
      expect(range, isNot(contains(fTwoPageMarkersTruth.page1Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('extract markdown format returns content', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final md = await extractTextToString(
        doc,
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(md, contains('Application'));
      await doc.dispose();
    }, timeout: t(1));

    test('extract HTML format returns content', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final html = await extractTextToString(
        doc,
        pages: const PdfPages.single(0),
        format: PdfExtractionFormat.html,
      );
      expect(html, isNotEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // ── Search ────────────────────────────────────────────────────

    test('search finds text with page and bounds', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final results = await doc.search(
        'Application',
        pages: const PdfPages.all(),
      );
      expect(results, isNotEmpty);
      for (final r in results) {
        expect(r.text, contains('Application'));
        expect(r.page, greaterThanOrEqualTo(0));
        expect(r.bounds.width, greaterThan(0));
      }
      await doc.dispose();
    }, timeout: t(1));

    test('search for nonexistent term returns empty', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final results = await doc.search(
        'xyznonexistent',
        pages: const PdfPages.all(),
      );
      expect(results, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // This test catches the search page-filtering bug:
    // if pages param is ignored, both searches return the same count.
    test('search respects page scope', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(PdfInput(src(bytes)));
      final allHits = await doc.search('ALPHA', pages: const PdfPages.all());
      final page0Hits = await doc.search(
        'ALPHA',
        pages: const PdfPages.single(0),
      );
      final page1Hits = await doc.search(
        'ALPHA',
        pages: const PdfPages.single(1),
      );

      expect(
        allHits,
        isNotEmpty,
        reason: 'ALPHA should be found in all-page search',
      );
      expect(page0Hits, isNotEmpty, reason: 'ALPHA should be on page 0');
      expect(page1Hits, isEmpty, reason: 'ALPHA should NOT be on page 1');
      await doc.dispose();
    }, timeout: t(1));

    // ── Render ────────────────────────────────────────────────────

    test('render single page yields one image with dimensions', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final pages = await renderToMemory(doc, pages: const PdfPages.single(0));
      expect(pages, hasLength(1));
      expect(pages[0].width, greaterThan(0));
      expect(pages[0].height, greaterThan(0));
      expect(pages[0].data.length, greaterThan(0));
      await doc.dispose();
    }, timeout: t(1));

    test('render all pages of 2-page PDF yields 2 results', () async {
      final pdf = createPdf();
      final bytes = fTwoPageMarkers;
      final doc = await pdf.open(PdfInput(src(bytes)));
      final pages = await renderToMemory(doc, pages: const PdfPages.all());
      expect(pages, hasLength(2));
      await doc.dispose();
    }, timeout: t(1));

    test('a ToUnicode CMap is read whatever its line breaks and ranges per '
        'line', () async {
      final doc = await createPdf().open(PdfInput(src(splitRangeCmapsPdf)));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      final lines = text
          .split('\n')
          .map((line) => line.trim())
          .where((line) => line.isNotEmpty)
          .toList();
      expect(lines, ['HI', 'HIJ', 'AB', 'HIJ']);
      await doc.dispose();
    }, timeout: t(1));

    // ── Extract images ────────────────────────────────────────────

    test('extractImages from blank PDF returns empty', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await extractToMemory(doc, pages: const PdfPages.all()), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('extractImages finds every foreign-embedded XObject image', () async {
      final doc = await createPdf().open(PdfInput(src(fImages)));
      final images = await extractToMemory(doc, pages: const PdfPages.all());
      expect(
        images,
        hasLength(fImagesTruth.imageCount),
        reason: 'one embedded image per page — declared truth',
      );
      for (final (:image, :data) in images) {
        expect(image.width, fImagesTruth.imageWidth);
        expect(image.height, fImagesTruth.imageHeight);
        expect(image.encoding, PdfImageEncoding.raw);
        // A stored-losslessly image arrives as a PNG file of its own size.
        expect(data.sublist(0, 8), _pngSignature);
        final header = ByteData.sublistView(data, 16, 24);
        expect(header.getUint32(0), fImagesTruth.imageWidth);
        expect(header.getUint32(4), fImagesTruth.imageHeight);
      }
      expect(
        [for (final i in images) (i.image.page, i.image.index)],
        [(0, 0), (1, 0), (2, 0)],
        reason: 'page by page, in the order found',
      );
      await doc.dispose();
    }, timeout: t(1));

    test(
      'extractImages passes a JPEG through byte for byte, never decoded',
      () async {
        final doc = await createPdf().open(PdfInput(src(fImagesJpeg)));
        final images = await extractToMemory(doc, pages: const PdfPages.all());
        expect(images, isNotEmpty);
        for (final (:image, :data) in images) {
          expect(image.encoding, PdfImageEncoding.jpeg);
          expect(data.sublist(0, 2), [0xFF, 0xD8]);
          expect(
            _indexOfBytes(fImagesJpeg, data),
            isNonNegative,
            reason:
                'the extracted bytes are a run of the source file: a '
                'decode and re-encode would not be',
          );
        }
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('extractImages closes every sink it opened, on every path', () async {
      final doc = await createPdf().open(PdfInput(src(fImages)));
      final sinks = <_ClosesCounted>[];
      _ClosesCounted open(PdfImageInfo image) {
        final sink = _ClosesCounted();
        sinks.add(sink);
        return sink;
      }

      await doc.extractImages(open, pages: const PdfPages.all());
      expect(sinks, hasLength(3));
      expect(sinks.map((s) => s.closes), everyElement(1));
      expect(sinks.map((s) => s.bytes), everyElement(greaterThan(0)));

      // A factory that throws on the second image: the first image is
      // delivered and closed, and the error is the caller's own.
      sinks.clear();
      await expectLater(
        doc.extractImages((image) {
          if (image.page == 1) throw StateError('no sink for this image');
          return open(image);
        }, pages: const PdfPages.all()),
        throwsA(isA<StateError>()),
      );
      expect(sinks, hasLength(1));
      expect(sinks.single.closes, 1);

      // A sink that throws on its first write closes too.
      final failing = _FailingOnWrite();
      await expectLater(
        doc.extractImages((image) => failing, pages: const PdfPages.all()),
        throwsA(isA<StateError>()),
      );
      expect(failing.closes, greaterThanOrEqualTo(1));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'extractImages cancelled keeps the images already delivered',
      () async {
        final doc = await createPdf().open(PdfInput(src(fImages)));
        final sinks = <_ClosesCounted>[];
        late final PdfTask<List<PdfImageInfo>> task;
        task = doc.extractImages((image) {
          final sink = _ClosesCounted();
          sinks.add(sink);
          if (sinks.length == 2) task.cancel();
          return sink;
        }, pages: const PdfPages.all());
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        expect(sinks, hasLength(2), reason: 'the third image is never opened');
        expect(sinks.first.closes, 1);
        expect(sinks.first.bytes, greaterThan(0));
        expect(sinks.last.closes, 1, reason: 'the image in flight is closed');
        await doc.dispose();
      },
      timeout: t(1),
    );

    test(
      'pdf.readOnce(source).extractImages works once and only once',
      () async {
        final pdf = createPdf();
        final reads = pdf.readOnce(PdfInput(src(fImages)));
        final images = await reads.extractImages(
          (image) => MemorySink(),
          pages: const PdfPages.all(),
        );
        expect(images, hasLength(fImagesTruth.imageCount));
        expect(
          () => reads.extractImages(
            (image) => MemorySink(),
            pages: const PdfPages.all(),
          ),
          throwsA(isA<StateError>()),
        );
      },
      timeout: t(1),
    );

    // ── Validate ──────────────────────────────────────────────────

    test('validatePdfA on minimal returns non-compliant with errors', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final result = await doc.validatePdfA();
      expect(result.isCompliant, isFalse);
      expect(result.errorCount, greaterThan(0));
      await doc.dispose();
    }, timeout: t(1));

    test('validatePdfUa on minimal returns non-compliant', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final result = await doc.validatePdfUa();
      expect(result.isCompliant, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('validate on a damaged file never reports it compliant', () async {
      final doc = await createPdf().open(PdfInput(src(brokenObjectPdf)));
      final a = await doc.validatePdfA();
      expect(a.isCompliant, isFalse);
      expect(
        a.errorCount,
        greaterThan(0),
        reason: 'a real error count, not -1',
      );
      expect((await doc.validatePdfUa()).isCompliant, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    // ── Classify ──────────────────────────────────────────────────

    test('classifyPages names what every page holds', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final result = await doc.classifyPages();
      expect(result, hasLength(1));
      expect(result.first.page, 0);
      expect(result.first.kind, PdfPageKind.empty);
      await doc.dispose();
    }, timeout: t(1));

    test('classifyDocument names what the document holds', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      final result = await doc.classifyDocument();
      expect(result, isA<PdfDocumentKind>());
      await doc.dispose();
    }, timeout: t(1));

    // ── Signatures ────────────────────────────────────────────────

    test('signatures empty for unsigned PDF', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await doc.signatures(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('verifySignatures false for unsigned PDF', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await doc.verifySignatures(), isFalse);
      await doc.dispose();
    }, timeout: t(1));

    // ── Bookmarks ─────────────────────────────────────────────────

    test('bookmarkRanges reads a handwritten outline', () async {
      final doc = await createPdf().open(PdfInput(src(bookmarkedPdf)));
      final splits = await doc.bookmarkRanges();
      expect(splits.length, 2);
      expect(splits[0].title, contains('Chapter 1'));
      expect(splits[1].title, contains('Chapter 2'));
      expect(splits[0].start, greaterThanOrEqualTo(0));
      await doc.dispose();
    }, timeout: t(1));

    test('bookmarkRanges reads a foreign outline tree', () async {
      // Same op, different producer — dart-pdf builds its outline
      // through anchors + a real /Outlines tree.
      final doc = await createPdf().open(PdfInput(src(fOutlineChapters)));
      final splits = await doc.bookmarkRanges();
      expect(splits, hasLength(fOutlineChaptersTruth.chapters.length));
      for (var i = 0; i < splits.length; i++) {
        expect(splits[i].title, fOutlineChaptersTruth.chapters[i]);
        expect(splits[i].start, fOutlineChaptersTruth.startPages[i]);
      }
      await doc.dispose();
    }, timeout: t(1));

    test(
      'encrypted fixture (qpdf): right password opens, wrong refuses',
      () async {
        // Third-party AES-256 encryption — neither producer in the
        // generated catalog can make this; see the fixture's provenance.
        final pdf = createPdf();
        final doc = await pdf.open(
          PdfInput(src(tpEncrypted), password: tpEncryptedTruth.userPassword),
        );
        expect(doc.encryption, isNotNull);
        expect(doc.pageCount, tpEncryptedTruth.pages);
        final text = await extractTextToString(
          doc,
          pages: const PdfPages.all(),
        );
        expect(text, contains(tpEncryptedTruth.marker));
        await doc.dispose();

        await expectLater(
          pdf.open(PdfInput(src(tpEncrypted), password: 'wrong')),
          throwsA(isA<PdfWrongPassword>()),
        );
        await expectLater(
          pdf.open(PdfInput(src(tpEncrypted))),
          throwsA(isA<PdfPasswordRequired>()),
        );
      },
      timeout: t(1),
    );

    for (final legacy in tpEncryptedLegacy) {
      test('legacy-encrypted fixture (qpdf): ${legacy.name} opens without a '
          'password', () async {
        final doc = await createPdf().open(PdfInput(src(legacy.bytes)));
        expect(doc.encryption, isNotNull);
        expect(doc.pageCount, tpEncryptedLegacyTruth.pages);
        final text = await extractTextToString(
          doc,
          pages: const PdfPages.all(),
        );
        expect(text, contains(tpEncryptedLegacyTruth.marker));
        await doc.dispose();
      }, timeout: t(1));
    }

    test('unicode text survives extraction intact', () async {
      final doc = await createPdf().open(PdfInput(src(fUnicode)));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(
        text,
        contains(fUnicodeTruth.marker),
        reason:
            'diacritics and symbols must round-trip — a reader '
            'that mangles encoding passes every ASCII test',
      );
      await doc.dispose();
    }, timeout: t(1));

    // ── XFA ──

    test('xfa reports the packet\'s field count and types', () async {
      final doc = await createPdf().open(PdfInput(src(xfaFormPdf)));
      final xfa = await doc.xfa();
      await doc.dispose();
      expect(xfa, isNotNull);
      expect(xfa!.fieldCount, 2);
      expect(xfa.pageCount, 1);
      expect(xfa.fieldTypes, ['Checkbox', 'Text']);
    }, timeout: t(1));

    test('xfa is null on a document with no XFA', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await doc.xfa(), isNull);
      await doc.dispose();
    }, timeout: t(1));

    // ── Attachments ──

    test('attachments lists the embedded file with its metadata', () async {
      final doc = await createPdf().open(PdfInput(src(attachmentPdf)));
      final files = await doc.attachments();
      await doc.dispose();
      expect(files, hasLength(1));
      expect(files.single.name, 'notes.txt');
      expect(files.single.description, 'Meeting notes');
      expect(files.single.mimeType, 'text/plain');
      expect(files.single.sizeBytes, 11);
    }, timeout: t(1));

    test('attachments is empty on a document with no embedded files', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await doc.attachments(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // ── Annotations ──

    test('annotations lists nothing on a page with no /Annots', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      expect(await doc.annotations(0), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('annotations past the end throws PdfPageOutOfRange', () async {
      final doc = await createPdf().open(PdfInput(src(minimalPdf)));
      await expectLater(
        doc.annotations(1),
        throwsA(
          isA<PdfPageOutOfRange>()
              .having((e) => e.page, 'page', 1)
              .having((e) => e.pageCount, 'pageCount', 1),
        ),
      );
      await doc.dispose();
    }, timeout: t(1));

    // Page 1 of a one-page document: every PdfDocument method that takes a
    // page answers PdfPageOutOfRange, never the engine's own error.
    final docPageOps = <String, Future<Object?> Function(PdfDocument doc)>{
      'extractText': (d) =>
          extractTextToString(d, pages: const PdfPages.single(1)),
      'extractText list': (d) =>
          extractTextToString(d, pages: PdfPages.list([0, 1])),
      'search': (d) => d.search('x', pages: const PdfPages.single(1)),
      'classifyPages': (d) => d.classifyPages(pages: const PdfPages.single(1)),
      'pageInfo': (d) => d.pageInfo(pages: const PdfPages.single(1)),
      'annotations': (d) => d.annotations(1),
      'render': (d) => renderToMemory(d, pages: const PdfPages.single(1)),
      'render list': (d) => renderToMemory(d, pages: PdfPages.list([1])),
      'extractImages': (d) =>
          extractToMemory(d, pages: const PdfPages.single(1)),
    };
    for (final entry in docPageOps.entries) {
      test('${entry.key} past the end throws PdfPageOutOfRange', () async {
        final doc = await createPdf().open(PdfInput(src(minimalPdf)));
        await expectLater(
          entry.value(doc),
          throwsA(
            isA<PdfPageOutOfRange>()
                .having((e) => e.page, 'page', 1)
                .having((e) => e.pageCount, 'pageCount', 1),
          ),
          reason: entry.key,
        );
        await doc.dispose();
      }, timeout: t(1));
    }

    // Page 1 of a one-page document, for every PdfEditor method that
    // takes a page index.
    const box = PdfRect(x: 0, y: 0, width: 10, height: 10);
    final editorPageOps = <String, Future<Object?> Function(PdfEditor e)>{
      'rotatePages': (e) =>
          e.rotatePages(by: PdfRotation.r90, pages: PdfPages.single(1)),
      'pageInfo': (e) => e.pageInfo(pages: PdfPages.single(1)),
      'setMediaBox': (e) => e.setMediaBox(box, pages: PdfPages.single(1)),
      'setCropBox': (e) => e.setCropBox(box, pages: PdfPages.single(1)),
      'setRotation': (e) =>
          e.setRotation(PdfRotation.r90, pages: PdfPages.single(1)),
      'removePages': (e) => e.removePages(PdfPages.single(1)),
      'movePage from': (e) => e.movePage(from: 1, to: 0),
      'movePage to': (e) => e.movePage(from: 0, to: 1),
      'keepPages': (e) => e.keepPages(PdfPages.single(1)),
      'redact': (e) =>
          e.redact(1, [box], overlay: const PdfRedactionOverlay.none()),
      'flattenForms': (e) => e.flattenForms(pages: PdfPages.single(1)),
      'flattenAnnotations': (e) =>
          e.flattenAnnotations(pages: PdfPages.single(1)),
      'pageImages': (e) => e.pageImages(1),
      'setImageBounds': (e) => e.setImageBounds(1, 'Im0', box),
      'markRedaction': (e) => e.markRedaction(1, [box]),
      'redactionCount': (e) => e.redactionCount(1),
      'addWatermark': (e) => e.addWatermark('W', pages: PdfPages.single(1)),
      'addStamp': (e) =>
          e.addStamp(PdfStamp.draft, bounds: box, pages: PdfPages.single(1)),
      'addImageStamp': (e) => e.addImageStamp(
        src(minimalPng),
        bounds: box,
        pages: PdfPages.single(1),
      ),
    };
    for (final entry in editorPageOps.entries) {
      test(
        'editor ${entry.key} past the end throws PdfPageOutOfRange',
        () async {
          final editor = await createPdf().edit(PdfInput(src(minimalPdf)));
          await expectLater(
            entry.value(editor),
            throwsA(
              isA<PdfPageOutOfRange>()
                  .having((e) => e.page, 'page', 1)
                  .having((e) => e.pageCount, 'pageCount', 1),
            ),
            reason: entry.key,
          );
          await editor.dispose();
        },
        timeout: t(1),
      );
    }

    test('annotations lists every form widget with its field name', () async {
      final doc = await createPdf().open(PdfInput(src(fFormFields)));
      final rows = await doc.annotations(0);
      await doc.dispose();
      expect(rows, isNotEmpty);
      for (final row in rows) {
        expect(row.subtype, PdfAnnotationSubtype.widget);
        expect(
          row.author,
          isNull,
          reason: "a widget's /T is its field name, not an author",
        );
      }
      expect(
        rows.map((r) => r.fieldName).whereType<String>().toSet(),
        fFormFieldsTruth.fieldNames.toSet(),
      );
    }, timeout: t(1));

    test(
      'formFields resolve /FT, /Ff and /V inherited from ancestors',
      () async {
        final doc = await createPdf().open(
          PdfInput(src(inheritedFieldAttrsForm)),
        );
        final fields = {for (final f in await doc.formFields()) f.name: f};
        await doc.dispose();
        // Neither the nameless grandparent (it carries /FT) nor the two
        // widgets without /T is a field of its own.
        expect(fields.keys.toSet(), {'choice', 'group', 'group.kid'});
        final radio = fields['choice']!;
        expect(radio.kind, PdfFormFieldKind.radioGroup);
        expect(radio.value, isA<PdfChoiceValue>());
        expect((radio.value as PdfChoiceValue).choice, 'B');
        final kid = fields['group.kid']!;
        expect(kid.kind, PdfFormFieldKind.textField);
        expect(kid.value, isA<PdfTextValue>());
        expect((kid.value as PdfTextValue).text, 'shared');
      },
      timeout: t(1),
    );

    test('formFields ends a /Kids cycle and lists each field once', () async {
      final doc = await createPdf().open(PdfInput(src(kidsCycleForm)));
      final names = [for (final f in await doc.formFields()) f.name];
      await doc.dispose();
      expect(names, unorderedEquals(['a', 'a.b']));
    }, timeout: t(1));

    test('extractAttachment round-trips the embedded bytes', () async {
      final doc = await createPdf().open(PdfInput(src(attachmentPdf)));
      final sink = TestSink();
      await doc.extractAttachment('notes.txt', sink);
      await doc.dispose();
      expect(sink.takeBytes(), utf8.encode('hello notes'));
    }, timeout: t(1));

    test('extractAttachment on an unknown name errors', () async {
      final doc = await createPdf().open(PdfInput(src(attachmentPdf)));
      await expectLater(
        doc.extractAttachment('absent.txt', TestSink()),
        throwsA(
          isA<PdfNotFound>()
              .having((e) => e.kind, 'kind', 'attachment')
              .having((e) => e.name, 'name', 'absent.txt'),
        ),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('pre-rotated pages report their foreign-set rotation', () async {
      final doc = await createPdf().open(PdfInput(src(fRotated)));
      expect(doc.pageCount, fRotatedTruth.pages);
      final pages = await doc.pageInfo();
      for (var i = 0; i < fRotatedTruth.pages; i++) {
        expect(
          pages[i].rotation.wire,
          'r${fRotatedTruth.rotations[i]}',
          reason: 'page $i carries /Rotate from the producer',
        );
      }
      await doc.dispose();
    }, timeout: t(1));
  });

  group('verifySignatures', () {
    Future<Uint8List> signWith(
      Pdf pdf,
      PdfSigningCredentials credentials,
    ) async {
      final sink = TestSink();
      await pdf.sign(PdfInput(src(minimalPdf)), sink, credentials: credentials);
      return sink.takeBytes();
    }

    test('signed pkcs12 verifies and covers the whole document', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(src(testPkcs12), 'changeit'),
      );
      final doc = await pdf.open(PdfInput(src(signed)));
      expect(await doc.verifySignatures(), isTrue);
      final sigs = await doc.signatures();
      expect(sigs, hasLength(1));
      expect(sigs.single.isValid, isTrue);
      expect(sigs.single.coversWholeDocument, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('changing one header digit breaks the signature', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(src(testPkcs12), 'changeit'),
      );
      // The header literal comes from the fixture itself: its version
      // has changed across producers and only the tamper target matters.
      final header = minimalPdf.sublist(0, 8);
      var at = -1;
      for (var i = 0; i + header.length <= signed.length; i++) {
        if (signed[i] == header[0] && _matches(signed, i, header)) {
          at = i;
          break;
        }
      }
      expect(at, 0, reason: 'the header sits at byte 0');
      final tampered = Uint8List.fromList(signed);
      tampered[at + 7] = 0x35; // '4' -> '5'
      final doc = await pdf.open(PdfInput(src(tampered)));
      expect(await doc.verifySignatures(), isFalse);
      expect((await doc.signatures()).single.isValid, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('unsigned document does not verify', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(minimalPdf)));
      expect(await doc.verifySignatures(), isFalse);
      expect(await doc.signatures(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'incremental save keeps the signature valid over its revision',
      () async {
        final pdf = createPdf();
        final signed = await signWith(
          pdf,
          PdfSigningCredentials.pkcs12(src(testPkcs12), 'changeit'),
        );
        final editor = await pdf.edit(PdfInput(src(signed)));
        await editor.setMetadata(
          (await editor.metadata()).copyWith(title: 'x'),
        );
        final sink = TestSink();
        await editor.save(sink, save: const PdfSave.incremental());
        await editor.dispose();

        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        expect(await doc.verifySignatures(), isTrue);
        expect((await doc.signatures()).single.coversWholeDocument, isFalse);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('a full rewrite after signing breaks the signature', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(src(testPkcs12), 'changeit'),
      );
      final editor = await pdf.edit(PdfInput(src(signed)));
      await editor.setMetadata((await editor.metadata()).copyWith(title: 'x'));
      final sink = TestSink();
      await editor.save(sink, save: const PdfSave.full());
      await editor.dispose();

      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(await doc.verifySignatures(), isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('signed pem verifies', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        const PdfSigningCredentials.pem(testCertPem, testKeyPem),
      );
      final doc = await pdf.open(PdfInput(src(signed)));
      expect(await doc.verifySignatures(), isTrue);
      await doc.dispose();
    }, timeout: t(1));
  });
}

/// The eight bytes every PNG file starts with.
const _pngSignature = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// The first place [needle] occurs in [haystack], or -1.
int _indexOfBytes(Uint8List haystack, Uint8List needle) {
  for (var at = 0; at + needle.length <= haystack.length; at++) {
    if (_matches(haystack, at, needle)) return at;
  }
  return -1;
}

/// A sink that counts what it was given and how often it was closed.
class _ClosesCounted implements DataSink {
  int closes = 0;
  int bytes = 0;

  @override
  void abort(Object reason) => bytes = 0;

  @override
  void close() => closes++;

  @override
  Future<void> write(Uint8List chunk) async {
    bytes += chunk.length;
  }
}

/// A sink that keeps what it was given and counts its closes.
class _KeepsBytes extends _ClosesCounted {
  final _written = BytesBuilder(copy: false);

  @override
  Future<void> write(Uint8List chunk) async {
    await super.write(chunk);
    _written.add(chunk);
  }

  @override
  void abort(Object reason) {
    super.abort(reason);
    _written.clear();
  }

  /// The bytes written so far.
  Uint8List get written => _written.toBytes();
}

/// A sink whose first write fails.
class _FailingOnWrite extends _ClosesCounted {
  @override
  Future<void> write(Uint8List chunk) async =>
      throw StateError('the sink refused the bytes');
}

/// Finds [needle] in [haystack] at `at`.
bool _matches(Uint8List haystack, int at, Uint8List needle) {
  for (var i = 0; i < needle.length; i++) {
    if (haystack[at + i] != needle[i]) return false;
  }
  return true;
}
