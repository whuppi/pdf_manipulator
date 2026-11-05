// Native-only: verify dispose() + process exit never fires a dead
// NativeCallable.
//
// The invariant under test: executor terminate flags every channel under its
// pair mutex BEFORE any Rust thread can notify, so no lane thread
// ever posts to a closed NativeCallable ("Callback invoked after it
// has been deleted" is a FATAL abort on Windows).
//
// Subprocess because FATAL aborts the process — test harness can't
// catch it. Same pattern as Dart SDK's own FFI callback tests.

// io-exempt: spawns a subprocess (a FATAL abort can't be caught in-process).
import 'dart:io';

import 'package:test/test.dart';
import '../../../harness/timeouts.dart';

void registerNativeFinalizerTests() {
  test('dispose exits cleanly (no dangling NativeCallable)', () async {
    // Do NOT change to `dart run` — it will fail on Windows with:
    //   PathAccessException: Cannot delete file, path = '...pdf_oxide.dll'
    //   (OS Error: Access is denied, errno = 5)
    // Windows locks loaded DLLs. The parent test process has pdf_oxide.dll
    // loaded via @Native FFI. `dart run` triggers build hooks which try to
    // copy the DLL → Access denied. `dart <file>` skips hooks entirely.
    final repro = File(
      'test/ops/platform/native/dispose_exit_payload.dart',
    ).absolute.path;
    final process = await Process.start(Platform.resolvedExecutable, [repro]);
    final exitCode = await process.exitCode.timeout(
      Duration(seconds: 30),
      onTimeout: () {
        process.kill();
        return -1;
      },
    );
    expect(
      exitCode,
      0,
      reason: exitCode == -1
          ? 'Process hung — dispose() did not shut down cleanly.'
          : 'Process crashed (exit $exitCode) — dangling NativeCallable.\n'
                'stderr: ${await process.stderr.transform(const SystemEncoding().decoder).join()}',
    );
    // Above the 30 s hang detector, so a slow subprocess start is never
    // reported as a failure the detector would not call a hang.
  }, timeout: t(35));

  test('a write cancelled mid-sink leaves the freed buffer alone', () async {
    // Same launch as above. Each run frees the write buffer while the sink
    // is busy and then lets the sink return; the payload itself fails when
    // the listener's guard did not see that late write.
    final payload = File(
      'test/ops/platform/native/slow_sink_cancel_payload.dart',
    ).absolute.path;
    for (var run = 0; run < 20; run++) {
      final process = await Process.start(Platform.resolvedExecutable, [
        payload,
      ]);
      final stderr = process.stderr
          .transform(const SystemEncoding().decoder)
          .join();
      final exitCode = await process.exitCode.timeout(
        Duration(seconds: 30),
        onTimeout: () {
          process.kill();
          return -1;
        },
      );
      expect(
        exitCode,
        0,
        reason: 'run $run exited $exitCode\nstderr: ${await stderr}',
      );
    }
  }, timeout: t(120));
}
