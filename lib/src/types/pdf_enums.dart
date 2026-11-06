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

  /// Asyncify — the engine wasm is instrumented to pause at a source read or
  /// a sink write and resume when the answer arrives, so it streams like
  /// [jspi] on a browser with neither JSPI nor `SharedArrayBuffer`. Needs
  /// the Asyncify wasm (`asyncify: true` in the app's
  /// `hooks: user_defines: pdf_manipulator:`); its larger parts are fetched
  /// only in this mode, and auto-detection picks it only where [opfs] cannot
  /// run.
  asyncify('asyncify'),

  /// OPFS — pre-copy entire source to Origin Private File System disk
  /// before processing. Universal web fallback, all modern browsers.
  ///
  /// **Trade-offs vs streaming modes (JSPI/Atomics/Asyncify):**
  /// - O(N) disk space (full copy of each source file)
  /// - O(N) latency before first output byte (copy must complete first)
  /// - Subject to browser storage quota
  /// - No sink backpressure (writes are fire-and-forget)
  ///
  /// If you tested locally with COOP/COEP headers (getting Atomics) or
  /// on Chrome 137+ (getting JSPI) and deploy without headers on an
  /// older browser, users silently fall back to OPFS, unless the app ships
  /// the Asyncify wasm. Use `ensureInitialized` to detect the mode and warn
  /// if needed.
  opfs('opfs');

  const PdfIoMode(this.wire);

  @override
  final String wire;
}

/// PDF encryption algorithm.
enum PdfEncryptionAlgorithm implements PdfWire {
  /// RC4, 40-bit key. Legacy, weak. PDF 1.1+.
  rc4Key40('rc4_40'),

  /// RC4, 128-bit key. PDF 1.4+.
  rc4Key128('rc4_128'),

  /// AES, 128-bit key. PDF 1.5+.
  aesKey128('aes128'),

  /// AES, 256-bit key. Recommended. PDF 1.7+.
  aesKey256('aes256');

  const PdfEncryptionAlgorithm(this.wire);

  @override
  final String wire;
}

/// Text/content extraction format.
enum PdfExtractionFormat implements PdfWire {
  /// Auto-detect the best method for the content.
  auto('auto'),

  /// Structured text preserving reading order and layout.
  layout('text'),

  /// Markdown with headings, lists, tables, images.
  markdown('markdown'),

  /// HTML with styled elements.
  html('html'),

  /// Flat plain text, no structure.
  plain('plainText');

  const PdfExtractionFormat(this.wire);

  @override
  final String wire;
}

/// Office document format for conversion.
enum PdfOfficeFormat implements PdfWire {
  /// Microsoft Word (.docx).
  docx('docx'),

  /// Microsoft PowerPoint (.pptx).
  pptx('pptx'),

  /// Microsoft Excel (.xlsx).
  xlsx('xlsx');

  const PdfOfficeFormat(this.wire);

  @override
  final String wire;
}

/// Standard stamp annotation type.
enum PdfStamp implements PdfWire {
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
  finalVersion('Final'),

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

  const PdfStamp(this.wire);

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

/// A page rotation: a quarter turn clockwise, as the page's `/Rotate` stores
/// it.
enum PdfRotation implements PdfWire {
  /// Upright.
  r0('r0'),

  /// A quarter turn clockwise.
  r90('r90'),

  /// A half turn.
  r180('r180'),

  /// Three quarter turns clockwise.
  r270('r270');

  const PdfRotation(this.wire);

  @override
  final String wire;
}

/// What a page holds, as the engine classifies it.
enum PdfPageKind implements PdfWire {
  /// Usable native text dominates.
  textLayer('TextLayer'),

  /// The page is image-dominated, with no or garbled text.
  scanned('Scanned'),

  /// Native text and image regions that hold text.
  imageText('ImageText'),

  /// The page mixes text with an image table or figure.
  mixed('Mixed'),

  /// The page is blank or near-empty.
  empty('Empty');

  const PdfPageKind(this.wire);

  @override
  final String wire;
}

/// What a document holds, as the engine summarises its pages.
enum PdfDocumentKind implements PdfWire {
  /// The document is predominantly born-digital text.
  mostlyText('MostlyText'),

  /// The document is predominantly scanned pages.
  mostlyScanned('MostlyScanned'),

  /// The document mixes text and scanned pages.
  mixed('Mixed'),

  /// The document has no meaningful content.
  empty('Empty');

  const PdfDocumentKind(this.wire);

  @override
  final String wire;
}

/// A PDF/A conformance level. The engine validates and converts to the
/// level's basic (`b`) conformance.
enum PdfALevel implements PdfWire {
  /// PDF/A-1b (ISO 19005-1).
  a1('a1'),

  /// PDF/A-2b (ISO 19005-2).
  a2('a2'),

  /// PDF/A-3b (ISO 19005-3).
  a3('a3');

  const PdfALevel(this.wire);

  @override
  final String wire;
}

/// A PDF/UA accessibility level.
enum PdfUaLevel implements PdfWire {
  /// PDF/UA-1 (ISO 14289-1).
  ua1('ua1'),

  /// PDF/UA-2 (ISO 14289-2).
  ua2('ua2');

  const PdfUaLevel(this.wire);

  @override
  final String wire;
}

/// A markup annotation drawn over the last-written text.
enum PdfMarkupKind implements PdfWire {
  /// A highlight behind the text.
  highlight('highlight'),

  /// A line under the text.
  underline('underline'),

  /// A line through the text.
  strikeout('strikeout'),

  /// A wavy line under the text.
  squiggly('squiggly');

  const PdfMarkupKind(this.wire);

  @override
  final String wire;
}

/// The horizontal alignment of text in a cell, a rectangle or a field.
enum PdfAlignment implements PdfWire {
  /// Aligned to the left edge.
  left('left'),

  /// Centred.
  center('center'),

  /// Aligned to the right edge.
  right('right');

  const PdfAlignment(this.wire);

  @override
  final String wire;
}

/// How an image fills the page it is placed on.
enum PdfImageFit implements PdfWire {
  /// The whole image fits inside the page, centred; the page keeps the
  /// space around it.
  contain('contain'),

  /// The page is filled; the image overflows the page's edges, which
  /// viewers clip.
  cover('cover'),

  /// The image is stretched to the page's edges, ignoring its own shape.
  fill('fill');

  const PdfImageFit(this.wire);

  @override
  final String wire;
}
