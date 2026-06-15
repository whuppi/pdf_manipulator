import 'dart:io';
import 'dart:typed_data';

import '../types/data_source.dart';

/// A [DataSource] backed by a file on disk. Native platforms only.
///
/// Reads arbitrary offsets via `pread`-style positioned reads — each call
/// returns only the bytes asked for, so the source holds one chunk at a
/// time, never the whole file. The engine pulls a 7 GB PDF in the same
/// small (≤64KB) bites as a 7 KB one.
///
/// ```dart
/// import 'package:pdf_manipulator/io.dart';
///
/// final doc = await pdf.open(FileSource(File('/path/to/big.pdf')));
/// ```
///
/// Web has no filesystem; this class is exported from
/// `package:pdf_manipulator/io.dart` (not the main library) so web builds
/// never pull in `dart:io`. On web, back the source with a `Blob` instead.
final class FileSource implements DataSource {
  /// Wraps `file`. Its length is read synchronously at construction; the
  /// file must exist and not be truncated while operations are reading it.
  FileSource(this._file) : length = _file.lengthSync();

  final File _file;

  @override
  final int length;

  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final raf = await _file.open();
    try {
      await raf.setPosition(offset);
      return await raf.read(count);
    } finally {
      await raf.close();
    }
  }
}
