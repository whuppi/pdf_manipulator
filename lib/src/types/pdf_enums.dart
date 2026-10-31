// Typed enums — no magic numbers anywhere.

import 'package:pdf_manipulator/src/types/pdf_wire.dart';

/// I/O mode — how the engine reads source bytes and writes output.
///
/// Detected automatically on first operation.
/// User can force a web mode via `PdfConfig.webIoMode`.
enum PdfIoMode implements PdfWire {
  /// Native FFI via dart:ffi. macOS, iOS, Android, Linux, Windows.
  native('native'),

  /// JSPI — WebAssembly promise suspension. True streaming, no headers.
  /// Chrome 137+, Firefox 139+.
  jspi('jspi'),

  /// Atomics — SharedArrayBuffer + Atomics.wait/notify. True streaming.
  /// Requires COOP/COEP headers.
  atomics('atomics'),

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
  opfs('opfs');

  const PdfIoMode(this.wire);

  @override
  final String wire;
}

/// PDF encryption algorithm.
enum PdfEncryptionAlgorithm implements PdfWire {
  /// RC4, 40-bit key. Legacy, weak. PDF 1.1+.
  rc4_40('rc4_40'),

  /// RC4, 128-bit key. PDF 1.4+.
  rc4_128('rc4_128'),

  /// AES, 128-bit key. PDF 1.5+.
  aes128('aes128'),

  /// AES, 256-bit key. Recommended. PDF 1.7+.
  aes256('aes256');

  const PdfEncryptionAlgorithm(this.wire);

  @override
  final String wire;
}

/// Text/content extraction format.
enum PdfExtractionFormat implements PdfWire {
  /// Auto-detect the best method for the content.
  auto('auto'),

  /// Structured text preserving reading order and layout.
  text('text'),

  /// Markdown with headings, lists, tables, images.
  markdown('markdown'),

  /// HTML with styled elements.
  html('html'),

  /// Flat plain text, no structure.
  plainText('plainText');

  const PdfExtractionFormat(this.wire);

  @override
  final String wire;
}

/// Office document format for conversion.
enum PdfDocumentFormat implements PdfWire {
  /// Microsoft Word (.docx).
  docx('docx'),

  /// Microsoft PowerPoint (.pptx).
  pptx('pptx'),

  /// Microsoft Excel (.xlsx).
  xlsx('xlsx');

  const PdfDocumentFormat(this.wire);

  @override
  final String wire;
}

/// Standard stamp annotation type.
enum PdfStampType implements PdfWire {
  /// Approved.
  approved('Approved'),

  /// Experimental.
  experimental('Experimental'),

  /// Not approved.
  notApproved('NotApproved'),

  /// As-is.
  asIs('AsIs'),

  /// Expired.
  expired('Expired'),

  /// Not for public release.
  notForPublicRelease('NotForPublicRelease'),

  /// Confidential.
  confidential('Confidential'),

  /// Final.
  final_('Final'),

  /// Sold.
  sold('Sold'),

  /// Departmental.
  departmental('Departmental'),

  /// For comment.
  forComment('ForComment'),

  /// Top secret.
  topSecret('TopSecret'),

  /// Draft.
  draft('Draft'),

  /// For public release.
  forPublicRelease('ForPublicRelease');

  const PdfStampType(this.wire);

  @override
  final String wire;
}

/// Which fallback font a `Pdf.registerFallbackFont` call supplies.
enum PdfFallbackFontKind implements PdfWire {
  /// CJK and other non-Latin text (a broad-coverage face, e.g. Droid Sans
  /// Fallback or Noto Sans CJK).
  cjk('cjk'),

  /// Emoji (rendered monochrome when baked into appearances).
  emoji('emoji');

  const PdfFallbackFontKind(this.wire);

  @override
  final String wire;
}
