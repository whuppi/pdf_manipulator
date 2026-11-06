// executor_worker.js ↔ lane protocol — the web wire-contract guard.
//
// The web twin of test/runtime/native/channel_buffers_test.dart:
// native pins the shared-memory layout both sides hardcode; this
// pins the message tags and injected codes both sides speak. Parses
// executor_worker.js from disk — zero hardcoded allowlists beyond the
// protocol file itself.

@TestOn('vm')
library;

// io-exempt: reads executor_worker.js from disk to pin the web wire contract.
import 'dart:io';

import 'package:pdf_manipulator/src/runtime/web/protocol.dart';
import 'package:test/test.dart';

Set<String> _extractJsCases(String source) {
  final pattern = RegExp(r"case\s+'([a-zA-Z.]+)'");
  return pattern.allMatches(source).map((m) => m.group(1)!).toSet();
}

void main() {
  group('executor_worker.js ↔ lane protocol', () {
    late String workerSource;

    setUpAll(() {
      final file = File('web_assets/executor_worker.js');
      if (!file.existsSync()) fail('executor_worker.js not found');
      workerSource = file.readAsStringSync();
    });

    test('worker calls the lane WASM surface', () {
      expect(
        workerSource,
        contains('execute'),
        reason: 'executor_worker.js must call execute from pdf_oxide.js',
      );
      expect(
        workerSource,
        contains('engine_state_new'),
        reason: 'executor_worker.js must call engine_state_new at bootstrap',
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
            'executor_worker.js must not dispatch per op — all ops go through execute',
      );
    });

    test('worker handles every Dart→worker message tag', () {
      const dartToWorker = {
        LaneMsg.init,
        LaneMsg.exec,
        LaneMsg.readAtResult,
        LaneMsg.chunkAck,
        LaneMsg.releaseHold,
        LaneMsg.reset,
        LaneMsg.opfsWrite,
        LaneMsg.opfsDrop,
        LaneMsg.spoolRead,
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
        LaneMsg.resetDone,
        LaneMsg.opfsWriteAck,
        LaneMsg.spoolChunk,
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
      // provide is undefined in JS — silent NaN/undefined behaviour.
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
              'worker reads P.\$key which protocol.dart '
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

    test('held readers: the worker and the web lane share one group order', () {
      // A handle's held set is its creating job's group: the worker mounts
      // the group's readers in creation order and files a later job's kept
      // readers under that group; the lane serves reads in the same order.
      final laneSource = File(
        'lib/src/runtime/web/lease.dart',
      ).readAsStringSync();
      expect(laneSource, contains("msg['holdGroupId']"));
      expect(laneSource, contains("'holdGroupId': holdGroupId"));
      expect(workerSource, contains('r.holdGroupId === holdGroupId'));
      expect(workerSource, contains('holdGroupId ?? jobId'));
      expect(
        laneSource,
        contains('(_heldSources[group] ??= []).addAll'),
        reason: 'kept sources append after the group\'s earlier ones',
      );
    });

    test('result, error and resetDone posts each carry the memory and the '
        'peak fields', () {
      for (final field in [LaneMsgFields.memBytes, LaneMsgFields.peakBytes]) {
        for (final tag in [LaneMsg.result, LaneMsg.error, LaneMsg.resetDone]) {
          final post = RegExp(
            "postMessage\\(\\{ type: '$tag'[^\\n]*\\b$field:",
          );
          expect(
            post.hasMatch(workerSource),
            isTrue,
            reason: 'the $tag post must carry $field',
          );
        }
      }
    });
    test('every opfsWriteAck names the write it answers, and the lane only '
        'accepts the one it waits for', () {
      for (final field in [LaneMsgFields.jobId, LaneMsgFields.writeOffset]) {
        final posts = RegExp(
          "postMessage\\(\\{ type: '${LaneMsg.opfsWriteAck}'[^\\n]*\\b$field:",
        ).allMatches(workerSource);
        expect(
          posts,
          hasLength(2),
          reason: 'the ack and the failed ack must both carry $field',
        );
      }
      final laneSource = File(
        'lib/src/runtime/web/lease.dart',
      ).readAsStringSync();
      expect(laneSource, contains('_int(obj, LaneMsgFields.jobId) =='));
      expect(laneSource, contains('_int(obj, LaneMsgFields.writeOffset) =='));
    });

    test('every message after the boot handshake carries the lease', () {
      final lease = LaneMsgFields.lease;
      // Everything the worker posts, bar the boot handshake: `booted`,
      // `ready` and the init failure (the one `error` that names no lease).
      final posts = RegExp(
        r"postMessage\(\{\s*type: '(\w+)',([^\n]*)",
      ).allMatches(workerSource);
      expect(posts, isNotEmpty);
      for (final post in posts) {
        final tag = post.group(1)!;
        final rest = post.group(2)!;
        if (tag == LaneMsg.error && rest.contains('WASM init failed')) continue;
        expect(
          rest,
          contains(' $lease'),
          reason: 'the worker posts $tag without the lease: $rest',
        );
      }

      final laneSource = File(
        'lib/src/runtime/web/lease.dart',
      ).readAsStringSync();
      expect(
        RegExp(
          'obj\\[LaneMsgFields\\.lease\\] = _lease\\.toJS',
        ).allMatches(laneSource),
        hasLength(2),
        reason: '_post and _postRaw both stamp the lease',
      );
      expect(laneSource, contains('..[LaneMsgFields.lease] = _lease.toJS'));
      expect(
        laneSource,
        contains('if (_int(obj, LaneMsgFields.lease) != _lease) return;'),
        reason: 'the adapter drops a message of another lease',
      );
    });

    test('worker admits only the lease it serves or a newer one when idle', () {
      final handler = workerSource.substring(
        workerSource.indexOf('self.onmessage'),
      );
      final blocks = handler.split(RegExp(r"\n    case '"));
      const guarded = {
        LaneMsg.readAtResult,
        LaneMsg.chunkAck,
        LaneMsg.releaseHold,
        LaneMsg.opfsDrop,
        LaneMsg.opfsWrite,
        LaneMsg.reset,
        LaneMsg.exec,
      };
      final seen = <String>{};
      for (final block in blocks.skip(1)) {
        final tag = block.substring(0, block.indexOf("'"));
        if (!guarded.contains(tag)) continue;
        seen.add(tag);
        expect(
          block,
          contains('admit(msg)'),
          reason: 'the $tag handler acts without checking the lease',
        );
      }
      expect(seen, guarded);
      expect(workerSource, contains('msg.${LaneMsgFields.lease}'));
    });

    test('exec and reset are refused while the worker is busy', () {
      final handler = workerSource.substring(
        workerSource.indexOf('self.onmessage'),
      );
      for (final tag in [LaneMsg.exec, LaneMsg.reset]) {
        final start = handler.indexOf("case '$tag'");
        final block = handler.substring(start, handler.indexOf('try {', start));
        expect(
          block,
          contains('busy !== null'),
          reason: 'the $tag handler may be re-entered',
        );
        expect(block, contains("busy = '"));
      }
      expect(
        workerSource,
        contains('${LaneMsgFields.protocolError}: true'),
        reason: 'a refusal is a typed protocol error',
      );
    });

    test('reset empties the readers, the job files and the OPFS directory', () {
      final reset = workerSource.substring(
        workerSource.indexOf('async function resetLane()'),
        workerSource.indexOf('// Exec — register readers'),
      );
      for (final step in [
        'housekeeping',
        'dropReader',
        'opfsWriteHandles.clear()',
        'jobFiles.clear()',
        'emptyLaneDir()',
        'engine_state_drop(statePtr)',
        'engine_state_new()',
      ]) {
        expect(reset, contains(step), reason: 'reset skips $step');
      }
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
