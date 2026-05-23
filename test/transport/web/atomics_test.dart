// Atomics mode test — SharedArrayBuffer + Atomics.wait/notify for reads.
//
// Requires SharedArrayBuffer to be available (Chrome enables it by default).
// Uses the chrome-coi platform from dart_test.yaml.
//
// Run: dart test test/bridge/web/atomics_test.dart -p chrome-coi

@TestOn('browser')
library;

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
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

  group('Atomics mode prerequisites', () {
    test('SharedArrayBuffer constructor exists', () {
      expect(globalContext.has('SharedArrayBuffer'), isTrue);
    });
  });

  group('Atomics mode open (pure — no OPFS pre-copy)', () {
    late web.Worker coordinator;
    final collected = <Map<String, Object?>>[];
    Completer<void>? resultCompleter;
    final pdfBytes = minimalPdf;

    setUp(() {
      collected.clear();
      coordinator = web.Worker(coordinatorBlobUrl.toJS);
      coordinator.onmessage = ((web.MessageEvent e) {
        final data = (e.data as JSAny).dartify();
        if (data is Map) {
          final entry = data.map((k, v) => MapEntry(k.toString(), v));
          collected.add(entry);
          final type = entry['type']?.toString();

          // Handle readAt requests — fulfill from pdfBytes
          if (type == 'readAt') {
            final readId = entry['readId']?.toString() ?? '';
            final offset = entry['offset'] as int? ?? 0;
            final count = entry['count'] as int? ?? 0;
            final end = (offset + count).clamp(0, pdfBytes.length);
            final chunk = pdfBytes.sublist(offset, end);
            coordinator.postMessage(<String, Object?>{
              'type': 'readAtResponse',
              'readId': readId,
              'bytes': chunk.buffer,
            }.jsify());
          }

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

    test('mode detected as atomics', () async {
      coordinator.postMessage(<String, Object?>{
        'type': 'init',
        'wasmWorkerUrl': 'http://localhost:$serverPort/web_assets/wasm_worker.js',
      }.jsify());
      await Future<void>.delayed(const Duration(seconds: 3));

      final ready = collected.firstWhere((m) => m['type'] == 'ready');
      expect(ready['ioMode'], 'atomics');
    });

    test('open PDF via Atomics.wait readAt chain', () async {
      coordinator.postMessage(<String, Object?>{
        'type': 'init',
        'wasmWorkerUrl': 'http://localhost:$serverPort/web_assets/wasm_worker.js',
      }.jsify());
      await Future<void>.delayed(const Duration(seconds: 3));

      // Submit open WITHOUT opfsFile — forces Atomics readAt chain
      resultCompleter = Completer<void>();
      coordinator.postMessage(<String, Object?>{
        'type': 'submit',
        'op': 'open',
        'args': <String, Object?>{'sourceLength': pdfBytes.length, 'password': null},
        // NO opfsFile — forces Atomics mode readAt
      }.jsify());

      await resultCompleter!.future.timeout(
        const Duration(seconds: 15),
        onTimeout: () {},
      );
      await Future<void>.delayed(const Duration(seconds: 1));

      final errors = collected.where((m) => m['type'] == 'error').toList();
      if (errors.isNotEmpty) {
        final readAts = collected.where((m) => m['type'] == 'readAt').length;
        fail('Error: ${errors.first['error']} (readAt requests: $readAts)');
      }

      final results = collected.where((m) => m['type'] == 'result').toList();
      expect(results, isNotEmpty, reason: 'Should get a result');
      final raw = results.last['result'];
      if (raw is Map) {
        final result = raw.map((k, v) => MapEntry(k.toString(), v));
        expect(result['pageCount'], 1);
      }
    });
  });
}
