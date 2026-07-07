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
//      including after mid-op kills. Registered LAST so it also
//      sweeps everything the whole suite left behind.
//   3. Clean boot failure — an unreachable worker URL must surface a
//      typed error on the first op, never an infinite hang.

import 'dart:js_interop';
import 'dart:js_interop_unsafe';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;

import 'package:pdf_manipulator/src/runtime/web/lane.dart' show WebLaneHost;

import '../../../fixtures/handwritten.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

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
/// any stray entry at session level. A surviving worker DIRECTORY is
/// reported too (as 'session/worker/') — a retired worker's dir must
/// be reclaimed whole, not just emptied.
Future<List<String>> _laneResidue() async {
  final root = await web.window.navigator.storage.getDirectory().toDart;
  final web.FileSystemDirectoryHandle ours;
  try {
    ours = await root.getDirectoryHandle(WebLaneHost.opfsRootDir).toDart;
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
        if (files.isEmpty) {
          residue.add('$session/$worker/');
        } else {
          residue.addAll(files.map((f) => '$session/$worker/$f'));
        }
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
          WebLaneHost.opfsRootDir,
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
      final doc = await bare.open(src(minimalPdf));
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
            webLaneWorkerUrl: '${workerUrl()}.does-not-exist.js',
            webIoMode: mode,
          ),
        );
        await expectLater(
          broken.open(src(minimalPdf)),
          throwsA(isA<Object>()),
          reason:
              'a misconfigured deployment must produce an error, '
              'never an infinite hang',
        );
        await broken.dispose();

        // The broken URL also poisoned the page-level inheritance
        // cache — a configured instance must restore it, and work.
        final restored = Pdf(
          config: PdfConfig(webLaneWorkerUrl: workerUrl(), webIoMode: mode),
        );
        final doc = await restored.open(src(minimalPdf));
        expect(doc.pageCount, 1);
        await doc.dispose();
        await restored.dispose();
      },
      timeout: t(2),
    ); // boot failure reports explicitly — instant, no deadline involved

    test('dead-session OPFS directories are reclaimed at startup', () async {
      // Plant a fake dead session: files on disk, NO liveness lock —
      // exactly what a crashed tab (or a forgotten dispose + closed
      // page) leaves behind.
      final root = await web.window.navigator.storage.getDirectory().toDart;
      final ours = await root
          .getDirectoryHandle(
            WebLaneHost.opfsRootDir,
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
            'pdf_lane_orphan',
            web.FileSystemGetFileOptions(create: true),
          )
          .toDart;

      // The harness's own LIVE session must survive the sweep (its
      // lock is held); the dead one must vanish.
      final live = await WebLaneHost.sessionDir();
      await live
          .getFileHandle(
            'pdf_lane_canary',
            web.FileSystemGetFileOptions(create: true),
          )
          .toDart;

      await WebLaneHost.reclaimDeadSessions();

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
        contains('${WebLaneHost.opfsSessionId}/pdf_lane_canary'),
        reason: 'the sweep must NEVER touch a live session',
      );
      await live.removeEntry('pdf_lane_canary').toDart;
    }, timeout: t(1));

    test(
      'zero OPFS residue after kills, cancels, and the whole suite',
      () async {
        // The runner's shared instance still holds sources for any
        // handle a test left open (dispose-cascade covers those at
        // tearDownAll — AFTER this check). Retire it first so the
        // verdict below judges completed work only. Idempotent: the
        // runner's own tearDownAll dispose becomes a no-op.
        await disposeSharedInstance();

        final pdf = Pdf(
          config: PdfConfig(webLaneWorkerUrl: workerUrl(), webIoMode: mode),
        );

        // Exercise the leak-prone paths once more, deliberately:
        // a held source (open + dispose) and a mid-flight kill.
        final doc = await pdf.open(src(minimalPdf));
        await doc.dispose();
        final victim = Pdf(
          config: PdfConfig(webLaneWorkerUrl: workerUrl(), webIoMode: mode),
        );
        final inflight = victim.merge([
          src(minimalPdf),
          src(minimalPdf),
        ], TestSink());
        await victim.dispose();
        try {
          await inflight;
        } on PdfCancelled {
          // expected when the kill won the race
        }
        await pdf.dispose();

        // Sweeps are fire-and-forget AND convergent-with-backoff (the
        // browser reaps a dead worker's file handles on its own clock,
        // worst-case retry ≈ 6.4s) — the verifier must out-wait the
        // converger.
        var residue = await _laneResidue();
        final deadline = DateTime.now().add(const Duration(seconds: 12));
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
      timeout: t(1),
    );
  });
}
