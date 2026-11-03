// Web-only: the lease id, the reset message and the worker's single-exec wall.
//
// The adapter side drives a real WebLane and replays messages of a replaced
// lease into it. The worker side posts raw messages to a bare worker taken
// from the host, so it sees exactly what a misbehaving adapter would send:
// a stale lease, a second exec while one runs, a reset under a running exec.
// Which of these a mode can reach differs: only JSPI suspends inside an exec
// (the handler re-enters), OPFS awaits the file open, and Atomics blocks the
// worker's thread so a second message waits its turn.

import 'dart:async';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/web/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:test/test.dart';
import 'package:web/web.dart' as web;
import 'package:web/web.dart' show EventStreamProviders;

import '../../../fixtures/generated/fixtures.dart';
import '../../../fixtures/handwritten.dart';
import '../../../harness/slow_source.dart';
import '../../../harness/test_source_sink.dart';
import '../../../harness/timeouts.dart';

Uint8List _open(int sourceLength) =>
    bin.encodeRequest('open', {'sourceLength': sourceLength, 'password': null});

Uint8List _info(int handle, List<int> pages) =>
    bin.encodeRequest('pageInfo', {'handleId': handle, 'pageIndices': pages});

bool _ok(Uint8List bytes) => bytes.isNotEmpty && bytes[0] == 1;

String _failure(Uint8List bytes) {
  try {
    bin.decodeResponse(bytes);
    return '';
  } catch (e) {
    return e.toString();
  }
}

// ── JS message helpers ──────────────────────────────────────────────

JSObject _msg(String type, Map<String, JSAny?> fields) {
  final obj = JSObject()..['type'] = type.toJS;
  fields.forEach((k, v) => obj[k] = v);
  return obj;
}

String? _type(JSObject m) => (m['type'] as JSString?)?.toDart;

int? _num(JSObject m, String key) => (m[key] as JSNumber?)?.toDartInt;

bool _flag(JSObject m, String key) => (m[key] as JSBoolean?)?.toDart ?? false;

Uint8List _bytesOf(JSObject m, String key) =>
    Uint8List.fromList((m[key] as JSArrayBuffer).toDart.asUint8List());

bool Function(JSObject) _is(String type, {int? jobId}) =>
    (m) => _type(m) == type && (jobId == null || _num(m, 'jobId') == jobId);

/// The files in every worker directory of this page session.
Future<int> _opfsFileCount() async {
  final root = await web.window.navigator.storage.getDirectory().toDart;
  final web.FileSystemDirectoryHandle ours;
  try {
    ours = await root.getDirectoryHandle(WebLaneHost.opfsRootDir).toDart;
  } catch (_) {
    return 0;
  }
  final web.FileSystemDirectoryHandle session;
  try {
    session = await ours.getDirectoryHandle(WebLaneHost.opfsSessionId).toDart;
  } catch (_) {
    return 0;
  }
  var count = 0;
  for (final worker in await _names(session)) {
    try {
      final dir = await session.getDirectoryHandle(worker).toDart;
      count += (await _names(dir)).length;
    } catch (_) {
      count++; // a file at session level
    }
  }
  return count;
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

/// A bare worker the test talks to in raw messages. It records everything
/// the worker posts, in order, so a test can also prove a message got no
/// answer.
class _RawWorker {
  _RawWorker._(this._host, this._worker) {
    _sub = EventStreamProviders.messageEvent.forTarget(_worker.js).listen((
      event,
    ) {
      final obj = event.data as JSObject?;
      if (obj == null) return;
      history.add(obj);
      _arrivals.add(obj);
    });
  }

  static Future<_RawWorker> take(WebLaneHost host) async =>
      _RawWorker._(host, (await host.takeWorker(() => false))!);

  final WebLaneHost _host;
  final LaneWorker _worker;
  final history = <JSObject>[];
  final _arrivals = StreamController<JSObject>.broadcast(sync: true);
  late final StreamSubscription<web.MessageEvent> _sub;

  void post(String type, Map<String, JSAny?> fields, [List<JSObject>? give]) {
    final obj = _msg(type, fields);
    if (give == null) {
      _worker.js.postMessage(obj);
    } else {
      _worker.js.postMessage(obj, give.toJS);
    }
  }

  void exec(
    int lease,
    int jobId,
    Uint8List request, {
    List<int> sourceLengths = const [],
    List<String>? opfsFiles,
  }) {
    final buffer = Uint8List.fromList(request).buffer.toJS;
    post(
      'exec',
      {
        'lease': lease.toJS,
        'jobId': jobId.toJS,
        'requestBytes': buffer,
        'sourceLengths': [for (final n in sourceLengths) n.toJS].toJS,
        'sinkCount': 0.toJS,
        'keepSources': <JSAny?>[].toJS,
        if (opfsFiles != null)
          'opfsFiles': [for (final f in opfsFiles) f.toJS].toJS,
      },
      [buffer],
    );
  }

  /// The first message that matches, already posted or yet to come.
  Future<JSObject> waitFor(bool Function(JSObject) test) {
    for (final message in history) {
      if (test(message)) return Future.value(message);
    }
    return _arrivals.stream
        .firstWhere(test)
        .timeout(
          const Duration(seconds: 20),
          onTimeout: () =>
              throw StateError('the worker never posted the message'),
        );
  }

  bool saw(bool Function(JSObject) test) => history.any(test);

  /// Answers every JSPI read of the worker from [bytes], once [go] is done.
  void serveReads(int lease, Uint8List bytes, Future<void> go) {
    _arrivals.stream.where(_is('readAt')).listen((read) async {
      await go;
      final offset = _num(read, 'offset')!;
      final end = math.min(offset + _num(read, 'count')!, bytes.length);
      final slice = Uint8List.fromList(
        offset >= bytes.length ? Uint8List(0) : bytes.sublist(offset, end),
      ).buffer.toJS;
      post('readAtResult', {'lease': lease.toJS, 'bytes': slice}, [slice]);
    });
  }

  Future<void> close() async {
    await _sub.cancel();
    await _arrivals.close();
    _host.retire(_worker);
  }
}

void registerWebLaneProtocolTests({
  required String Function() workerUrl,
  required PdfIoMode mode,
}) {
  group('web lane protocol (${mode.name})', () {
    late WebLaneHost host;

    setUp(() {
      host = WebLaneHost(laneWorkerUrl: workerUrl(), forceMode: mode);
    });

    test('the adapter drops every message of a replaced lease', () async {
      final lane = host.spawn() as WebLane;
      addTearDown(lane.kill);
      final slow = SlowSource(minimalPdf);
      final running = lane.submit(
        LaneJob(
          jobId: 1,
          request: _open(slow.length),
          sources: [slow],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      var done = false;
      unawaited(running.then((_) => done = true));
      await slow.firstRead; // the lane holds its worker, and the job runs

      final stale = lane.lease - 1;
      final success = Uint8List.fromList(const [1, 0, 0]).buffer.toJS;
      // Messages the worker of an earlier tenure could still be sending:
      // the result, a failure, a read, a reset answer.
      lane.deliver(
        _msg('result', {
          'lease': stale.toJS,
          'jobId': 1.toJS,
          'data': success,
          'mem': 1.toJS,
        }),
      );
      lane.deliver(
        _msg('error', {
          'lease': stale.toJS,
          'jobId': 1.toJS,
          'message': 'a stale failure'.toJS,
          'trap': true.toJS,
        }),
      );
      lane.deliver(
        _msg('readAt', {
          'lease': stale.toJS,
          'sourceIndex': 0.toJS,
          'offset': 0.toJS,
          'count': 8.toJS,
        }),
      );
      // The current lease, naming a job that is not the running one.
      final other = Uint8List.fromList(const [1, 0, 0]).buffer.toJS;
      lane.deliver(
        _msg('result', {
          'lease': lane.lease.toJS,
          'jobId': 99.toJS,
          'data': other,
          'mem': 1.toJS,
        }),
      );

      await Future<void>.delayed(const Duration(milliseconds: 150));
      expect(done, isFalse, reason: 'a stale message resolved the current job');

      slow.release();
      final result = await running;
      expect(_ok(result.bytes), isTrue);
      expect(
        peekResponseHandleId(result.bytes),
        isNotNull,
        reason: 'the job resolved with a fake result, not the engine\'s',
      );
    }, timeout: t(10));

    test('a reset gives the worker a fresh engine state', () async {
      final lane = host.spawn() as WebLane;
      addTearDown(lane.kill);
      final first = src(minimalPdf);
      final opened = await lane.submit(
        LaneJob(
          jobId: 1,
          request: _open(first.length),
          sources: [first],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      final old = peekResponseHandleId(opened.bytes)!;
      expect(opened.heldTokens, hasLength(1));
      if (mode == PdfIoMode.opfs) {
        expect(await _opfsFileCount(), greaterThan(0));
      }

      final mem = await lane.reset();
      expect(mem, isNotNull);
      expect(mem, greaterThan(0), reason: 'resetDone carries the wasm memory');
      if (mode == PdfIoMode.opfs) {
        expect(await _opfsFileCount(), 0, reason: 'the reset left a file');
      }

      final gone = await lane.submit(
        LaneJob(
          jobId: 2,
          request: _info(old, const []),
          sources: const [],
          sinks: const [],
          keepSources: const {},
          heldGroupId: 1,
        ),
      );
      expect(_failure(gone.bytes), contains('document not found'));

      // Job ids restart after a reset as they do under a new lease, so
      // group 1 names the new document's reader and nothing of the old
      // one. A reader the reset forgot would sit first in the group and
      // clip every read to its own, much shorter, length.
      final big = src(fThousandPage);
      final reopened = await lane.submit(
        LaneJob(
          jobId: 1,
          request: _open(big.length),
          sources: [big],
          sinks: const [],
          keepSources: const {0},
        ),
      );
      expect(_ok(reopened.bytes), isTrue);
      final handle = peekResponseHandleId(reopened.bytes)!;
      final text = await lane.submit(
        LaneJob(
          jobId: 3,
          request: bin.encodeRequest('extract', {
            'handleId': handle,
            'format': 'plainText',
            'page': 999,
          }),
          sources: const [],
          sinks: const [],
          keepSources: const {},
          heldGroupId: 1,
        ),
      );
      expect(_failure(text.bytes), isEmpty);
    }, timeout: t(15));

    test('the worker serves the current lease, adopts a newer one and '
        'drops the rest', () async {
      final raw = await _RawWorker.take(host);
      addTearDown(raw.close);
      final create = bin.encodeRequest('builderCreate', {});

      raw.exec(5, 1, create);
      final first = await raw.waitFor(_is('result', jobId: 1));
      expect(_num(first, 'lease'), 5, reason: 'the worker echoes its lease');

      // An older lease: no answer of any kind, for exec or reset.
      raw.exec(3, 2, create);
      raw.post('reset', {'lease': 3.toJS});
      raw.exec(5, 3, create);
      final third = await raw.waitFor(_is('result', jobId: 3));
      expect(_ok(_bytesOf(third, 'data')), isTrue);
      expect(raw.saw(_is('result', jobId: 2)), isFalse);
      expect(raw.saw(_is('error', jobId: 2)), isFalse);
      expect(raw.saw(_is('resetDone')), isFalse);

      // A newer lease opens a new tenure; the old one is stale at once.
      raw.exec(6, 4, create);
      final fourth = await raw.waitFor(_is('result', jobId: 4));
      expect(_num(fourth, 'lease'), 6);
      raw.exec(5, 5, create);
      raw.post('reset', {'lease': 6.toJS});
      final done = await raw.waitFor(_is('resetDone'));
      expect(_num(done, 'lease'), 6);
      expect(raw.saw(_is('result', jobId: 5)), isFalse);
      expect(raw.saw(_is('error', jobId: 5)), isFalse);
    }, timeout: t(10));

    test('the worker never starts a second exec while one runs', () async {
      final raw = await _RawWorker.take(host);
      addTearDown(raw.close);
      final create = bin.encodeRequest('builderCreate', {});
      final bytes = minimalPdf;

      switch (mode) {
        case PdfIoMode.jspi:
          // The first exec suspends on its first read, so the handler is
          // re-entered by whatever arrives next.
          final go = Completer<void>();
          raw.serveReads(1, bytes, go.future);
          raw.exec(1, 1, _open(bytes.length), sourceLengths: [bytes.length]);
          raw.exec(1, 2, create);
          final refused = await raw.waitFor(_is('error', jobId: 2));
          expect(_flag(refused, 'protocol'), isTrue);
          expect(_num(refused, 'lease'), 1);

          // A reset would pull the engine state from under the suspended exec.
          raw.post('reset', {'lease': 1.toJS});
          final noReset = await raw.waitFor(
            (m) => _type(m) == 'error' && _num(m, 'jobId') == null,
          );
          expect(_flag(noReset, 'protocol'), isTrue);

          go.complete();
          final result = await raw.waitFor(_is('result', jobId: 1));
          expect(_ok(_bytesOf(result, 'data')), isTrue);
          expect(raw.saw(_is('result', jobId: 2)), isFalse);
          expect(raw.saw(_is('resetDone')), isFalse);

        case PdfIoMode.opfs:
          // The first exec awaits the opening of its pre-copied file.
          const file = 'pdf_lane_raw_wall';
          final chunk = Uint8List.fromList(bytes).buffer.toJS;
          raw.post(
            'opfsWrite',
            {
              'lease': 1.toJS,
              'jobId': 1.toJS,
              'filename': file.toJS,
              'chunk': chunk,
              'offset': 0.toJS,
              'last': true.toJS,
            },
            [chunk],
          );
          await raw.waitFor(_is('opfsWriteAck'));
          raw.exec(
            1,
            1,
            _open(bytes.length),
            sourceLengths: [bytes.length],
            opfsFiles: [file],
          );
          raw.exec(1, 2, create);
          final refused = await raw.waitFor(_is('error', jobId: 2));
          expect(_flag(refused, 'protocol'), isTrue);
          final result = await raw.waitFor(_is('result', jobId: 1));
          expect(_ok(_bytesOf(result, 'data')), isTrue);
          expect(raw.saw(_is('result', jobId: 2)), isFalse);

        default:
          // Atomics blocks the worker's thread inside an exec, so a second
          // message waits for the first to finish: it runs after it, or the
          // worker refuses it where the first still awaits. Never both ways
          // at once, and the first always completes.
          raw.exec(1, 1, create);
          raw.exec(1, 2, create);
          final first = await raw.waitFor(_is('result', jobId: 1));
          expect(_ok(_bytesOf(first, 'data')), isTrue);
          final second = await raw.waitFor(
            (m) =>
                (_type(m) == 'result' || _type(m) == 'error') &&
                _num(m, 'jobId') == 2,
          );
          final ran = raw.saw(_is('result', jobId: 2));
          final refused = raw.saw(
            (m) => _is('error', jobId: 2)(m) && _flag(m, 'protocol'),
          );
          expect(ran != refused, isTrue, reason: 'the second exec: $second');
      }
    }, timeout: t(10));
  });
}
