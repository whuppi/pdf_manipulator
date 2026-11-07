// Web-only: the backing of a reader in the worker. A BlobSource is read in
// the worker with no main-thread read; a small source of any other kind is
// copied into the worker once; a source past the lane's transfer budget takes
// the mode's own path.
//
// Every main-thread move is observed from outside: a spy on
// Worker.postMessage records each message sent to a worker (and the shape of
// each exec), Blob.prototype.arrayBuffer counts the main-thread reads of a
// Blob, and a listener on the worker counts the reads it asked for.

import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:pdf_manipulator/pdf_manipulator.dart';
import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/bridge/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/web/lease.dart';
import 'package:pdf_manipulator/src/runtime/web/protocol.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/web.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;
import 'package:web/web.dart' show EventStreamProviders;

import '../../../fixtures/generated/fixtures.dart';
import '../../../fixtures/handwritten.dart';
import '../../../harness/slow_source.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';
import '../../../harness/worker_prelude.dart';

Uint8List _open(int sourceLength) =>
    bin.encodeRequest('open', {'sourceLength': sourceLength, 'password': null});

Uint8List _info(int handle) => bin.encodeRequest('pageInfo', {
  'handleId': handle,
  'pageIndices': [0],
});

bool _ok(Uint8List bytes) => bytes.isNotEmpty && bytes[0] == 1;

/// The error a failed response carries.
Object? _failure(Uint8List bytes) {
  try {
    bin.decodeResponse(bytes);
    return null;
  } catch (e) {
    return e;
  }
}

/// Spy on the page: every message posted to a worker by type, the shape of
/// every exec (one character per source: 1 when that source has a copy, a
/// Blob, or a pre-copy file), and the main-thread reads of a Blob.
const _spySource = r'''
(function () {
  const spy = { sent: {}, execs: [], blobReads: 0, onExec: null };
  const flags = (list) => (list ? Array.from(list, (x) => (x != null ? '1' : '0')).join('') : '');
  const post = Worker.prototype.postMessage;
  Worker.prototype.postMessage = function (m, ...rest) {
    if (m && m.type) {
      spy.sent[m.type] = (spy.sent[m.type] || 0) + 1;
      if (m.type === 'exec') {
        const n = m.sourceLengths.length;
        const pad = (s) => (s.length ? s : '0'.repeat(n));
        spy.execs.push({
          buffers: pad(flags(m.buffers)),
          blobs: pad(flags(m.blobs)),
          opfsFiles: pad(flags(m.opfsFiles)),
        });
        if (spy.onExec) spy.onExec();
      }
    }
    return post.call(this, m, ...rest);
  };
  const arrayBuffer = Blob.prototype.arrayBuffer;
  Blob.prototype.arrayBuffer = function () {
    spy.blobReads++;
    return arrayBuffer.call(this);
  };
  spy.field = (name) => JSON.stringify(spy.execs.map((e) => e[name]));
  globalThis.__backingSpy = spy;
  return () => {
    Worker.prototype.postMessage = post;
    Blob.prototype.arrayBuffer = arrayBuffer;
    delete globalThis.__backingSpy;
  };
})()
''';

class _Spy {
  _Spy._(this._restore);

  final JSFunction _restore;

  static _Spy install() => _Spy._(
    globalContext.callMethod<JSFunction>('eval'.toJS, _spySource.toJS),
  );

  JSObject get _spy => globalContext['__backingSpy']! as JSObject;

  int sent(String type) {
    final counts = _spy['sent']! as JSObject;
    return (counts[type] as JSNumber?)?.toDartInt ?? 0;
  }

  int get blobReads => (_spy['blobReads']! as JSNumber).toDartInt;

  /// The `buffers`, `blobs` or `opfsFiles` shape of each exec posted so far.
  List<String> shapes(String field) => [
    for (final s
        in jsonDecode(
              (_spy.callMethod<JSString>('field'.toJS, field.toJS)).toDart,
            )
            as List<dynamic>)
      s as String,
  ];

  set onExec(void Function() callback) => _spy['onExec'] = callback.toJS;

  void restore() => _restore.callAsFunction();
}

/// Counts the reads a worker asks the page for.
class _Reads {
  _Reads(WebExecutor executor) {
    _sub = EventStreamProviders.messageEvent.forTarget(executor.js).listen((e) {
      final m = e.data as JSObject?;
      if (m != null && (m['type'] as JSString?)?.toDart == 'readAt') count++;
    });
  }

  int count = 0;
  late final StreamSubscription<web.MessageEvent> _sub;

  Future<void> close() => _sub.cancel();
}

Future<int> _opfsFileCount(String workerDir) async {
  try {
    final root = await web.window.navigator.storage.getDirectory().toDart;
    final ours = await root
        .getDirectoryHandle(WebExecutorBooter.opfsRootDir)
        .toDart;
    final session = await ours
        .getDirectoryHandle(WebExecutorBooter.opfsSessionId)
        .toDart;
    final dir = await session.getDirectoryHandle(workerDir).toDart;
    final iter = (dir as JSObject).callMethod<JSObject>('keys'.toJS);
    var count = 0;
    while (true) {
      final next = await iter
          .callMethod<JSPromise<JSObject>>('next'.toJS)
          .toDart;
      if ((next['done']! as JSBoolean).toDart) break;
      count++;
    }
    return count;
  } catch (_) {
    return 0; // the worker never made its directory
  }
}

BlobSource _blobOf(Uint8List bytes) => BlobSource(
  web.Blob(
    [Uint8List.fromList(bytes).toJS].toJS,
    web.BlobPropertyBag(type: 'application/pdf'),
  ),
);

/// A worker whose synchronous Blob reads throw between `armBlob` and
/// `disarmBlob`, as those of a File that changed on disk do. The engine
/// retries a failed read, so the fault lasts.
const _failsBlobRead = r"""
const read = FileReaderSync.prototype.readAsArrayBuffer;
let armed = false;
self.addEventListener('message', (e) => {
  const type = e.data && e.data.type;
  if (type === 'armBlob' || type === 'disarmBlob') {
    armed = type === 'armBlob';
    e.stopImmediatePropagation();
  }
});
FileReaderSync.prototype.readAsArrayBuffer = function (...args) {
  if (armed) {
    throw new DOMException('The requested file could not be read', 'NotReadableError');
  }
  return read.apply(this, args);
};
""";

void registerWebBackingTests({
  required String Function() workerUrl,
  required PdfIoMode mode,
}) {
  group('web reader backings (${mode.name})', () {
    late WebExecutorBooter host;
    late _Spy spy;

    setUp(() {
      host = WebExecutorBooter(laneWorkerUrl: workerUrl(), forceMode: mode);
      spy = _Spy.install();
    });

    tearDown(() {
      spy.restore();
      WebExecutorBooter.transferBudget = kTransferBytes;
    });

    test('a BlobSource opens and reads with no main-thread read and no '
        'pre-copy file', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final reads = _Reads(executor);
      addTearDown(reads.close);
      final lane = executor.beginLane() as WebLease;

      final source = _blobOf(randomStreamPdf(300 * 1024));
      final opened = await lane.submit(
        Job(
          jobId: 1,
          request: _open(source.length),
          sources: [source],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      expect(_ok(opened.bytes), isTrue, reason: '${_failure(opened.bytes)}');
      final handle = peekResponseHandleId(opened.bytes)!;
      final info = await lane.submit(
        Job(
          jobId: 2,
          request: _info(handle),
          sources: const [],
          sinks: const [],
          keepSources: const {},
          holdGroupId: 1,
        ),
      );
      expect(_ok(info.bytes), isTrue, reason: '${_failure(info.bytes)}');

      expect(spy.shapes('blobs').first, '1');
      expect(spy.shapes('buffers').first, '0');
      expect(
        spy.shapes('opfsFiles').first,
        mode == PdfIoMode.opfs ? '1' : '0',
        reason: 'only OPFS mode pre-copies a Blob',
      );
      expect(reads.count, 0, reason: 'the worker asked the page for a read');
      expect(spy.blobReads, 0, reason: 'a Blob was read on the main thread');
      expect(
        spy.sent('opfsWrite'),
        0,
        reason: 'the page sent a pre-copy chunk',
      );
      expect(spy.sent('readAtResult'), 0);
      expect(executor.heldBytes, 0, reason: 'a Blob backing holds nothing');
    }, timeout: t(20));

    test(
      'OPFS mode pre-copies a BlobSource, in the worker and from the Blob',
      () async {
        final executor = await host.boot() as WebExecutor;
        addTearDown(executor.terminate);
        final lane = executor.beginLane() as WebLease;

        final bytes = randomStreamPdf(300 * 1024);
        final source = _blobOf(bytes);
        final opened = await lane.submit(
          Job(
            jobId: 1,
            request: _open(source.length),
            sources: [source],
            sinks: const [],
            keepSources: const {0},
          ),
        );
        expect(_ok(opened.bytes), isTrue, reason: '${_failure(opened.bytes)}');
        final handle = peekResponseHandleId(opened.bytes)!;

        expect(spy.shapes('opfsFiles').first, '1');
        expect(spy.sent('opfsWrite'), 0, reason: 'the page sent a chunk');
        expect(spy.blobReads, 0, reason: 'the page read the Blob');
        expect(
          await _opfsFileCount(executor.opfsDir!),
          1,
          reason: 'the Blob is on disk while its document is held',
        );
        final info = await lane.submit(
          Job(
            jobId: 2,
            request: _info(handle),
            sources: const [],
            sinks: const [],
            keepSources: const {},
            holdGroupId: 1,
          ),
        );
        expect(_ok(info.bytes), isTrue, reason: '${_failure(info.bytes)}');

        lane.releaseHold(opened.holds.values.first);
        await lane.reset();
        expect(await _opfsFileCount(executor.opfsDir!), 0);
      },
      timeout: t(20),
      skip: mode == PdfIoMode.opfs ? false : 'only OPFS mode pre-copies',
    );

    test('a MemorySource under the budget is copied once: no pre-copy, no '
        'read during the op, the caller\'s bytes intact', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final reads = _Reads(executor);
      addTearDown(reads.close);
      final lane = executor.beginLane() as WebLease;

      var execPosted = false;
      var readsBefore = 0;
      var readsDuring = 0;
      spy.onExec = () => execPosted = true;
      final counted = TestSource(
        randomStreamPdf(200 * 1024),
        onRead: (_, _) => execPosted ? readsDuring++ : readsBefore++,
      );
      final first = await lane.submit(
        Job(
          jobId: 1,
          request: _open(counted.length),
          sources: [counted],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      expect(_ok(first.bytes), isTrue, reason: '${_failure(first.bytes)}');
      final handle = peekResponseHandleId(first.bytes)!;
      final info = await lane.submit(
        Job(
          jobId: 2,
          request: _info(handle),
          sources: const [],
          sinks: const [],
          keepSources: const {},
          holdGroupId: 1,
        ),
      );
      expect(_ok(info.bytes), isTrue, reason: '${_failure(info.bytes)}');
      expect(readsBefore, greaterThan(0), reason: 'the source was read whole');
      expect(readsDuring, 0, reason: 'the op read the source on the page');
      expect(reads.count, 0);
      expect(spy.shapes('buffers').first, '1');
      expect(spy.sent('opfsWrite'), 0);
      if (mode == PdfIoMode.opfs) {
        expect(await _opfsFileCount(executor.opfsDir!), 0);
      }
      expect(executor.heldBytes, counted.length);

      final bytes = randomStreamPdf(100 * 1024);
      final size = bytes.length;
      final first0 = bytes[0];
      final second = await lane.submit(
        Job(
          jobId: 3,
          request: _open(size),
          sources: [MemorySource(bytes)],
          sinks: const [],
          keepSources: const {},
        ),
      );
      expect(_ok(second.bytes), isTrue, reason: '${_failure(second.bytes)}');
      expect(bytes.length, size, reason: 'the caller\'s buffer was detached');
      expect(bytes[0], first0);
    }, timeout: t(20));

    test('held equals the sizes transferred while a document lives, and is '
        'zero after its release and after a reset', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final lane = executor.beginLane() as WebLease;

      final a = src(randomStreamPdf(70 * 1024));
      final b = src(randomStreamPdf(90 * 1024));
      final openedA = await lane.submit(
        Job(
          jobId: 1,
          request: _open(a.length),
          sources: [a],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      expect(_ok(openedA.bytes), isTrue);
      expect(executor.heldBytes, a.length);
      final openedB = await lane.submit(
        Job(
          jobId: 2,
          request: _open(b.length),
          sources: [b],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      expect(_ok(openedB.bytes), isTrue);
      expect(executor.heldBytes, a.length + b.length);

      // A source that is not kept joins a job and leaves with it.
      final scratch = src(minimalPdf);
      final third = await lane.submit(
        Job(
          jobId: 3,
          request: _open(scratch.length),
          sources: [scratch],
          sinks: const [],
          keepSources: const {},
        ),
      );
      expect(_ok(third.bytes), isTrue);
      expect(executor.heldBytes, a.length + b.length);

      lane.releaseHold(openedA.holds.values.first);
      final fourth = await lane.submit(
        Job(
          jobId: 4,
          request: _open(scratch.length),
          sources: [scratch],
          sinks: const [],
          keepSources: const {},
        ),
      );
      expect(_ok(fourth.bytes), isTrue);
      expect(executor.heldBytes, b.length, reason: 'after A\'s release');

      await lane.reset();
      expect(executor.heldBytes, 0, reason: 'after a reset');
    }, timeout: t(20));

    test('a cancel during the whole read of a slow source completes the job '
        'cancelled without posting it', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final lane = executor.beginLane() as WebLease;

      final slow = SlowSource(minimalPdf);
      final running = lane.submit(
        Job(
          jobId: 1,
          request: _open(slow.length),
          sources: [slow],
          sinks: const [],
          keepSources: const {},
        ),
      );
      await slow.firstRead;
      lane.cancelJob(1);
      expect((await running).bytes, buildCancelledResponse());
      slow.release();
      await Future<void>.delayed(const Duration(milliseconds: 100));
      expect(spy.sent('exec'), 0, reason: 'the cancelled job was posted');
      expect(spy.sent('opfsWrite'), 0);

      // The lane serves the next job.
      final next = src(minimalPdf);
      final opened = await lane.submit(
        Job(
          jobId: 2,
          request: _open(next.length),
          sources: [next],
          sinks: const [],
          keepSources: const {},
        ),
      );
      expect(_ok(opened.bytes), isTrue);
    }, timeout: t(20));

    test('a BlobSource whose Blob fails to read fails the op with the I/O '
        'error', () async {
      final executor =
          await withWorkerPrelude(_failsBlobRead, host.boot) as WebExecutor;
      addTearDown(executor.terminate);
      final lane = executor.beginLane() as WebLease;

      final source = _blobOf(fThousandPage);
      void arm(String type) => executor.js.postMessage(
        JSObject()
          ..['type'] = type.toJS
          ..['lease'] = lane.lease.toJS,
      );
      Job openJob(int jobId) => Job(
        jobId: jobId,
        request: bin.encodeRequest('editorOpen', {
          'sourceLength': source.length,
          'password': null,
        }),
        sources: [source],
        sinks: const [],
        keepSources: const {0},
      );

      if (mode == PdfIoMode.opfs) {
        // The Blob is read once, by the worker's pre-copy.
        arm('armBlob');
        final failed = await lane.submit(openJob(1));
        expect(_ok(failed.bytes), isFalse);
        expect(_failure(failed.bytes), isA<PdfIoFailed>());
        arm('disarmBlob');
        final again = await lane.submit(openJob(2));
        expect(_ok(again.bytes), isTrue, reason: '${_failure(again.bytes)}');
        return;
      }

      final opened = await lane.submit(openJob(1));
      expect(_ok(opened.bytes), isTrue, reason: '${_failure(opened.bytes)}');
      final handle = peekResponseHandleId(opened.bytes)!;
      Job save(int jobId) => Job(
        jobId: jobId,
        request: bin.encodeRequest('editorSave', {
          'handleId': handle,
          ...codec.encodeSaveArgs(const PdfSave.full()),
        }),
        sources: const [],
        sinks: [TestSink()],
        keepSources: const {},
        holdGroupId: 1,
      );

      arm('armBlob');
      final failed = await lane.submit(save(2));
      expect(_ok(failed.bytes), isFalse);
      expect(_failure(failed.bytes), isA<PdfIoFailed>());

      // The File is readable again: the document saves on the next job.
      arm('disarmBlob');
      final again = await lane.submit(save(3));
      expect(_ok(again.bytes), isTrue, reason: '${_failure(again.bytes)}');
    }, timeout: t(20));

    test(
      'a merge of more inputs than the budget holds transfers the first '
      'and sends the rest down the mode\'s path, to the same output',
      () async {
        final pdf = Pdf(
          config: PdfConfig(webWorkerUrl: workerUrl(), webIoMode: mode),
        );
        addTearDown(pdf.dispose);
        final bytes = randomStreamPdf(1024 * 1024);
        final fit = kTransferBytes ~/ bytes.length;
        const inputs = 20;
        expect(fit, lessThan(inputs), reason: 'the fixture must overfill');

        Future<Uint8List> merged() async {
          final out = MemorySink();
          await pdf.merge([
            for (var i = 0; i < inputs; i++) PdfInput(MemorySource(bytes)),
          ], out);
          return out.takeBytes();
        }

        List<String> shape(String field) => [
          for (final s in spy.shapes(field))
            if (s.isNotEmpty) s,
        ];

        final today = await merged();
        final copied = shape('buffers').join();
        expect(copied.length, inputs);
        expect(
          copied,
          '1' * fit + '0' * (inputs - fit),
          reason: 'the sources that fit in what was left were transferred',
        );
        if (mode == PdfIoMode.opfs) {
          expect(
            shape('opfsFiles').join(),
            '0' * fit + '1' * (inputs - fit),
            reason: 'only the sources past the budget were pre-copied',
          );
        }

        WebExecutorBooter.transferBudget = 0;
        final before = shape('buffers').join().length;
        final fallback = await merged();
        expect(
          shape('buffers').join().substring(before),
          '0' * inputs,
          reason: 'a budget of zero sends every source down the mode\'s path',
        );
        expect(today, fallback);
      },
      timeout: t(120),
    );
  });
}
