// Router — the one place that decides, unit-tested against fake lanes.
//
// Everything here runs in milliseconds with no engine, no platform:
// placement (start-under-cap, least-loaded), response-driven pinning,
// held-resource lifecycle, instant dispose, and the per-op cancel
// binding (including the cancel-before-submit race the integration
// suites can only hit by luck).

@TestOn('vm')
library;

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:test/test.dart';

import '../harness/fake_executors.dart';

JobResult _ok({int? handleId, Map<int, Object> held = const {}}) {
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
  return JobResult(bytes, held);
}

Uint8List _open() => bin.encodeRequest('open', {'sourceLength': 1});
Uint8List _opOn(String op, int handleId) =>
    bin.encodeRequest(op, {'handleId': handleId});

void main() {
  late FakeLaneHost host;
  late ExecutorPool pool;
  late Router router;

  setUp(() {
    host = FakeLaneHost();
    pool = ExecutorPool(host: host, maxExecutors: 8);
    router = Router(pool: pool, maxLanes: 2, mode: PdfIoMode.native);
  });

  tearDown(() => pool.dispose());

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

      // The handle is gone: an op naming it is answered by the Router
      // itself and never placed on any lane.
      final submitted = laneA.submitted.length;
      final after = await router.execute(_opOn('extract', handle));
      expect(after.bytes, buildErrorResponse('document not found'));
      expect(laneA.submitted, hasLength(submitted));
      expect(host.lanes, hasLength(1));
    });

    test('handles on different lanes never share an id (cross-lane '
        'collision)', () async {
      // Two creates fire while the first is in-flight, so the second
      // starts a second lane (placement spreads them). Each lane's
      // engine mints handle id 1 — EngineState.next_handle restarts at 1
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

    test('a later job that keeps a source joins the handle\'s one resource, '
        'in the creating job\'s group', () async {
      final open = router.execute(_open(), keepSources: {0});
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;
      lane.pending[1]!.complete(_ok(handleId: 7, held: {0: 'editor-src'}));
      final opened = await open;
      final handle = peekResponseHandleId(opened.bytes)!;
      final editorResource = opened.resourceIds[0]!;
      expect(lane.submitted[0].holdGroupId, isNull);

      final merge = router.execute(
        _opOn('editorMergeFrom', handle),
        keepSources: {0},
      );
      await Future<void>.delayed(Duration.zero);
      expect(
        lane.submitted[1].holdGroupId,
        1,
        reason: 'the merge runs in the group of the job that made the handle',
      );
      lane.pending[2]!.complete(_ok(held: {0: 'merge-src'}));
      final merged = await merge;
      expect(
        merged.resourceIds[0],
        editorResource,
        reason: 'one resource id per handle',
      );

      await router.releaseSource(editorResource);
      expect(lane.releasedTokens, ['editor-src', 'merge-src']);
    });

    test(
      'a failed job registers nothing and frees its kept channels',
      () async {
        final open = router.execute(_open(), keepSources: {0});
        await Future<void>.delayed(Duration.zero);
        final lane = host.lanes.single;
        lane.pending[1]!.complete(_ok(handleId: 7, held: {0: 'editor-src'}));
        final opened = await open;
        final handle = peekResponseHandleId(opened.bytes)!;

        final merge = router.execute(
          _opOn('editorMergeFrom', handle),
          keepSources: {0},
        );
        await Future<void>.delayed(Duration.zero);
        lane.pending[2]!.complete(
          JobResult(buildErrorResponse('bad pdf'), const {0: 'merge-src'}),
        );
        final failed = await merge;

        expect(failed.resourceIds, isEmpty);
        expect(lane.releasedTokens, [
          'merge-src',
        ], reason: 'the failed merge\'s channel is freed, the editor\'s stays');

        final failedOpen = router.execute(_open(), keepSources: {0});
        await Future<void>.delayed(Duration.zero);
        lane.pending[3]!.complete(
          JobResult(buildErrorResponse('bad pdf'), const {0: 'open-src'}),
        );
        expect((await failedOpen).resourceIds, isEmpty);
        expect(lane.releasedTokens, ['merge-src', 'open-src']);
      },
    );
  });

  group('dispose', () {
    test('ends every lane and resolves later ops as cancelled', () async {
      final inflight = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;

      await router.dispose();
      expect(lane.ended, isTrue, reason: 'the lane was ended at once');

      // A real lane completes its pending submits when it ends; the fake
      // mimics that contract in drain.
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
      final solo = Router(pool: pool, maxLanes: 1, mode: PdfIoMode.native);
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
      // re-check, execute resumes on a dead router and starts a lane
      // that nothing will ever end — it runs the job to completion
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
        JobResult(buildCancelledResponse(), const {}),
      );
      final result = await pending;
      expect(result.bytes, [2]);
    });

    test(
      'cancel after completion is unbound — the lane never hears it',
      () async {
        final hook = CancelHook();
        final soloHost = FakeLaneHost()
          ..onBoot = (executor) => executor.lane.autoResult = _ok();
        final soloPool = ExecutorPool(host: soloHost);
        addTearDown(soloPool.dispose);
        final soloRouter = Router(
          pool: soloPool,
          maxLanes: 1,
          mode: PdfIoMode.native,
        );

        await soloRouter.execute(_open(), cancel: hook);
        final lane = soloHost.lanes.single;
        hook.cancel();
        expect(
          lane.cancelledJobs,
          isEmpty,
          reason: 'finished jobs are unbound; cancel becomes a no-op',
        );
      },
    );
  });

  group('a handle the Router does not hold', () {
    test('a stale id is answered not-found and never reaches a lane', () async {
      final open = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;
      lane.pending[1]!.complete(_ok(handleId: 1));
      final stale = peekResponseHandleId((await open).bytes)!;

      lane.autoResult = _ok();
      await router.execute(_opOn('docDispose', stale));
      final sent = lane.submitted.length;

      // Another document now lives on the same lane under the engine id
      // the stale handle had: forwarded raw, the op would name it.
      lane.autoResult = _ok(handleId: 2);
      await router.execute(_open());

      final result = await router.execute(_opOn('extract', stale));
      expect(
        result.bytes,
        buildErrorResponse('document not found'),
        reason: 'the engine-shaped not-found error, built by the Router',
      );
      expect(lane.submitted.length, sent + 1, reason: 'only the second open');

      final editor = await router.execute(_opOn('editorSave', stale));
      expect(editor.bytes, buildErrorResponse('editor not found'));
      final builder = await router.execute(_opOn('builderSave', 999));
      expect(builder.bytes, buildErrorResponse('builder not found'));
    });
  });

  group('cancel versus completion', () {
    test('a handle made in the window of a cancel is disposed, not leaked, '
        'and the caller is told cancelled', () async {
      final hook = CancelHook();
      final open = router.execute(_open(), keepSources: {0}, cancel: hook);
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;

      // The cancel lands after the engine finished, before the Router
      // reads the success.
      hook.cancel();
      lane.pending[1]!.complete(_ok(handleId: 7, held: {0: 'src'}));
      final result = await open;
      expect(result.bytes, [2]);
      expect(result.resourceIds, isEmpty);

      expect(lane.submitted, hasLength(2));
      final dispose = lane.submitted[1];
      expect(peekRequestOp(dispose.request), 'docDispose');
      expect(
        peekRequestHandleId(dispose.request),
        7,
        reason: 'the dispose names the engine id, on the same lane',
      );
      expect(
        lane.releasedTokens,
        isEmpty,
        reason: 'the channels stay until the document that reads them is gone',
      );

      lane.pending[dispose.jobId]!.complete(_ok());
      await Future<void>.delayed(Duration.zero);
      expect(lane.releasedTokens, ['src']);
    });

    test('an editor and a builder are disposed by their own ops', () async {
      for (final (create, disposeOp) in [
        ('editorOpen', 'editorDispose'),
        ('builderCreate', 'builderDispose'),
      ]) {
        final hook = CancelHook();
        final op = router.execute(bin.encodeRequest(create, {}), cancel: hook);
        await Future<void>.delayed(Duration.zero);
        final lane = host.lanes.last;
        hook.cancel();
        lane.pending[lane.submitted.last.jobId]!.complete(_ok(handleId: 3));
        await op;
        expect(peekRequestOp(lane.submitted.last.request), disposeOp);
        lane.completeAllPending();
      }
    });

    test('a finished edit on a live editor reports its success, not cancelled, '
        'even though the cancel fired', () async {
      final open = router.execute(bin.encodeRequest('editorOpen', {}));
      await Future<void>.delayed(Duration.zero);
      final lane = host.lanes.single;
      lane.pending[1]!.complete(_ok(handleId: 4));
      final editor = peekResponseHandleId((await open).bytes)!;

      final hook = CancelHook();
      final edit = router.execute(
        bin.encodeRequest('editorMutate', {
          'handleId': editor,
          'editOp': 'setMetadata',
          'title': 'x',
        }),
        cancel: hook,
      );
      await Future<void>.delayed(Duration.zero);
      hook.cancel();
      lane.pending[2]!.complete(_ok());
      expect(
        (await edit).bytes,
        _ok().bytes,
        reason: 'the editor holds the edit, so the caller must hear success',
      );
      expect(lane.submitted, hasLength(2), reason: 'nothing is disposed');
    });

    test(
      'a dispose that finished under a fired cancel still drops the pin',
      () async {
        final open = router.execute(_open());
        await Future<void>.delayed(Duration.zero);
        final lane = host.lanes.single;
        lane.pending[1]!.complete(_ok(handleId: 7));
        final handle = peekResponseHandleId((await open).bytes)!;

        final hook = CancelHook();
        final dispose = router.execute(
          _opOn('docDispose', handle),
          cancel: hook,
        );
        await Future<void>.delayed(Duration.zero);
        hook.cancel();
        lane.pending[2]!.complete(_ok());
        expect((await dispose).bytes, _ok().bytes);
        final after = await router.execute(_opOn('extract', handle));
        expect(after.bytes, buildErrorResponse('document not found'));
      },
    );

    test('a handle from an op that creates none is refused loudly', () async {
      final hook = CancelHook();
      final op = router.execute(
        bin.encodeRequest('extract', {'sourceLength': 1}),
        cancel: hook,
      );
      await Future<void>.delayed(Duration.zero);
      hook.cancel();
      host.lanes.single.pending[1]!.complete(_ok(handleId: 9));
      await expectLater(op, throwsStateError);
    });
  });

  group('not-found kinds', () {
    test('every editor and builder op answers its own kind', () async {
      for (final op in EngineOp.values) {
        final wire = op.wire;
        final isEditor = wire.startsWith('editor') && wire != 'editorOpen';
        final isBuilder = wire.startsWith('builder') && wire != 'builderCreate';
        if (!isEditor && !isBuilder) continue;
        final result = await router.execute(_opOn(wire, 12345));
        expect(
          result.bytes,
          buildErrorResponse(
            isEditor ? 'editor not found' : 'builder not found',
          ),
          reason: '$wire is missing from the Router\'s op sets',
        );
      }
    });
  });

  group('a lane whose engine died', () {
    JobResult died() => JobResult(
      buildErrorResponse('internal error: engine operation panicked'),
      const {},
      laneDied: true,
    );

    test('its handles fail typed at once, a new lane serves handle-less '
        'ops, and the dead lane gives up its executor once drained', () async {
      final open = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      final laneA = host.lanes.single;
      laneA.pending[1]!.complete(_ok(handleId: 5));
      final handle = peekResponseHandleId((await open).bytes)!;

      final bad = router.execute(_opOn('extract', handle));
      final queued = router.execute(_opOn('extract', handle));
      await Future<void>.delayed(Duration.zero);
      laneA.pending[2]!.complete(died());
      expect(
        (await bad).bytes,
        buildErrorResponse('internal error: engine operation panicked'),
      );
      expect(laneA.ended, isFalse, reason: 'a queued job is still to answer');
      laneA.engineDead = true;
      laneA.pending[3]!.complete(
        JobResult(buildErrorResponse(engineDeadMessage), const {}),
      );
      expect((await queued).bytes, buildErrorResponse(engineDeadMessage));
      expect(laneA.ended, isTrue);
      await Future<void>.delayed(Duration.zero);
      expect(
        host.executors[0].terminated,
        isTrue,
        reason: 'the pool drops a dead executor and never leases it again',
      );

      final before = laneA.submitted.length;
      final after = await router.execute(_opOn('extract', handle));
      expect(after.bytes, buildErrorResponse(engineDeadMessage));
      expect(laneA.submitted, hasLength(before), reason: 'never forwarded');

      final dispose = await router.execute(_opOn('docDispose', handle));
      expect(dispose.bytes[0], 1, reason: 'freeing a dead handle succeeds');

      final fresh = router.execute(_open());
      await Future<void>.delayed(Duration.zero);
      expect(host.lanes, hasLength(2), reason: 'a replacement lane');
      host.lanes.last.pending.values.single.complete(_ok(handleId: 1));
      expect((await fresh).bytes[0], 1);
    });
  });

  group('the lease', () {
    /// Lets the pool and the lane settle: a release and a lease are a few
    /// microtasks.
    Future<void> settle() => Future<void>.delayed(Duration.zero);

    test('a lane leases at its first job, not before', () async {
      await settle();
      expect(host.executors, isEmpty);

      final op = router.execute(_open());
      await settle();
      expect(host.executors.single.events, ['lease']);
      host.lanes.single.pending[1]!.complete(_ok());
      await op;
    });

    test('idle with no pin and no kept source: the executor is reset and '
        'returned', () async {
      final op = router.execute(_open());
      await settle();
      host.lanes.single.pending[1]!.complete(_ok());
      await op;
      await settle();

      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
      expect(pool.idleCount, 1);
      expect(host.executors.single.terminated, isFalse);
    });

    test('the next job leases again, on the same executor', () async {
      final first = router.execute(_open());
      await settle();
      host.lanes.single.pending[1]!.complete(_ok());
      await first;
      await settle();

      final second = router.execute(_open());
      await settle();
      expect(host.executors, hasLength(1));
      expect(host.executors.single.events, [
        'lease',
        'drain',
        'reset',
        'lease',
      ]);
      host.lanes.single.pending[2]!.complete(_ok());
      await second;
    });

    test('a pinned handle keeps the lease until its dispose', () async {
      final open = router.execute(_open());
      await settle();
      host.lanes.single.pending[1]!.complete(_ok(handleId: 7));
      final handle = peekResponseHandleId((await open).bytes)!;
      await settle();
      expect(host.executors.single.events, [
        'lease',
      ], reason: 'the engine holds the document: no reset');

      host.lanes.single.autoResult = _ok();
      await router.execute(_opOn('docDispose', handle));
      await settle();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
    });

    test('a kept source keeps the lease until it is released', () async {
      final open = router.execute(_open(), keepSources: {0});
      await settle();
      host.lanes.single.pending[1]!.complete(
        _ok(handleId: 7, held: {0: 'token'}),
      );
      final opened = await open;
      final handle = peekResponseHandleId(opened.bytes)!;

      host.lanes.single.autoResult = _ok();
      await router.execute(_opOn('docDispose', handle));
      await settle();
      expect(host.executors.single.events, [
        'lease',
      ], reason: 'the engine let go of the document, the source is still held');

      await router.releaseSource(opened.resourceIds[0]!);
      await settle();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
    });

    test('a queued job on the lane keeps the lease', () async {
      final solo = Router(pool: pool, maxLanes: 1, mode: PdfIoMode.native);
      final first = solo.execute(_open());
      final second = solo.execute(_open());
      await settle();
      final lane = host.lanes.single;

      lane.pending[1]!.complete(_ok());
      await first;
      await settle();
      expect(host.executors.single.events, ['lease']);

      lane.pending[2]!.complete(_ok());
      await second;
      await settle();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
    });

    test('a failed job with a kept source gives its channel back, then the '
        'executor', () async {
      final op = router.execute(_open(), keepSources: {0});
      await settle();
      host.lanes.single.pending[1]!.complete(
        JobResult(buildErrorResponse('bad pdf'), const {0: 'src'}),
      );
      await op;
      await settle();
      expect(host.lanes.single.releasedTokens, ['src']);
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
    });

    test(
      'dispose mid-job drains the executor and returns it to the pool',
      () async {
        final inflight = router.execute(_open());
        await settle();
        final lane = host.lanes.single;

        await router.dispose();
        expect(
          (await inflight).bytes,
          buildCancelledResponse(),
          reason: 'the caller is answered at once',
        );
        await settle();
        expect(host.executors.single.events, ['lease', 'drain', 'reset']);
        expect(host.executors.single.terminated, isFalse);
        expect(pool.idleCount, 1);
        expect(lane.ended, isTrue);
      },
    );

    test('dispose waits for a running job to post before the reset', () async {
      final inflight = router.execute(_open());
      await settle();
      final lane = host.lanes.single;
      lane.drainGate = Completer<bool>();

      await router.dispose();
      await inflight;
      await settle();
      expect(host.executors.single.events, ['lease', 'drain']);
      expect(pool.idleCount, 0, reason: 'the engine is still unwinding');

      lane.drainGate!.complete(true);
      await settle();
      expect(host.executors.single.events, ['lease', 'drain', 'reset']);
      expect(pool.idleCount, 1);
    });

    test('dispose while a job waits for its executor cancels it and keeps '
        'nothing', () async {
      host.bootGate = Completer<void>();
      final waiting = router.execute(_open());
      await settle();

      await router.dispose();
      expect((await waiting).bytes, buildCancelledResponse());

      host.bootGate!.complete();
      await settle();
      expect(host.lanes.single.submitted, isEmpty, reason: 'no zombie job');
      expect(pool.idleCount, 1, reason: 'the executor that arrived is idle');
    });

    test('the prelude replays under every lease and runs first', () async {
      final op = router.execute(_open());
      await settle();
      host.lanes.single.pending[1]!.complete(_ok());
      await op;
      await settle();

      final installed = await router.installPrelude(
        bin.encodeRequest('registerFallbackFont', {'kind': 'cjk'}),
        sourceBytes: Uint8List.fromList(const [1, 2, 3]),
      );
      expect(installed, isEmpty, reason: 'no lane holds an executor now');

      final next = router.execute(_open());
      await settle();
      final lane = host.lanes.single;
      expect(
        [for (final j in lane.submitted) peekRequestOp(j.request)],
        ['open', 'registerFallbackFont', 'open'],
      );
      lane.pending[lane.submitted.last.jobId]!.complete(_ok());
      await next;
    });
  });
}
