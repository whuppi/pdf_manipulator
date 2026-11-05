// NativeExecutorBooter — executor booting for the native runtime.
//
// Part of lease.dart (the executor and lease adapter live there). Native's booter is
// tiny on purpose: booting an executor is ONE FFI call (Rust owns the
// thread and its mailbox), so the only host-side state is the one-time
// Dart_PostCObject bootstrap that lets Rust post results back to Dart
// ports. The pool above owns the budget and the waiters.

part of 'lease.dart';

/// Boots native executors. Stateless beyond bootstrap.
class NativeExecutorBooter implements ExecutorBooter {
  static bool _bootstrapped = false;

  @override
  PdfIoMode get mode => PdfIoMode.native;

  @override
  int get defaultConcurrency => suggestedLaneCount(Platform.numberOfProcessors);

  @override
  String get poolKey => 'native';

  @override
  Future<Executor> boot() async {
    if (!_bootstrapped) {
      bindings.storeDartPostCobject(ffi.NativeApi.postCObject.cast());
      _bootstrapped = true;
    }
    return NativeExecutor._(bindings.executorBoot());
  }
}
