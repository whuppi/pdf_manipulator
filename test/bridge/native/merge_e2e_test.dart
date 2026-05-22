// End-to-end test: NativeBridge.merge
//
// Tests the full read + write pipeline:
// - Primary source via CallbackReader (condvar reads)
// - Secondary sources as byte arrays
// - Output via CallbackWriter (condvar writes → SinkServer → PdfSink)

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

  group('NativeBridge.merge', () {
    test('merges two PDFs — result has 2 pages', () async {
      final sink = TestSink();
      await bridge.merge([src(minimalPdf), src(minimalPdf)], sink);
      final merged = sink.takeBytes();

      expect(merged.length, greaterThan(0));

      // Verify the merged result is a valid PDF with 2 pages
      final doc = await bridge.open(src(merged));
      expect(doc.pageCount, 2);
    });

    test('merges three PDFs — result has 3 pages', () async {
      final sink = TestSink();
      await bridge.merge(
        [src(minimalPdf), src(minimalPdf), src(minimalPdf)],
        sink,
      );
      final merged = sink.takeBytes();

      final doc = await bridge.open(src(merged));
      expect(doc.pageCount, 3);
    });

    test('output goes to PdfSink (not returned as bytes)', () async {
      final sink = TestSink();
      await bridge.merge([src(minimalPdf), src(minimalPdf)], sink);
      expect(sink.length, greaterThan(0));
    });

    test('throws on single input', () async {
      expect(
        () => bridge.merge([src(minimalPdf)], TestSink()),
        throwsA(isA<ArgumentError>()),
      );
    });
  });
}
