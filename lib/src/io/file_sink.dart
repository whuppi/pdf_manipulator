import 'dart:io';
import 'dart:typed_data';

import '../types/data_sink.dart';

/// A [DataSink] that streams written chunks to a file on disk. Native
/// platforms only.
///
/// Each chunk is appended as it arrives, so memory stays constant — the
/// output never accumulates in RAM. Open it, pass it to an operation, then
/// [close] it to flush and release the handle.
///
/// ```dart
/// import 'package:pdf_manipulator/io.dart';
///
/// final out = await FileSink.create(File('/path/to/out.pdf'));
/// try {
///   await pdf.merge([a, b], out);
/// } finally {
///   await out.close();
/// }
/// ```
///
/// Web has no filesystem; this class is exported from
/// `package:pdf_manipulator/io.dart` (not the main library) so web builds
/// never pull in `dart:io`.
final class FileSink implements DataSink {
  FileSink._(this._raf);

  final RandomAccessFile _raf;

  /// Opens [file] for writing, truncating any existing content. Pair every
  /// `create` with a [close].
  static Future<FileSink> create(File file) async {
    final raf = await file.open(mode: FileMode.write);
    return FileSink._(raf);
  }

  @override
  Future<void> write(Uint8List chunk) => _raf.writeFrom(chunk);

  /// Flushes pending writes and releases the file handle. Idempotent only
  /// once — call exactly once when the operation completes.
  Future<void> close() async {
    await _raf.flush();
    await _raf.close();
  }
}
