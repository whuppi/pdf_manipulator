// Worker isolate entry point for the native bridge.
//
// Runs on a background isolate. Receives operation commands from the
// main isolate (NativeBridge). Handles FFI calls to the Rust bridge.
//
// This file imports dart:ffi — it's only used on native platforms.
//
// INTERNAL — used by NativeBridge only.

import 'dart:async';
import 'dart:ffi' as ffi;
import 'dart:isolate';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:pdf_manipulator/src/transport/native/bindings.dart' as bridge;
import 'package:pdf_manipulator/src/transport/native/shared_buffer.dart';
import 'package:pdf_manipulator/src/transport/native/worker_isolate.dart' as helpers;

/// Entry point for the worker isolate. Passed to Isolate.spawn.
void workerEntryPoint(SendPort mainPort) {
  final workerPort = ReceivePort();
  mainPort.send(workerPort.sendPort);

  SendPort? responsePort;


  helpers.initBridge();

  workerPort.listen((message) async {
    if (message is SendPort) {
      responsePort = message;
      return;
    }

    if (message is! List || message.length != 3) return;
    final id = message[0] as int;
    final op = message[1] as String;
    final args = message[2] as Map<String, Object?>;

    try {
      if (op == 'render' || op == 'extractImages') {
        final streamOpCode = op == 'render' ? 1 : 2;
        await _handleStream(args,
          opCode: streamOpCode,
          responsePort: responsePort!,
          requestId: id,
        );
      } else {
        final result = await _dispatch(op, args);
        responsePort?.send([id, false, result]);
      }
    } catch (e) {
      responsePort?.send([id, true, e.toString()]);
    }
  });
}

Future<Object?> _dispatch(String op, Map<String, Object?> args) async {
  switch (op) {
    case 'open':
      return _handleOpen(args);
    case 'merge':
      return _handleEdit(args, opCode: 1);
    case 'extractPages':
      return _handleEdit(args, opCode: 2);
    case 'deletePages':
      return _handleEdit(args, opCode: 3);
    case 'rotateAllPages':
      return _handleEdit(args, opCode: 6);
    case 'flattenForms':
      return _handleEdit(args, opCode: 7);
    case 'rotatePages':
      return _handleEdit(args, opCode: 5);
    case 'compress':
      return _handleEdit(args, opCode: 9);
    case 'applyRedactions':
      return _handleEdit(args, opCode: 8);
    case 'movePage':
      return _handleEdit(args, opCode: 10);
    case 'extract':
      return _handleRead(args, opCode: args['opCode'] as int? ?? 1);
    case 'search':
      return _handleRead(args, opCode: 3);
    case 'getSignatures':
      return _handleRead(args, opCode: 4);
    case 'verifySignatures':
      return _handleRead(args, opCode: 5);
    case 'validatePdfA':
      return _handleRead(args, opCode: 6);
    case 'validatePdfUa':
      return _handleRead(args, opCode: 7);
    case 'embedFile':
      return _handleEdit(args, opCode: 11);
    case 'eraseRegions':
      return _handleEdit(args, opCode: 12);
    case 'encrypt':
      return _handleEdit(args, opCode: 13);
    case 'decrypt':
      return _handleEdit(args, opCode: 14);
    case 'watermark':
      return _handleEdit(args, opCode: 15);
    case 'sign':
      return _handleEdit(args, opCode: 16);
    case 'signPem':
      return _handleEdit(args, opCode: 29);
    case 'planSplitByBookmarks':
      return _handleRead(args, opCode: 10);
    case 'classifyPage':
      return _handleRead(args, opCode: 8);
    case 'classifyDocument':
      return _handleRead(args, opCode: 9);
    case 'convertTo':
      return _handleRead(args, opCode: 11);
    case 'convertToPdf':
      return _handleRead(args, opCode: 12);
    case 'addStamp':
      return _handleEdit(args, opCode: 17);
    case 'addImageStamp':
      return _handleEdit(args, opCode: 18);
    case 'imagesToPdf':
      return _handleImagesToPdf(args);
    case 'editorOpen':
      return _handleEditorOpen(args);
    case 'editorMutate':
      return _handleEditorMutate(args);
    case 'editorSave':
      return _handleEditorSave(args);
    case 'editorDispose':
      bridge.bridgeEditorDispose(args['handleId'] as int);
      return Uint8List.fromList([1]); // success
    case 'editorGetMetadata':
      return _handleEditorGetMetadata(args);
    case 'editorPageMediaBox':
      return _handleEditorPageMediaBox(args);
    case 'editorExtractPages':
      return _handleEditorExtractPages(args);
    case 'editorMergeFrom':
      return _handleEditorMergeFrom(args);
    case 'builderCreate':
      return _handleBuilderCreate();
    case 'builderSetMetadata':
      await _handleBuilderSetMetadata(args);
      return null;
    case 'builderAddPage':
      return _handleBuilderAddPage(args);
    case 'builderPageOp':
      return _handleBuilderPageOp(args);
    case 'builderPageDone':
      return _handleBuilderPageDone(args);
    case 'builderSave':
      return _handleBuilderSave(args);
    case 'builderDispose':
      await _handleBuilderDispose(args);
      return null;
    default:
      throw UnimplementedError('Worker op: $op');
  }
}

Future<Uint8List> _handleEditorOpen(Map<String, Object?> args) async {
  final sourcePort = args['sourcePort'] as SendPort;
  final sourceLength = args['sourceLength'] as int;
  final password = args['password'] as String?;

  final readBuf = SharedReadBuffer();
  bridge.bridgeInitReadBuffer(readBuf.rawPtr);
  final listener = helpers.createReadListener(readBuf, sourcePort);
  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  ffi.Pointer<ffi.Char> passwordPtr = ffi.nullptr.cast();
  if (password != null && password.isNotEmpty) {
    passwordPtr = password.toNativeUtf8().cast();
  }

  try {
    final handleId = bridge.bridgeEditorOpen(
      readBuf.rawPtr, listener.ptr, sourceLength, passwordPtr, nativePort,
    );
    if (handleId == 0) throw StateError('Failed to open editor');
    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    listener.callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    calloc.free(readBuf.ptr);
    resultPort.close();
    if (passwordPtr.address != 0) calloc.free(passwordPtr);
  }
}

Future<Uint8List> _handleEditorMutate(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final opCode = args['opCode'] as int;
  final params = args['params'] as Uint8List?;
  final secondaries = args['secondaries'] as List<Uint8List>?;

  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  ffi.Pointer<ffi.Uint8> paramsPtr = ffi.nullptr.cast();
  if (params != null && params.isNotEmpty) {
    paramsPtr = calloc<ffi.Uint8>(params.length);
    paramsPtr.asTypedList(params.length).setAll(0, params);
  }

  final secCount = secondaries?.length ?? 0;
  final secPtrs = calloc<ffi.Pointer<ffi.Uint8>>(secCount == 0 ? 1 : secCount);
  final secLens = calloc<ffi.Int32>(secCount == 0 ? 1 : secCount);
  for (var i = 0; i < secCount; i++) {
    final sec = secondaries![i];
    final ptr = calloc<ffi.Uint8>(sec.length);
    ptr.asTypedList(sec.length).setAll(0, sec);
    secPtrs[i] = ptr;
    secLens[i] = sec.length;
  }

  try {
    bridge.bridgeEditorMutate(
      handleId, opCode, paramsPtr, params?.length ?? 0,
      secPtrs, secLens, secCount, nativePort,
    );
    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    if (paramsPtr.address != 0) calloc.free(paramsPtr);
    for (var i = 0; i < secCount; i++) {
      calloc.free(secPtrs[i]);
    }
    calloc.free(secPtrs);
    calloc.free(secLens);
    resultPort.close();
  }
}

Future<Uint8List> _handleEditorSave(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final sinkPort = args['sinkPort'] as SendPort;
  final compress = args['compress'] as bool? ?? true;
  final garbageCollect = args['garbageCollect'] as bool? ?? true;
  final linearize = args['linearize'] as bool? ?? false;
  final encryptAlgo = args['encryptAlgo'] as int? ?? 0;
  final encryptUserPw = args['encryptUserPw'] as String? ?? '';
  final encryptOwnerPw = args['encryptOwnerPw'] as String? ?? '';
  final encryptPermissions = args['encryptPermissions'] as int? ?? -1;

  final writeBuf = SharedWriteBuffer();
  bridge.bridgeInitWriteBuffer(writeBuf.rawPtr);
  final writeListener = helpers.createWriteListener(writeBuf, sinkPort);
  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  final userPwBytes = encryptUserPw.codeUnits;
  final ownerPwBytes = encryptOwnerPw.codeUnits;
  final userPwPtr = calloc<ffi.Uint8>(userPwBytes.isEmpty ? 1 : userPwBytes.length);
  final ownerPwPtr = calloc<ffi.Uint8>(ownerPwBytes.isEmpty ? 1 : ownerPwBytes.length);
  if (userPwBytes.isNotEmpty) userPwPtr.asTypedList(userPwBytes.length).setAll(0, userPwBytes);
  if (ownerPwBytes.isNotEmpty) ownerPwPtr.asTypedList(ownerPwBytes.length).setAll(0, ownerPwBytes);

  try {
    bridge.bridgeEditorSave(
      handleId, writeBuf.rawPtr, writeListener.ptr,
      compress, garbageCollect, linearize,
      encryptAlgo,
      userPwPtr, userPwBytes.length,
      ownerPwPtr, ownerPwBytes.length,
      encryptPermissions,
      nativePort,
    );
    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    writeListener.callable.close();
    bridge.bridgeDestroyWriteBuffer(writeBuf.rawPtr);
    calloc.free(writeBuf.ptr);
    calloc.free(userPwPtr);
    calloc.free(ownerPwPtr);
    resultPort.close();
  }
}

Future<Uint8List> _handleEditorGetMetadata(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final resultPort = ReceivePort();
  bridge.bridgeEditorGetMetadata(handleId, resultPort.sendPort.nativePort);
  final result = await resultPort.first;
  return result is Uint8List ? result : Uint8List.fromList(result as List<int>);
}

Future<Uint8List> _handleEditorPageMediaBox(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final page = args['page'] as int;
  final out = calloc<ffi.Double>(4);
  try {
    final rc = bridge.bridgeEditorGetPageMediaBox(handleId, page, out);
    final result = Uint8List(1 + 4 * 8); // status + 4 doubles
    result[0] = rc == 0 ? 1 : 0;
    final bd = ByteData.sublistView(result);
    bd.setFloat64(1, out[0], Endian.little);
    bd.setFloat64(9, out[1], Endian.little);
    bd.setFloat64(17, out[2], Endian.little);
    bd.setFloat64(25, out[3], Endian.little);
    return result;
  } finally {
    calloc.free(out);
  }
}

Future<Uint8List> _handleEditorExtractPages(Map<String, Object?> args) async {
  // extractPages via editor = opCode 2 in bridgeEditorMutate
  final handleId = args['handleId'] as int;
  final pages = args['pages'] as List;
  final params = Uint8List(4 + pages.length * 4);
  final bd = ByteData.sublistView(params);
  bd.setInt32(0, pages.length, Endian.little);
  for (var i = 0; i < pages.length; i++) {
    bd.setInt32(4 + i * 4, pages[i] as int, Endian.little);
  }
  return _handleEditorMutate({
    'handleId': handleId,
    'opCode': 2,
    'params': params,
  });
}

Future<Uint8List> _handleEditorMergeFrom(Map<String, Object?> args) async {
  // mergeFrom = opCode 1 in bridgeEditorMutate with secondary bytes
  final handleId = args['handleId'] as int;
  final otherBytes = args['otherBytes'] as Uint8List;
  return _handleEditorMutate({
    'handleId': handleId,
    'opCode': 1,
    'secondaries': [otherBytes],
  });
}

Future<Uint8List> _handleOpen(Map<String, Object?> args) async {
  final sourcePort = args['sourcePort'] as SendPort;
  final sourceLength = args['sourceLength'] as int;
  final password = args['password'] as String?;

  // Allocate shared read buffer
  final readBuf = SharedReadBuffer();
  bridge.bridgeInitReadBuffer(readBuf.rawPtr);


  final listener = helpers.createReadListener(readBuf, sourcePort);


  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  // Password as native string
  ffi.Pointer<ffi.Char> passwordPtr = ffi.nullptr.cast();
  if (password != null && password.isNotEmpty) {
    passwordPtr = password.toNativeUtf8().cast();
  }

  try {
    final opId = bridge.bridgeSubmitOpen(
      readBuf.rawPtr,
      listener.ptr,
      sourceLength,
      passwordPtr,
      nativePort,
    );

    if (opId == 0) {
      throw StateError('Failed to submit open operation');
    }

    // Wait for the result from the Rust pool thread via allo-isolate
    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    listener.callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    calloc.free(readBuf.ptr);
    resultPort.close();
    if (passwordPtr.address != 0) {
      calloc.free(passwordPtr);
    }
  }
}

/// Read-only operation handler. Opens source, runs a read op, posts result.
/// No CallbackWriter — no output PDF. Result comes back as bytes via allo-isolate.
Future<Uint8List> _handleRead(Map<String, Object?> args, {required int opCode}) async {
  final sourcePort = args['sourcePort'] as SendPort;
  final sourceLength = args['sourceLength'] as int;
  final password = args['password'] as String?;
  final opParams = args['params'] as Uint8List?;

  final readBuf = SharedReadBuffer();
  bridge.bridgeInitReadBuffer(readBuf.rawPtr);

  final listener = helpers.createReadListener(readBuf, sourcePort);
  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  ffi.Pointer<ffi.Char> passwordPtr = ffi.nullptr.cast();
  if (password != null && password.isNotEmpty) {
    passwordPtr = password.toNativeUtf8().cast();
  }

  ffi.Pointer<ffi.Uint8> paramsPtr = ffi.nullptr.cast();
  final paramsLen = opParams?.length ?? 0;
  if (opParams != null && opParams.isNotEmpty) {
    paramsPtr = calloc<ffi.Uint8>(paramsLen);
    paramsPtr.asTypedList(paramsLen).setAll(0, opParams);
  }

  try {
    final opId = bridge.bridgeSubmitRead(
      readBuf.rawPtr,
      listener.ptr,
      sourceLength,
      passwordPtr,
      opCode,
      paramsPtr,
      paramsLen,
      nativePort,
    );

    if (opId == 0) {
      throw StateError('Failed to submit read operation');
    }

    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    listener.callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    calloc.free(readBuf.ptr);
    resultPort.close();
    if (passwordPtr.address != 0) calloc.free(passwordPtr);
    if (paramsPtr.address != 0) calloc.free(paramsPtr);
  }
}

/// Generic edit operation handler. All read+write ops go through here.
/// `opCode` maps to the Rust `dispatch_edit_op` codes.
/// `args['params']` is an optional Uint8List of operation-specific params.
/// `args['secondaries']` is an optional list of secondary byte arrays.
Future<Uint8List> _handleEdit(Map<String, Object?> args, {required int opCode}) async {
  final sourcePort = args['sourcePort'] as SendPort;
  final sourceLength = args['sourceLength'] as int;
  final sinkPort = args['sinkPort'] as SendPort;
  final secondaryBytesList = (args['secondaries'] as List<Uint8List>?) ?? [];
  final opParams = args['params'] as Uint8List?;

  // Shared buffers
  final readBuf = SharedReadBuffer();
  bridge.bridgeInitReadBuffer(readBuf.rawPtr);
  final writeBuf = SharedWriteBuffer();
  bridge.bridgeInitWriteBuffer(writeBuf.rawPtr);

  // Listeners
  final readListener = helpers.createReadListener(readBuf, sourcePort);
  final writeListener = helpers.createWriteListener(writeBuf, sinkPort);

  // Result port
  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  // Op params as native pointer
  ffi.Pointer<ffi.Uint8> paramsPtr = ffi.nullptr.cast();
  final paramsLen = opParams?.length ?? 0;
  if (opParams != null && opParams.isNotEmpty) {
    paramsPtr = calloc<ffi.Uint8>(paramsLen);
    paramsPtr.asTypedList(paramsLen).setAll(0, opParams);
  }

  // Secondary byte arrays as native pointers
  final count = secondaryBytesList.length;
  final ptrs = calloc<ffi.Pointer<ffi.Uint8>>(count == 0 ? 1 : count);
  final lens = calloc<ffi.Size>(count == 0 ? 1 : count);
  final nativePtrs = <ffi.Pointer<ffi.Uint8>>[];

  for (var i = 0; i < count; i++) {
    final bytes = secondaryBytesList[i];
    final ptr = calloc<ffi.Uint8>(bytes.length);
    ptr.asTypedList(bytes.length).setAll(0, bytes);
    ptrs[i] = ptr;
    lens[i] = bytes.length;
    nativePtrs.add(ptr);
  }

  try {
    final opId = bridge.bridgeSubmitEdit(
      readBuf.rawPtr,
      readListener.ptr,
      sourceLength,
      writeBuf.rawPtr,
      writeListener.ptr,
      opCode,
      paramsPtr,
      paramsLen,
      ptrs,
      lens,
      count,
      nativePort,
    );

    if (opId == 0) {
      throw StateError('Failed to submit edit operation (code=$opCode)');
    }

    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    readListener.callable.close();
    writeListener.callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    bridge.bridgeDestroyWriteBuffer(writeBuf.rawPtr);
    calloc.free(readBuf.ptr);
    calloc.free(writeBuf.ptr);
    if (paramsPtr.address != 0) calloc.free(paramsPtr);
    for (final p in nativePtrs) {
      calloc.free(p);
    }
    calloc.free(ptrs);
    calloc.free(lens);
    resultPort.close();
  }
}

/// Images-to-PDF handler. No source PDF — takes image bytes, produces PDF.
Future<Uint8List> _handleImagesToPdf(Map<String, Object?> args) async {
  final images = args['images'] as List<Uint8List>;
  final sinkPort = args['sinkPort'] as SendPort;

  final writeBuf = SharedWriteBuffer();
  bridge.bridgeInitWriteBuffer(writeBuf.rawPtr);

  final writeListener = helpers.createWriteListener(writeBuf, sinkPort);

  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  final count = images.length;
  final ptrs = calloc<ffi.Pointer<ffi.Uint8>>(count == 0 ? 1 : count);
  final lens = calloc<ffi.Size>(count == 0 ? 1 : count);
  final nativePtrs = <ffi.Pointer<ffi.Uint8>>[];

  for (var i = 0; i < count; i++) {
    final bytes = images[i];
    final ptr = calloc<ffi.Uint8>(bytes.length);
    ptr.asTypedList(bytes.length).setAll(0, bytes);
    ptrs[i] = ptr;
    lens[i] = bytes.length;
    nativePtrs.add(ptr);
  }

  try {
    final opId = bridge.bridgeSubmitImagesToPdf(
      writeBuf.rawPtr,
      writeListener.ptr,
      ptrs,
      lens,
      count,
      nativePort,
    );

    if (opId == 0) {
      throw StateError('Failed to submit imagesToPdf operation');
    }

    final result = await resultPort.first;
    return result as Uint8List;
  } finally {
    writeListener.callable.close();
    bridge.bridgeDestroyWriteBuffer(writeBuf.rawPtr);
    calloc.free(writeBuf.ptr);
    for (final p in nativePtrs) {
      calloc.free(p);
    }
    calloc.free(ptrs);
    calloc.free(lens);
    resultPort.close();
  }
}

/// Streaming operation handler. Posts MULTIPLE items to responsePort.
/// Each item is [requestId, 'item', Uint8List].
/// Done is [requestId, 'done', null].
/// Error is [requestId, 'error', String].
Future<void> _handleStream(
  Map<String, Object?> args, {
  required int opCode,
  required SendPort responsePort,
  required int requestId,
}) async {
  final sourcePort = args['sourcePort'] as SendPort;
  final sourceLength = args['sourceLength'] as int;
  final password = args['password'] as String?;
  final opParams = args['params'] as Uint8List?;

  final readBuf = SharedReadBuffer();
  bridge.bridgeInitReadBuffer(readBuf.rawPtr);

  final listener = helpers.createReadListener(readBuf, sourcePort);
  final resultPort = ReceivePort();
  final nativePort = resultPort.sendPort.nativePort;

  ffi.Pointer<ffi.Char> passwordPtr = ffi.nullptr.cast();
  if (password != null && password.isNotEmpty) {
    passwordPtr = password.toNativeUtf8().cast();
  }

  ffi.Pointer<ffi.Uint8> paramsPtr = ffi.nullptr.cast();
  final paramsLen = opParams?.length ?? 0;
  if (opParams != null && opParams.isNotEmpty) {
    paramsPtr = calloc<ffi.Uint8>(paramsLen);
    paramsPtr.asTypedList(paramsLen).setAll(0, opParams);
  }

  try {
    final opId = bridge.bridgeSubmitStream(
      readBuf.rawPtr,
      listener.ptr,
      sourceLength,
      passwordPtr,
      opCode,
      paramsPtr,
      paramsLen,
      nativePort,
    );

    if (opId == 0) {
      responsePort.send([requestId, 'error', 'Failed to submit stream operation']);
      return;
    }

    // Listen for multiple items from the Rust pool thread
    await for (final data in resultPort) {
      final bytes = data as Uint8List;
      if (bytes.isEmpty) continue;
      final type = bytes[0];
      switch (type) {
        case 1: // item
          responsePort.send([requestId, 'item', bytes]);
        case 2: // done
          responsePort.send([requestId, 'done', null]);
          return;
        case 0: // error
          responsePort.send([requestId, 'error', 'Stream operation failed']);
          return;
        default:
          continue;
      }
    }
  } finally {
    listener.callable.close();
    bridge.bridgeDestroyReadBuffer(readBuf.rawPtr);
    calloc.free(readBuf.ptr);
    resultPort.close();
    if (passwordPtr.address != 0) calloc.free(passwordPtr);
    if (paramsPtr.address != 0) calloc.free(paramsPtr);
  }
}

// ── Builder handlers ──────────────────────────────────────────────────

Future<int> _handleBuilderCreate() async {
  return bridge.bridgeBuilderCreate();
}

Future<void> _handleBuilderSetMetadata(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final op = args['op'] as int;
  final value = args['value'] as String;
  final ptr = value.toNativeUtf8(allocator: calloc);
  try {
    bridge.bridgeBuilderSetMetadata(handleId, op, ptr.cast());
  } finally {
    calloc.free(ptr);
  }
}

Future<int> _handleBuilderAddPage(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final pageType = args['pageType'] as int;
  final width = (args['width'] as num?)?.toDouble() ?? 0.0;
  final height = (args['height'] as num?)?.toDouble() ?? 0.0;
  return bridge.bridgeBuilderAddPage(handleId, pageType, width, height);
}

Future<int> _handleBuilderPageOp(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final opCode = args['opCode'] as int;
  final params = args['params'] as Uint8List?;
  final secondary = args['secondary'] as Uint8List?;

  final paramsPtr = params != null ? _allocBytes(params) : ffi.nullptr.cast<ffi.Uint8>();
  final secPtr = secondary != null ? _allocBytes(secondary) : ffi.nullptr.cast<ffi.Uint8>();
  try {
    return bridge.bridgeBuilderPageOp(
      handleId, opCode,
      paramsPtr, params?.length ?? 0,
      secPtr, secondary?.length ?? 0,
    );
  } finally {
    if (params != null) calloc.free(paramsPtr);
    if (secondary != null) calloc.free(secPtr);
  }
}

Future<int> _handleBuilderPageDone(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  return bridge.bridgeBuilderPageOp(
    handleId, 17,
    ffi.nullptr.cast(), 0,
    ffi.nullptr.cast(), 0,
  );
}

ffi.Pointer<ffi.Uint8> _allocBytes(Uint8List bytes) {
  final ptr = calloc<ffi.Uint8>(bytes.length);
  ptr.asTypedList(bytes.length).setAll(0, bytes);
  return ptr;
}

Future<Uint8List> _handleBuilderSave(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  final sinkPort = args['sinkPort'] as SendPort;

  final writeBuf = SharedWriteBuffer();
  bridge.bridgeInitWriteBuffer(writeBuf.rawPtr);
  final writeListener = helpers.createWriteListener(writeBuf, sinkPort);
  final resultPort = ReceivePort();

  bridge.bridgeBuilderSave(
    handleId,
    writeBuf.rawPtr,
    writeListener.callable.nativeFunction,
    ffi.nullptr.cast(), 0,
    resultPort.sendPort.nativePort,
  );

  try {
    final result = await resultPort.first;
    if (result is Uint8List) return result;
    return Uint8List.fromList([0]);
  } finally {
    writeListener.callable.close();
    bridge.bridgeDestroyWriteBuffer(writeBuf.rawPtr);
    calloc.free(writeBuf.ptr);
    resultPort.close();
  }
}

Future<void> _handleBuilderDispose(Map<String, Object?> args) async {
  final handleId = args['handleId'] as int;
  bridge.bridgeBuilderDispose(handleId);
}
