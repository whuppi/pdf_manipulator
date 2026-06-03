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
      expect(p.rotation, equals(0));
    });

    test('effectiveWidth equals width at 0°', () {
      const p = PdfPageInfo(index: 0, width: 595, height: 842, rotation: 0);
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 90°', () {
      const p = PdfPageInfo(index: 0, width: 595, height: 842, rotation: 90);
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('effectiveWidth unchanged at 180°', () {
      const p = PdfPageInfo(index: 0, width: 595, height: 842, rotation: 180);
      expect(p.effectiveWidth, equals(595));
      expect(p.effectiveHeight, equals(842));
    });

    test('effectiveWidth swaps at 270°', () {
      const p = PdfPageInfo(index: 0, width: 595, height: 842, rotation: 270);
      expect(p.effectiveWidth, equals(842));
      expect(p.effectiveHeight, equals(595));
    });

    test('label is nullable', () {
      const p = PdfPageInfo(index: 0, width: 100, height: 200);
      expect(p.label, isNull);

      const p2 = PdfPageInfo(index: 0, width: 100, height: 200, label: 'i');
      expect(p2.label, equals('i'));
    });
  });
}
