// CHARTER — this battery alone proves: form-field names and values
// survive every PDF text-string encoding a real producer emits
// (ISO 32000-1 §7.9.2.2 PDFDocEncoding literals, UTF-16BE-with-BOM hex
// strings, and the spec-violating raw-UTF-8 literals LibreOffice-class
// tools write), through fill-by-name, save round-trips, and
// flatten-then-extract. The ASCII happy path is the editor battery's
// claim, not this one's.
//
// Born from a field report: German forms with ß in field names and
// values filled fine but flattened into mojibake. Latin-1-range
// characters (0x80–0xFF) are the exact range where UTF-8 bytes and
// PDFDoc/WinAnsi single bytes diverge — ASCII cancels the difference,
// CJK takes the fallback-font path, only this band exposes the seams.
//
// Diet: handwritten micro fixtures (one per /T encoding flavor) + the
// dart-pdf form_fields_de fixture (a fourth producer flavor).
// Presence proofs are SEMANTIC: flatten into content, then extract.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// The field name shared by all three handwritten fixtures.
const _sharedName = 'Prüfstraße und Hausnr';

void registerFormEncodingTests(Pdf Function() createPdf) {
  group('form text encoding', () {
    // ── Fill-by-name across /T encoding flavors ──

    test('fill finds a PDFDocEncoding (Latin-1) field name', () async {
      final editor = await createPdf().edit(src(formPdfdocNamePdf));
      await editor.setFormFieldValue(_sharedName, 'Ligusterweg 4');
      await editor.dispose();
    }, timeout: t(1));

    test('fill finds a UTF-16BE-with-BOM field name', () async {
      final editor = await createPdf().edit(src(formUtf16NamePdf));
      await editor.setFormFieldValue(_sharedName, 'Ligusterweg 4');
      await editor.dispose();
    }, timeout: t(1));

    test(
      'fill finds a raw-UTF-8 field name (spec-violating, real-world)',
      () async {
        final editor = await createPdf().edit(src(formUtf8NamePdf));
        await editor.setFormFieldValue(_sharedName, 'Ligusterweg 4');
        await editor.dispose();
      },
      timeout: t(1),
    );

    test('fill finds the dart-pdf-encoded field name', () async {
      final editor = await createPdf().edit(src(fFormFieldsDe));
      await editor.setFormFieldValue(
        fFormFieldsDeTruth.fieldName,
        'Ligusterweg 4',
      );
      await editor.dispose();
    }, timeout: t(1));

    // ── Values through flatten (the field-report symptom) ──

    // Control row: same fixture and pipeline as the Latin-1 test below,
    // ASCII-only value. Separates "the encoding bake is wrong" from
    // "fill → flatten loses the value on a field with no /AP at all."
    test('ASCII value survives fill → flatten on an AP-less field', () async {
      final pdf = createPdf();
      const value = 'Ligusterweg 4';
      final editor = await pdf.edit(src(formUtf16NamePdf));
      await editor.setFormFieldValue(_sharedName, value);
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(value));
      await doc.dispose();
    }, timeout: t(2));

    test('Latin-1-range value survives fill → flatten → extract', () async {
      final pdf = createPdf();
      const value = 'Königstraße 42, Grüße aus München äöüß';
      final editor = await pdf.edit(src(formUtf16NamePdf));
      await editor.setFormFieldValue(_sharedName, value);
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains(value),
        reason:
            'the flattened page must carry the value verbatim — '
            'Latin-1-range characters must not bake as UTF-8 bytes '
            'under a single-byte font encoding',
      );
      await doc.dispose();
    }, timeout: t(2));

    test('foreign pre-filled PDFDocEncoding value survives flatten', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(formPdfdocNamePdf));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(
        text,
        contains('Königsallee 42, München'),
        reason:
            'the /V bytes are PDFDocEncoding (not valid UTF-8) — '
            'flatten must decode them per §7.9.2.2, not lossily as UTF-8',
      );
      await doc.dispose();
    }, timeout: t(2));

    test('dart-pdf default value survives flatten', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsDe));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains(fFormFieldsDeTruth.textFieldDefault));
      await doc.dispose();
    }, timeout: t(2));

    test('CJK value survives fill → flatten → extract', () async {
      final pdf = createPdf();
      const value = '東京都渋谷区 123';
      final editor = await pdf.edit(src(formUtf16NamePdf));
      await editor.setFormFieldValue(_sharedName, value);
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      // Asserted as two runs, not one string: the flattener bakes the
      // CJK run (embedded fallback font) and the Latin run ' 123'
      // (/DA font, space glyph included) as separate Tj operators, and
      // the EXTRACTOR's word-gap heuristic drops the space at the
      // font-run boundary. Glyph fidelity is this battery's claim;
      // inter-run spacing belongs to the extraction pipeline.
      expect(
        text,
        contains('東京都渋谷区'),
        reason:
            'above-Latin-1 values take the fallback-font path; '
            'flatten must embed a covering font and bake real glyphs',
      );
      expect(
        text,
        contains('123'),
        reason: 'the Latin run must bake under the /DA font alongside',
      );
      await doc.dispose();
    }, timeout: t(2));

    test(
      'value with parentheses, backslash and umlauts survives flatten',
      () async {
        final pdf = createPdf();
        const value = r'(Grüße) \ Straße';
        final editor = await pdf.edit(src(formUtf16NamePdf));
        await editor.setFormFieldValue(_sharedName, value);
        await editor.flattenForms();
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        final text = await doc.extract(pages: const PdfPages.all());
        expect(text, contains(value));
        await doc.dispose();
      },
      timeout: t(2),
    );

    // ── Round-trips that must not corrupt what they touch ──

    test('PDFDocEncoding field name survives fill → save → refill', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(formPdfdocNamePdf));
      await editor.setFormFieldValue(_sharedName, 'Erste Füllung');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      // If save re-encoded /T wrongly, this second fill cannot find it.
      final editor2 = await pdf.edit(src(sink.takeBytes()));
      await editor2.setFormFieldValue(_sharedName, 'Zweite Füllung');
      await editor2.dispose();
    }, timeout: t(2));

    test('PDFDocEncoding document title decodes on read', () async {
      final editor = await createPdf().edit(src(formPdfdocNamePdf));
      expect(
        await editor.getTitle(),
        'Straßen-Formular für Prüfung',
        reason:
            'the /Title bytes are PDFDocEncoding (not valid UTF-8) — '
            'metadata reads must decode per §7.9.2.2',
      );
      await editor.dispose();
    }, timeout: t(1));
  });
}
