import 'dart:async';
import 'dart:typed_data';

/// Sequential writer for PDF output data.
///
/// Consumers implement this to receive PDF bytes into any backing store
/// (file, network, database, memory). The engine writes chunks as it
/// produces them — never accumulates the full output in memory.
///
/// [write] is called sequentially (never concurrently). Chunks arrive
/// in order. The consumer owns flushing and closing.
abstract interface class PdfSink {
  /// Write a chunk of output bytes.
  ///
  /// Called sequentially. The engine may call this many times with
  /// varying chunk sizes.
  FutureOr<void> write(Uint8List chunk);
}
