// NativeLaneHost — lane spawning for the native runtime.
//
// Part of lane.dart (the lane itself lives there). Native's
// host is tiny on purpose: a lane spawn is ONE FFI call (Rust owns
// the thread budget, the FIFO waiters, and the table), so the only
// host-side state is the one-time Dart_PostCObject bootstrap that
// lets Rust post results back to Dart ports.

part of 'lane.dart';

/// Spawns native lanes. One per Router; stateless beyond bootstrap.
class NativeLaneHost implements LaneHost {
  static bool _bootstrapped = false;

  @override
  PdfIoMode get mode => PdfIoMode.native;

  @override
  int get defaultLaneCount => suggestedLaneCount(Platform.numberOfProcessors);

  @override
  Lane spawn() {
    if (!_bootstrapped) {
      bindings.storeDartPostCobject(ffi.NativeApi.postCObject.cast());
      _bootstrapped = true;
    }
    return NativeLane._(bindings.laneSpawn());
  }
}
