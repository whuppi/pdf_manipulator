// Mirrors the shared buffer layout from Rust bridge/shared_buffer.rs.
//
// Both sides (Dart + Rust) agree on byte offsets. Any change here MUST
// be reflected in the Rust side and vice versa.
//
// INTERNAL — used by the native bridge only.

import 'dart:ffi' as ffi;
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

// ── Flag bits (must match Rust FLAG_* constants) ────────────────────

/// Indicates a response/chunk is ready.
const flagReady = 1 << 0;

/// Indicates an error occurred.
const flagError = 1 << 1;

/// Indicates the operation was cancelled.
const flagCancelled = 1 << 2;

/// Indicates the consumer acknowledged the chunk.
const flagAck = 1 << 3;

// ── Read channel offsets ────────────────────────────────────────────

const _readRequestOffset = 0;
const _readRequestCount = 8;
const _readResponseLength = 16;
const _readFlags = 24;
const _readData = 160;
const _readDataCapacity = 64 * 1024;

/// Total allocation size for a read condvar buffer.
const readBufferSize = _readData + _readDataCapacity;

// ── Write channel offsets ───────────────────────────────────────────

const _writeChunkLength = 0;
const _writeFlags = 8;
const _writeData = 144;
const _writeDataCapacity = 256 * 1024;

/// Total allocation size for a write condvar buffer.
const writeBufferSize = _writeData + _writeDataCapacity;

// ── Pointer helpers ─────────────────────────────────────────────────

extension _PtrOps on ffi.Pointer<ffi.Uint8> {
  int readI64At(int offset) {
    final p = (this + offset).cast<ffi.Int64>();
    return p.value;
  }

  void writeI64At(int offset, int value) {
    final p = (this + offset).cast<ffi.Int64>();
    p.value = value;
  }

  int readU32At(int offset) {
    final p = (this + offset).cast<ffi.Uint32>();
    return p.value;
  }

  void writeU32At(int offset, int value) {
    final p = (this + offset).cast<ffi.Uint32>();
    p.value = value;
  }

  ffi.Pointer<ffi.Uint8> dataAt(int offset) => this + offset;
}

// ── SharedReadBuffer ────────────────────────────────────────────────

/// Dart-side view of a Rust read condvar buffer.
class SharedReadBuffer {
  /// Allocates and returns a new read buffer.
  SharedReadBuffer() : ptr = calloc<ffi.Uint8>(readBufferSize);

  /// Raw pointer to the allocated condvar buffer.
  final ffi.Pointer<ffi.Uint8> ptr;

  /// Alias for [ptr] used by FFI init/destroy calls.
  ffi.Pointer<ffi.Uint8> get rawPtr => ptr;

  /// Byte offset requested by the Rust side.
  int get requestOffset => ptr.readI64At(_readRequestOffset);

  /// Byte count requested by the Rust side.
  int get requestCount => ptr.readI64At(_readRequestCount);

  /// Sets the response length before signalling.
  set responseLength(int value) =>
      ptr.writeI64At(_readResponseLength, value);

  /// Copies [bytes] into the data section of the buffer.
  void writeResponseData(Uint8List bytes) {
    final dst = ptr.dataAt(_readData);
    dst.asTypedList(bytes.length).setAll(0, bytes);
  }

  /// Sets the specified flag [bits] on the buffer.
  void setFlags(int bits) =>
      ptr.writeU32At(_readFlags, ptr.readU32At(_readFlags) | bits);

  /// Clears all flags to zero.
  void clearFlags() => ptr.writeU32At(_readFlags, 0);

  /// Returns whether the given flag [bit] is set.
  bool hasFlag(int bit) => (ptr.readU32At(_readFlags) & bit) != 0;

  /// Frees the allocated buffer memory.
  void dispose() => calloc.free(ptr);
}

// ── SharedWriteBuffer ───────────────────────────────────────────────

/// Dart-side view of a Rust write condvar buffer.
class SharedWriteBuffer {
  /// Allocates and returns a new write buffer.
  SharedWriteBuffer() : ptr = calloc<ffi.Uint8>(writeBufferSize);

  /// Raw pointer to the allocated condvar buffer.
  final ffi.Pointer<ffi.Uint8> ptr;

  /// Alias for [ptr] used by FFI init/destroy calls.
  ffi.Pointer<ffi.Uint8> get rawPtr => ptr;

  /// Length of the chunk written by the Rust side.
  int get chunkLength => ptr.readI64At(_writeChunkLength);

  /// Reads [length] bytes of chunk data from the buffer.
  Uint8List readChunkData(int length) {
    final src = ptr.dataAt(_writeData);
    return Uint8List.fromList(src.asTypedList(length));
  }

  /// Sets the specified flag [bits] on the buffer.
  void setFlags(int bits) =>
      ptr.writeU32At(_writeFlags, ptr.readU32At(_writeFlags) | bits);

  /// Clears all flags to zero.
  void clearFlags() => ptr.writeU32At(_writeFlags, 0);

  /// Returns whether the given flag [bit] is set.
  bool hasFlag(int bit) => (ptr.readU32At(_writeFlags) & bit) != 0;

  /// Frees the allocated buffer memory.
  void dispose() => calloc.free(ptr);
}
