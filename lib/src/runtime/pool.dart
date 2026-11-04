// ExecutorPool — the one owner of every executor, written once for both
// platforms.
//
// An executor is a thread or a worker with its engine state (lane.dart). It
// is booted once and leased, one lane at a time, over and over: a disposed
// Pdf hands its executor back, the pool resets it, and the next lane takes
// it, so churn boots almost nothing. Only a dead executor is ever
// terminated for good.
//
// ## States
//
//   booting -> idle -> leased -> draining -> idle
//                                         \-> dead
//
// - booting:  the platform is starting it; it counts against the budget.
// - idle:     reset and waiting; handed to the next request.
// - leased:   one lane holds it.
// - draining: its lane ended; the pool waits for the lane's job to post,
//             then resets it. A dead engine, a failed reset or a heap over
//             the keep line ends it as dead instead.
// - dead:     terminated and gone from the count; never reused.
//
// ## Budget
//
// The budget counts live executors: booting, idle, leased and draining. A
// terminated one leaves the count at once, so a request waiting for a slot
// is served the moment any executor is released or terminated, never only
// when a platform resource finally disappears.
//
// ## Order
//
// A request takes the idle executor with the smallest heap, else boots one
// under the budget, else waits. An executor that is booting or draining
// counts as on its way to idle, so a request waits for it rather than
// booting another: a Pdf disposed and replaced in a loop reuses one
// executor. Waiters are served
// strictly in arrival order: an executor that frees goes to the head of the
// queue, never to a request that arrived later.
//
// INTERNAL — used by the Router and the platform hosts.

import 'dart:async';

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';

/// Most executors alive at once in one pool. Instances the app never
/// disposes hold their executor, so the cap must dwarf any sane leak count
/// or idle instances starve fresh ones. Fixed, not device-derived:
/// behaviour must not vary by machine.
const kMaxExecutors = 64;

/// Most idle executors the pool keeps; a returned one beyond this is
/// terminated so parked executors cannot starve the budget.
const kMaxIdleExecutors = 4;

/// An executor whose heap reached this many bytes is terminated when it is
/// returned instead of reset: linear memory never shrinks, so one huge job
/// would otherwise leave the executor large forever.
const kDefaultWorkerRecycleBytes = 256 * 1024 * 1024;

/// Where an executor is in its life. See the library comment.
enum ExecutorState {
  /// The platform is starting it.
  booting,

  /// Reset and waiting for the next lease.
  idle,

  /// One lane holds it.
  leased,

  /// Its lane ended; a running job may still unwind, then it is reset.
  draining,

  /// Terminated and out of the count; never reused.
  dead,
}

class _Entry {
  _Entry();

  ExecutorState state = ExecutorState.booting;

  /// Null until the boot completes.
  Executor? executor;

  /// The heap size after the last reset; ranks idle executors.
  int heap = 0;
}

class _Waiter {
  final completer = Completer<ExecutorLease>();
}

/// One lane's hold on one executor: the lane that owns it and nobody else
/// until the pool takes it back through [ExecutorPool.release].
class ExecutorLease {
  ExecutorLease._(this._entry, this.lane);

  final _Entry _entry;

  /// The lane adapter bound to this lease's id.
  final Lane lane;

  /// Where the leased executor is. Tests read it.
  ExecutorState get state => _entry.state;
}

/// A request for an executor that may still be waiting.
class ExecutorRequest {
  ExecutorRequest._(this._pool, this._waiter);

  final ExecutorPool _pool;
  final _Waiter _waiter;

  /// Completes with the lease once an executor is handed over, or with the
  /// error of the boot that was started for it.
  Future<ExecutorLease> get lease => _waiter.completer.future;

  /// Leaves the queue when the request has not been served yet; does
  /// nothing once an executor was handed over (the new owner returns it).
  void cancel() => _pool._cancel(_waiter);
}

/// Owns every executor of one platform configuration.
class ExecutorPool {
  /// Creates a pool over [host]'s executors. [maxExecutors] is the budget,
  /// [maxIdle] how many idle executors are kept, [keepBytes] the heap size
  /// at which a returned executor is terminated instead of reset.
  ExecutorPool({
    required LaneHost host,
    this.maxExecutors = kMaxExecutors,
    this.maxIdle = kMaxIdleExecutors,
    this.keepBytes = kDefaultWorkerRecycleBytes,
  }) : assert(maxExecutors >= 1, 'maxExecutors must be at least 1'),
       _host = host;

  static final Map<String, ExecutorPool> _shared = {};
  static int _totalBoots = 0;

  /// The pool every instance of [host]'s configuration shares, page- or
  /// process-wide. The first use of the package creates it and boots one
  /// executor ahead of the first job, once.
  static ExecutorPool of(LaneHost host) {
    final existing = _shared[host.poolKey];
    if (existing != null) return existing;
    return _shared[host.poolKey] = ExecutorPool(host: host)..prewarm();
  }

  /// Executors booted by every pool since the process started. Tests count
  /// churn with it; nothing decides on it.
  static int get totalBoots => _totalBoots;

  final LaneHost _host;

  /// The budget: live executors at most.
  final int maxExecutors;

  /// Idle executors kept at most.
  final int maxIdle;

  /// The heap size at which a returned executor is terminated.
  final int keepBytes;

  final List<_Entry> _entries = [];
  final List<_Waiter> _waiters = [];
  bool _prewarmed = false;
  bool _disposed = false;
  int _boots = 0;

  /// Executors this pool booted. Tests only.
  int get bootCount => _boots;

  /// Live executors: booting, idle, leased and draining.
  int get liveCount => _entries.length;

  /// Executors waiting for a lease.
  int get idleCount => _count(ExecutorState.idle);

  /// Requests waiting for an executor.
  int get waitingCount => _waiters.length;

  int _count(ExecutorState state) =>
      _entries.where((e) => e.state == state).length;

  /// Boots one executor into the pool, once per pool, so the first lease
  /// does not wait for a cold boot. A failed prewarm is silent: the first
  /// real request boots again and reports the error on its own op.
  void prewarm() {
    if (_prewarmed || _disposed) return;
    _prewarmed = true;
    if (_entries.length >= maxExecutors) return;
    _boot();
  }

  /// Asks for an executor. Served at once when one is idle and nobody is
  /// waiting ahead, else in arrival order.
  ExecutorRequest acquire() {
    final waiter = _Waiter();
    if (_disposed) {
      waiter.completer.completeError(StateError('the pool was disposed'));
      waiter.completer.future.ignore();
      return ExecutorRequest._(this, waiter);
    }
    _waiters.add(waiter);
    _serve();
    return ExecutorRequest._(this, waiter);
  }

  void _cancel(_Waiter waiter) => _waiters.remove(waiter);

  /// Takes [lease]'s executor back. Its lane is ended (jobs cancelled), the
  /// pool waits for a running job to post, then resets the executor and
  /// makes it idle. [untouched] says the lane never ran a job, so the
  /// engine holds nothing and the reset is skipped. A dead engine, a failed
  /// reset, a heap at the keep line or a full idle shelf ends the executor.
  /// Idempotent.
  Future<void> release(ExecutorLease lease, {bool untouched = false}) async {
    final entry = lease._entry;
    if (entry.state != ExecutorState.leased) return;
    entry.state = ExecutorState.draining;
    final executor = entry.executor!;
    var intact = false;
    try {
      final draining = lease.lane.drain();
      intact = await draining;
      if (intact && !untouched) await lease.lane.reset();
    } catch (_) {
      intact = false;
    }
    if (_disposed || !intact || executor.heapBytes >= keepBytes) {
      _retire(entry);
      return;
    }
    entry.heap = executor.heapBytes;
    entry.state = ExecutorState.idle;
    _serve();
    if (entry.state == ExecutorState.idle && idleCount > maxIdle) {
      _retire(entry);
    }
  }

  /// Terminates every executor and refuses later requests. A job still
  /// running on a leased executor ends cancelled with it.
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    for (final entry in List.of(_entries)) {
      if (entry.state != ExecutorState.booting) _retire(entry);
    }
    for (final waiter in List.of(_waiters)) {
      waiter.completer.completeError(StateError('the pool was disposed'));
      waiter.completer.future.ignore();
    }
    _waiters.clear();
  }

  void _serve() {
    while (_waiters.isNotEmpty) {
      final lease = _leaseIdle();
      if (lease == null) break;
      _waiters.removeAt(0).completer.complete(lease);
    }
    // A booting or draining executor will be idle soon, so it covers a
    // waiter: churn waits for the executor its predecessor is returning
    // instead of booting a second one.
    var uncovered =
        _waiters.length -
        _count(ExecutorState.booting) -
        _count(ExecutorState.draining);
    while (uncovered > 0 && _entries.length < maxExecutors) {
      _boot();
      uncovered--;
    }
  }

  /// The idle executor with the smallest heap, leased; an executor that
  /// refuses its lease is dead and the next one is tried.
  ExecutorLease? _leaseIdle() {
    while (true) {
      _Entry? best;
      for (final entry in _entries) {
        if (entry.state != ExecutorState.idle) continue;
        if (best == null || entry.heap < best.heap) best = entry;
      }
      if (best == null) return null;
      try {
        final lane = best.executor!.beginLane();
        best.state = ExecutorState.leased;
        return ExecutorLease._(best, lane);
      } on LaneProtocolError {
        _retire(best);
      }
    }
  }

  void _boot() {
    final entry = _Entry();
    _entries.add(entry);
    _boots++;
    _totalBoots++;
    unawaited(
      _host.boot().then(
        (executor) {
          entry.executor = executor;
          if (_disposed) {
            _retire(entry);
            return;
          }
          entry.heap = executor.heapBytes;
          entry.state = ExecutorState.idle;
          _serve();
        },
        onError: (Object error, StackTrace stack) {
          _entries.remove(entry);
          entry.state = ExecutorState.dead;
          // The oldest waiter learns of the failure; the others boot again.
          if (_waiters.isNotEmpty) {
            final waiter = _waiters.removeAt(0);
            waiter.completer.completeError(error, stack);
            waiter.completer.future.ignore();
          }
          _serve();
        },
      ),
    );
  }

  void _retire(_Entry entry) {
    if (entry.state == ExecutorState.dead) return;
    entry.state = ExecutorState.dead;
    _entries.remove(entry);
    entry.executor?.terminate();
    _serve();
  }
}

/// A waiting job of a [PooledLane].
class _Waiting {
  _Waiting(this.job);

  final LaneJob job;
  final completer = Completer<LaneSubmitResult>();
}

/// One Pdf's queue of work on the pool: it leases an executor at its first
/// job and gives it back when the Router says it holds nothing more or the
/// Pdf ends. Implements [Lane] over whatever lease it holds now, so the
/// Router treats it as one lane for the Pdf's whole life. It offers the
/// verbs of [Lane] that make sense across leases.
class PooledLane {
  /// Creates a lane over a pool. [prelude] builds the jobs that run first
  /// under every new lease (fonts registered by the app): the engine state
  /// of a leased executor starts empty.
  PooledLane(this._pool, {required List<LaneJob> Function() prelude})
    : _prelude = prelude;

  final ExecutorPool _pool;
  final List<LaneJob> Function() _prelude;

  ExecutorLease? _lease;
  ExecutorRequest? _request;
  Future<void>? _returning;
  final List<_Waiting> _waiting = [];
  bool _ended = false;

  /// True once a job or prelude reached the leased lane; a lane that never
  /// ran one has nothing to reset.
  bool _touched = false;

  /// True while an executor is leased.
  bool get leased => _lease != null;

  /// Where the leased executor is, or null while none is leased. Tests only.
  ExecutorState? get executorState => _lease?.state;

  /// Runs [job] on the leased executor, leasing one first when the lane
  /// holds none. Always completes, as [Lane.submit].
  Future<LaneSubmitResult> submit(LaneJob job) {
    if (_ended) return Future.value(_cancelled());
    final inner = _lease?.lane;
    if (inner != null) {
      _touched = true;
      return inner.submit(job);
    }
    final waiting = _Waiting(job);
    _waiting.add(waiting);
    _acquire();
    return waiting.completer.future;
  }

  /// Cancels one job, whether it waits for an executor or runs on one.
  void cancelJob(int jobId) {
    final index = _waiting.indexWhere((w) => w.job.jobId == jobId);
    if (index >= 0) {
      _waiting.removeAt(index).completer.complete(_cancelled());
      if (_waiting.isEmpty) _abandonRequest();
      return;
    }
    _lease?.lane.cancelJob(jobId);
  }

  /// Releases one held channel of the leased lane.
  void releaseHeld(Object token) => _lease?.lane.releaseHeld(token);

  /// Gives the leased executor back: its jobs are cancelled and the pool
  /// resets it. The lane stays usable; its next job leases again. Does
  /// nothing without a lease.
  void release() {
    final lease = _lease;
    if (lease == null) return;
    _lease = null;
    final untouched = !_touched;
    _touched = false;
    _returning = _pool.release(lease, untouched: untouched).whenComplete(() {
      _returning = null;
      _acquire();
    });
  }

  /// Ends the lane for good: every waiting and running job completes
  /// cancelled at once and the executor goes back to the pool.
  void end() {
    _ended = true;
    for (final waiting in _waiting) {
      waiting.completer.complete(_cancelled());
    }
    _waiting.clear();
    _abandonRequest();
    release();
  }

  void _abandonRequest() {
    _request?.cancel();
    _request = null;
  }

  void _acquire() {
    if (_ended ||
        _lease != null ||
        _request != null ||
        _returning != null ||
        _waiting.isEmpty) {
      return;
    }
    final request = _request = _pool.acquire();
    unawaited(
      request.lease.then(
        (lease) => _leased(request, lease),
        onError: (Object error) => _acquireFailed(request, error),
      ),
    );
  }

  void _leased(ExecutorRequest request, ExecutorLease lease) {
    if (identical(_request, request)) _request = null;
    if (_ended || _lease != null || _waiting.isEmpty) {
      // Nobody wants it any more: the lane ended or every job left the queue.
      unawaited(_pool.release(lease, untouched: true));
      return;
    }
    _lease = lease;
    final inner = lease.lane;
    for (final job in _prelude()) {
      _touched = true;
      unawaited(inner.submit(job));
    }
    final queued = List.of(_waiting);
    _waiting.clear();
    for (final waiting in queued) {
      _touched = true;
      unawaited(
        inner
            .submit(waiting.job)
            .then(
              waiting.completer.complete,
              onError: waiting.completer.completeError,
            ),
      );
    }
  }

  void _acquireFailed(ExecutorRequest request, Object error) {
    if (!identical(_request, request)) return;
    _request = null;
    // The boot failed: every job waiting on it fails with it, and the next
    // job submitted asks the pool again.
    final failed = LaneSubmitResult(
      buildErrorResponse(error.toString()),
      const {},
    );
    for (final waiting in _waiting) {
      waiting.completer.complete(failed);
    }
    _waiting.clear();
  }

  LaneSubmitResult _cancelled() =>
      LaneSubmitResult(buildCancelledResponse(), const {});
}
