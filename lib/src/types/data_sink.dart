import 'dart:async';
import 'dart:typed_data';

/// Sequential byte destination — any output target.
///
/// Used for PDF output, converted office documents, rendered images —
/// any data the engine produces. Chunks are typically 4KB–256KB.
///
/// Implement with your destination: file (write), memory
/// (BytesBuilder), upload stream, HTTP upload, pipe, etc.
///
/// Whoever obtained a sink closes it. A sink you pass to an operation is
/// yours to close; a sink a factory you passed returns is the library's,
/// and it closes that one after its item, and on error or cancel.
abstract interface class DataSink {
  /// Write a chunk. Called sequentially, never concurrently.
  FutureOr<void> write(Uint8List chunk);

  /// Discards what was written: the operation this sink was handed to ended
  /// cancelled or failed, so [reason] (the `PdfCancelled` or the error)
  /// says why and the sink holds no partial output.
  ///
  /// The library calls it at most once, never writes to the sink after it,
  /// and a write that began before the cancel may still be finishing when it
  /// is called. A sink that is a file truncates it; a sink that is a
  /// buffer clears it; a sink that streams to a destination that cannot take
  /// bytes back (an upload) cancels the upload. The owner still closes the
  /// sink: the library closes a sink a factory returned after aborting it,
  /// as it closes one today.
  FutureOr<void> abort(Object reason);

  /// Flushes pending writes and releases whatever the sink holds (a file
  /// handle, a connection). Idempotent. A sink that holds nothing
  /// implements it as a no-op.
  FutureOr<void> close();
}
