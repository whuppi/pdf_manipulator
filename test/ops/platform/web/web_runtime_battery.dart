// Web-only runtime guarantees — registered by the three web runners.
//
// The web twin of platform/native/ (the process-death canary): each
// platform folder holds the guarantees only that platform can break.
// Web's are:
//
//   1. Page-level config inheritance — a bare Pdf() created after a
//      configured one must find the worker assets (real apps create
//      secondary instances without re-passing config).
//   2. Zero OPFS residue — pre-copied source files must not leak,
//      including after a dispose mid-op. Registered LAST so it also
//      sweeps everything the whole suite left behind.
//   3. Clean boot failure — an unreachable worker URL must surface a
//      typed error on the first op, never an infinite hang.

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import 'package:pdf_manipulator/src/runtime/pool.dart' show ExecutorPool;
import 'package:pdf_manipulator/src/runtime/web/lease.dart'
    show WebLease, WebExecutorBooter;

import '../../../fixtures/handwritten.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

/// How long a terminated worker's directory may take to be deleted: the
/// host retries its delete with a doubling delay, 12.8 s in all before it
/// gives up.
const _reapBudget = Duration(seconds: 15);

/// A sink whose first write blocks until [gate] completes.
class _BlockedSink implements DataSink {
  _BlockedSink(this.started, this.gate);

  final Completer<void> started;
  final Future<void> gate;

  @override
  Future<void> write(Uint8List chunk) async {
    if (!started.isCompleted) started.complete();
    await gate;
  }

  @override
  void abort(Object reason) {}

  @override
  void close() {}
}

Future<List<String>> _names(web.FileSystemDirectoryHandle dir) async {
  final names = <String>[];
  final iter = (dir as JSObject).callMethod<JSObject>('keys'.toJS);
  while (true) {
    final next = await iter.callMethod<JSPromise<JSObject>>('next'.toJS).toDart;
    if ((next['done']! as JSBoolean).toDart) break;
    names.add((next['value']! as JSString).toDart);
  }
  return names;
}

/// Everything under the package's OPFS directory, as full paths:
/// pre-copy files inside worker dirs ('session/worker/file'), plus
/// any stray entry at session level. An empty worker directory is the home
/// of a pooled worker that is alive and idle, so it is not residue.
Future<List<String>> _laneResidue() async {
  final root = await web.window.navigator.storage.getDirectory().toDart;
  final web.FileSystemDirectoryHandle ours;
  try {
    ours = await root.getDirectoryHandle(WebExecutorBooter.opfsRootDir).toDart;
  } catch (_) {
    return const []; // directory never created — perfectly clean
  }
  final residue = <String>[];
  for (final session in await _names(ours)) {
    final dir = await ours.getDirectoryHandle(session).toDart;
    for (final worker in await _names(dir)) {
      try {
        final wdir = await dir.getDirectoryHandle(worker).toDart;
        final files = await _names(wdir);
        residue.addAll(files.map((f) => '$session/$worker/$f'));
      } catch (_) {
        residue.add('$session/$worker'); // a file at session level
      }
    }
  }
  return residue;
}

/// Deletes the package's whole OPFS directory. Runners call this at
/// suite start: Chrome's OPFS can persist across test runs, and the
/// end-of-suite residue check must judge THIS run only.
Future<void> purgeOpfsLaneFiles() async {
  final root = await web.window.navigator.storage.getDirectory().toDart;
  try {
    await root
        .removeEntry(
          WebExecutorBooter.opfsRootDir,
          web.FileSystemRemoveOptions(recursive: true),
        )
        .toDart;
  } catch (_) {
    // Never created — fine.
  }
}

void registerWebPlatformTests({
  required String Function() workerUrl,
  required PdfIoMode mode,
  required Future<void> Function() disposeSharedInstance,
}) {
  group('web platform', () {
    test('bare Pdf() inherits the page-level web config', () async {
      // The runner's first instance carried an explicit workerUrl;
      // this one carries NOTHING. Without inheritance it would look
      // for assets at the default path, which this test server does
      // not serve — boot would fail.
      final bare = Pdf();
      final doc = await bare.open(PdfInput(src(minimalPdf)));
      expect(doc.pageCount, 1);
      expect(
        bare.ioMode,
        mode,
        reason: 'the forced I/O mode must be inherited too',
      );
      await doc.dispose();
      await bare.dispose();
    }, timeout: t(1));

    test(
      'unreachable worker URL fails cleanly — no hang, page recovers',
      () async {
        final broken = Pdf(
          config: PdfConfig(
            webWorkerUrl: '${workerUrl()}.does-not-exist.js',
            webIoMode: mode,
          ),
        );
        await expectLater(
          broken.open(PdfInput(src(minimalPdf))),
          throwsA(isA<Object>()),
          reason:
              'a misconfigured deployment must produce an error, '
              'never an infinite hang',
        );
        await broken.dispose();

        // The broken URL also poisoned the page-level inheritance
        // cache — a configured instance must restore it, and work.
        final restored = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
        );
        final doc = await restored.open(PdfInput(src(minimalPdf)));
        expect(doc.pageCount, 1);
        await doc.dispose();
        await restored.dispose();
      },
      timeout: t(2),
    ); // boot failure reports explicitly — instant, no deadline involved

    test(
      'a failed boot does not poison the lane — the next op boots again',
      () async {
        final channel = spawnHybridUri(
          '/test/harness/flaky_asset_server.dart',
          stayAlive: true,
        );
        final events = StreamIterator(channel.stream);
        await events.moveNext();
        final port = (events.current as double).toInt();
        channel.sink.add('fail');
        await events.moveNext(); // the server confirms the switch
        final url = 'http://localhost:$port/web_assets/executor_worker.js';

        final flaky = Pdf(
          config: PdfConfig(webWorkerUrl: url, webIoMode: mode, concurrency: 1),
        );
        await expectLater(
          flaky.open(PdfInput(src(minimalPdf))),
          throwsA(isA<Object>()),
          reason: 'the worker script is unreachable',
        );

        channel.sink.add('ok');
        await events.moveNext();
        final doc = await flaky.open(PdfInput(src(minimalPdf)));
        expect(doc.pageCount, 1, reason: 'the same Pdf boots again and works');
        await doc.dispose();
        await flaky.dispose();
        await events.cancel();

        // The failed boot left the page-level inheritance pointing at the
        // flaky server; point it back at the suite's.
        final restored = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
        );
        await (await restored.open(PdfInput(src(minimalPdf)))).dispose();
        await restored.dispose();
      },
      timeout: t(3),
    );

    test(
      'a cancel during a blocked sink write answers within 100 ms (JSPI)',
      () async {
        final pdf = Pdf(
          config: PdfConfig(
            webWorkerUrl: workerUrl(),
            webIoMode: mode,
            concurrency: 1,
          ),
        );
        final started = Completer<void>();
        final release = Completer<void>();
        final sink = _BlockedSink(started, release.future);
        final task = pdf.merge([
          PdfInput(src(minimalPdf)),
          PdfInput(src(minimalPdf)),
        ], sink);
        await started.future; // the worker is suspended inside the write
        final clock = Stopwatch()..start();
        task.cancel();
        await expectLater(task, throwsA(isA<PdfCancelled>()));
        clock.stop();
        expect(
          clock.elapsedMilliseconds,
          lessThan(100),
          reason:
              'the cancel waited for the sink instead of answering the write',
        );
        release.complete();
        await pdf.dispose();
      },
      timeout: t(10),
      skip: mode == PdfIoMode.jspi ? false : 'JSPI only',
    );

    test(
      'a sink that never returns does not make a lane undrainable',
      () async {
        // Its own pool, so the Pdf below can only be served by the worker
        // the first one left: any other idle worker would hide the hang.
        final config = PdfConfig(
          webWorkerUrl: '${workerUrl()}?hungSink',
          webIoMode: mode,
        );
        final victim = Pdf(config: config);
        final started = Completer<void>();
        final task = victim.merge([
          PdfInput(src(minimalPdf)),
          PdfInput(src(minimalPdf)),
        ], _BlockedSink(started, Completer<void>().future));
        await started.future; // a write is in flight and never returns
        await victim.dispose();
        await expectLater(task, throwsA(isA<PdfCancelled>()));

        final fresh = Pdf(config: config);
        addTearDown(fresh.dispose);
        final pages =
            await () async {
              final doc = await fresh.open(PdfInput(src(minimalPdf)));
              return doc.pageInfo();
            }().timeout(
              const Duration(seconds: 10),
              onTimeout: () => throw TestFailure(
                'the worker never left draining: a sink write that never '
                'returned kept the lane busy',
              ),
            );
        expect(pages, isNotEmpty);
      },
      timeout: t(20),
    );

    test('dead-session OPFS directories are reclaimed at startup', () async {
      // Plant a fake dead session: files on disk, NO liveness lock —
      // exactly what a crashed tab (or a forgotten dispose + closed
      // page) leaves behind.
      final root = await web.window.navigator.storage.getDirectory().toDart;
      final ours = await root
          .getDirectoryHandle(
            WebExecutorBooter.opfsRootDir,
            web.FileSystemGetDirectoryOptions(create: true),
          )
          .toDart;
      final dead = await ours
          .getDirectoryHandle(
            'deadsession00000001',
            web.FileSystemGetDirectoryOptions(create: true),
          )
          .toDart;
      await dead
          .getFileHandle(
            'pdf_executor_orphan',
            web.FileSystemGetFileOptions(create: true),
          )
          .toDart;

      // The harness's own LIVE session must survive the sweep (its
      // lock is held); the dead one must vanish.
      final live = await WebExecutorBooter.sessionDir();
      await live
          .getFileHandle(
            'pdf_executor_canary',
            web.FileSystemGetFileOptions(create: true),
          )
          .toDart;

      await WebExecutorBooter.reclaimDeadSessions();

      final after = await _laneResidue();
      expect(
        after.where((p) => p.startsWith('deadsession')),
        isEmpty,
        reason:
            'a session with no liveness lock is dead — its '
            'files are garbage and must be reclaimed',
      );
      expect(
        after,
        contains('${WebExecutorBooter.opfsSessionId}/pdf_executor_canary'),
        reason: 'the sweep must NEVER touch a live session',
      );
      await live.removeEntry('pdf_executor_canary').toDart;
    }, timeout: t(1));

    test('the directory a 5.x app left under the old root name is removed at '
        'startup, unless a live session still holds it', () async {
      // The name and layout 5.x used: root/{session}/{file}, with the
      // session's liveness lock named root/{session}. The literal is the
      // historic name on purpose; renaming the constant must not move it.
      const oldRoot = 'pdf_manipulator_lanes';
      const dead = 'deadsession00000002';
      const alive = 'livesession00000003';
      final top = await web.window.navigator.storage.getDirectory().toDart;
      Future<bool> exists() async {
        try {
          await top.getDirectoryHandle(oldRoot).toDart;
          return true;
        } catch (_) {
          return false;
        }
      }

      final create = web.FileSystemGetDirectoryOptions(create: true);
      final old = await top.getDirectoryHandle(oldRoot, create).toDart;
      for (final session in [dead, alive]) {
        final dir = await old.getDirectoryHandle(session, create).toDart;
        await dir
            .getFileHandle(
              'leftover',
              web.FileSystemGetFileOptions(create: true),
            )
            .toDart;
      }

      // A 5.x page that is still open holds its session's lock.
      final release = Completer<void>();
      final granted = Completer<void>();
      unawaited(
        web.window.navigator.locks
            .request(
              '$oldRoot/$alive',
              ((web.Lock? lock) {
                granted.complete();
                return release.future.toJS;
              }).toJS,
            )
            .toDart,
      );
      await granted.future;

      try {
        await WebExecutorBooter.reclaimDeadSessions();
        expect(
          await _names(old),
          [alive],
          reason:
              'the session with no lock is dead and goes; the one whose '
              'lock is held is live and stays, and keeps its root',
        );
      } finally {
        release.complete();
      }

      // The browser frees the lock a moment after the holder returns.
      final deadline = DateTime.now().add(const Duration(seconds: 5));
      while (await exists() && DateTime.now().isBefore(deadline)) {
        await WebExecutorBooter.reclaimDeadSessions();
        await Future<void>.delayed(const Duration(milliseconds: 50));
      }
      expect(
        await exists(),
        isFalse,
        reason: 'with no live session left the old root itself is removed',
      );
    }, timeout: t(10));

    test(
      'zero OPFS residue after disposes, cancels, and the whole suite',
      () async {
        // The runner's shared instance still holds sources for any
        // handle a test left open (dispose-cascade covers those at
        // tearDownAll — AFTER this check). Retire it first so the
        // verdict below judges completed work only. Idempotent: the
        // runner's own tearDownAll dispose becomes a no-op.
        await disposeSharedInstance();

        final pdf = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
        );

        // Exercise the leak-prone paths once more, deliberately:
        // a held source (open + dispose) and a mid-flight dispose.
        final doc = await pdf.open(PdfInput(src(minimalPdf)));
        await doc.dispose();
        final victim = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
        );
        final inflight = victim.merge([
          PdfInput(src(minimalPdf)),
          PdfInput(src(minimalPdf)),
        ], TestSink());
        await victim.dispose();
        try {
          await inflight;
        } on PdfCancelled {
          // expected when the dispose won the race
        }
        await pdf.dispose();

        // Two kinds of worker can hold files, and only one of them needs
        // the browser. A live worker is idle in the pool once its lane's
        // reset has answered, so its directory is empty at once. A
        // terminated worker's directory is deleted when the browser has
        // reaped its handles, on the browser's clock: the only wait here.
        // Leasing every pooled executor waits, event by event, for each
        // lane still draining and resetting, and names the live workers.
        final pool = ExecutorPool.of(
          WebExecutorBooter(laneWorkerUrl: workerUrl(), forceMode: mode),
        );
        final leases = [
          for (var i = pool.liveCount; i > 0; i--) await pool.acquire().lease,
        ];
        final live = {
          for (final lease in leases) (lease.lane as WebLease).executor.opfsDir,
        };
        for (final lease in leases) {
          await pool.release(lease, untouched: true);
        }
        final unclean = (await _laneResidue()).where(
          (path) => live.contains(path.split('/')[1]),
        );
        expect(
          unclean,
          isEmpty,
          reason: 'a live worker kept a pre-copy file after its reset',
        );

        var residue = await _laneResidue();
        final deadline = DateTime.now().add(_reapBudget);
        while (residue.isNotEmpty && DateTime.now().isBefore(deadline)) {
          await Future<void>.delayed(const Duration(milliseconds: 100));
          residue = await _laneResidue();
        }
        expect(
          residue,
          isEmpty,
          reason:
              'OPFS pre-copy files must never outlive their jobs '
              '(jspi/atomics modes must never create any at all)',
        );
      },
      timeout: t(30),
    );

    test('no worker was lost to a failed reset', () {
      // A failed reset retires its worker and the next lease boots a new
      // one, so it never shows in a result: only in this count.
      expect(
        ExecutorPool.totalResetFailures,
        0,
        reason: 'a reset failed and cost a cold boot',
      );
    }, timeout: t(1));

    test('no boot or drain outlasted its cover', () {
      // A transition that outlasts the cover stops hiding the requests
      // behind it, which then boot their own executor: the suite passes
      // and a worker is stuck. Only this count shows it.
      for (final slow in ExecutorPool.recentSlowTransitions) {
        printOnFailure('slow transition: $slow');
      }
      expect(
        ExecutorPool.totalSlowTransitions,
        0,
        reason: 'an executor stayed booting or draining past coverFor',
      );
    }, timeout: t(1));

    test('no pre-copy file was set aside and no create retried', () {
      // The browser sometimes leaves createSyncAccessHandle on a fresh file
      // unanswered; the worker then sets the file aside or retries on a new
      // one, and the suite still passes. Only these counts show it.
      for (final file in WebExecutorBooter.abandonedFiles) {
        printOnFailure('set-aside pre-copy file: $file');
      }
      expect(
        WebExecutorBooter.abandonedFiles,
        isEmpty,
        reason: 'a worker still waits for the browser to create a handle',
      );
      expect(
        WebExecutorBooter.retriedCreates,
        0,
        reason: 'createSyncAccessHandle passed kCreateHandleWait unanswered',
      );
    }, timeout: t(1));

    test('no dead worker directory was given up on', () {
      for (final dir in WebExecutorBooter.abandonedWorkerDirs) {
        printOnFailure('abandoned worker directory: $dir');
      }
      expect(
        WebExecutorBooter.abandonedWorkerDirs,
        isEmpty,
        reason: 'the host stopped retrying the delete of a dead worker dir',
      );
    }, timeout: t(1));
  });
}
