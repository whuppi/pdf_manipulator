import 'package:meta/meta.dart';

/// What `PdfEditor.sanitize` strips from a document. Defaults are what
/// "sanitizer" means: strip metadata and scripts, keep attachments unless
/// asked.
@immutable
class PdfSanitizeOptions {
  /// Creates sanitize options.
  const PdfSanitizeOptions({
    this.metadata = true,
    this.javascript = true,
    this.embeddedFiles = false,
  });

  /// Strip document/XMP/image metadata.
  final bool metadata;

  /// Remove document/field JavaScript and `/OpenAction`/`/AA`.
  final bool javascript;

  /// Remove embedded files and file attachment annotations.
  final bool embeddedFiles;
}
