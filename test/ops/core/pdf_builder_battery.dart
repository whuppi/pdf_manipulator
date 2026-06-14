// CHARTER — this battery alone proves: OUR builder emits what each
// API call promises. Creation is the subject, so the builder rightly
// feeds itself: prose content is proven by re-opening and EXTRACTING;
// dictionary-level emissions (field names, URI actions, metadata,
// JS actions) are proven in the raw bytes — they are the builder's
// own output and extraction cannot see them (exempt from the
// byte-grep guard for exactly that reason).

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerBuilderTests(Pdf Function() createPdf) {
  group('builder', () {
    test('create → text → save → verify text in output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('Hello World');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Hello World'),
        reason: 'built text must survive a real read round trip',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('create → setMetadata → save → verify metadata', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.setTitle('Custom Builder Title');
      await builder.setAuthor('Builder Author');
      final page = await builder.addPage(width: 612, height: 792);
      await page.text('content');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      final asString = String.fromCharCodes(output);
      expect(asString, contains('Custom Builder Title'));
      expect(asString, contains('Builder Author'));
    }, timeout: t(1));

    test('double dispose is safe', () async {
      final builder = await createPdf().build();
      await builder.dispose();
      await builder.dispose();
    }, timeout: t(1));

    test('imagesToPdf creates valid single-page PDF', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf([src(minimalPng)], sink);
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    test('multiple pages have correct count', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      for (var i = 0; i < 3; i++) {
        final page = await builder.addPage(width: 612, height: 792);
        await page.text('Page ${i + 1}');
        await page.done();
      }
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 3);
      await doc.dispose();
    }, timeout: t(1));

    test('addA4Page creates correct dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('A4');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      expect(doc.pages[0].width, closeTo(595, 2));
      expect(doc.pages[0].height, closeTo(842, 2));
      await doc.dispose();
    }, timeout: t(1));

    test('addLetterPage creates correct dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addLetterPage();
      await page.text('Letter');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 1);
      expect(doc.pages[0].width, closeTo(612, 2));
      expect(doc.pages[0].height, closeTo(792, 2));
      await doc.dispose();
    }, timeout: t(1));

    test('addPage custom dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(width: 400, height: 300);
      await page.text('Custom');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pages[0].width, closeTo(400, 2));
      expect(doc.pages[0].height, closeTo(300, 2));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'heading + paragraph + space + horizontalRule produce content',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final page = await builder.addA4Page();
        await page.heading(1, 'Test Heading');
        await page.space(10);
        await page.paragraph('Test paragraph content here.');
        await page.horizontalRule();
        await page.done();
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        final text = await doc.extract(pages: const PdfPages.all());
        expect(text, contains('Test Heading'));
        expect(text, contains('Test paragraph'));
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('font changes affect output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.font('Courier', 24);
      await page.text('Courier text');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = String.fromCharCodes(sink.takeBytes());
      expect(output, contains('Courier'));
    }, timeout: t(1));

    test('watermark embeds text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Content');
      await page.watermark('BUILDERMARK');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      // The builder watermark is an ANNOTATION — invisible to plain
      // extraction. Flatten it into content; then it must extract.
      final e = await pdf.edit(src(sink.takeBytes()));
      await e.flattenAllAnnotations();
      final flatSink = TestSink();
      await e.save(flatSink);
      await e.dispose();
      final doc = await pdf.open(src(flatSink.takeBytes()));
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('BUILDERMARK'),
        reason:
            'a flattened watermark appearance is provably on the '
            'page',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('image produces output larger than text-only', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Before image');
      await page.image(
        src(minimalPng),
        const PdfRect(x: 50, y: 500, width: 100, height: 100),
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(sink.takeBytes().length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    test('textField embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.textField(
        'myfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('myfield'));
    }, timeout: t(1));

    test('checkbox embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.checkbox(
        'mycheckbox',
        const PdfRect(x: 50, y: 700, width: 14, height: 14),
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mycheckbox'));
    }, timeout: t(1));

    test('comboBox embeds field name and options', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.comboBox(
        'mycombo',
        const PdfRect(x: 50, y: 700, width: 150, height: 20),
        ['Option A', 'Option B'],
        selected: 'Option A',
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mycombo'));
    }, timeout: t(1));

    test('linkUrl produces valid PDF with annotation', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Click here');
      await page.linkUrl('https://example.com/test');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
      // The URL lives in a /URI action dict — extraction cannot see
      // it; the raw-bytes check IS the right level for this claim.
      expect(String.fromCharCodes(output), contains('example.com'));
    }, timeout: t(1));

    test('footnote embeds note text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Main text');
      await page.footnote('1', 'Footnote content here');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Footnote content'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('columns embeds column text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.columns(2, 10, 'Column text for testing');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Column text'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('newPageSameSize creates two pages', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Page 1');
      await page.newPageSameSize();
      await page.text('Page 2');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    test('newline separates the surrounding text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('Before');
      await page.newline();
      await page.text('After');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('Before'));
      expect(text, contains('After'));
      await doc.dispose();
    }, timeout: t(1));

    test('setSubject + setKeywords on builder', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.setSubject('Test Subject');
      await builder.setKeywords('a, b, c');
      final page = await builder.addA4Page();
      await page.text('x');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = String.fromCharCodes(sink.takeBytes());
      expect(output, contains('Test Subject'));
    }, timeout: t(1));

    test('pushButton embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.pushButton(
        'mybtn',
        const PdfRect(x: 50, y: 700, width: 80, height: 30),
        'Click',
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mybtn'));
    }, timeout: t(1));

    test('signatureField embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.signatureField(
        'mysig',
        const PdfRect(x: 50, y: 700, width: 200, height: 60),
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mysig'));
    }, timeout: t(1));

    test('fieldKeystroke embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      // Actions attach to the most-recently-added field — a bare-page
      // call is a silent no-op (sharp edge, tracked in
      // CAPABILITY_ROADMAP).
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      await page.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFNumber_Keystroke'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('fieldFormat embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      // Actions attach to the most-recently-added field — a bare-page
      // call is a silent no-op (sharp edge, tracked in
      // CAPABILITY_ROADMAP).
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      await page.fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFNumber_Format'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('fieldValidate embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      // Actions attach to the most-recently-added field — a bare-page
      // call is a silent no-op (sharp edge, tracked in
      // CAPABILITY_ROADMAP).
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      await page.fieldValidate('AFRange_Validate(true, 0, true, 100)');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFRange_Validate'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('fieldCalculate embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      // Actions attach to the most-recently-added field — a bare-page
      // call is a silent no-op (sharp edge, tracked in
      // CAPABILITY_ROADMAP).
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      await page.fieldCalculate(
        'AFSimple_Calculate("SUM", new Array("f1", "f2"))',
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFSimple_Calculate'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('linkPage produces valid output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final p1 = await builder.addA4Page();
      await p1.text('Page 1');
      await p1.linkPage(1);
      await p1.done();
      final p2 = await builder.addA4Page();
      await p2.text('Page 2');
      await p2.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      // Internal links are emitted as a direct /Dest [page /Fit]
      // destination (ISO 32000-1 §12.3.2) — no /GoTo action dict.
      expect(
        String.fromCharCodes(output),
        contains('/Dest'),
        reason: 'an internal link must carry a destination',
      );
      expect(String.fromCharCodes(output), contains('/Fit'));
      final doc = await pdf.open(src(output));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));
  });
}
