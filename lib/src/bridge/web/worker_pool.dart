// Web Worker pool — session-based, pin operation to worker.
//
// One operation = one worker, start to finish. No operation hops
// between workers. This mirrors native (one task = one pool thread)
// and eliminates OPFS SyncAccessHandle lock conflicts.
//
// Pool size: max(2, hardwareConcurrency ~/ 2).
// Cancel = Worker.terminate() (instant kill, WASM memory freed by browser).
// Dispose = terminate all + OPFS cleanup.
//
// INTERNAL — used by WebBridge only.

import 'dart:async';
import 'dart:collection';
import 'dart:js_interop';
import 'dart:js_interop_unsafe';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

/// A dedicated worker for one operation's entire lifecycle.
///
/// All messages in a session go to the same worker — guaranteed
/// sequential, same OPFS context, same WASM instance. The worker
/// never serves another operation until this session is released.
class WebWorkerSession {
  WebWorkerSession._(this._worker);

  final web.Worker _worker;
  final _pending = <int, Completer<Map<Object?, Object?>>>{};
  int _nextMsgId = 0;
  bool _terminated = false;

  void Function(int id, Map<Object?, Object?> data)? onChunk;
  void Function(int id, Map<Object?, Object?> data)? onStreamItem;

  void _setupListener() {
    _worker.onmessage = (web.MessageEvent e) {
      final data = (e.data as JSAny).dartify()! as Map<Object?, Object?>;
      final type = data['type'] as String;

      if (type == 'ready') return;

      if (type == 'result') {
        final id = data['id'] as int;
        _pending.remove(id)?.complete(
            data['result'] as Map<Object?, Object?>? ?? {});
      } else if (type == 'error') {
        final id = data['id'] as int;
        _pending.remove(id)?.completeError(Exception('${data['error']}'));
      } else if (type == 'chunk') {
        final id = data['id'] as int;
        onChunk?.call(id, data);
      } else if (type == 'streamItem') {
        final id = data['id'] as int;
        onStreamItem?.call(id, data);
      }
    }.toJS;
  }

  /// Send a message to this session's worker and await the response.
  Future<Map<Object?, Object?>> send(
    String op,
    Map<String, Object?> rawArgs, {
    List<Object>? transferBuffers,
  }) {
    if (_terminated) throw StateError('Session terminated');

    final id = _nextMsgId++;
    final completer = Completer<Map<Object?, Object?>>();
    _pending[id] = completer;

    final jsArgs = JSObject();
    final transfers = <JSObject>[];
    for (final entry in rawArgs.entries) {
      final v = entry.value;
      if (v == null) {
        jsArgs[entry.key] = null;
      } else if (v is ByteBuffer) {
        final ab = v.toJS;
        jsArgs[entry.key] = ab;
        transfers.add(ab);
      } else {
        jsArgs[entry.key] = v.jsify();
      }
    }

    final jsMsg = JSObject();
    jsMsg['id'] = id.toJS;
    jsMsg['op'] = op.toJS;
    jsMsg['args'] = jsArgs;

    if (transfers.isNotEmpty) {
      _worker.postMessage(jsMsg, transfers.toJS);
    } else {
      _worker.postMessage(jsMsg);
    }

    return completer.future;
  }

  /// Terminate this worker. All pending operations fail.
  void terminate() {
    if (_terminated) return;
    _terminated = true;
    _worker.terminate();
    for (final c in _pending.values) {
      if (!c.isCompleted) {
        c.completeError(StateError('Worker terminated'));
      }
    }
    _pending.clear();
  }
}

/// Fixed-size Web Worker pool with session-based acquisition.
///
/// Operations acquire a session (dedicated worker), use it for
/// their entire lifecycle (OPFS write + engine open + process +
/// output + cleanup), then release it back to the pool.
class WebWorkerPool {
  WebWorkerPool({required this.workerUrl, int? poolSize})
      : size = poolSize ?? _defaultSize();

  final String workerUrl;
  final int size;

  final _idle = <WebWorkerSession>[];
  final _waiters = Queue<Completer<WebWorkerSession>>();
  int _totalCreated = 0;
  bool _disposed = false;

  static int _defaultSize() {
    try {
      final n = web.window.navigator.hardwareConcurrency;
      return (n ~/ 2).clamp(2, 8);
    } catch (_) {
      return 2;
    }
  }

  /// Acquire a dedicated worker for one operation.
  /// If all workers are busy, the caller waits until one is released.
  Future<WebWorkerSession> acquire() async {
    if (_disposed) throw StateError('Pool is disposed');

    if (_idle.isNotEmpty) return _idle.removeLast();

    if (_totalCreated < size) {
      return await _createSession();
    }

    final c = Completer<WebWorkerSession>();
    _waiters.add(c);
    return c.future;
  }

  /// Release the worker back to the pool after the operation finishes.
  void release(WebWorkerSession session) {
    if (_disposed) {
      session.terminate();
      return;
    }

    session.onChunk = null;
    session.onStreamItem = null;

    if (_waiters.isNotEmpty) {
      _waiters.removeFirst().complete(session);
    } else {
      _idle.add(session);
    }
  }

  /// Terminate ALL workers. Clean up everything.
  void dispose() {
    if (_disposed) return;
    _disposed = true;

    for (final s in _idle) {
      s.terminate();
    }
    _idle.clear();
    _totalCreated = 0;

    for (final c in _waiters) {
      if (!c.isCompleted) {
        c.completeError(StateError('Pool disposed'));
      }
    }
    _waiters.clear();
  }

  int get idleCount => _idle.length;
  int get busyCount => _totalCreated - _idle.length;
  int get queueLength => _waiters.length;

  String? _resolvedWorkerUrl;

  Future<String> _resolveWorkerUrl() async {
    if (_resolvedWorkerUrl != null) return _resolvedWorkerUrl!;

    if (workerUrl.startsWith('http://') || workerUrl.startsWith('https://')) {
      final baseUrl =
          workerUrl.substring(0, workerUrl.lastIndexOf('/') + 1);
      final resp = await web.window.fetch(workerUrl.toJS).toDart;
      if (!resp.ok) {
        throw Exception('Failed to fetch worker.js: ${resp.status}');
      }
      final text = (await resp.text().toDart).toDart;

      final rewritten = text.replaceAllMapped(
        RegExp(r"""from\s+['"](\./[^'"]+)['"]"""),
        (m) => "from '$baseUrl${m.group(1)!.substring(2)}'",
      );

      final blob = web.Blob(
        [rewritten.toJS].toJS,
        web.BlobPropertyBag(type: 'application/javascript'),
      );
      _resolvedWorkerUrl = web.URL.createObjectURL(blob);
    } else {
      _resolvedWorkerUrl = workerUrl;
    }
    return _resolvedWorkerUrl!;
  }

  Future<WebWorkerSession> _createSession() async {
    _totalCreated++;
    final url = await _resolveWorkerUrl();
    final raw = web.Worker(url.toJS, web.WorkerOptions(type: 'module'));
    final session = WebWorkerSession._(raw);
    session._setupListener();
    return session;
  }
}
