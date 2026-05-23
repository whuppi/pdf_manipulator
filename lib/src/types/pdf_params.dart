// Typed parameter groups — no loose parameters.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// RGB color for watermarks and annotations.
class PdfColor {
  final double r, g, b;
  const PdfColor(this.r, this.g, this.b);

  static const black = PdfColor(0, 0, 0);
  static const white = PdfColor(1, 1, 1);
  static const gray = PdfColor(0.5, 0.5, 0.5);
  static const red = PdfColor(1, 0, 0);
}

/// Permission flags for encrypted PDFs.
class PdfPermissions {
  final bool print, printHq, modify, copy;
  final bool annotate, fillForms, accessibility, assemble;

  const PdfPermissions({
    this.print = true,
    this.printHq = true,
    this.modify = true,
    this.copy = true,
    this.annotate = true,
    this.fillForms = true,
    this.accessibility = true,
    this.assemble = true,
  });

  const PdfPermissions.all() : this();

  const PdfPermissions.readOnly()
      : print = false,
        printHq = false,
        modify = false,
        copy = false,
        annotate = false,
        fillForms = false,
        accessibility = true,
        assemble = false;

  int toBits() {
    int bits = 0xFFFFF0C0; // reserved bits per PDF spec
    if (print) bits |= 1 << 2;
    if (modify) bits |= 1 << 3;
    if (copy) bits |= 1 << 4;
    if (annotate) bits |= 1 << 5;
    if (fillForms) bits |= 1 << 8;
    if (accessibility) bits |= 1 << 9;
    if (assemble) bits |= 1 << 10;
    if (printHq) bits |= 1 << 11;
    return bits;
  }
}

/// Encryption configuration.
class PdfEncryptionConfig {
  final String ownerPassword;
  final String userPassword;
  final PdfEncryptionAlgorithm algorithm;
  final PdfPermissions permissions;

  const PdfEncryptionConfig({
    required this.ownerPassword,
    this.userPassword = '',
    this.algorithm = PdfEncryptionAlgorithm.aes256,
    this.permissions = const PdfPermissions.all(),
  });
}

/// Save options — compression, GC, linearization, optional encryption.
class PdfSaveOptions {
  final bool compress;
  final bool garbageCollect;
  final bool linearize;
  final PdfEncryptionConfig? encryption;

  const PdfSaveOptions({
    this.compress = true,
    this.garbageCollect = true,
    this.linearize = false,
    this.encryption,
  });
}

/// Watermark text style.
class PdfWatermarkStyle {
  final double fontSize;
  final String? fontName;
  final double opacity;
  final double rotation;
  final PdfColor color;

  const PdfWatermarkStyle({
    this.fontSize = 48,
    this.fontName,
    this.opacity = 0.3,
    this.rotation = 45,
    this.color = PdfColor.gray,
  });
}

/// Watermark position (null = centered diagonal).
class PdfWatermarkPosition {
  final double x, y, width, height;
  final bool fixedPrint;
  final double fixedPrintH, fixedPrintV;

  const PdfWatermarkPosition({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
    this.fixedPrint = false,
    this.fixedPrintH = 0,
    this.fixedPrintV = 0,
  });
}

/// Output size constraint for rendering.
class PdfRenderSize {
  final int maxWidth;
  final int maxHeight;
  const PdfRenderSize({required this.maxWidth, required this.maxHeight});

  const PdfRenderSize.thumbnail(int size)
      : maxWidth = size,
        maxHeight = size;
}

/// Signing credentials — PKCS#12 bundle or separate PEM cert + key.
sealed class PdfSigningCredentials {
  const PdfSigningCredentials();

  /// PKCS#12 (.p12 / .pfx) bundle containing certificate + private key.
  const factory PdfSigningCredentials.pkcs12(
      Uint8List data, String password) = PdfPkcs12Credentials;

  /// Separate PEM-encoded certificate and private key.
  const factory PdfSigningCredentials.pem(
      String certPem, String keyPem) = PdfPemCredentials;
}

class PdfPkcs12Credentials extends PdfSigningCredentials {
  final Uint8List data;
  final String password;
  const PdfPkcs12Credentials(this.data, this.password);
}

class PdfPemCredentials extends PdfSigningCredentials {
  final String certPem;
  final String keyPem;
  const PdfPemCredentials(this.certPem, this.keyPem);
}

/// Validation result.
class PdfValidationResult {
  final bool compliant;
  final int errors;
  final int warnings;
  const PdfValidationResult({
    required this.compliant,
    required this.errors,
    required this.warnings,
  });
}

/// A bookmark-based split plan entry.
class PdfBookmarkSplit {
  final String title;
  final int startPage;
  final int endPage;
  const PdfBookmarkSplit({
    required this.title,
    required this.startPage,
    required this.endPage,
  });
}

/// Page classification result.
class PdfPageClassification {
  final String type;
  final double confidence;
  const PdfPageClassification({required this.type, required this.confidence});
}

/// Document classification result.
class PdfDocumentClassification {
  final String type;
  final double confidence;
  final int pageCount;
  const PdfDocumentClassification({
    required this.type,
    required this.confidence,
    required this.pageCount,
  });
}
