// Cross-isolate source server: PdfSource on main, worker reads on demand.
//
// SourceServer (main isolate) listens for read/length requests from
// the worker and fulfills them from the consumer's PdfSource.
//
// INTERNAL — used by the native platform implementation only.

import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_source.dart';

/// Runs on the MAIN isolate. Fulfills read/length requests from the worker.
///
/// Messages are lists (not custom classes — Dart isolates can only
/// serialize primitives, SendPort, TransferableTypedData, and collections):
/// - ['read', offset, count, replyPort]
/// - ['length', replyPort]
class SourceServer {
  SourceServer(this._source);

  final PdfSource _source;
  ReceivePort? _port;

  SendPort start() {
    _port = ReceivePort();
    _port!.listen((message) async {
      if (message is! List) return;
      final type = message[0] as String;
      if (type == 'read') {
        final offset = message[1] as int;
        final count = message[2] as int;
        final replyPort = message[3] as SendPort;
        try {
          final bytes = await _source.readAt(offset, count);
          replyPort.send(TransferableTypedData.fromList([bytes]));
        } catch (_) {
          replyPort.send(TransferableTypedData.fromList([Uint8List(0)]));
        }
      } else if (type == 'length') {
        final replyPort = message[1] as SendPort;
        replyPort.send(_source.length);
      }
    });
    return _port!.sendPort;
  }

  void stop() {
    _port?.close();
    _port = null;
  }
}
