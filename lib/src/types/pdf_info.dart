import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// What a document says about itself, read by `Pdf.info` without keeping the
/// document open.
@immutable
class PdfInfo {
  /// Creates a document description.
  const PdfInfo({
    required this.pageCount,
    required this.version,
    required this.isTagged,
    required this.title,
    required this.author,
    required this.subject,
    required this.keywords,
    required this.producer,
    required this.creator,
    required this.creationDate,
    required this.isEncrypted,
    required this.requiresPassword,
    required this.encryptionAlgorithm,
    required this.permissions,
  });

  /// Total number of pages in the document.
  final int pageCount;

  /// PDF version string (e.g. "1.7", "2.0").
  final String version;

  /// Whether the document contains tagged (accessible) structure.
  final bool isTagged;

  /// Document title from metadata, if present.
  final String? title;

  /// Document author from metadata, if present.
  final String? author;

  /// Document subject from metadata, if present.
  final String? subject;

  /// Document keywords from metadata, if present.
  final String? keywords;

  /// Document producer (the software that produced the PDF), if present.
  final String? producer;

  /// Document creator (the source application), if present.
  final String? creator;

  /// Document creation date as a raw PDF date string
  /// (e.g. `D:20240101120000Z`), if present.
  final String? creationDate;

  /// Whether the document is encrypted.
  final bool isEncrypted;

  /// Whether a password is required to open the document.
  final bool requiresPassword;

  /// Encryption algorithm used, if the document is encrypted.
  final PdfEncryptionAlgorithm? encryptionAlgorithm;

  /// Permission flags, if the document is encrypted.
  final PdfPermissions? permissions;

  @override
  bool operator ==(Object other) =>
      other is PdfInfo &&
      other.pageCount == pageCount &&
      other.version == version &&
      other.isTagged == isTagged &&
      other.title == title &&
      other.author == author &&
      other.subject == subject &&
      other.keywords == keywords &&
      other.producer == producer &&
      other.creator == creator &&
      other.creationDate == creationDate &&
      other.isEncrypted == isEncrypted &&
      other.requiresPassword == requiresPassword &&
      other.encryptionAlgorithm == encryptionAlgorithm &&
      other.permissions?.toBits() == permissions?.toBits();

  @override
  int get hashCode => Object.hash(
    pageCount,
    version,
    isTagged,
    title,
    author,
    subject,
    keywords,
    producer,
    creator,
    creationDate,
    isEncrypted,
    requiresPassword,
    encryptionAlgorithm,
    permissions?.toBits(),
  );

  @override
  String toString() => 'PdfInfo($pageCount pages, PDF $version)';
}
