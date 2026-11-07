import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_metadata.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';

/// What a document says about itself, read by `Pdf.info` without keeping the
/// document open.
@immutable
final class PdfDocumentInfo {
  /// Creates a document description.
  const PdfDocumentInfo({
    required this.pageCount,
    required this.version,
    required this.isTagged,
    required this.metadata,
    required this.encryption,
  });

  /// Total number of pages in the document.
  final int pageCount;

  /// PDF version string (e.g. "1.7", "2.0").
  final String version;

  /// Whether the document contains tagged (accessible) structure.
  final bool isTagged;

  /// What the document's info dictionary carries.
  final PdfMetadata metadata;

  /// How the document is encrypted, or null when it is not encrypted.
  final PdfEncryptionInfo? encryption;

  @override
  bool operator ==(Object other) =>
      other is PdfDocumentInfo &&
      other.pageCount == pageCount &&
      other.version == version &&
      other.isTagged == isTagged &&
      other.metadata == metadata &&
      other.encryption == encryption;

  @override
  int get hashCode =>
      Object.hash(pageCount, version, isTagged, metadata, encryption);

  @override
  String toString() => 'PdfDocumentInfo($pageCount pages, PDF $version)';
}

/// How a document is encrypted: the algorithm that protects it, what a
/// reader is allowed to do with it, and whether a password opens it.
@immutable
final class PdfEncryptionInfo {
  /// Creates an encryption description.
  const PdfEncryptionInfo({
    required this.algorithm,
    required this.permissions,
    required this.requiresPassword,
  });

  /// The algorithm the document is encrypted with.
  final PdfEncryptionAlgorithm algorithm;

  /// What the document's owner allowed a reader to do with it.
  final PdfPermissions permissions;

  /// Whether a password is needed to open the document.
  final bool requiresPassword;

  @override
  bool operator ==(Object other) =>
      other is PdfEncryptionInfo &&
      other.algorithm == algorithm &&
      other.permissions == permissions &&
      other.requiresPassword == requiresPassword;

  @override
  int get hashCode => Object.hash(algorithm, permissions, requiresPassword);

  @override
  String toString() =>
      'PdfEncryptionInfo($algorithm, requiresPassword: $requiresPassword)';
}
