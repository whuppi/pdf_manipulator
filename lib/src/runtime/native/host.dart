// NativeLaneHost — executor booting for the native runtime.
//
// Part of lane.dart (the executor and lane live there). Native's host is
// tiny on purpose: booting an executor is ONE FFI call (Rust owns the
// thread and its mailbox), so the only host-side state is the one-time
// Dart_PostCObject bootstrap that lets Rust post results back to Dart
// ports. The pool above owns the budget and the waiters.

part of 'lane.dart';

/// Boots native executors. Stateless beyond bootstrap.
class NativeLaneHost implements LaneHost {
  static bool _bootstrapped = false;

  @override
  PdfIoMode get mode => PdfIoMode.native;

  @override
  int get defaultLaneCount => suggestedLaneCount(Platform.numberOfProcessors);

  @override
  String get poolKey => 'native';

  @override
  Future<Executor> boot() async {
    if (!_bootstrapped) {
      bindings.storeDartPostCobject(ffi.NativeApi.postCObject.cast());
      _bootstrapped = true;
    }
    return NativeExecutor._(bindings.laneSpawn());
  }
}
