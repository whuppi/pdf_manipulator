// Typed parameter groups — no loose parameters.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/equality.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// RGB color for watermarks and annotations.
final class PdfColor {
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

  /// Yellow (1, 1, 0).
  static const yellow = PdfColor(1, 1, 0);

  @override
  bool operator ==(Object other) =>
      other is PdfColor && other.r == r && other.g == g && other.b == b;

  @override
  int get hashCode => Object.hash(r, g, b);

  @override
  String toString() => 'PdfColor($r, $g, $b)';
}

/// Permission flags for encrypted PDFs.
final class PdfPermissions {
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

  @override
  bool operator ==(Object other) =>
      other is PdfPermissions && other.toBits() == toBits();

  @override
  int get hashCode => toBits();
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
final class PdfEncryptionKeep extends PdfEncryption {
  /// Creates a keep-encryption intent.
  const PdfEncryptionKeep();

  @override
  bool operator ==(Object other) => other is PdfEncryptionKeep;

  @override
  int get hashCode => (PdfEncryptionKeep).hashCode;
}

/// Remove all encryption.
final class PdfEncryptionRemove extends PdfEncryption {
  /// Creates a remove-encryption intent.
  const PdfEncryptionRemove();

  @override
  bool operator ==(Object other) => other is PdfEncryptionRemove;

  @override
  int get hashCode => (PdfEncryptionRemove).hashCode;
}

/// Apply new encryption with explicit parameters.
final class PdfEncryptionConfig extends PdfEncryption {
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

  @override
  bool operator ==(Object other) =>
      other is PdfEncryptionConfig &&
      other.ownerPassword == ownerPassword &&
      other.userPassword == userPassword &&
      other.algorithm == algorithm &&
      other.permissions == permissions;

  @override
  int get hashCode =>
      Object.hash(ownerPassword, userPassword, algorithm, permissions);
}

/// Save strategy — sealed so invalid combos are unrepresentable.
sealed class PdfSaveOptions {
  const PdfSaveOptions();

  /// Full rewrite — rebuild the entire PDF. Supports compression,
  /// garbage collection, and encryption changes. Objects are packed into
  /// object streams with a cross-reference stream (PDF 1.5) unless
  /// [objectStreams] is false. A stream with no filter or a single
  /// FlateDecode is deflated again at [deflateLevel] and written so only when
  /// that is smaller than the bytes the source held. With [mergeDuplicates]
  /// the save writes one copy of each set of identical images, ICC profiles
  /// and embedded fonts. With [pruneResources] the save drops the entries of
  /// each page's resource dictionaries that no content names.
  const factory PdfSaveOptions.fullRewrite({
    bool deflate,
    bool garbageCollect,
    bool objectStreams,
    int deflateLevel,
    bool mergeDuplicates,
    bool pruneResources,
    PdfEncryption encryption,
  }) = PdfSaveFullRewrite;

  /// Incremental — the original bytes, unchanged, then the changed
  /// objects appended (ISO 32000-1 §7.5.6). A digital signature over the
  /// original bytes stays valid.
  ///
  /// It throws [PdfIncrementalRefused], writing nothing, when an append
  /// cannot carry the edits: pages were removed, reordered or merged in;
  /// content was redacted, scrubbed, drawn, erased or flattened; form
  /// fields or annotations were added, removed or changed beyond a value;
  /// files were embedded; the source is encrypted, was repaired on open,
  /// or carries a certifying signature that forbids the change. Removed
  /// content would survive in the original bytes, so save those with
  /// [PdfSaveOptions.fullRewrite].
  const factory PdfSaveOptions.incremental() = PdfSaveIncremental;
}

/// Full-rewrite save with compression, GC, and encryption control.
final class PdfSaveFullRewrite extends PdfSaveOptions {
  /// Creates full-rewrite save options.
  const PdfSaveFullRewrite({
    this.deflate = true,
    this.garbageCollect = true,
    this.objectStreams = true,
    this.deflateLevel = 9,
    this.mergeDuplicates = false,
    this.pruneResources = false,
    this.encryption = const PdfEncryption.keep(),
  });

  /// Whether to deflate streams.
  final bool deflate;

  /// Whether to remove unreferenced objects.
  final bool garbageCollect;

  /// Whether to pack the non-stream objects into object streams and write
  /// a cross-reference stream (PDF 1.5) instead of a classic table.
  final bool objectStreams;

  /// The zlib level, 0 (stored) to 9 (smallest), of every stream the save
  /// deflates. Saving with a level outside that range throws a
  /// [PdfInvalidArgument].
  final int deflateLevel;

  /// Whether to write one copy of each set of identical image XObjects,
  /// ICC profiles and embedded font programs, across the document and
  /// every merged source, and point every reference at it. Two streams are
  /// one when their dictionaries and encoded bytes are equal; a stream with
  /// `/OC`, `/StructParent` or `/StructParents` is never merged. A stream of
  /// an encrypted source is merged only when its data is at most 256 KiB.
  final bool mergeDuplicates;

  /// Whether to drop, from each resource dictionary of a kept or merged
  /// page, the fonts, images, color spaces, graphics states, patterns,
  /// shadings and property lists that no content using the dictionary
  /// names, and with [garbageCollect] the objects only those entries
  /// reached. A dictionary whose content cannot be read in full, or that
  /// another object also points at, stays whole, and so do the resources of
  /// a page the save edits. The form's default resources are never touched.
  final bool pruneResources;

  /// Encryption intent for the output.
  final PdfEncryption encryption;

  @override
  bool operator ==(Object other) =>
      other is PdfSaveFullRewrite &&
      other.deflate == deflate &&
      other.garbageCollect == garbageCollect &&
      other.objectStreams == objectStreams &&
      other.deflateLevel == deflateLevel &&
      other.mergeDuplicates == mergeDuplicates &&
      other.pruneResources == pruneResources &&
      other.encryption == encryption;

  @override
  int get hashCode => Object.hash(
    deflate,
    garbageCollect,
    objectStreams,
    deflateLevel,
    mergeDuplicates,
    pruneResources,
    encryption,
  );
}

/// Incremental save — see [PdfSaveOptions.incremental] for what it writes.
final class PdfSaveIncremental extends PdfSaveOptions {
  /// Creates incremental save options.
  const PdfSaveIncremental();

  @override
  bool operator ==(Object other) => other is PdfSaveIncremental;

  @override
  int get hashCode => (PdfSaveIncremental).hashCode;
}

/// Watermark text style.
final class PdfWatermarkStyle {
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

  /// Text angle in degrees (not a page rotation).
  final double rotation;

  /// Text color.
  final PdfColor color;

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkStyle &&
      other.fontSize == fontSize &&
      other.fontName == fontName &&
      other.opacity == opacity &&
      other.rotation == rotation &&
      other.color == color;

  @override
  int get hashCode => Object.hash(fontSize, fontName, opacity, rotation, color);
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
final class PdfWatermarkCenter extends PdfWatermarkPosition {
  /// Creates a center-positioned watermark.
  const PdfWatermarkCenter();

  @override
  bool operator ==(Object other) => other is PdfWatermarkCenter;

  @override
  int get hashCode => (PdfWatermarkCenter).hashCode;
}

/// Corner-anchored watermark position.
final class PdfWatermarkCorner extends PdfWatermarkPosition {
  /// Creates a corner-anchored watermark.
  const PdfWatermarkCorner(this.corner, {this.marginX = 20, this.marginY = 20});

  /// Which corner to anchor to.
  final PdfCorner corner;

  /// Horizontal margin from the corner in points.
  final double marginX;

  /// Vertical margin from the corner in points.
  final double marginY;

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkCorner &&
      other.corner == corner &&
      other.marginX == marginX &&
      other.marginY == marginY;

  @override
  int get hashCode => Object.hash(corner, marginX, marginY);
}

/// Tiled watermark position.
final class PdfWatermarkTiled extends PdfWatermarkPosition {
  /// Creates a tiled watermark.
  const PdfWatermarkTiled({this.columns = 3, this.rows = 4});

  /// Number of columns in the tile grid.
  final int columns;

  /// Number of rows in the tile grid.
  final int rows;

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkTiled &&
      other.columns == columns &&
      other.rows == rows;

  @override
  int get hashCode => Object.hash(columns, rows);
}

/// Exact-coordinate watermark position.
final class PdfWatermarkExact extends PdfWatermarkPosition {
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

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkExact &&
      other.x == x &&
      other.y == y &&
      other.width == width &&
      other.height == height;

  @override
  int get hashCode => Object.hash(x, y, width, height);
}

/// Which corner to anchor a watermark.
enum PdfCorner implements PdfWire {
  /// Top-left corner.
  topLeft('topLeft'),

  /// Top-right corner.
  topRight('topRight'),

  /// Bottom-left corner.
  bottomLeft('bottomLeft'),

  /// Bottom-right corner.
  bottomRight('bottomRight');

  const PdfCorner(this.wire);

  @override
  final String wire;
}

/// Whether the watermark renders above or below page content.
enum PdfWatermarkLayer implements PdfWire {
  /// Renders on top of page content (annotation-based). Default.
  foreground('foreground'),

  /// Renders behind page content (content-stream, prepended in painting order).
  background('background');

  const PdfWatermarkLayer(this.wire);

  @override
  final String wire;
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
final class PdfPkcs12Credentials extends PdfSigningCredentials {
  /// Creates PKCS#12 credentials.
  const PdfPkcs12Credentials(this.data, this.password);

  /// The raw .p12 / .pfx bytes.
  final Uint8List data;

  /// Password to unlock the bundle.
  final String password;

  @override
  bool operator ==(Object other) =>
      other is PdfPkcs12Credentials &&
      other.password == password &&
      listsEqual(other.data, data);

  @override
  int get hashCode => Object.hash(password, Object.hashAll(data));
}

/// PEM-encoded signing credentials.
final class PdfPemCredentials extends PdfSigningCredentials {
  /// Creates PEM credentials.
  const PdfPemCredentials(this.certPem, this.keyPem);

  /// PEM-encoded certificate.
  final String certPem;

  /// PEM-encoded private key.
  final String keyPem;

  @override
  bool operator ==(Object other) =>
      other is PdfPemCredentials &&
      other.certPem == certPem &&
      other.keyPem == keyPem;

  @override
  int get hashCode => Object.hash(certPem, keyPem);
}

/// Validation result.
final class PdfValidationResult {
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

  @override
  bool operator ==(Object other) =>
      other is PdfValidationResult &&
      other.compliant == compliant &&
      other.errors == errors &&
      other.warnings == warnings;

  @override
  int get hashCode => Object.hash(compliant, errors, warnings);
}

/// A bookmark's page range: the pages under one top-level bookmark.
final class PdfBookmarkRange {
  /// Creates a bookmark range.
  const PdfBookmarkRange({
    required this.title,
    required this.start,
    required this.end,
  });

  /// The bookmark's title, or null when it has none.
  final String? title;

  /// First page of the range (inclusive).
  final int start;

  /// Last page of the range (exclusive).
  final int end;

  @override
  bool operator ==(Object other) =>
      other is PdfBookmarkRange &&
      other.title == title &&
      other.start == start &&
      other.end == end;

  @override
  int get hashCode => Object.hash(title, start, end);
}

/// What one page holds, as the engine classified it.
final class PdfPageClassification {
  /// Creates a page classification.
  const PdfPageClassification({required this.page, required this.type});

  /// Zero-based page index.
  final int page;

  /// What the page holds.
  final PdfPageType type;

  @override
  bool operator ==(Object other) =>
      other is PdfPageClassification &&
      other.page == page &&
      other.type == type;

  @override
  int get hashCode => Object.hash(page, type);
}
