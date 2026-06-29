// Pdf — entry point. Lifecycle + handle creation only.
//
//   pdf_doc.dart       — read-only doc queries (open handle)
//   pdf_editor.dart    — mutations (edit handle, editor = mutations only)
//   pdf_builder.dart   — create from scratch (build handle)
//   pdf_standalone.dart — source in, sink out, no handle (sign, convert)
//   pdf_sugar.dart     — convenience wrappers over editor/builder

import 'package:pdf_manipulator/src/ops/pdf_editor.dart';
import 'package:pdf_manipulator/src/ops/pdf_builder.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:meta/meta.dart';

import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/create_bridge.dart';
import 'package:pdf_manipulator/src/bridge/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/ops/pdf_doc.dart';

/// Entry point for all PDF operations — open, edit, build, and standalone ops.
class Pdf {
  /// Creates a new [Pdf] instance with an optional engine configuration.
  Pdf({PdfConfig? config}) : _bridge = createBridge(config: config);

  final PdfBridge _bridge;
  bool _disposed = false;

  /// INTERNAL — the door PdfStandalone and PdfSugar extensions use
  /// (extensions in other files cannot reach `_bridge`). Consumers
  /// must never call this; the analyzer flags any outside-package
  /// use via `@internal`.
  @internal
  PdfBridge get bridge {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
    return _bridge;
  }

  /// Detected I/O mode. Null before [ensureInitialized] or first op.
  PdfIoMode? get ioMode => _bridge.ioMode;

  /// Eagerly initialize the engine and return the detected I/O mode.
  ///
  /// Call this at startup to detect the mode and react accordingly.
  /// On web, [PdfIoMode.opfs] means the engine will pre-copy each
  /// source file to disk before processing (O(N) latency + disk).
  /// You may want to show a warning or prompt the user to enable
  /// COOP/COEP headers for streaming mode.
  ///
  /// Idempotent — second call returns the cached result instantly.
  Future<PdfIoMode> ensureInitialized() {
    _check();
    return _bridge.ensureInitialized();
  }

  void _check() {
    if (_disposed) throw StateError('This Pdf instance has been disposed');
  }

  // ── Document handle — open once, query many, dispose ──

  /// Opens a PDF document for read-only queries.
  PdfTask<PdfDoc> open(DataSource source, {String? password}) {
    _check();
    return _bridge.open(source, password: password).map((handle) async {
      try {
        final map = handle.openResult;
        return PdfDoc.internal(
          handle,
          pageCount: map['pageCount'] as int? ?? 0,
          version: map['version'] as String? ?? '1.0',
          pages: codec.decodePageList(map),
          title: map['title'] as String?,
          author: map['author'] as String?,
          subject: map['subject'] as String?,
          keywords: map['keywords'] as String?,
          producer: map['producer'] as String?,
          creator: map['creator'] as String?,
          creationDate: map['creationDate'] as String?,
          isEncrypted: map['isEncrypted'] as bool? ?? false,
          requiresPassword: map['requiresPassword'] as bool? ?? false,
          isTagged: map['isTagged'] as bool? ?? false,
          encryptionAlgorithm: codec.decodeEncryptionAlgorithmFromMap(map),
          permissions: codec.decodePermissionsFromMap(map),
        );
      } catch (e) {
        await handle.dispose();
        rethrow;
      }
    });
  }

  // ── Editor handle — open, mutate, save, dispose ──

  /// Opens a PDF document for editing (mutate, then save).
  PdfTask<PdfEditor> edit(DataSource source, {String? password}) {
    _check();
    return PdfTask.group((hook) async {
      final doc = await hook.guard(open(source, password: password));
      try {
        final handle = await hook.guard(
          _bridge.openEditor(source, password: password),
        );
        return PdfEditor.internal(
          _bridge,
          handle,
          sourceEncryption: doc.encryptionAlgorithm,
          sourcePermissions: doc.permissions,
          password: password,
          sourceDoc: doc,
        );
      } catch (e) {
        await doc.dispose();
        rethrow;
      }
    });
  }

  // ── Builder handle — create from scratch, save, dispose ──

  /// Creates a new PDF document from scratch.
  PdfTask<PdfBuilder> build() {
    _check();
    return _bridge.createBuilder().map(
      (handle) => PdfBuilder.internal(_bridge, handle),
    );
  }

  // ── Lifecycle ──

  /// Releases engine resources. Idempotent.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _bridge.dispose();
  }
}
