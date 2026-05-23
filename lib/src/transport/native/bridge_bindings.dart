// FFI bindings for the Rust bridge C API.
//
// These @Native declarations bind to the functions in
// vendor/pdf_oxide/src/bridge/ffi_api.rs and allo-isolate.
//
// All Dart names are camelCase. The `symbol:` parameter maps to the
// actual C symbol name (snake_case). No `// ignore:` comments needed.
//
// INTERNAL — used by the native bridge only.

@ffi.DefaultAsset('package:pdf_manipulator/src/ffi/native_bindings.g.dart')
library;

import 'dart:ffi' as ffi;

// ── Init / Shutdown ─────────────────────────────────────────────────

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>>)>(
    symbol: 'store_dart_post_cobject')
external void storeDartPostCobject(
  ffi.Pointer<ffi.NativeFunction<ffi.Bool Function(ffi.Int64, ffi.Pointer<ffi.Void>)>> postCObject,
);

@ffi.Native<ffi.Void Function()>(symbol: 'bridge_init')
external void bridgeInit();

@ffi.Native<ffi.Void Function()>(symbol: 'bridge_shutdown')
external void bridgeShutdown();

// ── Operation management ────────────────────────────────────────────

@ffi.Native<ffi.Void Function(ffi.Uint64)>(symbol: 'bridge_cancel')
external void bridgeCancel(int opId);

@ffi.Native<ffi.Void Function()>(symbol: 'bridge_cancel_all')
external void bridgeCancelAll();

@ffi.Native<ffi.Int32 Function()>(symbol: 'bridge_pool_size')
external int bridgePoolSize();

// ── Shared buffer management ────────────────────────────────────────

@ffi.Native<ffi.Int32 Function()>(symbol: 'bridge_read_buffer_size')
external int bridgeReadBufferSize();

@ffi.Native<ffi.Int32 Function()>(symbol: 'bridge_write_buffer_size')
external int bridgeWriteBufferSize();

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_init_read_buffer')
external void bridgeInitReadBuffer(ffi.Pointer<ffi.Uint8> buf);

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_init_write_buffer')
external void bridgeInitWriteBuffer(ffi.Pointer<ffi.Uint8> buf);

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_destroy_read_buffer')
external void bridgeDestroyReadBuffer(ffi.Pointer<ffi.Uint8> buf);

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_destroy_write_buffer')
external void bridgeDestroyWriteBuffer(ffi.Pointer<ffi.Uint8> buf);

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_signal_read')
external void bridgeSignalRead(ffi.Pointer<ffi.Uint8> buf);

@ffi.Native<ffi.Void Function(ffi.Pointer<ffi.Uint8>)>(symbol: 'bridge_signal_write')
external void bridgeSignalWrite(ffi.Pointer<ffi.Uint8> buf);

// ── Submit operations ───────────────────────────────────────────────

@ffi.Native<
    ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int64,
        ffi.Pointer<ffi.Char>,
        ffi.Int64)>(symbol: 'bridge_submit_open')
external int bridgeSubmitOpen(
  ffi.Pointer<ffi.Uint8> readBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> readNotifyFn,
  int sourceLength,
  ffi.Pointer<ffi.Char> password,
  int resultPort,
);

@ffi.Native<
    ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int64,
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>,
        ffi.Int32,
        ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
        ffi.Pointer<ffi.Size>,
        ffi.Size,
        ffi.Int64)>(symbol: 'bridge_submit_edit')
external int bridgeSubmitEdit(
  ffi.Pointer<ffi.Uint8> readBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> readNotifyFn,
  int sourceLength,
  ffi.Pointer<ffi.Uint8> writeBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> writeNotifyFn,
  int opCode,
  ffi.Pointer<ffi.Uint8> opParams,
  int opParamsLen,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> secondaryPtrs,
  ffi.Pointer<ffi.Size> secondaryLens,
  int secondaryCount,
  int resultPort,
);

@ffi.Native<
    ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int64,
        ffi.Pointer<ffi.Char>,
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>,
        ffi.Int32,
        ffi.Int64)>(symbol: 'bridge_submit_read')
external int bridgeSubmitRead(
  ffi.Pointer<ffi.Uint8> readBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> readNotifyFn,
  int sourceLength,
  ffi.Pointer<ffi.Char> password,
  int opCode,
  ffi.Pointer<ffi.Uint8> opParams,
  int opParamsLen,
  int resultPort,
);

/// Submit a streaming read operation. Posts MULTIPLE results to resultPort:
/// one per item (type=1), then a done marker (type=2), or error (type=0).
@ffi.Native<ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int64,
        ffi.Pointer<ffi.Char>,
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>,
        ffi.Int32,
        ffi.Int64)>(symbol: 'bridge_submit_stream')
external int bridgeSubmitStream(
  ffi.Pointer<ffi.Uint8> readBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> readNotifyFn,
  int sourceLength,
  ffi.Pointer<ffi.Char> password,
  int opCode,
  ffi.Pointer<ffi.Uint8> opParams,
  int opParamsLen,
  int resultPort,
);

// ── Images to PDF ──────────────────────────────────────────────────────

@ffi.Native<ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
        ffi.Pointer<ffi.Size>,
        ffi.Size,
        ffi.Int64)>(symbol: 'bridge_submit_images_to_pdf')
external int bridgeSubmitImagesToPdf(
  ffi.Pointer<ffi.Uint8> writeBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> writeNotifyFn,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> imagePtrs,
  ffi.Pointer<ffi.Size> imageLens,
  int imageCount,
  int resultPort,
);

// ── Persistent editor handles ─────────────────────────────────────────

@ffi.Native<ffi.Uint64 Function(
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Int64,
        ffi.Pointer<ffi.Char>,
        ffi.Int64)>(symbol: 'bridge_editor_open')
external int bridgeEditorOpen(
  ffi.Pointer<ffi.Uint8> readBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> readNotifyFn,
  int sourceLength,
  ffi.Pointer<ffi.Char> password,
  int resultPort,
);

@ffi.Native<ffi.Void Function(
        ffi.Uint64,
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>,
        ffi.Int32,
        ffi.Pointer<ffi.Pointer<ffi.Uint8>>,
        ffi.Pointer<ffi.Int32>,
        ffi.Int32,
        ffi.Int64)>(symbol: 'bridge_editor_mutate')
external void bridgeEditorMutate(
  int handleId,
  int opCode,
  ffi.Pointer<ffi.Uint8> params,
  int paramsLen,
  ffi.Pointer<ffi.Pointer<ffi.Uint8>> secondaries,
  ffi.Pointer<ffi.Int32> secondaryLens,
  int secondaryCount,
  int resultPort,
);

@ffi.Native<ffi.Void Function(
        ffi.Uint64,
        ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Bool, ffi.Bool, ffi.Bool,
        ffi.Int32,
        ffi.Pointer<ffi.Uint8>, ffi.Int32,
        ffi.Pointer<ffi.Uint8>, ffi.Int32,
        ffi.Int32,
        ffi.Int64)>(symbol: 'bridge_editor_save')
external void bridgeEditorSave(
  int handleId,
  ffi.Pointer<ffi.Uint8> writeBuf,
  ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> writeNotifyFn,
  bool compress, bool garbageCollect, bool linearize,
  int encryptAlgo,
  ffi.Pointer<ffi.Uint8> encryptUserPw, int encryptUserPwLen,
  ffi.Pointer<ffi.Uint8> encryptOwnerPw, int encryptOwnerPwLen,
  int encryptPermissions,
  int resultPort,
);

@ffi.Native<ffi.Void Function(ffi.Uint64)>(symbol: 'bridge_editor_dispose')
external void bridgeEditorDispose(int handleId);

@ffi.Native<ffi.Int32 Function(ffi.Uint64)>(symbol: 'bridge_editor_page_count')
external int bridgeEditorPageCount(int handleId);

@ffi.Native<ffi.Int32 Function(ffi.Uint64, ffi.Int32, ffi.Pointer<ffi.Double>)>(
    symbol: 'bridge_editor_get_page_media_box')
external int bridgeEditorGetPageMediaBox(int handleId, int page, ffi.Pointer<ffi.Double> out);

@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Int64)>(symbol: 'bridge_editor_get_metadata')
external void bridgeEditorGetMetadata(int handleId, int resultPort);

// ── Builder handle ops ──────────────────────────────────────────────

@ffi.Native<ffi.Uint64 Function()>(symbol: 'bridge_builder_create')
external int bridgeBuilderCreate();

@ffi.Native<ffi.Void Function(ffi.Uint64, ffi.Int32, ffi.Pointer<ffi.Char>)>(
    symbol: 'bridge_builder_set_metadata')
external void bridgeBuilderSetMetadata(
    int handleId, int op, ffi.Pointer<ffi.Char> value);

@ffi.Native<ffi.Int32 Function(ffi.Uint64, ffi.Int32, ffi.Double, ffi.Double)>(
    symbol: 'bridge_builder_add_page')
external int bridgeBuilderAddPage(
    int handleId, int pageType, double width, double height);

@ffi.Native<
    ffi.Int32 Function(ffi.Uint64, ffi.Int32, ffi.Pointer<ffi.Uint8>,
        ffi.Int32, ffi.Pointer<ffi.Uint8>, ffi.Int32)>(
    symbol: 'bridge_builder_page_op')
external int bridgeBuilderPageOp(
    int handleId, int opCode,
    ffi.Pointer<ffi.Uint8> params, int paramsLen,
    ffi.Pointer<ffi.Uint8> secondary, int secondaryLen);

@ffi.Native<
    ffi.Void Function(ffi.Uint64, ffi.Pointer<ffi.Uint8>,
        ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>>,
        ffi.Pointer<ffi.Uint8>, ffi.Int32, ffi.Int64)>(
    symbol: 'bridge_builder_save')
external void bridgeBuilderSave(
    int handleId,
    ffi.Pointer<ffi.Uint8> writeBuf,
    ffi.Pointer<ffi.NativeFunction<ffi.Void Function()>> writeNotify,
    ffi.Pointer<ffi.Uint8> saveOptions, int saveOptionsLen,
    int resultPort);

@ffi.Native<ffi.Void Function(ffi.Uint64)>(symbol: 'bridge_builder_dispose')
external void bridgeBuilderDispose(int handleId);
