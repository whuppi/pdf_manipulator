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
import 'package:pdf_manipulator/src/runtime/web/protocol.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
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

String? _str(JSObject m, String key) => (m[key] as JSString?)?.toDart;

List<String> _strings(JSObject m, String key) => [
  for (final v in (m[key] as JSArray<JSString>?)?.toDart ?? const <JSString>[])
    v.toDart,
];

bool _flag(JSObject m, String key) => (m[key] as JSBoolean?)?.toDart ?? false;

Uint8List _bytesOf(JSObject m, String key) =>
    Uint8List.fromList((m[key] as JSArrayBuffer).toDart.asUint8List());

bool Function(JSObject) _is(String type, {int? jobId}) =>
    (m) => _type(m) == type && (jobId == null || _num(m, 'jobId') == jobId);

/// The files in one worker's own directory: another worker's files, live or
/// left by one the browser has not reaped yet, say nothing about this one.
Future<int> _opfsFileCount(String workerDir) async {
  try {
    final root = await web.window.navigator.storage.getDirectory().toDart;
    final ours = await root.getDirectoryHandle(WebLaneHost.opfsRootDir).toDart;
    final session = await ours
        .getDirectoryHandle(WebLaneHost.opfsSessionId)
        .toDart;
    final dir = await session.getDirectoryHandle(workerDir).toDart;
    return (await _names(dir)).length;
  } catch (_) {
    return 0; // the worker never made its directory
  }
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

/// Worker prelude: the worker stops at the OPFS call a test names until the
/// test says go. `removeEntry` holds every delete; a gate opens once and stays
/// open. A held call posts `gated`, a finished delete posts `removed`.
const _gates = r'''
const dir = FileSystemDirectoryHandle.prototype;
const gates = {};
for (const what of ['removeEntry']) {
  gates[what] = new Promise((go) => { gates[what + 'Go'] = go; });
}
self.addEventListener('message', (e) => {
  const m = e.data;
  if (m && m.type === 'gateOpen') {
    gates[m.what + 'Go']();
    e.stopImmediatePropagation();
  }
});
function held(what, name) {
  postMessage({ type: 'gated', what, name });
  return gates[what];
}
const removeEntry = dir.removeEntry;
dir.removeEntry = async function (name, options) {
  await held('removeEntry', name);
  const result = await removeEntry.call(this, name, options);
  postMessage({ type: 'removed', name });
  return result;
};
''';

/// Worker prelude: drops every `opfsWrite` unanswered, as a worker that has
/// died or hung would, and says so with `dropped`.
const _dropsOpfsWrite = r'''
self.addEventListener('message', (e) => {
  if (e.data && e.data.type === 'opfsWrite') {
    e.stopImmediatePropagation();
    postMessage({ type: 'dropped' });
  }
});
''';

/// Worker prelude: a sync access handle cannot be created on a file whose
/// handle was closed less than two seconds ago, as in a browser that
/// releases the lock after `close()` returns; each refusal posts `refused`.
const _refusesReopen = r'''
const closedAt = new Map();
const fileHandle = FileSystemFileHandle.prototype;
const create = fileHandle.createSyncAccessHandle;
fileHandle.createSyncAccessHandle = async function (...args) {
  const name = this.name;
  const closed = closedAt.get(name);
  if (closed !== undefined && performance.now() - closed < 2000) {
    postMessage({ type: 'refused', name });
    throw new DOMException('the previous handle is still being released', 'NoModificationAllowedError');
  }
  const handle = await create.apply(this, args);
  const close = handle.close.bind(handle);
  handle.close = () => {
    closedAt.set(name, performance.now());
    return close();
  };
  return handle;
};
''';

/// Worker prelude: the creation of a sync access handle is held until
/// `goCreate` (`held` says it is), and is then refused with the
/// NoModificationAllowedError of a browser that has been asked to delete the
/// file meanwhile (`refused`). `ping` is answered with `pong`.
const _holdsCreate = r'''
const removed = new Set();
const dirProto = FileSystemDirectoryHandle.prototype;
const removeEntry = dirProto.removeEntry;
dirProto.removeEntry = function (name, options) {
  removed.add(name);
  return removeEntry.call(this, name, options);
};
let go;
const gate = new Promise((resolve) => { go = resolve; });
const fileProto = FileSystemFileHandle.prototype;
const create = fileProto.createSyncAccessHandle;
fileProto.createSyncAccessHandle = async function (...args) {
  const name = this.name;
  postMessage({ type: 'held', name });
  await gate;
  if (removed.has(name)) {
    postMessage({ type: 'refused', name });
    throw new DOMException('the file is being deleted', 'NoModificationAllowedError');
  }
  return create.apply(this, args);
};
self.addEventListener('message', (e) => {
  const m = e.data;
  if (m && m.type === 'goCreate') { go(); e.stopImmediatePropagation(); }
  else if (m && m.type === 'ping') { postMessage({ type: 'pong' }); e.stopImmediatePropagation(); }
});
''';

/// Worker prelude: the first `getDirectoryHandle` that creates a directory
/// rejects, as a browser whose storage is briefly unavailable does, and
/// `rejected` says so.
const _rejectsDirOnce = r'''
const proto = FileSystemDirectoryHandle.prototype;
const getDirectoryHandle = proto.getDirectoryHandle;
let rejected = false;
proto.getDirectoryHandle = function (name, options) {
  if (!rejected && options && options.create) {
    rejected = true;
    postMessage({ type: 'rejected' });
    return Promise.reject(new DOMException('storage is unavailable', 'UnknownError'));
  }
  return getDirectoryHandle.call(this, name, options);
};
''';

/// Worker prelude: every `opfsWriteAck` is held until `ackRelease` names its
/// job (`ackHeld` says one is held). `ping` is answered with `pong` and
/// every `exec` that reaches the worker is reported as `execSeen`, in the
/// order the worker received them.
const _holdsAcks = r'''
const post = self.postMessage.bind(self);
const held = {};
self.postMessage = (m, t) => {
  if (m && m.type === 'opfsWriteAck') {
    (held[m.jobId] ??= []).push(m);
    post({ type: 'ackHeld', jobId: m.jobId });
    return;
  }
  return post(m, t);
};
self.addEventListener('message', (e) => {
  const m = e.data;
  if (!m) return;
  if (m.type === 'ackRelease') {
    for (const ack of held[m.jobId] ?? []) post(ack);
    held[m.jobId] = [];
    post({ type: 'ackReleased', jobId: m.jobId });
    e.stopImmediatePropagation();
  } else if (m.type === 'ping') {
    post({ type: 'pong' });
    e.stopImmediatePropagation();
  } else if (m.type === 'exec') {
    post({ type: 'execSeen', jobId: m.jobId });
  }
});
''';

/// Worker prelude: once armed by `xhrArm`, the first synchronous XHR send
/// throws the TimeoutError a browser too busy to answer produces, and every
/// send reports the timeout it was given.
const _timesOutOnePoll = r'''
let armed = false;
let thrown = false;
self.addEventListener('message', (e) => {
  if (e.data && e.data.type === 'xhrArm') {
    armed = true;
    e.stopImmediatePropagation();
  }
});
const send = XMLHttpRequest.prototype.send;
XMLHttpRequest.prototype.send = function (...args) {
  if (!armed) return send.apply(this, args);
  postMessage({ type: 'xhrSend', timeout: this.timeout });
  if (!thrown) {
    thrown = true;
    postMessage({ type: 'xhrTimeout' });
    throw new DOMException('the poll timed out', 'TimeoutError');
  }
  return send.apply(this, args);
};
''';

/// Worker prelude: `createSyncAccessHandle` returns a promise that does not
/// settle, as Chrome's sometimes does on a fresh file while the thread keeps
/// running: the first call only, or every call. `hung` names the file of each
/// such call. `release` settles them all with the real create. `holdReset`
/// makes the worker drop every `reset` it is sent from then on.
String _hangsCreate({required bool every}) =>
    '''
const every = $every;
let seen = 0;
let release;
const gate = new Promise((resolve) => { release = resolve; });
const proto = FileSystemFileHandle.prototype;
const create = proto.createSyncAccessHandle;
proto.createSyncAccessHandle = function (...args) {
  if (!every && seen++ > 0) return create.apply(this, args);
  postMessage({ type: 'hung', name: this.name });
  return gate.then(() => create.apply(this, args));
};
let holdReset = false;
self.addEventListener('message', (e) => {
  const m = e.data;
  if (!m) return;
  if (m.type === 'release') { release(); e.stopImmediatePropagation(); }
  else if (m.type === 'holdReset') { holdReset = true; e.stopImmediatePropagation(); }
  else if (m.type === 'reset' && holdReset) e.stopImmediatePropagation();
});
''';

/// Every message a booted worker posts, in order, for a test that drives a
/// real lane over it.
class _Watched {
  _Watched(WebExecutor executor) {
    _sub = EventStreamProviders.messageEvent.forTarget(executor.js).listen((e) {
      final m = e.data as JSObject?;
      if (m == null) return;
      seen.add(m);
      _arrivals.add(m);
    });
  }

  final seen = <JSObject>[];
  final _arrivals = StreamController<JSObject>.broadcast(sync: true);
  late final StreamSubscription<web.MessageEvent> _sub;

  Future<JSObject> next(bool Function(JSObject) test) {
    for (final m in seen) {
      if (test(m)) return Future.value(m);
    }
    return _arrivals.stream.firstWhere(test);
  }

  Future<void> close() async {
    await _sub.cancel();
    await _arrivals.close();
  }
}

/// Waits until [done] holds, polling.
Future<void> _until(Future<bool> Function() done, String what) async {
  final deadline = DateTime.now().add(const Duration(seconds: 10));
  while (!await done()) {
    if (DateTime.now().isAfter(deadline)) throw TestFailure(what);
    await Future<void>.delayed(const Duration(milliseconds: 25));
  }
}

/// Every answer is a protocol refusal, for these jobs in this order.
void _expectRefused(List<JSObject> answers, List<int?> jobIds) {
  expect([
    for (final a in answers) _type(a),
  ], List.filled(jobIds.length, 'error'));
  expect([for (final a in answers) _num(a, 'jobId')], jobIds);
  expect(answers.every((a) => _flag(a, 'protocol')), isTrue);
}

bool Function(JSObject) _gated(String what) =>
    (m) => _type(m) == 'gated' && (m['what'] as JSString?)?.toDart == what;

/// A bare worker the test talks to in raw messages. It records everything
/// the worker posts, in order, so a test can also prove a message got no
/// answer.
class _RawWorker {
  _RawWorker._(this._worker) {
    _sub = EventStreamProviders.messageEvent.forTarget(_worker.js).listen((
      event,
    ) {
      final obj = event.data as JSObject?;
      if (obj == null) return;
      history.add(obj);
      _arrivals.add(obj);
    });
  }

  static Future<_RawWorker> take(WebLaneHost host, {String? prelude}) async =>
      _RawWorker._(
        await (prelude == null
                ? host.boot()
                : withWorkerPrelude(prelude, host.boot))
            as WebExecutor,
      );

  final WebExecutor _worker;

  /// The worker's own OPFS directory.
  String get directory => _worker.opfsDir!;
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
    List<int> keepSources = const [],
    int sinkCount = 0,
    int? heldGroupId,
    String? cancelUrl,
  }) {
    final buffer = Uint8List.fromList(request).buffer.toJS;
    post(
      'exec',
      {
        'lease': lease.toJS,
        'jobId': jobId.toJS,
        'requestBytes': buffer,
        'sourceLengths': [for (final n in sourceLengths) n.toJS].toJS,
        'sinkCount': sinkCount.toJS,
        'keepSources': [for (final i in keepSources) i.toJS].toJS,
        if (heldGroupId != null) 'heldGroupId': heldGroupId.toJS,
        if (opfsFiles != null)
          'opfsFiles': [for (final f in opfsFiles) f.toJS].toJS,
        if (cancelUrl != null) 'cancelUrl': cancelUrl.toJS,
      },
      [buffer],
    );
  }

  /// Pre-copies [bytes] into the worker's directory as [filename], whole.
  void precopy(int lease, int jobId, String filename, Uint8List bytes) {
    final chunk = Uint8List.fromList(bytes).buffer.toJS;
    post(
      'opfsWrite',
      {
        'lease': lease.toJS,
        'jobId': jobId.toJS,
        'filename': filename.toJS,
        'chunk': chunk,
        'offset': 0.toJS,
        'last': true.toJS,
      },
      [chunk],
    );
  }

  /// Lets the worker pass the [what] gate of [_gates].
  void openGate(String what) => post('gateOpen', {'what': what.toJS});

  /// The answers the worker gave under [lease], in arrival order.
  List<JSObject> answers(int lease) => [
    for (final m in history)
      if (_num(m, 'lease') == lease &&
          const {
            'result',
            'error',
            'resetDone',
            'opfsWriteAck',
          }.contains(_type(m)))
        m,
  ];

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
    _worker.terminate();
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

    test('a drain answers a parked job at once, settles when the worker has '
        'posted, and the worker serves the next lease', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final lane = executor.beginLane() as WebLane;
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
      await slow.firstRead;

      final drained = lane.drain();
      expect((await running).bytes, buildCancelledResponse());
      expect(await drained, isTrue, reason: 'the engine is intact');
      slow.release();

      await lane.reset();
      final next = executor.beginLane();
      final source = src(minimalPdf);
      final opened = await next.submit(
        LaneJob(
          jobId: 1,
          request: _open(source.length),
          sources: [source],
          sinks: const [],
          keepSources: const {},
        ),
      );
      expect(_ok(opened.bytes), isTrue, reason: 'the next lease serves');
    }, timeout: t(10));

    test('the adapter drops every message of a replaced lease', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      final lane = executor.beginLane() as WebLane;
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

    Future<void> expectResetGivesFreshState(WebExecutor executor) async {
      final lane = executor.beginLane() as WebLane;
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
        expect(await _opfsFileCount(executor.opfsDir!), greaterThan(0));
      }

      final mem = await lane.reset();
      expect(mem, isNotNull);
      expect(mem, greaterThan(0), reason: 'resetDone carries the wasm memory');
      if (mode == PdfIoMode.opfs) {
        expect(
          await _opfsFileCount(executor.opfsDir!),
          0,
          reason: 'the reset left a file',
        );
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
    }

    test('a reset gives the worker a fresh engine state', () async {
      final executor = await host.boot() as WebExecutor;
      addTearDown(executor.terminate);
      await expectResetGivesFreshState(executor);
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

    if (mode == PdfIoMode.opfs) {
      test('a reset is judged by its own directory, not by the files of '
          'workers that are gone', () async {
        final dead = await host.boot() as WebExecutor;
        final held = src(minimalPdf);
        await (dead.beginLane() as WebLane).submit(
          LaneJob(
            jobId: 1,
            request: _open(held.length),
            sources: [held],
            sinks: const [],
            keepSources: const {0},
          ),
        );
        expect(await _opfsFileCount(dead.opfsDir!), greaterThan(0));
        dead.terminate();
        // The browser reaps the dead worker's directory on its own clock;
        // a file the page plants in a directory of its own stays until the
        // test removes it, whatever the reaper does.
        final session = await WebLaneHost.sessionDir();
        const strayDir = 'pdf_lane_stray_worker';
        final stray = await session
            .getDirectoryHandle(
              strayDir,
              web.FileSystemGetDirectoryOptions(create: true),
            )
            .toDart;
        addTearDown(() async {
          try {
            await session
                .removeEntry(
                  strayDir,
                  web.FileSystemRemoveOptions(recursive: true),
                )
                .toDart;
          } catch (_) {}
        });
        await stray
            .getFileHandle(
              'pdf_lane_stray',
              web.FileSystemGetFileOptions(create: true),
            )
            .toDart;

        final executor = await host.boot() as WebExecutor;
        addTearDown(executor.terminate);
        await expectResetGivesFreshState(executor);
      }, timeout: t(15));

      test(
        'exec and reset return only after the pre-copy file is deleted',
        () async {
          final raw = await _RawWorker.take(host, prelude: _gates);
          addTearDown(raw.close);
          const file = 'pdf_lane_awaited_drop';
          raw.precopy(1, 1, file, minimalPdf);
          await raw.waitFor(_is('opfsWriteAck'));
          raw.exec(
            1,
            1,
            _open(minimalPdf.length),
            sourceLengths: [minimalPdf.length],
            opfsFiles: [file],
          );
          await raw.waitFor(_gated('removeEntry'));
          raw.openGate('removeEntry');
          final result = await raw.waitFor(_is('result', jobId: 1));
          expect(_ok(_bytesOf(result, 'data')), isTrue);

          final removedAt = raw.history.indexWhere(_is('removed'));
          expect(
            removedAt >= 0 && removedAt < raw.history.indexOf(result),
            isTrue,
            reason: 'the result was posted while the delete was in flight',
          );
          raw.post('reset', {'lease': 1.toJS});
          final end = await raw.waitFor(
            (m) => _type(m) == 'resetDone' || _type(m) == 'error',
          );
          expect(
            _type(end),
            'resetDone',
            reason: 'reset: ${_str(end, 'message')}',
          );
        },
        timeout: t(10),
      );

      test(
        'a drain settles a job whose pre-copy write is never answered',
        () async {
          final executor =
              await withWorkerPrelude(_dropsOpfsWrite, host.boot)
                  as WebExecutor;
          addTearDown(executor.terminate);
          final dropped = EventStreamProviders.messageEvent
              .forTarget(executor.js)
              .firstWhere((e) => _type(e.data as JSObject) == 'dropped');
          final lane = executor.beginLane() as WebLane;
          final source = src(minimalPdf);
          final running = lane.submit(
            LaneJob(
              jobId: 1,
              request: _open(source.length),
              sources: [source],
              sinks: const [],
              keepSources: const {},
            ),
          );
          await dropped; // the lane is waiting for the acknowledgement

          final drained = lane.drain();
          expect((await running).bytes, buildCancelledResponse());
          expect(
            await drained.timeout(
              const Duration(seconds: 5),
              onTimeout: () => throw TestFailure(
                'the drain never settled: the job stayed running',
              ),
            ),
            isTrue,
          );
        },
        timeout: t(10),
      );

      test('a message the worker cannot admit is answered once, under its '
          'own lease', () async {
        final create = bin.encodeRequest('builderCreate', {});
        final bytes = minimalPdf;
        const file = 'pdf_lane_busy';

        // Busy with an exec, held at the delete that ends its reader.
        final execing = await _RawWorker.take(host, prelude: _gates);
        addTearDown(execing.close);
        execing.precopy(1, 1, file, bytes);
        await execing.waitFor(_is('opfsWriteAck'));
        execing.exec(
          1,
          1,
          _open(bytes.length),
          sourceLengths: [bytes.length],
          opfsFiles: [file],
        );
        await execing.waitFor(_gated('removeEntry'));
        // A newer lease cannot be adopted while busy: the verbs that wait
        // for an answer get one, the notifications that do not get none.
        execing.exec(2, 2, create);
        execing.post('reset', {'lease': 2.toJS});
        execing.precopy(2, 3, 'pdf_lane_refused', bytes);
        execing.post('readAtResult', {'lease': 2.toJS});
        execing.post('chunkAck', {'lease': 2.toJS});
        execing.post('releaseHeld', {'lease': 2.toJS, 'heldGroupId': 1.toJS});
        execing.post('opfsDrop', {'lease': 2.toJS, 'jobId': 3.toJS});
        execing.openGate('removeEntry');
        await execing.waitFor(_is('result', jobId: 1));
        _expectRefused(execing.answers(2), [2, null, 3]);
        expect(
          [for (final a in execing.answers(1)) _type(a)],
          ['opfsWriteAck', 'result'],
        );

        // Busy with a reset, held at its first delete.
        final resetting = await _RawWorker.take(host, prelude: _gates);
        addTearDown(resetting.close);
        resetting.precopy(1, 1, file, bytes);
        await resetting.waitFor(_is('opfsWriteAck'));
        resetting.post('reset', {'lease': 1.toJS});
        await resetting.waitFor(_gated('removeEntry'));
        resetting.exec(1, 2, create);
        resetting.exec(2, 3, create);
        resetting.post('reset', {'lease': 2.toJS});
        resetting.precopy(2, 4, 'pdf_lane_refused', bytes);
        resetting.openGate('removeEntry');
        await resetting.waitFor(_is('resetDone'));
        expect(
          [for (final a in resetting.answers(1)) _type(a)],
          ['opfsWriteAck', 'error', 'resetDone'],
        );
        _expectRefused(resetting.answers(2), [3, null, 4]);

        // Idle: a tenure that has ended gets no answer, a newer one is
        // adopted and served.
        resetting.exec(0, 5, create);
        resetting.post('reset', {'lease': 0.toJS});
        resetting.precopy(0, 6, file, bytes);
        resetting.exec(3, 7, create);
        await resetting.waitFor(_is('result', jobId: 7));
        expect(resetting.answers(0), isEmpty);
        expect(resetting.answers(3), hasLength(1));
      }, timeout: t(15));

      test('a pre-copied file is read through the handle that wrote it, '
          'never closed and reopened', () async {
        final raw = await _RawWorker.take(host, prelude: _refusesReopen);
        addTearDown(raw.close);
        const file = 'pdf_lane_reopen';
        raw.precopy(1, 1, file, minimalPdf);
        await raw.waitFor(_is('opfsWriteAck'));
        raw.exec(
          1,
          1,
          _open(minimalPdf.length),
          sourceLengths: [minimalPdf.length],
          opfsFiles: [file],
        );
        final done = await raw.waitFor(
          (m) => _is('result', jobId: 1)(m) || _is('error', jobId: 1)(m),
        );
        expect(
          raw.saw(_is('refused')),
          isFalse,
          reason: 'the file was opened again after its handle was closed',
        );
        expect(_type(done), 'result', reason: 'exec: ${_str(done, 'message')}');
        expect(_ok(_bytesOf(done, 'data')), isTrue);
      }, timeout: t(10));

      test('a job\'s drop waits for its pre-copy write in flight', () async {
        final raw = await _RawWorker.take(host, prelude: _holdsCreate);
        addTearDown(raw.close);
        const file = 'pdf_lane_dropped';
        raw.precopy(1, 1, file, minimalPdf);
        await raw.waitFor(_is('held'));
        // The job was cancelled: its drop reaches the worker while the
        // write is still creating the file's handle.
        raw.post('opfsDrop', {'lease': 1.toJS, 'jobId': 1.toJS});
        raw.post('ping', {});
        await raw.waitFor(_is('pong'));
        raw.post('goCreate', {});

        final ack = await raw.waitFor(_is('opfsWriteAck'));
        expect(
          raw.saw(_is('refused')),
          isFalse,
          reason: 'the drop deleted the file under the write that creates it',
        );
        expect(_str(ack, 'error'), isNull);
        raw.post('reset', {'lease': 1.toJS});
        final end = await raw.waitFor(
          (m) => _type(m) == 'resetDone' || _type(m) == 'error',
        );
        expect(_type(end), 'resetDone', reason: _str(end, 'message'));
        expect(await _opfsFileCount(raw.directory), 0);
      }, timeout: t(10));

      test('a reset does not wait for a pre-copy write whose handle creation '
          'never answers, and the file goes once it does', () async {
        final raw = await _RawWorker.take(
          host,
          prelude: _hangsCreate(every: false),
        );
        addTearDown(raw.close);
        const file = 'pdf_lane_hung';
        raw.precopy(1, 1, file, minimalPdf);
        await raw.waitFor(_is('hung'));

        // The job was cancelled: its drop and the lane's reset follow.
        raw.post('opfsDrop', {'lease': 1.toJS, 'jobId': 1.toJS});
        raw.post('reset', {'lease': 1.toJS});
        final done = await raw
            .waitFor(_is('resetDone'))
            .timeout(
              const Duration(seconds: 5),
              onTimeout: () => throw TestFailure(
                'the reset waited for the write behind a '
                'createSyncAccessHandle that never answered',
              ),
            );
        expect(_strings(done, 'abandoned'), [file]);

        raw.post('release', {});
        await _until(
          () async => await _opfsFileCount(raw.directory) == 0,
          'the set-aside file stayed after its create answered',
        );
        final before = raw.history.where(_is('resetDone')).toList();
        raw.post('reset', {'lease': 1.toJS});
        final again = await raw.waitFor(
          (m) => _is('resetDone')(m) && !before.any((b) => identical(b, m)),
        );
        expect(_strings(again, 'abandoned'), isEmpty);
      }, timeout: t(15));

      test('a create that does not answer is retried once on a fresh file, '
          'and the job succeeds', () async {
        final executor =
            await withWorkerPrelude(_hangsCreate(every: false), host.boot)
                as WebExecutor;
        addTearDown(executor.terminate);
        executor.provoked = true;
        final watched = _Watched(executor);
        addTearDown(watched.close);

        final lane = executor.beginLane() as WebLane;
        final source = src(minimalPdf);
        final opened = await lane.submit(
          LaneJob(
            jobId: 1,
            request: _open(source.length),
            sources: [source],
            sinks: const [],
            keepSources: const {},
          ),
        );
        expect(
          _ok(opened.bytes),
          isTrue,
          reason: 'the job: ${_failure(opened.bytes)}',
        );
        expect(watched.seen.where(_is('hung')), hasLength(1));
        expect(executor.retriedCreates, 1);
        expect(WebLaneHost.retriedCreates, 0, reason: 'a provoked worker');

        executor.js.postMessage(_msg('release', {}));
        await _until(
          () async => await _opfsFileCount(executor.opfsDir!) == 0,
          'the abandoned file stayed after its create answered',
        );
        await lane.drain();
        await lane.reset();
        expect(executor.setAsideFiles, isEmpty);
      }, timeout: t(20));

      test('a create that never answers fails the job with an I/O error '
          'within twice the bound', () async {
        final executor =
            await withWorkerPrelude(_hangsCreate(every: true), host.boot)
                as WebExecutor;
        addTearDown(executor.terminate);
        executor.provoked = true;
        final watched = _Watched(executor);
        addTearDown(watched.close);

        final lane = executor.beginLane() as WebLane;
        final source = src(minimalPdf);
        final clock = Stopwatch()..start();
        final failed = await lane.submit(
          LaneJob(
            jobId: 1,
            request: _open(source.length),
            sources: [source],
            sinks: const [],
            keepSources: const {},
          ),
        );
        clock.stop();
        expect(
          () => bin.decodeResponse(failed.bytes),
          throwsA(
            isA<PdfIoFailed>().having(
              (e) => e.message,
              'message',
              allOf(
                contains('createSyncAccessHandle'),
                contains('did not answer'),
              ),
            ),
          ),
        );
        printOnFailure('the job failed after ${clock.elapsedMilliseconds} ms');
        // Two waits of the bound, one per file, and the job's own work.
        expect(
          clock.elapsed,
          lessThan(kCreateHandleWait * 2 + const Duration(seconds: 1)),
        );
        expect(watched.seen.where(_is('hung')), hasLength(2));
        expect(executor.retriedCreates, 1);

        executor.js.postMessage(_msg('release', {}));
        await _until(
          () async => await _opfsFileCount(executor.opfsDir!) == 0,
          'the abandoned files stayed after their creates answered',
        );
      }, timeout: t(20));

      test('a reset stuck behind a pre-copy write names the call it waits '
          'for', () async {
        final executor =
            await withWorkerPrelude(_hangsCreate(every: true), host.boot)
                as WebExecutor;
        addTearDown(executor.terminate);
        executor.provoked = true;
        final watched = _Watched(executor);
        addTearDown(watched.close);

        final lane = executor.beginLane() as WebLane;
        final source = src(minimalPdf);
        final running = lane.submit(
          LaneJob(
            jobId: 1,
            request: _open(source.length),
            sources: [source],
            sinks: const [],
            keepSources: const {},
          ),
        );
        final hung = await watched.next(_is('hung'));
        executor.js.postMessage(_msg('holdReset', {}));
        lane.cancelJob(1);
        expect((await running).bytes, buildCancelledResponse());
        expect(await lane.drain(), isTrue);

        unawaited(lane.reset().then((_) => 0, onError: (_) => 0));
        expect(
          lane.phase,
          allOf(
            contains('awaiting resetDone'),
            contains(_str(hung, 'name')!),
            contains('createSyncAccessHandle'),
          ),
        );

        executor.js.postMessage(_msg('release', {}));
        await _until(
          () async => await _opfsFileCount(executor.opfsDir!) == 0,
          'the abandoned files stayed after their creates answered',
        );
      }, timeout: t(20));

      test('a rejected lane directory is asked for again', () async {
        final raw = await _RawWorker.take(host, prelude: _rejectsDirOnce);
        addTearDown(raw.close);
        raw.precopy(1, 1, 'pdf_lane_first', minimalPdf);
        final first = await raw.waitFor(_is('opfsWriteAck'));
        expect(raw.saw(_is('rejected')), isTrue);
        expect(_str(first, 'error'), isNotNull, reason: 'the directory failed');

        raw.precopy(1, 2, 'pdf_lane_second', minimalPdf);
        final second = await raw.waitFor(_is('opfsWriteAck', jobId: 2));
        expect(
          _str(second, 'error'),
          isNull,
          reason: 'the worker kept the rejection of its directory',
        );
      }, timeout: t(10));

      test('a late pre-copy ack of one job does not complete the write of '
          'another', () async {
        final executor =
            await withWorkerPrelude(_holdsAcks, host.boot) as WebExecutor;
        addTearDown(executor.terminate);
        final seen = <JSObject>[];
        final arrivals = StreamController<JSObject>.broadcast(sync: true);
        addTearDown(arrivals.close);
        final sub = EventStreamProviders.messageEvent
            .forTarget(executor.js)
            .listen((e) {
              final m = e.data as JSObject;
              seen.add(m);
              arrivals.add(m);
            });
        addTearDown(sub.cancel);
        Future<JSObject> next(bool Function(JSObject) test) {
          for (final m in seen) {
            if (test(m)) return Future.value(m);
          }
          return arrivals.stream.firstWhere(test);
        }

        final lane = executor.beginLane() as WebLane;
        LaneJob open(int id) {
          final source = src(minimalPdf);
          return LaneJob(
            jobId: id,
            request: _open(source.length),
            sources: [source],
            sinks: const [],
            keepSources: const {},
          );
        }

        final first = lane.submit(open(1));
        final second = lane.submit(open(2));
        await next(_is('ackHeld', jobId: 1));
        lane.cancelJob(1);
        expect((await first).bytes, buildCancelledResponse());
        await next(_is('ackHeld', jobId: 2));

        // Job 1's ack arrives while job 2's write is the one pending.
        executor.js.postMessage(_msg('ackRelease', {'jobId': 1.toJS}));
        await next(_is('ackReleased', jobId: 1));
        executor.js.postMessage(_msg('ping', {}));
        await next(_is('pong'));
        expect(
          seen.any(_is('execSeen', jobId: 2)),
          isFalse,
          reason: 'job 2 left its pre-copy on the ack of job 1',
        );

        executor.js.postMessage(_msg('ackRelease', {'jobId': 2.toJS}));
        expect(_ok((await second).bytes), isTrue);
      }, timeout: t(10));

      test('a cancel poll that times out does not cancel the job', () async {
        final raw = await _RawWorker.take(host, prelude: _timesOutOnePoll);
        addTearDown(raw.close);
        final url = web.URL.createObjectURL(web.Blob(<JSAny>[].toJS));
        addTearDown(() => web.URL.revokeObjectURL(url));
        const file = 'pdf_lane_poll';
        raw.precopy(1, 1, file, fThousandPage);
        await raw.waitFor(_is('opfsWriteAck'));
        raw.exec(
          1,
          1,
          _open(fThousandPage.length),
          sourceLengths: [fThousandPage.length],
          opfsFiles: [file],
          keepSources: const [0],
        );
        final opened = await raw.waitFor(_is('result', jobId: 1));
        final handle = peekResponseHandleId(_bytesOf(opened, 'data'))!;

        raw.post('xhrArm', {});
        raw.exec(
          1,
          2,
          bin.encodeRequest('extract', {
            'handleId': handle,
            'format': 'plainText',
            'hasSink': true,
          }),
          heldGroupId: 1,
          sinkCount: 1,
          cancelUrl: url,
        );
        final done = await raw.waitFor(_is('result', jobId: 2));
        expect(
          raw.saw(_is('xhrTimeout')),
          isTrue,
          reason: 'no cancel poll ran: the job was too short to reach one',
        );
        final poll = raw.history.firstWhere(_is('xhrSend'));
        expect(
          _num(poll, 'timeout'),
          greaterThan(0),
          reason: 'the poll is a synchronous XHR without a timeout',
        );
        expect(
          _ok(_bytesOf(done, 'data')),
          isTrue,
          reason: 'the poll that timed out cancelled the job',
        );
      }, timeout: t(15));
    }
  });
}
