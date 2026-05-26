/// Typed error hierarchy for PDF operations.
///
/// Maps 1:1 to pdf_oxide's C API error codes:
/// `0=success, 1=invalid arg, 2=IO, 3=parse, 4=extraction,
///  5=internal, 6=invalid page, 7=search, 8=unsupported`
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
  final String message;
  const PdfError(this.message);

  @override
  String toString() => '$runtimeType: $message';
}

class PdfInvalidArgument extends PdfError {
  const PdfInvalidArgument(super.message);
}

class PdfIoError extends PdfError {
  final Object? cause;
  const PdfIoError(super.message, {this.cause});
}

class PdfCorrupted extends PdfError {
  final Object? cause;
  const PdfCorrupted(super.message, {this.cause});
}

class PdfExtractionFailed extends PdfError {
  const PdfExtractionFailed(super.message);
}

class PdfEngineError extends PdfError {
  final Object? cause;
  const PdfEngineError(super.message, {this.cause});
}

class PdfPageRangeError extends PdfError {
  final int page;
  final int pageCount;
  const PdfPageRangeError({required this.page, required this.pageCount})
      : super('Page $page out of range ($pageCount pages)');
}

class PdfSearchError extends PdfError {
  const PdfSearchError(super.message);
}

class PdfUnsupported extends PdfError {
  const PdfUnsupported(super.message);
}

class PdfPasswordRequired extends PdfError {
  const PdfPasswordRequired() : super('PDF is password-protected');
}

class PdfWrongPassword extends PdfError {
  const PdfWrongPassword() : super('Incorrect password');
}

class PdfCryptoError extends PdfError {
  final Object? cause;
  const PdfCryptoError(super.message, {this.cause});
}

/// Operation was cancelled (dispose called during execution).
class PdfCancelled extends PdfError {
  const PdfCancelled() : super('Operation cancelled');
}
