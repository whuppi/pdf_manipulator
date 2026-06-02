import 'dart:async';
import 'dart:typed_data';

/// Random-access byte source — any file, any backing store.
///
/// The engine reads arbitrary offsets on demand (xref at end of file,
/// objects scattered throughout). This is NOT a forward-only stream —
/// the engine seeks backward and forward. A one-shot socket or pipe
/// that can only read sequentially cannot be a DataSource. Buffer the
/// full content first, or use a backing store with random access.
///
/// Memory is O(1) per read (64KB max per call). The full file is
/// never buffered by the engine — only the backing store holds it.
///
/// Implement with your backing store: file (pread), memory
/// (sublistView), HTTP (Range header), IndexedDB, web Blob.slice.
abstract interface class DataSource {
  /// Total size in bytes.
  int get length;

  /// Read [count] bytes starting at [offset].
  FutureOr<Uint8List> readAt(int offset, int count);
}

/// Read the entire DataSource into bytes. Used internally by the
/// bridge when the FFI layer needs the full data (secondary inputs).
Future<Uint8List> readAllBytes(DataSource source) async {
  return await source.readAt(0, source.length);
}
