// CHARTER — this battery alone proves: every read-only query answers
// correctly on foreign-produced PDFs (open, extract, search, render,
// images, signatures, validation, classification, outline plans, and
// encrypted-document refusal). Mutations live in editor/sugar; scale
// in stress.
//
// Diet: generated foreign fixtures (dart-pdf), the handwritten micro
// fixtures (a second independent producer), and the committed
// qpdf-encrypted fixture.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/third_party/tp_encrypted.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerDocTests(Pdf Function() createPdf) {
  group('doc', () {
    // ── Open + page info ──────────────────────────────────────────

    test('open returns pageCount 1 for minimal PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('open returns version containing dot', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.version, contains('.'));
      await doc.dispose();
    }, timeout: t(1));

    test('A4 dimensions are 595×842', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, closeTo(595, 1));
      expect(doc.pages[0].height, closeTo(842, 1));
      await doc.dispose();
    }, timeout: t(1));

    test('Letter dimensions are 612×792', () async {
      final doc = await createPdf().open(src(letterPdf));
      expect(doc.pages[0].width, closeTo(612, 1));
      expect(doc.pages[0].height, closeTo(792, 1));
      await doc.dispose();
    }, timeout: t(1));

    test('isEncrypted false for unencrypted PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(doc.isEncrypted, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('multi-page PDF has correct page count', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(src(bytes));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    // ── Open with password ────────────────────────────────────────

    test('open encrypted PDF with correct password succeeds', () async {
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.encrypt(
        src(minimalPdf),
        encSink,
        encryption: const PdfEncryptionConfig(ownerPassword: 'testpw'),
      );
      final encBytes = encSink.takeBytes();

      final doc = await pdf.open(src(encBytes), password: 'testpw');
      expect(doc.isEncrypted, isTrue);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'user-password PDF refuses a wrong password with typed error',
      () async {
        final pdf = createPdf();
        final encSink = TestSink();
        await pdf.encrypt(
          src(minimalPdf),
          encSink,
          encryption: const PdfEncryptionConfig(
            ownerPassword: 'owner-pw',
            userPassword: 'user-pw',
          ),
        );
        final encBytes = encSink.takeBytes();

        await expectLater(
          pdf.open(src(encBytes), password: 'not-the-password'),
          throwsA(isA<PdfEngineError>()),
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
      await pdf.encrypt(
        src(minimalPdf),
        encSink,
        encryption: const PdfEncryptionConfig(
          ownerPassword: 'owner-pw',
          userPassword: 'user-pw',
        ),
      );
      final encBytes = encSink.takeBytes();

      await expectLater(
        pdf.open(src(encBytes)),
        throwsA(isA<PdfEngineError>()),
      );
    }, timeout: t(1));

    test('owner-only encryption opens without a password (PDF spec)', () async {
      // PDF semantics: the USER password gates opening; the OWNER
      // password gates permissions. Owner-only encryption leaves the
      // user password empty, so any reader may open the file. This
      // pin exists so nobody "fixes" that into a lockout.
      final pdf = createPdf();
      final encSink = TestSink();
      await pdf.encrypt(
        src(minimalPdf),
        encSink,
        encryption: const PdfEncryptionConfig(ownerPassword: 'testpw'),
      );
      final doc = await pdf.open(src(encSink.takeBytes()));
      expect(doc.isEncrypted, isTrue);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    // ── Extract text ──────────────────────────────────────────────

    test('extract from blank page returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final text = await doc.extract(pages: const PdfPages.single(0));
      expect(text.trim(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('foreign blank page: empty extraction, declared dimensions', () async {
      // The handwritten blank above is OUR minimal skeleton; this one
      // is dart-pdf's idea of a blank page (it still carries fonts and
      // resources). Emptiness must hold for both producers.
      final doc = await createPdf().open(src(fBlankA4));
      expect(doc.pageCount, fBlankA4Truth.pages);
      expect(doc.pages[0].width, closeTo(fBlankA4Truth.width, 1));
      expect(doc.pages[0].height, closeTo(fBlankA4Truth.height, 1));
      final text = await doc.extract(pages: const PdfPages.all());
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
      final doc = await createPdf().open(src(bytes));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(fTwoPageMarkersTruth.page0Marker));
      expect(text, contains(fTwoPageMarkersTruth.page1Marker));
      await doc.dispose();
    }, timeout: t(1));

    test('extract single page returns only that page text', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(src(bytes));
      final page0 = await doc.extract(pages: const PdfPages.single(0));
      final page1 = await doc.extract(pages: const PdfPages.single(1));
      expect(page0, contains(fTwoPageMarkersTruth.page0Marker));
      expect(page0, isNot(contains(fTwoPageMarkersTruth.page1Marker)));
      expect(page1, contains(fTwoPageMarkersTruth.page1Marker));
      expect(page1, isNot(contains(fTwoPageMarkersTruth.page0Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('extract range returns subset', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(src(bytes));
      final range = await doc.extract(pages: const PdfPages.range(0, 1));
      expect(range, contains(fTwoPageMarkersTruth.page0Marker));
      expect(range, isNot(contains(fTwoPageMarkersTruth.page1Marker)));
      await doc.dispose();
    }, timeout: t(1));

    test('extract markdown format returns content', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(src(bytes));
      final md = await doc.extract(
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect(md, contains('Application'));
      await doc.dispose();
    }, timeout: t(1));

    test('extract HTML format returns content', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(src(bytes));
      final html = await doc.extract(
        pages: const PdfPages.single(0),
        format: PdfExtractionFormat.html,
      );
      expect(html, isNotEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // ── Search ────────────────────────────────────────────────────

    test('search finds text with page and rect', () async {
      final bytes = fFormFields;
      final doc = await createPdf().open(src(bytes));
      final results = await doc.search(
        query: 'Application',
        pages: const PdfPages.all(),
      );
      expect(results, isNotEmpty);
      for (final r in results) {
        expect(r.text, contains('Application'));
        expect(r.page, greaterThanOrEqualTo(0));
        expect(r.rect.width, greaterThan(0));
      }
      await doc.dispose();
    }, timeout: t(1));

    test('search for nonexistent term returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final results = await doc.search(
        query: 'xyznonexistent',
        pages: const PdfPages.all(),
      );
      expect(results, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // This test catches the search page-filtering bug:
    // if pages param is ignored, both searches return the same count.
    test('search respects page scope', () async {
      final bytes = fTwoPageMarkers;
      final doc = await createPdf().open(src(bytes));
      final allHits = await doc.search(
        query: 'ALPHA',
        pages: const PdfPages.all(),
      );
      final page0Hits = await doc.search(
        query: 'ALPHA',
        pages: const PdfPages.single(0),
      );
      final page1Hits = await doc.search(
        query: 'ALPHA',
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
      final doc = await createPdf().open(src(minimalPdf));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(pages: const PdfPages.single(0))) {
        pages.add(page);
      }
      expect(pages, hasLength(1));
      expect(pages[0].width, greaterThan(0));
      expect(pages[0].height, greaterThan(0));
      expect(pages[0].data.length, greaterThan(0));
      await doc.dispose();
    }, timeout: t(1));

    test('render all pages of 2-page PDF yields 2 results', () async {
      final pdf = createPdf();
      final bytes = fTwoPageMarkers;
      final doc = await pdf.open(src(bytes));
      final pages = <RenderedPage>[];
      await for (final page in doc.render(pages: const PdfPages.all())) {
        pages.add(page);
      }
      expect(pages, hasLength(2));
      await doc.dispose();
    }, timeout: t(1));

    // ── Extract images ────────────────────────────────────────────

    test('extractImages from blank PDF returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final images = <PdfImage>[];
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        images.add(img);
      }
      expect(images, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('extractImages finds every foreign-embedded XObject image', () async {
      final doc = await createPdf().open(src(fImages));
      final images = <PdfImage>[];
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        images.add(img);
      }
      expect(
        images,
        hasLength(fImagesTruth.imageCount),
        reason: 'one embedded image per page — declared truth',
      );
      for (final img in images) {
        expect(img.width, fImagesTruth.imageWidth);
        expect(img.height, fImagesTruth.imageHeight);
        expect(
          img.data.length,
          greaterThan(0),
          reason:
              'an extracted image with no pixel data is not '
              'an extraction',
        );
      }
      await doc.dispose();
    }, timeout: t(1));

    // ── Validate ──────────────────────────────────────────────────

    test('validatePdfA on minimal returns non-compliant with errors', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.validatePdfA();
      expect(result.compliant, isFalse);
      expect(result.errors, greaterThan(0));
      await doc.dispose();
    }, timeout: t(1));

    test('validatePdfUa on minimal returns false', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.validatePdfUa(), isFalse);
      await doc.dispose();
    }, timeout: t(1));

    // ── Classify ──────────────────────────────────────────────────

    test('classifyPage returns type and confidence in range', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.classifyPage(0);
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
      await doc.dispose();
    }, timeout: t(1));

    test('classifyDocument returns type and confidence in range', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final result = await doc.classifyDocument();
      expect(result.type, isNotEmpty);
      expect(result.confidence, greaterThanOrEqualTo(0.0));
      expect(result.confidence, lessThanOrEqualTo(1.0));
      await doc.dispose();
    }, timeout: t(1));

    // ── Signatures ────────────────────────────────────────────────

    test('getSignatures empty for unsigned PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.getSignatures(), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('verifySignatures false for unsigned PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.verifySignatures(), isFalse);
      await doc.dispose();
    }, timeout: t(1));

    // ── Bookmarks ─────────────────────────────────────────────────

    test('planSplitByBookmarks reads a handwritten outline', () async {
      final doc = await createPdf().open(src(bookmarkedPdf));
      final splits = await doc.planSplitByBookmarks();
      expect(splits.length, 2);
      expect(splits[0].title, contains('Chapter 1'));
      expect(splits[1].title, contains('Chapter 2'));
      expect(splits[0].startPage, greaterThanOrEqualTo(0));
      await doc.dispose();
    }, timeout: t(1));

    test('planSplitByBookmarks reads a foreign outline tree', () async {
      // Same op, different producer — dart-pdf builds its outline
      // through anchors + a real /Outlines tree.
      final doc = await createPdf().open(src(fOutlineChapters));
      final splits = await doc.planSplitByBookmarks();
      expect(splits, hasLength(fOutlineChaptersTruth.chapters.length));
      for (var i = 0; i < splits.length; i++) {
        expect(splits[i].title, fOutlineChaptersTruth.chapters[i]);
        expect(splits[i].startPage, fOutlineChaptersTruth.startPages[i]);
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
          src(tpEncrypted),
          password: tpEncryptedTruth.userPassword,
        );
        expect(doc.isEncrypted, isTrue);
        expect(doc.pageCount, tpEncryptedTruth.pages);
        final text = await doc.extract(pages: const PdfPages.all());
        expect(text, contains(tpEncryptedTruth.marker));
        await doc.dispose();

        await expectLater(
          pdf.open(src(tpEncrypted), password: 'wrong'),
          throwsA(isA<PdfEngineError>()),
        );
        await expectLater(
          pdf.open(src(tpEncrypted)),
          throwsA(isA<PdfEngineError>()),
        );
      },
      timeout: t(1),
    );

    test('unicode text survives extraction intact', () async {
      final doc = await createPdf().open(src(fUnicode));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains(fUnicodeTruth.marker),
        reason:
            'diacritics and symbols must round-trip — a reader '
            'that mangles encoding passes every ASCII test',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('pre-rotated pages report their foreign-set rotation', () async {
      final doc = await createPdf().open(src(fRotated));
      expect(doc.pageCount, fRotatedTruth.pages);
      for (var i = 0; i < fRotatedTruth.pages; i++) {
        expect(
          doc.pages[i].rotation,
          fRotatedTruth.rotations[i],
          reason: 'page $i carries /Rotate from the producer',
        );
      }
      await doc.dispose();
    }, timeout: t(1));
  });
}
