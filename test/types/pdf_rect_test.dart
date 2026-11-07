// PdfRect — coordinate storage and equality.

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

    test(
      'edges: left and bottom are the origin, right and top the far corner',
      () {
        const r = PdfRect(x: 10, y: 20, width: 100, height: 200);
        expect(r.left, equals(10));
        expect(r.bottom, equals(20));
        expect(r.right, equals(110));
        expect(r.top, equals(220));
      },
    );

    test('top is above bottom for a positive height', () {
      const r = PdfRect(x: 0, y: 50, width: 10, height: 30);
      expect(r.top, greaterThan(r.bottom));
    });

    test('fromEdges is the rectangle with those edges', () {
      const r = PdfRect.fromEdges(left: 10, bottom: 20, right: 110, top: 220);
      expect(r, equals(const PdfRect(x: 10, y: 20, width: 100, height: 200)));
      expect(r.top, equals(220));
      expect(r.bottom, equals(20));
    });

    test('equal rectangles are == and share a hashCode', () {
      const a = PdfRect(x: 1, y: 2, width: 3, height: 4);
      const b = PdfRect(x: 1, y: 2, width: 3, height: 4);
      const c = PdfRect(x: 1, y: 2, width: 3, height: 5);
      expect(a, equals(b));
      expect(a.hashCode, equals(b.hashCode));
      expect(a, isNot(equals(c)));
    });

    test('toString is readable', () {
      const r = PdfRect(x: 0, y: 0, width: 595, height: 842);
      expect(r.toString(), contains('595'));
      expect(r.toString(), contains('842'));
    });
  });
}
