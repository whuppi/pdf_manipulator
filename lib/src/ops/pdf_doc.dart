// PdfDoc — read-only queries on a parsed PDF document.
//
// Open once, query many times, dispose when done. The Rust engine
// keeps the parsed PDF in memory (handle map) until dispose is called.
// Read-only — no mutations here. Use PdfEditor for mutations,
// PdfStandalone for one-shot export ops.

part of 'pdf.dart';

/// A parsed PDF document — live handle to the Rust engine.
///
/// Created by [Pdf.open]. The document is parsed once and stays in
/// memory until [dispose] is called. All read operations reuse the
/// already-parsed structure — no re-opening.
///
/// The facts the open read anyway are sync fields; everything per page is
/// a read of its own ([pageInfo], [extractText], [search] and the rest,
/// declared on [PdfReads]).
base class PdfDoc extends PdfReads implements _Handle {
  /// Internal constructor — use [Pdf.open] to create instances.
  @internal
  PdfDoc.internal(
    this._handle, {
    required this.pageCount,
    required this.version,
    required this.isTagged,
    required this.metadata,
    required this.encryption,
  }) : super._() {
    HandleBackstop.attach(this, 'PdfDoc', _handle.dispose);
  }

  final BridgeDocHandle _handle;
  bool _disposed = false;

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

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
    _check();
    return read(_handle);
  }

  void _check() {
    if (_disposed) throw const PdfDisposed('This PdfDoc has been disposed');
  }

  // ── Lifecycle ──

  /// Releases the parsed document handle. Idempotent.
  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    HandleBackstop.detach(this);
    await _handle.dispose();
  }
}
