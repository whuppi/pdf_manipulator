// PdfEdits — every edit of a document, declared once.
//
// [PdfEditor] and the object `Pdf.edits` returns both extend it and add only
// `_run`, so an edit exists on both doors the moment it compiles. A new edit
// is a new member here. Members of PdfEditor that are not declared here are
// listed with their reason in test/guards/one_shot_exclusions.dart.

part of 'pdf.dart';

/// The edits of a document. Extended by [PdfEditor], which applies each to
/// an open handle, and by the object [Pdf.edits] returns, which opens the
/// document, applies one edit, saves and closes.
abstract base class PdfEdits {
  PdfEdits._();

  /// Runs [apply] on the engine's editor handle: on the open one for
  /// [PdfEditor], on one opened, saved and closed for the call for a
  /// one-shot.
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeEditorHandle handle) apply);

  /// Replaces the document's standard info fields with [metadata]: a field
  /// it leaves null is removed, so `setMetadata(await editor.metadata())`
  /// changes nothing. Change one field with
  /// `setMetadata(m.copyWith(title: 'X'))`.
  PdfTask<void> setMetadata(PdfMetadata metadata) =>
      _run((handle) => handle.setMetadata(metadata));

  /// Turns every page in [pages] by [by] on top of its current rotation.
  /// A page listed twice, an empty list or a page the document does not
  /// have throws before any page is turned.
  PdfTask<void> rotatePages(PdfPages pages, {required PdfRotation by}) =>
      _run((handle) => handle.rotatePages(pages, by: by));

  /// Sets the rotation of every page in [pages] to [rotation], replacing
  /// what each had. Refuses the same page lists as [rotatePages].
  PdfTask<void> setRotation(PdfPages pages, PdfRotation rotation) =>
      _run((handle) => handle.setRotation(pages, rotation));

  /// Sets the media box, the physical size, of every page in [pages].
  /// Refuses the same page lists as [rotatePages].
  PdfTask<void> setMediaBox(PdfPages pages, PdfRect box) =>
      _run((handle) => handle.setMediaBox(pages, box));

  /// Sets the crop box, the visible area, of every page in [pages]; a null
  /// [box] removes the page's own crop box. Refuses the same page lists as
  /// [rotatePages].
  PdfTask<void> setCropBox(PdfPages pages, PdfRect? box) =>
      _run((handle) => handle.setCropBox(pages, box));

  /// Removes every page in [pages]. Deleting every page of the document
  /// throws [PdfInvalidArgument]; so do the page lists [rotatePages] refuses.
  PdfTask<void> deletePages(PdfPages pages) =>
      _run((handle) => handle.deletePages(pages));

  /// Keeps only the pages in [order] (0-based), in that order, and removes
  /// all others. A page listed twice or an empty list throws
  /// [PdfInvalidArgument].
  PdfTask<void> keepPages(List<int> order) {
    if (order.toSet().length != order.length) {
      throw PdfInvalidArgument('a page is listed twice: $order');
    }
    return _run((handle) => handle.keepPages(order));
  }

  /// Moves the page at index [from] to index [to].
  PdfTask<void> movePage({required int from, required int to}) =>
      _run((handle) => handle.movePage(from: from, to: to));

  /// Adds a text watermark to every page in [pages]. Refuses the same page
  /// lists as [rotatePages], unless [pages] is every page.
  PdfTask<void> addWatermark(
    String text, {
    PdfPages pages = const PdfPages.all(),
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _run(
    (handle) => handle.addWatermark(
      pages,
      text,
      style: style,
      position: position,
      layer: layer,
    ),
  );
}

/// The object [Pdf.edits] returns: opens the document for its one edit,
/// saves it to the output and closes it, on success, error and cancel.
final class _OneShotEdits extends PdfEdits with _SingleUse {
  _OneShotEdits(
    this._pdf,
    this._source,
    this._output,
    this._password,
    this._options,
  ) : super._();

  final Pdf _pdf;
  final DataSource _source;
  final DataSink _output;
  final String? _password;
  final PdfSaveOptions _options;

  @override
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeEditorHandle handle) apply) {
    _spend('edit', 'rewrite for several');
    return _own(_pdf.edit(_source, password: _password), (editor, hook) async {
      final result = await hook.guard(apply(editor._handle));
      await hook.guard(editor.save(_output, options: _options));
      return result;
    });
  }
}
