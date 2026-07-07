// Router — the shared brain, unit-tested against fake lanes.
//
// Everything here runs in milliseconds with no engine, no platform:
// placement (spawn-under-cap, least-loaded), response-driven pinning,
// held-resource lifecycle, instant dispose, and the per-op cancel
// binding (including the cancel-before-submit race the integration
// suites can only hit by luck).

@TestOn('vm')
library;

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:test/test.dart';

/// A lane that records every verb and completes jobs on command.
class FakeLane implements Lane {
  final submitted = <LaneJob>[];
  final cancelledJobs = <int>[];
  final releasedTokens = <Object>[];
  bool killed = false;

  /// Pending completers by jobId — tests complete them explicitly.
  final pending = <int, Completer<LaneSubmitResult>>{};

  /// When set, submit completes immediately with this result.
  LaneSubmitResult? autoResult;

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    submitted.add(job);
    final auto = autoResult;
    if (auto != null) return Future.value(auto);
    final completer = Completer<LaneSubmitResult>();
    pending[job.jobId] = completer;
    return completer.future;
  }

  /// Completes every still-pending job successfully.
  void completeAllPending() {
    for (final completer in pending.values) {
      if (!completer.isCompleted) completer.complete(_ok());
    }
  }

  @override
  void cancelJob(int jobId) => cancelledJobs.add(jobId);

  @override
  void releaseHeld(Object token) => releasedTokens.add(token);

  @override
  void kill() => killed = true;
}

class FakeLaneHost implements LaneHost {
  final lanes = <FakeLane>[];

  @override
  PdfIoMode get mode => PdfIoMode.native;

  @override
  int get defaultLaneCount => 2;

  @override
  Lane spawn() {
    final lane = FakeLane();
    lanes.add(lane);
    return lane;
  }
}

LaneSubmitResult _ok({int? handleId, Map<int, Object> held = const {}}) {
  final bytes = handleId == null
      ? Uint8List.fromList(const [1, 0, 0]) // success, zero fields
      : (BytesBuilder()
              ..addByte(1)
              ..add([1, 0])
              ..addByte(8)
              ..add('handleId'.codeUnits)
              ..addByte(1)
              ..add(
                Uint8List(4)
                  ..buffer.asByteData().setInt32(0, handleId, Endian.little),
              ))
            .takeBytes();
  return LaneSubmitResult(bytes, held);
}

Uint8List _open() => bin.encodeRequest('open', {'sourceLength': 1});
Uint8List _opOn(String op, int handleId) =>
    bin.encodeRequest(op, {'handleId': handleId});

void main() {
  late FakeLaneHost host;
  late Router router;

  setUp(() {
    host = FakeLaneHost();
    router = Router(host: host, maxLanes: 2, mode: PdfIoMode.native);
  });

  group('placement', () {
    test('first op spawns one lane; an idle lane is reused', () async {
      final first = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      expect(host.lanes, hasLength(1));
      host.lanes.single.pending[1]!.complete(_ok());
      await first;

      final second = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      expect(
        host.lanes,
        hasLength(1),
        reason: 'an idle lane is reused, not duplicated',
      );
      host.lanes.single.pending[2]!.complete(_ok());
      await second;
    });

    test(
      'all-busy spawns under the cap; beyond it queues least-loaded',
      () async {
        final first = router.execute(_open());
        final second = router.execute(_open());
        final third = router.execute(_open());
        await Future<void>.delayed(Duration.zero);

        expect(
          host.lanes,
          hasLength(2),
          reason: 'spawn while busy, but never past maxLanes',
        );
        final jobs = host.lanes.fold(0, (n, lane) => n + lane.submitted.length);
        expect(jobs, 3, reason: 'the overflow op rides an existing lane');

        for (final lane in host.lanes) {
          lane.completeAllPending();
        }
        await Future.wait([first, second, third]);
      },
    );
  });

  group('pinning', () {
    test('a response handle pins later ops to the creating lane', () async {
      // Lane A creates a handle; the caller receives the Router's global
      // id for it (the engine's raw id stays inside the lane).
      final open = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final laneA = host.lanes.single;
      laneA.pending[1]!.complete(_ok(handleId: 7));
      final handle = peekResponseHandleId((await open).bytes)!;

      // Make lane A busy so least-loaded would pick a fresh lane —
      // the pin must win anyway.
      final busy = router.execute(_open());
      await Future<void>.delayed(Duration.zero);

      final pinned = router.execute(_opOn('extract', handle));
      await Future<void>.delayed(Duration.zero);
      expect(
        laneA.submitted,
        hasLength(3),
        reason: 'the handle lives on lane A — its ops follow it',
      );

      laneA.pending[2]!.complete(_ok());
      laneA.pending[3]!.complete(_ok());
      await Future.wait([busy, pinned]);
    });

    test('a dispose op removes the pin', () async {
      final open = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final laneA = host.lanes.single;
      laneA.pending[1]!.complete(_ok(handleId: 7));
      final handle = peekResponseHandleId((await open).bytes)!;

      laneA.autoResult = _ok();
      await router.execute(_opOn('docDispose', handle));

      // The handle is gone: the next op for it is placed, not pinned —
      // with lane A idle it still lands there, so prove the unpin by
      // checking the pin map indirectly: a BUSY lane A + unpinned op
      // must go elsewhere.
      laneA.autoResult = null;
      final busy = router.execute(_open()); // occupies lane A
      await Future<void>.delayed(Duration.zero);
      final after = router.execute(_opOn('extract', handle));
      await Future<void>.delayed(Duration.zero);

      expect(
        host.lanes,
        hasLength(2),
        reason: 'unpinned op on a busy instance spawns/uses lane B',
      );
      final laneB = host.lanes[1];
      expect(laneB.submitted, isNotEmpty);

      for (final lane in host.lanes) {
        lane.completeAllPending();
      }
      await Future.wait([busy, after]);
    });

    test('handles on different lanes never share an id (cross-lane '
        'collision)', () async {
      // Two creates fire while the first is in-flight, so the second
      // spawns a second lane (placement spreads them). Each lane's
      // engine mints handle id 1 — LaneState.next_handle restarts at 1
      // per lane, so this is the exact id collision the native runtime
      // produces under parallel doc+editor+builder load.
      final createA = router.execute(_open());
      final createB = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      expect(
        host.lanes,
        hasLength(2),
        reason: 'two in-flight creates spread onto two lanes',
      );
      final laneA = host.lanes[0];
      final laneB = host.lanes[1];

      laneA.pending[1]!.complete(_ok(handleId: 1));
      laneB.pending[2]!.complete(_ok(handleId: 1));
      final resA = await createA;
      final resB = await createB;

      final idA = peekResponseHandleId(resA.bytes);
      final idB = peekResponseHandleId(resB.bytes);

      // The ids the callers receive must be distinct, or the pin map
      // conflates the two handles and a handle's later ops route to the
      // wrong lane — the "editor not found" race.
      expect(idA, isNotNull);
      expect(idB, isNotNull);
      expect(
        idA,
        isNot(equals(idB)),
        reason: 'handles on different lanes must not share an id',
      );

      // Handle A lives on lane A. Its ops must follow lane A even though
      // lane B created its handle last (last write wins in a raw-id pin
      // map, which would mis-route A's ops to lane B).
      final follow = router.execute(_opOn('extract', idA!));
      await Future<void>.delayed(Duration.zero);
      expect(
        laneA.submitted,
        hasLength(2),
        reason: "handle A's ops follow lane A, not the last-pinned lane",
      );
      expect(
        laneB.submitted,
        hasLength(1),
        reason: 'lane B only ran its own create',
      );

      laneA.completeAllPending();
      laneB.completeAllPending();
      await follow;
    });
  });

  group('held resources', () {
    test('held tokens become resourceIds; release reaches the lane', () async {
      final open = router.execute(_open(), keepSources: {0});
      await Future<void>.delayed(Duration.zero);
      host.lanes.single.pending[1]!.complete(
        _ok(handleId: 7, held: {0: 'token-A'}),
      );
      final result = await open;

      expect(result.resourceIds, hasLength(1));
      final resourceId = result.resourceIds[0]!;

      await router.releaseSource(resourceId);
      expect(host.lanes.single.releasedTokens, ['token-A']);

      await router.releaseSource(resourceId); // already gone
      await router.releaseSource(999); // never existed
      expect(
        host.lanes.single.releasedTokens,
        hasLength(1),
        reason: 'release is idempotent and unknown ids are no-ops',
      );
    });
  });

  group('dispose', () {
    test('kills every lane and resolves later ops as cancelled', () async {
      final inflight = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;

      await router.dispose();
      expect(lane.killed, isTrue);

      // A real lane completes its pending submits on kill; the fake
      // mimics that contract here.
      lane.pending[1]!.complete(
        LaneSubmitResult(buildCancelledResponse(), const {}),
      );
      final r = await inflight;
      expect(r.bytes, [2]);

      final after = await router.execute(_open());
      expect(after.bytes, [
        2,
      ], reason: 'ops on a disposed instance resolve cancelled');
      expect(
        host.lanes,
        hasLength(1),
        reason: 'a disposed router never spawns again',
      );

      await router.dispose(); // double dispose is a no-op
      final again = await router.execute(_open());
      expect(again.bytes, [
        2,
      ], reason: 'still disposed after the second dispose');
      expect(host.lanes, hasLength(1));
    });

    test('placement preserves submission order onto a lane', () async {
      final solo = Router(host: host, maxLanes: 1, mode: PdfIoMode.native);
      final ops = [
        solo.execute(_open()),
        solo.execute(_open()),
        solo.execute(_open()),
      ];
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;
      expect(
        [for (final j in lane.submitted) j.jobId],
        [1, 2, 3],
        reason:
            'jobs reach the lane in the order callers submitted '
            '— a lane runs them in mailbox order, so this IS the '
            'execution order',
      );
      lane.completeAllPending();
      await Future.wait(ops);
      await solo.dispose();
    });

    test('dispose during the pre-submit await spawns no zombie lane', () async {
      // The twin of the cancel-before-submit race: dispose lands while
      // execute is parked on its internal await. Without the disposed
      // re-check, execute resumes on a dead router and spawns a lane
      // that no kill will ever reach — it runs the job to completion
      // and holds its sources forever.
      final pending = router.execute(_open(), keepSources: {0});
      await router.dispose(); // same tick — no lane exists yet

      final result = await pending;
      expect(result.bytes, [2]);
      expect(
        host.lanes,
        isEmpty,
        reason:
            'a lane spawned after dispose is a zombie: never '
            'killed, leaking its held resources for the page life',
      );
    });
  });

  group('per-op cancellation', () {
    test('a pre-cancelled hook never reaches a lane', () async {
      final hook = CancelHook()..cancel();
      final result = await router.execute(_open(), cancel: hook);
      expect(result.bytes, [2]);
      expect(
        host.lanes,
        isEmpty,
        reason: 'cancelled before submit: no lane, no job',
      );
    });

    test('cancel during the pre-submit await still cancels cleanly', () async {
      // The race the integration suites can only hit by luck: cancel
      // lands while execute is parked on its internal await, before
      // the job exists anywhere.
      final hook = CancelHook();
      final pending = router.execute(_open(), cancel: hook);
      hook.cancel(); // same tick — submit has not happened yet
      final result = await pending;
      expect(result.bytes, [2]);
      expect(
        host.lanes,
        isEmpty,
        reason:
            're-check before submit must catch the same-tick '
            'cancel — no lane is placed, no job exists',
      );
    });

    test('cancel mid-job reaches the lane with the right jobId', () async {
      final hook = CancelHook();
      final pending = router.execute(_open(), cancel: hook);
      await Future<void>.delayed(Duration.zero); // job submitted
      final lane = host.lanes.single;
      expect(lane.submitted, hasLength(1));

      hook.cancel();
      expect(lane.cancelledJobs, [lane.submitted.single.jobId]);

      lane.pending[lane.submitted.single.jobId]!.complete(
        LaneSubmitResult(buildCancelledResponse(), const {}),
      );
      final result = await pending;
      expect(result.bytes, [2]);
    });

    test(
      'cancel after completion is unbound — the lane never hears it',
      () async {
        final hook = CancelHook();
        final lane = FakeLane()..autoResult = _ok();
        final soloHost = _SingleLaneHost(lane);
        final soloRouter = Router(
          host: soloHost,
          maxLanes: 1,
          mode: PdfIoMode.native,
        );

        await soloRouter.execute(_open(), cancel: hook);
        hook.cancel();
        expect(
          lane.cancelledJobs,
          isEmpty,
          reason: 'finished jobs are unbound; cancel becomes a no-op',
        );
      },
    );
  });
}

class _SingleLaneHost implements LaneHost {
  _SingleLaneHost(this.lane);
  final FakeLane lane;
  @override
  PdfIoMode get mode => PdfIoMode.native;
  @override
  int get defaultLaneCount => 1;
  @override
  Lane spawn() => lane;
}
