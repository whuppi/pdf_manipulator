@TestOn('browser')
library;

import 'dart:typed_data';

import 'package:test/test.dart';
import 'package:pdf_manipulator/src/bridge/web/web_bridge.dart';

import '../../helpers/new_memory_io.dart';
import '../../helpers/pdf_fixtures.dart';

void main() {
  late WebBridge bridge;
  late int serverPort;

  setUpAll(() async {
    final channel = spawnHybridUri('../../web/asset_server.dart');
    serverPort = await channel.stream.first as int;
  });

  setUp(() {
    bridge = WebBridge(
      workerUrl: 'http://localhost:$serverPort/web_assets/worker.js',
    );
  });

  tearDown(() async {
    await bridge.dispose();
  });

  group('WebBridge.open', () {
    test('opens minimal PDF — page count is 1', () async {
      final doc = await bridge.open(sourceOf(minimalPdf));
      expect(doc.pageCount, 1);
    });

    test('reads version string', () async {
      final doc = await bridge.open(sourceOf(minimalPdf));
      expect(doc.version, contains('.'));
    });

    test('reads page dimensions', () async {
      final doc = await bridge.open(sourceOf(minimalPdf));
      expect(doc.pages.length, 1);
      expect(doc.pages[0].width, greaterThan(0));
      expect(doc.pages[0].height, greaterThan(0));
    });

    test('isEncrypted is false for unencrypted PDF', () async {
      final doc = await bridge.open(sourceOf(minimalPdf));
      expect(doc.isEncrypted, false);
    });

    test('throws on garbage bytes', () async {
      final garbage = TestPdfSource(Uint8List.fromList([1, 2, 3, 4]));
      expect(() => bridge.open(garbage), throwsA(anything));
    });
  });
}
