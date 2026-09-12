// CHARTER — this battery alone proves: the standalone ops (sign,
// extractPages, office conversion) transform source→sink correctly
// with no handle exposed. These ops live here and nowhere else —
// sugar composes extractPages internally but never re-proves it.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../fixtures/handwritten_docx.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerStandaloneTests(Pdf Function() createPdf) {
  group('standalone', () {
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
      final sigs = await doc.getSignatures();
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
      final sigs = await doc.getSignatures();
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
        throwsA(isA<PdfEngineError>()),
        reason:
            'a broken certificate is an engine failure with a '
            'message — never an untyped throw',
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
          throwsA(isA<PdfEngineError>()),
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
