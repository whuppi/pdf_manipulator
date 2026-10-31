import 'dart:convert';
import 'dart:typed_data';

import '../types/data_sink.dart';

/// A [DataSink] that collects written chunks into an in-memory buffer.
///
/// The simplest sink — let an operation write its output, then take the
/// finished bytes with [takeBytes], or the finished text with [takeString].
///
/// ```dart
/// final out = MemorySink();
/// await pdf.merge([a, b], out);
/// final merged = out.takeBytes();
/// ```
///
/// The whole output is held in memory. For large outputs, write to a file
/// sink instead so the output streams to disk rather than piling up in RAM.
final class MemorySink implements DataSink {
  final _buffer = BytesBuilder(copy: false);

  @override
  void write(Uint8List chunk) => _buffer.add(chunk);

  /// Holds nothing to release; the buffer stays readable after [close].
  @override
  void close() {}

  /// Returns the accumulated bytes and clears the buffer. Call once, after
  /// the operation completes.
  Uint8List takeBytes() => _buffer.takeBytes();

  /// Returns the accumulated bytes decoded as UTF-8 and clears the buffer.
  /// Call once, after the operation completes.
  String takeString() => utf8.decode(_buffer.takeBytes());
}
