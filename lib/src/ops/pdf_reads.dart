// PdfReads — every read of a document, declared once.
//
// [PdfDoc] and the object `Pdf.reads` returns both extend it and add only
// `_run`, so a read exists on both doors the moment it compiles. A new read
// is a new member here. Members of PdfDoc that are not declared here are
// listed with their reason in test/guards/one_shot_exclusions.dart.

part of 'pdf.dart';

/// The reads of a document. Extended by [PdfDoc], which answers from an
/// open handle, and by the object [Pdf.reads] returns, which opens the
/// document, answers one call and closes it.
abstract base class PdfReads {
  PdfReads._();

  /// Runs [read] on the engine's document handle: on the open one for
  /// [PdfDoc], on one opened and closed for the call for a one-shot.
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeDocHandle handle) read);

  /// Searches for [query] text across the specified [pages].
  PdfTask<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) {
    KeepRecord.op('extract');
    return _run((handle) => handle.search(query: query, pages: pages));
  }
}

/// A one-shot runs one call: a second call is refused before any I/O.
mixin _SingleUse {
  bool _spent = false;

  void _spend(String kind, String several) {
    if (_spent) {
      throw PdfInvalidArgument('a one-shot runs one $kind; use $several');
    }
    _spent = true;
  }
}

/// The object [Pdf.reads] returns: opens the document for its one call and
/// closes it afterwards, on success, error and cancel.
final class _OneShotReads extends PdfReads with _SingleUse {
  _OneShotReads(this._pdf, this._source, this._password) : super._();

  final Pdf _pdf;
  final DataSource _source;
  final String? _password;

  @override
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeDocHandle handle) read) {
    _spend('read', 'read for several');
    return _own(
      _pdf.open(_source, password: _password),
      (doc, hook) => hook.guard(read(doc._handle)),
    );
  }
}
