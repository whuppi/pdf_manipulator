// Pdf — entry point: lifecycle, the handle doors, the scope doors and every
// one-call door.
//
// One library, so the pieces that must stay private can: the handle
// creators sit behind `_own`, and the base classes of the one-shots hide
// `_run`.
//
//   pdf_doc.dart      — PdfDocument and its handle
//   pdf_reads.dart    — every read, declared once, for PdfDocument and pdf.readOnce
//   pdf_editor.dart   — PdfEditor and its handle
//   pdf_edits.dart    — every edit, declared once, for PdfEditor and pdf.editOnce
//   pdf_builder.dart  — create from scratch (PdfBuilder and its handle)
//   pdf_jobs.dart     — the bodies of the doors that open several handles

import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:meta/meta.dart';
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
import 'package:pdf_manipulator/src/types/pdf_image_info.dart';
import 'package:pdf_manipulator/src/types/pdf_image_page.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_document_info.dart';
import 'package:pdf_manipulator/src/types/pdf_metadata.dart';
import 'package:pdf_manipulator/src/types/pdf_image_placement.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_page_size.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_overlay.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_render.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_split.dart';
import 'package:pdf_manipulator/src/types/pdf_stroke.dart';
import 'package:pdf_manipulator/src/types/pdf_table.dart';
import 'package:pdf_manipulator/src/types/pdf_bitmap.dart';
import 'package:pdf_manipulator/src/types/pdf_input.dart';
import 'package:pdf_manipulator/src/types/pdf_link.dart';
import 'package:pdf_manipulator/src/types/pdf_point.dart';
import 'package:pdf_manipulator/src/types/pdf_radio_option.dart';
import 'package:pdf_manipulator/src/types/pdf_field_scripts.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_text_style.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/pdf_search_hit.dart';

part 'pdf_builder.dart';
part 'pdf_doc.dart';
part 'pdf_edits.dart';
part 'pdf_editor.dart';
part 'pdf_jobs.dart';
part 'pdf_reads.dart';

/// A handle the library can open and must close: the common face of
/// [PdfDocumentHandle], [PdfEditorHandle] and [PdfBuilderHandle] for `_own`.
abstract interface class _Handle {
  /// Makes every op issued on the handle from now on register its cancel
  /// with [hook].
  void _scopeTo(CancelHook hook);

  Future<void> dispose();
}

/// Opens a handle, runs [body] on it and disposes it in a `finally`: after
/// success, after an error and after a cancel. The one place a handle the
/// library creates is closed, so a handle the library opens never leaks.
///
/// [opening] is the door call (`pdf.open(...)`, `pdf.edit(...)`,
/// `pdf.build()`); the handle-creator wall allows those calls only here, as
/// the argument of `_own`, and in the doors themselves. The handle carries
/// the returned task's hook: cancelling the task cancels every op [body]
/// has in flight on the handle, and an op issued after the cancel fails at
/// once with [PdfCancelled]. [body] guards the steps it starts on other
/// handles with `hook.guard`.
///
/// [output] is the sink the whole job writes to, if it has one. A job that
/// ends cancelled or failed aborts it, even when the save into it had
/// finished: a combinator that reports [PdfCancelled] leaves no output.
PdfTask<R> _own<H extends _Handle, R>(
  PdfTask<H> opening,
  Future<R> Function(H handle, CancelHook hook) body, {
  DataSink? output,
}) => PdfTask.group((hook) async {
  try {
    final handle = await hook.guard(opening);
    try {
      handle._scopeTo(hook);
      final result = await body(handle, hook);
      // A body that caught its ops' cancellation still ends cancelled: one
      // cancel ends the whole job.
      if (hook.isCancelled) throw const PdfCancelled();
      return result;
    } finally {
      await handle.dispose();
    }
  } catch (error) {
    if (output != null) await hook.abortSinks([output], error);
    rethrow;
  }
});

/// Entry point for all PDF operations — open, edit, build, and the ops that
/// need no handle.
///
/// Every door that opens a PDF takes a [PdfInput]: the source and, when the
/// file is encrypted, its password. The doors come in four kinds:
///
/// - handles you own and dispose: [open], [edit], [build];
/// - scopes where the library opens, runs your body, saves and closes:
///   [read], [rewrite], [buildTo];
/// - one-shots, one call on one document: [readOnce], [editOnce], [info],
///   [resave];
/// - doors over several documents or a policy: [merge], [split], [compress],
///   [imagesToPdf], and the ones that need no handle: [sign],
///   [convertToOffice], [convertFromOffice], [convertXfaToAcroForm].
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
  /// [font] holds a complete TTF/OTF face, which is read whole. It reaches
  /// every current and future worker of this instance; register once, before
  /// the operations that need it. Without a registered font (and without the
  /// engine's optional compiled-in fallback) such values keep the plain `/DA`
  /// appearance and the flatten records a warning.
  PdfTask<void> registerFallbackFont(
    PdfFallbackFontKind kind,
    DataSource font,
  ) {
    return _bridge.registerFallbackFont(kind.wire, font);
  }

  void _check() {
    if (_disposed) {
      throw StateError('This Pdf instance has been disposed');
    }
  }

  // ── Handles — whoever opens one closes it ──

  /// Opens a PDF document for read-only queries.
  ///
  /// The handle is yours: call [PdfDocumentHandle.dispose] when you are done,
  /// on every path including errors. For one read, use [read], which opens
  /// the document, runs your code and always closes it. A handle you forget
  /// is released when the garbage collector frees it, and in debug mode the
  /// leak is logged with where it was opened; that is a backstop, not a
  /// plan.
  ///
  /// A source [input] borrows stays the caller's; one it owns is closed with
  /// the handle.
  PdfTask<PdfDocumentHandle> open(PdfInput input) {
    _check();
    return PdfTask.group((hook) async {
      final lease = await input.opened;
      try {
        final handle = await hook.guard(
          _bridge.open(lease.source, password: input.password),
        );
        try {
          final map = handle.openResult;
          return PdfDocumentHandle.internal(
            handle,
            pageCount: map['pageCount'] as int? ?? 0,
            version: map['version'] as String? ?? '1.0',
            isTagged: map['isTagged'] as bool? ?? false,
            metadata: codec.decodeMetadata(map),
            encryption: codec.decodeEncryptionInfo(map),
            release: lease.release,
          );
        } catch (_) {
          await handle.dispose();
          rethrow;
        }
      } catch (_) {
        await lease.release();
        rethrow;
      }
    });
  }

  /// Opens a PDF document for editing (mutate, then save).
  ///
  /// The handle is yours: call [PdfEditorHandle.dispose] when you are done,
  /// on every path including errors. For one edit pass, use [rewrite]
  /// (several edits, one save) or [editOnce] (one edit), which open, save and
  /// always close. A handle you forget is released when the garbage
  /// collector frees it, and in debug mode the leak is logged with where it
  /// was opened; that is a backstop, not a plan.
  PdfTask<PdfEditorHandle> edit(PdfInput input) {
    _check();
    return PdfTask.group((hook) async {
      final lease = await input.opened;
      try {
        final handle = await hook.guard(
          _bridge.openEditor(lease.source, password: input.password),
        );
        return PdfEditorHandle.internal(this, handle, release: lease.release);
      } catch (_) {
        await lease.release();
        rethrow;
      }
    });
  }

  /// Creates a new PDF document from scratch.
  ///
  /// The handle is yours: call [PdfBuilderHandle.dispose] when you are done,
  /// on every path including errors. For one document, use [buildTo], which
  /// builds, saves and always closes. A handle you forget is released when
  /// the garbage collector frees it, and in debug mode the leak is logged
  /// with where it was opened; that is a backstop, not a plan.
  PdfTask<PdfBuilderHandle> build() {
    _check();
    return _bridge.createBuilder().map(PdfBuilderHandle.internal);
  }

  // ── Scopes — the library opens the handle and closes it ──

  /// Opens [input], runs [body] on the document and closes it, after
  /// success, after an error and after a cancel. The document is the
  /// library's: do not keep it past [body].
  ///
  /// Cancelling the task stops the open and every op [body] has running on
  /// `doc`, which resolves with [PdfCancelled]; an op [body] issues on `doc`
  /// after the cancel fails at once with [PdfCancelled].
  PdfTask<T> read<T>(
    PdfInput input,
    Future<T> Function(PdfDocument doc) body,
  ) => _own(open(input), (doc, hook) => body(doc));

  /// Opens [input], runs [body] on the editor, saves the result to [output]
  /// with [save] and closes the editor, after success, after an error and
  /// after a cancel. Several edits make one parse and one save, all or
  /// nothing. The editor is the library's: do not keep it past [body].
  /// Nothing is written when [body] throws, and [output] is aborted
  /// (`DataSink.abort`) when the call ends cancelled or failed.
  ///
  /// Cancelling the task stops the open, every op [body] has running on the
  /// editor, which resolves with [PdfCancelled], and the save: an op issued
  /// after the cancel fails at once with [PdfCancelled] and nothing is
  /// written to [output].
  PdfTask<void> rewrite(
    PdfInput input,
    DataSink output,
    Future<void> Function(PdfEditor editor) body, {
    PdfSave save = const PdfSave.full(),
  }) => _own(edit(input), (editor, hook) async {
    await body(editor);
    await hook.guard(editor.save(output, save: save));
  }, output: output);

  /// Creates a document, runs [body] on its builder, saves it to [output]
  /// and closes the builder, after success, after an error and after a
  /// cancel. Page content is Flate-compressed unless [deflate] is false.
  /// The builder is the library's: do not keep it past [body]. Nothing is
  /// written when [body] throws, and [output] is aborted (`DataSink.abort`)
  /// when the call ends cancelled or failed.
  ///
  /// Cancelling the task stops every op [body] has running on the builder
  /// and its page builders, which resolves with [PdfCancelled], and the
  /// save: an op issued after the cancel fails at once with [PdfCancelled]
  /// and nothing is written to [output].
  PdfTask<void> buildTo(
    DataSink output,
    Future<void> Function(PdfBuilder builder) body, {
    bool deflate = true,
  }) => _own(build(), (builder, hook) async {
    await body(builder);
    await hook.guard(builder.save(output, deflate: deflate));
  }, output: output);

  // ── One-shots — one call, generated from PdfReads and PdfEdits ──

  /// Applies one read of [PdfReads] to [input]: the object returned runs
  /// exactly one call, then the document is closed. A second call throws
  /// [StateError]; use [read] for several reads.
  ///
  /// ```dart
  /// final hits = await pdf.readOnce(input).search('invoice');
  /// ```
  PdfReads readOnce(PdfInput input) {
    _check();
    return _OneShotReads(this, input);
  }

  /// Applies one edit of [PdfEdits] to [input] and writes the result to
  /// [output]: the object returned runs exactly one call, then the document
  /// is saved with [save] and closed. A second call throws [StateError]; use
  /// [rewrite] for several edits.
  ///
  /// ```dart
  /// await pdf.editOnce(input, output).addWatermark('DRAFT');
  /// ```
  PdfEdits editOnce(
    PdfInput input,
    DataSink output, {
    PdfSave save = const PdfSave.full(),
  }) {
    _check();
    return _OneShotEdits(this, input, output, save);
  }

  /// What the file says about itself without keeping it open: page count,
  /// version, metadata and encryption.
  PdfTask<PdfDocumentInfo> info(PdfInput input) => read(
    input,
    (doc) async => PdfDocumentInfo(
      pageCount: doc.pageCount,
      version: doc.version,
      isTagged: doc.isTagged,
      metadata: doc.metadata,
      encryption: doc.encryption,
    ),
  );

  /// Rewrites [input] to [output] with [save] and no edit: encrypt,
  /// decrypt, re-deflate. The document is closed afterwards.
  PdfTask<void> resave(
    PdfInput input,
    DataSink output, {
    PdfSave save = const PdfSave.full(),
  }) => rewrite(input, output, (editor) async {}, save: save);

  // ── Several documents, or a policy ──

  /// Merges [inputs] into one document written to [output] with [save], in
  /// the order given: the first is the base and the others are appended
  /// whole. An empty [inputs] throws [ArgumentError]. To append only some
  /// pages of a document, use [rewrite] and `PdfEditor.appendPages`.
  PdfTask<void> merge(
    List<PdfInput> inputs,
    DataSink output, {
    PdfSave save = const PdfSave.full(),
  }) {
    _check();
    return _merge(this, inputs, output, save);
  }

  /// Splits [input] into the parts [by] says. [sinkFor] is asked for each
  /// part's sink before the part is written; the library closes it after the
  /// part, and aborts it first when the part fails or is cancelled. The parts
  /// delivered before a failure or a cancel stand. The task resolves with
  /// what each part holds, in order; the bytes went to the sinks.
  ///
  /// [PdfSplitRule.maxBytes] cuts a part at the most pages whose file stays
  /// within the limit, as the engine measures it; a page that alone exceeds
  /// the limit is a part of its own.
  PdfTask<List<PdfSplitPart>> split(
    PdfInput input,
    FutureOr<DataSink> Function(PdfSplitPart part) sinkFor, {
    required PdfSplitRule by,
  }) {
    _check();
    return _split(this, input, sinkFor, by);
  }

  /// Compresses the PDF: images through `PdfEdits.reduceImages` under
  /// [images] (`PdfImagePolicy.lossless` unless you pass another preset or
  /// your own numbers, so the picture never gets worse), then a full save
  /// with stream compression, one copy of identical images, ICC profiles and
  /// embedded fonts, the resource entries no content names dropped and
  /// garbage collection.
  ///
  /// It never makes a file larger: when the result is not smaller than
  /// [input], [output] receives the bytes of [input] unchanged.
  PdfTask<void> compress(
    PdfInput input,
    DataSink output, {
    PdfImagePolicy images = PdfImagePolicy.lossless,
  }) {
    _check();
    return _compress(this, input, output, images);
  }

  /// Creates a PDF from a list of images, one per page.
  ///
  /// Each page is the image's own size, one point per pixel, unless [page]
  /// names a size; [PdfImagePage.fit] says how the image fills it.
  PdfTask<void> imagesToPdf(
    List<DataSource> images,
    DataSink output, {
    PdfImagePage page = const PdfImagePage(),
    bool deflate = true,
  }) => buildTo(output, (builder) async {
    for (final image in images) {
      await builder.addImagePage(image, page: page);
    }
  }, deflate: deflate);

  // ── No handle — input in, output out ──

  /// Digitally signs a PDF with the given [credentials]. An encrypted
  /// [input] throws [PdfUnsupported]: the appended signature would have to
  /// be encrypted with the input's keys.
  PdfTask<void> sign(
    PdfInput input,
    DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  }) {
    _check();
    KeepRecord.op('signatures');
    return _withInput(
      input,
      output,
      (source) => _bridge.sign(
        source,
        output,
        credentials: credentials,
        reason: reason,
        location: location,
      ),
    );
  }

  /// Converts a PDF to an Office document of [format].
  PdfTask<void> convertToOffice(
    PdfInput input,
    DataSink output, {
    required PdfOfficeFormat format,
  }) {
    _check();
    KeepRecord.op('office');
    return _withInput(
      input,
      output,
      (source) => _bridge.convertToOffice(
        source,
        output,
        format: format,
        password: input.password,
      ),
    );
  }

  /// Converts the Office [document] of the given [format] into a PDF.
  PdfTask<void> convertFromOffice(
    DataSource document,
    DataSink output, {
    required PdfOfficeFormat format,
  }) {
    _check();
    KeepRecord.op('office');
    return _bridge.convertFromOffice(document, output, format: format);
  }

  /// Converts the XFA form in [input] into a plain AcroForm document written
  /// to [output]. A document without XFA throws [PdfEngineFailed].
  PdfTask<void> convertXfaToAcroForm(PdfInput input, DataSink output) {
    _check();
    return _withInput(
      input,
      output,
      (source) => _bridge.convertXfaToAcroForm(
        source,
        output,
        password: input.password,
      ),
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
