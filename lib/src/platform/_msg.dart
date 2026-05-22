// Typed messages for the native isolate protocol.
// No closures. Bytes travel as TransferableTypedData (one memcpy + O(1) transfer).

import 'dart:isolate';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/platform/_op.dart';

/// Request from main isolate → worker isolate.
///
/// [bytes] and [bytesList] use TransferableTypedData for zero-copy.
/// [args] carries all other parameters as a sendable map — no numbered
/// slots, no running-out-of-fields, self-documenting at the call site.
class WorkerMsg {
  final int id;
  final Op op;

  // Transferable byte payloads
  final TransferableTypedData? bytes;
  final List<TransferableTypedData>? bytesList;

  /// SendPort to a SourceServer on the main isolate (streaming input).
  final SendPort? sourcePort;

  /// SendPort to a SinkServer on the main isolate (streaming output).
  final SendPort? sinkPort;

  // Named args — any sendable primitive (int, double, bool, String, List, Map)
  final Map<String, Object?> args;

  const WorkerMsg({
    required this.id,
    required this.op,
    this.bytes,
    this.bytesList,
    this.sourcePort,
    this.sinkPort,
    this.args = const {},
  });
}

/// Response from worker isolate → main isolate.
class WorkerResult {
  final int id;
  final Object? value;
  final Object? error;
  const WorkerResult({required this.id, this.value, this.error});
}

/// Streaming response — one item at a time. Sent for ops that yield
/// multiple results (extractImages, renderAllPages). The final item
/// has [done] = true and no [value].
class WorkerStreamItem {
  final int id;
  final Object? value;
  final Object? error;
  final bool done;
  const WorkerStreamItem({
    required this.id,
    this.value,
    this.error,
    this.done = false,
  });
}

/// Wrap Uint8List for zero-copy transfer.
TransferableTypedData transfer(Uint8List bytes) =>
    TransferableTypedData.fromList([bytes]);

/// Wrap multiple Uint8Lists.
List<TransferableTypedData> transferList(List<Uint8List> list) =>
    list.map((b) => TransferableTypedData.fromList([b])).toList();

/// Materialize received TransferableTypedData back to Uint8List.
Uint8List materialize(TransferableTypedData t) =>
    t.materialize().asUint8List();

/// Materialize a list.
List<Uint8List> materializeList(List<TransferableTypedData> list) =>
    list.map((t) => t.materialize().asUint8List()).toList();
