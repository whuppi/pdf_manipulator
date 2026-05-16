import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  group('PdfRect', () {
    test('stores coordinates', () {
      const r = PdfRect(x: 10, y: 20, width: 100, height: 200);
      expect(r.x, equals(10));
      expect(r.y, equals(20));
      expect(r.width, equals(100));
      expect(r.height, equals(200));
    });

    test('computes right and bottom', () {
      const r = PdfRect(x: 10, y: 20, width: 100, height: 200);
      expect(r.right, equals(110));
      expect(r.bottom, equals(220));
    });

    test('toString is readable', () {
      const r = PdfRect(x: 0, y: 0, width: 595, height: 842);
      expect(r.toString(), contains('595'));
      expect(r.toString(), contains('842'));
    });
  });
}
