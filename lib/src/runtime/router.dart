// Router — the ONE shared brain for both platforms.
//
// Implements PdfTransport on top of the Lane interface. Everything
// that decides lives here, exactly once:
//
//   - pinning: a handle's ops always run on the lane that owns it
//   - placement: handle-less ops go to the least-loaded lane,
//     spawning lanes up to [maxLanes]
//   - job identity + completion ownership (after a lane kill, the
//     LANE completes its pending submits as cancelled — the Router
//     never awaits a dead lane)
//   - held-resource lifecycle (keepSources → resourceIds → release)
//   - dispose: instant — kill every lane, clear every map, return
//
// The Router runs entirely on the main isolate's event loop: its
// state has exactly one owner thread, so there is nothing to lock
// and no check-then-act race is possible — single owner.
//
// INTERNAL — constructed by the per-platform create functions.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/io/memory_source.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/bridge/pdf_transport.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Ops that destroy a handle — their success removes the pin.
const _disposeOps = {'docDispose', 'editorDispose', 'builderDispose'};

/// The only ops whose success creates a handle, each with the op that
/// destroys what it made.
const _handleCreators = {
  'open': 'docDispose',
  'editorOpen': 'editorDispose',
  'builderCreate': 'builderDispose',
};

/// Ops that name an editor handle. An op in neither this set nor
/// [_builderOps] names a document.
const _editorOps = {
  'editorDispose',
  'editorMutate',
  'editorSave',
  'editorGetMetadata',
  'editorIsModified',
  'editorPageMediaBox',
  'editorPageCropBox',
  'editorPageImages',
  'editorRedactionCount',
  'editorMergeFrom',
  'editorExtractPages',
};

/// Ops that name a builder handle.
const _builderOps = {
  'builderDispose',
  'builderSetMetadata',
  'builderAddPage',
  'builderAddImagePage',
  'builderPageOp',
  'builderPageDone',
  'builderSave',
};

/// A web worker whose WASM memory peaked at or above this is retired
/// once it is idle: linear memory never shrinks, so one huge job would
/// otherwise leave the worker large forever.
const kDefaultWorkerRecycleBytes = 256 * 1024 * 1024;

class _LaneSlot {
  _LaneSlot(this.lane);

  final Lane lane;

  /// Jobs currently submitted and not yet completed. Drives
  /// least-loaded placement.
  int activeJobs = 0;

  /// True once the lane's engine trapped or panicked: the lane serves
  /// nothing more, and every handle pinned to it is gone.
  bool dead = false;
}

class _Pin {
  _Pin(this.slot, this.engineHandleId, this.groupId);

  final _LaneSlot slot;

  /// The id the engine on [slot] knows this handle by. Per-lane engine
  /// ids collide across lanes (each lane counts from 1), so the Router
  /// keys its pin map by a global id and translates back to this on the
  /// way to the lane.
  final int engineHandleId;

  /// Id of the job that created the handle. Every source the handle
  /// keeps, from that job or a later one, belongs to this group; it is
  /// passed with every op on the handle so web lanes can mount the
  /// group's held readers.
  final int groupId;

  /// The one resource that covers every channel this handle keeps; null
  /// until the first job that keeps a source succeeds.
  int? resourceId;
}

class _HeldResource {
  _HeldResource(this.slot, this.tokens);

  final _LaneSlot slot;
  final List<Object> tokens;
}

/// The shared routing brain. One per Pdf() instance.
class Router implements PdfTransport {
  /// Creates a router over [host], capping concurrent lanes at
  /// [maxLanes] per instance (a process-wide budget lives below the
  /// host — hitting either cap queues, never errors).
  Router({
    required LaneHost host,
    required int maxLanes,
    required PdfIoMode mode,
    int workerRecycleBytes = kDefaultWorkerRecycleBytes,
  }) : assert(maxLanes >= 1, 'maxLanes must be at least 1'),
       _host = host,
       _maxLanes = maxLanes,
       _mode = mode,
       _workerRecycleBytes = workerRecycleBytes;

  final LaneHost _host;
  final int _maxLanes;
  final int _workerRecycleBytes;
  final PdfIoMode _mode;

  final List<_LaneSlot> _lanes = [];
  final List<({Uint8List request, Uint8List bytes})> _preludes = [];
  final Map<int, _Pin> _pins = {};
  final Map<int, _HeldResource> _held = {};
  int _nextJobId = 1;
  int _nextHandleId = 1;
  int _nextResourceId = 1;
  bool _disposed = false;
  bool _initialized = false;

  @override
  PdfIoMode? get ioMode => _initialized ? _mode : null;

  @override
  Future<PdfIoMode> ensureInitialized() async {
    if (_disposed) throw const PdfDisposed('Router disposed');
    _initialized = true;
    return _mode;
  }

  @override
  Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
    Uint8List request, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
    Set<int> keepSources = const {},
    CancelHook? cancel,
  }) async {
    if (_disposed || (cancel?.isCancelled ?? false)) {
      // Any op reaching a disposed instance resolves with the typed
      // cancelled error — same outcome as a job killed mid-flight.
      return (bytes: buildCancelledResponse(), resourceIds: const <int, int>{});
    }
    await ensureInitialized();

    // Same-tick cancels AND disposes land during the await above —
    // before the job exists anywhere. Without the disposed re-check,
    // execution resumes on a disposed Router and spawns a ZOMBIE
    // lane: never killed (dispose already ran), holding its sources
    // forever. From here to submit is synchronous (single owner, one
    // event loop), so these checks are exact, not races.
    if (_disposed || (cancel?.isCancelled ?? false)) {
      return (bytes: buildCancelledResponse(), resourceIds: const <int, int>{});
    }

    // The caller holds a Router-global handle id; the engine on the
    // pinned lane knows only its own per-lane id (those collide across
    // lanes). Translate global → engine before the request leaves so
    // the lane looks the handle up correctly.
    var req = request;
    final globalHandleId = peekRequestHandleId(req);
    final pin = globalHandleId != null ? _pins[globalHandleId] : null;
    if (globalHandleId != null) {
      if (pin == null) {
        // No pin: the handle was disposed (global ids are never reused).
        // Forwarded, the raw id could name another live document on
        // whichever lane placement picks, so the Router answers itself.
        return (
          bytes: buildErrorResponse(_notFoundMessage(peekRequestOp(req))),
          resourceIds: const <int, int>{},
        );
      }
      if (pin.slot.dead) {
        // The handle died with its lane; disposing it has nothing to free.
        final disposing = _disposeOps.contains(peekRequestOp(req));
        if (disposing) _pins.remove(globalHandleId);
        return (
          bytes: disposing
              ? Uint8List.fromList(const [1, 0, 0])
              : buildErrorResponse(laneDeadMessage),
          resourceIds: const <int, int>{},
        );
      }
      req = rewriteRequestHandleId(req, pin.engineHandleId);
    }
    final slot = pin?.slot ?? _placeFor(req);
    final job = LaneJob(
      jobId: _nextJobId++,
      request: req,
      sources: sources,
      sinks: sinks,
      keepSources: keepSources,
      heldGroupId: pin?.groupId,
    );

    slot.activeJobs++;
    // submit() registers the job synchronously before returning its
    // future — registering AFTER that line means a fired cancel always
    // finds its job. A finished job's cancel is a lane-level no-op,
    // but unregistering keeps it exact.
    final pending = slot.lane.submit(job);
    final unregister = cancel?.register(() => slot.lane.cancelJob(job.jobId));
    final LaneSubmitResult result;
    try {
      result = await pending;
    } finally {
      slot.activeJobs--;
      unregister?.call();
    }
    if (result.laneDied && !_disposed) _retireDead(slot);
    _killIfDrained(slot);

    // Pin bookkeeping is response-driven, not op-name-driven: any op
    // whose success carries a handleId created a handle on this lane.
    // Mint a Router-global id for it (engine ids collide across lanes),
    // pin it to the lane + engine id, and hand the caller the global id.
    var bytes = result.bytes;

    // Only a success promotes kept channels. A failed (or cancelled)
    // job leaves its channels with the lane, which holds them until
    // released: free them now, nothing will ever name them again.
    // A router disposed meanwhile already killed every lane.
    final succeeded = bytes.isNotEmpty && bytes[0] == 1;
    if (!succeeded) {
      if (!_disposed) {
        for (final token in result.heldTokens.values) {
          slot.lane.releaseHeld(token);
        }
        _maybeRecycle(slot, result);
      }
      return (bytes: bytes, resourceIds: const <int, int>{});
    }

    final engineHandle = peekResponseHandleId(bytes);
    if (engineHandle != null && (cancel?.isCancelled ?? false)) {
      // The caller's cancel fired in the window between the engine
      // finishing and this result arriving. For an op that CREATES a
      // handle the cancel wins: the caller is told cancelled, so the
      // handle has no id anyone holds and is disposed on its own lane.
      // Every other success is returned as it is, so the caller's view
      // matches the engine's: a finished edit stays on its live editor.
      final disposeOp = _handleCreators[peekRequestOp(req)];
      if (disposeOp == null) {
        // Only a creator's success carries a new handle. Anything else is
        // a bug in the engine or in this map: loud in debug, and in
        // release the result is returned and the handle pinned, since
        // disposing a handle the Router cannot name the owner of is worse.
        var debug = false;
        assert(debug = true);
        if (debug) {
          throw StateError(
            'op "${peekRequestOp(req)}" returned a handle but is not a '
            'handle creator',
          );
        }
      } else {
        if (!_disposed) {
          _disposeOrphan(slot, engineHandle, disposeOp, result);
          _maybeRecycle(slot, result);
        }
        return (
          bytes: buildCancelledResponse(),
          resourceIds: const <int, int>{},
        );
      }
    }

    var owner = pin;
    if (engineHandle != null) {
      final globalId = _nextHandleId++;
      owner = _Pin(slot, engineHandle, job.jobId);
      _pins[globalId] = owner;
      bytes = rewriteResponseHandleId(bytes, globalId);
    }
    if (owner != null && result.heldTokens.isNotEmpty) {
      // A later job on a live handle that keeps sources (a merge) adds
      // them to the handle's one resource, released with it.
      final resource = owner.resourceId == null
          ? null
          : _held[owner.resourceId];
      if (resource == null) {
        owner.resourceId = _newResource(slot, result.heldTokens.values);
      } else {
        resource.tokens.addAll(result.heldTokens.values);
      }
    }
    if (globalHandleId != null && _disposeOps.contains(peekRequestOp(req))) {
      _pins.remove(globalHandleId);
    }

    final resourceId =
        owner?.resourceId ??
        (result.heldTokens.isEmpty
            ? null
            : _newResource(slot, result.heldTokens.values));
    final resourceIds = <int, int>{
      if (resourceId != null)
        for (final sourceIndex in result.heldTokens.keys)
          sourceIndex: resourceId,
    };
    _maybeRecycle(slot, result);
    return (bytes: bytes, resourceIds: resourceIds);
  }

  /// Disposes a handle nobody holds. It rides the lane's own queue, so it
  /// runs after the job that made it, and the job's kept channels are
  /// released only once the engine has dropped the document that reads them.
  void _disposeOrphan(
    _LaneSlot slot,
    int engineHandle,
    String disposeOp,
    LaneSubmitResult created,
  ) {
    final job = LaneJob(
      jobId: _nextJobId++,
      request: bin.encodeRequest(disposeOp, {'handleId': engineHandle}),
      sources: const [],
      sinks: const [],
      keepSources: const {},
    );
    slot.activeJobs++;
    unawaited(
      slot.lane.submit(job).whenComplete(() {
        slot.activeJobs--;
        if (_disposed) return;
        for (final token in created.heldTokens.values) {
          slot.lane.releaseHeld(token);
        }
        _killIfDrained(slot);
      }),
    );
  }

  /// A dead lane takes no more placements and fails the handles on it. It
  /// is killed once the jobs already on it have each had their answer.
  void _retireDead(_LaneSlot slot) {
    slot.dead = true;
    _lanes.remove(slot);
  }

  void _killIfDrained(_LaneSlot slot) {
    if (slot.dead && slot.activeJobs == 0) slot.lane.kill();
  }

  /// The engine's own not-found error for the kind of handle [op] names.
  String _notFoundMessage(String? op) {
    if (_editorOps.contains(op)) return 'editor not found';
    if (_builderOps.contains(op)) return 'builder not found';
    return 'document not found';
  }

  /// Retires an idle lane whose worker grew past the recycle threshold.
  /// Native lanes never recycle: their results carry no [LaneSubmitResult.peakBytes].
  /// A pin or held resource on the slot keeps it alive; the next op
  /// spawns a replacement lane through [_placeFor].
  void _maybeRecycle(_LaneSlot slot, LaneSubmitResult result) {
    final peak = result.peakBytes;
    if (_disposed || peak == null || peak < _workerRecycleBytes) return;
    if (slot.activeJobs > 0) return;
    if (_pins.values.any((p) => p.slot == slot)) return;
    if (_held.values.any((r) => r.slot == slot)) return;
    if (!_lanes.remove(slot)) return;
    slot.lane.kill();
  }

  int _newResource(_LaneSlot slot, Iterable<Object> tokens) {
    final id = _nextResourceId++;
    _held[id] = _HeldResource(slot, List.of(tokens));
    return id;
  }

  @override
  Future<void> releaseSource(int resourceId) async {
    final resource = _held.remove(resourceId);
    if (resource == null) return; // already gone (dispose covered it)
    for (final token in resource.tokens) {
      resource.slot.lane.releaseHeld(token);
    }
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;

    // Instant: each kill flips the lane's flag, wakes its parked
    // I/O, completes its pending submits as cancelled, and frees its
    // platform resources. No joins, no awaits, no timeouts.
    for (final slot in _lanes) {
      slot.lane.kill();
    }
    _lanes.clear();
    _pins.clear();
    _held.clear();
  }

  @override
  Future<List<Uint8List>> installPrelude(
    Uint8List request, {
    required Uint8List sourceBytes,
  }) async {
    if (_disposed) return const [];
    await ensureInitialized();
    _preludes.add((request: request, bytes: sourceBytes));
    final results = <Uint8List>[];
    for (final slot in List.of(_lanes)) {
      results.add(await _runPrelude(slot, request, sourceBytes));
    }
    return results;
  }

  /// Preludes ride the same per-lane FIFO as user jobs, so one queued at
  /// spawn time always executes before any job routed to that lane later.
  Future<Uint8List> _runPrelude(
    _LaneSlot slot,
    Uint8List request,
    Uint8List bytes,
  ) async {
    final job = LaneJob(
      jobId: _nextJobId++,
      request: request,
      sources: [MemorySource(bytes)],
      sinks: const [],
      keepSources: const {},
    );
    slot.activeJobs++;
    try {
      return (await slot.lane.submit(job)).bytes;
    } finally {
      slot.activeJobs--;
    }
  }

  /// Least-loaded lane, spawning under the cap. (Pinned routing is
  /// resolved by the caller; an unknown handle falls through here —
  /// any lane can host the engine's "not found" error.)
  _LaneSlot _placeFor(Uint8List request) {
    if (_lanes.isEmpty || (_lanes.length < _maxLanes && _allBusy())) {
      final slot = _LaneSlot(_host.spawn());
      _lanes.add(slot);
      for (final p in _preludes) {
        // Fire-and-forget on fresh lanes: a failed replay degrades that
        // lane to the engine's no-fallback warning path, never fails jobs.
        unawaited(_runPrelude(slot, p.request, p.bytes));
      }
      return slot;
    }

    var best = _lanes.first;
    for (final slot in _lanes.skip(1)) {
      if (slot.activeJobs < best.activeJobs) best = slot;
    }
    return best;
  }

  bool _allBusy() => _lanes.every((s) => s.activeJobs > 0);
}
