// Doc handle — read-only document queries.
//
// Part of shared_bridge.dart — same library so the handles reach the
// bridge's private _exec/_execKeepSource (where every PdfTask is
// born). One handle concern per file; zero logic beyond
// encode → _exec → decode.

part of 'shared_bridge.dart';

// ══════════════════════════════════════════════════════════════════
// Document handle — wraps a handleId, routes read ops to the engine
// ══════════════════════════════════════════════════════════════════

class _SharedDocHandle extends BridgeDocHandle {
  _SharedDocHandle(
    this._bridge,
    this._handleId,
    this._openResult,
    this._resourceId,
  );
  final SharedBridge _bridge;
  final int _handleId;
  final Map<String, Object?> _openResult;
  final int? _resourceId;

  int get _pageCount => _openResult['pageCount'] as int? ?? 0;

  List<int> _pages(PdfPages pages) => _bridge._resolvePages(pages, _pageCount);

  PdfTask<Map<String, Object?>> _exec(
    EngineOp op,
    Map<String, Object?> args, {
    List<DataSink> sinks = const [],
  }) => _bridge._exec(op, {'handleId': _handleId, ...args}, sinks: sinks);

  @override
  Map<String, Object?> get openResult => _openResult;

  @override
  PdfTask<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    final fmt = codec.putEnum(format);
    return PdfTask.group((hook) async {
      return switch (pages) {
        PdfPagesAll() => () async {
          // One frame per page keeps the engine at one page of text; the
          // final response is decoded first so an engine error wins.
          final buf = StringBuffer();
          final sink = _FrameCollectorSink((frame) {
            onEngineFrame?.call();
            buf.write(codec.decodeExtractResult(bin.decodeResponse(frame)));
          });
          await hook.guard(
            _exec(
              EngineOp.extract,
              {'format': fmt, 'hasSink': true},
              sinks: [sink],
            ),
          );
          return buf.toString();
        }(),
        PdfPagesSingle(:final index) => codec.decodeExtractResult(
          await hook.guard(
            _exec(EngineOp.extract, {'format': fmt, 'page': index}),
          ),
        ),
        PdfPagesList(:final indices) => () async {
          final buf = StringBuffer();
          for (final i in indices) {
            buf.write(
              codec.decodeExtractResult(
                await hook.guard(
                  _exec(EngineOp.extract, {'format': fmt, 'page': i}),
                ),
              ),
            );
          }
          return buf.toString();
        }(),
        PdfPagesRange(:final start, :final end) => () async {
          final buf = StringBuffer();
          for (var i = start; i < end; i++) {
            buf.write(
              codec.decodeExtractResult(
                await hook.guard(
                  _exec(EngineOp.extract, {'format': fmt, 'page': i}),
                ),
              ),
            );
          }
          return buf.toString();
        }(),
      };
    });
  }

  @override
  PdfTask<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  }) {
    return PdfTask.group((hook) async {
      return switch (pages) {
        PdfPagesAll() => codec.decodeSearchResults(
          await hook.guard(_exec(EngineOp.search, {'query': query})),
        ),
        PdfPagesSingle(:final index) => codec.decodeSearchResults(
          await hook.guard(
            _exec(EngineOp.search, {'query': query, 'page': index}),
          ),
        ),
        PdfPagesList(:final indices) => () async {
          final all = <SearchResult>[];
          for (final i in indices) {
            all.addAll(
              codec.decodeSearchResults(
                await hook.guard(
                  _exec(EngineOp.search, {'query': query, 'page': i}),
                ),
              ),
            );
          }
          return all;
        }(),
        PdfPagesRange(:final start, :final end) => () async {
          final all = <SearchResult>[];
          for (var i = start; i < end; i++) {
            all.addAll(
              codec.decodeSearchResults(
                await hook.guard(
                  _exec(EngineOp.search, {'query': query, 'page': i}),
                ),
              ),
            );
          }
          return all;
        }(),
      };
    });
  }

  /// The engine writes each page as a header, data frames and an end frame;
  /// [_ChunkDelivery] opens the page's sink at the header, writes each data
  /// frame before it takes the next, and closes the sink at the end frame.
  /// Whatever stops the render (an error, a cancel, a sink that throws)
  /// closes the sink of the page in flight; pages already delivered stand.
  @override
  PdfTask<List<PdfRenderedPage>> render(
    FutureOr<DataSink> Function(PdfRenderedPage page) sinkFor, {
    required PdfPages pages,
    required PdfRenderSize size,
    required PdfRenderFormat format,
  }) => _deliver(
    EngineOp.render,
    {
      'pageIndices': _pages(pages),
      ..._renderSizeArgs(size),
      'format': codec.putEnum(format),
    },
    sinkFor,
    codec.decodeRenderFrame,
  );

  /// The engine writes each image as a header, data frames and an end frame,
  /// the same chunked wire as a render; the sink [sinkFor] returns is opened
  /// at the header and closed at the end frame.
  @override
  PdfTask<List<PdfImage>> extractImages(
    FutureOr<DataSink> Function(PdfImage image) sinkFor, {
    required PdfPages pages,
  }) => _deliver(
    EngineOp.extractImages,
    {'pageIndices': _pages(pages)},
    sinkFor,
    codec.decodeImageFrame,
  );

  /// Runs [op] with a [_ChunkDelivery] as its sink and resolves with the
  /// items delivered.
  PdfTask<List<H>> _deliver<H>(
    EngineOp op,
    Map<String, Object?> arguments,
    FutureOr<DataSink> Function(H item) sinkFor,
    codec.ChunkFrame<H> Function(Map<String, Object?> frame) decode,
  ) {
    return PdfTask.group((hook) async {
      final delivery = _ChunkDelivery<H>(sinkFor, hook, decode);
      try {
        final res = await _bridge._transport.execute(
          bin.encodeRequest(op.wire, {
            'handleId': _handleId,
            ...arguments,
            'hasSink': true,
          }),
          sinks: [delivery],
          cancel: hook,
        );
        bin.decodeResponse(res.bytes);
        // The engine's last chunk is flushed as it finishes, so a sink that
        // failed on it did not fail the op: the caller's error ends the
        // op all the same.
        if (delivery.failure case final failure?) {
          Error.throwWithStackTrace(failure.error, failure.stack);
        }
        if (delivery.dropped) throw const PdfCancelled();
      } catch (_) {
        await delivery.abandon();
        // A sink or factory that threw made the engine fail its write; the
        // caller's error is the cause, the engine's is its echo.
        if (delivery.failure case final failure?) {
          Error.throwWithStackTrace(failure.error, failure.stack);
        }
        rethrow;
      }
      return delivery.items;
    });
  }

  @override
  PdfTask<List<PdfSignatureInfo>> get signatures => _exec(
    EngineOp.getSignatures,
    {},
  ).map((map) => codec.decodeSignatures(map));

  @override
  PdfTask<bool> verifySignatures() => _exec(
    EngineOp.verifySignatures,
    {},
  ).map((map) => codec.decodeVerifySignatures(map));

  @override
  PdfTask<PdfValidationResult> validatePdfA({int level = 2}) => _exec(
    EngineOp.validatePdfA,
    {'level': level},
  ).map((map) => codec.decodeValidationResult(map));

  @override
  PdfTask<bool> validatePdfUa({int level = 1}) => _exec(
    EngineOp.validatePdfUa,
    {'level': level},
  ).map((map) => codec.decodeValidatePdfUa(map));

  @override
  PdfTask<List<PdfBookmarkSplit>> planSplitByBookmarks() => _exec(
    EngineOp.planSplitByBookmarks,
    {},
  ).map((map) => codec.decodeBookmarkSplits(map));

  @override
  PdfTask<PdfPageClassification> classifyPage(int page) => _exec(
    EngineOp.classifyPage,
    {'page': page},
  ).map((map) => codec.decodeClassifyPage(map));

  @override
  PdfTask<PdfDocumentClassification> classifyDocument() => _exec(
    EngineOp.classifyDocument,
    {},
  ).map((map) => codec.decodeClassifyDocument(map));

  @override
  PdfTask<List<PdfFormField>> get formFields =>
      _exec(EngineOp.formFields, {}).map(codec.decodeFormFields);

  @override
  PdfTask<void> exportFormData(DataSink output, {required String format}) =>
      _exec(EngineOp.exportFormData, {'format': format}, sinks: [output]);

  @override
  PdfTask<PdfXfaInfo?> get xfa =>
      _exec(EngineOp.xfa, {}).map(codec.decodeXfaInfo);

  @override
  PdfTask<List<PdfAttachment>> get attachments =>
      _exec(EngineOp.attachments, {}).map(codec.decodeAttachments);

  @override
  PdfTask<List<PdfAnnotationInfo>> annotations(int page) =>
      _exec(EngineOp.annotations, {'page': page}).map(codec.decodeAnnotations);

  @override
  PdfTask<void> extractAttachment(String name, DataSink output) =>
      _exec(EngineOp.extractAttachment, {'name': name}, sinks: [output]);

  @override
  Future<void> dispose() async {
    await _bridge._exec(EngineOp.docDispose, {'handleId': _handleId});
    if (_resourceId != null) {
      await _bridge._transport.releaseSource(_resourceId);
    }
  }
}

/// The sink the engine writes a chunked sequence to (the pages of a render,
/// the images of an extraction): it cuts the stream into frames, hands each
/// item to the caller's sink and keeps nothing of its bytes. The items are of
/// type [H], the descriptor the header frame carries.
///
/// The transport awaits each [write] before the engine sends the next chunk,
/// so a slow caller sink slows the engine (back-pressure). A frame is at
/// most one data frame (256 KiB) plus a few bytes, and a chunk is the
/// transport's 256 KiB buffer, so what waits here is bounded by both.
class _ChunkDelivery<H> implements DataSink {
  _ChunkDelivery(this._sinkFor, this._hook, this._decode);

  final FutureOr<DataSink> Function(H item) _sinkFor;

  final codec.ChunkFrame<H> Function(Map<String, Object?> frame) _decode;

  /// Once it is cancelled the frames still in flight are dropped, never
  /// written: a cancel stops the writes to the caller's sinks.
  final CancelHook _hook;

  /// The items whose end frame arrived: their sinks are closed.
  final items = <H>[];

  /// The first error the caller's factory or sink threw, if any.
  ({Object error, StackTrace stack})? failure;

  /// Whether a cancel made it drop frames the engine had already written: the
  /// render then ends cancelled even when the engine finished.
  bool dropped = false;

  final _pending = BytesBuilder(copy: false);
  DataSink? _open;
  H? _openItem;

  @override
  Future<void> write(Uint8List chunk) async {
    try {
      _pending.add(chunk);
      final bytes = _pending.takeBytes();
      // Every frame the chunk holds is counted before the first is taken,
      // as the engine wrote them all before this chunk was delivered.
      final frames = <Uint8List>[];
      var at = 0;
      while (bytes.length - at >= 4) {
        final length = ByteData.sublistView(
          bytes,
          at,
          at + 4,
        ).getUint32(0, Endian.little);
        // A zero length ends the stream: no frame follows.
        if (length == 0) {
          at += 4;
          continue;
        }
        if (bytes.length - at < 4 + length) break;
        frames.add(Uint8List.sublistView(bytes, at + 4, at + 4 + length));
        at += 4 + length;
        onEngineFrame?.call();
      }
      if (at < bytes.length) _pending.add(Uint8List.sublistView(bytes, at));
      for (final frame in frames) {
        if (_hook.isCancelled) {
          dropped = true;
          break;
        }
        await _take(_decode(bin.decodeResponse(frame)));
      }
    } catch (error, stack) {
      failure ??= (error: error, stack: stack);
      rethrow;
    }
  }

  Future<void> _take(codec.ChunkFrame<H> frame) async {
    switch (frame) {
      case codec.ChunkHeader<H>(:final item):
        await abandon();
        _openItem = item;
        _open = await _sinkFor(item);
      case codec.ChunkData<H>(:final bytes):
        final sink = _open;
        if (sink == null) {
          throw const PdfEngineError('a data frame came before its header');
        }
        await sink.write(bytes);
      case codec.ChunkEnd<H>():
        final sink = _open;
        if (sink == null) {
          throw const PdfEngineError('an end frame came before its header');
        }
        final item = _openItem as H;
        _open = null;
        _openItem = null;
        await sink.close();
        items.add(item);
    }
  }

  /// Closes the sink of the item in flight, if any. A sink that fails to
  /// close here is not reported: whatever stopped the op is the error.
  Future<void> abandon() async {
    final sink = _open;
    _open = null;
    _openItem = null;
    if (sink == null) return;
    try {
      await sink.close();
    } catch (_) {
      // The error that stopped the op is the one reported.
    }
  }

  @override
  void close() {}
}
