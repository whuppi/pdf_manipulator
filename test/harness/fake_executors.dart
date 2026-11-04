// Fake executors for the pool and Router unit tests: no engine, no
// platform. A fake lane records every verb and completes jobs on command;
// a fake executor models the lease counter and the drop of a stale lease;
// a fake host boots executors on command.

import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/runtime/wire_peek.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// A success response with zero fields.
Uint8List okBytes() => Uint8List.fromList(const [1, 0, 0]);

/// The lane of a fake executor: one object for the executor's whole life,
/// whose [lease] changes with each tenure.
class FakeLane implements Lane {
  FakeLane(this.executor);

  final FakeExecutor executor;

  /// The lease the lane holds now.
  int lease = 0;

  final submitted = <LaneJob>[];
  final cancelledJobs = <int>[];
  final releasedTokens = <Object>[];

  /// Pending completers by jobId — tests complete them explicitly.
  final pending = <int, Completer<LaneSubmitResult>>{};

  /// When set, submit completes immediately with this result.
  LaneSubmitResult? autoResult;

  /// When set, [drain] completes only when this does, as a worker whose job
  /// is still unwinding.
  Completer<bool>? drainGate;

  /// The engine died: drain answers false.
  bool engineDead = false;

  /// True between [drain] and the next lease.
  bool ended = false;

  int drains = 0;
  int resets = 0;

  @override
  Future<LaneSubmitResult> submit(LaneJob job) {
    submitted.add(job);
    if (ended) return Future.value(_cancelled());
    final auto = autoResult;
    if (auto != null) return Future.value(auto);
    final completer = Completer<LaneSubmitResult>();
    pending[job.jobId] = completer;
    return completer.future;
  }

  /// Completes every still-pending job successfully.
  void completeAllPending() {
    for (final completer in pending.values) {
      if (!completer.isCompleted) {
        completer.complete(LaneSubmitResult(okBytes(), const {}));
      }
    }
  }

  @override
  void cancelJob(int jobId) => cancelledJobs.add(jobId);

  @override
  void releaseHeld(Object token) => releasedTokens.add(token);

  @override
  Future<int?> reset() async {
    resets++;
    executor.events.add('reset');
    if (executor.failReset) throw const LaneProtocolError('reset failed');
    return executor.heap;
  }

  @override
  Future<bool> drain() {
    drains++;
    executor.events.add('drain');
    ended = true;
    for (final completer in pending.values) {
      if (!completer.isCompleted) completer.complete(_cancelled());
    }
    return drainGate?.future ?? Future.value(!engineDead);
  }

  LaneSubmitResult _cancelled() =>
      LaneSubmitResult(buildCancelledResponse(), const {});
}

class FakeExecutor implements Executor {
  FakeExecutor(this.id) {
    lane = FakeLane(this);
  }

  final int id;
  late final FakeLane lane;

  /// What a lease start refuses with: the engine died.
  bool failLease = false;

  /// What a reset fails with.
  bool failReset = false;

  bool terminated = false;
  int lastLease = 0;
  int heap = 0;

  /// Every drain and reset of this executor, in order.
  final events = <String>[];

  /// The lease the executor serves now.
  int get currentLease => lastLease;

  /// Whether a message under [lease] reaches the lane or is dropped.
  bool accepts(int lease) => !terminated && lease == lastLease;

  @override
  int get heapBytes => heap;

  @override
  Lane beginLane() {
    if (failLease) throw const LaneProtocolError('the engine died');
    lane
      ..lease = ++lastLease
      ..ended = false;
    events.add('lease');
    return lane;
  }

  @override
  void terminate() => terminated = true;
}

class FakeLaneHost implements LaneHost {
  FakeLaneHost({this.key = 'fake'});

  final String key;
  final executors = <FakeExecutor>[];

  /// The lane of every executor booted so far, in boot order.
  List<FakeLane> get lanes => [for (final e in executors) e.lane];

  /// When set, a boot waits for it to complete.
  Completer<void>? bootGate;

  /// Boots fail with this error while it is set.
  Object? bootError;

  /// The next this many boots fail, then boots succeed again.
  int bootFailures = 0;

  /// Called with each executor as it boots.
  void Function(FakeExecutor executor)? onBoot;

  int bootAttempts = 0;

  @override
  PdfIoMode get mode => PdfIoMode.native;

  @override
  int get defaultLaneCount => 2;

  @override
  String get poolKey => key;

  @override
  Future<Executor> boot() async {
    bootAttempts++;
    final gate = bootGate;
    if (gate != null) await gate.future;
    final error = bootError;
    if (error != null) throw error;
    if (bootFailures > 0) {
      bootFailures--;
      throw StateError('boot failed');
    }
    final executor = FakeExecutor(executors.length);
    executors.add(executor);
    onBoot?.call(executor);
    return executor;
  }
}
