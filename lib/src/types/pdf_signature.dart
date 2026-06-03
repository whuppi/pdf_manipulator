import 'package:meta/meta.dart';

@immutable
class PdfSignatureInfo {
  final String? signerName;
  final DateTime? signingTime;
  final String? reason;
  final String? location;
  final bool isValid;
  final PdfCertificateInfo? certificate;

  const PdfSignatureInfo({
    this.signerName,
    this.signingTime,
    this.reason,
    this.location,
    required this.isValid,
    this.certificate,
  });

  @override
  String toString() =>
      'PdfSignatureInfo(signer: $signerName, valid: $isValid, '
      'time: $signingTime, reason: $reason)';
}

@immutable
class PdfCertificateInfo {
  final String? subject;
  final String? issuer;
  final String? serial;
  final DateTime? notBefore;
  final DateTime? notAfter;
  final bool isValid;

  const PdfCertificateInfo({
    this.subject,
    this.issuer,
    this.serial,
    this.notBefore,
    this.notAfter,
    required this.isValid,
  });

  @override
  String toString() =>
      'PdfCertificateInfo(subject: $subject, issuer: $issuer, valid: $isValid)';
}
