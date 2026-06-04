// FFI bindings for the per-instance Rust bridge.
//
// Three entry points:
//   bridgeInit()     → creates an InstanceState, returns opaque pointer
//   bridgeExecute()  → runs an op on the instance's thread pool
//   bridgeShutdown() → drops the instance, frees all memory + threads
//
// Multi-source/multi-sink: bridgeExecute takes parallel arrays of
// (buf, notify, length) triples for sources and (buf, notify) pairs
// for sinks. Any count — not hardcoded.
//
// INTERNAL — used by coordinator.dart only.

@ffi.DefaultAsset('package:pdf_manipulator/src/ffi/native_bindings.g.dart')
library;

import 'dart:ffi' as ffi;

// ── allo-isolate bootstrap ───────────────────────────────────────

/// Registers the Dart post-C-object callback for allo-isolate.
@ffi.Native<ffi.Void Function(
    ffi.Pointer<ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>>)>(
    symbol: 'store_dart_post_cobject')
external void storeDartPostCobject(
  ffi.Pointer<ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>> postCObject,
);

// ── Instance lifecycle ──────────────────────────────────────────

/// Creates a new Rust engine instance and returns an opaque pointer.
@ffi.Native<ffi.Pointer<ffi.Void> Function()>(symbol: 'bridge_init')
external ffi.Pointer<ffi.Void> bridgeInit();

/// Drops the Rust engine instance, freeing all memory and threads.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Void>)>(
    symbol: 'bridge_shutdown')
external void bridgeShutdown(ffi.Pointer<ffi.Void> instance);

// ── Operation execution ─────────────────────────────────────────

/// Execute a binary-encoded operation on the instance's thread pool.
///
/// Sources (indexed 0..sourceCount-1): parallel arrays of
///   [sourceBufs] condvar buffer pointers,
///   [sourceNotifyFns] NativeCallable listener pointers,
///   [sourceLengths] byte counts.
///
/// Sinks (indexed 0..sinkCount-1): parallel arrays of
///   [sinkBufs] condvar buffer pointers,
///   [sinkNotifyFns] NativeCallable listener pointers.
@ffi.Native<ffi.Void Function(
    ffi.Pointer<ffi.Void>,
    ffi.Pointer<ffi.Uint8>, ffi.Int32,
    ffi.Int32,
    ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
    ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>,
    ffi.Pointer<ffi.Int64>,
    ffi.Int32,
    ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
    ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>,
    ffi.Int64)>(symbol: 'bridge_execute')
external void bridgeExecute(
  ffi.Pointer<ffi.Void> instance,
  ffi.Pointer<ffi.Uint8> requestPtr,
  int requestLen,
  int sourceCount,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> sourceBufs,
  ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>> sourceNotifyFns,
  ffi.Pointer<ffi.Int64> sourceLengths,
  int sinkCount,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> sinkBufs,
  ffi.Pointer<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>> sinkNotifyFns,
  int resultPort,
);

// ── Condvar buffer management ───────────────────────────────────

/// Returns the size in bytes required for a read condvar buffer.
@ffi.Native<ffi.Int32 Function()>(symbol: 'bridge_read_buffer_size')
external int bridgeReadBufferSize();

/// Returns the size in bytes required for a write condvar buffer.
@ffi.Native<ffi.Int32 Function()>(symbol: 'bridge_write_buffer_size')
external int bridgeWriteBufferSize();

/// Initialises the mutex and condvar inside a read buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_init_read_buffer')
external void bridgeInitReadBuffer(ffi.Pointer<ffi.Uint8> buf);

/// Initialises the mutex and condvar inside a write buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_init_write_buffer')
external void bridgeInitWriteBuffer(ffi.Pointer<ffi.Uint8> buf);

/// Destroys the mutex and condvar inside a read buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_destroy_read_buffer')
external void bridgeDestroyReadBuffer(ffi.Pointer<ffi.Uint8> buf);

/// Destroys the mutex and condvar inside a write buffer.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_destroy_write_buffer')
external void bridgeDestroyWriteBuffer(ffi.Pointer<ffi.Uint8> buf);

/// Signals the Rust side that a read response is ready.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_signal_read')
external void bridgeSignalRead(ffi.Pointer<ffi.Uint8> buf);

/// Signals the Rust side that a write ack is ready.
@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(
    symbol: 'bridge_signal_write')
external void bridgeSignalWrite(ffi.Pointer<ffi.Uint8> buf);
