import 'dart:async';
import 'dart:typed_data';

/// Sequential writer for PDF output data.
///
/// Consumers implement this to receive PDF bytes into any backing store
/// (file, network, database, memory). The engine writes chunks as it
/// produces them — never accumulates the full output in memory.
abstract interface class PdfSink {
  /// Write a chunk of output bytes. Called sequentially, never concurrently.
  FutureOr<void> write(Uint8List chunk);
}
