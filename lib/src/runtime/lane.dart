// Lease and Executor — the platform-blind interfaces to one executor and to
// one lease on it.
//
// An Executor is the platform resource: a Rust thread with its engine state,
// or a Web Worker with a WASM instance. The ExecutorPool (pool.dart) owns
// every executor and leases each to one lane at a time. A Lease is the
// lease adapter: it carries the lease id on every message.
//
// A lease adapter decides NOTHING: implementations of these interfaces
// translate verbs into platform calls. Routing,
// pinning, queuing, cancellation bookkeeping, and completion ownership
// all live in the Router and the pool. If an `if` that makes a decision
// appears in an implementation, the design has failed — move it there.
//
//   Native: one Rust thread + mailbox        (native/lease.dart)
//   Web:    one Web Worker                   (web/lease.dart)
//
// INTERNAL — used by the Router and the pool only.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// The message of the typed error a handle or job gets after its executor's
/// engine trapped or panicked. Matches `ENGINE_DEAD_MESSAGE` in the engine.
const engineDeadMessage = 'the engine stopped after an operation panicked';

/// The error message of a job or reset that carried a lease which was not
/// the executor's current one. Matches `STALE_LEASE_MESSAGE` in the engine.
const staleLeaseMessage =
    "the executor's lease was replaced; this message is stale";

/// The error message of a reset refused because a job was outstanding.
/// Matches `EXECUTOR_BUSY_MESSAGE` in the engine.
const executorBusyMessage =
    'the executor cannot reset while a job is outstanding';

/// An executor refused a message because it broke the protocol: a reset while
/// a job is outstanding, a lease that is not current, an executor that stopped.
/// The adapters never send such a message, so this reports a bug in the
/// caller, never a failure of the document.
class LeaseProtocolError implements Exception {
  /// Creates the error with the executor's [message].
  const LeaseProtocolError(this.message);

  /// What the executor refused, in its own words.
  final String message;

  @override
  String toString() => 'LeaseProtocolError: $message';
}

/// One operation, fully described. Built by the Router.
class Job {
  /// Creates a job descriptor. [jobId] is unique per Router lifetime.
  Job({
    required this.jobId,
    required this.request,
    required this.sources,
    required this.sinks,
    required this.keepSources,
    this.holdGroupId,
  });

  /// Router-assigned identity (single-threaded counter — the one
  /// owner of job identity on this platform).
  final int jobId;

  /// Binary-encoded request (op name + args).
  final Uint8List request;

  /// Indexed sources, served on demand during the job.
  final List<DataSource> sources;

  /// Indexed sinks, receiving output chunks during the job.
  final List<DataSink> sinks;

  /// Source indices whose I/O channel outlives the job (the engine
  /// moves these sources into a handle). Their held tokens come back
  /// in [JobResult.holds].
  final Set<int> keepSources;

  /// For ops on an existing handle: the id of the job that created it
  /// (Router bookkeeping — the Router stores it at pin time and passes
  /// it back here). Every source the handle keeps, from that job or a
  /// later one, is held under this group. Web lanes use it to mount the
  /// handle's held readers and to file newly kept ones; native lanes
  /// ignore it (the engine holds its readers inside the document).
  final int? holdGroupId;
}

/// What a completed job hands back to the Router.
class JobResult {
  /// Creates a result. [holds] maps kept source indices to
  /// opaque lane-owned tokens for later [Lease.releaseHold].
  JobResult(this.bytes, this.holds, {this.executorDied = false});

  /// True when the engine on this executor trapped or panicked: the executor
  /// is dead and its handles are gone, and the Router must retire it.
  final bool executorDied;

  /// Binary response bytes (success or engine-encoded error).
  final Uint8List bytes;

  /// Kept-channel tokens by source index. Opaque to the Router.
  final Map<int, Object> holds;
}

/// One lane's lease on one executor: its adapter for the lease it holds.
/// Six verbs, no decisions.
abstract interface class Lease {
  /// Run one job. Always completes — with result bytes, an
  /// engine-encoded error, or (after [drain]) a cancelled error built
  /// by the lane itself. Never throws for expected failures.
  Future<JobResult> submit(Job job);

  /// Cancel one job on this lane. Instant, idempotent, fire-and-forget.
  void cancelJob(int jobId);

  /// Release one held channel (its handle was disposed).
  void releaseHold(Object token);

  /// Give the executor a fresh engine state: every document, held reader
  /// and job file it owned is gone, the thread or worker stays. Completes
  /// with the wasm memory size on web, null on native. Throws
  /// [LeaseProtocolError] while a job is outstanding (the caller drains the
  /// lane first) and after the engine died; resetting an idle executor
  /// again is fine.
  Future<int?> reset();

  /// End this lane's use of its executor. Instant for the caller: every
  /// pending [submit] future completes with a cancelled error at once, a
  /// running job is cancelled, and no platform resource of the lane is
  /// freed before its job has posted. Completes with true once the
  /// executor holds nothing of this lane and its engine is intact (the
  /// executor may be reset and leased again), or with false when the engine
  /// died. Idempotent.
  Future<bool> drain();
}

/// A [Lease] that can say what it waits for, for the record of a transition
/// that outlasted its cover (see ExecutorPool).
abstract interface class LanePhase {
  /// The lease and what the lane awaits now: a running job's post, a reset's
  /// answer, or nothing.
  String get phase;
}

/// One platform executor: a thread with its engine state, or a worker with
/// its WASM instance. Owned by the ExecutorPool.
abstract interface class Executor {
  /// Start a new lease on this executor and return the lane that holds it.
  /// Every message of the lane carries the lease id, and the executor drops
  /// one of an earlier lease. Throws [LeaseProtocolError] when the executor
  /// refuses a lease: its engine died, or a job is still outstanding.
  Lease beginLane();

  /// The most bytes the executor's engine has held since its last reset, as of
  /// its last answer: peak live bytes, on both platforms.
  int get heapBytes;

  /// End the executor for good. Web: terminates the worker. Native:
  /// abandons the thread, which exits once the job it runs unwinds. Never
  /// called while a lane's job can still answer through it, except for a
  /// dead engine. Idempotent.
  void terminate();
}

/// Boots executors for one platform. The ExecutorPool owns how many and
/// when.
abstract interface class ExecutorBooter {
  /// Start one executor and complete once it is ready for a lease. Throws
  /// when the boot fails; a failed boot leaves nothing behind.
  Future<Executor> boot();

  /// Decides this booter's I/O mode, once, and completes with it — the Router
  /// reports it as the instance's [PdfIoMode]. A web booter learns what the
  /// app shipped from the network, so the decision is asynchronous; it throws
  /// `PdfUnsupported` when no mode can run. A failure is not kept: the next
  /// call decides again.
  Future<PdfIoMode> resolveMode();

  /// How many lanes run at once when the caller doesn't override it, sized
  /// from the platform's CPU count.
  int get defaultConcurrency;

  /// What the executors of this booter are: booters with the same key boot
  /// interchangeable executors and share one pool.
  String get poolKey;
}

/// One lane per two cores, at least two — headroom for the app's own
/// threads; idle lanes sleep at zero CPU. Each host sizes from its own
/// platform's core count.
int suggestedLaneCount(int cores) => cores ~/ 2 < 2 ? 2 : cores ~/ 2;
