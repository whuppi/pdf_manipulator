// End-to-end tests for structural ops through NativeBridge.
// Each test verifies the full pipeline: source → CallbackReader → engine
// → mutation → CallbackWriter → SinkServer → PdfSink.

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/bridge/native/native_bridge.dart';

import '../../helpers/pdf_fixtures.dart';
import '../test_helpers.dart';

void main() {
  late NativeBridge bridge;

  setUp(() {
    bridge = NativeBridge();
  });

  tearDown(() async {
    await bridge.dispose();
  });

  group('NativeBridge.deletePages', () {
    test('deleting page 0 from 2-page PDF leaves 1 page', () async {
      // First create a 2-page PDF by merging
      final mergeSink = TestSink();
      await bridge.merge([src(minimalPdf), src(minimalPdf)], mergeSink);
      final twoPage = mergeSink.takeBytes();

      final doc = await bridge.open(src(twoPage));
      expect(doc.pageCount, 2);

      // Delete page 0
      final deleteSink = TestSink();
      await bridge.deletePages(src(twoPage), deleteSink, pages: [0]);
      final result = deleteSink.takeBytes();

      final afterDelete = await bridge.open(src(result));
      expect(afterDelete.pageCount, 1);
    });
  });

  group('NativeBridge.rotateAllPages', () {
    test('rotates all pages by 90 degrees', () async {
      final sink = TestSink();
      await bridge.rotateAllPages(src(minimalPdf), sink, degrees: 90);
      final rotated = sink.takeBytes();

      final doc = await bridge.open(src(rotated));
      expect(doc.pages[0].rotation, 90);
    });
  });

  group('NativeBridge.flattenForms', () {
    test('flatten on a PDF with no forms produces valid output', () async {
      final sink = TestSink();
      await bridge.flattenForms(src(minimalPdf), sink);
      final result = sink.takeBytes();

      final doc = await bridge.open(src(result));
      expect(doc.pageCount, 1);
    });
  });

  group('NativeBridge.compress', () {
    test('compress produces valid output', () async {
      final sink = TestSink();
      await bridge.compress(src(minimalPdf), sink, imageQuality: 75);
      final compressed = sink.takeBytes();

      final doc = await bridge.open(src(compressed));
      expect(doc.pageCount, 1);
    });
  });
}
