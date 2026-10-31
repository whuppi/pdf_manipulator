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
import 'package:pdf_manipulator/src/types/extracted_image.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/third_party/tp_encrypted.dart';
import '../../fixtures/third_party/tp_encrypted_legacy.dart';
import '../../harness/render_pages.dart';
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

    test('permissions read back exactly as they were written', () async {
      final pdf = createPdf();
      const written = PdfPermissions(
        print: true,
        printHq: false,
        modify: false,
        copy: false,
        annotate: true,
        fillForms: true,
        accessibility: true,
        assemble: false,
      );
      final encSink = TestSink();
      await pdf.encrypt(
        src(minimalPdf),
        encSink,
        encryption: const PdfEncryptionConfig(
          ownerPassword: 'owner-pw',
          userPassword: 'user-pw',
          permissions: written,
        ),
      );
      final doc = await pdf.open(src(encSink.takeBytes()), password: 'user-pw');
      final read = doc.permissions!;
      await doc.dispose();
      expect(read.print, isTrue);
      expect(read.printHq, isFalse);
      expect(read.modify, isFalse);
      expect(read.copy, isFalse);
      expect(read.annotate, isTrue);
      expect(read.fillForms, isTrue);
      expect(read.accessibility, isTrue);
      expect(read.assemble, isFalse);
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

    // All pages arrive as one frame per page; the joined result must be
    // exactly what the per-page ops give plus the page glue.
    for (final (format, glue) in [
      (PdfExtractionFormat.text, '\n'),
      (PdfExtractionFormat.html, '\n'),
      (PdfExtractionFormat.markdown, '\n---\n\n'),
    ]) {
      test(
        'extract all ${format.name} equals the pages joined by the glue',
        () async {
          final doc = await createPdf().open(src(dashTextThreePagePdf));
          final all = await doc.extract(
            pages: const PdfPages.all(),
            format: format,
          );
          final pages = [
            for (var i = 0; i < 3; i++)
              await doc.extract(pages: PdfPages.single(i), format: format),
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
      final doc = await createPdf().open(src(dashTextThreePagePdf));
      final md = await doc.extract(
        pages: const PdfPages.all(),
        format: PdfExtractionFormat.markdown,
      );
      expect('\n---\n\n'.allMatches(md), hasLength(2));
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
      final doc = await pdf.open(src(bytes));
      final pages = await renderToMemory(doc, pages: const PdfPages.all());
      expect(pages, hasLength(2));
      await doc.dispose();
    }, timeout: t(1));

    // ── Extract images ────────────────────────────────────────────

    test('extractImages from blank PDF returns empty', () async {
      final doc = await createPdf().open(src(minimalPdf));
      final images = <ExtractedImage>[];
      await for (final img in doc.extractImages(pages: const PdfPages.all())) {
        images.add(img);
      }
      expect(images, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('extractImages finds every foreign-embedded XObject image', () async {
      final doc = await createPdf().open(src(fImages));
      final images = <ExtractedImage>[];
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

    test('validate on a damaged file never reports it compliant', () async {
      final doc = await createPdf().open(src(brokenObjectPdf));
      final a = await doc.validatePdfA();
      expect(a.compliant, isFalse);
      expect(a.errors, greaterThan(0), reason: 'a real error count, not -1');
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

    test('signatures empty for unsigned PDF', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.signatures, isEmpty);
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
          throwsA(isA<PdfWrongPassword>()),
        );
        await expectLater(
          pdf.open(src(tpEncrypted)),
          throwsA(isA<PdfPasswordRequired>()),
        );
      },
      timeout: t(1),
    );

    for (final legacy in tpEncryptedLegacy) {
      test('legacy-encrypted fixture (qpdf): ${legacy.name} opens without a '
          'password', () async {
        final doc = await createPdf().open(src(legacy.bytes));
        expect(doc.isEncrypted, isTrue);
        expect(doc.pageCount, tpEncryptedLegacyTruth.pages);
        final text = await doc.extract(pages: const PdfPages.all());
        expect(text, contains(tpEncryptedLegacyTruth.marker));
        await doc.dispose();
      }, timeout: t(1));
    }

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

    // ── XFA ──

    test('xfa reports the packet\'s field count and types', () async {
      final doc = await createPdf().open(src(xfaFormPdf));
      final xfa = await doc.xfa;
      await doc.dispose();
      expect(xfa, isNotNull);
      expect(xfa!.fieldCount, 2);
      expect(xfa.pageCount, 1);
      expect(xfa.fieldTypes, ['Checkbox', 'Text']);
    }, timeout: t(1));

    test('xfa is null on a document with no XFA', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.xfa, isNull);
      await doc.dispose();
    }, timeout: t(1));

    // ── Attachments ──

    test('attachments lists the embedded file with its metadata', () async {
      final doc = await createPdf().open(src(attachmentPdf));
      final files = await doc.attachments;
      await doc.dispose();
      expect(files, hasLength(1));
      expect(files.single.name, 'notes.txt');
      expect(files.single.description, 'Meeting notes');
      expect(files.single.mimeType, 'text/plain');
      expect(files.single.size, 11);
    }, timeout: t(1));

    test('attachments is empty on a document with no embedded files', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.attachments, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    // ── Annotations ──

    test('annotations lists nothing on a page with no /Annots', () async {
      final doc = await createPdf().open(src(minimalPdf));
      expect(await doc.annotations(0), isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('annotations past the end throws PdfPageRangeError', () async {
      final doc = await createPdf().open(src(minimalPdf));
      await expectLater(
        doc.annotations(1),
        throwsA(
          isA<PdfPageRangeError>()
              .having((e) => e.page, 'page', 1)
              .having((e) => e.pageCount, 'pageCount', 1),
        ),
      );
      await doc.dispose();
    }, timeout: t(1));

    // Page 1 of a one-page document: every PdfDoc method that takes a
    // page answers PdfPageRangeError, never the engine's own error.
    final docPageOps = <String, Future<Object?> Function(PdfDoc doc)>{
      'extract': (d) => d.extract(pages: const PdfPages.single(1)),
      'extract list': (d) => d.extract(pages: const PdfPages.list([0, 1])),
      'search': (d) => d.search(query: 'x', pages: const PdfPages.single(1)),
      'classifyPage': (d) => d.classifyPage(1),
      'annotations': (d) => d.annotations(1),
      'render': (d) => renderToMemory(d, pages: const PdfPages.single(1)),
      'render list': (d) => renderToMemory(d, pages: const PdfPages.list([1])),
      'extractImages': (d) =>
          d.extractImages(pages: const PdfPages.single(1)).toList(),
    };
    for (final entry in docPageOps.entries) {
      test('${entry.key} past the end throws PdfPageRangeError', () async {
        final doc = await createPdf().open(src(minimalPdf));
        await expectLater(
          entry.value(doc),
          throwsA(
            isA<PdfPageRangeError>()
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
      'rotatePage': (e) => e.rotatePage(1, degrees: 90),
      'pageMediaBox': (e) => e.pageMediaBox(1),
      'pageCropBox': (e) => e.pageCropBox(1),
      'setPageMediaBox': (e) => e.setPageMediaBox(1, box),
      'setPageCropBox': (e) => e.setPageCropBox(1, box),
      'setPageRotation': (e) => e.setPageRotation(1, degrees: 90),
      'deletePage': (e) => e.deletePage(1),
      'movePage from': (e) => e.movePage(from: 1, to: 0),
      'movePage to': (e) => e.movePage(from: 0, to: 1),
      'selectPages': (e) => e.selectPages([1]),
      'eraseRegions': (e) => e.eraseRegions(1, [box]),
      'clearEraseRegions': (e) => e.clearEraseRegions(1),
      'flattenForms': (e) => e.flattenForms(page: 1),
      'flattenAnnotations': (e) => e.flattenAnnotations(page: 1),
      'pageImages': (e) => e.pageImages(1),
      'resizeImage': (e) => e.resizeImage(1, 'Im0', width: 10, height: 10),
      'repositionImage': (e) => e.repositionImage(1, 'Im0', x: 0, y: 0),
      'setImageBounds': (e) => e.setImageBounds(1, 'Im0', box),
      'addRedaction': (e) => e.addRedaction(1, box),
      'redactionCount': (e) => e.redactionCount(1),
      'addWatermark': (e) => e.addWatermark(1, 'W'),
      'addStamp': (e) => e.addStamp(1, type: PdfStampType.draft, rect: box),
      'addImageStamp': (e) => e.addImageStamp(1, src(minimalPng), rect: box),
    };
    for (final entry in editorPageOps.entries) {
      test(
        'editor ${entry.key} past the end throws PdfPageRangeError',
        () async {
          final editor = await createPdf().edit(src(minimalPdf));
          await expectLater(
            entry.value(editor),
            throwsA(
              isA<PdfPageRangeError>()
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
      final doc = await createPdf().open(src(fFormFields));
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
        final doc = await createPdf().open(src(inheritedFieldAttrsForm));
        final fields = {for (final f in await doc.formFields) f.name: f};
        await doc.dispose();
        // Neither the nameless grandparent (it carries /FT) nor the two
        // widgets without /T is a field of its own.
        expect(fields.keys.toSet(), {'choice', 'group', 'group.kid'});
        final radio = fields['choice']!;
        expect(radio.type, PdfFormFieldType.radioGroup);
        expect(radio.value, isA<PdfChoiceValue>());
        expect((radio.value as PdfChoiceValue).choice, 'B');
        final kid = fields['group.kid']!;
        expect(kid.type, PdfFormFieldType.text);
        expect(kid.value, isA<PdfTextValue>());
        expect((kid.value as PdfTextValue).text, 'shared');
      },
      timeout: t(1),
    );

    test('formFields ends a /Kids cycle and lists each field once', () async {
      final doc = await createPdf().open(src(kidsCycleForm));
      final names = [for (final f in await doc.formFields) f.name];
      await doc.dispose();
      expect(names, unorderedEquals(['a', 'a.b']));
    }, timeout: t(1));

    test('extractAttachment round-trips the embedded bytes', () async {
      final doc = await createPdf().open(src(attachmentPdf));
      final sink = TestSink();
      await doc.extractAttachment('notes.txt', sink);
      await doc.dispose();
      expect(sink.takeBytes(), utf8.encode('hello notes'));
    }, timeout: t(1));

    test('extractAttachment on an unknown name errors', () async {
      final doc = await createPdf().open(src(attachmentPdf));
      await expectLater(
        doc.extractAttachment('absent.txt', TestSink()),
        throwsA(isA<PdfEngineError>()),
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

  group('verifySignatures', () {
    Future<Uint8List> signWith(
      Pdf pdf,
      PdfSigningCredentials credentials,
    ) async {
      final sink = TestSink();
      await pdf.sign(src(minimalPdf), sink, credentials: credentials);
      return sink.takeBytes();
    }

    test('signed pkcs12 verifies and covers the whole document', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(signed));
      expect(await doc.verifySignatures(), isTrue);
      final sigs = await doc.signatures;
      expect(sigs, hasLength(1));
      expect(sigs.single.isValid, isTrue);
      expect(sigs.single.coversWholeDocument, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('changing one header digit breaks the signature', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
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
      final doc = await pdf.open(src(tampered));
      expect(await doc.verifySignatures(), isFalse);
      expect((await doc.signatures).single.isValid, isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('unsigned document does not verify', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(minimalPdf));
      expect(await doc.verifySignatures(), isFalse);
      expect(await doc.signatures, isEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'incremental save keeps the signature valid over its revision',
      () async {
        final pdf = createPdf();
        final signed = await signWith(
          pdf,
          PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
        );
        final editor = await pdf.edit(src(signed));
        await editor.setTitle('x');
        final sink = TestSink();
        await editor.save(sink, options: const PdfSaveOptions.incremental());
        await editor.dispose();

        final doc = await pdf.open(src(sink.takeBytes()));
        expect(await doc.verifySignatures(), isTrue);
        expect((await doc.signatures).single.coversWholeDocument, isFalse);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('a full rewrite after signing breaks the signature', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final editor = await pdf.edit(src(signed));
      await editor.setTitle('x');
      final sink = TestSink();
      await editor.save(sink, options: const PdfSaveOptions.fullRewrite());
      await editor.dispose();

      final doc = await pdf.open(src(sink.takeBytes()));
      expect(await doc.verifySignatures(), isFalse);
      await doc.dispose();
    }, timeout: t(1));

    test('signed pem verifies', () async {
      final pdf = createPdf();
      final signed = await signWith(
        pdf,
        const PdfSigningCredentials.pem(testCertPem, testKeyPem),
      );
      final doc = await pdf.open(src(signed));
      expect(await doc.verifySignatures(), isTrue);
      await doc.dispose();
    }, timeout: t(1));
  });
}

/// Finds [needle] in [haystack] at `at`.
bool _matches(Uint8List haystack, int at, Uint8List needle) {
  for (var i = 0; i < needle.length; i++) {
    if (haystack[at + i] != needle[i]) return false;
  }
  return true;
}
