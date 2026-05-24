// Worker isolate for the native bridge.
//
// Runs on a background isolate. Receives WorkerMsg from the main isolate.
// Dispatches operations to Rust via FFI (bridge_submit_*).
// Fulfills read requests from the SourceServer via shared buffer.
// Forwards write chunks to the SinkServer via shared buffer.
//
// INTERNAL — used by NativeBridge only.

import 'dart:ffi' as ffi;
import 'dart:isolate';
import 'package:pdf_manipulator/src/transport/native/bindings.dart' as bridge;
import 'package:pdf_manipulator/src/transport/native/shared_buffer.dart';

void initBridge() {
  bridge.storeDartPostCobject(ffi.NativeApi.postCObject.cast());
  bridge.bridgeInit();
}

///
/// When a Rust pool thread needs bytes, it calls this function pointer.
/// The listener fires on the worker isolate's event loop. The listener
/// reads the request from the shared buffer, sends it to the main
/// isolate's SourceServer, gets bytes back, writes them to the shared
/// buffer, and signals the condvar.
///
({ffi.NativeCallable<ffi.Void Function()> callable, ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> ptr})
createReadListener(
  SharedReadBuffer readBuf,
  SendPort sourceServerPort,
) {
  late final ffi.NativeCallable<ffi.Void Function()> callable;

  callable = ffi.NativeCallable<ffi.Void Function()>.listener(() async {
    // Read request from shared buffer (written by Rust pool thread)
    final offset = readBuf.requestOffset;
    final count = readBuf.requestCount;

    // Ask main isolate's SourceServer for the bytes
    final replyPort = ReceivePort();
    sourceServerPort.send(['read', offset, count, replyPort.sendPort]);

    final response = await replyPort.first;
    replyPort.close();

    if (response is TransferableTypedData) {
      final bytes = response.materialize().asUint8List();
      readBuf.responseLength = bytes.length;
      readBuf.writeResponseData(bytes);
      readBuf.setFlags(flagReady);
    } else {
      readBuf.responseLength = 0;
      readBuf.setFlags(flagError);
    }

    // Signal the condvar (wake the sleeping pool thread)
    bridge.bridgeSignalRead(readBuf.rawPtr);
  });

  return (callable: callable, ptr: callable.nativeFunction);
}

///
/// When a Rust pool thread produces output, it calls this function pointer.
/// The listener reads the chunk from the shared buffer, sends it to the
/// main isolate's SinkServer, gets ack, and signals the condvar.
({ffi.NativeCallable<ffi.Void Function()> callable, ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> ptr})
createWriteListener(
  SharedWriteBuffer writeBuf,
  SendPort sinkServerPort,
) {
  late final ffi.NativeCallable<ffi.Void Function()> callable;

  callable = ffi.NativeCallable<ffi.Void Function()>.listener(() async {
    // Read chunk from shared buffer (written by Rust pool thread)
    final length = writeBuf.chunkLength;
    final chunk = writeBuf.readChunkData(length);

    // Send to main isolate's SinkServer
    final replyPort = ReceivePort();
    sinkServerPort.send([
      'write',
      TransferableTypedData.fromList([chunk]),
      replyPort.sendPort,
    ]);

    final ack = await replyPort.first;
    replyPort.close();

    if (ack == true) {
      writeBuf.setFlags(flagAck);
    } else {
      writeBuf.setFlags(flagError);
    }

    // Signal the condvar (wake the sleeping pool thread)
    bridge.bridgeSignalWrite(writeBuf.rawPtr);
  });

  return (callable: callable, ptr: callable.nativeFunction);
}

/// Cancel all operations and clean up shared buffers.
void cancelAndCleanup(
  List<SharedReadBuffer> readBuffers,
  List<SharedWriteBuffer> writeBuffers,
  List<ffi.NativeCallable> callables,
) {
  bridge.bridgeCancelAll();

  // Signal all condvars so sleeping threads wake up and see cancellation
  for (final buf in readBuffers) {
    buf.setFlags(flagCancelled);
    bridge.bridgeSignalRead(buf.rawPtr);
  }
  for (final buf in writeBuffers) {
    buf.setFlags(flagCancelled);
    bridge.bridgeSignalWrite(buf.rawPtr);
  }

  // Wait briefly for cooperative exit, then clean up
  Future.delayed(const Duration(milliseconds: 100), () {
    for (final c in callables) {
      c.close();
    }
    for (final buf in readBuffers) {
      bridge.bridgeDestroyReadBuffer(buf.rawPtr);
      buf.dispose();
    }
    for (final buf in writeBuffers) {
      bridge.bridgeDestroyWriteBuffer(buf.rawPtr);
      buf.dispose();
    }
  });
}
