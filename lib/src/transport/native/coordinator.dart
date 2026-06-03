// Native coordinator — Dart isolate that owns one Rust engine instance.
//
// Each Pdf() creates one coordinator isolate. The coordinator:
//   1. Calls bridgeInit() → gets an opaque instance pointer
//   2. Routes every op through bridgeExecute(instance, ...)
//   3. On shutdown: calls bridgeShutdown(instance) → all memory freed
//
// Multi-source/multi-sink: bridgeExecute receives arrays of condvar
// channels. Each source gets its own shared buffer + notify callback.
// Each sink gets its own shared buffer + notify callback. Any count.
//
// Handle-aware: when keepSources is non-empty, the specified source
// channels are kept alive for the handle's lifetime.
//
// INTERNAL — spawned by NativeTransport only.

import 'dart:ffi' as ffi;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:pdf_manipulator/src/transport/native/bindings.dart' as bridge;
import 'package:pdf_manipulator/src/transport/native/shared_buffer.dart';

// ── Condvar channel constants (must match Rust shared_buffer.rs) ────

const _flagReady = 1 << 0;
const _flagError = 1 << 1;
const _flagAck = 1 << 3;

// ── Per-source resources (kept alive until dispose) ──────────────

class _SourceResources {
  final SharedReadBuffer readBuf;
  final ffi.NativeCallable<ffi.Void Function()> callable;

  _SourceResources(this.readBuf, this.callable);

  void dispose() {
    callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    calloc.free(readBuf.ptr);
  }
}

final _heldResources = <int, _SourceResources>{};
var _nextResourceId = 1;

// ── Coordinator isolate entry point ─────────────────────────────────

void coordinatorEntryPoint(SendPort mainPort) {
  final port = ReceivePort();
  mainPort.send(port.sendPort);

  SendPort? responsePort;

  bridge.storeDartPostCobject(ffi.NativeApi.postCObject.cast());
  final instance = bridge.bridgeInit();

  port.listen((message) async {
    if (message is SendPort) {
      responsePort = message;
      return;
    }
    if (message is! List) return;

    final tag = message[1] as String;

    if (tag == 'shutdown') {
      for (final res in _heldResources.values) {
        res.dispose();
      }
      _heldResources.clear();
      bridge.bridgeShutdown(instance);
      responsePort?.send([message[0] as int, 'result', Uint8List(0)]);
      return;
    }

    if (tag == 'releaseSource') {
      final resourceId = message[2] as int;
      _heldResources.remove(resourceId)?.dispose();
      responsePort?.send([message[0] as int, 'result', Uint8List(0)]);
      return;
    }

    final id = message[0] as int;
    final requestBytes = message[2] as Uint8List;
    final sourcePorts = message[3] as List<SendPort?>;
    final sourceLengths = message[4] as List<int>;
    final sinkPorts = message[5] as List<SendPort?>;
    final keepSources = message[6] as Set<int>;

    try {
      final result = await _executeOp(
        instance, requestBytes, sourcePorts, sourceLengths, sinkPorts, keepSources,
      );
      responsePort?.send([id, 'result', result.bytes]);
      if (result.keptResourceIds.isNotEmpty) {
        responsePort?.send([id, 'resourceIds', result.keptResourceIds]);
      }
    } catch (e) {
      responsePort?.send([id, 'error', e.toString()]);
    }
  });
}

// ── Result with optional resource IDs ──────────────────────────────

class _OpResult {
  final Uint8List bytes;
  final Map<int, int> keptResourceIds; // sourceIndex → resourceId
  _OpResult(this.bytes, {this.keptResourceIds = const {}});
}

// ── Execute one operation via bridge_execute FFI ────────────────────

Future<_OpResult> _executeOp(
  ffi.Pointer<ffi.Void> instance,
  Uint8List requestBytes,
  List<SendPort?> sourcePorts,
  List<int> sourceLengths,
  List<SendPort?> sinkPorts,
  Set<int> keepSources,
) async {
  final requestPtr = calloc<ffi.Uint8>(requestBytes.length);
  requestPtr.asTypedList(requestBytes.length).setAll(0, requestBytes);

  final sourceCount = sourcePorts.length;
  final sinkCount = sinkPorts.length;

  // Allocate source arrays
  final srcBufsPtr = calloc<ffi.Pointer<ffi.Uint8>>(sourceCount);
  final srcNotifyPtr = calloc<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>(sourceCount);
  final srcLengthsPtr = calloc<ffi.Int64>(sourceCount);
  final srcReadBufs = <int, SharedReadBuffer>{};
  final srcCallables = <int, ffi.NativeCallable<ffi.Void Function()>>{};

  for (var i = 0; i < sourceCount; i++) {
    if (sourcePorts[i] != null) {
      final readBuf = SharedReadBuffer();
      bridge.bridgeInitReadBuffer(readBuf.rawPtr);
      final callable = _createReadListener(readBuf, sourcePorts[i]!);
      srcReadBufs[i] = readBuf;
      srcCallables[i] = callable.callable;
      srcBufsPtr[i] = readBuf.rawPtr;
      srcNotifyPtr[i] = callable.ptr;
      srcLengthsPtr[i] = sourceLengths[i];
    } else {
      srcBufsPtr[i] = ffi.nullptr;
      srcNotifyPtr[i] = ffi.nullptr;
      srcLengthsPtr[i] = 0;
    }
  }

  // Allocate sink arrays
  final snkBufsPtr = calloc<ffi.Pointer<ffi.Uint8>>(sinkCount);
  final snkNotifyPtr = calloc<ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>>(sinkCount);
  final snkWriteBufs = <int, SharedWriteBuffer>{};
  final snkCallables = <int, ffi.NativeCallable<ffi.Void Function()>>{};

  for (var i = 0; i < sinkCount; i++) {
    if (sinkPorts[i] != null) {
      final writeBuf = SharedWriteBuffer();
      bridge.bridgeInitWriteBuffer(writeBuf.rawPtr);
      final callable = _createWriteListener(writeBuf, sinkPorts[i]!);
      snkWriteBufs[i] = writeBuf;
      snkCallables[i] = callable.callable;
      snkBufsPtr[i] = writeBuf.rawPtr;
      snkNotifyPtr[i] = callable.ptr;
    } else {
      snkBufsPtr[i] = ffi.nullptr;
      snkNotifyPtr[i] = ffi.nullptr;
    }
  }

  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  try {
    bridge.bridgeExecute(
      instance,
      requestPtr, requestBytes.length,
      sourceCount,
      srcBufsPtr, srcNotifyPtr, srcLengthsPtr,
      sinkCount,
      snkBufsPtr, snkNotifyPtr,
      nativePort,
    );

    final result = await resultPort.first;

    // Keep specified source channels alive for handle lifetime
    final keptResourceIds = <int, int>{};
    for (final idx in keepSources) {
      if (srcReadBufs.containsKey(idx) && srcCallables.containsKey(idx)) {
        final resourceId = _nextResourceId++;
        _heldResources[resourceId] = _SourceResources(
          srcReadBufs[idx]!, srcCallables[idx]!,
        );
        keptResourceIds[idx] = resourceId;
      }
    }

    return _OpResult(result as Uint8List, keptResourceIds: keptResourceIds);
  } finally {
    calloc.free(requestPtr);
    resultPort.close();

    // Free source channels NOT kept for a handle
    for (var i = 0; i < sourceCount; i++) {
      if (keepSources.contains(i) && srcReadBufs.containsKey(i)) continue;
      srcCallables[i]?.close();
      if (srcReadBufs.containsKey(i)) {
        bridge.bridgeDestroyReadBuffer(srcReadBufs[i]!.rawPtr);
        calloc.free(srcReadBufs[i]!.ptr);
      }
    }

    // Free all sink channels (never kept)
    for (var i = 0; i < sinkCount; i++) {
      snkCallables[i]?.close();
      if (snkWriteBufs.containsKey(i)) {
        bridge.bridgeDestroyWriteBuffer(snkWriteBufs[i]!.rawPtr);
        calloc.free(snkWriteBufs[i]!.ptr);
      }
    }

    // Free the arrays themselves
    calloc.free(srcBufsPtr);
    calloc.free(srcNotifyPtr);
    calloc.free(srcLengthsPtr);
    calloc.free(snkBufsPtr);
    calloc.free(snkNotifyPtr);
  }
}

// ── Condvar read listener ───────────────────────────────────────────

({ffi.NativeCallable<ffi.Void Function()> callable,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> ptr})
_createReadListener(SharedReadBuffer readBuf, SendPort sourcePort) {
  late final ffi.NativeCallable<ffi.Void Function()> callable;
  callable = ffi.NativeCallable<ffi.Void Function()>.listener(() async {
    final offset = readBuf.requestOffset;
    final count = readBuf.requestCount;

    final replyPort = ReceivePort();
    sourcePort.send(['read', offset, count, replyPort.sendPort]);
    final response = await replyPort.first;
    replyPort.close();

    if (response is TransferableTypedData) {
      final bytes = response.materialize().asUint8List();
      readBuf.responseLength = bytes.length;
      readBuf.writeResponseData(bytes);
      readBuf.setFlags(_flagReady);
    } else {
      readBuf.responseLength = 0;
      readBuf.setFlags(_flagError);
    }
    bridge.bridgeSignalRead(readBuf.rawPtr);
  });
  return (callable: callable, ptr: callable.nativeFunction);
}

// ── Condvar write listener ──────────────────────────────────────────

({ffi.NativeCallable<ffi.Void Function()> callable,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> ptr})
_createWriteListener(SharedWriteBuffer writeBuf, SendPort sinkPort) {
  late final ffi.NativeCallable<ffi.Void Function()> callable;
  callable = ffi.NativeCallable<ffi.Void Function()>.listener(() async {
    final length = writeBuf.chunkLength;
    final chunk = writeBuf.readChunkData(length);

    final replyPort = ReceivePort();
    sinkPort.send([
      'write',
      TransferableTypedData.fromList([chunk]),
      replyPort.sendPort,
    ]);
    final ack = await replyPort.first;
    replyPort.close();

    if (ack == true) {
      writeBuf.setFlags(_flagAck);
    } else {
      writeBuf.setFlags(_flagError);
    }
    bridge.bridgeSignalWrite(writeBuf.rawPtr);
  });
  return (callable: callable, ptr: callable.nativeFunction);
}
