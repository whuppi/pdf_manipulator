import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// Annotation subtype per ISO 32000-1 Table 169.
enum PdfAnnotationSubtype implements PdfWire {
  /// Text annotation (sticky note).
  text('text'),

  /// Link annotation.
  link('link'),

  /// Free text annotation.
  freeText('freeText'),

  /// Line annotation.
  line('line'),

  /// Square annotation.
  square('square'),

  /// Circle annotation.
  circle('circle'),

  /// Polygon annotation.
  polygon('polygon'),

  /// Polyline annotation.
  polyLine('polyLine'),

  /// Highlight annotation.
  highlight('highlight'),

  /// Underline annotation.
  underline('underline'),

  /// Squiggly underline annotation.
  squiggly('squiggly'),

  /// Strikeout annotation.
  strikeOut('strikeOut'),

  /// Rubber stamp annotation.
  stamp('stamp'),

  /// Caret annotation.
  caret('caret'),

  /// Ink annotation.
  ink('ink'),

  /// Popup annotation.
  popup('popup'),

  /// File attachment annotation.
  fileAttachment('fileAttachment'),

  /// Sound annotation.
  sound('sound'),

  /// Movie annotation.
  movie('movie'),

  /// Widget annotation (form field).
  widget('widget'),

  /// Screen annotation.
  screen('screen'),

  /// Printer's mark annotation.
  printerMark('printerMark'),

  /// Trap network annotation.
  trapNet('trapNet'),

  /// Watermark annotation.
  watermark('watermark'),

  /// 3D annotation.
  threeD('threeD'),

  /// Redaction annotation.
  redact('redact'),

  /// RichMedia annotation.
  richMedia('richMedia'),

  /// A subtype this package does not know.
  unknown('unknown');

  const PdfAnnotationSubtype(this.wire);

  @override
  final String wire;

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
