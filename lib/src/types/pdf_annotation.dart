import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Annotation subtype per ISO 32000-1 Table 169.
enum PdfAnnotationSubtype {
  /// Text annotation (sticky note).
  text,

  /// Link annotation.
  link,

  /// Free text annotation.
  freeText,

  /// Line annotation.
  line,

  /// Square annotation.
  square,

  /// Circle annotation.
  circle,

  /// Polygon annotation.
  polygon,

  /// Polyline annotation.
  polyLine,

  /// Highlight annotation.
  highlight,

  /// Underline annotation.
  underline,

  /// Squiggly underline annotation.
  squiggly,

  /// Strikeout annotation.
  strikeOut,

  /// Rubber stamp annotation.
  stamp,

  /// Caret annotation.
  caret,

  /// Ink annotation.
  ink,

  /// Popup annotation.
  popup,

  /// File attachment annotation.
  fileAttachment,

  /// Sound annotation.
  sound,

  /// Movie annotation.
  movie,

  /// Widget annotation (form field).
  widget,

  /// Screen annotation.
  screen,

  /// Printer's mark annotation.
  printerMark,

  /// Trap network annotation.
  trapNet,

  /// Watermark annotation.
  watermark,

  /// 3D annotation.
  threeD,

  /// Redaction annotation.
  redact,

  /// RichMedia annotation.
  richMedia,

  /// A subtype this package does not know.
  unknown;

  /// Maps a raw `/Subtype` name (e.g. `Text`, `3D`) to its enum value.
  /// Anything unmapped, `""` included, is [unknown].
  static PdfAnnotationSubtype fromPdfName(String name) {
    if (name == '3D') return PdfAnnotationSubtype.threeD;
    for (final value in PdfAnnotationSubtype.values) {
      if (value == PdfAnnotationSubtype.unknown) continue;
      final n = value.name;
      if ('${n[0].toUpperCase()}${n.substring(1)}' == name) return value;
    }
    return PdfAnnotationSubtype.unknown;
  }
}

/// One annotation listed by `PdfDoc.annotations(page)`.
@immutable
class PdfAnnotationInfo {
  /// Creates an annotation record from the engine's listing.
  const PdfAnnotationInfo({
    required this.subtype,
    required this.subtypeName,
    required this.rect,
    required this.contents,
    required this.author,
    required this.fieldName,
  });

  /// The annotation's subtype.
  final PdfAnnotationSubtype subtype;

  /// The raw `/Subtype` name, or `""` when the annotation has none.
  final String subtypeName;

  /// The annotation's rectangle, or `null` when it has no `/Rect`.
  final PdfRect? rect;

  /// The annotation's text contents, or `null` when it has none.
  final String? contents;

  /// The annotation's author (`/T`), or `null` when it has none. Always
  /// `null` on a form widget, whose `/T` is [fieldName].
  final String? author;

  /// The form field name on a widget annotation, or `null` when the
  /// annotation is not a named widget.
  final String? fieldName;
}
