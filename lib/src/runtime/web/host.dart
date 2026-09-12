// WebLaneHost — worker fleet physics for the web lane runtime.
//
// Part of lane.dart (the lane itself lives there). This file owns
// everything about WORKERS AS A RESOURCE, nothing about jobs:
//
//   - the boot handshake (spawn → booted → init → ready): the worker
//     announces when its message handler is attached, because a
//     message posted before that is silently dropped
//   - the page-global worker budget (FIFO waiters past the cap — an
//     op can wait, it can never fail for capacity)
//   - the pristine pool: a lane killed before posting any work
//     returns its worker here instead of terminating it, so rapid
//     create+dispose churn recycles workers instead of booting
//     thousands
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
    // Start warming one pristine worker — the first lane of the
    // first instance adopts an already-booted worker.
    _prewarm();
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

  /// Terminates [worker] and returns its budget slot only once the
  /// browser confirms the agent is dead. `terminate()` is a request:
  /// the worker's WASM memory — and the address-space reservation the
  /// browser holds for it — is released later, asynchronously. A slot
  /// freed at the call let rapid create+dispose churn boot a new
  /// worker into that window, and its instantiate failed with "Cannot
  /// allocate Wasm memory" while dozens of dead workers were still
  /// being reaped. The budget counts what the browser holds, not what
  /// we asked for — the web twin of the native lane table, where a
  /// dying thread's last act is handing its slot to the next waiter.
  ///
  /// The worker's liveness lock is the death certificate (the same
  /// signal OPFS reclaim keys on); a used OPFS worker's directory is
  /// deleted under the same grant. Where Web Locks are unavailable
  /// (non-secure context) no death signal exists, so the slot frees
  /// at once and the next session sweep remains the OPFS backstop.
  void _terminateAndReleaseSlot(LaneWorker worker) {
    worker.js.terminate();
    unawaited(_releaseSlotWhenDead(worker));
  }

  static Future<void> _releaseSlotWhenDead(LaneWorker worker) async {
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
      // No death signal available — fall through to the release.
    } finally {
      _liveBudget--;
      _wakeWaiter();
    }
  }

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

  // ── Page-global worker budget + pristine pool ──
  //
  // Mirrors the native global lane budget: worker boots are capped
  // page-wide; takers past the cap queue FIFO and are woken as
  // workers free — an op can wait, it can never fail for capacity.
  //
  // The pool holds PRISTINE workers (booted + inited, never given
  // work). A lane killed before posting any work returns its worker
  // here instead of terminating it, so rapid create+dispose churn
  // recycles one worker instead of booting thousands — without this,
  // a 5000x instance churn floods the page with concurrent worker
  // boots and stalls every later boot.

  /// Page-wide cap on live workers (adopted + pooled + booting).
  /// Mirrors the native global lane budget's headroom: instances the
  /// app never disposes hold their worker, so the cap must dwarf any
  /// sane leak count or idle instances starve fresh ones. Fixed, not
  /// device-derived: behavior must not vary by machine.
  static const _maxWorkers = 64;

  /// Pristine pool ceiling per key — beyond this a returned worker is
  /// terminated so pooled idle workers can't starve the budget.
  static const _maxPooled = 4;

  static int _liveBudget = 0;
  static final Map<String, List<LaneWorker>> _pool = {};
  static final Set<String> _warming = {};
  static final List<Completer<void>> _waiters = [];

  String get _spareKey => '$_laneWorkerUrl#${mode.name}';

  /// Adopt a pooled pristine worker, boot one under the budget, or
  /// queue until a slot frees. Never fails for capacity.
  ///
  /// [isKilled] is the requesting lane's kill flag: a lane that dies
  /// while queued must take nothing — and must pass its wake signal
  /// on, or one dead waiter strands every live waiter behind it.
  Future<LaneWorker?> takeWorker(bool Function() isKilled) async {
    while (true) {
      if (isKilled()) return null;
      final pool = _pool[_spareKey];
      if (pool != null && pool.isNotEmpty) return pool.removeLast();
      if (_liveBudget < _maxWorkers) {
        _liveBudget++;
        // A failed boot returns its own slot (see _bootWorker).
        return await _bootWorker();
      }
      final waiter = Completer<void>();
      _waiters.add(waiter);
      await waiter.future;
      if (isKilled()) {
        _wakeWaiter();
        return null;
      }
    }
  }

  /// A killed lane's worker that never received work: still pristine
  /// (init done, no engine state, no files — work is a precondition
  /// for writing), safe for any future lane. Pool-overflow terminate
  /// therefore needs no directory reclaim.
  void returnPristine(LaneWorker worker) {
    final pool = _pool[_spareKey] ??= [];
    if (pool.length >= _maxPooled) {
      _terminateAndReleaseSlot(worker);
    } else {
      pool.add(worker);
      _wakeWaiter();
    }
  }

  /// Terminates a USED worker; its slot and OPFS directory return
  /// once the liveness lock confirms the agent is gone.
  void retire(LaneWorker worker) => _terminateAndReleaseSlot(worker);

  static void _wakeWaiter() {
    if (_waiters.isNotEmpty) _waiters.removeAt(0).complete();
  }

  /// Boot one pristine worker into the pool if none exists or is on
  /// the way. Constructor-time only — keeps the first op on a fresh
  /// page off the cold-boot path.
  void _prewarm() {
    if ((_pool[_spareKey]?.isNotEmpty ?? false) ||
        _warming.contains(_spareKey) ||
        _liveBudget >= _maxWorkers) {
      return;
    }
    _liveBudget++;
    _warming.add(_spareKey);
    unawaited(
      _bootWorker()
          .then((worker) {
            _warming.remove(_spareKey);
            (_pool[_spareKey] ??= []).add(worker);
            _wakeWaiter();
          })
          .catchError((Object _) {
            // A failed prewarm is silent by design — the first real take
            // boots inline and surfaces the error on the op that needs it.
            // The slot came back through _bootWorker's own failure path.
            _warming.remove(_spareKey);
          }),
    );
  }

  /// Spawn a worker and drive its boot handshake to `ready`. The
  /// temporary listener is detached before the worker is handed out;
  /// an idle ready worker sends nothing, so no message can be lost.
  Future<LaneWorker> _bootWorker() async {
    // Every worker holds a liveness lock for its lifetime, in every
    // mode: it is the budget's death signal. Only OPFS mode also gives
    // the worker a directory under the same name.
    final workerId = _newSessionId();
    final lockName = _workerLockName(workerId);
    final opfsDir = mode == PdfIoMode.opfs ? workerId : null;
    final web.Worker worker;
    try {
      worker = _spawnWorker();
    } catch (_) {
      _liveBudget--;
      _wakeWaiter();
      rethrow;
    }
    // Everything after the spawn runs inside the slot-owning try: a throw
    // anywhere here must still terminate the worker and return the slot.
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
      return LaneWorker(worker, opfsDir, lockName);
    } catch (_) {
      // A worker that never readied wrote no files (writing requires
      // work, work requires ready) — no directory to reclaim, but it
      // may already hold its lock with a WASM instance behind it, so
      // its slot returns the same way every other worker's does.
      _terminateAndReleaseSlot(LaneWorker(worker, null, lockName));
      rethrow;
    } finally {
      unawaited(sub?.cancel() ?? Future<void>.value());
      unawaited(errSub?.cancel() ?? Future<void>.value());
    }
  }

  @override
  Lane spawn() => WebLane._(this);

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

// ── A booted worker plus its OPFS identity ─────────────────────────

/// A ready worker as handed out by the host. [opfsDir] is the
/// worker's own directory name under the session (OPFS mode only) —
/// the key the host reclaims by after the worker is retired.
class LaneWorker {
  /// Wraps a [js] worker with its OPFS directory name (null outside
  /// OPFS mode) and the name of the liveness lock it holds.
  LaneWorker(this.js, this.opfsDir, this.lockName);

  /// The underlying Web Worker.
  final web.Worker js;

  /// The worker's own directory name under the session, or null when
  /// the mode touches no disk.
  final String? opfsDir;

  /// The Web Lock the worker holds from before `ready` until the
  /// browser tears it down — the host's death signal.
  final String lockName;
}
