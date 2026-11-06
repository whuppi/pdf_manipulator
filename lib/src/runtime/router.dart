// Router — the one place that decides, for both platforms.
//
// Implements PdfTransport on top of the Lease interface. Everything
// that decides lives here, exactly once:
//
//   - pinning: a handle's ops always run on the lane that owns it
//   - placement: handle-less ops go to the least-loaded lane,
//     starting lanes up to [maxLanes]
//   - job identity + completion ownership (after a lane ends, the
//     LANE completes its pending submits as cancelled — the Router
//     never awaits a dead lane)
//   - held-resource lifecycle (keepSources → resourceIds → release)
//   - dispose: instant — end every lane (its executor goes back to the
//     pool), clear every map, return
//   - the lease: a lane takes an executor from the pool at its first job
//     and gives it back once it is idle and holds no pin and no kept source
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
import 'package:pdf_manipulator/src/runtime/pool.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/bridge/pdf_transport.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
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
  'editorPageInfo',
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

class _LaneRecord {
  _LaneRecord(this.lane);

  final Lane lane;

  /// Jobs currently submitted and not yet completed. Drives
  /// least-loaded placement.
  int activeJobs = 0;

  /// Handles pinned to this lane, and held resources it keeps: either one
  /// keeps the lane's executor.
  int pins = 0;
  int heldResources = 0;

  /// True once the lane's engine trapped or panicked: the lane serves
  /// nothing more, and every handle pinned to it is gone.
  bool dead = false;
}

class _Pin {
  _Pin(this.slot, this.engineHandleId, this.groupId);

  final _LaneRecord slot;

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

  final _LaneRecord slot;
  final List<Object> tokens;
}

/// The shared routing brain. One per Pdf() instance.
class Router implements PdfTransport {
  /// Creates a router over [pool], capping concurrent lanes at
  /// [maxLanes] per instance (the pool's budget lives below — hitting
  /// either cap queues, never errors).
  Router({
    required ExecutorPool pool,
    required int maxLanes,
    required PdfIoMode mode,
  }) : assert(maxLanes >= 1, 'maxLanes must be at least 1'),
       _pool = pool,
       _maxLanes = maxLanes,
       _mode = mode;

  final ExecutorPool _pool;
  final int _maxLanes;
  final PdfIoMode _mode;

  final List<_LaneRecord> _lanes = [];

  /// Dead lanes that still hold their executor until their jobs answer.
  final Set<_LaneRecord> _dying = {};
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
    if (_disposed) throw StateError('Router disposed');
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
      // cancelled error — same outcome as a job cancelled mid-flight.
      return (bytes: buildCancelledResponse(), resourceIds: const <int, int>{});
    }
    await ensureInitialized();

    // Same-tick cancels AND disposes land during the await above —
    // before the job exists anywhere. Without the disposed re-check,
    // execution resumes on a disposed Router and starts a lane that
    // nothing will end (dispose already ran), holding its sources
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
        if (disposing) _unpin(globalHandleId);
        return (
          bytes: disposing
              ? Uint8List.fromList(const [1, 0, 0])
              : buildErrorResponse(engineDeadMessage),
          resourceIds: const <int, int>{},
        );
      }
      req = rewriteRequestHandleId(req, pin.engineHandleId);
    }
    final slot = pin?.slot ?? _placeFor(req);
    final job = Job(
      jobId: _nextJobId++,
      request: req,
      sources: sources,
      sinks: sinks,
      keepSources: keepSources,
      holdGroupId: pin?.groupId,
    );

    slot.activeJobs++;
    // submit() registers the job synchronously before returning its
    // future — registering AFTER that line means a fired cancel always
    // finds its job. A finished job's cancel is a lane-level no-op,
    // but unregistering keeps it exact.
    final pending = slot.lane.submit(job);
    final unregister = cancel?.register(() => slot.lane.cancelJob(job.jobId));
    final JobResult result;
    try {
      result = await pending;
    } finally {
      slot.activeJobs--;
      unregister?.call();
    }
    try {
      if (result.executorDied && !_disposed) _retireDead(slot);
      return _complete(slot, job, req, pin, globalHandleId, result, cancel);
    } finally {
      // After the bookkeeping: a pin or kept source it adds keeps the lease.
      _releaseIfIdle(slot);
    }
  }

  /// Turns one finished job into the caller's answer: pins a handle the
  /// job created, files the sources it kept, and applies the cancel rule.
  ({Uint8List bytes, Map<int, int> resourceIds}) _complete(
    _LaneRecord slot,
    Job job,
    Uint8List req,
    _Pin? pin,
    int? globalHandleId,
    JobResult result,
    CancelHook? cancel,
  ) {
    // Pin bookkeeping is response-driven, not op-name-driven: any op
    // whose success carries a handleId created a handle on this lane.
    // Mint a Router-global id for it (engine ids collide across lanes),
    // pin it to the lane + engine id, and hand the caller the global id.
    var bytes = result.bytes;

    // Only a success promotes kept channels. A failed (or cancelled)
    // job leaves its channels with the lane, which holds them until
    // released: free them now, nothing will ever name them again.
    // A router disposed meanwhile already ended every lane.
    final succeeded = bytes.isNotEmpty && bytes[0] == 1;
    if (!succeeded) {
      if (!_disposed) {
        for (final token in result.holds.values) {
          slot.lane.releaseHold(token);
        }
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
      slot.pins++;
      bytes = rewriteResponseHandleId(bytes, globalId);
    }
    if (owner != null && result.holds.isNotEmpty) {
      // A later job on a live handle that keeps sources (a merge) adds
      // them to the handle's one resource, released with it.
      final resource = owner.resourceId == null
          ? null
          : _held[owner.resourceId];
      if (resource == null) {
        owner.resourceId = _newResource(slot, result.holds.values);
      } else {
        resource.tokens.addAll(result.holds.values);
      }
    }
    if (globalHandleId != null && _disposeOps.contains(peekRequestOp(req))) {
      _unpin(globalHandleId);
    }

    final resourceId =
        owner?.resourceId ??
        (result.holds.isEmpty ? null : _newResource(slot, result.holds.values));
    final resourceIds = <int, int>{
      if (resourceId != null)
        for (final sourceIndex in result.holds.keys) sourceIndex: resourceId,
    };
    return (bytes: bytes, resourceIds: resourceIds);
  }

  /// Disposes a handle nobody holds. It rides the lane's own queue, so it
  /// runs after the job that made it, and the job's kept channels are
  /// released only once the engine has dropped the document that reads them.
  void _disposeOrphan(
    _LaneRecord slot,
    int engineHandle,
    String disposeOp,
    JobResult created,
  ) {
    final job = Job(
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
        for (final token in created.holds.values) {
          slot.lane.releaseHold(token);
        }
        _releaseIfIdle(slot);
      }),
    );
  }

  /// A dead lane takes no more placements and fails the handles on it. Its
  /// executor is given up once the jobs already on it have each had their
  /// answer.
  void _retireDead(_LaneRecord slot) {
    slot.dead = true;
    _lanes.remove(slot);
    _dying.add(slot);
  }

  /// Gives the lane's executor back to the pool when the lane is idle and
  /// holds nothing: no job, no pinned handle, no kept source. A dead lane
  /// gives it up whatever it holds, since its handles are gone.
  void _releaseIfIdle(_LaneRecord slot) {
    if (_disposed || slot.activeJobs > 0) return;
    if (!slot.dead && (slot.pins > 0 || slot.heldResources > 0)) return;
    _dying.remove(slot);
    slot.lane.release();
  }

  void _unpin(int globalHandleId) {
    final pin = _pins.remove(globalHandleId);
    if (pin != null) pin.slot.pins--;
  }

  /// The engine's own not-found error for the kind of handle [op] names.
  String _notFoundMessage(String? op) {
    if (_editorOps.contains(op)) return 'editor not found';
    if (_builderOps.contains(op)) return 'builder not found';
    return 'document not found';
  }

  int _newResource(_LaneRecord slot, Iterable<Object> tokens) {
    final id = _nextResourceId++;
    _held[id] = _HeldResource(slot, List.of(tokens));
    slot.heldResources++;
    return id;
  }

  @override
  Future<void> releaseSource(int resourceId) async {
    final resource = _held.remove(resourceId);
    if (resource == null) return; // already gone (dispose covered it)
    for (final token in resource.tokens) {
      resource.slot.lane.releaseHold(token);
    }
    resource.slot.heldResources--;
    _releaseIfIdle(resource.slot);
  }

  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;

    // Instant: each lane completes its pending submits as cancelled,
    // cancels its running job and wakes its parked I/O; the pool takes the
    // executor back once the job has posted. No joins, no awaits, no
    // timeouts.
    for (final slot in [..._lanes, ..._dying]) {
      slot.lane.end();
    }
    _lanes.clear();
    _dying.clear();
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
    // A lane without an executor replays every prelude when it leases one.
    final results = <Uint8List>[];
    for (final slot in List.of(_lanes)) {
      if (slot.lane.leased) {
        results.add(await _runPrelude(slot, request, sourceBytes));
      }
    }
    return results;
  }

  /// Preludes ride the same per-lane FIFO as user jobs, so the ones replayed
  /// at lease time always execute before any job routed to that lane later.
  Future<Uint8List> _runPrelude(
    _LaneRecord slot,
    Uint8List request,
    Uint8List bytes,
  ) async {
    final job = _preludeJob(request, bytes);
    slot.activeJobs++;
    try {
      return (await slot.lane.submit(job)).bytes;
    } finally {
      slot.activeJobs--;
    }
  }

  Job _preludeJob(Uint8List request, Uint8List bytes) => Job(
    jobId: _nextJobId++,
    request: request,
    sources: [MemorySource(bytes)],
    sinks: const [],
    keepSources: const {},
  );

  /// Least-loaded lane, starting a new one under the cap. (Pinned routing is
  /// resolved by the caller; an unknown handle falls through here —
  /// any lane can host the engine's "not found" error.)
  _LaneRecord _placeFor(Uint8List request) {
    if (_lanes.isEmpty || (_lanes.length < _maxLanes && _allBusy())) {
      // Fire-and-forget replay at every lease: a failed one degrades that
      // lease to the engine's no-fallback warning path, never fails jobs.
      final slot = _LaneRecord(
        Lane(
          _pool,
          prelude: () => [
            for (final p in _preludes) _preludeJob(p.request, p.bytes),
          ],
        ),
      );
      _lanes.add(slot);
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
