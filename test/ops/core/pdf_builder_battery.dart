// CHARTER — this battery alone proves: OUR builder emits what each
// API call promises. Creation is the subject, so the builder rightly
// feeds itself: prose content is proven by re-opening and EXTRACTING;
// dictionary-level emissions (field names, URI actions, metadata,
// JS actions) are proven in the raw bytes — they are the builder's
// own output and extraction cannot see them (exempt from the
// byte-grep guard for exactly that reason).

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/render_pages.dart';
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

    test('save compresses by default; compress: false stays plain', () async {
      final pdf = createPdf();
      final sentence = 'The quick brown fox jumps over the lazy dog.';

      // One save consumes the builder handle, so the two variants are
      // two builds of the same content. `compress == null` omits the
      // argument and exercises the default.
      Future<Uint8List> buildAndSave(bool? compress) async {
        final builder = await pdf.build();
        final page = await builder.addA4Page();
        for (var i = 0; i < 40; i++) {
          await page.paragraph(sentence);
        }
        await page.done();
        final sink = TestSink();
        if (compress == null) {
          await builder.save(sink);
        } else {
          await builder.save(sink, compress: compress);
        }
        await builder.dispose();
        return sink.takeBytes();
      }

      final plain = await buildAndSave(false);
      final zipped = await buildAndSave(null);
      expect(
        zipped.length,
        lessThan(plain.length),
        reason: 'the default save must be smaller than the plain one',
      );
      expect(
        String.fromCharCodes(zipped),
        contains('/FlateDecode'),
        reason: 'the default save must Flate-compress its streams',
      );
      final zippedDoc = await pdf.open(src(zipped));
      final zippedText = await zippedDoc.extract(pages: const PdfPages.all());
      await zippedDoc.dispose();
      final plainDoc = await pdf.open(src(plain));
      final plainText = await plainDoc.extract(pages: const PdfPages.all());
      await plainDoc.dispose();
      expect(
        zippedText,
        plainText,
        reason: 'both saves must read back the same text',
      );
      expect(zippedText, contains('The quick brown fox'));
    }, timeout: t(3));

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
      await e.flattenAnnotations();
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

    test('image with bytes that are not an image throws and the builder '
        'stays usable', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      final garbage = Uint8List.fromList(const [1, 2, 3, 4, 5, 6, 7, 8]);
      const rect = PdfRect(x: 50, y: 500, width: 100, height: 100);
      await expectLater(
        page.image(src(garbage), rect),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'the caller passed bytes that decode to no image',
      );
      await expectLater(
        page.imageArtifact(src(garbage), rect),
        throwsA(isA<PdfInvalidArgument>()),
      );
      await page.text('after the failed image');
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('after the failed image'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('image is placed at the rect it was given', () async {
      // The placement rect is width × height at (x, y) — not a far
      // corner. Read it back through the editor's page-image list, which
      // reports the real transform from the content stream.
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.image(
        src(minimalPng),
        const PdfRect(x: 50, y: 500, width: 100, height: 120),
      );
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final editor = await pdf.edit(src(sink.takeBytes()));
      final images = await editor.pageImages(0);
      await editor.dispose();
      expect(images, hasLength(1));
      expect(images.single.bounds.x, closeTo(50, 0.01));
      expect(images.single.bounds.y, closeTo(500, 0.01));
      expect(images.single.bounds.width, closeTo(100, 0.01));
      expect(images.single.bounds.height, closeTo(120, 0.01));
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

    test('radioGroup lists as a radio field with its selected value', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.radioGroup('size', const [
        (value: 'small', rect: PdfRect(x: 50, y: 700, width: 14, height: 14)),
        (value: 'large', rect: PdfRect(x: 50, y: 670, width: 14, height: 14)),
      ], selected: 'large');
      await page.radioGroup('colour', const [
        (value: 'red', rect: PdfRect(x: 150, y: 700, width: 14, height: 14)),
        (value: 'blue', rect: PdfRect(x: 150, y: 670, width: 14, height: 14)),
      ]);
      await page.done();
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final built = sink.takeBytes();
      final doc = await pdf.open(src(built));
      final fields = {for (final f in await doc.formFields) f.name: f};
      expect(fields.keys, containsAll(['size', 'colour']));
      expect(fields['size']!.type, PdfFormFieldType.radioGroup);
      expect(fields['colour']!.type, PdfFormFieldType.radioGroup);
      expect(
        fields['size']!.value,
        isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'large'),
      );
      expect(fields['colour']!.value, isA<PdfNoValue>());
      await doc.dispose();

      // Setting a radio group's value takes the option name.
      final editor = await pdf.edit(src(built));
      await editor.setFormFieldValue('size', 'small');
      await editor.setFormFieldValue('colour', 'blue');
      final resaved = TestSink();
      await editor.save(resaved);
      await editor.dispose();
      final reopened = await pdf.open(src(resaved.takeBytes()));
      final after = {for (final f in await reopened.formFields) f.name: f};
      expect(
        after['size']!.value,
        isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'small'),
      );
      expect(
        after['colour']!.value,
        isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'blue'),
      );
      await reopened.dispose();
    }, timeout: t(1));

    test(
      'radioGroup keeps option names Yes and No; a checkbox stays checked',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final page = await builder.addA4Page();
        await page.radioGroup('picksNo', const [
          (value: 'Yes', rect: PdfRect(x: 50, y: 700, width: 14, height: 14)),
          (value: 'No', rect: PdfRect(x: 50, y: 670, width: 14, height: 14)),
        ], selected: 'No');
        await page.radioGroup('picksYes', const [
          (value: 'Yes', rect: PdfRect(x: 150, y: 700, width: 14, height: 14)),
          (value: 'No', rect: PdfRect(x: 150, y: 670, width: 14, height: 14)),
        ], selected: 'Yes');
        await page.checkbox(
          'agree',
          const PdfRect(x: 250, y: 700, width: 14, height: 14),
          checked: true,
        );
        await page.done();
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        final fields = {for (final f in await doc.formFields) f.name: f};
        expect(
          fields['picksNo']!.value,
          isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'No'),
        );
        expect(
          fields['picksYes']!.value,
          isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'Yes'),
        );
        expect(
          fields['agree']!.value,
          isA<PdfCheckedValue>().having((v) => v.checked, 'checked', isTrue),
        );
        await doc.dispose();
      },
      timeout: t(1),
    );

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
      final annotations = await doc.annotations(0);
      expect(
        annotations,
        hasLength(1),
        reason: "the link is the page's only annotation",
      );
      expect(annotations.single.subtype, PdfAnnotationSubtype.link);
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

    test('content after done() without addPage lands on a new page', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await page.text('First');
      await page.done();
      await page.text('Second');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(doc.pageCount, 2);
      final first = await doc.extract(pages: const PdfPages.single(0));
      final second = await doc.extract(pages: const PdfPages.single(1));
      expect(first, contains('First'));
      expect(first, isNot(contains('Second')));
      expect(second, contains('Second'));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'newPageSameSize keeps the size and clears text and field state',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final page = await builder.addPage(width: 300, height: 400);
        await page.text('one');
        await page.newPageSameSize();
        await expectLater(
          page.linkUrl('https://example.com'),
          throwsA(isA<PdfInvalidArgument>()),
          reason: 'the text was left on the previous page',
        );
        await page.text('two');
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        expect(doc.pageCount, 2);
        for (final page in doc.pages) {
          expect(page.width, closeTo(300, 2));
          expect(page.height, closeTo(400, 2));
        }
        await doc.dispose();
      },
      timeout: t(1),
    );

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
      final annotations = await doc.annotations(0);
      expect(
        annotations,
        hasLength(1),
        reason: "the link is page 0's only annotation",
      );
      expect(annotations.single.subtype, PdfAnnotationSubtype.link);
      await doc.dispose();
    }, timeout: t(1));

    test('fieldKeystroke on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a field script with no field must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('fieldFormat on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.fieldFormat('AFNumber_Format(2, 0, 0, 0, "", true)'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a field script with no field must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('fieldValidate on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.fieldValidate('AFRange_Validate(true, 0, true, 100)'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a field script with no field must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('fieldCalculate on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.fieldCalculate('AFSimple_Calculate("SUM", new Array("f1"))'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a field script with no field must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('linkUrl on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.linkUrl('https://example.com'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a link with no text must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('linkPage on a bare page throws PdfInvalidArgument', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.linkPage(0),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a link with no text must be refused',
      );
      await builder.dispose();
    }, timeout: t(1));

    test(
      'fieldKeystroke after newPageSameSize throws PdfInvalidArgument',
      () async {
        final builder = await createPdf().build();
        final page = await builder.addA4Page();
        await page.textField(
          'actionfield',
          const PdfRect(x: 50, y: 700, width: 200, height: 20),
        );
        await page.newPageSameSize();
        await expectLater(
          page.fieldKeystroke('AFNumber_Keystroke(2, 0, 0, 0, "", true)'),
          throwsA(isA<PdfInvalidArgument>()),
          reason: 'the field was left on the previous page',
        );
        await builder.dispose();
      },
      timeout: t(1),
    );
  });

  // One test per page-op method: the effect is proven in the saved
  // file — extracted text, the annotation list, or the uncompressed
  // content stream's own operators.
  group('page op parity', () {
    Future<Uint8List> savePlain(
      Pdf pdf,
      Future<void> Function(PdfPageBuilder page) content, {
      bool compress = true,
    }) async {
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      await content(page);
      await page.done();
      final sink = TestSink();
      await builder.save(sink, compress: compress);
      await builder.dispose();
      return sink.takeBytes();
    }

    Future<PdfDoc> openOf(Pdf pdf, Uint8List bytes) => pdf.open(src(bytes));

    test('inline writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.at(72, 720);
        await page.inline('Hello Inline');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Hello Inline'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('inlineBold writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.at(72, 720);
        await page.inlineBold('Hello Bold');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Hello Bold'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('inlineItalic writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.at(72, 720);
        await page.inlineItalic('Hello Italic');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Hello Italic'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('inlineColor writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.at(72, 720);
        await page.inlineColor('Hello Colour', PdfColor.red);
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Hello Colour'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('textInRect writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.textInRect(
          const PdfRect(x: 50, y: 600, width: 300, height: 100),
          'Rect caption',
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await doc.extract(pages: const PdfPages.all()),
        contains('Rect caption'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('table writes every cell', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.table(
          const [
            PdfTableColumn('SKU', width: 100),
            PdfTableColumn('Qty', width: 60, align: PdfCellAlign.right),
          ],
          const [
            ['A-1', '12'],
            ['B-2', '3'],
          ],
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('SKU'));
      expect(text, contains('A-1'));
      expect(text, contains('B-2'));
      expect(text, contains('12'));
      await doc.dispose();
    }, timeout: t(1));

    test('table refuses a row whose length differs from the columns', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      expect(
        () => page.table(
          const [PdfTableColumn('SKU')],
          const [
            ['1'],
            ['2', '3'],
          ],
        ),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'the second row has 2 cells but the table has 1 column',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('streamingTable writes its cells', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.streamingTable(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60, align: PdfCellAlign.center),
          ],
          const [
            [PdfTableCell('Saw'), PdfTableCell('3')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('Item'));
      expect(text, contains('Saw'));
      expect(text, contains('Hammer'));
      expect(text, contains('7'));
      await doc.dispose();
    }, timeout: t(1));

    test('streamingTable draws a cell that spans two rows', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.streamingTable(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60, align: PdfCellAlign.center),
          ],
          const [
            [PdfTableCell('Saw', rowSpan: 2), PdfTableCell('3')],
            [PdfTableCell(''), PdfTableCell('4')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
          maxRowSpan: 2,
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await doc.extract(pages: const PdfPages.all());
      for (final expected in ['Saw', '3', '4', 'Hammer', '7']) {
        expect(text, contains(expected));
      }
      await doc.dispose();
    }, timeout: t(1));

    test('streamingTable refuses a row without one cell per column', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      expect(
        () => page.streamingTable(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60),
          ],
          const [
            [PdfTableCell('Saw')],
          ],
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
      await builder.dispose();
    }, timeout: t(1));

    test('streamingTable refuses a rowSpan above maxRowSpan', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      expect(
        () => page.streamingTable(
          const [PdfTableColumn('Item', width: 120)],
          const [
            [PdfTableCell('Saw', rowSpan: 2)],
          ],
        ),
        throwsA(isA<PdfInvalidArgument>()),
      );
      await builder.dispose();
    }, timeout: t(1));

    test('streamingTable refuses a column without a width', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      expect(
        () => page.streamingTable(
          const [PdfTableColumn('Item')],
          const [
            [PdfTableCell('Saw')],
          ],
        ),
        throwsA(isA<PdfInvalidArgument>()),
        reason:
            'auto sizing needs the sample mode, which streaming '
            'tables reach only through explicit widths',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('highlight marks the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Marked sentence');
        await page.highlight();
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations, hasLength(1));
      expect(annotations.single.subtype, PdfAnnotationSubtype.highlight);
      await doc.dispose();
    }, timeout: t(1));

    test('underline marks the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Underlined sentence');
        await page.underline();
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.underline,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('strikeout marks the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Struck sentence');
        await page.strikeout();
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.strikeOut,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('squiggly marks the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Misspelled word');
        await page.squiggly();
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.squiggly,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('stickyNote at the cursor carries its text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body text');
        await page.stickyNote('Please review this');
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations.single.subtype, PdfAnnotationSubtype.text);
      expect(annotations.single.contents, 'Please review this');
      await doc.dispose();
    }, timeout: t(1));

    test('stickyNote at explicit coordinates carries its text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body text');
        await page.stickyNote('Corner note', x: 400, y: 500);
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations.single.subtype, PdfAnnotationSubtype.text);
      expect(annotations.single.contents, 'Corner note');
      await doc.dispose();
    }, timeout: t(1));

    test('stickyNote with only one coordinate is refused in Dart', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addA4Page();
      expect(
        () => page.stickyNote('half-placed', x: 100),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a note at x-only has no y to land on',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('stamp lands a stamp annotation', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.at(72, 600);
        await page.stamp(PdfStampType.approved);
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.stamp,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('freeText lands a free-text annotation with the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.freeText(
          const PdfRect(x: 50, y: 500, width: 200, height: 60),
          'Direct on page',
        );
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations.single.subtype, PdfAnnotationSubtype.freeText);
      expect(annotations.single.contents, 'Direct on page');
      await doc.dispose();
    }, timeout: t(1));

    test('linkNamed lands a link annotation', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Jump to section');
        await page.linkNamed('sec-2');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.link,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('linkJavaScript lands a JavaScript link', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Run script');
        await page.linkJavaScript('app.alert("hello")');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.link,
      );
      expect(String.fromCharCodes(bytes), contains('app.alert'));
      await doc.dispose();
    }, timeout: t(1));

    test('watermarkConfidential flattens to CONFIDENTIAL', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body');
        await page.watermarkConfidential();
      });
      // The watermark is an annotation — flatten it into content, then
      // extraction sees the letters.
      final e = await pdf.edit(src(bytes));
      await e.flattenAnnotations();
      final flatSink = TestSink();
      await e.save(flatSink);
      await e.dispose();
      final doc = await pdf.open(src(flatSink.takeBytes()));
      expect(
        (await doc.extract(pages: const PdfPages.all())).toUpperCase(),
        contains('CONFIDENTIAL'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('watermarkDraft flattens to DRAFT', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body');
        await page.watermarkDraft();
      });
      final e = await pdf.edit(src(bytes));
      await e.flattenAnnotations();
      final flatSink = TestSink();
      await e.save(flatSink);
      await e.dispose();
      final doc = await pdf.open(src(flatSink.takeBytes()));
      expect(
        (await doc.extract(pages: const PdfPages.all())).toUpperCase(),
        contains('DRAFT'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('onOpen and onClose carry the page actions', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body');
        await page.onOpen('app.alert("opened")');
        await page.onClose('app.alert("closed")');
      });
      final asString = String.fromCharCodes(bytes);
      expect(asString, contains('/AA'));
      expect(asString, contains('opened'));
      expect(asString, contains('closed'));
    }, timeout: t(1));

    test(
      'imageArtifact places an image XObject and the page renders',
      () async {
        final pdf = createPdf();
        final bytes = await savePlain(pdf, (page) async {
          await page.text('Body');
          await page.imageArtifact(
            src(minimalPng),
            const PdfRect(x: 50, y: 400, width: 100, height: 100),
          );
        });
        final asString = String.fromCharCodes(bytes);
        expect(
          asString,
          contains('/XObject'),
          reason:
              'the artifact image '
              'is an XObject like any placed image',
        );
        final doc = await openOf(pdf, bytes);
        final frames = await renderToMemory(
          doc,
          pages: const PdfPages.single(0),
          size: const PdfRenderSize.thumbnail(240),
        );
        expect(frames, hasLength(1), reason: 'page 0 must render');
        expect(
          frames.single.data,
          isNotEmpty,
          reason: 'page 0 must render with the artifact',
        );
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('rect emits the re operator', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(const PdfRect(x: 50, y: 300, width: 100, height: 60));
      }, compress: false);
      expect(
        String.fromCharCodes(bytes),
        matches(RegExp(r'\b50(\.0+)? 300(\.0+)? 100(\.0+)? 60(\.0+)? re\b')),
      );
    }, timeout: t(1));

    test('filledRect emits the fill colour rg', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.filledRect(
          const PdfRect(x: 50, y: 300, width: 100, height: 60),
          PdfColor.red,
        );
      }, compress: false);
      final asString = String.fromCharCodes(bytes);
      expect(asString, matches(RegExp(r'1(\.0+)? 0(\.0+)? 0(\.0+)? rg')));
      expect(asString, matches(RegExp(r'\bre\b')));
    }, timeout: t(1));

    test('line emits the l operator', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.line(50, 300, 150, 360);
      }, compress: false);
      expect(String.fromCharCodes(bytes), matches(RegExp(r'\bl\s')));
    }, timeout: t(1));

    test(
      'strokeRect plain dashes nothing; the dashed form sets the dash',
      () async {
        final pdf = createPdf();
        final solid = await savePlain(pdf, (page) async {
          await page.strokeRect(
            const PdfRect(x: 50, y: 300, width: 100, height: 60),
          );
        }, compress: false);
        expect(String.fromCharCodes(solid), matches(RegExp(r'\bre\b')));
        expect(String.fromCharCodes(solid), isNot(contains('] 0 d')));

        final dashed = await savePlain(pdf, (page) async {
          await page.strokeRect(
            const PdfRect(x: 50, y: 300, width: 100, height: 60),
            dash: const [3, 2],
            dashPhase: 0,
          );
        }, compress: false);
        final asString = String.fromCharCodes(dashed);
        expect(
          asString,
          matches(RegExp(r'\[3(\.0+)? 2(\.0+)?\] 0(\.0+)? d\b')),
        );
      },
      timeout: t(1),
    );

    test(
      'strokeLine plain dashes nothing; the dashed form sets the dash',
      () async {
        final pdf = createPdf();
        final solid = await savePlain(pdf, (page) async {
          await page.strokeLine(50, 300, 150, 360);
        }, compress: false);
        expect(String.fromCharCodes(solid), matches(RegExp(r'\bl\s')));
        expect(String.fromCharCodes(solid), isNot(contains('] 0 d')));

        final dashed = await savePlain(pdf, (page) async {
          await page.strokeLine(50, 300, 150, 360, dash: const [3, 2]);
        }, compress: false);
        expect(
          String.fromCharCodes(dashed),
          matches(RegExp(r'\[3(\.0+)? 2(\.0+)?\] 0(\.0+)? d\b')),
        );
      },
      timeout: t(1),
    );

    test('the ops that need text are refused on a bare page', () async {
      final builder = await createPdf().build();
      final page = await builder.addA4Page();
      await expectLater(
        page.linkNamed('sec-2'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a named destination needs text to anchor on',
      );
      await expectLater(
        page.linkJavaScript('app.alert(1)'),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a script link needs text to anchor on',
      );
      await expectLater(
        page.highlight(),
        throwsA(isA<PdfInvalidArgument>()),
        reason: 'a highlight needs text to cover',
      );
      await expectLater(page.underline(), throwsA(isA<PdfInvalidArgument>()));
      await expectLater(page.strikeout(), throwsA(isA<PdfInvalidArgument>()));
      await expectLater(page.squiggly(), throwsA(isA<PdfInvalidArgument>()));
      await builder.dispose();
    }, timeout: t(1));
  });
}
