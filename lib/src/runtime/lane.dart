// Lane — the platform-blind interface to one isolated execution unit.
//
// Shared brain, dumb edges: implementations of this interface
// translate verbs into platform physics and decide NOTHING. Routing,
// pinning, queuing, cancellation bookkeeping, and completion ownership
// all live in the Router. If an `if` that makes a decision appears in
// a Lane implementation, the design has failed — move it to the Router.
//
//   Native: one Rust thread + mailbox        (native/lane.dart)
//   Web:    one Web Worker                   (web/lane.dart)
//
// INTERNAL — used by the Router only.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// The message of the typed error a handle or job gets after its lane's
/// engine trapped or panicked. Matches `LANE_DEAD_MESSAGE` in the engine.
const laneDeadMessage =
    'the engine lane stopped after an engine operation panicked';

/// The error message of a job or reset that carried a lease which was not
/// the executor's current one. Matches `STALE_LEASE_MESSAGE` in the engine.
const laneStaleLeaseMessage =
    "the lane's lease was replaced; this message is stale";

/// The error message of a reset refused because a job was outstanding.
/// Matches `LANE_BUSY_MESSAGE` in the engine.
const laneBusyMessage = 'the lane cannot reset while a job is outstanding';

/// A lane refused a message because it broke the protocol: a reset while a
/// job is outstanding, a lease that is not current, a lane that stopped.
/// The adapters never send such a message, so this reports a bug in the
/// caller, never a failure of the document.
class LaneProtocolError implements Exception {
  /// Creates the error with the executor's [message].
  const LaneProtocolError(this.message);

  /// What the executor refused, in its own words.
  final String message;

  @override
  String toString() => 'LaneProtocolError: $message';
}

/// One operation, fully described. Built by the Router.
class LaneJob {
  /// Creates a job descriptor. [jobId] is unique per Router lifetime.
  LaneJob({
    required this.jobId,
    required this.request,
    required this.sources,
    required this.sinks,
    required this.keepSources,
    this.heldGroupId,
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
  /// in [LaneSubmitResult.heldTokens].
  final Set<int> keepSources;

  /// For ops on an existing handle: the id of the job that created it
  /// (Router bookkeeping — the Router stores it at pin time and passes
  /// it back here). Every source the handle keeps, from that job or a
  /// later one, is held under this group. Web lanes use it to mount the
  /// handle's held readers and to file newly kept ones; native lanes
  /// ignore it (the engine holds its readers inside the document).
  final int? heldGroupId;
}

/// What a completed job hands back to the Router.
class LaneSubmitResult {
  /// Creates a result. [heldTokens] maps kept source indices to
  /// opaque lane-owned tokens for later [Lane.releaseHeld].
  LaneSubmitResult(
    this.bytes,
    this.heldTokens, {
    this.peakBytes,
    this.laneDied = false,
  });

  /// True when the engine on this lane trapped or panicked: the lane has
  /// stopped serving, its handles are gone, and the Router must retire it.
  final bool laneDied;

  /// Binary response bytes (success or engine-encoded error).
  final Uint8List bytes;

  /// The lane's lifetime peak committed memory after this job, when the
  /// lane can tell (web workers); null otherwise, always on native.
  final int? peakBytes;

  /// Kept-channel tokens by source index. Opaque to the Router.
  final Map<int, Object> heldTokens;
}

/// One isolated execution unit. Five verbs, no decisions.
abstract interface class Lane {
  /// Run one job. Always completes — with result bytes, an
  /// engine-encoded error, or (after [kill]) a cancelled error built
  /// by the lane itself. Never throws for expected failures.
  Future<LaneSubmitResult> submit(LaneJob job);

  /// Cancel one job on this lane. Instant, idempotent, fire-and-forget.
  void cancelJob(int jobId);

  /// Release one held channel (its handle was disposed).
  void releaseHeld(Object token);

  /// Give the executor a fresh engine state: every document, held reader
  /// and job file it owned is gone, the thread or worker stays. Completes
  /// with the wasm memory size on web, null on native. Throws
  /// [LaneProtocolError] while a job is outstanding (the caller drains the
  /// lane first) and after the engine died; resetting an idle executor
  /// again is fine.
  Future<int?> reset();

  /// Kill this lane and everything on it. Instant: pending [submit]
  /// futures complete with a cancelled error, all platform resources
  /// are released, no callback fires afterwards. Idempotent.
  void kill();
}

/// Spawns lanes for one platform. The Router owns how many and when.
abstract interface class LaneHost {
  /// Create a new lane. Never blocks, never errors (platform budget
  /// pressure queues work instead of failing).
  Lane spawn();

  /// This host's I/O mode — the Router reports it as the instance's
  /// [PdfIoMode].
  PdfIoMode get mode;

  /// Lane count when the caller doesn't override it, sized from the
  /// platform's CPU count.
  int get defaultLaneCount;
}

/// One lane per two cores, at least two — headroom for the app's own
/// threads; idle lanes sleep at zero CPU. Each host sizes from its own
/// platform's core count.
int suggestedLaneCount(int cores) => cores ~/ 2 < 2 ? 2 : cores ~/ 2;
