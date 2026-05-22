// End-to-end test: NativeBridge.open
//
// Tests the full pipeline:
// Dart main → SourceServer → worker isolate → NativeCallable.listener
// → shared buffer → Rust pool thread → CallbackReader → condvar dance
// → pdf_oxide engine → result posted via allo-isolate → Dart decodes PdfDoc

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

  group('NativeBridge.open', () {
    test('opens minimal PDF — page count is 1', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.pageCount, 1);
    });

    test('reads version string', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.version, contains('.'));
    });

    test('reads page dimensions', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, greaterThan(0));
      expect(doc.pages[0].height, greaterThan(0));
    });

    test('isEncrypted is false for minimal PDF', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.isEncrypted, isFalse);
    });

    test('throws on garbage bytes', () async {
      expect(
        () => bridge.open(src(garbageBytes)),
        throwsA(isA<StateError>()),
      );
    });
  });
}
