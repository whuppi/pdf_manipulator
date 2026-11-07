// Result and option types compare by value: equal fields, equal hash.

import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void expectValue<T>(T a, T same, T other) {
  expect(a, equals(same));
  expect(a.hashCode, equals(same.hashCode));
  expect(a, isNot(equals(other)));
}

void main() {
  group('value equality', () {
    test('PdfColor', () {
      expectValue(
        const PdfColor(1, 0, 0),
        const PdfColor(1, 0, 0),
        const PdfColor(0, 1, 0),
      );
      expect(PdfColor.red, equals(const PdfColor(1, 0, 0)));
    });

    test('PdfPermissions compares by flags', () {
      expectValue(
        const PdfPermissions(canPrint: false),
        const PdfPermissions(canPrint: false),
        const PdfPermissions.all(),
      );
      expect(
        const PdfPermissions.all(),
        equals(const PdfPermissions(canCopy: true)),
      );
    });

    test('PdfSave', () {
      expectValue<PdfSave>(
        const PdfSave.full(deflate: PdfDeflate.none),
        const PdfSave.full(deflate: PdfDeflate.none),
        const PdfSave.full(),
      );
      expectValue<PdfSave>(
        const PdfSave.incremental(),
        const PdfSave.incremental(),
        const PdfSave.full(),
      );
    });

    test('PdfEncryption', () {
      expectValue<PdfEncryption>(
        const PdfEncryption.password(ownerPassword: 'a'),
        const PdfEncryption.password(ownerPassword: 'a'),
        const PdfEncryption.password(ownerPassword: 'b'),
      );
      expectValue<PdfEncryption>(
        const PdfEncryption.keep(),
        const PdfEncryption.keep(),
        const PdfEncryption.remove(),
      );
    });

    test('PdfPages', () {
      expectValue<PdfPages>(
        PdfPages.list([1, 2]),
        PdfPages.list([2, 1]),
        PdfPages.list([1, 3]),
      );
      expectValue<PdfPages>(
        const PdfPages.range(0, 3),
        const PdfPages.range(0, 3),
        const PdfPages.single(0),
      );
      expectValue<PdfPages>(
        const PdfPages.all(),
        const PdfPages.all(),
        const PdfPages.single(0),
      );
    });

    test('PdfWatermarkPosition and PdfWatermarkStyle', () {
      expectValue<PdfWatermarkPosition>(
        const PdfWatermarkPosition.corner(PdfCorner.topLeft),
        const PdfWatermarkPosition.corner(PdfCorner.topLeft),
        const PdfWatermarkPosition.corner(PdfCorner.topRight),
      );
      expectValue(
        const PdfWatermarkStyle(angleDegrees: 30),
        const PdfWatermarkStyle(angleDegrees: 30),
        const PdfWatermarkStyle(),
      );
    });

    test('PdfSigningCredentials compares its bundle source and password', () {
      final bundle = MemorySource(Uint8List.fromList([1, 2, 3]));
      expectValue<PdfSigningCredentials>(
        PdfSigningCredentials.pkcs12(bundle, 'pw'),
        PdfSigningCredentials.pkcs12(bundle, 'pw'),
        PdfSigningCredentials.pkcs12(bundle, 'other'),
      );
      expectValue<PdfSigningCredentials>(
        const PdfSigningCredentials.pem('cert', 'key'),
        const PdfSigningCredentials.pem('cert', 'key'),
        const PdfSigningCredentials.pem('cert', 'other'),
      );
    });

    test('PdfImageReport compares its rows', () {
      expectValue(
        const PdfImageReport([]),
        const PdfImageReport([]),
        const PdfImageReport([
          PdfImageOutcome(
            objectId: 1,
            encoding: PdfImageEncoding.raw,
            color: PdfImageColor.rgb,
            isIndexed: false,
            bitsPerComponent: 8,
            width: 2,
            height: 2,
            hasSoftMask: false,
            placementCount: 1,
            minPpi: null,
            action: PdfImageAction.kept,
            keepReason: null,
            bytesBefore: 12,
            bytesAfter: 12,
            widthAfter: 2,
            heightAfter: 2,
          ),
        ]),
      );
    });

    test('PdfFormFieldValue', () {
      expectValue<PdfFormFieldValue>(
        const PdfMultiChoiceValue(['a', 'b']),
        const PdfMultiChoiceValue(['a', 'b']),
        const PdfMultiChoiceValue(['a']),
      );
      expectValue<PdfFormFieldValue>(
        const PdfTextValue('x'),
        const PdfTextValue('x'),
        const PdfNoValue(),
      );
    });

    test('the PdfFormFieldValue factories build the subclasses', () {
      expect(const PdfFormFieldValue.text('a'), const PdfTextValue('a'));
      expect(
        const PdfFormFieldValue.checked(true),
        const PdfCheckedValue(true),
      );
      expect(const PdfFormFieldValue.choice('a'), const PdfChoiceValue('a'));
      expect(
        const PdfFormFieldValue.choices(['a', 'b']),
        const PdfMultiChoiceValue(['a', 'b']),
      );
      expect(const PdfFormFieldValue.none(), const PdfNoValue());
    });

    test('PdfRedactionOverlay', () {
      expectValue<PdfRedactionOverlay>(
        const PdfRedactionOverlay.fill(PdfColor.black),
        const PdfRedactionOverlay.fill(PdfColor.black),
        const PdfRedactionOverlay.none(),
      );
      expectValue<PdfRedactionOverlay>(
        const PdfRedactionOverlay.fill(PdfColor.black),
        const PdfRedactionOverlay.fill(PdfColor(0, 0, 0)),
        const PdfRedactionOverlay.fill(PdfColor.red),
      );
    });

    test('PdfSearchHit', () {
      const rect = PdfRect(x: 1, y: 2, width: 3, height: 4);
      expectValue(
        const PdfSearchHit(text: 'a', page: 0, bounds: rect),
        const PdfSearchHit(text: 'a', page: 0, bounds: rect),
        const PdfSearchHit(text: 'a', page: 1, bounds: rect),
      );
    });

    test('PdfPoint', () {
      expectValue(
        const PdfPoint(1, 2),
        const PdfPoint(1, 2),
        const PdfPoint(2, 1),
      );
    });

    test('PdfLink variants', () {
      expectValue<PdfLink>(
        const PdfLink.url('https://a'),
        const PdfLink.url('https://a'),
        const PdfLink.url('https://b'),
      );
      expectValue<PdfLink>(
        const PdfLink.page(1),
        const PdfLink.page(1),
        const PdfLink.page(2),
      );
      expectValue<PdfLink>(
        const PdfLink.named('top'),
        const PdfLink.named('top'),
        const PdfLink.named('end'),
      );
      expectValue<PdfLink>(
        const PdfLink.javaScript('a()'),
        const PdfLink.javaScript('a()'),
        const PdfLink.javaScript('b()'),
      );
      expect(const PdfLink.page(1), isNot(equals(const PdfLink.named('1'))));
    });

    test('PdfMarkup', () {
      expectValue(
        const PdfMarkup(PdfMarkupKind.highlight, color: PdfColor.red),
        const PdfMarkup(PdfMarkupKind.highlight, color: PdfColor.red),
        const PdfMarkup(PdfMarkupKind.underline, color: PdfColor.red),
      );
      expect(
        const PdfMarkup(PdfMarkupKind.highlight),
        isNot(
          equals(const PdfMarkup(PdfMarkupKind.highlight, color: PdfColor.red)),
        ),
      );
    });

    test('PdfFieldScripts', () {
      expectValue(
        const PdfFieldScripts(keystroke: 'k', calculate: 'c'),
        const PdfFieldScripts(keystroke: 'k', calculate: 'c'),
        const PdfFieldScripts(keystroke: 'k', validate: 'c'),
      );
    });

    test('PdfRadioOption', () {
      const bounds = PdfRect(x: 1, y: 2, width: 3, height: 4);
      expectValue(
        const PdfRadioOption('a', bounds),
        const PdfRadioOption('a', bounds),
        const PdfRadioOption('b', bounds),
      );
    });

    test('PdfBitmap compares its pixels, not identity', () {
      expectValue(
        PdfBitmap(page: 0, width: 1, height: 1, bytes: Uint8List.fromList([1])),
        PdfBitmap(page: 0, width: 1, height: 1, bytes: Uint8List.fromList([1])),
        PdfBitmap(page: 0, width: 1, height: 1, bytes: Uint8List.fromList([2])),
      );
    });

    test('PdfInput', () {
      final source = MemorySource(Uint8List.fromList([1]));
      expectValue(
        PdfInput(source, password: 'a'),
        PdfInput(source, password: 'a'),
        PdfInput(source, password: 'b'),
      );
      expect(
        PdfInput(source),
        isNot(equals(PdfInput(MemorySource(Uint8List(1))))),
      );
    });

    test('PdfPages.from and the other selections differ by kind', () {
      expectValue<PdfPages>(
        const PdfPages.from(2),
        const PdfPages.from(2),
        const PdfPages.from(3),
      );
      expect(const PdfPages.from(0), isNot(equals(const PdfPages.all())));
    });

    test('PdfPageOrder compares its order', () {
      expectValue(
        PdfPageOrder([2, 0]),
        PdfPageOrder([2, 0]),
        PdfPageOrder([0, 2]),
      );
    });

    test('the PdfSave variants differ from each other', () {
      expectValue<PdfSave>(
        const PdfSave.incrementalOrRewrite(deflate: PdfDeflate.fast),
        const PdfSave.incrementalOrRewrite(deflate: PdfDeflate.fast),
        const PdfSave.incrementalOrRewrite(),
      );
      expect(
        const PdfSave.incrementalOrRewrite(),
        isNot(equals(const PdfSave.full())),
      );
      expectValue<PdfSave>(
        const PdfSave.full(useObjectStreams: false, mergeDuplicates: true),
        const PdfSave.full(useObjectStreams: false, mergeDuplicates: true),
        const PdfSave.full(useObjectStreams: false),
      );
    });

    test('PdfDeflate crosses the wire by name', () {
      expect(PdfDeflate.values.map((d) => d.wire), [
        'none',
        'fast',
        'balanced',
        'best',
      ]);
    });

    test('PdfRotation, PdfALevel and PdfUaLevel cross the wire by name', () {
      expect(PdfRotation.values.map((r) => r.wire), [
        'r0',
        'r90',
        'r180',
        'r270',
      ]);
      expect(PdfALevel.values.map((l) => l.wire), ['a1', 'a2', 'a3']);
      expect(PdfUaLevel.values.map((l) => l.wire), ['ua1', 'ua2']);
    });

    test('PdfPageSize compares by its dimensions', () {
      expectValue<PdfPageSize>(
        const PdfPageSize.a4(),
        const PdfPageSize.a4(),
        const PdfPageSize.letter(),
      );
      expectValue<PdfPageSize>(
        const PdfPageSize.points(200, 300),
        const PdfPageSize.points(200, 300),
        const PdfPageSize.points(300, 200),
      );
    });

    test('PdfTextStyle compares by its fields', () {
      expectValue<PdfTextStyle>(
        const PdfTextStyle(bold: true),
        const PdfTextStyle(bold: true),
        const PdfTextStyle(italic: true),
      );
    });

    test('PdfStroke compares by its dash pattern', () {
      expectValue<PdfStroke>(
        const PdfStroke(dash: [3, 2]),
        const PdfStroke(dash: [3, 2]),
        const PdfStroke(dash: [2, 3]),
      );
      expectValue<PdfStroke>(
        const PdfStroke(),
        const PdfStroke(),
        const PdfStroke(width: 2),
      );
    });

    test('PdfMetadata compares by its fields', () {
      expectValue<PdfMetadata>(
        const PdfMetadata(title: 'A', creator: 'C'),
        const PdfMetadata(title: 'A', creator: 'C'),
        const PdfMetadata(title: 'A', creator: 'D'),
      );
    });

    test('PdfTableOptions compares by its fields', () {
      expectValue<PdfTableOptions>(
        const PdfTableOptions(maxRowSpan: 2),
        const PdfTableOptions(maxRowSpan: 2),
        const PdfTableOptions(),
      );
    });

    test('PdfImagePage compares by its fields', () {
      expectValue<PdfImagePage>(
        const PdfImagePage(fit: PdfImageFit.cover),
        const PdfImagePage(fit: PdfImageFit.cover),
        const PdfImagePage(),
      );
    });

    test('PdfSplitPart compares by every field', () {
      expectValue(
        const PdfSplitPart(index: 1, start: 2, end: 4, title: 'B'),
        const PdfSplitPart(index: 1, start: 2, end: 4, title: 'B'),
        const PdfSplitPart(index: 1, start: 2, end: 4),
      );
    });

    test('PdfSplitRule compares by its rule and its number', () {
      expectValue<PdfSplitRule>(
        const PdfSplitRule.every(5),
        const PdfSplitRule.every(5),
        const PdfSplitRule.every(6),
      );
      expectValue<PdfSplitRule>(
        const PdfSplitRule.maxBytes(5),
        const PdfSplitRule.maxBytes(5),
        const PdfSplitRule.every(5),
      );
      expectValue<PdfSplitRule>(
        const PdfSplitRule.bookmarks(),
        const PdfSplitRule.bookmarks(),
        const PdfSplitRule.maxBytes(5),
      );
    });

    test(
      'PdfAlignment, PdfMarkupKind and PdfImageFit cross the wire by name',
      () {
        expect(PdfAlignment.values.map((a) => a.wire), [
          'left',
          'center',
          'right',
        ]);
        expect(PdfMarkupKind.values.map((m) => m.wire), [
          'highlight',
          'underline',
          'strikeout',
          'squiggly',
        ]);
        expect(PdfImageFit.values.map((f) => f.wire), [
          'contain',
          'cover',
          'fill',
        ]);
      },
    );
  });
}
