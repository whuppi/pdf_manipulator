// Pump strategies — frame-advancing helpers that never hang.
//
// pumpAndSettle waits for ALL animations to stop. If a screen has an
// indeterminate/transient animation (a CircularProgressIndicator while a
// job runs), it never stops and pumpAndSettle throws `timed out`. These
// helpers advance frames deliberately instead, so a test is never at the
// mercy of a spinner.
//
// App-agnostic — part of harness/.

import 'package:flutter/scheduler.dart';
import 'package:flutter_test/flutter_test.dart';

/// Pump frames until [condition] is true, or fail after [timeout].
///
/// Polls by pumping a fixed [step] each iteration — works whether or not
/// an animation is running, because it never waits for "settled", only
/// for the condition the caller actually cares about.
Future<void> pumpUntil(
  WidgetTester tester,
  bool Function() condition, {
  Duration timeout = const Duration(seconds: 10),
  Duration step = const Duration(milliseconds: 50),
  String describe = 'condition',
}) async {
  final deadline = tester.binding.clock.now().add(timeout);
  while (!condition()) {
    if (tester.binding.clock.now().isAfter(deadline)) {
      throw TestFailure('pumpUntil: $describe not met within $timeout');
    }
    await tester.pump(step);
  }
}

/// Advance frames to a stable point, but stop the moment the only thing
/// keeping the app busy is a transient animation that won't end.
///
/// Strategy: pump until no frame is scheduled (truly idle) OR a small
/// frame budget is exhausted (a spinner is ticking forever). Either way
/// returns — never hangs. The budget is generous enough that real
/// one-shot transitions (a tab swap, a fade) complete first.
Future<void> settleWithoutHanging(
  WidgetTester tester, {
  int maxFrames = 60,
  Duration step = const Duration(milliseconds: 16),
}) async {
  for (var i = 0; i < maxFrames; i++) {
    // No frame scheduled → genuinely idle, nothing left to animate.
    if (!SchedulerBinding.instance.hasScheduledFrame) return;
    await tester.pump(step);
  }
  // Budget spent: a perpetual animation is still ticking. That's fine —
  // the caller's own assertion (or pumpUntil) gates correctness, not the
  // absence of animation.
}
