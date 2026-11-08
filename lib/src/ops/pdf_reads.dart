// PdfReads — every read of a document, declared once.
//
// [PdfDocument] and the object `Pdf.readOnce` returns both extend it and add only
// `_run`, so a read exists on both doors the moment it compiles. A new read
// is a new member here. Members of PdfDocument that are not declared here are
// listed with their reason in test/guards/one_shot_exclusions.dart.

part of 'pdf.dart';

/// The reads of a document. Extended by [PdfDocument], which answers from an
/// open handle, and by the object [Pdf.readOnce] returns, which opens the
/// document, answers one call and closes it.
abstract base class PdfReads {
  PdfReads._();

  /// Runs [read] on the engine's document handle: on the open one for
  /// [PdfDocument], on one opened and closed for the call for a one-shot. [output]
  /// is the sink the read writes to, if it has one: a one-shot that ends
  /// cancelled or failed aborts it, even when the read into it had finished.
  PdfTask<T> _run<T>(
    PdfTask<T> Function(BridgeDocHandle handle) read, {
    DataSink? output,
  });

  /// The facts of the pages named: their size, their rotation and their
  /// label. Every page by default.
  PdfTask<List<PdfPageInfo>> pageInfo({
    PdfPages pages = const PdfPages.all(),
  }) => _run((handle) => handle.pageInfo(pages: pages));

  /// Writes the text of the pages named to [output] as the engine produces
  /// it; every page when [pages] is all. The sink is the caller's: the
  /// library writes to it, aborts it when the read ends cancelled or failed,
  /// and never closes it.
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
    PdfPages pages = const PdfPages.all(),
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    KeepRecord.op('extract');
    return _run(
      (handle) => handle.extractText(output, pages: pages, format: format),
      output: output,
    );
  }

  /// The text of the pages named as a string, in the same shape
  /// [extractText] writes: every page by default, [format] as there.
  ///
  /// The string holds the text whole, so it suits a page or a short
  /// document; for a long one write to a sink with [extractText], which
  /// never holds it.
  PdfTask<String> text({
    PdfPages pages = const PdfPages.all(),
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    KeepRecord.op('extract');
    return _run(
      (handle) => PdfTask.group((hook) async {
        final sink = MemorySink();
        await hook.guard(
          handle.extractText(sink, pages: pages, format: format),
        );
        return sink.takeString();
      }),
    );
  }

  /// Searches for [query] in the text of the pages named; every page by
  /// default.
  PdfTask<List<PdfSearchHit>> search(
    String query, {
    PdfPages pages = const PdfPages.all(),
  }) {
    KeepRecord.op('extract');
    return _run((handle) => handle.search(query: query, pages: pages));
  }

  /// Renders the pages named and writes each one to the sink [sinkFor]
  /// returns for it, in page order; resolves with the pages written.
  /// Every page by default.
  ///
  /// [sinkFor] receives the page's descriptor, with its pixel size, before
  /// any pixel is written. The sink it returns is the library's: it is
  /// closed after its page, and on an error or a cancel the sink of the page
  /// in flight is aborted, then closed. Pages already delivered stand. Each write
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
  /// Cancelling the task aborts and closes the current page's sink, stops the
  /// render and resolves the task with [PdfCancelled].
  PdfTask<List<PdfRenderedPage>> render(
    FutureOr<DataSink> Function(PdfRenderedPage page) sinkFor, {
    PdfPages pages = const PdfPages.all(),
    PdfRenderSize size = const PdfRenderSize.dpi(150),
    PdfRenderFormat format = PdfRenderFormat.png,
  }) {
    KeepRecord.op('render');
    return _run(
      (handle) =>
          handle.render(sinkFor, pages: pages, size: size, format: format),
    );
  }

  /// Renders the one page [page] (0-based) and returns it whole: its size in
  /// pixels and the bytes in [format]. A page is bounded, so the bytes are
  /// held; to write many pages, or a page straight to a file, use [render].
  PdfTask<PdfBitmap> bitmap(
    int page, {
    PdfRenderSize size = const PdfRenderSize.dpi(150),
    PdfRenderFormat format = PdfRenderFormat.png,
  }) {
    KeepRecord.op('render');
    return _run(
      (handle) => PdfTask.group((hook) async {
        final sink = MemorySink();
        final rendered = await hook.guard(
          handle.render(
            (_) => sink,
            pages: PdfPages.single(page),
            size: size,
            format: format,
          ),
        );
        final only = rendered.single;
        return PdfBitmap(
          page: only.page,
          width: only.width,
          height: only.height,
          bytes: sink.takeBytes(),
        );
      }),
    );
  }

  /// Extracts the images drawn on the pages named and writes each one to the
  /// sink [sinkFor] returns for it, page by page in the order found;
  /// resolves with the images written. Every page by default.
  ///
  /// [sinkFor] receives the image's descriptor before any byte is written.
  /// The sink it returns is the library's: it is closed after its image, and
  /// on an error or a cancel the sink of the image in flight is aborted, then
  /// closed.
  /// Images already delivered stand. Each write is awaited before the engine
  /// goes on, and no image is ever held whole: the library holds only the
  /// bytes of one chunk in transit.
  ///
  /// What each [PdfImageInfo.encoding] yields:
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
  /// Cancelling the task aborts and closes the current image's sink, stops
  /// the extraction and resolves the task with [PdfCancelled].
  PdfTask<List<PdfImageInfo>> extractImages(
    FutureOr<DataSink> Function(PdfImageInfo image) sinkFor, {
    PdfPages pages = const PdfPages.all(),
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
  PdfTask<PdfValidationReport> validatePdfA({PdfALevel level = PdfALevel.a2}) {
    KeepRecord.op('pdfa');
    return _run((handle) => handle.validatePdfA(level: level));
  }

  /// Validates PDF/UA (accessibility) conformance at the given [level].
  PdfTask<PdfValidationReport> validatePdfUa({
    PdfUaLevel level = PdfUaLevel.ua1,
  }) {
    KeepRecord.op('pdfa');
    return _run((handle) => handle.validatePdfUa(level: level));
  }

  /// The page range of each top-level bookmark of the document.
  PdfTask<List<PdfBookmarkRange>> bookmarkRanges() =>
      _run((handle) => handle.bookmarkRanges());

  /// Classifies the pages named by what they hold; every page by default.
  PdfTask<List<PdfPageClassification>> classifyPages({
    PdfPages pages = const PdfPages.all(),
  }) {
    KeepRecord.op('extract');
    return _run((handle) => handle.classifyPages(pages: pages));
  }

  /// Classifies the whole document by what it holds.
  PdfTask<PdfDocumentKind> classifyDocument() {
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
  }) => _run(
    (handle) => handle.exportFormData(output, format: format.wire),
    output: output,
  );

  /// What the document's XFA packet declares, or `null` when the document
  /// has no XFA form.
  PdfTask<PdfXfaInfo?> xfa() => _run((handle) => handle.xfa());

  /// Lists the files embedded in the document, metadata only.
  PdfTask<List<PdfAttachment>> attachments() =>
      _run((handle) => handle.attachments());

  /// Lists the annotations on [page] (0-based), in /Annots order: links,
  /// comments, stamps, form widgets. Throws [PdfPageOutOfRange] for a
  /// page past the end.
  PdfTask<List<PdfAnnotationInfo>> annotations(int page) =>
      _run((handle) => handle.annotations(page));

  /// Writes the embedded file named [name] to [output]. A name the document
  /// has no attachment for fails with [PdfNotFound] and writes nothing.
  PdfTask<void> extractAttachment(String name, DataSink output) => _run(
    (handle) => PdfTask.group((hook) async {
      final attachments = await hook.guard(handle.attachments());
      if (!attachments.any((attachment) => attachment.name == name)) {
        throw PdfNotFound(kind: 'attachment', name: name);
      }
      await hook.guard(handle.extractAttachment(name, output));
    }),
    output: output,
  );
}

/// A one-shot runs one call: a second call is refused before any I/O.
mixin _SingleUse {
  bool _spent = false;

  void _spend(String kind, String several) {
    if (_spent) {
      throw StateError('a one-shot runs one $kind; use $several');
    }
    _spent = true;
  }
}

/// The object [Pdf.readOnce] returns: opens the document for its one call and
/// closes it afterwards, on success, error and cancel.
final class _OneShotReads extends PdfReads with _SingleUse {
  _OneShotReads(this._pdf, this._input) : super._();

  final Pdf _pdf;
  final PdfInput _input;

  @override
  PdfTask<T> _run<T>(
    PdfTask<T> Function(BridgeDocHandle handle) read, {
    DataSink? output,
  }) {
    _spend('read', 'read for several');
    return _own(
      _pdf.open(_input),
      (doc, hook) => hook.guard(read(doc._handle)),
      output: output,
    );
  }
}
