// PdfEdits — every edit of a document, declared once.
//
// [PdfEditor] and the object `Pdf.editOnce` returns both extend it and add only
// `_run`, so an edit exists on both doors the moment it compiles. A new edit
// is a new member here. Members of PdfEditor that are not declared here are
// listed with their reason in test/guards/one_shot_exclusions.dart.
//
// A method whose only argument is the page selection takes it positional;
// every other method takes `pages` as a named parameter that defaults to every
// page.

part of 'pdf.dart';

/// The edits of a document. Extended by [PdfEditor], which applies each to
/// an open handle, and by the object [Pdf.editOnce] returns, which opens the
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
  /// A page the document does not have fails with [PdfPageOutOfRange] before
  /// any page is turned.
  PdfTask<void> rotatePages({
    required PdfRotation by,
    PdfPages pages = const PdfPages.all(),
  }) => _run((handle) => handle.rotatePages(pages, by: by));

  /// Sets the rotation of every page in [pages] to [rotation], replacing
  /// what each had. Refuses the same pages as [rotatePages].
  PdfTask<void> setRotation(
    PdfRotation rotation, {
    PdfPages pages = const PdfPages.all(),
  }) => _run((handle) => handle.setRotation(pages, rotation));

  /// Sets the media box, the physical size, of every page in [pages].
  /// Refuses the same pages as [rotatePages].
  PdfTask<void> setMediaBox(
    PdfRect box, {
    PdfPages pages = const PdfPages.all(),
  }) => _run((handle) => handle.setMediaBox(pages, box));

  /// Sets the crop box, the visible area, of every page in [pages]; a null
  /// [box] removes the page's own crop box. Refuses the same pages as
  /// [rotatePages].
  PdfTask<void> setCropBox(
    PdfRect? box, {
    PdfPages pages = const PdfPages.all(),
  }) => _run((handle) => handle.setCropBox(pages, box));

  /// Removes every page in [pages]. Removing every page of the document
  /// fails with [PdfEngineFailed], as does a page the document does not have.
  PdfTask<void> removePages(PdfPages pages) =>
      _run((handle) => handle.removePages(pages));

  /// Keeps only the pages in [pages], in document order, and removes all
  /// others. A page the document does not have fails with
  /// [PdfPageOutOfRange].
  PdfTask<void> keepPages(PdfPages pages) => _run(
    (handle) => PdfTask.group((hook) async {
      final order = await _keptOrder(hook, handle, pages);
      if (order != null) await hook.guard(handle.keepPages(order));
    }),
  );

  /// Puts the document's pages in [order]: the pages it names stay, in that
  /// order, and every page it leaves out is removed, as the engine keeps
  /// them. An order that names every page is a pure reorder; to drop pages
  /// and keep the document's order, use [keepPages] or [removePages]. A page
  /// the document does not have fails with [PdfPageOutOfRange].
  PdfTask<void> reorderPages(PdfPageOrder order) =>
      _run((handle) => handle.keepPages(order.indices));

  /// Moves the page at index [from] to index [to].
  PdfTask<void> movePage({required int from, required int to}) =>
      _run((handle) => handle.movePage(from: from, to: to));

  /// Adds a text watermark to every page in [pages]. Refuses the same pages
  /// as [rotatePages].
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

  /// Adds a stamp annotation of [stamp] in [bounds] to every page in
  /// [pages] at [opacity], from 0 (clear) to 1 (opaque). Refuses the same
  /// pages as [rotatePages], and a merged-in page; no page is stamped then.
  PdfTask<void> addStamp(
    PdfStamp stamp, {
    required PdfRect bounds,
    PdfPages pages = const PdfPages.all(),
    double opacity = 1,
  }) => _run(
    (handle) =>
        handle.addStamp(pages, stamp: stamp, bounds: bounds, opacity: opacity),
  );

  /// Adds a stamp of the image in [image] (PNG or JPEG) in [bounds] to every
  /// page in [pages] at [opacity]. The pages share one copy of the image.
  /// Refuses the pages [addStamp] refuses, and an image the engine cannot
  /// decode; no page is stamped then.
  PdfTask<void> addImageStamp(
    DataSource image, {
    required PdfRect bounds,
    PdfPages pages = const PdfPages.all(),
    double opacity = 1,
  }) => _run(
    (handle) =>
        handle.addImageStamp(pages, image, bounds: bounds, opacity: opacity),
  );

  /// Flattens the interactive form fields on every page in [pages] into
  /// static content. With every page the form is gone; with some, the other
  /// pages stay fillable. Refuses the pages [addStamp] refuses.
  PdfTask<void> flattenForms({PdfPages pages = const PdfPages.all()}) =>
      _run((handle) => handle.flattenForms(pages));

  /// Flattens the annotations (forms, comments, stamps) on every page in
  /// [pages] into static content. Refuses the pages [addStamp] refuses.
  PdfTask<void> flattenAnnotations({PdfPages pages = const PdfPages.all()}) =>
      _run((handle) => handle.flattenAnnotations(pages));

  /// Converts the document to PDF/A at conformance [level].
  PdfTask<void> convertToPdfA({PdfALevel level = PdfALevel.a2}) {
    KeepRecord.op('pdfa');
    return _run((handle) => handle.convertToPdfA(level: level));
  }

  /// Removes the content under every rectangle of [regions] on [page] for
  /// good, draws [overlay] over each, and reports what was removed. It
  /// applies only these regions: marks queued earlier with
  /// `PdfEditor.markRedaction` stay queued and are not applied. A failure
  /// leaves the page as it was and no mark queued. An empty [regions] throws
  /// [ArgumentError].
  PdfTask<PdfRedactionReport> redact(
    int page,
    List<PdfRect> regions, {
    PdfRedactionOverlay overlay = const PdfRedactionOverlay.fill(
      PdfColor.black,
    ),
  }) {
    if (regions.isEmpty) {
      throw ArgumentError.value(regions, 'regions', 'must not be empty');
    }
    KeepRecord.op('render');
    return _run((handle) => handle.redact(page, regions, overlay));
  }

  /// Re-encodes and downsamples every image drawn on the document's pages
  /// under [policy] — `PdfImagePolicy.screen`, `.ebook`, `.print`,
  /// `.lossless`, or your own numbers — and reports one row per image.
  ///
  /// Placement never changes: `PdfEditor.pageImages` reads the same names,
  /// bounds and transforms afterwards. A replacement is written only when it
  /// is smaller than the original; soft masks are resampled with their
  /// image; JPEG 2000, JBIG2, spot-colour and colour-key-masked images are
  /// reported as kept. Only images drawn by page content (directly or
  /// through Form XObjects) are visited: an image referenced solely from an
  /// annotation appearance stream or a pattern, and inline `BI … EI`
  /// images, are left as stored and do not appear in the report.
  PdfTask<PdfImageReport> reduceImages(PdfImagePolicy policy) {
    KeepRecord.op('render');
    return _run((handle) => handle.reduceImages(policy));
  }

  /// Removes embedded copies of standard PDF fonts to reduce file size, and
  /// returns how many it removed.
  PdfTask<int> unembedStandardFonts() =>
      _run((handle) => handle.unembedStandardFonts());

  /// Attaches [data] to the document as a file with the given [name].
  ///
  /// [description] and [mimeType] go into the file spec, where
  /// `PdfDocument.attachments` reads them back; [relationship] states how the
  /// file relates to this document.
  PdfTask<void> addAttachment(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  }) => _run(
    (handle) => handle.addAttachment(
      name,
      data,
      description: description,
      mimeType: mimeType,
      relationship: relationship,
    ),
  );

  /// Crops every page by the given margins, in points. The engine crops the
  /// whole document: to crop some pages, set their crop box with
  /// [setCropBox].
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  }) => _run(
    (handle) =>
        handle.cropMargins(left: left, right: right, top: top, bottom: bottom),
  );

  /// Moves and resizes the named image on [page] to [bounds] in points in
  /// one edit. Names come from `PdfEditor.pageImages`, whose `bounds` this
  /// mirrors. A name the page has no image for fails with [PdfNotFound].
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds) =>
      _run(
        (handle) => PdfTask.group((hook) async {
          final images = await hook.guard(handle.pageImages(page));
          if (!images.any((image) => image.name == imageName)) {
            throw PdfNotFound(kind: 'image', name: imageName);
          }
          await hook.guard(handle.setImageBounds(page, imageName, bounds));
        }),
      );

  /// Strips what [stripMetadata], [stripJavaScript] and [stripAttachments]
  /// name: document, XMP and image metadata; document and field JavaScript
  /// with `/OpenAction` and `/AA`; embedded files and file attachment
  /// annotations.
  PdfTask<void> sanitize({
    bool stripMetadata = true,
    bool stripJavaScript = true,
    bool stripAttachments = false,
  }) => _run(
    (handle) => handle.sanitize(
      stripMetadata: stripMetadata,
      stripJavaScript: stripJavaScript,
      stripAttachments: stripAttachments,
    ),
  );
}

/// The pages `keepPages` keeps, as the indices the engine keeps in order;
/// null when [pages] is every page and nothing is to be removed. A selection
/// from a page past the last names that page alone, which the engine refuses
/// with the page and the page count.
Future<List<int>?> _keptOrder(
  CancelHook hook,
  BridgeEditorHandle handle,
  PdfPages pages,
) async {
  switch (pages) {
    case PdfPagesAll():
      return null;
    case PdfPagesSingle(:final index):
      return [index];
    case PdfPagesList(:final indices):
      return indices;
    case PdfPagesRange(:final start, :final end):
      return [for (var page = start; page < end; page++) page];
    case PdfPagesFrom(:final start):
      final count = await hook.guard(handle.pageCount());
      return start < count
          ? [for (var page = start; page < count; page++) page]
          : [start];
  }
}

/// The object [Pdf.editOnce] returns: opens the document for its one edit,
/// saves it to the output and closes it, on success, error and cancel.
final class _OneShotEdits extends PdfEdits with _SingleUse {
  _OneShotEdits(this._pdf, this._input, this._output, this._save) : super._();

  final Pdf _pdf;
  final PdfInput _input;
  final DataSink _output;
  final PdfSave _save;

  @override
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeEditorHandle handle) apply) {
    _spend('edit', 'rewrite for several');
    return _own(_pdf.edit(_input), (editor, hook) async {
      final result = await hook.guard(apply(editor._handle));
      await hook.guard(editor.save(_output, save: _save));
      return result;
    }, output: _output);
  }
}
