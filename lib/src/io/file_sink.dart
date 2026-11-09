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
/// final out = await FileSink.open(File('/path/to/out.pdf'));
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
  bool _closed = false;

  /// The write in flight: an abort waits for it, since a file handle runs
  /// one async operation at a time.
  Future<void> _writing = Future.value();

  /// Opens [file] for writing, truncating any existing content. Holds one
  /// file handle until [close].
  static Future<FileSink> open(File file) async {
    final raf = await file.open(mode: FileMode.write);
    return FileSink._(raf);
  }

  @override
  Future<void> write(Uint8List chunk) =>
      _writing = _raf.writeFrom(chunk).then((_) {});

  /// Truncates the file to zero length: it holds only the output of the
  /// operation that ended cancelled or failed. Call it before [close]; on a
  /// closed sink it does nothing.
  @override
  Future<void> abort(Object reason) async {
    if (_closed) return;
    try {
      await _writing;
    } catch (_) {
      // A failed write may be what ended the operation; the file is
      // truncated all the same.
    }
    await _raf.truncate(0);
    await _raf.setPosition(0);
  }

  /// Flushes pending writes and releases the file handle. Idempotent: safe
  /// to call more than once (e.g. from a `finally` plus an error path); the
  /// second and later calls are no-ops.
  @override
  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    await _raf.flush();
    await _raf.close();
  }
}
