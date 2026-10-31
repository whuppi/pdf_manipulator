// CHARTER — this battery alone proves: the standalone ops (sign,
// extractPages, office conversion) transform source→sink correctly
// with no handle exposed. These ops live here and nowhere else —
// sugar composes extractPages internally but never re-proves it.

import 'dart:convert';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/handwritten_docx.dart';
import '../../fixtures/third_party/tp_page_prune_objstm.dart';
import '../../harness/page_prune_expect.dart';
import '../../harness/plain_bytes.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerStandaloneTests(Pdf Function() createPdf) {
  group('standalone', () {
    // ── XFA conversion ──

    test(
      'convertXfaToAcroForm turns the XFA packet into real fields',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.convertXfaToAcroForm(src(xfaFormPdf), sink);
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.pageCount, greaterThanOrEqualTo(1));
        final names = (await doc.formFields).map((f) => f.name).toList();
        await doc.dispose();
        expect(names, containsAll(<String>['given', 'agree']));
      },
      timeout: t(1),
    );

    test('convertXfaToAcroForm refuses a document with no XFA', () async {
      await expectLater(
        createPdf().convertXfaToAcroForm(src(minimalPdf), TestSink()),
        throwsA(isA<PdfEngineError>()),
      );
    }, timeout: t(1));

    // ── Sign ──

    test('sign with PKCS12 adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      expect(signed.length, greaterThan(minimalPdf.length));
      final sigs = await doc.signatures;
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      expect(sigs.first.signerName, isNotEmpty);
      await doc.dispose();
    }, timeout: t(1));

    test('sign with PEM adds a retrievable signature', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: const PdfSigningCredentials.pem(testCertPem, testKeyPem),
      );
      final signed = sink.takeBytes();
      final doc = await pdf.open(src(signed));
      expect(doc.pageCount, 1);
      final sigs = await doc.signatures;
      expect(sigs, isNotEmpty);
      expect(sigs.first.signerName, isNotNull);
      await doc.dispose();
    }, timeout: t(1));

    test('sign with invalid cert throws', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await expectLater(
        pdf.sign(
          src(minimalPdf),
          sink,
          credentials: PdfSigningCredentials.pkcs12(
            Uint8List.fromList([1, 2, 3, 4]),
            'wrong',
          ),
        ),
        throwsA(isA<PdfCryptoError>()),
        reason:
            'a broken certificate is an engine failure with a '
            'message — never an untyped throw',
      );
    }, timeout: t(1));

    test('sign keeps the catalog: bookmarks survive', () async {
      final pdf = createPdf();
      final unsigned = await pdf.open(src(bookmarkedPdf));
      final titles = (await unsigned.planSplitByBookmarks())
          .map((s) => s.title)
          .toList();
      await unsigned.dispose();
      final sink = TestSink();
      await pdf.sign(
        src(bookmarkedPdf),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        (await doc.planSplitByBookmarks()).map((s) => s.title),
        titles,
        reason:
            'signing must not replace the catalog — the outline tree '
            'the bookmarks read lives there',
      );
      expect(await doc.verifySignatures(), isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('sign keeps the form: fields and their values survive', () async {
      final pdf = createPdf();
      final unsigned = await pdf.open(src(fFormFields));
      final valuesBefore = {
        for (final f in await unsigned.formFields) f.name: f.value,
      };
      await unsigned.dispose();
      final sink = TestSink();
      await pdf.sign(
        src(fFormFields),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      final fields = await doc.formFields;
      expect(fields.map((f) => f.name).toSet(), {
        ...fFormFieldsTruth.fieldNames,
        'Sig1',
      });
      for (final f in fields) {
        if (f.name == 'Sig1') continue;
        final before = valuesBefore[f.name]!;
        switch (before) {
          case PdfTextValue(:final text):
            expect(
              f.value,
              isA<PdfTextValue>().having((v) => v.text, 'text', text),
              reason: 'field ${f.name}',
            );
          case PdfCheckedValue(:final checked):
            expect(
              f.value,
              isA<PdfCheckedValue>().having(
                (v) => v.checked,
                'checked',
                checked,
              ),
              reason: 'field ${f.name}',
            );
          case PdfChoiceValue(:final choice):
            expect(
              f.value,
              isA<PdfChoiceValue>().having((v) => v.choice, 'choice', choice),
              reason: 'field ${f.name}',
            );
          case PdfMultiChoiceValue(:final choices):
            expect(
              f.value,
              isA<PdfMultiChoiceValue>().having(
                (v) => v.choices,
                'choices',
                choices,
              ),
              reason: 'field ${f.name}',
            );
          default:
            expect(
              f.value.runtimeType,
              before.runtimeType,
              reason: 'field ${f.name}',
            );
        }
      }
      expect(await doc.verifySignatures(), isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('sign twice keeps both signatures on Sig1 and Sig2', () async {
      final pdf = createPdf();
      final first = TestSink();
      await pdf.sign(
        src(minimalPdf),
        first,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final second = TestSink();
      await pdf.sign(
        src(first.takeBytes()),
        second,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(second.takeBytes()));
      final sigs = await doc.signatures;
      expect(sigs, hasLength(2));
      expect((await doc.formFields).map((f) => f.name).toSet(), {
        'Sig1',
        'Sig2',
      }, reason: 'the second signature must not replace the first field');
      expect(await doc.verifySignatures(), isTrue);
      expect(sigs.first.coversWholeDocument, isFalse);
      expect(sigs.last.coversWholeDocument, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('sign an xref-stream source writes a stream-style update', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(tpPagePruneObjStm),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final signed = sink.takeBytes();
      final appended = latin1.decode(signed.sublist(tpPagePruneObjStm.length));
      expect(appended, contains('/Type /XRef'));
      expect(
        appended.contains('\nxref\n'),
        isFalse,
        reason: 'a stream-style source never gets a classic table back',
      );
      final doc = await pdf.open(src(signed));
      expect(await doc.verifySignatures(), isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test('sign an encrypted source refuses with no output', () async {
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
      final sink = TestSink();
      await expectLater(
        pdf.sign(
          src(encSink.takeBytes()),
          sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
        ),
        throwsA(isA<PdfUnsupported>()),
        reason:
            'signing an encrypted source is refused before any byte '
            'is written, never a broken file',
      );
      expect(sink.takeBytes(), isEmpty);
    }, timeout: t(1));

    test(
      'sign reads back a reason with separators and a non-ASCII location',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.sign(
          src(minimalPdf),
          sink,
          credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
          reason: 'a (b) \\ c',
          location: 'Zürich',
        );
        final doc = await pdf.open(src(sink.takeBytes()));
        final sigs = await doc.signatures;
        expect(sigs, hasLength(1));
        expect(sigs.single.reason, 'a (b) \\ c');
        expect(sigs.single.location, 'Zürich');
        expect(await doc.verifySignatures(), isTrue);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('sign adds the widget to page 0 annotations', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      final annots = await doc.annotations(0);
      expect(annots, hasLength(1));
      expect(annots.single.subtype, PdfAnnotationSubtype.widget);
      expect(annots.single.fieldName, 'Sig1');
      await doc.dispose();
    }, timeout: t(1));

    test('sign writes the signing time', () async {
      final before = DateTime.now().toUtc().subtract(
        const Duration(seconds: 2),
      );
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.sign(
        src(minimalPdf),
        sink,
        credentials: PdfSigningCredentials.pkcs12(testPkcs12, 'changeit'),
      );
      final doc = await pdf.open(src(sink.takeBytes()));
      final time = (await doc.signatures).single.signingTime;
      await doc.dispose();
      expect(time, isNotNull);
      expect(
        time!.isAfter(before),
        isTrue,
        reason: '/M is the time of signing',
      );
    }, timeout: t(1));

    // ── Extract pages ──

    test(
      'extractPages with an out-of-range index throws typed error',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await expectLater(
          pdf.extractPages(src(minimalPdf), sink, pages: const [99]),
          throwsA(
            isA<PdfPageRangeError>()
                .having((e) => e.page, 'page', 99)
                .having((e) => e.pageCount, 'pageCount', 1),
          ),
          reason:
              'page 99 of a 1-page PDF does not exist — silently '
              'producing an empty PDF would hide caller bugs',
        );
      },
      timeout: t(1),
    );

    test('extractPages keeps exactly the selected pages and content', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.extractPages(src(fThreePageMarkers), sink, pages: [0, 2]);
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(fThreePageMarkersTruth.markers[0]));
      expect(text, contains(fThreePageMarkersTruth.markers[2]));
      expect(
        text,
        isNot(contains(fThreePageMarkersTruth.markers[1])),
        reason:
            'the unselected page must not smuggle its content '
            'into the output',
      );
      await doc.dispose();
    }, timeout: t(1));

    test(
      'extractPages writes nothing only a dropped page reaches (#261)',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.extractPages(src(pagePrunePdf), sink, pages: const [0]);
        await expectOnlyPrunedPages(pdf, sink.takeBytes(), const [0]);
      },
      timeout: t(2),
    );

    test(
      'extractPages across both inner page nodes keeps the given order',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.extractPages(src(pagePrunePdf), sink, pages: const [5, 2]);
        await expectOnlyPrunedPages(pdf, sink.takeBytes(), const [5, 2]);
      },
      timeout: t(2),
    );

    test(
      'extractPages of every page flattens the tree and keeps inheritance',
      () async {
        final pdf = createPdf();
        final sink = TestSink();
        final all = [for (var i = 0; i < pagePruneTruth.pageCount; i++) i];
        await pdf.extractPages(src(pagePrunePdf), sink, pages: all);
        await expectOnlyPrunedPages(pdf, sink.takeBytes(), all);
      },
      timeout: t(2),
    );

    test('extractPages writes a reference to a dropped page as null', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.extractPages(src(structTreePdf), sink, pages: const [0]);
      final out = sink.takeBytes();
      // bytegrep-exempt: the claim is about references in the file.
      final plain = await plainBytes(pdf, out);
      final raw = String.fromCharCodes(plain); // bytegrep-exempt
      for (final id in const [4, 10]) {
        expect(
          RegExp('(?<![0-9])$id 0 R').hasMatch(raw),
          isFalse,
          reason: 'object $id is not written, so nothing may point at it',
        );
      }
      expect(raw, contains('/Pg null'));
      expect(raw, contains('/IRT null'));
      final doc = await pdf.open(src(out));
      expect(doc.pageCount, 1);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('TAGGED-0'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('extractPages refuses a page listed twice', () {
      expect(
        () => createPdf().extractPages(
          src(pagePrunePdf),
          TestSink(),
          pages: const [0, 0],
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
    }, timeout: t(1));

    // ── Convert ──

    test('convertTo DOCX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.docx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50); // PK header
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertTo PPTX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.pptx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertTo XLSX produces valid ZIP', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        sink,
        format: PdfDocumentFormat.xlsx,
      );
      final bytes = sink.takeBytes();
      expect(bytes[0], 0x50);
      expect(bytes[1], 0x4B);
    }, timeout: t(1));

    test('convertToPdf from DOCX produces valid PDF', () async {
      final pdf = createPdf();
      final docxSink = TestSink();
      await pdf.convertTo(
        src(minimalPdf),
        docxSink,
        format: PdfDocumentFormat.docx,
      );
      final pdfSink = TestSink();
      await pdf.convertToPdf(
        src(docxSink.takeBytes()),
        pdfSink,
        format: PdfDocumentFormat.docx,
      );
      final pdfBytes = pdfSink.takeBytes();
      expect(String.fromCharCodes(pdfBytes.sublist(0, 5)), startsWith('%PDF'));
    }, timeout: t(1));

    test(
      'convertToPdf DOCX fixed-layout table honors declared column widths',
      () async {
        // Issue #243: the fixture's 11 gridCol widths sum to 10.275 in on a
        // 10 in usable landscape page, yet the engine laid the table out at
        // ~3.4x that width — text runs out past x=2500 pt on a 792 pt page.
        // Every column marker must land inside the page.
        final pdf = createPdf();
        final sink = TestSink();
        await pdf.convertToPdf(
          src(buildWideTableDocx()),
          sink,
          format: PdfDocumentFormat.docx,
        );
        final doc = await pdf.open(src(sink.takeBytes()));
        const pageWidthPt =
            792.0; // 15840 twips landscape, as the DOCX declares
        final lefts = <double>[];
        var lastWidth = 0.0;
        for (final marker in docxWideTableMarkers) {
          final hits = await doc.search(
            query: marker,
            pages: const PdfPages.all(),
          );
          expect(hits, hasLength(1), reason: '$marker: one hit expected');
          final hit = hits.single;
          // The buggy layout ran the row out to ~3074 pt on this page.
          expect(
            hit.rect.x + hit.rect.width,
            lessThanOrEqualTo(pageWidthPt + 1),
            reason:
                '$marker spans to x=${hit.rect.x + hit.rect.width} — off the '
                '$pageWidthPt pt page (declared widths ignored, issue #243)',
          );
          lefts.add(hit.rect.x);
          lastWidth = hit.rect.width;
        }
        // "On the page" alone would also pass a content-sampled layout, so
        // check the geometry: each column's left edge advances by its
        // declared gridCol width times ONE scale factor. Identical prose in
        // every cell makes a sampled layout step by equal gaps instead, and
        // the 666-twip first column gives that away at once.
        final scales = <double>[];
        for (var i = 1; i < lefts.length; i++) {
          final gap = lefts[i] - lefts[i - 1];
          expect(
            gap,
            greaterThan(0),
            reason: 'columns must advance left→right',
          );
          scales.add(gap / (docxWideTableGridCols[i - 1] / 20.0));
        }
        // The declared widths exceed the printable width, so the converter
        // scales them down to fill it: the row must span most of the page.
        // A layout that shrank every column (or fell back to the 20 pt
        // minimum) would keep every marker on the page and still be wrong.
        final rightEdge = lefts.last + lastWidth;
        expect(
          rightEdge - lefts.first,
          greaterThanOrEqualTo(pageWidthPt * 0.75),
          reason:
              'the row spans only ${rightEdge - lefts.first} pt of the '
              '$pageWidthPt pt page — declared widths were shrunk, not '
              'honored (issue #243)',
        );
        final k = scales.reduce((a, b) => a + b) / scales.length;
        for (var i = 0; i < scales.length; i++) {
          expect(
            scales[i],
            closeTo(k, k * 0.03),
            reason:
                'column ${i + 1} width is not the declared gridCol scaled by '
                'the table-wide factor $k (issue #243)',
          );
        }
        await doc.dispose();
      },
      timeout: t(1),
    );
  });
}
