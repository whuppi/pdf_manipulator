/// Typed error hierarchy for PDF operations.
///
/// ```dart
/// try {
///   final doc = await Pdf.open(bytes);
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

/// An argument passed to a PDF operation was invalid.
class PdfInvalidArgument extends PdfError {
  /// Creates an invalid-argument error.
  const PdfInvalidArgument(super.message);
}

/// A file-system or network I/O error during a PDF operation.
class PdfIoError extends PdfError {
  /// Creates an I/O error.
  const PdfIoError(super.message, {this.cause});

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

/// Text or content extraction failed.
class PdfExtractionFailed extends PdfError {
  /// Creates an extraction-failure error.
  const PdfExtractionFailed(super.message);
}

/// The PDF engine encountered an internal error.
class PdfEngineError extends PdfError {
  /// Creates an engine error.
  const PdfEngineError(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// A page index was outside the valid range.
class PdfPageRangeError extends PdfError {
  /// Creates a page-range error.
  const PdfPageRangeError({required this.page, required this.pageCount})
      : super('Page $page out of range ($pageCount pages)');

  /// The requested page index.
  final int page;

  /// Total number of pages in the document.
  final int pageCount;
}

/// A text search operation failed.
class PdfSearchError extends PdfError {
  /// Creates a search error.
  const PdfSearchError(super.message);
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
class PdfCryptoError extends PdfError {
  /// Creates a crypto error.
  const PdfCryptoError(super.message, {this.cause});

  /// The underlying exception, if any.
  final Object? cause;
}

/// Operation was cancelled (dispose called during execution).
class PdfCancelled extends PdfError {
  /// Creates a cancellation error.
  const PdfCancelled() : super('Operation cancelled');
}
