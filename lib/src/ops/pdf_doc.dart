// PdfDocument — read-only queries on a parsed PDF document.
//
// Open once, query many times, dispose when done. The Rust engine
// keeps the parsed PDF in memory (handle map) until dispose is called.
// Read-only — no mutations here. Use PdfEditor for mutations, `Pdf` for
// the ops that need no handle.
//
// PdfDocument is the view a scope body gets; PdfDocumentHandle is the one
// `Pdf.open` returns, and the only one that can be disposed.

part of 'pdf.dart';

/// A parsed PDF document — what a [Pdf.read] body queries.
///
/// The document is parsed once and stays in memory until its owner lets go.
/// All read operations reuse the already-parsed structure — no re-opening.
///
/// The facts the open read anyway are sync fields; everything per page is
/// a read of its own ([pageInfo], [extractText], [search] and the rest,
/// declared on [PdfReads]).
base class PdfDocument extends PdfReads {
  /// Internal constructor — use [Pdf.open] to create instances.
  @internal
  PdfDocument.internal(
    this._handle, {
    required this.pageCount,
    required this.version,
    required this.isTagged,
    required this.metadata,
    required this.encryption,
  }) : super._();

  final BridgeDocHandle _handle;
  bool _disposed = false;

  // ── Metadata (read at open time) ──

  /// Total number of pages in the document.
  final int pageCount;

  /// PDF version string (e.g. "1.7", "2.0").
  final String version;

  /// Whether the document contains tagged (accessible) structure.
  final bool isTagged;

  /// What the document's info dictionary carries.
  final PdfMetadata metadata;

  /// How the document is encrypted, or null when it is not encrypted.
  final PdfEncryptionInfo? encryption;

  @override
  PdfTask<T> _run<T>(
    PdfTask<T> Function(BridgeDocHandle handle) read, {
    DataSink? output,
  }) {
    if (_disposed) throw StateError('This PdfDocument has been disposed');
    return read(_handle);
  }
}

/// A [PdfDocument] you own: the handle [Pdf.open] returns.
///
/// Call [dispose] when you are done, on every path including errors. For one
/// read, use [Pdf.read], which opens the document, runs your code and always
/// closes it. A handle you forget is released when the garbage collector
/// frees it, and in debug mode the leak is logged with where it was opened;
/// that is a backstop, not a plan.
final class PdfDocumentHandle extends PdfDocument implements _Handle {
  /// Internal constructor — use [Pdf.open] to create instances.
  @internal
  PdfDocumentHandle.internal(
    super.handle, {
    required super.pageCount,
    required super.version,
    required super.isTagged,
    required super.metadata,
    required super.encryption,
    required Future<void> Function() release,
  }) : _close = _closing(handle.dispose, release),
       super.internal() {
    HandleBackstop.attach(this, 'PdfDocumentHandle', _close);
  }

  final Future<void> Function() _close;

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

  /// Releases the parsed document, and the source when the input owns it.
  /// Idempotent.
  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    HandleBackstop.detach(this);
    await _close();
  }
}
