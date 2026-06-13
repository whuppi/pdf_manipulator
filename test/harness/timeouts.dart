import 'package:test/test.dart';

/// Per-test timeout that respects the `--timeout` CLI multiplier.
///
/// Locally: `t(3)` = 3 seconds (tight, catches perf regressions).
/// CI with `--timeout=30x`: `t(3)` = 90 seconds (absorbs runner variance).
///
/// Uses `Timeout.factor()` because `Timeout(Duration())` ignores the
/// CLI multiplier entirely — the Dart test framework only multiplies
/// factor-based timeouts.
Timeout t(int seconds) => Timeout.factor(seconds / 30);
