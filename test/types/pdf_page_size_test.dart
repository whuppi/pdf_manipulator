// PdfPageSize — the named page sizes and the landscape turn.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

void main() {
  group('PdfPageSize', () {
    test('the named sizes carry their dimensions in points', () {
      expect(const PdfPageSize.a4().width, 595);
      expect(const PdfPageSize.a4().height, 842);
      expect(const PdfPageSize.letter().width, 612);
      expect(const PdfPageSize.letter().height, 792);
      expect(const PdfPageSize.legal().width, 612);
      expect(const PdfPageSize.legal().height, 1008);
      expect(const PdfPageSize.a3().width, 842);
      expect(const PdfPageSize.a3().height, 1190);
      expect(const PdfPageSize.a5().width, 420);
      expect(const PdfPageSize.a5().height, 595);
    });

    test('a custom size keeps the dimensions it was given', () {
      const size = PdfPageSize.points(200, 300);
      expect(size.width, 200);
      expect(size.height, 300);
    });

    test('landscape turns the size on its side', () {
      final landscape = const PdfPageSize.a4().landscape;
      expect(landscape.width, const PdfPageSize.a4().height);
      expect(landscape.height, const PdfPageSize.a4().width);
    });

    test('a size read through the type keeps its landscape turn', () {
      const PdfPageSize size = PdfPageSize.a4();
      expect(size.landscape.width, 842);
      expect(size.landscape.height, 595);
    });
  });
}
