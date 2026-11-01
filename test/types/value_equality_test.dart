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
        const PdfPermissions(print: false),
        const PdfPermissions(print: false),
        const PdfPermissions.all(),
      );
      expect(
        const PdfPermissions.all(),
        equals(const PdfPermissions(copy: true)),
      );
    });

    test('PdfSaveOptions', () {
      expectValue<PdfSaveOptions>(
        const PdfSaveOptions.fullRewrite(deflate: false),
        const PdfSaveOptions.fullRewrite(deflate: false),
        const PdfSaveOptions.fullRewrite(),
      );
      expectValue<PdfSaveOptions>(
        const PdfSaveOptions.incremental(),
        const PdfSaveOptions.incremental(),
        const PdfSaveOptions.fullRewrite(),
      );
    });

    test('PdfEncryption', () {
      expectValue<PdfEncryption>(
        const PdfEncryption.config(ownerPassword: 'a'),
        const PdfEncryption.config(ownerPassword: 'a'),
        const PdfEncryption.config(ownerPassword: 'b'),
      );
      expectValue<PdfEncryption>(
        const PdfEncryption.keep(),
        const PdfEncryption.keep(),
        const PdfEncryption.remove(),
      );
    });

    test('PdfPages', () {
      expectValue<PdfPages>(
        const PdfPages.list([1, 2]),
        const PdfPages.list([1, 2]),
        const PdfPages.list([2, 1]),
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
        const PdfWatermarkStyle(rotation: 30),
        const PdfWatermarkStyle(rotation: 30),
        const PdfWatermarkStyle(),
      );
    });

    test('PdfSigningCredentials compares bytes, not identity', () {
      expectValue<PdfSigningCredentials>(
        PdfSigningCredentials.pkcs12(Uint8List.fromList([1, 2, 3]), 'pw'),
        PdfSigningCredentials.pkcs12(Uint8List.fromList([1, 2, 3]), 'pw'),
        PdfSigningCredentials.pkcs12(Uint8List.fromList([1, 2, 4]), 'pw'),
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
            indexed: false,
            bits: 8,
            width: 2,
            height: 2,
            hasSoftMask: false,
            uses: 1,
            ppiMin: null,
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

    test('PdfSearchHit', () {
      const rect = PdfRect(x: 1, y: 2, width: 3, height: 4);
      expectValue(
        const PdfSearchHit(text: 'a', page: 0, rect: rect),
        const PdfSearchHit(text: 'a', page: 0, rect: rect),
        const PdfSearchHit(text: 'a', page: 1, rect: rect),
      );
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
  });
}
