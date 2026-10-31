import 'package:meta/meta.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// One file embedded in a document's `/Names /EmbeddedFiles` tree.
///
/// Listing reads the file spec only; the bytes come from
/// `PdfDoc.extractAttachment`.
@immutable
class PdfAttachment {
  /// Creates an attachment record from the engine's listing.
  const PdfAttachment({
    required this.name,
    required this.size,
    required this.description,
    required this.mimeType,
  });

  /// The attachment's file name — the key `extractAttachment` takes.
  final String name;

  /// Byte size as the file spec declares it, or `null` when it declares
  /// none. A declared size is the producer's claim, not a measurement.
  final int? size;

  /// The file spec's description, or `null` when it has none.
  final String? description;

  /// MIME type of the embedded stream, or `null` when it declares none.
  final String? mimeType;
}

/// How an embedded file relates to the document that carries it
/// (ISO 32000-2 associated files).
enum PdfAttachmentRelationship implements PdfWire {
  /// The original source the document was made from.
  source('source'),

  /// Data the document's content references.
  data('data'),

  /// An alternative representation of the document.
  alternative('alternative'),

  /// Supplementary material.
  supplement('supplement'),

  /// No stated relationship.
  unspecified('unspecified');

  const PdfAttachmentRelationship(this.wire);

  @override
  final String wire;
}
