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

  /// Adds a stamp annotation of [type] in [rect] to every page in [pages]
  /// at [opacity], from 0 (clear) to 1 (opaque). Refuses the same page lists
  /// as [rotatePages], and a merged-in page; no page is stamped then.
  PdfTask<void> addStamp(
    PdfPages pages, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) => _run(
    (handle) =>
        handle.addStamp(pages, type: type, rect: rect, opacity: opacity),
  );

  /// Adds a stamp of the image in [image] (PNG or JPEG) in [rect] to every
  /// page in [pages] at [opacity]. The pages share one copy of the image.
  /// Refuses the page lists [addStamp] refuses, and an image the engine
  /// cannot decode; no page is stamped then.
  PdfTask<void> addImageStamp(
    PdfPages pages,
    DataSource image, {
    required PdfRect rect,
    double opacity = 1.0,
  }) => _run(
    (handle) =>
        handle.addImageStamp(pages, image, rect: rect, opacity: opacity),
  );

  /// Flattens the interactive form fields on every page in [pages] into
  /// static content. With every page the form is gone; with some, the other
  /// pages stay fillable. Refuses the page lists [addStamp] refuses.
  PdfTask<void> flattenForms({PdfPages pages = const PdfPages.all()}) =>
      _run((handle) => handle.flattenForms(pages));

  /// Flattens the annotations (forms, comments, stamps) on every page in
  /// [pages] into static content. Refuses the page lists [addStamp] refuses.
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
  /// [PdfInvalidArgument].
  PdfTask<PdfRedactionReport> redact(
    int page,
    List<PdfRect> regions, {
    PdfRedactionOverlay overlay = const PdfRedactionOverlay.fill(
      PdfColor.black,
    ),
  }) {
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

  /// Embeds [data] as an attached file with the given [name].
  ///
  /// [description] and [mimeType] go into the file spec, where
  /// `PdfDoc.attachments` reads them back; [relationship] states how the
  /// file relates to this document.
  PdfTask<void> embedFile(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  }) => _run(
    (handle) => handle.embedFile(
      name,
      data,
      description: description,
      mimeType: mimeType,
      relationship: relationship,
    ),
  );

  /// Crops every page by the given margins, in points.
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
  /// mirrors.
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds) =>
      _run((handle) => handle.setImageBounds(page, imageName, bounds));

  /// Removes all document metadata (title, author, timestamps, etc.).
  /// Equivalent to `sanitize(const PdfSanitizeOptions(javascript: false))`.
  PdfTask<void> scrubMetadata() => _run((handle) => handle.scrubMetadata());

  /// Strips metadata, JavaScript and/or embedded files per [options].
  PdfTask<void> sanitize(PdfSanitizeOptions options) =>
      _run((handle) => handle.sanitize(options));
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
    }, output: _output);
  }
}
