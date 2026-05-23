// OPFS pipeline — direct coordinator test (no WebBridge).
// Verifies: OPFS write → finalize → submit → WASM reads from SyncAccessHandle.

@TestOn('browser')
library;

import 'dart:async';
import 'dart:js_interop';
import 'dart:typed_data';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import '../../helpers/fixtures.dart';
import 'web_test_helper.dart';

void main() {
  late int serverPort;
  late String coordinatorBlobUrl;

  setUpAll(() async {
    final channel = spawnHybridUri('/test/helpers/asset_server.dart');
    serverPort = ((await channel.stream.first) as num).toInt();
    coordinatorBlobUrl = await fetchAsBlobUrl(
        'http://localhost:$serverPort/web_assets/coordinator.js');
  });

  // Direct coordinator test — bypasses WebBridge to verify the JS pipeline
  // end-to-end with visible message collection.
  group('OPFS pipeline (direct coordinator)', () {
    late web.Worker coordinator;
    final collected = <Map<String, Object?>>[];
    Completer<void>? resultCompleter;

    setUp(() {
      collected.clear();
      coordinator = web.Worker(coordinatorBlobUrl.toJS);
      coordinator.onmessage = ((web.MessageEvent e) {
        final data = (e.data as JSAny).dartify();
        if (data is Map) {
          final entry = data.map((k, v) => MapEntry(k.toString(), v));
          collected.add(entry);
          final type = entry['type']?.toString();
          if ((type == 'result' || type == 'error') &&
              resultCompleter != null &&
              !resultCompleter!.isCompleted) {
            resultCompleter!.complete();
          }
        }
      }).toJS;
    });

    tearDown(() {
      coordinator.terminate();
    });

    Future<void> initCoordinator() async {
      coordinator.postMessage(<String, Object?>{
        'type': 'init',
        'wasmWorkerUrl': 'http://localhost:$serverPort/web_assets/wasm_worker.js',
      }.jsify());
      await Future<void>.delayed(const Duration(seconds: 3));
      expect(collected.any((m) => m['type'] == 'ready'), isTrue,
          reason: 'Coordinator should send ready');
    }

    Future<void> writeToOpfs(String filename, List<int> bytes) async {
      coordinator.postMessage(<String, Object?>{
        'type': 'opfs.write',
        'opId': 0,
        'filename': filename,
        'chunk': (bytes is Uint8List ? bytes : Uint8List.fromList(bytes)).buffer,
        'offset': 0,
      }.jsify());
      await Future<void>.delayed(const Duration(milliseconds: 500));
      coordinator.postMessage(<String, Object?>{
        'type': 'opfs.finalize',
        'opId': 0,
      }.jsify());
      await Future<void>.delayed(const Duration(milliseconds: 500));
    }

    Future<Map<String, Object?>?> submitAndWait(String op, Map<String, Object?> args, {String? opfsFile}) async {
      resultCompleter = Completer<void>();
      coordinator.postMessage(<String, Object?>{
        'type': 'submit',
        'op': op,
        'args': args,
        if (opfsFile != null) 'opfsFile': opfsFile,
      }.jsify());
      await resultCompleter!.future.timeout(
        const Duration(seconds: 15),
        onTimeout: () {},
      );
      final results = collected.where((m) => m['type'] == 'result').toList();
      if (results.isEmpty) return null;
      final raw = results.last['result'];
      if (raw is Map) return raw.map((k, v) => MapEntry(k.toString(), v));
      return null;
    }

    test('opens minimal PDF — returns pageCount 1', () async {
      await initCoordinator();
      await writeToOpfs('_test_open.tmp', minimalPdf);
      final result = await submitAndWait('open', {
        'sourceLength': minimalPdf.length,
        'password': null,
      }, opfsFile: '_test_open.tmp');

      expect(result, isNotNull);
      expect(result!['pageCount'], 1);
    });

    test('opens minimal PDF — returns page dimensions', () async {
      await initCoordinator();
      await writeToOpfs('_test_dims.tmp', minimalPdf);
      final result = await submitAndWait('open', {
        'sourceLength': minimalPdf.length,
        'password': null,
      }, opfsFile: '_test_dims.tmp');

      expect(result, isNotNull);
      final pages = result!['pages'] as List;
      expect(pages, hasLength(1));
      final page = pages[0] as Map;
      expect(page['width'], greaterThan(0));
      expect(page['height'], greaterThan(0));
    });

    test('mode is detected as atomics or opfs', () async {
      await initCoordinator();
      final ready = collected.firstWhere((m) => m['type'] == 'ready');
      final mode = ready['ioMode']?.toString();
      expect(mode, anyOf('atomics', 'opfs'));
    });
  });
}
