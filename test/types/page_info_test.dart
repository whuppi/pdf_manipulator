// PdfPageInfo — dimensions, rotation, effective width/height.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  group('PdfPageInfo', () {
    test('stores dimensions', () {
      const p = PdfPageInfo(index: 0, width: 595, height: 842);
      expect(p.width, equals(595));
      expect(p.height, equals(842));
    });

    test('rotation defaults to 0', () {
      const p = PdfPageInfo(index: 0, width: 100, height: 200);
      expect(p.rotation, equals(PdfRotation.r0));
    });

    test('effectiveWidth equals width at 0°', () {
      const p = PdfPageInfo(
        index: 0,
        width: 595,
        height: 842,
        rotation: PdfRotation.r0,
      );
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 90°', () {
      const p = PdfPageInfo(
        index: 0,
        width: 595,
        height: 842,
        rotation: PdfRotation.r90,
      );
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('effectiveWidth unchanged at 180°', () {
      const p = PdfPageInfo(
        index: 0,
        width: 595,
        height: 842,
        rotation: PdfRotation.r180,
      );
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 270°', () {
      const p = PdfPageInfo(
        index: 0,
        width: 595,
        height: 842,
        rotation: PdfRotation.r270,
      );
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('equal values are equal and hash alike', () {
      const a = PdfPageInfo(index: 1, width: 10, height: 20, label: 'i');
      const b = PdfPageInfo(index: 1, width: 10, height: 20, label: 'i');
      const c = PdfPageInfo(index: 1, width: 10, height: 20);
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));
      expect(a, isNot(equals(c)));
    });

    test('label is nullable', () {
      const p = PdfPageInfo(index: 0, width: 100, height: 200);
      expect(p.label, isNull);

      const p2 = PdfPageInfo(index: 0, width: 100, height: 200, label: 'i');
      expect(p2.label, equals('i'));
    });
  });
}
