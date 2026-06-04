// Native-only: verify dispose() shuts down cleanly without crashing.
//
// Before the fix: dispose() killed the coordinator isolate while Rust
// pool threads still held NativeCallable pointers. On Windows this
// crashed with "Callback invoked after it has been deleted" (FATAL).
//
// After the fix: dispose() cancels held buffers (wakes blocked Rust
// threads), calls bridgeShutdown (joins threads), THEN closes
// NativeCallables. Clean exit, no dangling pointers.
//
// Subprocess because FATAL aborts the process — test harness can't
// catch it. Same pattern as Dart SDK's own FFI callback tests.

import 'dart:io';

import 'package:test/test.dart';

void registerNativeFinalizerTests() {
  test('dispose exits cleanly (no dangling NativeCallable)',
      () async {
    final process = await Process.start(Platform.resolvedExecutable, [
      'run',
      'test/ops/native/native_gc_repro.dart',
    ]);
    final exitCode = await process.exitCode.timeout(
      Duration(seconds: 30),
      onTimeout: () {
        process.kill();
        return -1;
      },
    );
    expect(exitCode, 0,
        reason: exitCode == -1
            ? 'Process hung — dispose() did not shut down cleanly.'
            : 'Process crashed (exit $exitCode) — dangling NativeCallable.\n'
                'stderr: ${await process.stderr.transform(const SystemEncoding().decoder).join()}');
  }, timeout: Timeout(Duration(seconds: 45)));
}
