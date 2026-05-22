import 'dart:async';
import 'dart:typed_data';

/// Random-access reader for PDF input data.
///
/// Consumers implement this to provide PDF bytes from any backing store
/// (file, network, database, memory). The engine reads only the ranges
/// it needs — xref tables, page objects, font data — never the full file.
///
/// Both [length] and [readAt] must be safe to call from any isolate.
/// The implementation owns its own I/O — the engine never caches the
/// returned bytes beyond the immediate parse.
abstract interface class PdfSource {
  /// Total size of the source in bytes.
  int get length;

  /// Read [count] bytes starting at [offset].
  ///
  /// Returns exactly [count] bytes, or fewer only if [offset] + [count]
  /// exceeds [length]. Never returns an empty list unless [count] is 0
  /// or [offset] >= [length].
  FutureOr<Uint8List> readAt(int offset, int count);
}
