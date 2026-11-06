import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

import '../types/data_source.dart';

/// A [DataSource] backed by a browser `Blob`. Web only.
///
/// A `File` from a file picker or a drop is a `Blob`. The Blob goes to the
/// lane's worker with the job, by reference and without a copy, and the
/// worker reads the slices the engine asks for synchronously (in OPFS mode it
/// first copies the Blob to its own OPFS file): memory stays O(1) for a file of
/// any size, and no read crosses to the main thread.
///
/// ```dart
/// import 'package:pdf_manipulator/web.dart';
///
/// final doc = await pdf.open(PdfInput(BlobSource(file)));
/// ```
///
/// A file that changes on disk while an operation reads it fails that
/// operation with `PdfIoFailed`; it never reads wrong bytes.
final class BlobSource implements DataSource {
  /// Wraps [blob] without reading it.
  BlobSource(this.blob);

  /// The wrapped Blob.
  final web.Blob blob;

  @override
  int get length => blob.size;

  /// Reads on the main thread through `Blob.slice().arrayBuffer()`. A lane
  /// does not call it for this source; it is for reading the source yourself.
  @override
  Future<Uint8List> readAt(int offset, int count) async {
    final start = offset.clamp(0, length);
    final end = (offset + count).clamp(start, length);
    final buffer = await blob.slice(start, end).arrayBuffer().toDart;
    return buffer.toDart.asUint8List();
  }

  /// Holds nothing to release.
  @override
  void close() {}
}
