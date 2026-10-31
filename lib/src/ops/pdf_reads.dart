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

  /// Renders [pages] and writes each one to the sink [sinkFor] returns for
  /// it, in page order; resolves with the pages written.
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
  ///   pages: const PdfPages.all(),
  /// );
  /// ```
  ///
  /// Cancelling the task closes the current page's sink, stops the render
  /// and resolves the task with [PdfCancelled].
  PdfTask<List<PdfRenderedPage>> render(
    FutureOr<DataSink> Function(PdfRenderedPage page) sinkFor, {
    required PdfPages pages,
    PdfRenderSize size = const PdfRenderSize.dpi(150),
    PdfRenderFormat format = PdfRenderFormat.png,
  }) {
    KeepRecord.op('render');
    return _run(
      (handle) =>
          handle.render(sinkFor, pages: pages, size: size, format: format),
    );
  }

  /// Extracts the images drawn on [pages] and writes each one to the sink
  /// [sinkFor] returns for it, page by page in the order found; resolves with
  /// the images written.
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
  ///   pages: const PdfPages.all(),
  /// );
  /// ```
  ///
  /// Cancelling the task closes the current image's sink, stops the
  /// extraction and resolves the task with [PdfCancelled].
  PdfTask<List<PdfImage>> extractImages(
    FutureOr<DataSink> Function(PdfImage image) sinkFor, {
    required PdfPages pages,
  }) {
    KeepRecord.op('extractImages');
    return _run((handle) => handle.extractImages(sinkFor, pages: pages));
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
