// Typed errors for the new bridge architecture.
// PdfCancelled is new — thrown when an operation is cancelled via dispose.

/// Operation was cancelled (dispose called during execution).
class PdfCancelled implements Exception {
  const PdfCancelled();
  @override
  String toString() => 'PdfCancelled: Operation cancelled';
}
