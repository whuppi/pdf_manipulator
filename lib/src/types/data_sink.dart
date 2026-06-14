import 'dart:async';
import 'dart:typed_data';

/// Sequential byte destination — any output target.
///
/// Used for PDF output, converted office documents, rendered images —
/// any data the engine produces. Chunks are typically 4KB–256KB.
///
/// Implement with your destination: file (write), memory
/// (BytesBuilder), upload stream, HTTP upload, pipe, etc.
abstract interface class DataSink {
  /// Write a chunk. Called sequentially, never concurrently.
  FutureOr<void> write(Uint8List chunk);
}
