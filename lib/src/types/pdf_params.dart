// Typed parameter groups — no loose parameters.

import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
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
    this.canPrint = true,
    this.canPrintHighQuality = true,
    this.canModify = true,
    this.canCopy = true,
    this.canAnnotate = true,
    this.canFillForms = true,
    this.canExtractForAccessibility = true,
    this.canAssemble = true,
  });

  /// All permissions granted.
  const PdfPermissions.all() : this();

  /// Read-only — most permissions revoked except accessibility.
  const PdfPermissions.readOnly()
    : canPrint = false,
      canPrintHighQuality = false,
      canModify = false,
      canCopy = false,
      canAnnotate = false,
      canFillForms = false,
      canExtractForAccessibility = true,
      canAssemble = false;

  /// Whether printing at standard quality is allowed.
  final bool canPrint;

  /// Whether high-quality printing is allowed.
  final bool canPrintHighQuality;

  /// Whether the document may be modified.
  final bool canModify;

  /// Whether content may be copied.
  final bool canCopy;

  /// Whether annotations may be added.
  final bool canAnnotate;

  /// Whether form fields may be filled.
  final bool canFillForms;

  /// Whether text may be extracted for accessibility.
  final bool canExtractForAccessibility;

  /// Whether pages may be assembled (inserted, deleted, rotated).
  final bool canAssemble;

  /// The permission flags as a signed 32-bit int per the PDF spec.
  @internal
  int get bits {
    // Build as signed 32-bit int. The reserved bits (0xFFFFF0C0) set the
    // upper bits per PDF spec. Using .toSigned(32) ensures the value fits
    // in the binary codec's i32 type on both VM and web (dart2js has no i64).
    int bits = 0xFFFFF0C0;
    if (canPrint) bits |= 1 << 2;
    if (canModify) bits |= 1 << 3;
    if (canCopy) bits |= 1 << 4;
    if (canAnnotate) bits |= 1 << 5;
    if (canFillForms) bits |= 1 << 8;
    if (canExtractForAccessibility) bits |= 1 << 9;
    if (canAssemble) bits |= 1 << 10;
    if (canPrintHighQuality) bits |= 1 << 11;
    return bits.toSigned(32);
  }

  @override
  bool operator ==(Object other) =>
      other is PdfPermissions && other.bits == bits;

  @override
  int get hashCode => bits;
}

/// Encryption intent on save — sealed, compiler-enforced.
sealed class PdfEncryption {
  const PdfEncryption();

  /// Preserve the source PDF's existing encryption.
  /// If the source was not encrypted, output is not encrypted.
  const factory PdfEncryption.keep() = PdfEncryptionKeep;

  /// Strip all encryption. Output is plaintext.
  const factory PdfEncryption.remove() = PdfEncryptionRemove;

  /// Encrypts the output with passwords. [userPassword] null means the
  /// document opens without one.
  const factory PdfEncryption.password({
    required String ownerPassword,
    String? userPassword,
    PdfEncryptionAlgorithm algorithm,
    PdfPermissions permissions,
  }) = PdfEncryptionPassword;
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

/// Encrypt the output with passwords.
final class PdfEncryptionPassword extends PdfEncryption {
  /// Creates the intent to encrypt with passwords.
  const PdfEncryptionPassword({
    required this.ownerPassword,
    this.userPassword,
    this.algorithm = PdfEncryptionAlgorithm.aesKey256,
    this.permissions = const PdfPermissions.all(),
  });

  /// Owner password (full access).
  final String ownerPassword;

  /// User password (restricted by [permissions]), or null for a document
  /// that opens without one.
  final String? userPassword;

  /// Encryption algorithm to use.
  final PdfEncryptionAlgorithm algorithm;

  /// Access permissions for the user password.
  final PdfPermissions permissions;

  @override
  bool operator ==(Object other) =>
      other is PdfEncryptionPassword &&
      other.ownerPassword == ownerPassword &&
      other.userPassword == userPassword &&
      other.algorithm == algorithm &&
      other.permissions == permissions;

  @override
  int get hashCode =>
      Object.hash(ownerPassword, userPassword, algorithm, permissions);
}

/// How a document is written back, sealed so an invalid combination cannot
/// be built.
sealed class PdfSave {
  const PdfSave();

  /// Full save — rebuild the entire PDF. Supports compression, garbage
  /// collection, and encryption changes. Objects are packed into object
  /// streams with a cross-reference stream (PDF 1.5) unless [useObjectStreams]
  /// is false. A stream with no filter or a single FlateDecode is deflated
  /// again at [deflate] and written so only when that is smaller than the
  /// bytes the source held. With [mergeDuplicates] the save writes one copy of
  /// each set of identical images, ICC profiles and embedded fonts. With
  /// [pruneResources] the save drops the entries of each page's resource
  /// dictionaries that no content names.
  const factory PdfSave.full({
    PdfDeflate deflate,
    bool garbageCollect,
    bool useObjectStreams,
    bool mergeDuplicates,
    bool pruneResources,
    PdfEncryption encryption,
  }) = PdfSaveFull;

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
  /// [PdfSave.full] or [PdfSave.incrementalOrRewrite].
  const factory PdfSave.incremental() = PdfSaveIncremental;

  /// Incremental when an append can carry the edits, a full save with the
  /// given fields when [PdfIncrementalRefused] says it cannot. The fallback
  /// breaks a signature over the original bytes, as any full save does: a
  /// caller who must keep one uses [PdfSave.incremental].
  const factory PdfSave.incrementalOrRewrite({
    PdfDeflate deflate,
    bool garbageCollect,
    bool useObjectStreams,
    bool mergeDuplicates,
    bool pruneResources,
    PdfEncryption encryption,
  }) = PdfSaveIncrementalOrFull;
}

/// How hard a full save deflates the streams it writes.
enum PdfDeflate implements PdfWire {
  /// Streams are written stored, without compression.
  none('none'),

  /// Fastest deflate, the largest compressed output.
  fast('fast'),

  /// A balance of speed and size.
  balanced('balanced'),

  /// Smallest output, slowest save. The default.
  best('best');

  const PdfDeflate(this.wire);

  @override
  final String wire;
}

/// [PdfSave.full].
final class PdfSaveFull extends PdfSave {
  /// Creates full-save options.
  const PdfSaveFull({
    this.deflate = PdfDeflate.best,
    this.garbageCollect = true,
    this.useObjectStreams = true,
    this.mergeDuplicates = false,
    this.pruneResources = false,
    this.encryption = const PdfEncryption.keep(),
  });

  /// How hard the save deflates its streams.
  final PdfDeflate deflate;

  /// Whether to remove unreferenced objects.
  final bool garbageCollect;

  /// Whether to pack the non-stream objects into object streams and write
  /// a cross-reference stream (PDF 1.5) instead of a classic table.
  final bool useObjectStreams;

  /// Whether to write one copy of each set of identical image XObjects,
  /// ICC profiles and embedded font programs, across the document and
  /// every merged source, and point every reference at it. Two streams are
  /// one when their dictionaries and encoded bytes are equal; a stream with
  /// `/OC`, `/StructParent` or `/StructParents` is never merged. A stream of
  /// an encrypted source is compared decrypted, read in small pieces.
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
      other is PdfSaveFull &&
      other.deflate == deflate &&
      other.garbageCollect == garbageCollect &&
      other.useObjectStreams == useObjectStreams &&
      other.mergeDuplicates == mergeDuplicates &&
      other.pruneResources == pruneResources &&
      other.encryption == encryption;

  @override
  int get hashCode => Object.hash(
    deflate,
    garbageCollect,
    useObjectStreams,
    mergeDuplicates,
    pruneResources,
    encryption,
  );
}

/// [PdfSave.incremental].
final class PdfSaveIncremental extends PdfSave {
  /// Creates incremental save options.
  const PdfSaveIncremental();

  @override
  bool operator ==(Object other) => other is PdfSaveIncremental;

  @override
  int get hashCode => (PdfSaveIncremental).hashCode;
}

/// [PdfSave.incrementalOrRewrite].
final class PdfSaveIncrementalOrFull extends PdfSave {
  /// Creates the incremental-or-full save options.
  const PdfSaveIncrementalOrFull({
    this.deflate = PdfDeflate.best,
    this.garbageCollect = true,
    this.useObjectStreams = true,
    this.mergeDuplicates = false,
    this.pruneResources = false,
    this.encryption = const PdfEncryption.keep(),
  });

  /// The [PdfSaveFull.deflate] of the fallback save.
  final PdfDeflate deflate;

  /// The [PdfSaveFull.garbageCollect] of the fallback save.
  final bool garbageCollect;

  /// The [PdfSaveFull.useObjectStreams] of the fallback save.
  final bool useObjectStreams;

  /// The [PdfSaveFull.mergeDuplicates] of the fallback save.
  final bool mergeDuplicates;

  /// The [PdfSaveFull.pruneResources] of the fallback save.
  final bool pruneResources;

  /// The [PdfSaveFull.encryption] of the fallback save.
  final PdfEncryption encryption;

  /// The full save written when an incremental one is refused.
  PdfSaveFull get fallback => PdfSaveFull(
    deflate: deflate,
    garbageCollect: garbageCollect,
    useObjectStreams: useObjectStreams,
    mergeDuplicates: mergeDuplicates,
    pruneResources: pruneResources,
    encryption: encryption,
  );

  @override
  bool operator ==(Object other) =>
      other is PdfSaveIncrementalOrFull && other.fallback == fallback;

  @override
  int get hashCode => Object.hash(PdfSaveIncrementalOrFull, fallback);
}

/// Watermark text style.
final class PdfWatermarkStyle {
  /// Creates a watermark text style.
  const PdfWatermarkStyle({
    this.fontSize = 48,
    this.fontName,
    this.opacity = 0.3,
    this.angleDegrees = 45,
    this.color = PdfColor.gray,
  });

  /// Font size in points.
  final double fontSize;

  /// Optional font name. Uses a default sans-serif if null.
  final String? fontName;

  /// Opacity from 0.0 (invisible) to 1.0 (opaque).
  final double opacity;

  /// Text angle in degrees (not a page rotation).
  final double angleDegrees;

  /// Text color.
  final PdfColor color;

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkStyle &&
      other.fontSize == fontSize &&
      other.fontName == fontName &&
      other.opacity == opacity &&
      other.angleDegrees == angleDegrees &&
      other.color == color;

  @override
  int get hashCode =>
      Object.hash(fontSize, fontName, opacity, angleDegrees, color);
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

  /// The rectangle [rect] on the page, in points — the caller computes it
  /// against the page dimensions.
  const factory PdfWatermarkPosition.exact(PdfRect rect) = PdfWatermarkExact;
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

/// Exact-rectangle watermark position.
final class PdfWatermarkExact extends PdfWatermarkPosition {
  /// Creates an exact-position watermark.
  const PdfWatermarkExact(this.rect);

  /// Where the watermark sits, in points.
  final PdfRect rect;

  @override
  bool operator ==(Object other) =>
      other is PdfWatermarkExact && other.rect == rect;

  @override
  int get hashCode => rect.hashCode;
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

  /// PKCS#12 (.p12 / .pfx) [bundle] holding the certificate and private key,
  /// read whole and unlocked with [password]. The caller keeps the source
  /// and closes it.
  const factory PdfSigningCredentials.pkcs12(
    DataSource bundle,
    String password,
  ) = PdfPkcs12Credentials;

  /// Separate PEM-encoded certificate and private key.
  const factory PdfSigningCredentials.pem(String certPem, String keyPem) =
      PdfPemCredentials;
}

/// PKCS#12 signing credentials.
final class PdfPkcs12Credentials extends PdfSigningCredentials {
  /// Creates PKCS#12 credentials.
  const PdfPkcs12Credentials(this.bundle, this.password);

  /// The source of the .p12 / .pfx bytes.
  final DataSource bundle;

  /// Password to unlock the bundle.
  final String password;

  @override
  bool operator ==(Object other) =>
      other is PdfPkcs12Credentials &&
      other.password == password &&
      other.bundle == bundle;

  @override
  int get hashCode => Object.hash(password, bundle);
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

/// What a PDF/A or PDF/UA validation found.
final class PdfValidationReport {
  /// Creates a validation report.
  const PdfValidationReport({
    required this.isCompliant,
    required this.errorCount,
    required this.warningCount,
  });

  /// Whether the document is compliant with the checked standard.
  final bool isCompliant;

  /// Number of validation errors.
  final int errorCount;

  /// Number of validation warnings.
  final int warningCount;

  @override
  bool operator ==(Object other) =>
      other is PdfValidationReport &&
      other.isCompliant == isCompliant &&
      other.errorCount == errorCount &&
      other.warningCount == warningCount;

  @override
  int get hashCode => Object.hash(isCompliant, errorCount, warningCount);
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

  /// Page after the last of the range (exclusive).
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
  const PdfPageClassification({required this.page, required this.kind});

  /// Zero-based page index.
  final int page;

  /// What the page holds.
  final PdfPageKind kind;

  @override
  bool operator ==(Object other) =>
      other is PdfPageClassification &&
      other.page == page &&
      other.kind == kind;

  @override
  int get hashCode => Object.hash(page, kind);
}
