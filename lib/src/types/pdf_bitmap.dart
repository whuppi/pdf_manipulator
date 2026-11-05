import 'dart:typed_data';

import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/equality.dart';

/// One rendered page held whole: which page, its size in pixels and the
/// bytes in the format the render asked for. The one value that holds
/// pixels; `PdfReads.render` streams pages to sinks instead.
@immutable
final class PdfBitmap {
  /// Creates a bitmap of [page], [width] by [height] pixels, holding [bytes].
  const PdfBitmap({
    required this.page,
    required this.width,
    required this.height,
    required this.bytes,
  });

  /// Zero-based index of the page that was rendered.
  final int page;

  /// Image width in pixels.
  final int width;

  /// Image height in pixels.
  final int height;

  /// A PNG file or raw RGBA rows, as the render's `PdfRenderFormat` says.
  final Uint8List bytes;

  @override
  bool operator ==(Object other) =>
      other is PdfBitmap &&
      other.page == page &&
      other.width == width &&
      other.height == height &&
      listsEqual(other.bytes, bytes);

  @override
  int get hashCode => Object.hash(page, width, height, Object.hashAll(bytes));

  @override
  String toString() => 'PdfBitmap(page $page, ${width}x$height)';
}
