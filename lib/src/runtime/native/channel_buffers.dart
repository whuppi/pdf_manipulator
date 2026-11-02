// Mirrors the shared channel-buffer layout from Rust shared_buffer.rs.
//
// Both sides (Dart + Rust) agree on byte offsets. Any change here MUST
// be reflected in the Rust side and vice versa.
//
// Every channel carries three u32 words, each with ONE writer:
//
//   request word  — written only by Rust: a number bumped on every
//                   request the channel carries.
//   response word — written only here. Low nibble: the kind of the
//                   response (ready / error / ack); upper bits: the
//                   request number being answered, echoed. Rust takes a
//                   response only when it echoes the request it waits
//                   on. Before answering, a listener reads the request
//                   word again after its await and drops its answer
//                   when the request has moved on or the buffer is freed.
//   cancel word   — written only by Rust. FLAG_CANCELLED is STICKY; a
//                   held channel collaterally flagged by a finished
//                   job's cancel is revived by Rust when the next job
//                   adopts it. Dart only reads it.
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

/// Cancel-word bit: the operation was cancelled. Sticky — see header.
const flagCancelled = 1 << 2;

/// Indicates the consumer acknowledged the chunk.
const flagAck = 1 << 3;

// ── Read channel offsets ────────────────────────────────────────────

const _readRequestOffset = 0;
const _readRequestCount = 8;
const _readResponseLength = 16;
const _readResponse = 24;
const _readCancel = 28;
const _readRequestSeq = 40;
const _readData = 160;
const _readDataCapacity = 64 * 1024;

/// Offset of the Rust-written request-number word of a read channel.
const readRequestSeqOffset = _readRequestSeq;

/// Offset of the Dart-written response word of a read channel.
const readResponseOffset = _readResponse;

/// Offset of the Rust-written cancel word of a read channel.
const readCancelOffset = _readCancel;

/// Total allocation size for a read channel buffer.
const readBufferSize = _readData + _readDataCapacity;

// ── Write channel offsets ───────────────────────────────────────────

const _writeChunkLength = 0;
const _writeResponse = 8;
const _writeCancel = 12;
const _writeRequestSeq = 24;
const _writeData = 144;
const _writeDataCapacity = 256 * 1024;

/// Offset of the Rust-written request-number word of a write channel.
const writeRequestSeqOffset = _writeRequestSeq;

/// Offset of the Dart-written response word of a write channel.
const writeResponseOffset = _writeResponse;

/// Offset of the Rust-written cancel word of a write channel.
const writeCancelOffset = _writeCancel;

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

  /// Publishes a response of [kind] to request number [request] in one
  /// store.
  void respond(int responseOffset, int request, int kind) {
    writeU32At(responseOffset, ((request << 4) | kind) & 0xFFFFFFFF);
  }
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

  /// The number of the request the engine posted last.
  int get requestSeq => ptr.readU32At(_readRequestSeq);

  /// Answers request number [request] with [kind] (ready or error).
  void respond(int request, int kind) =>
      ptr.respond(_readResponse, request, kind);

  /// Whether Rust has flagged this channel cancelled.
  bool get cancelled => ptr.readU32At(_readCancel) & flagCancelled != 0;

  /// True once [free] ran. A read the source was still serving when its
  /// channel was freed (an abandoned or cancelled job) must not touch
  /// the buffer when it finally resolves.
  bool get freed => _freed;
  bool _freed = false;

  /// Frees the buffer memory. Caller guarantees the engine can no
  /// longer touch it (its job posted, or the channel was released).
  void free() {
    _freed = true;
    calloc.free(ptr);
  }
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

  /// The number of the request the engine posted last.
  int get requestSeq => ptr.readU32At(_writeRequestSeq);

  /// Answers request number [request] with [kind] (ack or error).
  void respond(int request, int kind) =>
      ptr.respond(_writeResponse, request, kind);

  /// Whether Rust has flagged this channel cancelled.
  bool get cancelled => ptr.readU32At(_writeCancel) & flagCancelled != 0;

  /// Sink writes that resolved after the buffer was freed and so touched
  /// nothing. A guard check for tests; never read in production.
  static int writesAfterFree = 0;

  /// True once [free] ran. A write the sink was still serving when its
  /// channel was freed (a cancelled job whose sink had not finished) must
  /// not touch the buffer when it finally resolves.
  bool get freed => _freed;
  bool _freed = false;

  /// Frees the buffer memory. Caller guarantees the engine can no
  /// longer touch it (its job posted).
  void free() {
    _freed = true;
    calloc.free(ptr);
  }
}
