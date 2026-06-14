// Typed enums — no magic numbers anywhere.

/// I/O mode — how the engine reads source bytes and writes output.
///
/// Detected automatically on first operation.
/// User can force a web mode via `PdfConfig.webIoMode`.
enum PdfIoMode {
  /// Native FFI via dart:ffi. macOS, iOS, Android, Linux, Windows.
  native,

  /// JSPI — WebAssembly promise suspension. True streaming, no headers.
  /// Chrome 137+, Firefox 139+.
  jspi,

  /// Atomics — SharedArrayBuffer + Atomics.wait/notify. True streaming.
  /// Requires COOP/COEP headers.
  atomics,

  /// OPFS — pre-copy entire source to Origin Private File System disk
  /// before processing. Universal web fallback, all modern browsers.
  ///
  /// **Trade-offs vs streaming modes (JSPI/Atomics):**
  /// - O(N) disk space (full copy of each source file)
  /// - O(N) latency before first output byte (copy must complete first)
  /// - Subject to browser storage quota
  /// - No sink backpressure (writes are fire-and-forget)
  ///
  /// If you tested locally with COOP/COEP headers (getting Atomics) or
  /// on Chrome 137+ (getting JSPI) and deploy without headers on an
  /// older browser, users silently fall back to OPFS. Use
  /// `ensureInitialized` to detect the mode and warn if needed.
  opfs,
}

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

/// Office document format for conversion.
enum PdfDocumentFormat {
  /// Microsoft Word (.docx).
  docx,

  /// Microsoft PowerPoint (.pptx).
  pptx,

  /// Microsoft Excel (.xlsx).
  xlsx,
}

/// Standard stamp annotation type.
enum PdfStampType {
  /// Approved.
  approved,

  /// Experimental.
  experimental,

  /// Not approved.
  notApproved,

  /// As-is.
  asIs,

  /// Expired.
  expired,

  /// Not for public release.
  notForPublicRelease,

  /// Confidential.
  confidential,

  /// Final.
  final_,

  /// Sold.
  sold,

  /// Departmental.
  departmental,

  /// For comment.
  forComment,

  /// Top secret.
  topSecret,

  /// Draft.
  draft,

  /// For public release.
  forPublicRelease,
}
