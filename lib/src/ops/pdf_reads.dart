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

  /// The facts of the pages named: their size, their rotation and their
  /// label. Every page when [pages] is null.
  PdfTask<List<PdfPageInfo>> pageInfo({PdfPages? pages}) =>
      _run((handle) => handle.pageInfo(pages: pages));

  /// Writes the text of the pages named to [output] as the engine produces
  /// it; every page when [pages] is null. The sink is the caller's: the
  /// library writes to it and never closes it.
  ///
  /// The text reaches [output] as UTF-8, page by page, and the library
  /// never holds the document's text whole. A caller that wants a string
  /// writes to a [MemorySink] and takes it:
  ///
  /// ```dart
  /// final sink = MemorySink();
  /// await doc.extractText(sink);
  /// final text = sink.takeString();
  /// ```
  ///
  /// [format] is the shape of the text: plain text, structured text,
  /// Markdown or HTML. [PdfExtractionFormat.auto] lets the engine choose.
  PdfTask<void> extractText(
    DataSink output, {
    PdfPages? pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    KeepRecord.op('extract');
    return _run(
      (handle) => handle.extractText(output, pages: pages, format: format),
    );
  }

  /// Searches for [query] in the text of the pages named; every page when
  /// [pages] is null.
  PdfTask<List<PdfSearchHit>> search(String query, {PdfPages? pages}) {
    KeepRecord.op('extract');
    return _run((handle) => handle.search(query: query, pages: pages));
  }

  /// Renders the pages named and writes each one to the sink [sinkFor]
  /// returns for it, in page order; resolves with the pages written.
  /// Every page when [pages] is null.
  ///
  /// [sinkFor] receives the page's descriptor, with its pixel size, before
  /// any pixel is written. The sink it returns is the library's: it is
  /// closed after its page, and on an error or a cancel the sink of the page
  /// in flight is closed too. Pages already delivered stand. Each write
  /// is awaited before the engine goes on, so a slow sink slows the render;
  /// the library holds no page, only the bytes of one chunk in transit.
  ///
  /// [size] is the size of each page (`.dpi(150)` by default: 150 pixels per
  /// inch of the page box); [format] is what the sink receives, a PNG file
  /// or raw straight-alpha RGBA rows, top to bottom.
  ///
  /// ```dart
  /// final pages = await doc.render(
  ///   (page) => FileSink.open(File('page-${page.page}.png')),
  /// );
  /// ```
  ///
  /// Cancelling the task closes the current page's sink, stops the render
  /// and resolves the task with [PdfCancelled].
  PdfTask<List<PdfRenderedPage>> render(
    FutureOr<DataSink> Function(PdfRenderedPage page) sinkFor, {
    PdfPages? pages,
    PdfRenderSize size = const PdfRenderSize.dpi(150),
    PdfRenderFormat format = PdfRenderFormat.png,
  }) {
    KeepRecord.op('render');
    return _run(
      (handle) =>
          handle.render(sinkFor, pages: pages, size: size, format: format),
    );
  }

  /// Extracts the images drawn on the pages named and writes each one to the
  /// sink [sinkFor] returns for it, page by page in the order found;
  /// resolves with the images written. Every page when [pages] is null.
  ///
  /// [sinkFor] receives the image's descriptor before any byte is written.
  /// The sink it returns is the library's: it is closed after its image, and
  /// on an error or a cancel the sink of the image in flight is closed too.
  /// Images already delivered stand. Each write is awaited before the engine
  /// goes on, and no image is ever held whole: the library holds only the
  /// bytes of one chunk in transit.
  ///
  /// What each [PdfImage.encoding] yields:
  ///
  /// - [PdfImageEncoding.jpeg] and [PdfImageEncoding.jpx]: the stored
  ///   codestream, byte for byte (a JPEG file or a JPEG 2000 codestream, as
  ///   the file holds it, decrypted and never decoded). The image's masks and
  ///   `/Decode` are not applied; read them from the file if they matter.
  /// - every other encoding ([PdfImageEncoding.raw], [PdfImageEncoding.ccitt]
  ///   and [PdfImageEncoding.jbig2]): a PNG file written row by row. It is
  ///   1-bit gray for a stencil mask and for a 1-bit gray image
  ///   ([PdfImageColor.bilevel]), 8-bit gray for [PdfImageColor.gray], and
  ///   8-bit RGB for [PdfImageColor.rgb], [PdfImageColor.cmyk] and
  ///   [PdfImageColor.other] (converted as a page render converts them). An
  ///   image with an `/SMask`, a `/Mask` stream or a `/Mask` colour-key
  ///   array is written as 8-bit RGBA, the mask as its alpha, because the
  ///   picture alone would show pixels the page never shows. Samples of 16
  ///   bits are written at 8.
  ///
  /// Images narrower or shorter than 8 pixels, and images stored with a
  /// filter the engine does not know, are not listed.
  ///
  /// ```dart
  /// final images = await doc.extractImages(
  ///   (image) => FileSink.open(
  ///     File(
  ///       'p${image.page}-${image.index}.'
  ///       '${image.encoding == PdfImageEncoding.jpeg ? 'jpg' : 'png'}',
  ///     ),
  ///   ),
  /// );
  /// ```
  ///
  /// Cancelling the task closes the current image's sink, stops the
  /// extraction and resolves the task with [PdfCancelled].
  PdfTask<List<PdfImage>> extractImages(
    FutureOr<DataSink> Function(PdfImage image) sinkFor, {
    PdfPages? pages,
  }) {
    KeepRecord.op('render');
    return _run((handle) => handle.extractImages(sinkFor, pages: pages));
  }

  /// Returns metadata for all digital signatures in the document.
  PdfTask<List<PdfSignatureInfo>> signatures() {
    KeepRecord.op('signatures');
    return _run((handle) => handle.signatures());
  }

  /// Whether the document has at least one signature and every
  /// signature verifies: the digest of the bytes each one covers
  /// matches, and the signer's key verifies it. Certificate trust and
  /// revocation are not checked. A signature over an earlier revision
  /// still counts; see [PdfSignatureInfo.coversWholeDocument].
  PdfTask<bool> verifySignatures() {
    KeepRecord.op('signatures');
    return _run((handle) => handle.verifySignatures());
  }

  /// Validates PDF/A conformance at the given [level].
  PdfTask<PdfValidationResult> validatePdfA({PdfALevel level = PdfALevel.a2}) {
    KeepRecord.op('pdfa');
    return _run((handle) => handle.validatePdfA(level: level));
  }

  /// Validates PDF/UA (accessibility) conformance at the given [level].
  PdfTask<PdfValidationResult> validatePdfUa({
    PdfUaLevel level = PdfUaLevel.ua1,
  }) {
    KeepRecord.op('pdfa');
    return _run((handle) => handle.validatePdfUa(level: level));
  }

  /// The page range of each top-level bookmark of the document.
  PdfTask<List<PdfBookmarkRange>> bookmarkRanges() =>
      _run((handle) => handle.bookmarkRanges());

  /// Classifies the pages named by what they hold; every page when [pages]
  /// is null.
  PdfTask<List<PdfPageClassification>> classifyPages({PdfPages? pages}) {
    KeepRecord.op('extract');
    return _run((handle) => handle.classifyPages(pages: pages));
  }

  /// Classifies the whole document by what it holds.
  PdfTask<PdfDocumentType> classifyDocument() {
    KeepRecord.op('extract');
    return _run((handle) => handle.classifyDocument());
  }

  /// Reads every AcroForm field of the document.
  PdfTask<List<PdfFormField>> formFields() =>
      _run((handle) => handle.formFields());

  /// Exports every AcroForm field's current value to [output] in [format].
  PdfTask<void> exportFormData(
    DataSink output, {
    PdfFormDataFormat format = PdfFormDataFormat.xfdf,
  }) => _run((handle) => handle.exportFormData(output, format: format.name));

  /// What the document's XFA packet declares, or `null` when the document
  /// has no XFA form.
  PdfTask<PdfXfaInfo?> xfa() => _run((handle) => handle.xfa());

  /// Lists the files embedded in the document, metadata only.
  PdfTask<List<PdfAttachment>> attachments() =>
      _run((handle) => handle.attachments());

  /// Lists the annotations on [page] (0-based), in /Annots order: links,
  /// comments, stamps, form widgets. Throws [PdfPageRangeError] for a
  /// page past the end.
  PdfTask<List<PdfAnnotationInfo>> annotations(int page) =>
      _run((handle) => handle.annotations(page));

  /// Writes the embedded file named [name] to [output]. An unknown name
  /// throws [PdfEngineError].
  PdfTask<void> extractAttachment(String name, DataSink output) =>
      _run((handle) => handle.extractAttachment(name, output));
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
