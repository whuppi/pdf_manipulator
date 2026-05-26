import 'dart:async';
import 'dart:typed_data';

/// Random-access byte source — any file, any backing store.
///
/// Used for PDFs, images, office documents, embedded attachments —
/// any data the engine needs to read. The engine reads only the
/// ranges it needs, never the full file.
///
/// Implement with your backing store: file (pread), memory
/// (sublistView), HTTP (Range header), IndexedDB (key range), etc.
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
