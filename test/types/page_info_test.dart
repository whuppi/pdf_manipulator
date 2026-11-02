// PdfPageInfo — boxes, rotation, effective width/height.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

PdfRect _box(double width, double height) =>
    PdfRect(x: 0, y: 0, width: width, height: height);

void main() {
  group('PdfPageInfo', () {
    test('width and height are the media box', () {
      final p = PdfPageInfo(
        index: 0,
        mediaBox: const PdfRect(x: 10, y: 20, width: 595, height: 842),
      );
      expect(p.width, equals(595));
      expect(p.height, equals(842));
    });

    test('the crop box is null when the page has none', () {
      final p = PdfPageInfo(index: 0, mediaBox: _box(100, 200));
      expect(p.cropBox, isNull);

      final cropped = PdfPageInfo(
        index: 0,
        mediaBox: _box(100, 200),
        cropBox: const PdfRect(x: 5, y: 5, width: 90, height: 190),
      );
      expect(
        cropped.cropBox,
        equals(const PdfRect(x: 5, y: 5, width: 90, height: 190)),
      );
    });

    test('rotation defaults to 0', () {
      final p = PdfPageInfo(index: 0, mediaBox: _box(100, 200));
      expect(p.rotation, equals(PdfRotation.r0));
    });

    test('effectiveWidth equals width at 0°', () {
      final p = PdfPageInfo(
        index: 0,
        mediaBox: _box(595, 842),
        rotation: PdfRotation.r0,
      );
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 90°', () {
      final p = PdfPageInfo(
        index: 0,
        mediaBox: _box(595, 842),
        rotation: PdfRotation.r90,
      );
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('effectiveWidth unchanged at 180°', () {
      final p = PdfPageInfo(
        index: 0,
        mediaBox: _box(595, 842),
        rotation: PdfRotation.r180,
      );
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 270°', () {
      final p = PdfPageInfo(
        index: 0,
        mediaBox: _box(595, 842),
        rotation: PdfRotation.r270,
      );
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('equal values are equal and hash alike', () {
      final a = PdfPageInfo(index: 1, mediaBox: _box(10, 20), label: 'i');
      final b = PdfPageInfo(index: 1, mediaBox: _box(10, 20), label: 'i');
      final c = PdfPageInfo(index: 1, mediaBox: _box(10, 20));
      final d = PdfPageInfo(
        index: 1,
        mediaBox: _box(10, 20),
        label: 'i',
        cropBox: _box(5, 5),
      );
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));
      expect(a, isNot(equals(c)));
      expect(a, isNot(equals(d)));
    });

    test('label is nullable', () {
      final p = PdfPageInfo(index: 0, mediaBox: _box(100, 200));
      expect(p.label, isNull);

      final p2 = PdfPageInfo(index: 0, mediaBox: _box(100, 200), label: 'i');
      expect(p2.label, equals('i'));
    });
  });
}
