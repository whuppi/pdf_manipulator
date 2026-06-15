import 'dart:typed_data';

import '../types/data_source.dart';

/// A [DataSource] backed by an in-memory byte buffer.
///
/// The simplest source — wrap a `Uint8List` you already hold (a download,
/// an asset, a clipboard paste) and read from it. Random access is a plain
/// sublist view, so reads are zero-copy and O(1).
///
/// ```dart
/// final doc = await pdf.open(MemorySource(pdfBytes));
/// ```
///
/// For data that doesn't fit in memory, back the source with a file or an
/// HTTP range reader instead so the input streams in chunks rather than
/// sitting fully in RAM.
final class MemorySource implements DataSource {
  /// Wraps `bytes` without copying. The buffer must not be mutated for the
  /// lifetime of any operation reading from this source.
  MemorySource(this._bytes);

  final Uint8List _bytes;

  @override
  int get length => _bytes.length;

  @override
  Uint8List readAt(int offset, int count) =>
      Uint8List.sublistView(_bytes, offset, (offset + count).clamp(0, _bytes.length));
}
