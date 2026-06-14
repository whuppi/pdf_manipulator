// Mirrors the shared channel-buffer layout from Rust shared_buffer.rs.
//
// Both sides (Dart + Rust) agree on byte offsets. Any change here MUST
// be reflected in the Rust side and vice versa.
//
// Cancellation contract: FLAG_CANCELLED is STICKY — Rust clears only
// the response bits between requests. A held channel collaterally
// flagged by a previous job's cancel is revived via [clearCancelled]
// by the lane, strictly between jobs (the only moment nothing can be
// parked in it).
//
// INTERNAL — used by the native lane only.

import 'dart:ffi' as ffi;
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

// ── Flag bits (must match Rust FLAG_* constants) ────────────────────

/// Indicates a response/chunk is ready.
const flagReady = 1 << 0;

/// Indicates an error occurred.
const flagError = 1 << 1;

/// Indicates the operation was cancelled. Sticky — see header.
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

/// Total allocation size for a read channel buffer.
const readBufferSize = _readData + _readDataCapacity;

// ── Write channel offsets ───────────────────────────────────────────

const _writeChunkLength = 0;
const _writeFlags = 8;
const _writeData = 144;
const _writeDataCapacity = 256 * 1024;

/// Total allocation size for a write channel buffer.
const writeBufferSize = _writeData + _writeDataCapacity;

// ── Pointer helpers ─────────────────────────────────────────────────

extension _PtrOps on ffi.Pointer<ffi.Uint8> {
  int readI64At(int offset) => (this + offset).cast<ffi.Int64>().value;

  void writeI64At(int offset, int value) =>
      (this + offset).cast<ffi.Int64>().value = value;

  int readU32At(int offset) => (this + offset).cast<ffi.Uint32>().value;

  void writeU32At(int offset, int value) =>
      (this + offset).cast<ffi.Uint32>().value = value;

  ffi.Pointer<ffi.Uint8> dataAt(int offset) => this + offset;
}

// ── ReadChannelBuffer ───────────────────────────────────────────────

/// Dart-side view of a read channel buffer (engine pulls source bytes).
class ReadChannelBuffer {
  /// Allocates a new read channel buffer.
  ReadChannelBuffer() : ptr = calloc<ffi.Uint8>(readBufferSize);

  /// Raw pointer to the allocated buffer.
  final ffi.Pointer<ffi.Uint8> ptr;

  /// Byte offset requested by the engine.
  int get requestOffset => ptr.readI64At(_readRequestOffset);

  /// Byte count requested by the engine.
  int get requestCount => ptr.readI64At(_readRequestCount);

  /// Sets the response length before signalling.
  set responseLength(int value) => ptr.writeI64At(_readResponseLength, value);

  /// Copies [bytes] into the data section of the buffer.
  void writeResponseData(Uint8List bytes) {
    ptr.dataAt(_readData).asTypedList(bytes.length).setAll(0, bytes);
  }

  /// Sets the specified flag [bits] on the buffer.
  void setFlags(int bits) =>
      ptr.writeU32At(_readFlags, ptr.readU32At(_readFlags) | bits);

  /// Revives a held channel: clears the sticky cancelled flag. Call
  /// ONLY between jobs — never while a lane thread can be parked here.
  void clearCancelled() =>
      ptr.writeU32At(_readFlags, ptr.readU32At(_readFlags) & ~flagCancelled);

  /// Frees the buffer memory. Caller guarantees the engine can no
  /// longer touch it (its job posted, or the channel was released).
  void free() => calloc.free(ptr);
}

// ── WriteChannelBuffer ──────────────────────────────────────────────

/// Dart-side view of a write channel buffer (engine pushes output).
class WriteChannelBuffer {
  /// Allocates a new write channel buffer.
  WriteChannelBuffer() : ptr = calloc<ffi.Uint8>(writeBufferSize);

  /// Raw pointer to the allocated buffer.
  final ffi.Pointer<ffi.Uint8> ptr;

  /// Length of the chunk written by the engine.
  int get chunkLength => ptr.readI64At(_writeChunkLength);

  /// Reads [length] bytes of chunk data from the buffer.
  Uint8List readChunkData(int length) =>
      Uint8List.fromList(ptr.dataAt(_writeData).asTypedList(length));

  /// Sets the specified flag [bits] on the buffer.
  void setFlags(int bits) =>
      ptr.writeU32At(_writeFlags, ptr.readU32At(_writeFlags) | bits);

  /// Frees the buffer memory. Caller guarantees the engine can no
  /// longer touch it (its job posted).
  void free() => calloc.free(ptr);
}
