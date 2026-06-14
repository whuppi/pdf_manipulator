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

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/cancel_hook.dart';
import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/bridge/pdf_transport.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// Ops that destroy a handle — their success removes the pin.
const _disposeOps = {'docDispose', 'editorDispose', 'builderDispose'};

class _LaneSlot {
  _LaneSlot(this.lane);

  final Lane lane;

  /// Jobs currently submitted and not yet completed. Drives
  /// least-loaded placement.
  int activeJobs = 0;
}

class _Pin {
  _Pin(this.slot, this.heldToken);

  final _LaneSlot slot;

  /// Held token of the job that created the handle — passed with
  /// every op on this handle so web lanes can mount its held readers.
  final Object? heldToken;
}

class _HeldResource {
  _HeldResource(this.slot, this.token);

  final _LaneSlot slot;
  final Object token;
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
  }) : assert(maxLanes >= 1, 'maxLanes must be at least 1'),
       _host = host,
       _maxLanes = maxLanes,
       _mode = mode;

  final LaneHost _host;
  final int _maxLanes;
  final PdfIoMode _mode;

  final List<_LaneSlot> _lanes = [];
  final Map<int, _Pin> _pins = {};
  final Map<int, _HeldResource> _held = {};
  int _nextJobId = 1;
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

    final handleId = peekRequestHandleId(request);
    final pin = handleId != null ? _pins[handleId] : null;
    final slot = pin?.slot ?? _placeFor(request);
    final job = LaneJob(
      jobId: _nextJobId++,
      request: request,
      sources: sources,
      sinks: sinks,
      keepSources: keepSources,
      heldToken: pin?.heldToken,
    );

    slot.activeJobs++;
    // submit() registers the job synchronously before returning its
    // future — binding AFTER that line means a fired cancel always
    // finds its job. A finished job's cancel is a lane-level no-op,
    // but unbinding keeps it exact.
    final pending = slot.lane.submit(job);
    cancel?.bind(() => slot.lane.cancelJob(job.jobId));
    final LaneSubmitResult result;
    try {
      result = await pending;
    } finally {
      slot.activeJobs--;
      cancel?.unbind();
    }

    // Pin bookkeeping is response-driven, not op-name-driven: any op
    // whose success carries a handleId created a handle on this lane.
    final createdHandle = peekResponseHandleId(result.bytes);
    if (createdHandle != null) {
      final token = result.heldTokens.isEmpty
          ? null
          : result.heldTokens.values.first;
      _pins[createdHandle] = _Pin(slot, token);
    }
    if (_disposeOps.contains(peekRequestOp(request))) {
      final disposedHandle = peekRequestHandleId(request);
      if (disposedHandle != null) {
        _pins.remove(disposedHandle);
      }
    }

    final resourceIds = <int, int>{};
    result.heldTokens.forEach((sourceIndex, token) {
      final id = _nextResourceId++;
      _held[id] = _HeldResource(slot, token);
      resourceIds[sourceIndex] = id;
    });

    return (bytes: result.bytes, resourceIds: resourceIds);
  }

  @override
  Future<void> releaseSource(int resourceId) async {
    final resource = _held.remove(resourceId);
    if (resource == null) return; // already gone (dispose covered it)
    resource.slot.lane.releaseHeld(resource.token);
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

  /// Least-loaded lane, spawning under the cap. (Pinned routing is
  /// resolved by the caller; an unknown handle falls through here —
  /// any lane can host the engine's "not found" error.)
  _LaneSlot _placeFor(Uint8List request) {
    if (_lanes.isEmpty || (_lanes.length < _maxLanes && _allBusy())) {
      final slot = _LaneSlot(_host.spawn());
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
