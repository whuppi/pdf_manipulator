// Cross-isolate sink server: PdfSink on main, worker pushes chunks.
//
// SinkServer (main isolate) receives write chunks from the worker
// and pushes them to the consumer's PdfSink.
//
// INTERNAL — used by the native platform implementation only.

import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/api/pdf_sink.dart';

/// Runs on the MAIN isolate. Receives write chunks from the worker.
///
/// Messages are lists:
/// - ['write', TransferableTypedData, replyPort]
class SinkServer {
  SinkServer(this._sink);

  final PdfSink _sink;
  ReceivePort? _port;

  SendPort start() {
    _port = ReceivePort();
    _port!.listen((message) async {
      if (message is! List || message[0] != 'write') return;
      final chunk = message[1] as TransferableTypedData;
      final replyPort = message[2] as SendPort;
      try {
        final bytes = chunk.materialize().asUint8List();
        await _sink.write(bytes);
        replyPort.send(true);
      } catch (_) {
        replyPort.send(false);
      }
    });
    return _port!.sendPort;
  }

  void stop() {
    _port?.close();
    _port = null;
  }
}

/// Push [bytes] to a SinkServer via the write protocol.
///
/// Used by dispatch ops where the Rust API returns bytes (sign,
/// extractPages, builder build — non-streamable by design).
Future<void> writeBytesToSink(SendPort sinkPort, Uint8List bytes) async {
  final replyPort = ReceivePort();
  sinkPort.send([
    'write',
    TransferableTypedData.fromList([bytes]),
    replyPort.sendPort,
  ]);
  await replyPort.first;
}
