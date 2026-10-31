// Pdf — entry point: lifecycle, handle creation and the batch combinators.
//
// One library, so the pieces that must stay private can: the handle
// creators sit behind `_own`, and the base classes of the one-shots hide
// `_run`.
//
//   pdf_doc.dart        — read-only doc queries (open handle)
//   pdf_reads.dart      — every read, declared once, for PdfDoc and pdf.reads
//   pdf_editor.dart     — the editor handle (edit handle)
//   pdf_edits.dart      — every edit, declared once, for PdfEditor and pdf.edits
//   pdf_builder.dart    — create from scratch (build handle)
//   pdf_standalone.dart — source in, sink out, no handle (sign, convert)
//   pdf_sugar.dart      — convenience wrappers over editor/builder

import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:pdf_manipulator/src/bridge/create_bridge.dart';
import 'package:pdf_manipulator/src/bridge/pdf_bridge.dart';
import 'package:pdf_manipulator/src/bridge/protocol/codec.dart' as codec;
import 'package:pdf_manipulator/src/io/memory_sink.dart';
import 'package:pdf_manipulator/src/keep/record_use_shim.dart';
import 'package:pdf_manipulator/src/ops/handle_backstop.dart';
import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/errors.dart';
import 'package:pdf_manipulator/src/types/pdf_annotation.dart';
import 'package:pdf_manipulator/src/types/pdf_attachment.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_form_field.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_info.dart';
import 'package:pdf_manipulator/src/types/pdf_page_image.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_render.dart';
import 'package:pdf_manipulator/src/types/pdf_sanitize_options.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_table.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

part 'pdf_builder.dart';
part 'pdf_doc.dart';
part 'pdf_edits.dart';
part 'pdf_editor.dart';
part 'pdf_reads.dart';
part 'pdf_standalone.dart';
part 'pdf_sugar.dart';

/// A handle the library can open and must close: the common face of
/// [PdfDoc], [PdfEditor] and [PdfBuilder] for `_own`.
abstract interface class _Handle {
  Future<void> dispose();
}

/// Opens a handle, runs [body] on it and disposes it in a `finally`: after
/// success, after an error and after a cancel. The one place a handle the
/// library creates is closed, so a handle the library opens never leaks.
///
/// [opening] is the door call (`pdf.open(...)`, `pdf.edit(...)`,
/// `pdf.build()`); the handle-creator wall allows those calls only here, as
/// the argument of `_own`, and in the doors themselves. [body] guards each
/// engine step it starts with `hook.guard`, so cancelling the returned task
/// cancels the step in flight.
PdfTask<R> _own<H extends _Handle, R>(
  PdfTask<H> opening,
  Future<R> Function(H handle, CancelHook hook) body,
) => PdfTask.group((hook) async {
  final handle = await hook.guard(opening);
  try {
    return await body(handle, hook);
  } finally {
    await handle.dispose();
  }
});

/// Entry point for all PDF operations — open, edit, build, and standalone ops.
class Pdf {
  /// Creates a new [Pdf] instance with an optional engine configuration.
  Pdf({PdfConfig? config}) : _bridge = createBridge(config: config);

  final PdfBridge _bridge;
  bool _disposed = false;

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

  /// Registers a fallback font for baking form values the field's own font
  /// cannot render (CJK or emoji) — used by `flattenForms` and the
  /// `/NeedAppearances` regeneration paths.
  ///
  /// [bytes] is a complete TTF/OTF face. It reaches every current and
  /// future worker of this instance; register once, before the operations
  /// that need it. Without a registered font (and without the engine's
  /// optional compiled-in fallback) such values keep the plain `/DA`
  /// appearance and the flatten records a warning.
  PdfTask<void> registerFallbackFont(
    PdfFallbackFontKind kind,
    Uint8List bytes,
  ) {
    return _bridge.registerFallbackFont(kind.wire, bytes);
  }

  void _check() {
    if (_disposed) {
      throw const PdfDisposed('This Pdf instance has been disposed');
    }
  }

  PdfBridge get _live {
    _check();
    return _bridge;
  }

  // ── Handles — whoever opens one closes it ──

  /// Opens a PDF document for read-only queries.
  ///
  /// The handle is yours: call [PdfDoc.dispose] when you are done, on every
  /// path including errors. For one read, use [read], which opens the
  /// document, runs your code and always closes it. A handle you forget is
  /// released when the garbage collector frees it, and in debug mode the
  /// leak is logged with where it was opened; that is a backstop, not a
  /// plan.
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

  /// Opens a PDF document for editing (mutate, then save).
  ///
  /// The handle is yours: call [PdfEditor.dispose] when you are done, on
  /// every path including errors. For one edit pass, use [rewrite] (several
  /// edits, one save) or [edits] (one edit), which open, save and always
  /// close. A handle you forget is released when the garbage collector
  /// frees it, and in debug mode the leak is logged with where it was
  /// opened; that is a backstop, not a plan.
  PdfTask<PdfEditor> edit(DataSource source, {String? password}) {
    _check();
    return PdfTask.group((hook) async {
      final handle = await hook.guard(
        _bridge.openEditor(source, password: password),
      );
      return PdfEditor.internal(_bridge, handle);
    });
  }

  /// Creates a new PDF document from scratch.
  ///
  /// The handle is yours: call [PdfBuilder.dispose] when you are done, on
  /// every path including errors. For one document, use [buildTo], which
  /// builds, saves and always closes. A handle you forget is released when
  /// the garbage collector frees it, and in debug mode the leak is logged
  /// with where it was opened; that is a backstop, not a plan.
  PdfTask<PdfBuilder> build() {
    _check();
    return _bridge.createBuilder().map(
      (handle) => PdfBuilder.internal(_bridge, handle),
    );
  }

  // ── Batch combinators — the library opens the handle and closes it ──

  /// Opens [source], runs [body] on the document and closes it, after
  /// success, after an error and after a cancel. The handle is the
  /// library's: do not keep it past [body].
  ///
  /// Cancelling the task stops the open; a task [body] started is cancelled
  /// through its own [PdfTask].
  PdfTask<T> read<T>(
    DataSource source,
    Future<T> Function(PdfDoc doc) body, {
    String? password,
  }) => _own(open(source, password: password), (doc, hook) => body(doc));

  /// Opens [source], runs [edits] on the editor, saves the result to
  /// [output] with [options] and closes the editor, after success, after an
  /// error and after a cancel. Several edits make one parse and one save,
  /// all or nothing. The handle is the library's: do not keep it past
  /// [edits]. Nothing is written when [edits] throws.
  ///
  /// Cancelling the task stops the open and the save; a task [edits] started
  /// is cancelled through its own [PdfTask].
  PdfTask<void> rewrite(
    DataSource source,
    DataSink output,
    Future<void> Function(PdfEditor editor) edits, {
    String? password,
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) => _own(edit(source, password: password), (editor, hook) async {
    await edits(editor);
    await hook.guard(editor.save(output, options: options));
  });

  /// Creates a document, runs [body] on its builder, saves it to [output]
  /// and closes the builder, after success, after an error and after a
  /// cancel. Page content is Flate-compressed unless [deflate] is false.
  /// The handle is the library's: do not keep it past [body]. Nothing is
  /// written when [body] throws.
  ///
  /// Cancelling the task stops the save; a task [body] started is cancelled
  /// through its own [PdfTask].
  PdfTask<void> buildTo(
    DataSink output,
    Future<void> Function(PdfBuilder builder) body, {
    bool deflate = true,
  }) => _own(build(), (builder, hook) async {
    await body(builder);
    await hook.guard(builder.save(output, compress: deflate));
  });

  /// Applies one edit of [PdfEdits] to [source] and writes the result to
  /// [output]: the object returned runs exactly one call, then the document
  /// is saved and closed. A second call throws [PdfInvalidArgument]; use
  /// [rewrite] for several edits.
  ///
  /// ```dart
  /// await pdf.edits(source, output).addWatermark(0, 'DRAFT');
  /// ```
  PdfEdits edits(
    DataSource source,
    DataSink output, {
    String? password,
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) {
    _check();
    return _OneShotEdits(this, source, output, password, options);
  }

  /// Applies one read of [PdfReads] to [source]: the object returned runs
  /// exactly one call, then the document is closed. A second call throws
  /// [PdfInvalidArgument]; use [read] for several reads.
  ///
  /// ```dart
  /// final hits = await pdf.reads(source)
  ///     .search(query: 'invoice', pages: const PdfPages.all());
  /// ```
  PdfReads reads(DataSource source, {String? password}) {
    _check();
    return _OneShotReads(this, source, password);
  }

  /// What the file says about itself without keeping it open: page count,
  /// version, metadata and encryption.
  PdfTask<PdfInfo> info(DataSource source, {String? password}) => read(
    source,
    (doc) async => PdfInfo(
      pageCount: doc.pageCount,
      version: doc.version,
      isTagged: doc.isTagged,
      title: doc.title,
      author: doc.author,
      subject: doc.subject,
      keywords: doc.keywords,
      producer: doc.producer,
      creator: doc.creator,
      creationDate: doc.creationDate,
      isEncrypted: doc.isEncrypted,
      requiresPassword: doc.requiresPassword,
      encryptionAlgorithm: doc.encryptionAlgorithm,
      permissions: doc.permissions,
    ),
    password: password,
  );

  /// Rewrites [source] to [output] with [options] and no edit: encrypt,
  /// decrypt, re-deflate. The document is closed afterwards.
  PdfTask<void> resave(
    DataSource source,
    DataSink output, {
    String? password,
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) => rewrite(
    source,
    output,
    (editor) async {},
    password: password,
    options: options,
  );

  // ── Lifecycle ──

  /// Releases engine resources. Idempotent.
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    await _bridge.dispose();
  }
}
