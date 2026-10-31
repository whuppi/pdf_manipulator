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

  /// Frames reach the stream as the engine writes them. An error
  /// response ends the stream with its error after the frames already
  /// delivered; cancelling the subscription cancels the job.
  @override
  Stream<RenderedPage> render({required PdfPages pages, PdfRenderSize? size}) =>
      _streamFrames((hook, sink) async {
        final req = bin.encodeRequest(EngineOp.render.wire, {
          'handleId': _handleId,
          'pageIndices': _pages(pages),
          ..._renderSizeArgs(size),
          'hasSink': true,
        });
        final res = await _bridge._transport.execute(
          req,
          sinks: [sink],
          cancel: hook,
        );
        bin.decodeResponse(res.bytes);
      }, codec.decodeRenderedPage);

  /// Same delivery order as [render]: frames as written, the final
  /// response last, a cancelled subscription cancels the job.
  @override
  Stream<ExtractedImage> extractImages({required PdfPages pages}) =>
      _streamFrames((hook, sink) async {
        for (final page in _pages(pages)) {
          if (hook.isCancelled) return;
          final req = bin.encodeRequest(EngineOp.extractImages.wire, {
            'handleId': _handleId,
            'page': page,
            'hasSink': true,
          });
          final res = await _bridge._transport.execute(
            req,
            sinks: [sink],
            cancel: hook,
          );
          bin.decodeResponse(res.bytes);
        }
      }, codec.decodePdfImage);

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
