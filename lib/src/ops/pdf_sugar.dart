// Sugar — the one-shots that open several handles, apply a policy or loop a
// builder step. Every other edit is a member of PdfEdits and runs through
// `rewrite` or `pdf.edits`.
//
// Every handle a one-shot opens goes through `_own`, which closes it after
// success, error and cancel. A sink a one-shot obtains from a factory is the
// library's and is closed after its item, on error and on cancel.

part of 'pdf.dart';

/// One-shot operations that open several handles, apply a policy or loop a
/// builder step.
extension PdfSugar on Pdf {
  /// Merges [sources] into one document written to [output], in the order
  /// given. A source that is password-protected opens with its entry of
  /// [passwords]; an entry for a source that is not in [sources] throws
  /// [PdfInvalidArgument].
  PdfTask<void> merge(
    List<DataSource> sources,
    DataSink output, {
    Map<DataSource, String> passwords = const {},
  }) {
    if (sources.isEmpty) {
      throw const PdfInvalidArgument('sources must not be empty');
    }
    for (final source in passwords.keys) {
      if (!sources.contains(source)) {
        throw const PdfInvalidArgument(
          'passwords names a source that is not in sources',
        );
      }
    }
    return rewrite(sources.first, output, (editor) async {
      for (var i = 1; i < sources.length; i++) {
        await editor.mergeFrom(sources[i], password: passwords[sources[i]]);
      }
    }, password: passwords[sources.first]);
  }

  /// Splits [source] into the parts [by] says. [sinkFor] is asked for each
  /// part's sink before the part is written; the library closes it after the
  /// part, and aborts it first when the part fails or is cancelled. The parts
  /// delivered before a failure or a cancel stand. The task resolves with
  /// what each part holds, in order; the bytes went to the sinks.
  ///
  /// [PdfSplitRule.maxBytes] cuts a part at the most pages whose file stays
  /// within the limit, as the engine measures it; a page that alone exceeds
  /// the limit is a part of its own.
  PdfTask<List<PdfSplitPart>> split(
    DataSource source,
    FutureOr<DataSink> Function(PdfSplitPart part) sinkFor, {
    required PdfSplitRule by,
    String? password,
  }) {
    if (by case PdfSplitEvery(pages: < 1)) {
      throw const PdfInvalidArgument('every must be >= 1');
    }
    if (by case PdfSplitMaxBytes(bytes: < 1)) {
      throw const PdfInvalidArgument('maxBytes must be >= 1');
    }
    return _own(edit(source, password: password), (editor, hook) async {
      final handle = editor._handle;
      final parts = <PdfSplitPart>[];

      Future<void> deliver(PdfSplitPart part) async {
        final sink = await sinkFor(part);
        try {
          await handle.extractPages(_pageRun(part.start, part.end + 1), sink);
          // A cancel that lands as the part's last write does is the part's:
          // it is not delivered.
          if (hook.isCancelled) throw const PdfCancelled();
        } catch (error) {
          // The op aborted the sink if it ran; the hook aborts it once.
          await hook.abortSinks([sink], error);
          rethrow;
        } finally {
          await sink.close();
        }
        parts.add(part);
      }

      final total = await handle.pageCount();
      switch (by) {
        case PdfSplitEvery(:final pages):
          for (var start = 0; start < total; start += pages) {
            await deliver(
              PdfSplitPart(
                index: parts.length,
                start: start,
                end: min(start + pages, total) - 1,
              ),
            );
          }
        case PdfSplitMaxBytes(:final bytes):
          for (var start = 0; start < total;) {
            final count = await _pagesThatFit(handle, start, total, bytes);
            await deliver(
              PdfSplitPart(
                index: parts.length,
                start: start,
                end: start + count - 1,
              ),
            );
            start += count;
          }
        case PdfSplitBookmarks():
          final ranges = await hook.guard(
            _own(
              open(source, password: password),
              (doc, planHook) => planHook.guard(doc.bookmarkRanges()),
            ),
          );
          for (final range in ranges) {
            await deliver(
              PdfSplitPart(
                index: parts.length,
                start: range.start,
                end: range.end - 1,
                title: range.title,
              ),
            );
          }
      }
      return parts;
    });
  }

  /// Compresses the PDF: images through `PdfEdits.reduceImages` under
  /// [images] (`PdfImagePolicy.ebook` unless you pass another preset or
  /// your own numbers), then a full rewrite with stream compression, one
  /// copy of identical images, ICC profiles and embedded fonts, the
  /// resource entries no content names dropped and garbage collection.
  /// [password] opens an encrypted [source].
  ///
  /// It never makes a file larger: when the result is not smaller than
  /// [source], [output] receives the bytes of [source] unchanged.
  PdfTask<void> compress(
    DataSource source,
    DataSink output, {
    PdfImagePolicy images = PdfImagePolicy.ebook,
    String? password,
  }) {
    KeepRecord.op('render');
    return _own(edit(source, password: password), (editor, hook) async {
      await hook.guard(editor.reduceImages(images));
      const options = PdfSaveOptions.fullRewrite(
        mergeDuplicates: true,
        pruneResources: true,
      );
      if (source.length <= _compressInMemoryLimit) {
        final result = MemorySink();
        await hook.guard(editor.save(result, options: options));
        final bytes = result.takeBytes();
        if (bytes.length < source.length) {
          for (var at = 0; at < bytes.length; at += _compressChunk) {
            final end = min(at + _compressChunk, bytes.length);
            await output.write(Uint8List.sublistView(bytes, at, end));
          }
        } else {
          await _copyUnchanged(source, output);
        }
      } else {
        final size = await hook.guard(
          editor._handle.measureSave(options: options),
        );
        if (size < source.length) {
          await hook.guard(editor.save(output, options: options));
        } else {
          await _copyUnchanged(source, output);
        }
      }
    }, output: output);
  }

  /// Creates a PDF from a list of images, one per page.
  ///
  /// Each page is the image's own size, one point per pixel, unless [page]
  /// names a size; [PdfImagePageOptions.fit] says how the image fills it.
  PdfTask<void> imagesToPdf(
    List<DataSource> images,
    DataSink output, {
    PdfImagePageOptions page = const PdfImagePageOptions(),
    bool deflate = true,
  }) => buildTo(output, (builder) async {
    for (final image in images) {
      await builder.addImagePage(image, page: page);
    }
  }, deflate: deflate);
}

/// A source up to this size is compressed into memory and compared with the
/// source; a larger one is measured by the engine first.
const _compressInMemoryLimit = 4 * 1024 * 1024;

const _compressChunk = 64 * 1024;

Future<void> _copyUnchanged(DataSource source, DataSink output) async {
  for (var at = 0; at < source.length;) {
    final chunk = await source.readAt(
      at,
      min(_compressChunk, source.length - at),
    );
    if (chunk.isEmpty) {
      throw PdfIoError('source ended at $at of ${source.length} bytes');
    }
    await output.write(chunk);
    at += chunk.length;
  }
}

/// The page indices from [start] up to, not including, [end].
List<int> _pageRun(int start, int end) => [for (var p = start; p < end; p++) p];

/// How many pages from [start] make the longest part within [maxBytes]: the
/// engine measures the file of a page run, the run doubles while it fits,
/// and the last doubling is bisected. A single page is a part whatever its
/// size.
Future<int> _pagesThatFit(
  BridgeEditorHandle handle,
  int start,
  int total,
  int maxBytes,
) async {
  Future<bool> fits(int count) async =>
      await handle.measurePages(_pageRun(start, start + count)) <= maxBytes;

  final remaining = total - start;
  var probe = 1;
  var lastFit = 1;
  while (probe <= remaining) {
    if (await fits(probe)) {
      lastFit = probe;
      probe *= 2;
    } else {
      break;
    }
  }

  var lo = lastFit;
  var hi = probe.clamp(1, remaining);
  var best = lastFit;
  if (lo < hi) {
    while (lo <= hi) {
      final mid = (lo + hi) ~/ 2;
      if (await fits(mid)) {
        best = mid;
        lo = mid + 1;
      } else {
        hi = mid - 1;
      }
    }
  }
  return best;
}
