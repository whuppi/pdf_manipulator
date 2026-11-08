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
import '../../harness/extract_text.dart';
import '../../harness/render_pages.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerBuilderTests(Pdf Function() createPdf) {
  group('builder', () {
    test('create → text → save → verify text in output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(PdfPageSize.points(612, 792));
      await page.text('Hello World');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Hello World'),
        reason: 'built text must survive a real read round trip',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('create → setMetadata → save → verify metadata', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.setMetadata(
        const PdfMetadata(
          title: 'Custom Builder Title',
          author: 'Builder Author',
        ),
      );
      final page = await builder.addPage(PdfPageSize.points(612, 792));
      await page.text('content');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
      final asString = String.fromCharCodes(output);
      expect(asString, contains('Custom Builder Title'));
      expect(asString, contains('Builder Author'));
    }, timeout: t(1));

    test('save deflates by default; deflate: false stays plain', () async {
      final pdf = createPdf();
      final sentence = 'The quick brown fox jumps over the lazy dog.';

      // One save consumes the builder handle, so the two variants are
      // two builds of the same content. `deflate == null` omits the
      // argument and exercises the default.
      Future<Uint8List> buildAndSave(bool? deflate) async {
        final builder = await pdf.build();
        final page = await builder.addPage();
        for (var i = 0; i < 40; i++) {
          await page.paragraph(sentence);
        }
        final sink = TestSink();
        if (deflate == null) {
          await builder.save(sink);
        } else {
          await builder.save(sink, deflate: deflate);
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
      final zippedDoc = await pdf.open(PdfInput(src(zipped)));
      final zippedText = await extractTextToString(
        zippedDoc,
        pages: const PdfPages.all(),
      );
      await zippedDoc.dispose();
      final plainDoc = await pdf.open(PdfInput(src(plain)));
      final plainText = await extractTextToString(
        plainDoc,
        pages: const PdfPages.all(),
      );
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
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
      expect(output.length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    test('imagesToPdf gives every page the size it is told', () async {
      final pdf = createPdf();
      final sink = TestSink();
      await pdf.imagesToPdf(
        [src(minimalPng), src(minimalPng)],
        sink,
        page: const PdfImagePage(
          size: PdfImagePageSize.fixed(PdfPageSize.points(200, 300)),
        ),
      );
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(doc.pageCount, 2);
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(200, 2));
      expect(pages[0].height, closeTo(300, 2));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'addImagePage sizes the page to the image when no size is named',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        await builder.addImagePage(src(minimalPng));
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        expect(doc.pageCount, 1);
        final pages = await doc.pageInfo();
        expect(
          pages[0].width,
          closeTo(2, 2),
          reason: 'one point per pixel of the 2x2 image',
        );
        expect(pages[0].height, closeTo(2, 2));
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('addImagePage takes the size and the margin it is told', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.addImagePage(
        src(minimalPng),
        page: const PdfImagePage(
          size: PdfImagePageSize.fixed(PdfPageSize.points(200, 300)),
          fit: PdfImageFit.fill,
          margin: 10,
        ),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(200, 2));
      expect(pages[0].height, closeTo(300, 2));
      await doc.dispose();
    }, timeout: t(1));

    test('multiple pages have correct count', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      for (var i = 0; i < 3; i++) {
        final page = await builder.addPage(PdfPageSize.points(612, 792));
        await page.text('Page ${i + 1}');
      }
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(doc.pageCount, 3);
      await doc.dispose();
    }, timeout: t(1));

    test('addPage with no size creates an A4 page', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.text('A4');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(doc.pageCount, 1);
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(595, 2));
      expect(pages[0].height, closeTo(842, 2));
      await doc.dispose();
    }, timeout: t(1));

    test('addPage with the Letter size creates correct dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(PdfPageSize.letter());
      await page.text('Letter');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(doc.pageCount, 1);
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(612, 2));
      expect(pages[0].height, closeTo(792, 2));
      await doc.dispose();
    }, timeout: t(1));

    test('addPage custom dimensions', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage(PdfPageSize.points(400, 300));
      await page.text('Custom');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final pages = await doc.pageInfo();
      expect(pages[0].width, closeTo(400, 2));
      expect(pages[0].height, closeTo(300, 2));
      await doc.dispose();
    }, timeout: t(1));

    test(
      'heading + paragraph + space + horizontalRule produce content',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final page = await builder.addPage();
        await page.heading(1, 'Test Heading');
        await page.space(10);
        await page.paragraph('Test paragraph content here.');
        await page.horizontalRule();
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        final text = await extractTextToString(
          doc,
          pages: const PdfPages.all(),
        );
        expect(text, contains('Test Heading'));
        expect(text, contains('Test paragraph'));
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('font changes affect output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.setFont('Courier', 24);
      await page.text('Courier text');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = String.fromCharCodes(sink.takeBytes());
      expect(output, contains('Courier'));
    }, timeout: t(1));

    test('watermark embeds text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.text('Content');
      await page.watermark('BUILDERMARK');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      // The builder watermark is an ANNOTATION — invisible to plain
      // extraction. Flatten it into content; then it must extract.
      final e = await pdf.edit(PdfInput(src(sink.takeBytes())));
      await e.flattenAnnotations();
      final flatSink = TestSink();
      await e.save(flatSink);
      await e.dispose();
      final doc = await pdf.open(PdfInput(src(flatSink.takeBytes())));
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
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
      final page = await builder.addPage();
      await page.text('Before image');
      await page.image(
        src(minimalPng),
        const PdfRect(x: 50, y: 500, width: 100, height: 100),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(sink.takeBytes().length, greaterThan(minimalPdf.length));
    }, timeout: t(1));

    test('image with bytes that are not an image throws and the builder '
        'stays usable', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      final garbage = Uint8List.fromList(const [1, 2, 3, 4, 5, 6, 7, 8]);
      const rect = PdfRect(x: 50, y: 500, width: 100, height: 100);
      await expectLater(
        page.image(src(garbage), rect),
        throwsA(isA<PdfEngineFailed>()),
        reason: 'the caller passed bytes that decode to no image',
      );
      await expectLater(
        page.image(src(garbage), rect, altText: 'a broken picture'),
        throwsA(isA<PdfEngineFailed>()),
      );
      await page.text('after the failed image');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
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
      final page = await builder.addPage();
      await page.image(
        src(minimalPng),
        const PdfRect(x: 50, y: 500, width: 100, height: 120),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();

      final editor = await pdf.edit(PdfInput(src(sink.takeBytes())));
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
      final page = await builder.addPage();
      await page.textField(
        'myfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('myfield'));
    }, timeout: t(1));

    test('checkbox embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.checkbox(
        'mycheckbox',
        const PdfRect(x: 50, y: 700, width: 14, height: 14),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mycheckbox'));
    }, timeout: t(1));

    test('comboBox embeds field name and options', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.comboBox(
        'mycombo',
        const PdfRect(x: 50, y: 700, width: 150, height: 20),
        ['Option A', 'Option B'],
        selected: 'Option A',
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mycombo'));
    }, timeout: t(1));

    test('radioGroup lists as a radio field with its selected value', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.radioGroup('size', const [
        PdfRadioOption('small', PdfRect(x: 50, y: 700, width: 14, height: 14)),
        PdfRadioOption('large', PdfRect(x: 50, y: 670, width: 14, height: 14)),
      ], selected: 'large');
      await page.radioGroup('colour', const [
        PdfRadioOption('red', PdfRect(x: 150, y: 700, width: 14, height: 14)),
        PdfRadioOption('blue', PdfRect(x: 150, y: 670, width: 14, height: 14)),
      ]);
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final built = sink.takeBytes();
      final doc = await pdf.open(PdfInput(src(built)));
      final fields = {for (final f in await doc.formFields()) f.name: f};
      expect(fields.keys, containsAll(['size', 'colour']));
      expect(fields['size']!.kind, PdfFormFieldKind.radioGroup);
      expect(fields['colour']!.kind, PdfFormFieldKind.radioGroup);
      expect(
        fields['size']!.value,
        isA<PdfChoiceValue>().having((v) => v.choice, 'choice', 'large'),
      );
      expect(fields['colour']!.value, isA<PdfNoValue>());
      await doc.dispose();

      // Setting a radio group's value takes the option name.
      final editor = await pdf.edit(PdfInput(src(built)));
      await editor.field('size').setValue(PdfFormFieldValue.text('small'));
      await editor.field('colour').setValue(PdfFormFieldValue.text('blue'));
      final resaved = TestSink();
      await editor.save(resaved);
      await editor.dispose();
      final reopened = await pdf.open(PdfInput(src(resaved.takeBytes())));
      final after = {for (final f in await reopened.formFields()) f.name: f};
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
        final page = await builder.addPage();
        await page.radioGroup('picksNo', const [
          PdfRadioOption('Yes', PdfRect(x: 50, y: 700, width: 14, height: 14)),
          PdfRadioOption('No', PdfRect(x: 50, y: 670, width: 14, height: 14)),
        ], selected: 'No');
        await page.radioGroup('picksYes', const [
          PdfRadioOption('Yes', PdfRect(x: 150, y: 700, width: 14, height: 14)),
          PdfRadioOption('No', PdfRect(x: 150, y: 670, width: 14, height: 14)),
        ], selected: 'Yes');
        await page.checkbox(
          'agree',
          const PdfRect(x: 250, y: 700, width: 14, height: 14),
          isChecked: true,
        );
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        final fields = {for (final f in await doc.formFields()) f.name: f};
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
          isA<PdfCheckedValue>().having(
            (v) => v.isChecked,
            'isChecked',
            isTrue,
          ),
        );
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('a url link produces valid PDF with annotation', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.text(
        'Click here',
        link: const PdfLink.url('https://example.com/test'),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      final doc = await pdf.open(PdfInput(src(output)));
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
      final page = await builder.addPage();
      await page.text('Main text');
      await page.footnote('1', 'Footnote content here');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Footnote content'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('columns embeds column text', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.columns(2, 10, 'Column text for testing');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Column text'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('adding a second page creates two pages', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final first = await builder.addPage();
      await first.text('Page 1');
      final second = await builder.addPage();
      await second.text('Page 2');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(doc.pageCount, 2);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'two pages added with the same explicit size keep that size',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final first = await builder.addPage(PdfPageSize.points(300, 400));
        await first.text('one');
        final second = await builder.addPage(PdfPageSize.points(300, 400));
        await second.text('two');
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        expect(doc.pageCount, 2);
        for (final page in await doc.pageInfo()) {
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
      final page = await builder.addPage();
      await page.text('Before');
      await page.newline();
      await page.text('After');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains('Before'));
      expect(text, contains('After'));
      await doc.dispose();
    }, timeout: t(1));

    test('setMetadata carries the subject and the keywords', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      await builder.setMetadata(
        const PdfMetadata(subject: 'Test Subject', keywords: 'a, b, c'),
      );
      final page = await builder.addPage();
      await page.text('x');
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = String.fromCharCodes(sink.takeBytes());
      expect(output, contains('Test Subject'));
    }, timeout: t(1));

    test(
      'setMetadata carries the creator, the producer and the dates',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        await builder.setMetadata(
          PdfMetadata(
            creator: 'pdf_manipulator',
            producer: 'pdf_manipulator',
            creationDate: DateTime.utc(2024, 1, 15, 12, 30, 45),
            modificationDate: DateTime.utc(2024, 2, 1),
          ),
        );
        final page = await builder.addPage();
        await page.text('x');
        final sink = TestSink();
        await builder.save(sink);
        await builder.dispose();
        final output = String.fromCharCodes(sink.takeBytes());
        expect(output, contains('/Creator'));
        expect(output, contains('/Producer'));
        expect(output, contains('D:20240115123045Z'));
        expect(output, contains('D:20240201000000Z'));
      },
      timeout: t(1),
    );

    test('pushButton embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.pushButton(
        'mybtn',
        const PdfRect(x: 50, y: 700, width: 80, height: 30),
        'Click',
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mybtn'));
    }, timeout: t(1));

    test('signatureField embeds field name', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.signatureField(
        'mysig',
        const PdfRect(x: 50, y: 700, width: 200, height: 60),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      expect(String.fromCharCodes(sink.takeBytes()), contains('mysig'));
    }, timeout: t(1));

    test('a keystroke script embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
        scripts: PdfFieldScripts(
          keystroke: 'AFNumber_Keystroke(2, 0, 0, 0, "", true)',
        ),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFNumber_Keystroke'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a format script embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
        scripts: PdfFieldScripts(
          format: 'AFNumber_Format(2, 0, 0, 0, "", true)',
        ),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFNumber_Format'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a validate script embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
        scripts: PdfFieldScripts(
          validate: 'AFRange_Validate(true, 0, true, 100)',
        ),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFRange_Validate'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a calculate script embeds the action script', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      await page.textField(
        'actionfield',
        const PdfRect(x: 50, y: 700, width: 200, height: 20),
        scripts: PdfFieldScripts(
          calculate: 'AFSimple_Calculate("SUM", new Array("f1", "f2"))',
        ),
      );
      final sink = TestSink();
      await builder.save(sink);
      await builder.dispose();
      final output = sink.takeBytes();
      expect(
        String.fromCharCodes(output),
        contains('AFSimple_Calculate'),
        reason: 'the action script must land on the field',
      );
      final doc = await pdf.open(PdfInput(src(output)));
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a page link produces valid output', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final p1 = await builder.addPage();
      await p1.text('Page 1', link: const PdfLink.page(1));
      final p2 = await builder.addPage();
      await p2.text('Page 2');
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
      final doc = await pdf.open(PdfInput(src(output)));
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
  });

  // One test per page-op method: the effect is proven in the saved
  // file — extracted text, the annotation list, or the uncompressed
  // content stream's own operators.
  group('page op parity', () {
    Future<Uint8List> savePlain(
      Pdf pdf,
      Future<void> Function(PdfPageBuilder page) content, {
      bool deflate = true,
    }) async {
      final builder = await pdf.build();
      final page = await builder.addPage();
      await content(page);
      final sink = TestSink();
      await builder.save(sink, deflate: deflate);
      await builder.dispose();
      return sink.takeBytes();
    }

    Future<PdfDocumentHandle> openOf(Pdf pdf, Uint8List bytes) =>
        pdf.open(PdfInput(src(bytes)));

    test('appendText writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.moveTo(const PdfPoint(72, 720));
        await page.appendText('Hello Inline');
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Hello Inline'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('appendText writes bold text in a styled run', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.moveTo(const PdfPoint(72, 720));
        await page.appendText(
          'Hello Bold',
          style: const PdfTextStyle(bold: true),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Hello Bold'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('appendText writes italic text in a styled run', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.moveTo(const PdfPoint(72, 720));
        await page.appendText(
          'Hello Italic',
          style: const PdfTextStyle(italic: true),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Hello Italic'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('appendText writes coloured text in a styled run', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.moveTo(const PdfPoint(72, 720));
        await page.appendText(
          'Hello Colour',
          style: const PdfTextStyle(color: PdfColor.red),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Hello Colour'),
      );
      await doc.dispose();
    }, timeout: t(1));

    test('text writes a styled run in the style\'s font and size', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Styled Text',
          style: const PdfTextStyle(font: 'Courier', size: 24),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
        contains('Styled Text'),
      );
      await doc.dispose();
      expect(
        String.fromCharCodes(bytes),
        contains('Courier'),
        reason: 'the run must use the font the style names',
      );
    }, timeout: t(1));

    test('textIn writes text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.textIn(
          const PdfRect(x: 50, y: 600, width: 300, height: 100),
          'Rect caption',
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        await extractTextToString(doc, pages: const PdfPages.all()),
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
            PdfTableColumn('Qty', width: 60, align: PdfAlignment.right),
          ],
          const [
            [PdfTableCell('A-1'), PdfTableCell('12')],
            [PdfTableCell('B-2'), PdfTableCell('3')],
          ],
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains('SKU'));
      expect(text, contains('A-1'));
      expect(text, contains('B-2'));
      expect(text, contains('12'));
      await doc.dispose();
    }, timeout: t(1));

    test('table refuses a row whose length differs from the columns', () async {
      final pdf = createPdf();
      final builder = await pdf.build();
      final page = await builder.addPage();
      expect(
        () => page.table(
          const [PdfTableColumn('SKU', width: 100)],
          const [
            [PdfTableCell('1')],
            [PdfTableCell('2'), PdfTableCell('3')],
          ],
        ),
        throwsA(isA<ArgumentError>()),
        reason: 'the second row has 2 cells but the table has 1 column',
      );
      await builder.dispose();
    }, timeout: t(1));

    test('table writes its cells', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.table(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60, align: PdfAlignment.center),
          ],
          const [
            [PdfTableCell('Saw'), PdfTableCell('3')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains('Item'));
      expect(text, contains('Saw'));
      expect(text, contains('Hammer'));
      expect(text, contains('7'));
      await doc.dispose();
    }, timeout: t(1));

    test('table draws a cell that spans two rows', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.table(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60, align: PdfAlignment.center),
          ],
          const [
            [PdfTableCell('Saw', rowSpan: 2), PdfTableCell('3')],
            [PdfTableCell(''), PdfTableCell('4')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
          options: const PdfTableOptions(maxRowSpan: 2),
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      for (final expected in ['Saw', '3', '4', 'Hammer', '7']) {
        expect(text, contains(expected));
      }
      await doc.dispose();
    }, timeout: t(1));

    test('table refuses a row without one cell per column', () async {
      final builder = await createPdf().build();
      final page = await builder.addPage();
      expect(
        () => page.table(
          const [
            PdfTableColumn('Item', width: 120),
            PdfTableColumn('Count', width: 60),
          ],
          const [
            [PdfTableCell('Saw')],
          ],
        ),
        throwsA(isA<ArgumentError>()),
      );
      await builder.dispose();
    }, timeout: t(1));

    test('table refuses a rowSpan above the options\' max row span', () async {
      final builder = await createPdf().build();
      final page = await builder.addPage();
      expect(
        () => page.table(
          const [PdfTableColumn('Item', width: 120)],
          const [
            [PdfTableCell('Saw', rowSpan: 2)],
          ],
        ),
        throwsA(isA<ArgumentError>()),
      );
      await builder.dispose();
    }, timeout: t(1));

    test(
      'table refuses a column without a width in the fixed sizing',
      () async {
        final pdf = createPdf();
        final builder = await pdf.build();
        final page = await builder.addPage();
        expect(
          () => page.table(
            const [PdfTableColumn('Item')],
            const [
              [PdfTableCell('Saw')],
            ],
          ),
          throwsA(
            isA<ArgumentError>().having(
              (e) => e.message,
              'message',
              contains('PdfTableSizing.sample'),
            ),
          ),
          reason:
              'the fixed sizing needs a width on every column, and the '
              'message names the sizings that measure instead',
        );
        await builder.dispose();
      },
      timeout: t(1),
    );

    test('table measures its columns in the sample sizing', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.table(
          const [PdfTableColumn('Item'), PdfTableColumn('Count')],
          const [
            [PdfTableCell('Saw'), PdfTableCell('3')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
          options: const PdfTableOptions(sizing: PdfTableSizing.sample),
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains('Item'));
      expect(text, contains('Saw'));
      expect(text, contains('Hammer'));
      expect(text, contains('7'));
      await doc.dispose();
    }, timeout: t(1));

    test('table measures every row in the autoAll sizing', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.table(
          const [PdfTableColumn('Item'), PdfTableColumn('Count')],
          const [
            [PdfTableCell('Saw'), PdfTableCell('3')],
            [PdfTableCell('Hammer'), PdfTableCell('7')],
          ],
          options: const PdfTableOptions(sizing: PdfTableSizing.autoAll),
        );
      });
      final doc = await openOf(pdf, bytes);
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      expect(text, contains('Item'));
      expect(text, contains('Saw'));
      expect(text, contains('Hammer'));
      expect(text, contains('7'));
      await doc.dispose();
    }, timeout: t(1));

    test('markup highlights the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Marked sentence',
          markup: const PdfMarkup(PdfMarkupKind.highlight),
        );
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations, hasLength(1));
      expect(annotations.single.subtype, PdfAnnotationSubtype.highlight);
      await doc.dispose();
    }, timeout: t(1));

    test('markup underlines the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Underlined sentence',
          markup: const PdfMarkup(PdfMarkupKind.underline),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.underline,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('markup strikes the text out', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Struck sentence',
          markup: const PdfMarkup(PdfMarkupKind.strikeout),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.strikeOut,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('markup squiggles under the text', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Misspelled word',
          markup: const PdfMarkup(PdfMarkupKind.squiggly),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.squiggly,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('markup takes the colour it is given', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Coloured sentence',
          markup: const PdfMarkup(
            PdfMarkupKind.highlight,
            color: PdfColor(0, 1, 0),
          ),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.highlight,
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
        await page.stickyNote('Corner note', at: const PdfPoint(400, 500));
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(annotations.single.subtype, PdfAnnotationSubtype.text);
      expect(annotations.single.contents, 'Corner note');
      await doc.dispose();
    }, timeout: t(1));

    test('stamp lands a stamp annotation', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.moveTo(const PdfPoint(72, 600));
        await page.stamp(PdfStamp.approved);
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

    test('a named link lands a link annotation', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Jump to section', link: const PdfLink.named('sec-2'));
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.link,
      );
      await doc.dispose();
    }, timeout: t(1));

    test('a JavaScript link lands a JavaScript link', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text(
          'Run script',
          link: const PdfLink.javaScript('app.alert("hello")'),
        );
      });
      final doc = await openOf(pdf, bytes);
      expect(
        (await doc.annotations(0)).single.subtype,
        PdfAnnotationSubtype.link,
      );
      expect(String.fromCharCodes(bytes), contains('app.alert'));
      await doc.dispose();
    }, timeout: t(1));

    test('a confidential stamp lands a stamp annotation', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.text('Body');
        await page.stamp(PdfStamp.confidential);
      });
      final doc = await openOf(pdf, bytes);
      final annotations = await doc.annotations(0);
      expect(
        annotations.map((a) => a.subtype),
        contains(PdfAnnotationSubtype.stamp),
        reason: 'the confidential stamp is the preset the old watermark used',
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
      'an image without alt text places an XObject and the page renders',
      () async {
        final pdf = createPdf();
        final bytes = await savePlain(pdf, (page) async {
          await page.text('Body');
          await page.image(
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
          size: const PdfRenderSize.fit(maxWidth: 240, maxHeight: 240),
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
      }, deflate: false);
      expect(
        String.fromCharCodes(bytes),
        matches(RegExp(r'\b50(\.0+)? 300(\.0+)? 100(\.0+)? 60(\.0+)? re\b')),
      );
    }, timeout: t(1));

    test('a filled rect emits the fill colour rg', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(
          const PdfRect(x: 50, y: 300, width: 100, height: 60),
          fill: PdfColor.red,
        );
      }, deflate: false);
      final asString = String.fromCharCodes(bytes);
      expect(asString, matches(RegExp(r'1(\.0+)? 0(\.0+)? 0(\.0+)? rg')));
      expect(asString, matches(RegExp(r'\bre\b')));
    }, timeout: t(1));

    test('a rect with the default stroke draws an outline', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(const PdfRect(x: 50, y: 300, width: 100, height: 60));
      }, deflate: false);
      expect(
        String.fromCharCodes(bytes),
        matches(RegExp(r'\bRG\b')),
        reason: 'the default stroke sets a stroke colour for the outline',
      );
    }, timeout: t(1));

    test('a rect with stroke: null and a fill draws the fill only', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(
          const PdfRect(x: 50, y: 300, width: 100, height: 60),
          fill: PdfColor.red,
          stroke: null,
        );
      }, deflate: false);
      final asString = String.fromCharCodes(bytes);
      expect(asString, matches(RegExp(r'1(\.0+)? 0(\.0+)? 0(\.0+)? rg')));
      expect(asString, matches(RegExp(r'\bre\b')));
      expect(
        asString,
        isNot(matches(RegExp(r'\bRG\b'))),
        reason: 'no stroke means no stroke colour is set',
      );
    }, timeout: t(1));

    test('a rect with no fill and stroke: null is accepted', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(
          const PdfRect(x: 50, y: 300, width: 100, height: 60),
          stroke: null,
        );
      }, deflate: false);
      final doc = await openOf(pdf, bytes);
      expect(doc.pageCount, 1);
      await doc.dispose();
    }, timeout: t(1));

    test('a rect with a fill and a stroke draws both', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.rect(
          const PdfRect(x: 50, y: 300, width: 100, height: 60),
          fill: PdfColor.red,
          stroke: const PdfStroke(color: PdfColor(0, 0, 1), width: 2),
        );
      }, deflate: false);
      final asString = String.fromCharCodes(bytes);
      expect(asString, matches(RegExp(r'1(\.0+)? 0(\.0+)? 0(\.0+)? rg')));
      expect(asString, matches(RegExp(r'0(\.0+)? 0(\.0+)? 1(\.0+)? RG')));
      expect(asString, matches(RegExp(r'2(\.0+)? w\b')));
    }, timeout: t(1));

    test('line emits the l operator', () async {
      final pdf = createPdf();
      final bytes = await savePlain(pdf, (page) async {
        await page.line(const PdfPoint(50, 300), const PdfPoint(150, 360));
      }, deflate: false);
      expect(String.fromCharCodes(bytes), matches(RegExp(r'\bl\s')));
    }, timeout: t(1));

    test(
      'a plain stroke dashes nothing; a dashed stroke sets the dash',
      () async {
        final pdf = createPdf();
        final solid = await savePlain(pdf, (page) async {
          await page.rect(
            const PdfRect(x: 50, y: 300, width: 100, height: 60),
            stroke: const PdfStroke(),
          );
        }, deflate: false);
        expect(String.fromCharCodes(solid), matches(RegExp(r'\bre\b')));
        expect(String.fromCharCodes(solid), isNot(contains('] 0 d')));

        final dashed = await savePlain(pdf, (page) async {
          await page.rect(
            const PdfRect(x: 50, y: 300, width: 100, height: 60),
            stroke: const PdfStroke(dash: [3, 2], dashPhase: 0),
          );
        }, deflate: false);
        final asString = String.fromCharCodes(dashed);
        expect(
          asString,
          matches(RegExp(r'\[3(\.0+)? 2(\.0+)?\] 0(\.0+)? d\b')),
        );
      },
      timeout: t(1),
    );

    test('a plain line dashes nothing; a dashed line sets the dash', () async {
      final pdf = createPdf();
      final solid = await savePlain(pdf, (page) async {
        await page.line(const PdfPoint(50, 300), const PdfPoint(150, 360));
      }, deflate: false);
      expect(String.fromCharCodes(solid), matches(RegExp(r'\bl\s')));
      expect(String.fromCharCodes(solid), isNot(contains('] 0 d')));

      final dashed = await savePlain(pdf, (page) async {
        await page.line(
          const PdfPoint(50, 300),
          const PdfPoint(150, 360),
          stroke: const PdfStroke(dash: [3, 2]),
        );
      }, deflate: false);
      expect(
        String.fromCharCodes(dashed),
        matches(RegExp(r'\[3(\.0+)? 2(\.0+)?\] 0(\.0+)? d\b')),
      );
    }, timeout: t(1));
  });
}
