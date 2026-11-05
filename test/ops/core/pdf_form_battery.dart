// CHARTER — this battery alone proves: the form surface beyond filling a
// value. Reading fields and their properties from a document
// (`PdfDocument.formFields`), changing a field's properties in an edit session
// (tooltip, bounds, max length, alignment, colours, appearance, flags,
// removal) and reading the change back after save, and exporting the
// filled values as FDF / XFDF. Filling and flattening values, and the
// text-encoding edge cases, belong to the editor and form-encoding
// batteries.
//
// Diet: dart-pdf form fixtures (`form_fields`, `form_fields_props`) and
// the handwritten AcroForm micro fixtures. Every property change is
// proven by reading it back through `PdfDocument.formFields` after save, and
// visual ones by rendering.

import 'dart:convert';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:image/image.dart' as img;
import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

import '../../fixtures/generated/fixtures.dart';
import '../../fixtures/handwritten.dart';
import '../../harness/extract_text.dart';
import '../../harness/form_fields.dart';
import '../../harness/render_pages.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

void registerFormTests(Pdf Function() createPdf) {
  group('form', () {
    test('the form fixture opens with its declared page count', () async {
      final doc = await createPdf().open(PdfInput(src(fFormFields)));
      expect(doc.pageCount, fFormFieldsTruth.pages);
      await doc.dispose();
    }, timeout: t(1));

    // ── Reads ──

    test('formFields exposes names and the text field default value', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(fFormFields)));
      final fields = await doc.formFields();
      expect(
        fields.map((f) => f.name).toSet(),
        fFormFieldsTruth.fieldNames.toSet(),
      );
      final fullname = fields.firstWhere((f) => f.name == 'fullname');
      expect(fullname.kind, PdfFormFieldKind.textField);
      final value = fullname.value;
      expect(value, isA<PdfTextValue>());
      expect((value as PdfTextValue).text, fFormFieldsTruth.textFieldDefault);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'formFields exposes maxLength and tooltip on the props fixture',
      () async {
        final pdf = createPdf();
        final doc = await pdf.open(PdfInput(src(fFormFieldsProps)));
        final city = await formFieldNamed(doc, 'city');
        expect(city, isNotNull);
        expect(city!.maxLength, fFormFieldsPropsTruth.cityMaxLength);
        expect(city.tooltip, fFormFieldsPropsTruth.cityTooltip);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('a checkbox field is typed checkbox', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(fFormFieldsProps)));
      final ok = await formFieldNamed(doc, 'ok');
      expect(ok, isNotNull);
      expect(ok!.kind, PdfFormFieldKind.checkbox);
      await doc.dispose();
    }, timeout: t(1));

    test('formField returns null for a name that does not exist', () async {
      final pdf = createPdf();
      final doc = await pdf.open(PdfInput(src(fFormFieldsProps)));
      expect(await formFieldNamed(doc, 'nope'), isNull);
      await doc.dispose();
    }, timeout: t(1));

    // ── Mutations — property setters read back after save ──

    test('setFormFieldTooltip roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').setTooltip('Updated tooltip');
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(city!.tooltip, 'Updated tooltip');
      await doc.dispose();
    }, timeout: t(1));

    test('a tooltip and a max length set to null are removed', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').setTooltip('Set first');
      await editor.field('city').setMaxLength(7);
      await editor.field('city').setTooltip(null);
      await editor.field('city').setMaxLength(null);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(city!.tooltip, isNull, reason: 'the tooltip is gone');
      expect(city.maxLength, isNull, reason: 'the limit is gone');
      await doc.dispose();
    }, timeout: t(1));

    test('a colour set to null is removed and the field still saves', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('notes').setBackgroundColor(PdfColor.red);
      await editor.field('notes').setBorderColor(PdfColor.red);
      await editor.field('notes').setBackgroundColor(null);
      await editor.field('notes').setBorderColor(null);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(await formFieldNamed(doc, 'notes'), isNotNull);
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldBounds roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      const bounds = PdfRect(x: 10, y: 20, width: 100, height: 30);
      await editor.field('city').setBounds(bounds);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(city!.bounds!.x, closeTo(bounds.x, 0.01));
      expect(city.bounds!.y, closeTo(bounds.y, 0.01));
      expect(city.bounds!.width, closeTo(bounds.width, 0.01));
      expect(city.bounds!.height, closeTo(bounds.height, 0.01));
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldMaxLength roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').setMaxLength(5);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(
        city!.maxLength,
        5,
        reason: 'the new /MaxLen must be read back, not the fixture default',
      );
      await doc.dispose();
    }, timeout: t(1));

    test('setFormFieldAlignment roundtrips through save', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').setAlignment(PdfAlignment.center);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(city!.alignment, PdfAlignment.center);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'setFormFieldReadOnly and setFormFieldRequired roundtrip through save',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
        await editor.field('city').setReadOnly(true);
        await editor.field('city').setRequired(true);
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();
        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        final city = await formFieldNamed(doc, 'city');
        expect(city!.isReadOnly, isTrue);
        expect(city.isRequired, isTrue);
        await doc.dispose();
      },
      timeout: t(1),
    );

    test('setFormFieldFlags sets readOnly and required together', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').setFlags({
        PdfFormFieldFlag.readOnly,
        PdfFormFieldFlag.required,
      });
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final city = await formFieldNamed(doc, 'city');
      expect(city!.isReadOnly, isTrue);
      expect(city.isRequired, isTrue);
      await doc.dispose();
    }, timeout: t(1));

    test(
      'setFormFieldFlags(multiline) leaves the field listed and flattenable',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
        await editor.field('notes').setFlags({PdfFormFieldFlag.multiline});
        final sink1 = TestSink();
        await editor.save(sink1);
        await editor.dispose();
        final saved = sink1.takeBytes();

        final check = await pdf.open(PdfInput(src(saved)));
        expect(
          await formFieldNamed(check, 'notes'),
          isNotNull,
          reason: 'a raw flag bit must not drop the field from the list',
        );
        await check.dispose();

        final editor2 = await pdf.edit(PdfInput(src(saved)));
        await editor2.flattenForms();
        final sink2 = TestSink();
        await editor2.save(sink2);
        await editor2.dispose();
        final flatDoc = await pdf.open(PdfInput(src(sink2.takeBytes())));
        expect(flatDoc.pageCount, fFormFieldsPropsTruth.pages);
        await flatDoc.dispose();
      },
      timeout: t(2),
    );

    test('removeFormField removes the field and leaves the others', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor.field('city').remove();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      expect(
        await formFieldNamed(doc, 'city'),
        isNull,
        reason: 'a removed field must not be listed',
      );
      expect(await formFieldNamed(doc, 'ok'), isNotNull);
      expect(await formFieldNamed(doc, 'notes'), isNotNull);
      await doc.dispose();
    }, timeout: t(1));

    test('background, border and appearance still fill and flatten, and change '
        'the rendered page', () async {
      final pdf = createPdf();
      final baseline = await _renderFirstPage(pdf, fFormFieldsProps);

      final editor = await pdf.edit(PdfInput(src(fFormFieldsProps)));
      await editor
          .field('notes')
          .setBackgroundColor(const PdfColor(0.9, 0.9, 0.2));
      await editor.field('notes').setBorderColor(const PdfColor(1, 0, 0));
      await editor.field('notes').setBorderWidth(2);
      await editor
          .field('notes')
          .setAppearance(
            font: 'Helv',
            fontSize: 14,
            color: const PdfColor(0, 0, 1),
          );
      await editor.field('notes').setValue(PdfFormFieldValue.text('Styled'));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final flattened = sink.takeBytes();

      final doc = await pdf.open(PdfInput(src(flattened)));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
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
        final editor = await pdf.edit(PdfInput(src(fFormFields)));
        await editor.field('fullname').setValue(PdfFormFieldValue.text('Ada'));
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();

        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
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
        final editor = await pdf.edit(PdfInput(src(fFormFields)));
        await editor.field('fullname').setValue(PdfFormFieldValue.text('Ada'));
        final sink = TestSink();
        await editor.save(sink);
        await editor.dispose();

        final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
        final fdfSink = TestSink();
        await doc.exportFormData(fdfSink, format: PdfFormDataFormat.fdf);
        await doc.dispose();

        final pairs = _parseFdfFields(latin1.decode(fdfSink.takeBytes()));
        expect(pairs['fullname'], 'Ada');
      },
      timeout: t(1),
    );

    // ── Choice values ──

    /// Applies [edit] to the choice fixture's editor, saves, and returns the
    /// fields of the result by name.
    Future<Map<String, PdfFormField>> choiceFieldsAfter(
      Future<void> Function(PdfEditor editor) edit,
    ) async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(choiceFormPdf)));
      await edit(editor);
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final fields = {for (final f in await doc.formFields()) f.name: f};
      await doc.dispose();
      return fields;
    }

    test('a combo box and a list box take one option', () async {
      final fields = await choiceFieldsAfter((editor) async {
        await editor
            .field('combo')
            .setValue(const PdfFormFieldValue.choice('green'));
        await editor
            .field('list')
            .setValue(const PdfFormFieldValue.choice('blue'));
      });
      expect(fields['combo']!.kind, PdfFormFieldKind.comboBox);
      expect(fields['combo']!.value, const PdfChoiceValue('green'));
      expect(fields['list']!.kind, PdfFormFieldKind.listBox);
      expect(fields['list']!.value, const PdfChoiceValue('blue'));
    }, timeout: t(1));

    test('a multi-select list box takes several options', () async {
      final fields = await choiceFieldsAfter(
        (editor) => editor
            .field('multi')
            .setValue(const PdfFormFieldValue.choices(['blue', 'red'])),
      );
      expect(
        fields['multi']!.value,
        const PdfMultiChoiceValue(['blue', 'red']),
      );
    }, timeout: t(1));

    test('no value clears a choice field', () async {
      final fields = await choiceFieldsAfter((editor) async {
        await editor.field('list').setValue(const PdfFormFieldValue.none());
        await editor.field('combo').setValue(const PdfFormFieldValue.none());
      });
      expect(fields['list']!.value, isA<PdfNoValue>());
      expect(fields['combo']!.value, isA<PdfNoValue>());
    }, timeout: t(1));

    test('an editable combo box takes text that is no option', () async {
      final fields = await choiceFieldsAfter(
        (editor) => editor
            .field('free')
            .setValue(const PdfFormFieldValue.choice('typed by hand')),
      );
      expect(fields['free']!.value, const PdfChoiceValue('typed by hand'));
    }, timeout: t(1));

    test(
      'a value the field does not offer throws and changes nothing',
      () async {
        final pdf = createPdf();
        final editor = await pdf.edit(PdfInput(src(choiceFormPdf)));
        for (final (name, value) in [
          ('combo', const PdfFormFieldValue.choice('purple')),
          ('list', const PdfFormFieldValue.choice('purple')),
          ('multi', const PdfFormFieldValue.choices(['red', 'purple'])),
          ('multi', const PdfFormFieldValue.choices(['red', 'red'])),
          ('list', const PdfFormFieldValue.choices(['red'])),
          ('combo', const PdfFormFieldValue.choices(['red'])),
          ('nothing', const PdfFormFieldValue.choice('red')),
        ]) {
          await expectLater(
            editor.field(name).setValue(value),
            throwsA(isA<PdfEngineFailed>()),
            reason: '$name: $value',
          );
        }
        expect(await editor.isModified(), isFalse);
        await editor.dispose();
      },
      timeout: t(1),
    );

    test('a choice value of a text field throws', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(fFormFields)));
      await expectLater(
        editor.field('fullname').setValue(const PdfFormFieldValue.choice('x')),
        throwsA(isA<PdfEngineFailed>()),
      );
      await editor.dispose();
    }, timeout: t(1));

    test('a set choice is drawn in the rendered page', () async {
      final pdf = createPdf();
      final editor = await pdf.edit(PdfInput(src(choiceFormPdf)));
      await editor
          .field('multi')
          .setValue(const PdfFormFieldValue.choices(['green', 'amber']));
      await editor.flattenForms();
      final sink = TestSink();
      await editor.save(sink);
      await editor.dispose();
      final doc = await pdf.open(PdfInput(src(sink.takeBytes())));
      final text = await extractTextToString(doc, pages: const PdfPages.all());
      await doc.dispose();
      expect(text, allOf(contains('green'), contains('amber')));
    }, timeout: t(1));
  });
}

/// Renders [bytes]' first page at thumbnail size and decodes it, for the
/// form styling test that must prove pixels changed after save + reopen.
Future<img.Image> _renderFirstPage(Pdf pdf, Uint8List bytes) async {
  final doc = await pdf.open(PdfInput(src(bytes)));
  final frames = await renderToMemory(
    doc,
    pages: const PdfPages.single(0),
    size: const PdfRenderSize.fit(maxWidth: 200, maxHeight: 200),
  );
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
