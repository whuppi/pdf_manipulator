// Typed enums — no magic numbers anywhere.

/// PDF encryption algorithm.
enum PdfEncryptionAlgorithm {
  /// RC4, 40-bit key. Legacy, weak. PDF 1.1+.
  rc4_40,
  /// RC4, 128-bit key. PDF 1.4+.
  rc4_128,
  /// AES, 128-bit key. PDF 1.5+.
  aes128,
  /// AES, 256-bit key. Recommended. PDF 1.7+.
  aes256,
}

/// Text/content extraction format.
enum PdfExtractionFormat {
  /// Auto-detect the best method for the content.
  auto,
  /// Structured text preserving reading order and layout.
  text,
  /// Markdown with headings, lists, tables, images.
  markdown,
  /// HTML with styled elements.
  html,
  /// Flat plain text, no structure.
  plainText,
}

/// Standard stamp annotation type.
enum PdfStampType {
  approved,
  experimental,
  notApproved,
  asIs,
  expired,
  notForPublicRelease,
  confidential,
  final_,
  sold,
  departmental,
  forComment,
  topSecret,
  draft,
  forPublicRelease,
}
