import 'package:meta/meta.dart';

/// What a destructive redaction pass actually removed, so the caller can
/// assert the redaction did real work instead of trusting a silent no-op.
@immutable
final class PdfRedactionReport {
  /// Creates a report from the engine's counts.
  const PdfRedactionReport({
    required this.regions,
    required this.glyphsRemoved,
    required this.imagesModified,
    required this.imagesRemoved,
    required this.pathsPruned,
    required this.xobjectsSpecialized,
  });

  /// Number of redaction regions applied.
  final int regions;

  /// Glyphs physically removed from content streams.
  final int glyphsRemoved;

  /// Images whose covered pixels were overwritten and re-encoded.
  final int imagesModified;

  /// Images deleted entirely because they were fully covered.
  final int imagesRemoved;

  /// Path subpaths dropped or geometry-clipped.
  final int pathsPruned;

  /// Shared XObjects, patterns or Type3 fonts cloned and specialized so the
  /// redaction did not leak into an unredacted sibling that shared them.
  final int xobjectsSpecialized;

  @override
  bool operator ==(Object other) =>
      other is PdfRedactionReport &&
      other.regions == regions &&
      other.glyphsRemoved == glyphsRemoved &&
      other.imagesModified == imagesModified &&
      other.imagesRemoved == imagesRemoved &&
      other.pathsPruned == pathsPruned &&
      other.xobjectsSpecialized == xobjectsSpecialized;

  @override
  int get hashCode => Object.hash(
    regions,
    glyphsRemoved,
    imagesModified,
    imagesRemoved,
    pathsPruned,
    xobjectsSpecialized,
  );
}
