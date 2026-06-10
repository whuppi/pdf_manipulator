# Flutter test hangs after Android integration tests pass on slow emulators

Investigation and fix for `flutter test` hanging indefinitely after
all integration tests pass when running on an Android emulator without
hardware acceleration (SwiftShader / software rendering).

---

## Summary

`flutter test integration_test/` on an Android emulator completes all
tests successfully but the `flutter test` process never exits. The Dart
VM hangs during teardown, preventing the test summary (`🎉 N tests
passed.`) from printing and the process from returning.

**Affected environment:** Any Android emulator running without hardware
GPU acceleration — specifically macOS Intel CI runners (macos-15-intel)
where the emulator uses SwiftShader software rendering at ~200% CPU on
a 4-core host.

**Not affected:** Linux with KVM (ubuntu-24.04), local machines with
hardware acceleration, Windows emulators (different teardown path).

---

## Root cause

### The hang chain

```
flutter test
  └── finalize() — runs finalizers, NO TIMEOUT
        └── testDevice.kill()
              └── device.stopApp(package)
                    └── _processUtils.stream(['adb', 'shell', 'am', 'force-stop', packageId])
                          └── adb shell am force-stop — HANGS HERE
```

### Why `adb shell am force-stop` hangs

`am force-stop` is a high-level Android command processed by
`ActivityManagerService` inside the emulator's `system_server` JVM
process. On a SwiftShader emulator:

- **qemu runs at ~200% CPU** doing software rendering
- **system_server is CPU-starved** — barely gets scheduled
- `am force-stop` requires ActivityManager to acquire locks, iterate
  processes, send signals — all CPU-intensive operations that never
  complete when the guest OS is starved

Other `adb` commands work fine during the hang:
- `adb devices` → works (handled by host-side adb server, never
  enters the emulator)
- `adb shell getprop sys.boot_completed` → returns `1` (reads a
  property file via kernel, minimal CPU)
- `adb shell am force-stop` → hangs (requires ActivityManager in
  system_server, needs significant CPU)

**This is CPU starvation, not a deadlock.** The command is queued
inside the emulator but never gets CPU time to execute.

### Why the process never exits

`_processUtils.stream()` in Flutter's `android_device.dart` has
**no timeout parameter**. It calls `Process.start()` and awaits the
exit code forever. When `adb shell` never returns, `stream()` never
returns, `stopApp()` never returns, `kill()` never returns, the
finalizer never completes, and `flutter test` hangs indefinitely.

**Source locations:**
- `packages/flutter_tools/lib/src/android/android_device.dart` —
  `stopApp()` calls `_processUtils.stream(adb shell am force-stop)`
- `packages/flutter_tools/lib/src/test/integration_test_device.dart` —
  `kill()` awaits `device.stopApp()` then `device.uninstallApp()`
- `packages/flutter_tools/lib/src/test/flutter_platform.dart` —
  `finalize()` runs finalizers with no timeout (`await finalizer()`)
- `packages/flutter_tools/lib/src/base/process.dart` — `stream()`
  interface has no timeout parameter

### Why Linux works fine

Linux CI (ubuntu-24.04) uses KVM hardware acceleration. The emulator
runs at ~5% CPU. `system_server` gets plenty of CPU time.
`am force-stop` completes in milliseconds. The teardown chain finishes
instantly.

---

## Proof of concept

### Setup

Background debug probe running every 60 seconds alongside
`flutter test`, monitoring process state, open file handles, and
child processes of the Dart VM.

### Evidence: the stuck adb process

At probe 22m (2 minutes after all tests passed):

```
-- adb child of dartvm (stopApp hang indicator) --
  child 42123 of dartvm 92498:
    /Users/runner/Library/Android/sdk/platform-tools/adb
    (elapsed=01:01, cpu=0.0%)
  *** STUCK ADB DETECTED (child of dartvm) ***
```

The `adb` process (PID 42123) is:
- A child of `dartvm` (PID 92498, the flutter_tools process)
- Running for 1+ minute at **0% CPU** — blocked on I/O
- Connected to `localhost:5037` (the adb server) via TCP

The `dartvm` process holds 3 orphan PIPEs (fd 14, 16, 17 — no peer)
and is blocked waiting for the `adb` child to exit.

### Evidence: killing adb unblocks everything

```
>>> KILLING stuck adb PID 42123 to test if flutter test unblocks <<<
>>> KILLED. Watching if flutter test exits... <<<
adb uninstall failed: ProcessException: Process exited abnormally
  with exit code -9
🎉 49 tests passed.
```

After killing the stuck `adb` process:
1. `stopApp()` receives an error return → continues
2. `uninstallApp()` tries its own adb call → also fails (killed) →
   prints error but doesn't crash
3. `device.dispose()` runs → stops log readers
4. `_ddsLauncher.shutdown()` runs → shuts down DDS
5. Flutter test prints `🎉 49 tests passed.` → writes test report →
   **exits with code 0**
6. Emulator terminates cleanly
7. Total elapsed: ~22 minutes (build + tests + hang + recovery)

### Evidence: consistent across runs

| Run | Tests | Hang? | adb stuck? | Kill fixed? |
|-----|-------|-------|------------|-------------|
| #1  | 49 ✅ | Yes   | PID 72449, 8 min at 0% CPU | Not tested |
| #2  | 49 ✅ | Yes   | PID 72449, 7+ min at 0% CPU | Not tested |
| #3  | 49 ✅ | Yes   | PID 42123, 1 min at 0% CPU | **Yes → 🎉** |

---

## The bugs

### 1. Flutter: `stopApp()` has no timeout (primary)

**File:** `packages/flutter_tools/lib/src/android/android_device.dart`

```dart
@override
Future<bool> stopApp(ApplicationPackage? app, ...) async {
  final command = adbCommandForDevice(['shell', 'am', 'force-stop', app.id]);
  return _processUtils.stream(command)  // ← NO TIMEOUT
      .then<bool>((int exitCode) => exitCode == 0 || ...);
}
```

**Fix:** Add a timeout to `stream()` or use `Process.start()` with
a `Timer` that kills the process after N seconds. `stopApp` should
return `false` on timeout, not hang forever.

**Alternative fix:** Use `adb shell kill <pid>` instead of
`am force-stop`. The `kill` syscall is handled by the Linux kernel
inside the emulator, not by `ActivityManagerService`. It works even
when `system_server` is starved.

### 2. dart-lang/native: hooks_runner strips SCCACHE env vars

**File:** `pkgs/hooks_runner/lib/src/build_runner/build_runner.dart`

The hooks runner's environment filter passes through `CCACHE_` prefix
(line 628) but not `SCCACHE_` or `RUSTC_WRAPPER`. Rust packages using
sccache for CI compilation caching get zero cache hits because the
env vars are stripped.

```dart
const variablePrefixesFilter = {
  'CCACHE_',  // ← C/C++ cache supported
  'DOTNET_',
  'NIX_',
  'NUGET_',
  // Missing: 'SCCACHE_'
};
// Missing from staticVariablesFilter: 'RUSTC_WRAPPER'
```

**Fix:** Add `'SCCACHE_'` to `variablePrefixesFilter` and
`'RUSTC_WRAPPER'` to `staticVariablesFilter`.

---

## Workaround (our implementation)

Until Flutter fixes the timeout, we detect and kill the stuck `adb`
child process from a background monitor in the Makefile:

1. Run `flutter test` in background
2. Probe every 60s — walk dartvm's child processes
3. If an `adb` child exists at 0% CPU → it's the stuck `stopApp` call
4. Kill it once → `stopApp()` returns with error → teardown continues
5. Flutter test exits cleanly with `🎉`

For the sccache issue, `build.dart` detects sccache on PATH (which IS
passed through) and sets `RUSTC_WRAPPER` in the cargo `Process.run`
environment explicitly.

---

## Environment details

| Component | Version/Spec |
|-----------|-------------|
| Runner | macos-15-intel (4 cores, 14 GB, Intel x86_64) |
| Emulator | Android Emulator 36.6.11, aosp_atd API 30, x86_64 |
| GPU | SwiftShader (software rendering, no hardware acceleration) |
| Flutter | 3.44.1 stable |
| Dart | 3.12.1 |
| qemu CPU usage | ~200% (2 of 4 cores) |
| Emulator boot time | ~314 seconds |
| Gradle assembleDebug | ~864 seconds (with gradle cache) |
| Test execution | ~90 seconds (49 tests) |
| Hang duration | Indefinite (killed by job timeout or probe) |

---

## References

- Flutter `stopApp()`: [`android_device.dart`](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/android/android_device.dart)
- Flutter `kill()`: [`integration_test_device.dart`](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/test/integration_test_device.dart)
- Flutter `finalize()`: [`flutter_platform.dart`](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/test/flutter_platform.dart)
- hooks_runner env filter: [`build_runner.dart` L608-632](https://github.com/dart-lang/native/blob/main/pkgs/hooks_runner/lib/src/build_runner/build_runner.dart)
- ReactiveCircus emulator-runner hang: [#385](https://github.com/ReactiveCircus/android-emulator-runner/issues/385)
