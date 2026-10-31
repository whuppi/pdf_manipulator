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

  /// Rotates a single [page] by [degrees] (must be a multiple of 90).
  PdfTask<void> rotatePage(int page, {required int degrees}) =>
      _run((handle) => handle.rotatePage(page, degrees: degrees));

  /// Adds a text watermark to the specified [page].
  PdfTask<void> addWatermark(
    int page,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  }) => _run(
    (handle) => handle.addWatermark(
      page,
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
