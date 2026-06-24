// lane_worker.js ↔ lane protocol — the web wire-contract guard.
//
// The web twin of test/runtime/native/channel_buffers_test.dart:
// native pins the shared-memory layout both sides hardcode; this
// pins the message tags and injected codes both sides speak. Parses
// lane_worker.js from disk — zero hardcoded allowlists beyond the
// protocol file itself.

@TestOn('vm')
library;

// io-exempt: reads lane_worker.js from disk to pin the web wire contract.
import 'dart:io';

import 'package:pdf_manipulator/src/runtime/web/lane_protocol.dart';
import 'package:test/test.dart';

Set<String> _extractJsCases(String source) {
  final pattern = RegExp(r"case\s+'([a-zA-Z.]+)'");
  return pattern.allMatches(source).map((m) => m.group(1)!).toSet();
}

void main() {
  group('lane_worker.js ↔ lane protocol', () {
    late String workerSource;

    setUpAll(() {
      final file = File('web_assets/lane_worker.js');
      if (!file.existsSync()) fail('lane_worker.js not found');
      workerSource = file.readAsStringSync();
    });

    test('worker calls the lane WASM surface', () {
      expect(
        workerSource,
        contains('lane_execute'),
        reason: 'lane_worker.js must call lane_execute from pdf_oxide.js',
      );
      expect(
        workerSource,
        contains('lane_init'),
        reason: 'lane_worker.js must call lane_init at bootstrap',
      );
    });

    test('worker has no per-op dispatch switch', () {
      final perOpPattern = RegExp(
        r"case\s+'(open|extract|search|render|sign)'",
      );
      expect(
        perOpPattern.hasMatch(workerSource),
        isFalse,
        reason:
            'lane_worker.js must not dispatch per op — all ops go through lane_execute',
      );
    });

    test('worker handles every Dart→worker message tag', () {
      const dartToWorker = {
        LaneMsg.init,
        LaneMsg.exec,
        LaneMsg.readAtResult,
        LaneMsg.chunkAck,
        LaneMsg.releaseHeld,
        LaneMsg.opfsWrite,
        LaneMsg.opfsDrop,
      };
      final cases = _extractJsCases(workerSource);
      for (final tag in dartToWorker) {
        expect(cases, contains(tag), reason: 'worker missing case: $tag');
      }
    });

    test('worker posts only worker→Dart message tags', () {
      const workerToDart = {
        LaneMsg.booted,
        LaneMsg.ready,
        LaneMsg.readAt,
        LaneMsg.chunk,
        LaneMsg.result,
        LaneMsg.error,
        LaneMsg.opfsWriteAck,
      };
      final posted = RegExp(
        "postMessage\\(\\{ type: '([a-zA-Z.]+)'",
      ).allMatches(workerSource).map((m) => m.group(1)!).toSet();
      for (final tag in posted) {
        expect(
          workerToDart,
          contains(tag),
          reason: 'worker posts unknown tag: $tag — add it to LaneMsg',
        );
      }
    });

    test('every P.* the worker reads is a real injected protocol key', () {
      // Drift direction that matters: a P.key the injection doesn't
      // provide is undefined in JS — silent NaN/undefined physics.
      final injected = laneProtocolCodes().keys.toSet();
      final used = RegExp(
        r'P\.([a-zA-Z0-9_]+)',
      ).allMatches(workerSource).map((m) => m.group(1)!).toSet();
      expect(
        used,
        isNotEmpty,
        reason: 'worker must read its codes from the injected protocol',
      );
      for (final key in used) {
        expect(
          injected,
          contains(key),
          reason:
              'worker reads P.\$key which lane_protocol.dart '
              'does not inject',
        );
      }
    });

    test('worker returns no literal host I/O codes', () {
      expect(
        RegExp(r'return\s+-[0-9]').hasMatch(workerSource),
        isFalse,
        reason: 'host I/O codes must come from P.*, not literals',
      );
    });

    test('worker contains no routing or queuing logic', () {
      for (final word in [
        'pinnedHandles',
        'waitQueue',
        'idleWorkers',
        'acquireWorker',
        'poolSize',
      ]) {
        expect(
          workerSource.contains(word),
          isFalse,
          reason: 'routing/pool logic ($word) belongs in the Dart Router',
        );
      }
    });
  });

  // ── Dumb-edge guard — Dart platform adapters ──
}
