// CHARTER — this battery alone proves: the form surface beyond filling a
// value. Reading fields and their properties from a document
// (`PdfDoc.formFields`), changing a field's properties in an edit session
// (tooltip, bounds, max length, alignment, colours, appearance, flags,
// removal) and reading the change back after save, and exporting the
// filled values as FDF / XFDF. Filling and flattening values, and the
// text-encoding edge cases, belong to the editor and form-encoding
// batteries.
//
// Diet: dart-pdf form fixtures (`form_fields`, `form_fields_props`) and
// the handwritten AcroForm micro fixtures. Every property change is
// proven by reading it back through `PdfDoc.formFields` after save, and
// visual ones by rendering.

import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerFormTests(Pdf Function() createPdf) {
  group('form', () {
    test('the form fixture opens with its declared page count', () async {
      final doc = await createPdf().open(src(fFormFields));
      expect(doc.pageCount, fFormFieldsTruth.pages);
      await doc.dispose();
    }, timeout: t(1));

    // ── Reads ──

    test('formFields exposes names and the text field default value', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(fFormFields));
      final fields = await doc.formFields;
      expect(
        fields.map((f) => f.name).toSet(),
        fFormFieldsTruth.fieldNames.toSet(),
      );
      final fullname = fields.firstWhere((f) => f.name == 'fullname');
      expect(fullname.type, PdfFormFieldType.text);
      final value = fullname.value;
      expect(value, isA<PdfTextValue>());
      expect((value as PdfTextValue).text, fFormFieldsTruth.textFieldDefault);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'formFields exposes maxLength and tooltip on the props fixture',
      () async {
        final pdf = createPdf();
        final doc = await pdf.open(src(fFormFieldsProps));
        final city = await doc.formField('city');
        expect(city, isNotNull);
        expect(city!.maxLength, fFormFieldsPropsTruth.cityMaxLength);
        expect(city.tooltip, fFormFieldsPropsTruth.cityTooltip);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('a checkbox field is typed checkbox', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(fFormFieldsProps));
      final ok = await doc.formField('ok');
      expect(ok, isNotNull);
      expect(ok!.type, PdfFormFieldType.checkbox);
      await doc.dispose();
    }, timeout: t(1));

    test('formField returns null for a name that does not exist', () async {
      final pdf = createPdf();
      final doc = await pdf.open(src(fFormFieldsProps));
      expect(await doc.formField('nope'), isNull);
      await doc.dispose();
    }, timeout: t(1));

    // ── Mutations — property setters read back after save ──

    test('setFormFieldTooltip roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.setFormFieldTooltip('city', 'Updated tooltip');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final city = await doc.formField('city');
      expect(city!.tooltip, 'Updated tooltip');
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldBounds roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      const bounds = PdfRect(x: 10, y: 20, width: 100, height: 30);
      await editor.setFormFieldBounds('city', bounds);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final city = await doc.formField('city');
      expect(city!.bounds!.x, closeTo(bounds.x, 0.01));
      expect(city.bounds!.y, closeTo(bounds.y, 0.01));
      expect(city.bounds!.width, closeTo(bounds.width, 0.01));
      expect(city.bounds!.height, closeTo(bounds.height, 0.01));
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldMaxLength roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.setFormFieldMaxLength('city', 5);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final city = await doc.formField('city');
      expect(
        city!.maxLength,
        5,
        reason: 'the new /MaxLen must be read back, not the fixture default',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldAlignment roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.setFormFieldAlignment('city', PdfTextAlignment.center);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final city = await doc.formField('city');
      expect(city!.alignment, PdfTextAlignment.center);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'setFormFieldReadOnly and setFormFieldRequired roundtrip through save',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fFormFieldsProps));
        await editor.setFormFieldReadOnly('city', true);
        await editor.setFormFieldRequired('city', true);
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final doc = await pdf.open(src(sink.takeBytes()));
        final city = await doc.formField('city');
        expect(city!.readOnly, isTrue);
        expect(city.required, isTrue);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('setFormFieldFlags sets readOnly and required together', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.setFormFieldFlags('city', {
        PdfFormFieldFlag.readOnly,
        PdfFormFieldFlag.required,
      });
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      final city = await doc.formField('city');
      expect(city!.readOnly, isTrue);
      expect(city.required, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'setFormFieldFlags(multiline) leaves the field listed and flattenable',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fFormFieldsProps));
        await editor.setFormFieldFlags('notes', {PdfFormFieldFlag.multiline});
        final sink1 = TestSink();
        await editor.save(sink1);
        await editor.dispose();
        final saved = sink1.takeBytes();

        final check = await pdf.open(src(saved));
        expect(
          await check.formField('notes'),
          isNotNull,
          reason: 'a raw flag bit must not drop the field from the list',
        );
        await check.dispose();

        final editor2 = await pdf.edit(src(saved));
        await editor2.flattenForms();
        final sink2 = TestSink();
        await editor2.save(sink2);
        await editor2.dispose();
        final flatDoc = await pdf.open(src(sink2.takeBytes()));
        expect(flatDoc.pageCount, fFormFieldsPropsTruth.pages);
        await flatDoc.dispose();
      },
      timeout: t(2),
    );

    test('removeFormField removes the field and leaves the others', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.removeFormField('city');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(src(sink.takeBytes()));
      expect(
        await doc.formField('city'),
        isNull,
        reason: 'a removed field must not be listed',
      );
      expect(await doc.formField('ok'), isNotNull);
      expect(await doc.formField('notes'), isNotNull);
      await doc.dispose();
    }, timeout: t(1));

    test('background, border and appearance still fill and flatten, and change '
        'the rendered page', () async {
      final pdf = createPdf();
      final baseline = await _renderFirstPage(pdf, fFormFieldsProps);

      final editor = await pdf.edit(src(fFormFieldsProps));
      await editor.setFormFieldBackgroundColor(
        'notes',
        const PdfColor(0.9, 0.9, 0.2),
      );
      await editor.setFormFieldBorderColor('notes', const PdfColor(1, 0, 0));
      await editor.setFormFieldBorderWidth('notes', 2);
      await editor.setFormFieldAppearance(
        'notes',
        font: 'Helv',
        fontSize: 14,
        color: const PdfColor(0, 0, 1),
      );
      await editor.setFormFieldValue('notes', 'Styled');
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flattened = sink.takeBytes();

      final doc = await pdf.open(src(flattened));
      final text = await doc.extract(pages: const PdfPages.all());
      expect(text, contains('Styled'));
      await doc.dispose();

      final styled = await _renderFirstPage(pdf, flattened);
      expect(
        _psnr(baseline, styled).isFinite,
        isTrue,
        reason: 'styling + a filled value must change the rendered page',
      );
    }, timeout: t(2));

    // ── Export ──

    test(
      'exportFormData(xfdf) exports the filled field as a parsed pair',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fFormFields));
        await editor.setFormFieldValue('fullname', 'Ada');
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();

        final doc = await pdf.open(src(sink.takeBytes()));
        final xfdfSink = TestSink();
        await doc.exportFormData(xfdfSink, format: PdfFormDataFormat.xfdf);
        await doc.dispose();

        final pairs = _parseXfdfFields(utf8.decode(xfdfSink.takeBytes()));
        expect(pairs['fullname'], 'Ada');
      },
      timeout: t(1),
    );

    test(
      'exportFormData(fdf) exports the filled field as a parsed pair',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(src(fFormFields));
        await editor.setFormFieldValue('fullname', 'Ada');
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();

        final doc = await pdf.open(src(sink.takeBytes()));
        final fdfSink = TestSink();
        await doc.exportFormData(fdfSink, format: PdfFormDataFormat.fdf);
        await doc.dispose();

        final pairs = _parseFdfFields(latin1.decode(fdfSink.takeBytes()));
        expect(pairs['fullname'], 'Ada');
      },
      timeout: t(1),
    );
  });
}

/// Renders [bytes]' first page at thumbnail size and decodes it, for the
/// form styling test that must prove pixels changed after save + reopen.
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

/// Parses `<field name="…"><value>…</value></field>` pairs out of an XFDF
/// document — a tolerant hand parser (no `xml` dev dependency in
/// `pubspec.yaml`), never a byte-grep of the raw export.
Map<String, String> _parseXfdfFields(String xml) {
  final pairs = <String, String>{};
  final re = RegExp(
    r'<field name="([^"]*)">\s*<value>([^<]*)</value>',
    dotAll: true,
  );
  for (final m in re.allMatches(xml)) {
    pairs[m.group(1)!] = m.group(2)!;
  }
  return pairs;
}

/// Parses `/T (name) /V (value)` pairs out of the decoded FDF object
/// stream — a structural parse of the PDF dictionary syntax, never a
/// byte-grep of the raw export.
Map<String, String> _parseFdfFields(String fdf) {
  final pairs = <String, String>{};
  final re = RegExp(r'/T \(([^()]*)\)\s*/V \(([^()]*)\)');
  for (final m in re.allMatches(fdf)) {
    pairs[m.group(1)!] = m.group(2)!;
  }
  return pairs;
}
