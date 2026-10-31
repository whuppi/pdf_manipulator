import 'dart:io';
import 'dart:typed_data';

import '../types/data_source.dart';
import '../types/errors.dart';

/// A [DataSource] backed by a file on disk. Native platforms only.
///
/// Reads arbitrary offsets with positioned reads — each call returns only
/// the bytes asked for, so the source holds one chunk at a time, never the
/// whole file. The engine pulls a 7 GB PDF in the same small (≤64KB) bites
/// as a 7 KB one.
///
/// ```dart
/// import 'package:pdf_manipulator/io.dart';
///
/// final source = await FileSource.open(File('/path/to/big.pdf'));
/// try {
///   final doc = await pdf.open(source);
///   ...
/// } finally {
///   await source.close();
/// }
/// ```
///
/// Web has no filesystem; this class is exported from
/// `package:pdf_manipulator/io.dart` (not the main library) so web builds
/// never pull in `dart:io`. On web, back the source with a `Blob` instead.
final class FileSource implements DataSource {
  FileSource._(this._raf, this.length);

  final RandomAccessFile _raf;

  // A RandomAccessFile refuses overlapping async calls, and reads can arrive
  // from several lanes at once: each read waits for the one before it, so
  // the position set by one read is never moved by another.
  Future<void> _tail = Future.value();
  bool _closed = false;

  /// Opens [file] and reads its length. Holds one file handle until [close];
  /// the file must exist and not be truncated while operations read it.
  static Future<FileSource> open(File file) async {
    final raf = await file.open();
    try {
      return FileSource._(raf, await raf.length());
    } catch (_) {
      await raf.close();
      rethrow;
    }
  }

  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) {
    if (_closed) {
      throw const PdfDisposed('This FileSource has been closed');
    }
    final read = _tail.then((_) async {
      await _raf.setPosition(offset);
      return _raf.read(count);
    });
    _tail = read.then<void>((_) {}, onError: (Object _) {});
    return read;
  }

  /// Waits for the reads in flight, then releases the file handle.
  /// Idempotent.
  @override
  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    await _tail;
    await _raf.close();
  }
}
