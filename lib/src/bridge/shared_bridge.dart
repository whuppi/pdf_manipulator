import 'dart:async';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/pdf_transport.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/errors.dart';

import 'package:pdf_manipulator/src/bridge/protocol/binary_codec.dart' as bin;
import 'package:pdf_manipulator/src/bridge/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/bridge/protocol/op.dart';

part 'shared_bridge_doc.dart';
part 'shared_bridge_editor.dart';
part 'shared_bridge_builder.dart';

/// One bridge class for both platforms. Encodes ops to binary,
/// sends via PdfTransport, decodes binary response.
///
/// O(1)-memory I/O: source bytes served on demand via DataSource,
/// output chunks flow via DataSink. The binary codec handles only
/// the ~200 bytes of op arguments — PDF data travels through the
/// transport's dedicated I/O channels.
class SharedBridge extends PdfBridge {
  /// Creates a bridge backed by [_transport].
  SharedBridge(this._transport);
  final PdfTransport _transport;

  @override
  PdfIoMode? get ioMode => _transport.ioMode;

  @override
  Future<PdfIoMode> ensureInitialized() => _transport.ensureInitialized();

  /// Encode → send → decode. The core pattern for every operation.
  /// Births the op's [PdfTask]: the hook created here is what
  /// task.cancel() drives, and the Router binds it to the lane job.
  PdfTask<Map<String, Object?>> _exec(
    EngineOp op,
    Map<String, Object?> args, {
    List<DataSource> sources = const [],
    List<DataSink> sinks = const [],
  }) {
    final hook = CancelHook();
    final future = () async {
      final req = bin.encodeRequest(op.wire, args);
      final res = await _transport.execute(
        req,
        sources: sources,
        sinks: sinks,
        cancel: hook,
      );
      final map = bin.decodeResponse(res.bytes);
      if (map.containsKey('error')) {
        // Typed by contract: every engine failure is a PdfError, so
        // callers can pattern-match it — and so it can never be
        // mistaken for a StateError, which is reserved for programmer
        // errors (use after dispose).
        throw PdfEngineError(map['error'] as String? ?? 'Engine error');
      }
      return map;
    }();
    return PdfTask.internal(future, hook);
  }

  /// Execute keeping sources[0] alive for handle lifetime.
  PdfTask<({Map<String, Object?> map, int? resourceId})> _execKeepSource(
    EngineOp op,
    Map<String, Object?> args, {
    required DataSource source,
  }) {
    final hook = CancelHook();
    final future = () async {
      final req = bin.encodeRequest(op.wire, args);
      final res = await _transport.execute(
        req,
        sources: [source],
        keepSources: {0},
        cancel: hook,
      );
      final map = bin.decodeResponse(res.bytes);
      if (map.containsKey('error')) {
        // Engine failures are PdfError, never StateError (reserved
        // for use-after-dispose) — same contract as [_exec].
        throw PdfEngineError(map['error'] as String? ?? 'Engine error');
      }
      return (map: map, resourceId: res.resourceIds[0]);
    }();
    return PdfTask.internal(future, hook);
  }

  // ══════════════════════════════════════════════════════════════════
  // Document handle — open once, query many, dispose
  // ══════════════════════════════════════════════════════════════════

  @override
  PdfTask<BridgeDocHandle> open(DataSource source, {String? password}) =>
      _execKeepSource(EngineOp.open, {
        'sourceLength': source.length,
        'password': password,
      }, source: source).map((res) {
        final handleId = res.map['handleId'] as int;
        return _SharedDocHandle(this, handleId, res.map, res.resourceId);
      });

  // ══════════════════════════════════════════════════════════════════
  // One-shot ops — source consumed in one call, no handle
  // ══════════════════════════════════════════════════════════════════

  @override
  PdfTask<void> sign(
    DataSource source,
    DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    final args = <String, Object?>{
      'sourceLength': source.length,
      'reason': reason,
      'location': location,
    };
    switch (credentials) {
      case PdfPkcs12Credentials(:final data, :final password):
        args['certificate'] = data;
        args['certificatePassword'] = password;
      case PdfPemCredentials(:final certPem, :final keyPem):
        args['certPem'] = certPem;
        args['keyPem'] = keyPem;
    }
    return _exec(EngineOp.sign, args, sources: [source], sinks: [output]);
  }

  @override
  PdfTask<void> convertTo(
    DataSource source,
    DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  }) => _exec(
    EngineOp.convertTo,
    {
      'sourceLength': source.length,
      'format': format.name,
      'password': password,
    },
    sources: [source],
    sinks: [output],
  );

  @override
  PdfTask<void> convertToPdf(
    DataSource document,
    DataSink output, {
    required PdfDocumentFormat format,
  }) => _exec(
    EngineOp.convertToPdf,
    {'sourceLength': document.length, 'format': format.name},
    sources: [document],
    sinks: [output],
  );

  // ══════════════════════════════════════════════════════════════════
  // Editor + Builder — handle-based sessions
  // ══════════════════════════════════════════════════════════════════

  @override
  PdfTask<BridgeEditorHandle> openEditor(
    DataSource source, {
    String? password,
  }) =>
      _execKeepSource(EngineOp.editorOpen, {
        'sourceLength': source.length,
        'password': password,
      }, source: source).map((res) {
        final handleId = codec.decodeEditorOpen(res.map);
        return _SharedEditorHandle(this, handleId, res.resourceId);
      });

  @override
  PdfTask<BridgeBuilderHandle> createBuilder() =>
      _exec(EngineOp.builderCreate, {}).map((map) {
        final handleId = map['handleId'] as int;
        return _SharedBuilderHandle(this, handleId);
      });

  @override
  Future<void> dispose() {
    // In-flight ops resolve as PdfCancelled when their lanes die;
    // each op's PdfTask absorbs the cancelled outcome for
    // fire-and-forget callers, so nothing needs silencing here.
    return _transport.dispose();
  }

  // ── Helpers ──

  List<int> _resolvePages(PdfPages pages, int pageCount) => switch (pages) {
    PdfPagesAll() => List.generate(pageCount, (i) => i),
    PdfPagesSingle(:final index) => [index],
    PdfPagesList(:final indices) => indices,
    PdfPagesRange(:final start, :final end) => List.generate(
      end - start,
      (i) => start + i,
    ),
  };

  String _encodeExtractionFormat(PdfExtractionFormat format) =>
      switch (format) {
        PdfExtractionFormat.auto => 'auto',
        PdfExtractionFormat.text => 'text',
        PdfExtractionFormat.markdown => 'markdown',
        PdfExtractionFormat.html => 'html',
        PdfExtractionFormat.plainText => 'plainText',
      };
}

// ── O(1)-memory framed sink for streaming ops ──────────────────────

class _FrameCollectorSink implements DataSink {
  final _buffer = BytesBuilder(copy: false);
  final _frames = <Uint8List>[];

  List<Uint8List> get frames => _frames;

  @override
  void write(Uint8List chunk) {
    _buffer.add(chunk);
    _parseFrames();
  }

  void _parseFrames() {
    while (true) {
      final bytes = _buffer.toBytes();
      if (bytes.length < 4) break;
      final len = ByteData.sublistView(bytes).getUint32(0, Endian.little);
      if (len == 0) {
        _buffer.clear();
        if (bytes.length > 4) {
          _buffer.add(Uint8List.sublistView(bytes, 4));
        }
        break;
      }
      if (bytes.length < 4 + len) break;
      _frames.add(Uint8List.sublistView(bytes, 4, 4 + len));
      _buffer.clear();
      if (bytes.length > 4 + len) {
        _buffer.add(Uint8List.sublistView(bytes, 4 + len));
      }
    }
  }
}
