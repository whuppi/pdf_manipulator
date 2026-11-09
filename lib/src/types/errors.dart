/// Typed error hierarchy for PDF operations.
///
/// A [PdfError] is a runtime failure: it reaches the caller through the
/// task, and a `switch` over it is exhaustive. A programmer mistake is an
/// [ArgumentError] or a [StateError], thrown before any I/O and outside this
/// hierarchy.
///
/// ```dart
/// try {
///   final doc = await pdf.open(PdfInput(source));
/// } on PdfPasswordRequired {
///   // prompt user for password
/// } on PdfCorrupted catch (e) {
///   print('Bad PDF: ${e.message}');
/// }
/// ```
sealed class PdfError implements Exception {
  /// Creates a PDF error with the given [message].
  const PdfError(this.message);

  /// Human-readable error description.
  final String message;

  @override
  String toString() => '$runtimeType: $message';
}

/// A file-system or network I/O error during a PDF operation.
class PdfIoFailed extends PdfError {
  /// Creates an I/O error.
  const PdfIoFailed(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// The PDF data is structurally corrupt or unparseable.
class PdfCorrupted extends PdfError {
  /// Creates a corruption error.
  const PdfCorrupted(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// The PDF engine encountered an internal error.
class PdfEngineFailed extends PdfError {
  /// Creates an engine error.
  const PdfEngineFailed(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// A page index was outside the valid range.
class PdfPageOutOfRange extends PdfError {
  /// Creates a page-range error.
  const PdfPageOutOfRange({required this.page, required this.pageCount})
    : super('Page $page out of range ($pageCount pages)');

  /// The requested page index.
  final int page;

  /// Total number of pages in the document.
  final int pageCount;
}

/// A name the operation looks up is not in the document: [kind] says what
/// was looked for (`attachment`, `image`) and [name] is the name that was
/// given.
class PdfNotFound extends PdfError {
  /// Creates a not-found error.
  const PdfNotFound({required this.kind, required this.name})
    : super('No $kind named "$name"');

  /// What was looked for: `attachment` or `image`.
  final String kind;

  /// The name that was looked for.
  final String name;
}

/// The requested operation is not supported by this engine.
class PdfUnsupported extends PdfError {
  /// Creates an unsupported-operation error.
  const PdfUnsupported(super.message);
}

/// The PDF is password-protected and no password was supplied.
class PdfPasswordRequired extends PdfError {
  /// Creates a password-required error.
  const PdfPasswordRequired() : super('PDF is password-protected');
}

/// The supplied password was incorrect.
class PdfWrongPassword extends PdfError {
  /// Creates a wrong-password error.
  const PdfWrongPassword() : super('Incorrect password');
}

/// An encryption or decryption error.
class PdfCryptoFailed extends PdfError {
  /// Creates a crypto error.
  const PdfCryptoFailed(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// An incremental save cannot carry the staged edits: they remove
/// content the original bytes would keep, change page content, or the
/// source is encrypted, repaired or certified against the change. Nothing
/// was written to the sink; save with `PdfSave.full()`
/// instead. Carried on the wire as error kind incrementalRefused,
/// never matched by message text.
class PdfIncrementalRefused extends PdfError {
  /// Creates an incremental-refused error.
  const PdfIncrementalRefused(super.message);
}

/// The operation was cancelled — its `Pdf` instance was disposed, or
/// the task itself was cancelled. Carried on the wire as response
/// status 2, never matched by message text.
class PdfCancelled extends PdfError {
  /// Creates a cancellation error.
  const PdfCancelled() : super('Operation cancelled');
}
