import 'package:meta/meta.dart';

/// Information about a digital signature in a PDF.
@immutable
class PdfSignatureInfo {
  /// Creates signature info.
  const PdfSignatureInfo({
    this.signerName,
    this.signingTime,
    this.reason,
    this.location,
    required this.isValid,
    required this.coversWholeDocument,
    this.certificate,
  });

  /// Name of the signer.
  final String? signerName;

  /// When the document was signed.
  final DateTime? signingTime;

  /// Stated reason for signing.
  final String? reason;

  /// Location where the document was signed.
  final String? location;

  /// Whether the signature's digest matches the bytes it covers and the
  /// signer's key verifies it. Certificate trust and revocation are not
  /// checked.
  final bool isValid;

  /// Whether the signed byte ranges reach the last byte of the file.
  /// False when the file was changed after signing, by an incremental
  /// save for example; isValid then still describes the signed revision.
  final bool coversWholeDocument;

  /// Certificate details, if available.
  final PdfCertificateInfo? certificate;

  @override
  String toString() =>
      'PdfSignatureInfo(signer: $signerName, valid: $isValid, '
      'coversWholeDocument: $coversWholeDocument, '
      'time: $signingTime, reason: $reason)';
}

/// X.509 certificate information.
@immutable
class PdfCertificateInfo {
  /// Creates certificate info.
  const PdfCertificateInfo({
    this.subject,
    this.issuer,
    this.serial,
    this.notBefore,
    this.notAfter,
    required this.isValid,
  });

  /// Certificate subject (CN/O/OU).
  final String? subject;

  /// Certificate issuer.
  final String? issuer;

  /// Serial number.
  final String? serial;

  /// Validity start date.
  final DateTime? notBefore;

  /// Validity end date.
  final DateTime? notAfter;

  /// Whether the certificate is currently valid.
  final bool isValid;

  @override
  String toString() =>
      'PdfCertificateInfo(subject: $subject, issuer: $issuer, valid: $isValid)';
}
