// Construction contracts: runtime-built selections throw ArgumentError;
// the constant constructors assert on invalid inputs.

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';

Matcher get _assertion => throwsA(isA<AssertionError>());
Matcher get _argument => throwsA(isA<ArgumentError>());

void main() {
  // Values come from a function so no call is a constant expression.
  int v(int n) => n;
  double d(double n) => n;

  group('PdfPages.list', () {
    test('rejects an empty list', () {
      expect(() => PdfPages.list(const []), _argument);
    });

    test('rejects a negative page', () {
      expect(() => PdfPages.list([0, -1]), _argument);
    });

    test('rejects a duplicate page', () {
      expect(() => PdfPages.list([1, 2, 1]), _argument);
    });

    test('sorts the pages', () {
      expect(PdfPages.list([3, 0, 2]), PdfPages.list([0, 2, 3]));
      expect(PdfPages.list([3, 0, 2]), isNot(PdfPages.list([3, 0])));
    });

    test('accepts any iterable', () {
      expect(PdfPages.list({4, 1}), PdfPages.list([1, 4]));
    });
  });

  group('PdfPageOrder', () {
    test('rejects an empty order', () {
      expect(() => PdfPageOrder([]), _argument);
    });

    test('rejects a negative page', () {
      expect(() => PdfPageOrder([1, -2]), _argument);
    });

    test('rejects a duplicate page', () {
      expect(() => PdfPageOrder([1, 1]), _argument);
    });

    test('keeps the order given', () {
      expect(PdfPageOrder([2, 0, 1]).indices, [2, 0, 1]);
    });
  });

  group('asserting constructors', () {
    test('PdfPages.single rejects a negative index', () {
      expect(() => PdfPages.single(v(-1)), _assertion);
    });

    test('PdfPages.range rejects an empty or backwards range', () {
      expect(() => PdfPages.range(v(2), v(2)), _assertion);
      expect(() => PdfPages.range(v(3), v(1)), _assertion);
      expect(() => PdfPages.range(v(-1), v(2)), _assertion);
    });

    test('PdfPages.from rejects a negative start', () {
      expect(() => PdfPages.from(v(-1)), _assertion);
    });

    test('valid selections build', () {
      expect(PdfPages.single(v(0)), const PdfPages.single(0));
      expect(PdfPages.range(v(0), v(1)), const PdfPages.range(0, 1));
      expect(PdfPages.from(v(0)), const PdfPages.from(0));
    });

    test('PdfRect rejects a negative size', () {
      expect(() => PdfRect(x: 0, y: 0, width: d(-1), height: 1), _assertion);
      expect(() => PdfRect(x: 0, y: 0, width: 1, height: d(-1)), _assertion);
    });

    test('PdfRect.fromEdges asserts right >= left and top >= bottom', () {
      expect(
        () => PdfRect.fromEdges(left: d(5), bottom: 0, right: 1, top: 1),
        _assertion,
      );
      expect(
        () => PdfRect.fromEdges(left: 0, bottom: d(5), right: 1, top: 1),
        _assertion,
      );
      final r = PdfRect.fromEdges(left: d(1), bottom: 2, right: 4, top: 7);
      expect([r.x, r.y, r.width, r.height], [1, 2, 3, 5]);
    });

    test('PdfSplitRule.every rejects fewer than one page', () {
      expect(() => PdfSplitRule.every(v(0)), _assertion);
    });

    test('PdfSplitRule.maxBytes rejects fewer than one byte', () {
      expect(() => PdfSplitRule.maxBytes(v(0)), _assertion);
    });

    test('PdfRenderSize.dpi rejects a dpi below one', () {
      expect(() => PdfRenderSize.dpi(v(0)), _assertion);
    });

    test('PdfRenderSize.fit rejects a side below one pixel', () {
      expect(
        () => PdfRenderSize.fit(maxWidth: v(0), maxHeight: 10),
        _assertion,
      );
      expect(
        () => PdfRenderSize.fit(maxWidth: 10, maxHeight: v(0)),
        _assertion,
      );
    });
  });
}
