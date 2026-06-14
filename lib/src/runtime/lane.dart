// Lane — the platform-blind interface to one isolated execution unit.
//
// Shared brain, dumb edges: implementations of this interface
// translate verbs into platform physics and decide NOTHING. Routing,
// pinning, queuing, cancellation bookkeeping, and completion ownership
// all live in the Router. If an `if` that makes a decision appears in
// a Lane implementation, the design has failed — move it to the Router.
//
//   Native: one Rust thread + mailbox        (native/native_lane.dart)
//   Web:    one Web Worker                   (web/web_lane.dart)
//
// INTERNAL — used by the Router only.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';

/// One operation, fully described. Built by the Router.
class LaneJob {
  /// Creates a job descriptor. [jobId] is unique per Router lifetime.
  LaneJob({
    required this.jobId,
    required this.request,
    required this.sources,
    required this.sinks,
    required this.keepSources,
    this.heldToken,
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

  /// For ops on an existing handle: the held token of the job that
  /// created it (Router bookkeeping — the Router stores it at pin
  /// time and passes it back here). Web lanes use it to mount the
  /// handle's held readers; native lanes ignore it (the engine holds
  /// its reader inside the document).
  final Object? heldToken;
}

/// What a completed job hands back to the Router.
class LaneSubmitResult {
  /// Creates a result. [heldTokens] maps kept source indices to
  /// opaque lane-owned tokens for later [Lane.releaseHeld].
  LaneSubmitResult(this.bytes, this.heldTokens);

  /// Binary response bytes (success or engine-encoded error).
  final Uint8List bytes;

  /// Kept-channel tokens by source index. Opaque to the Router.
  final Map<int, Object> heldTokens;
}

/// One isolated execution unit. Four verbs, no decisions.
abstract interface class Lane {
  /// Run one job. Always completes — with result bytes, an
  /// engine-encoded error, or (after [kill]) a cancelled error built
  /// by the lane itself. Never throws for expected failures.
  Future<LaneSubmitResult> submit(LaneJob job);

  /// Cancel one job on this lane. Instant, idempotent, fire-and-forget.
  void cancelJob(int jobId);

  /// Release one held channel (its handle was disposed).
  void releaseHeld(Object token);

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
}
