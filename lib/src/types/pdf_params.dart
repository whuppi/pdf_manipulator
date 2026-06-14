// Typed parameter groups — no loose parameters.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/pdf_enums.dart';

/// RGB color for watermarks and annotations.
class PdfColor {
  /// Creates an RGB color with components in the range 0.0 to 1.0.
  const PdfColor(this.r, this.g, this.b);

  /// Red component.
  final double r;

  /// Green component.
  final double g;

  /// Blue component.
  final double b;

  /// Black (0, 0, 0).
  static const black = PdfColor(0, 0, 0);

  /// White (1, 1, 1).
  static const white = PdfColor(1, 1, 1);

  /// Gray (0.5, 0.5, 0.5).
  static const gray = PdfColor(0.5, 0.5, 0.5);

  /// Red (1, 0, 0).
  static const red = PdfColor(1, 0, 0);
}

/// Permission flags for encrypted PDFs.
class PdfPermissions {
  /// Creates permission flags with individual overrides.
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

  /// All permissions granted.
  const PdfPermissions.all() : this();

  /// Read-only — most permissions revoked except accessibility.
  const PdfPermissions.readOnly()
    : print = false,
      printHq = false,
      modify = false,
      copy = false,
      annotate = false,
      fillForms = false,
      accessibility = true,
      assemble = false;

  /// Allow printing at standard quality.
  final bool print;

  /// Allow high-quality printing.
  final bool printHq;

  /// Allow document modification.
  final bool modify;

  /// Allow content copying.
  final bool copy;

  /// Allow annotations.
  final bool annotate;

  /// Allow form filling.
  final bool fillForms;

  /// Allow text extraction for accessibility.
  final bool accessibility;

  /// Allow page assembly (insert, delete, rotate).
  final bool assemble;

  /// Encodes the permission flags as a signed 32-bit int per the PDF spec.
  int toBits() {
    // Build as signed 32-bit int. The reserved bits (0xFFFFF0C0) set the
    // upper bits per PDF spec. Using .toSigned(32) ensures the value fits
    // in the binary codec's i32 type on both VM and web (dart2js has no i64).
    int bits = 0xFFFFF0C0;
    if (print) bits |= 1 << 2;
    if (modify) bits |= 1 << 3;
    if (copy) bits |= 1 << 4;
    if (annotate) bits |= 1 << 5;
    if (fillForms) bits |= 1 << 8;
    if (accessibility) bits |= 1 << 9;
    if (assemble) bits |= 1 << 10;
    if (printHq) bits |= 1 << 11;
    return bits.toSigned(32);
  }
}

/// Encryption intent on save — sealed, compiler-enforced.
sealed class PdfEncryption {
  const PdfEncryption();

  /// Preserve the source PDF's existing encryption.
  /// If the source was not encrypted, output is not encrypted.
  const factory PdfEncryption.keep() = PdfEncryptionKeep;

  /// Strip all encryption. Output is plaintext.
  const factory PdfEncryption.remove() = PdfEncryptionRemove;

  /// Apply new encryption.
  const factory PdfEncryption.config({
    required String ownerPassword,
    String userPassword,
    PdfEncryptionAlgorithm algorithm,
    PdfPermissions permissions,
  }) = PdfEncryptionConfig;
}

/// Keep existing encryption unchanged.
class PdfEncryptionKeep extends PdfEncryption {
  /// Creates a keep-encryption intent.
  const PdfEncryptionKeep();
}

/// Remove all encryption.
class PdfEncryptionRemove extends PdfEncryption {
  /// Creates a remove-encryption intent.
  const PdfEncryptionRemove();
}

/// Apply new encryption with explicit parameters.
class PdfEncryptionConfig extends PdfEncryption {
  /// Creates an encryption configuration.
  const PdfEncryptionConfig({
    required this.ownerPassword,
    this.userPassword = '',
    this.algorithm = PdfEncryptionAlgorithm.aes256,
    this.permissions = const PdfPermissions.all(),
  });

  /// Owner password (full access).
  final String ownerPassword;

  /// User password (restricted by [permissions]).
  final String userPassword;

  /// Encryption algorithm to use.
  final PdfEncryptionAlgorithm algorithm;

  /// Access permissions for the user password.
  final PdfPermissions permissions;
}

/// Save strategy — sealed so invalid combos are unrepresentable.
sealed class PdfSaveOptions {
  const PdfSaveOptions();

  /// Full rewrite — rebuild the entire PDF. Supports compression,
  /// garbage collection, and encryption changes.
  const factory PdfSaveOptions.fullRewrite({
    bool compress,
    bool garbageCollect,
    PdfEncryption encryption,
  }) = PdfSaveFullRewrite;

  /// Incremental — append only changed objects. Fastest for small
  /// edits on large files. Preserves digital signatures. No GC,
  /// no compression, no encryption changes possible.
  const factory PdfSaveOptions.incremental() = PdfSaveIncremental;
}

/// Full-rewrite save with compression, GC, and encryption control.
class PdfSaveFullRewrite extends PdfSaveOptions {
  /// Creates full-rewrite save options.
  const PdfSaveFullRewrite({
    this.compress = true,
    this.garbageCollect = true,
    this.encryption = const PdfEncryption.keep(),
  });

  /// Whether to compress streams.
  final bool compress;

  /// Whether to remove unreferenced objects.
  final bool garbageCollect;

  /// Encryption intent for the output.
  final PdfEncryption encryption;
}

/// Incremental save — append-only, preserves signatures.
class PdfSaveIncremental extends PdfSaveOptions {
  /// Creates incremental save options.
  const PdfSaveIncremental();
}

/// Watermark text style.
class PdfWatermarkStyle {
  /// Creates a watermark text style.
  const PdfWatermarkStyle({
    this.fontSize = 48,
    this.fontName,
    this.opacity = 0.3,
    this.rotation = 45,
    this.color = PdfColor.gray,
  });

  /// Font size in points.
  final double fontSize;

  /// Optional font name. Uses a default sans-serif if null.
  final String? fontName;

  /// Opacity from 0.0 (invisible) to 1.0 (opaque).
  final double opacity;

  /// Rotation angle in degrees.
  final double rotation;

  /// Text color.
  final PdfColor color;
}

/// Where to place the watermark on the page.
///
/// The engine resolves named positions (center, corner, tiled) to
/// coordinates using each page's media box at render time — the caller
/// never computes pixel values. Pages in the same PDF can be mixed
/// sizes (A4, Letter, A3); the engine handles each independently.
sealed class PdfWatermarkPosition {
  const PdfWatermarkPosition();

  /// Centered on the page. The default.
  const factory PdfWatermarkPosition.center() = PdfWatermarkCenter;

  /// Anchored to a corner with optional margin.
  const factory PdfWatermarkPosition.corner(
    PdfCorner corner, {
    double marginX,
    double marginY,
  }) = PdfWatermarkCorner;

  /// Tiled across the page in a grid.
  const factory PdfWatermarkPosition.tiled({int columns, int rows}) =
      PdfWatermarkTiled;

  /// Exact coordinates — caller is responsible for computing against
  /// the page dimensions.
  const factory PdfWatermarkPosition.exact({
    required double x,
    required double y,
    required double width,
    required double height,
  }) = PdfWatermarkExact;
}

/// Centered watermark position.
class PdfWatermarkCenter extends PdfWatermarkPosition {
  /// Creates a center-positioned watermark.
  const PdfWatermarkCenter();
}

/// Corner-anchored watermark position.
class PdfWatermarkCorner extends PdfWatermarkPosition {
  /// Creates a corner-anchored watermark.
  const PdfWatermarkCorner(this.corner, {this.marginX = 20, this.marginY = 20});

  /// Which corner to anchor to.
  final PdfCorner corner;

  /// Horizontal margin from the corner in points.
  final double marginX;

  /// Vertical margin from the corner in points.
  final double marginY;
}

/// Tiled watermark position.
class PdfWatermarkTiled extends PdfWatermarkPosition {
  /// Creates a tiled watermark.
  const PdfWatermarkTiled({this.columns = 3, this.rows = 4});

  /// Number of columns in the tile grid.
  final int columns;

  /// Number of rows in the tile grid.
  final int rows;
}

/// Exact-coordinate watermark position.
class PdfWatermarkExact extends PdfWatermarkPosition {
  /// Creates an exact-position watermark.
  const PdfWatermarkExact({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  /// X-coordinate in points.
  final double x;

  /// Y-coordinate in points.
  final double y;

  /// Width in points.
  final double width;

  /// Height in points.
  final double height;
}

/// Which corner to anchor a watermark.
enum PdfCorner {
  /// Top-left corner.
  topLeft,

  /// Top-right corner.
  topRight,

  /// Bottom-left corner.
  bottomLeft,

  /// Bottom-right corner.
  bottomRight,
}

/// Whether the watermark renders above or below page content.
enum PdfWatermarkLayer {
  /// Renders on top of page content (annotation-based). Default.
  foreground,

  /// Renders behind page content (content-stream, prepended in painting order).
  background,
}

/// Output size constraint for rendering.
class PdfRenderSize {
  /// Creates a render size constraint.
  const PdfRenderSize({required this.maxWidth, required this.maxHeight});

  /// Creates a square thumbnail constraint.
  const PdfRenderSize.thumbnail(int size) : maxWidth = size, maxHeight = size;

  /// Maximum output width in pixels.
  final int maxWidth;

  /// Maximum output height in pixels.
  final int maxHeight;
}

/// Signing credentials — PKCS#12 bundle or separate PEM cert + key.
sealed class PdfSigningCredentials {
  const PdfSigningCredentials();

  /// PKCS#12 (.p12 / .pfx) bundle containing certificate + private key.
  const factory PdfSigningCredentials.pkcs12(Uint8List data, String password) =
      PdfPkcs12Credentials;

  /// Separate PEM-encoded certificate and private key.
  const factory PdfSigningCredentials.pem(String certPem, String keyPem) =
      PdfPemCredentials;
}

/// PKCS#12 signing credentials.
class PdfPkcs12Credentials extends PdfSigningCredentials {
  /// Creates PKCS#12 credentials.
  const PdfPkcs12Credentials(this.data, this.password);

  /// The raw .p12 / .pfx bytes.
  final Uint8List data;

  /// Password to unlock the bundle.
  final String password;
}

/// PEM-encoded signing credentials.
class PdfPemCredentials extends PdfSigningCredentials {
  /// Creates PEM credentials.
  const PdfPemCredentials(this.certPem, this.keyPem);

  /// PEM-encoded certificate.
  final String certPem;

  /// PEM-encoded private key.
  final String keyPem;
}

/// Validation result.
class PdfValidationResult {
  /// Creates a validation result.
  const PdfValidationResult({
    required this.compliant,
    required this.errors,
    required this.warnings,
  });

  /// Whether the document is compliant with the checked standard.
  final bool compliant;

  /// Number of validation errors.
  final int errors;

  /// Number of validation warnings.
  final int warnings;
}

/// A bookmark-based split plan entry.
class PdfBookmarkSplit {
  /// Creates a bookmark split entry.
  const PdfBookmarkSplit({
    required this.title,
    required this.startPage,
    required this.endPage,
  });

  /// Bookmark title.
  final String title;

  /// Start page (inclusive).
  final int startPage;

  /// End page (exclusive).
  final int endPage;
}

/// Page classification result.
class PdfPageClassification {
  /// Creates a page classification result.
  const PdfPageClassification({required this.type, required this.confidence});

  /// Detected page type (e.g. "text", "image", "form").
  final String type;

  /// Confidence score from 0.0 to 1.0.
  final double confidence;
}

/// Document classification result.
class PdfDocumentClassification {
  /// Creates a document classification result.
  const PdfDocumentClassification({
    required this.type,
    required this.confidence,
    required this.pageCount,
  });

  /// Detected document type.
  final String type;

  /// Confidence score from 0.0 to 1.0.
  final double confidence;

  /// Number of pages analyzed.
  final int pageCount;
}
