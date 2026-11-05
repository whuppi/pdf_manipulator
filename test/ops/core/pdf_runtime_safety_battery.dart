// CHARTER — this battery alone proves the Router and the lanes under it
// keep their ownership promises: a cancel that races a completion leaves no
// engine handle behind, a stale handle id never names another document, a
// cancel of a queued job answers at once, and a lane whose engine died
// fails typed instead of serving on. It drives the Router over the real
// lanes of the platform under test.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/bridge/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/runtime/host.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/router.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:test/test.dart';

import '../../fixtures/handwritten.dart';
import '../../harness/slow_source.dart';
import '../../harness/test_source_sink.dart';
import '../../harness/timeouts.dart';

/// A lane host that hands every executor it boots to the test, and lets the
/// test act on each result the moment its lane delivers it.
class _ObservedHost implements ExecutorBooter {
  _ObservedHost(this._inner);

  final ExecutorBooter _inner;
  final executors = <_ObservedExecutor>[];

  /// Called with the job id each time a lane delivers a job's result,
  /// before the Router sees it.
  void Function(int jobId)? onResult;

  @override
  PdfIoMode get mode => _inner.mode;

  @override
  int get defaultConcurrency => _inner.defaultConcurrency;

  @override
  String get poolKey => _inner.poolKey;

  @override
  Future<Executor> boot() async {
    final executor = _ObservedExecutor(await _inner.boot(), this);
    executors.add(executor);
    return executor;
  }
}

class _ObservedExecutor implements Executor {
  _ObservedExecutor(this.real, this._host);

  final Executor real;
  final _ObservedHost _host;
  final lanes = <_ObservedLane>[];

  @override
  int get heapBytes => real.heapBytes;

  @override
  Lease beginLane() {
    final lane = _ObservedLane(real.beginLane(), _host);
    lanes.add(lane);
    return lane;
  }

  @override
  void terminate() => real.terminate();
}

class _ObservedLane implements Lease {
  _ObservedLane(this.real, this._host);

  final Lease real;
  final _ObservedHost _host;

  @override
  Future<JobResult> submit(Job job) => real.submit(job).then((result) {
    _host.onResult?.call(job.jobId);
    return result;
  });

  @override
  void cancelJob(int jobId) => real.cancelJob(jobId);

  @override
  void releaseHold(Object token) => real.releaseHold(token);

  @override
  Future<int?> reset() => real.reset();

  @override
  Future<bool> drain() => real.drain();
}

/// A sink whose writes take [delay], and which fires [onFirstWrite] as the
/// first one starts.
class _SlowSink implements DataSink {
  _SlowSink(this.delay, this.onFirstWrite);

  final Duration delay;
  final void Function() onFirstWrite;
  int inFlight = 0;
  int bytes = 0;
  bool _started = false;

  @override
  Future<void> write(Uint8List chunk) async {
    inFlight++;
    if (!_started) {
      _started = true;
      onFirstWrite();
    }
    await Future<void>.delayed(delay);
    bytes += chunk.length;
    inFlight--;
  }

  @override
  void abort(Object reason) {}

  @override
  void close() {}
}

Uint8List _open(DataSource source) => bin.encodeRequest('open', {
  'sourceLength': source.length,
  'password': null,
});

Uint8List _dispose(int handle) =>
    bin.encodeRequest('docDispose', {'handleId': handle});

/// Answers success only while the document behind [handle] is alive.
/// (A dispose says yes either way: it is idempotent.)
Uint8List _info(int handle) =>
    bin.encodeRequest('pageInfo', {'handleId': handle, 'pageIndices': <int>[]});

/// The error text of a failed response, empty for a success.
String _failure(Uint8List bytes) {
  try {
    bin.decodeResponse(bytes);
    return '';
  } catch (e) {
    return e.toString();
  }
}

bool _ok(Uint8List bytes) => bytes.isNotEmpty && bytes[0] == 1;

void registerRuntimeSafetyTests({PdfConfig? Function()? config}) {
  group('runtime safety', () {
    late _ObservedHost host;
    late ExecutorPool pool;
    late Router router;

    Router make(int maxLanes) {
      host = _ObservedHost(createBooter(config: config?.call()));
      pool = ExecutorPool(host: host);
      return router = Router(pool: pool, maxLanes: maxLanes, mode: host.mode);
    }

    tearDown(() async {
      await router.dispose();
      pool.dispose();
    });

    test('a cancel landing as the open succeeds leaves no document in the '
        'engine', () async {
      make(1);
      // A document of its own keeps the lane's executor leased, so the
      // probe below finds the engine as the cancelled open left it.
      final keeper = src(minimalPdf);
      await router.execute(_open(keeper), sources: [keeper], keepSources: {0});
      final hook = CancelHook();
      final source = src(minimalPdf);
      host.onResult = (_) {
        host.onResult = null;
        hook.cancel(); // after the engine finished, before the Router reads it
      };
      final result = await router.execute(
        _open(source),
        sources: [source],
        keepSources: {0},
        cancel: hook,
      );
      expect(result.bytes, [2], reason: 'the caller is told cancelled');

      // The lane's second document has engine id 2 (the keeper has 1).
      // Asking for it straight on the lane (past the Router) finds nothing
      // when the Router disposed it.
      final probe = Job(
        jobId: 1 << 20,
        request: _info(2),
        sources: const [],
        sinks: const [],
        keepSources: const {},
      );
      final answer = await host.executors.single.lanes.last.real.submit(probe);
      expect(
        _ok(answer.bytes),
        isFalse,
        reason: 'the document the cancelled open made is still in the engine',
      );
    }, timeout: t(10));

    test('a stale handle id never names another live document', () async {
      make(2);
      final a = src(minimalPdf);
      final b = src(minimalPdf);
      final opens = await Future.wait([
        router.execute(_open(a), sources: [a], keepSources: {0}),
        router.execute(_open(b), sources: [b], keepSources: {0}),
      ]);
      final staleId = peekResponseHandleId(opens[1].bytes)!;
      final d = src(minimalPdf);
      final third = await router.execute(
        _open(d),
        sources: [d],
        keepSources: {0},
      );
      final survivor = peekResponseHandleId(third.bytes)!;

      expect(_ok((await router.execute(_info(staleId))).bytes), isTrue);
      await router.execute(_dispose(staleId));
      final stale = await router.execute(_info(staleId));
      expect(_ok(stale.bytes), isFalse);

      expect(
        _ok((await router.execute(_info(survivor))).bytes),
        isTrue,
        reason: 'the stale id was forwarded and disposed another document',
      );
      final first = peekResponseHandleId(opens[0].bytes)!;
      expect(_ok((await router.execute(_info(first))).bytes), isTrue);
    }, timeout: t(10));

    test(
      'cancelling a queued job answers at once, behind a running one',
      () async {
        make(1);
        final slow = SlowSource(minimalPdf);
        final running = router.execute(
          _open(slow),
          sources: [slow],
          keepSources: {0},
          cancel: CancelHook(),
        );
        await slow.firstRead; // the lane is busy in job 1

        final hook = CancelHook();
        final waiting = src(minimalPdf);
        final queued = router.execute(
          _open(waiting),
          sources: [waiting],
          keepSources: {0},
          cancel: hook,
        );
        await Future<void>.delayed(const Duration(milliseconds: 20));

        final clock = Stopwatch()..start();
        hook.cancel();
        final result = await queued;
        clock.stop();
        expect(result.bytes, [2]);
        expect(
          clock.elapsedMilliseconds,
          lessThan(50),
          reason: 'the queued job waited for the running one',
        );

        slow.release();
        expect(_ok((await running).bytes), isTrue);
      },
      timeout: t(10),
    );

    test('an op that panics fails typed, ends its lane, and a new lane '
        'serves the next op', () async {
      make(1);
      final a = src(minimalPdf);
      final opened = await router.execute(
        _open(a),
        sources: [a],
        keepSources: {0},
      );
      final handle = peekResponseHandleId(opened.bytes)!;

      final panic = await router.execute(bin.encodeRequest('testPanic', {}));
      if (_failure(panic.bytes).contains('unknown op')) {
        markTestSkipped(
          'the engine was built without the test-support feature',
        );
        return;
      }
      expect(_ok(panic.bytes), isFalse);

      final clock = Stopwatch()..start();
      final orphan = await router.execute(_info(handle));
      expect(clock.elapsedMilliseconds, lessThan(1000));
      expect(_ok(orphan.bytes), isFalse, reason: 'its handle died with it');

      final b = src(minimalPdf);
      final next = await router.execute(
        _open(b),
        sources: [b],
        keepSources: {0},
      );
      expect(_ok(next.bytes), isTrue, reason: 'a new lane serves');
      expect(host.executors, hasLength(2));
    }, timeout: t(10));

    Future<int> openEditor(Router router) async {
      final source = src(minimalPdf);
      final opened = await router.execute(
        bin.encodeRequest('editorOpen', {
          'sourceLength': source.length,
          'password': null,
        }),
        sources: [source],
        keepSources: {0},
      );
      return peekResponseHandleId(opened.bytes)!;
    }

    test('a finished edit whose cancel fired after the engine finished reports '
        'success, and the edit is there', () async {
      make(1);
      final editor = await openEditor(router);

      final hook = CancelHook();
      host.onResult = (_) {
        host.onResult = null;
        hook.cancel(); // after the engine finished, before the Router reads it
      };
      final edit = await router.execute(
        bin.encodeRequest('editorMutate', {
          'handleId': editor,
          'editOp': 'setMetadata',
          'title': 'kept',
        }),
        cancel: hook,
      );
      expect(_ok(edit.bytes), isTrue, reason: 'the edit happened: not [2]');

      final modified = await router.execute(
        bin.encodeRequest('editorIsModified', {'handleId': editor}),
      );
      expect(bin.decodeResponse(modified.bytes)['modified'], isTrue);
    }, timeout: t(10));

    test('a save whose cancel fired after the engine finished resolves only '
        'after the sink took its last chunk', () async {
      make(1);
      final editor = await openEditor(router);

      final hook = CancelHook();
      final sink = _SlowSink(const Duration(milliseconds: 300), hook.cancel);
      final save = await router.execute(
        bin.encodeRequest('editorSave', {
          'handleId': editor,
          ...codec.encodeSaveArgs(const PdfSave.full()),
        }),
        sinks: [sink],
        cancel: hook,
      );
      if (_ok(save.bytes)) {
        expect(
          sink.inFlight,
          0,
          reason: 'told "saved" while a chunk was still being written',
        );
        expect(sink.bytes, greaterThan(0));
      } else {
        expect(save.bytes, [
          2,
        ], reason: 'cancelled mid-write is the other legal end');
      }
      if (config?.call()?.webIoMode == PdfIoMode.opfs) {
        // OPFS writes are fire-and-forget: the engine is long done when the
        // cancel lands, so the save succeeded and must have waited.
        expect(_ok(save.bytes), isTrue);
      }
    }, timeout: t(10));
  });
}
