// WebLaneHost — worker physics for the web lane runtime.
//
// Part of lane.dart (the lane itself lives there). This file owns
// everything about WORKERS AS A RESOURCE, nothing about jobs and nothing
// about how many exist or who holds them (the pool does):
//
//   - the boot handshake (spawn → booted → init → ready): the worker
//     announces when its message handler is attached, because a
//     message posted before that is silently dropped
//   - terminating a worker and reclaiming its OPFS directory
//   - WASM module compile-once caching + I/O mode detection

part of 'lane.dart';

// ── Host ────────────────────────────────────────────────────────────

/// Spawns web lanes. Detects the I/O mode once and pre-compiles the
/// WASM module once per page (workers receive the compiled module).
class WebLaneHost implements LaneHost {
  /// Creates a host. [laneWorkerUrl] locates lane_worker.js;
  /// [forceMode] overrides auto-detection (test runners use this).
  WebLaneHost({String? laneWorkerUrl, PdfIoMode? forceMode})
    : _laneWorkerUrl =
          laneWorkerUrl ?? _cachedWorkerUrl ?? 'pdf_manipulator/lane_worker.js',
      mode = _detectMode(forceMode ?? _cachedForceMode) {
    // Page-level config inheritance: assets live in ONE place per
    // page, so explicitly-configured URLs/mode become the default
    // for later bare Pdf() instances (test generators, secondary
    // instances in apps).
    if (laneWorkerUrl != null) _cachedWorkerUrl = laneWorkerUrl;
    if (forceMode != null) _cachedForceMode = forceMode;
    // Claim this page-session's liveness lock and reclaim dead
    // sessions' OPFS files (once per page).
    _startSessionOnce();
  }

  static String? _cachedWorkerUrl;
  static PdfIoMode? _cachedForceMode;

  // ── OPFS namespace + liveness-keyed reclamation ──
  //
  // Pre-copied source files live under ONE package-owned directory,
  // namespaced per page session, then per worker:
  //
  //     pdf_manipulator_lanes/{sessionId}/{workerId}/pdf_lane_…
  //
  // The cleanup rule: NEVER race the living, ONLY sweep the dead.
  //
  //   living — a worker deletes its own files (job end, release):
  //            same agent as the open handles, so no race exists.
  //   dead   — every owner holds a Web Lock for its lifetime; the
  //            browser releases it on agent death (a spec guarantee
  //            — the web's analogue of the native lane's every-job-
  //            posts-exactly-once contract). An acquirable lock IS
  //            the death certificate; only then is the directory
  //            reclaimed.
  //
  // Two reclaim layers, same key: a retired worker's dir (its lock,
  // on terminate) and a dead page's whole session dir (its lock, at
  // the next session's boot). GC hooks are deliberately NOT used:
  // FinalizationRegistry is best-effort by spec and never fires on
  // tab close.

  /// The package-owned OPFS directory. Everything inside it is
  /// reconciled by this host; nothing outside is ever touched.
  static const opfsRootDir = 'pdf_manipulator_lanes';

  /// This page session's namespace (random per page load).
  static final String opfsSessionId = _newSessionId();

  static String _newSessionId() {
    final r = math.Random();
    return List.generate(16, (_) => r.nextInt(16).toRadixString(16)).join();
  }

  static String _sessionLockName(String sessionId) => '$opfsRootDir/$sessionId';

  static String _workerLockName(String workerId) =>
      '$opfsRootDir/$opfsSessionId/$workerId';

  static bool _sessionStarted = false;

  static void _startSessionOnce() {
    if (_sessionStarted) return;
    _sessionStarted = true;
    // Hold this session's lock until the page dies — the browser
    // releases it on tab close/crash, which is exactly the liveness
    // signal the reclaimer keys on.
    final never = Completer<JSAny?>();
    unawaited(
      web.window.navigator.locks
          .request(
            _sessionLockName(opfsSessionId),
            ((web.Lock? lock) => never.future.toJS).toJS,
          )
          .toDart
          .catchError((Object _) => null),
    );
    unawaited(reclaimDeadSessions());
  }

  /// Opens (creating as needed) this session's OPFS directory.
  static Future<web.FileSystemDirectoryHandle> sessionDir() async {
    final root = await web.window.navigator.storage.getDirectory().toDart;
    final ours = await root
        .getDirectoryHandle(
          opfsRootDir,
          web.FileSystemGetDirectoryOptions(create: true),
        )
        .toDart;
    return ours
        .getDirectoryHandle(
          opfsSessionId,
          web.FileSystemGetDirectoryOptions(create: true),
        )
        .toDart;
  }

  /// Deletes every session directory whose liveness lock is free —
  /// its page is gone, so its files are garbage. Never touches a
  /// directory whose lock is held (including the current session's).
  /// Safe to call any time; visible for tests.
  static Future<void> reclaimDeadSessions() async {
    final web.FileSystemDirectoryHandle ours;
    try {
      final root = await web.window.navigator.storage.getDirectory().toDart;
      ours = await root.getDirectoryHandle(opfsRootDir).toDart;
    } catch (_) {
      return; // no directory yet — nothing to reclaim
    }

    final sessions = <String>[];
    final iter = (ours as JSObject).callMethod<JSObject>('keys'.toJS);
    while (true) {
      final next = await iter
          .callMethod<JSPromise<JSObject>>('next'.toJS)
          .toDart;
      if ((next['done']! as JSBoolean).toDart) break;
      sessions.add((next['value']! as JSString).toDart);
    }

    for (final session in sessions) {
      if (session == opfsSessionId) continue;
      await web.window.navigator.locks
          .request(
            _sessionLockName(session),
            web.LockOptions(ifAvailable: true),
            ((web.Lock? lock) {
              if (lock == null) return null; // owner alive — skip
              return ours
                  .removeEntry(
                    session,
                    web.FileSystemRemoveOptions(recursive: true),
                  )
                  .toDart
                  .catchError((Object _) => null)
                  .toJS;
            }).toJS,
          )
          .toDart
          .catchError((Object _) => null);
    }
  }

  /// Reclaims a terminated worker's OPFS directory once the browser
  /// confirms the agent is dead. `terminate()` is a request: the worker's
  /// WASM memory — and the handles it held on its files — are released
  /// later, asynchronously, so the directory cannot be deleted at the call.
  ///
  /// The worker's liveness lock is the death certificate (the same signal
  /// OPFS session reclaim keys on); a used OPFS worker's directory is
  /// deleted under the same grant. Where Web Locks are unavailable
  /// (non-secure context) no death signal exists, and the next session
  /// sweep remains the OPFS backstop.
  static Future<void> _reapDeadWorker(WebExecutor worker) async {
    try {
      await web.window.navigator.locks
          .request(
            worker.lockName,
            ((web.Lock? lock) {
              final dir = worker.opfsDir;
              final done = dir == null
                  ? Future<void>.value()
                  : _deleteDeadWorkerDir(dir);
              return done.toJS;
            }).toJS,
          )
          .toDart;
    } catch (_) {
      // No death signal available — the session sweep reclaims it.
    }
  }

  static final List<String> _abandonedDirs = [];
  static final Map<String, List<String>> _setAsideFiles = {};
  static int _retriedCreates = 0;

  /// Pre-copy files that live workers set aside because the browser never
  /// answered the creation of their handle, and have not removed yet, as the
  /// last reset of each worker reported them (`worker directory/file`). A
  /// file leaves with its pending call. Tests assert it is empty; nothing
  /// decides on it. Workers a test marked [WebExecutor.provoked] are not in
  /// it.
  static List<String> get abandonedFiles => List.unmodifiable([
    for (final MapEntry(:key, :value) in _setAsideFiles.entries)
      for (final name in value) '$key/$name',
  ]);

  /// Pre-copy writes since the process started that created their handle a
  /// second time because the first call passed [kCreateHandleWait]
  /// unanswered. Tests assert it is zero; nothing decides on it. Workers a
  /// test marked [WebExecutor.provoked] are not counted.
  static int get retriedCreates => _retriedCreates;

  /// Directories of dead workers that [_deleteDeadWorkerDir] gave up on,
  /// by name: each one stays on disk until the next session's sweep. Tests
  /// assert it is empty; nothing decides on it.
  static List<String> get abandonedWorkerDirs =>
      List.unmodifiable(_abandonedDirs);

  /// Deletes a PROVABLY-DEAD worker's directory. The only convergent
  /// loop in the runtime, and the only one that is sound: the owner
  /// is gone, so the browser's asynchronous reaping of its handles is
  /// the sole obstacle — and it always completes. (Against a LIVE
  /// owner the same loop could lose forever; that case cannot reach
  /// here.)
  static Future<void> _deleteDeadWorkerDir(String workerId) async {
    final session = await sessionDir();
    var delay = const Duration(milliseconds: 25);
    for (var attempt = 0; attempt < 10; attempt++) {
      try {
        await session.getDirectoryHandle(workerId).toDart;
      } catch (_) {
        return; // no directory — the worker never wrote a file
      }
      try {
        await session
            .removeEntry(workerId, web.FileSystemRemoveOptions(recursive: true))
            .toDart;
        return;
      } catch (_) {
        // Handles not reaped yet — converge.
      }
      await Future<void>.delayed(delay);
      delay *= 2;
    }
    _abandonedDirs.add(workerId);
  }

  final String _laneWorkerUrl;

  /// The detected (or forced) I/O mode — the Router reports this as
  /// the instance's [PdfIoMode].
  @override
  final PdfIoMode mode;

  @override
  int get defaultLaneCount =>
      suggestedLaneCount(web.window.navigator.hardwareConcurrency);

  static JSObject? _cachedModule;
  static Future<void>? _compileInFlight;

  @override
  String get poolKey => '$_laneWorkerUrl#${mode.name}';

  @override
  Future<Executor> boot() => _bootWorker();

  /// Spawn a worker and drive its boot handshake to `ready`. The
  /// temporary listener is detached before the worker is handed out;
  /// an idle ready worker sends nothing, so no message can be lost.
  Future<WebExecutor> _bootWorker() async {
    // Every worker holds a liveness lock for its lifetime, in every
    // mode: it is the death signal that reclaims its files. Only OPFS mode
    // also gives the worker a directory under the same name.
    final workerId = _newSessionId();
    final lockName = _workerLockName(workerId);
    final opfsDir = mode == PdfIoMode.opfs ? workerId : null;
    final worker = _spawnWorker();
    // Everything after the spawn runs inside the try: a throw anywhere here
    // must still terminate the worker.
    StreamSubscription<web.MessageEvent>? sub;
    StreamSubscription<web.Event>? errSub;
    try {
      final booted = Completer<void>();
      final ready = Completer<void>();
      // On boot failure both completers carry the error but only `booted`
      // is awaited (its throw aborts before the `ready` await exists) —
      // mark `ready` observed so the abandoned error can't surface as an
      // unhandled async error in the caller's zone.
      ready.future.ignore();
      sub = EventStreamProviders.messageEvent.forTarget(worker).listen((event) {
        final obj = event.data as JSObject?;
        if (obj == null) return;
        final type = (obj[LaneMsgFields.type] as JSString?)?.toDart;
        if (type == LaneMsg.booted && !booted.isCompleted) {
          booted.complete();
        } else if (type == LaneMsg.ready && !ready.isCompleted) {
          ready.complete();
        } else if ((type == LaneMsg.error || type == LaneMsg.bootFailed) &&
            !ready.isCompleted) {
          final error = StateError(
            (obj['error'] as JSString?)?.toDart ?? 'web lane init failed',
          );
          if (!booted.isCompleted) booted.completeError(error);
          ready.completeError(error);
        }
      });
      errSub = EventStreamProviders.errorEvent.forTarget(worker).listen((
        event,
      ) {
        if (!ready.isCompleted) {
          final error = StateError('worker script failed to load');
          if (!booted.isCompleted) booted.completeError(error);
          ready.completeError(error);
        }
      });

      final wasmModule = await _wasmModule();
      // The worker announces `booted` once its message handler is
      // attached; posting before that drops the message silently
      // (the blob bootstrap's dynamic import is not part of module
      // evaluation, so the port enables before the handler exists).
      // No deadline: every boot-failure class completes this future
      // with a typed error (bootfailed post / worker error event), so
      // a wall clock here would only kill healthy boots on slow
      // networks. Timeouts are never load-bearing.
      await booted.future;
      final init = JSObject()
        ..[LaneMsgFields.type] = LaneMsg.init.toJS
        ..['ioMode'] = mode.name.toJS
        ..['baseUrl'] = _baseUrl.toJS
        ..['protocol'] = () {
          final p = JSObject();
          laneProtocolCodes().forEach((k, v) => p[k] = v.toJS);
          return p;
        }();
      if (wasmModule != null) init['wasmModule'] = wasmModule;
      init['livenessLock'] = lockName.toJS;
      if (opfsDir != null) {
        init['opfsDir'] = [
          opfsRootDir.toJS,
          opfsSessionId.toJS,
          opfsDir.toJS,
        ].toJS;
      }
      worker.postMessage(init);

      // No deadline here either: asset-fetch and engine-init failures
      // arrive as worker error messages; a 21MB wasm on a slow network
      // takes as long as it takes and is not a failure.
      await ready.future;
      return WebExecutor._(worker, opfsDir, lockName, mode)..attach();
    } catch (_) {
      // A worker that never readied wrote no files (writing requires
      // work, work requires ready) — no directory to reclaim.
      worker.terminate();
      rethrow;
    } finally {
      unawaited(sub?.cancel() ?? Future<void>.value());
      unawaited(errSub?.cancel() ?? Future<void>.value());
    }
  }

  static PdfIoMode _detectMode(PdfIoMode? force) {
    if (force != null) return force;
    final wasm = globalContext['WebAssembly'];
    if (wasm != null &&
        (wasm as JSObject).hasProperty('Suspending'.toJS).toDart) {
      return PdfIoMode.jspi;
    }
    if (globalContext.hasProperty('SharedArrayBuffer'.toJS).toDart) {
      return PdfIoMode.atomics;
    }
    return PdfIoMode.opfs;
  }

  String get _baseUrl {
    final url = web.URL(_laneWorkerUrl, web.window.location.href);
    final s = url.href;
    return s.substring(0, s.lastIndexOf('/') + 1);
  }

  /// Compile the WASM once per page; failures fall back to letting
  /// each worker stream-compile its own copy.
  Future<JSObject?> _wasmModule() async {
    if (_cachedModule != null) return _cachedModule;
    _compileInFlight ??= () async {
      try {
        final response = await _fetch(
          '${_baseUrl}pdf_oxide_bg.wasm'.toJS,
        ).toDart;
        _cachedModule = await _wasmCompileStreaming(response).toDart;
      } catch (_) {
        _cachedModule = null;
      }
    }();
    await _compileInFlight;
    return _cachedModule;
  }

  web.Worker _spawnWorker() {
    final url = _laneWorkerUrl;
    if (url.startsWith('http://') || url.startsWith('https://')) {
      // Cross-origin: bootstrap through a same-origin blob. The import
      // failure MUST be caught and posted — a rejected dynamic import
      // is an unhandled rejection inside the worker, which never fires
      // the Worker error event. Without this post, an unreachable URL
      // would hang the boot forever.
      final script =
          "import('${url.replaceAll("'", r"\'")}')"
          ".catch((e) => postMessage({type: 'bootfailed', error: String(e)}));";
      final blob = web.Blob(
        [script.toJS].toJS,
        web.BlobPropertyBag(type: 'application/javascript'),
      );
      return web.Worker(
        web.URL.createObjectURL(blob).toJS,
        web.WorkerOptions(type: 'module'),
      );
    }
    return web.Worker(url.toJS, web.WorkerOptions(type: 'module'));
  }
}

// ── A booted worker: the web executor ─────────────────────────────

/// A ready worker with its OPFS identity, as the pool leases it. [opfsDir]
/// is the worker's own directory name under the session (OPFS mode only) —
/// the key the host reclaims by after the worker is terminated.
class WebExecutor implements Executor {
  WebExecutor._(this.js, this.opfsDir, this.lockName, this._mode);

  /// The underlying Web Worker.
  final web.Worker js;

  /// The worker's own directory name under the session, or null when
  /// the mode touches no disk.
  final String? opfsDir;

  /// The Web Lock the worker holds from before `ready` until the
  /// browser tears it down — the host's death signal.
  final String lockName;

  final PdfIoMode _mode;
  WebLane? _lane;
  StreamSubscription<web.MessageEvent>? _sub;
  int _lastLease = 0;
  int _heapBytes = 0;
  int _memBytes = 0;
  bool _dead = false;
  bool _terminated = false;

  /// Set by a test that makes the browser's handle creation hang on purpose:
  /// this worker's set-aside files and retried creates are kept here and
  /// stay out of [WebLaneHost.abandonedFiles] and
  /// [WebLaneHost.retriedCreates], which judge the runs that did not.
  bool provoked = false;

  /// The files this worker reported set aside at its last reset.
  List<String> get setAsideFiles => List.unmodifiable(_setAside);
  List<String> _setAside = const [];

  /// How many creates of this worker were retried.
  int get retriedCreates => _retriedCreates;
  int _retriedCreates = 0;

  void _retriedCreate() {
    _retriedCreates++;
    if (!provoked) WebLaneHost._retriedCreates++;
  }

  void _setAsideNow(List<String> names) {
    _setAside = names;
    if (_terminated || provoked) return;
    if (names.isEmpty) {
      WebLaneHost._setAsideFiles.remove(opfsDir ?? lockName);
    } else {
      WebLaneHost._setAsideFiles[opfsDir ?? lockName] = names;
    }
  }

  @override
  int get heapBytes => _heapBytes;

  /// The worker's WASM linear memory as of its last answer; it never falls.
  /// A diagnostic: no decision reads it.
  int get memBytes => _memBytes;

  /// Routes the worker's messages to the lane that holds the lease now.
  void attach() {
    _sub = EventStreamProviders.messageEvent.forTarget(js).listen((event) {
      _lane?.deliver(event.data as JSObject?);
    });
  }

  /// Starts a new lease and returns its lane. One counter per worker: ids
  /// count up from 1 and are never reused, so a smaller id always names a
  /// tenure that has ended.
  @override
  Lane beginLane() {
    if (_dead || _terminated) throw const LaneProtocolError(laneDeadMessage);
    return _lane = WebLane._(this, ++_lastLease);
  }

  @override
  void terminate() {
    if (_terminated) return;
    _terminated = true;
    WebLaneHost._setAsideFiles.remove(opfsDir ?? lockName);
    // Browser kill: frees the worker, its WASM heap, and every open
    // SyncAccessHandle in one stroke. Nothing can call back after.
    unawaited(_sub?.cancel());
    _sub = null;
    js.terminate();
    _lane?._abandon();
    unawaited(WebLaneHost._reapDeadWorker(this));
  }
}
