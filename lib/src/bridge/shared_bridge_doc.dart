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

  PdfTask<Map<String, Object?>> _exec(
    EngineOp op,
    Map<String, Object?> args, {
    List<DataSink> sinks = const [],
  }) => _bridge._exec(
    op,
    {'handleId': _handleId, ...args},
    sinks: sinks,
    scope: scope,
  );

  @override
  Map<String, Object?> get openResult => _openResult;

  /// The pages a read names; every page when [pages] is null.
  List<int> _pageList(PdfPages? pages) =>
      codec.resolvePageIndices(pages, _pageCount);

  @override
  PdfTask<List<PdfPageInfo>> pageInfo({PdfPages? pages}) => _exec(
    EngineOp.pageInfo,
    {'pageIndices': _pageList(pages)},
  ).map(codec.decodePageInfos);

  @override
  PdfTask<void> extractText(
    DataSink output, {
    PdfPages? pages,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  }) {
    final fmt = codec.putEnum(format);
    return PdfTask.group((hook) async {
      if (pages == null || pages is PdfPagesAll) {
        // One frame per page: the text reaches the sink as each frame
        // arrives, and the library never holds the document's text.
        final sink = _FrameSink((frame) async {
          onEngineFrame?.call();
          await output.write(
            utf8.encode(codec.decodeExtractResult(bin.decodeResponse(frame))),
          );
        });
        await hook.guard(
          _exec(
            EngineOp.extract,
            {'format': fmt, 'hasSink': true},
            sinks: [sink],
          ),
        );
        return;
      }
      for (final page in _pageList(pages)) {
        final text = codec.decodeExtractResult(
          await hook.guard(
            _exec(EngineOp.extract, {'format': fmt, 'page': page}),
          ),
        );
        await output.write(utf8.encode(text));
      }
    });
  }

  @override
  PdfTask<List<PdfSearchHit>> search({required String query, PdfPages? pages}) {
    return PdfTask.group((hook) async {
      if (pages == null || pages is PdfPagesAll) {
        return codec.decodeSearchHits(
          await hook.guard(_exec(EngineOp.search, {'query': query})),
        );
      }
      final all = <PdfSearchHit>[];
      for (final page in _pageList(pages)) {
        all.addAll(
          codec.decodeSearchHits(
            await hook.guard(
              _exec(EngineOp.search, {'query': query, 'page': page}),
            ),
          ),
        );
      }
      return all;
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
    PdfPages? pages,
    required PdfRenderSize size,
    required PdfRenderFormat format,
  }) => _deliver(
    EngineOp.render,
    {
      'pageIndices': _pageList(pages),
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
    PdfPages? pages,
  }) => _deliver(
    EngineOp.extractImages,
    {'pageIndices': _pageList(pages)},
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
  PdfTask<List<PdfSignatureInfo>> signatures() => _exec(
    EngineOp.getSignatures,
    {},
  ).map((map) => codec.decodeSignatures(map));

  @override
  PdfTask<bool> verifySignatures() => _exec(
    EngineOp.verifySignatures,
    {},
  ).map((map) => codec.decodeVerifySignatures(map));

  @override
  PdfTask<PdfValidationResult> validatePdfA({required PdfALevel level}) =>
      _exec(EngineOp.validatePdfA, {
        'level': codec.putEnum(level),
      }).map((map) => codec.decodeValidationResult(map));

  @override
  PdfTask<PdfValidationResult> validatePdfUa({required PdfUaLevel level}) =>
      _exec(EngineOp.validatePdfUa, {
        'level': codec.putEnum(level),
      }).map((map) => codec.decodeValidationResult(map));

  @override
  PdfTask<List<PdfBookmarkRange>> bookmarkRanges() => _exec(
    EngineOp.planSplitByBookmarks,
    {},
  ).map((map) => codec.decodeBookmarkRanges(map));

  @override
  PdfTask<List<PdfPageClassification>> classifyPages({PdfPages? pages}) {
    return PdfTask.group((hook) async {
      final out = <PdfPageClassification>[];
      for (final page in _pageList(pages)) {
        out.add(
          codec.decodeClassifyPage(
            await hook.guard(_exec(EngineOp.classifyPage, {'page': page})),
            page: page,
          ),
        );
      }
      return out;
    });
  }

  @override
  PdfTask<PdfDocumentType> classifyDocument() => _exec(
    EngineOp.classifyDocument,
    {},
  ).map((map) => codec.decodeClassifyDocument(map));

  @override
  PdfTask<List<PdfFormField>> formFields() =>
      _exec(EngineOp.formFields, {}).map(codec.decodeFormFields);

  @override
  PdfTask<void> exportFormData(DataSink output, {required String format}) =>
      _exec(EngineOp.exportFormData, {'format': format}, sinks: [output]);

  @override
  PdfTask<PdfXfaInfo?> xfa() =>
      _exec(EngineOp.xfa, {}).map(codec.decodeXfaInfo);

  @override
  PdfTask<List<PdfAttachment>> attachments() =>
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
