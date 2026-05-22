// End-to-end test: NativeBridge.extract
//
// Tests the read-only pipeline:
// Dart main → SourceServer → worker isolate → NativeCallable.listener
// → shared buffer → Rust pool thread → CallbackReader → condvar dance
// → pdf_oxide extract_text / to_markdown → result posted via allo-isolate

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/api/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/api/types/pdf_pages.dart';
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

  group('NativeBridge.extract', () {
    test('extracts text from a single page', () async {
      final source = TestSource(minimalPdf);
      final text = await bridge.extract(source,
          pages: const PdfPages.single(0));
      expect(text, isA<String>());
    });

    test('extracts text from all pages', () async {
      final source = TestSource(minimalPdf);
      final text = await bridge.extract(source,
          pages: const PdfPages.all());
      expect(text, isA<String>());
    });

    test('extracts markdown from all pages', () async {
      final source = TestSource(minimalPdf);
      final text = await bridge.extract(source,
          pages: const PdfPages.all(),
          format: PdfExtractionFormat.markdown);
      expect(text, isA<String>());
    });
  });
}
