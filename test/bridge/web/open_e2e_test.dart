// End-to-end test: WebBridge.open
//
// Tests the full web pipeline:
// Dart main → stream PdfSource to OPFS → Web Worker → WASM engine
// reads from OPFS via JsCallbackReader (SyncAccessHandle) → result
// comes back via postMessage
//
// Run: dart test test/bridge/web/open_e2e_test.dart -p chrome

@TestOn('browser')
library;

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/bridge/web/web_bridge.dart';

import '../../helpers/pdf_fixtures.dart';
import '../test_helpers.dart';

void main() {
  late WebBridge bridge;

  setUpAll(() async {
    final channel = spawnHybridUri('asset_server.dart');
    final serverPort = await channel.stream.first as int;
    bridge = WebBridge(
      workerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    );
  });

  tearDownAll(() async {
    await bridge.dispose();
  });

  group('WebBridge.open', () {
    test('opens minimal PDF — page count, version, dimensions', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.pageCount, 1);
      expect(doc.version, contains('.'));
      expect(doc.pages, hasLength(1));
      expect(doc.pages[0].width, greaterThan(0));
      expect(doc.pages[0].height, greaterThan(0));
    });

    test('opens a second PDF (different call, same bridge)', () async {
      final doc = await bridge.open(src(minimalPdf));
      expect(doc.pageCount, 1);
    });
  });
}
