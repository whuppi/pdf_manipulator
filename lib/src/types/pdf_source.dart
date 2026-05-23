import 'dart:async';
import 'dart:typed_data';

/// Random-access reader for PDF input data.
///
/// Consumers implement this to provide PDF bytes from any backing store
/// (file, network, database, memory). The engine reads only the ranges
/// it needs — xref tables, page objects, font data — never the full file.
abstract interface class PdfSource {
  /// Total size of the source in bytes.
  int get length;

  /// Read [count] bytes starting at [offset].
  FutureOr<Uint8List> readAt(int offset, int count);
}
