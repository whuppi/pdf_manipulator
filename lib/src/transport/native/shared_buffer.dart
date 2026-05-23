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

const flagReady = 1 << 0;
const flagError = 1 << 1;
const flagCancelled = 1 << 2;
const flagAck = 1 << 3;

// ── Read channel offsets ────────────────────────────────────────────

const _readRequestOffset = 0;
const _readRequestCount = 8;
const _readResponseLength = 16;
const _readFlags = 24;
const _readData = 160;
const _readDataCapacity = 64 * 1024;
const readBufferSize = _readData + _readDataCapacity;

// ── Write channel offsets ───────────────────────────────────────────

const _writeChunkLength = 0;
const _writeFlags = 8;
const _writeData = 144;
const _writeDataCapacity = 256 * 1024;
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

class SharedReadBuffer {
  SharedReadBuffer() : ptr = calloc<ffi.Uint8>(readBufferSize);

  final ffi.Pointer<ffi.Uint8> ptr;

  ffi.Pointer<ffi.Uint8> get rawPtr => ptr;

  int get requestOffset => ptr.readI64At(_readRequestOffset);
  int get requestCount => ptr.readI64At(_readRequestCount);

  set responseLength(int value) =>
      ptr.writeI64At(_readResponseLength, value);

  void writeResponseData(Uint8List bytes) {
    final dst = ptr.dataAt(_readData);
    dst.asTypedList(bytes.length).setAll(0, bytes);
  }

  void setFlags(int bits) =>
      ptr.writeU32At(_readFlags, ptr.readU32At(_readFlags) | bits);

  void clearFlags() => ptr.writeU32At(_readFlags, 0);

  bool hasFlag(int bit) => (ptr.readU32At(_readFlags) & bit) != 0;

  void dispose() => calloc.free(ptr);
}

// ── SharedWriteBuffer ───────────────────────────────────────────────

class SharedWriteBuffer {
  SharedWriteBuffer() : ptr = calloc<ffi.Uint8>(writeBufferSize);

  final ffi.Pointer<ffi.Uint8> ptr;

  ffi.Pointer<ffi.Uint8> get rawPtr => ptr;

  int get chunkLength => ptr.readI64At(_writeChunkLength);

  Uint8List readChunkData(int length) {
    final src = ptr.dataAt(_writeData);
    return Uint8List.fromList(src.asTypedList(length));
  }

  void setFlags(int bits) =>
      ptr.writeU32At(_writeFlags, ptr.readU32At(_writeFlags) | bits);

  void clearFlags() => ptr.writeU32At(_writeFlags, 0);

  bool hasFlag(int bit) => (ptr.readU32At(_writeFlags) & bit) != 0;

  void dispose() => calloc.free(ptr);
}
