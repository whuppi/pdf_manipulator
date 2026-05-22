// Dart-friendly wrappers over generated @Native FFI bindings.
//
// Every method: allocate → call → check error → copy result → free native.
// INTERNAL — only platform/_native.dart imports this.

import 'dart:ffi' as ffi;
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'package:pdf_manipulator/src/ffi/native_bindings.g.dart' as native;
import 'package:pdf_manipulator/src/core/errors.dart';
import 'package:pdf_manipulator/src/core/pdf_image.dart';
import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/core/pdf_signature.dart';
import 'package:pdf_manipulator/src/core/search_result.dart';

/// Memory-safe Dart wrappers over the generated pdf_oxide FFI bindings.
///
/// The pdf_oxide C API uses two handle types:
/// - `pdf_document_*` — read-only (opens from file path only)
/// - `document_editor_*` — read+write (opens from bytes or path)
///
/// Since our public API is bytes-only (no dart:io, no paths), we use
/// the editor handle for everything. It supports all read operations
/// plus mutations.
class PdfBindings {
  const PdfBindings();

  // ── Error mapping ─────────────────────────────────────────────────

  void _check(ffi.Pointer<ffi.Int> code) {
    final c = code.value;
    if (c == 0) return;
    throw switch (c) {
      1 => const PdfInvalidArgument('Invalid argument'),
      2 => const PdfIoError('I/O error'),
      3 => const PdfCorrupted('Failed to parse PDF'),
      4 => const PdfExtractionFailed('Extraction failed'),
      5 => const PdfEngineError('Internal engine error'),
      6 => const PdfPageRangeError(page: -1, pageCount: -1),
      7 => const PdfSearchError('Search error'),
      8 => const PdfUnsupported('Unsupported feature'),
      _ => PdfEngineError('Unknown error code: $c'),
    };
  }

  // ── String helpers ────────────────────────────────────────────────

  String _readFreeString(ffi.Pointer<ffi.Char> ptr) {
    if (ptr == ffi.nullptr) return '';
    final s = ptr.cast<Utf8>().toDartString();
    native.free_string(ptr);
    return s;
  }

  // ── Byte helpers ──────────────────────────────────────────────────

  ffi.Pointer<ffi.Uint8> _allocBytes(Uint8List bytes) {
    final ptr = calloc<ffi.Uint8>(bytes.length);
    ptr.asTypedList(bytes.length).setAll(0, bytes);
    return ptr;
  }

  Uint8List _readFreeBytes(ffi.Pointer<ffi.Uint8> ptr, int len) {
    final result = Uint8List(len);
    result.setAll(0, ptr.asTypedList(len));
    native.free_bytes(ptr.cast());
    return result;
  }

  // ── Editor: open / close ──────────────────────────────────────────

  ffi.Pointer<ffi.Void> editorOpen(Uint8List bytes) {
    final dataPtr = _allocBytes(bytes);
    final err = calloc<ffi.Int>();
    try {
      final handle = native.document_editor_open_from_bytes(
        dataPtr,
        bytes.length,
        err,
      );
      _check(err);
      return handle;
    } finally {
      calloc.free(dataPtr);
      calloc.free(err);
    }
  }

  void editorFree(ffi.Pointer<ffi.Void> h) {
    native.document_editor_free(h);
  }

  // ── Editor: inspect ───────────────────────────────────────────────

  int editorPageCount(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final n = native.document_editor_get_page_count(h, err);
      _check(err);
      return n;
    } finally {
      calloc.free(err);
    }
  }

  String editorVersion(ffi.Pointer<ffi.Void> h) {
    final major = calloc<ffi.Uint8>();
    final minor = calloc<ffi.Uint8>();
    try {
      native.document_editor_get_version(h, major, minor);
      return '${major.value}.${minor.value}';
    } finally {
      calloc.free(major);
      calloc.free(minor);
    }
  }

  bool editorIsModified(ffi.Pointer<ffi.Void> h) =>
      native.document_editor_is_modified(h);

  // ── Editor: metadata ──────────────────────────────────────────────

  String editorGetTitle(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(native.document_editor_get_title(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetTitle(ffi.Pointer<ffi.Void> h, String v) {
    final err = calloc<ffi.Int>();
    final ptr = v.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_title(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  String editorGetAuthor(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(native.document_editor_get_author(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetAuthor(ffi.Pointer<ffi.Void> h, String v) {
    final err = calloc<ffi.Int>();
    final ptr = v.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_author(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  String editorGetSubject(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(native.document_editor_get_subject(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetSubject(ffi.Pointer<ffi.Void> h, String v) {
    final err = calloc<ffi.Int>();
    final ptr = v.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_subject(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  String editorGetKeywords(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(native.document_editor_get_keywords(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetKeywords(ffi.Pointer<ffi.Void> h, String v) {
    final err = calloc<ffi.Int>();
    final ptr = v.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_keywords(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  // ── Editor: page manipulation ─────────────────────────────────────

  void editorRotatePage(ffi.Pointer<ffi.Void> h, int page, int degrees) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_rotate_page_by(h, page, degrees, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  void editorRotateAllPages(ffi.Pointer<ffi.Void> h, int degrees) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_rotate_all_pages(h, degrees, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  PdfRect editorGetPageMediaBox(ffi.Pointer<ffi.Void> h, int page) {
    final x = calloc<ffi.Double>();
    final y = calloc<ffi.Double>();
    final w = calloc<ffi.Double>();
    final ht = calloc<ffi.Double>();
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_get_page_media_box(h, page, x, y, w, ht, err);
      _check(err);
      return PdfRect(x: x.value, y: y.value, width: w.value, height: ht.value);
    } finally {
      calloc.free(x);
      calloc.free(y);
      calloc.free(w);
      calloc.free(ht);
      calloc.free(err);
    }
  }

  // ── Editor: page rotation getter ───────────────────────────────────

  int editorGetPageRotation(ffi.Pointer<ffi.Void> h, int page) {
    final err = calloc<ffi.Int>();
    try {
      final rot = native.document_editor_get_page_rotation(h, page, err);
      _check(err);
      return rot;
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: delete / move / extract pages ──────────────────────────

  void editorDeletePage(ffi.Pointer<ffi.Void> h, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_delete_page(h, pageIndex, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  void editorMovePage(ffi.Pointer<ffi.Void> h, int from, int to) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_move_page(h, from, to, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  Uint8List editorExtractPages(ffi.Pointer<ffi.Void> h, List<int> pages) {
    final pagesPtr = calloc<ffi.Int32>(pages.length);
    for (var i = 0; i < pages.length; i++) {
      pagesPtr[i] = pages[i];
    }
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.document_editor_extract_pages_to_bytes(
        h, pagesPtr, pages.length, outLen, err);
      _check(err);
      return _readFreeBytes(ptr, outLen.value);
    } finally {
      calloc.free(pagesPtr);
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  // ── Editor: merge ─────────────────────────────────────────────────

  void editorMerge(ffi.Pointer<ffi.Void> h, Uint8List otherPdf) {
    final dataPtr = _allocBytes(otherPdf);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_merge_from_bytes(
        h,
        dataPtr.cast(),
        otherPdf.length,
        err,
      );
      _check(err);
    } finally {
      calloc.free(dataPtr);
      calloc.free(err);
    }
  }

  // ── Editor: forms ─────────────────────────────────────────────────

  void editorFlattenForms(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_flatten_forms(h, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: redaction ─────────────────────────────────────────────

  void editorApplyAllRedactions(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_apply_all_redactions(h, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: save ──────────────────────────────────────────────────

  Uint8List editorSave(ffi.Pointer<ffi.Void> h) {
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.document_editor_save_to_bytes(h, outLen, err);
      _check(err);
      return _readFreeBytes(ptr.cast(), outLen.value);
    } finally {
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  Uint8List editorSaveWithOptions(
    ffi.Pointer<ffi.Void> h, {
    bool compress = true,
    bool garbageCollect = true,
    bool linearize = false,
  }) {
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.document_editor_save_to_bytes_with_options(
        h,
        compress,
        garbageCollect,
        linearize,
        outLen,
        err,
      );
      _check(err);
      return _readFreeBytes(ptr.cast(), outLen.value);
    } finally {
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  // ── Editor: page boxes ─────────────────────────────────────────────

  PdfRect editorGetPageCropBox(ffi.Pointer<ffi.Void> h, int page) {
    final x = calloc<ffi.Double>();
    final y = calloc<ffi.Double>();
    final w = calloc<ffi.Double>();
    final ht = calloc<ffi.Double>();
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_get_page_crop_box(h, page, x, y, w, ht, err);
      _check(err);
      return PdfRect(x: x.value, y: y.value, width: w.value, height: ht.value);
    } finally {
      calloc.free(x);
      calloc.free(y);
      calloc.free(w);
      calloc.free(ht);
      calloc.free(err);
    }
  }

  void editorSetPageMediaBox(
      ffi.Pointer<ffi.Void> h, int page, PdfRect box) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_set_page_media_box(
          h, page, box.x, box.y, box.width, box.height, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  void editorSetPageCropBox(
      ffi.Pointer<ffi.Void> h, int page, PdfRect box) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_set_page_crop_box(
          h, page, box.x, box.y, box.width, box.height, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: embed file ────────────────────────────────────────────

  void editorEmbedFile(
    ffi.Pointer<ffi.Void> h,
    String name,
    Uint8List data,
  ) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final dataPtr = _allocBytes(data);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_embed_file(
        h,
        namePtr.cast(),
        dataPtr,
        data.length,
        err,
      );
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(dataPtr);
      calloc.free(err);
    }
  }

  // ── Editor: erase regions ─────────────────────────────────────────

  void editorEraseRegions(
    ffi.Pointer<ffi.Void> h,
    int page,
    List<PdfRect> rects,
  ) {
    // Flatten rects into [x0, y0, w0, h0, x1, y1, w1, h1, ...]
    final flat = calloc<ffi.Double>(rects.length * 4);
    for (var i = 0; i < rects.length; i++) {
      flat[i * 4 + 0] = rects[i].x;
      flat[i * 4 + 1] = rects[i].y;
      flat[i * 4 + 2] = rects[i].width;
      flat[i * 4 + 3] = rects[i].height;
    }
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_erase_regions(
          h, page, flat, rects.length, err);
      _check(err);
    } finally {
      calloc.free(flat);
      calloc.free(err);
    }
  }

  // ── Editor: producer ──────────────────────────────────────────────

  String editorGetProducer(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(native.document_editor_get_producer(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetProducer(ffi.Pointer<ffi.Void> h, String v) {
    final err = calloc<ffi.Int>();
    final ptr = v.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_producer(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  // ── Editor: creation date ─────────────────────────────────────────

  String editorGetCreationDate(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      return _readFreeString(
          native.document_editor_get_creation_date(h, err));
    } finally {
      calloc.free(err);
    }
  }

  void editorSetCreationDate(ffi.Pointer<ffi.Void> h, String dateStr) {
    final err = calloc<ffi.Int>();
    final ptr = dateStr.toNativeUtf8(allocator: calloc);
    try {
      native.document_editor_set_creation_date(h, ptr.cast(), err);
      _check(err);
    } finally {
      calloc.free(ptr);
      calloc.free(err);
    }
  }

  // ── Read-only document: open/close ────────────────────────────────

  ffi.Pointer<ffi.Void> docOpenFromBytes(
    Uint8List bytes, {
    String? password,
  }) {
    final dataPtr = _allocBytes(bytes);
    final err = calloc<ffi.Int>();
    try {
      ffi.Pointer<ffi.Void> handle;
      if (password != null) {
        final passPtr = password.toNativeUtf8(allocator: calloc);
        try {
          handle = native.pdf_document_open_from_bytes_with_password(
            dataPtr, bytes.length, passPtr.cast(), err);
        } finally {
          calloc.free(passPtr);
        }
      } else {
        handle = native.pdf_document_open_from_bytes(dataPtr, bytes.length, err);
      }
      _check(err);
      return handle;
    } finally {
      calloc.free(dataPtr);
      calloc.free(err);
    }
  }

  void docFree(ffi.Pointer<ffi.Void> h) {
    native.pdf_document_free(h);
  }

  // ── Read-only document: extraction ────────────────────────────────

  int docPageCount(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final n = native.pdf_document_get_page_count(h, err);
      _check(err);
      return n;
    } finally {
      calloc.free(err);
    }
  }

  String docVersion(ffi.Pointer<ffi.Void> h) {
    final major = calloc<ffi.Uint8>();
    final minor = calloc<ffi.Uint8>();
    try {
      native.pdf_document_get_version(h, major, minor);
      return '${major.value}.${minor.value}';
    } finally {
      calloc.free(major);
      calloc.free(minor);
    }
  }

  bool docHasStructureTree(ffi.Pointer<ffi.Void> h) =>
      native.pdf_document_has_structure_tree(h);

  String docExtractText(ffi.Pointer<ffi.Void> h, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_extract_text(h, pageIndex, err);
      _check(err);
      return _readFreeString(ptr);
    } finally {
      calloc.free(err);
    }
  }

  String docToMarkdown(ffi.Pointer<ffi.Void> h, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_to_markdown(h, pageIndex, err);
      _check(err);
      return _readFreeString(ptr);
    } finally {
      calloc.free(err);
    }
  }

  String docToMarkdownAll(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_to_markdown_all(h, err);
      _check(err);
      return _readFreeString(ptr);
    } finally {
      calloc.free(err);
    }
  }

  String docToHtml(ffi.Pointer<ffi.Void> h, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_to_html(h, pageIndex, err);
      _check(err);
      return _readFreeString(ptr);
    } finally {
      calloc.free(err);
    }
  }

  String docToPlainText(ffi.Pointer<ffi.Void> h, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_to_plain_text(h, pageIndex, err);
      _check(err);
      return _readFreeString(ptr);
    } finally {
      calloc.free(err);
    }
  }

  // ── Read-only document: search ────────────────────────────────────

  List<SearchResult> docSearchPage(
    ffi.Pointer<ffi.Void> h,
    int pageIndex,
    String query, {
    bool caseSensitive = false,
  }) {
    final queryPtr = query.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      final results = native.pdf_document_search_page(
        h, pageIndex, queryPtr.cast(), caseSensitive, err);
      _check(err);
      return _readSearchResults(results);
    } finally {
      calloc.free(queryPtr);
      calloc.free(err);
    }
  }

  List<SearchResult> docSearchAll(
    ffi.Pointer<ffi.Void> h,
    String query, {
    bool caseSensitive = false,
  }) {
    final queryPtr = query.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      final results = native.pdf_document_search_all(
        h, queryPtr.cast(), caseSensitive, err);
      _check(err);
      return _readSearchResults(results);
    } finally {
      calloc.free(queryPtr);
      calloc.free(err);
    }
  }

  List<SearchResult> _readSearchResults(ffi.Pointer<ffi.Void> handle) {
    if (handle == ffi.nullptr) return [];
    final count = native.pdf_oxide_search_result_count(handle);
    final results = <SearchResult>[];
    final err = calloc<ffi.Int>();
    try {
      for (var i = 0; i < count; i++) {
        final text = _readFreeString(
            native.pdf_oxide_search_result_get_text(handle, i, err));
        final page = native.pdf_oxide_search_result_get_page(handle, i, err);
        final bx = calloc<ffi.Float>();
        final by = calloc<ffi.Float>();
        final bw = calloc<ffi.Float>();
        final bh = calloc<ffi.Float>();
        native.pdf_oxide_search_result_get_bbox(
            handle, i, bx, by, bw, bh, err);
        results.add(SearchResult(
          text: text,
          page: page,
          rect: PdfRect(
              x: bx.value, y: by.value, width: bw.value, height: bh.value),
        ));
        calloc.free(bx);
        calloc.free(by);
        calloc.free(bw);
        calloc.free(bh);
      }
    } finally {
      calloc.free(err);
      native.pdf_oxide_search_result_free(handle);
    }
    return results;
  }

  // ── Read-only document: page info ─────────────────────────────────

  double docPageWidth(ffi.Pointer<ffi.Void> h, int page) {
    final err = calloc<ffi.Int>();
    try {
      final v = native.pdf_page_get_width(h, page, err);
      _check(err);
      return v;
    } finally {
      calloc.free(err);
    }
  }

  double docPageHeight(ffi.Pointer<ffi.Void> h, int page) {
    final err = calloc<ffi.Int>();
    try {
      final v = native.pdf_page_get_height(h, page, err);
      _check(err);
      return v;
    } finally {
      calloc.free(err);
    }
  }

  int docPageRotation(ffi.Pointer<ffi.Void> h, int page) {
    final err = calloc<ffi.Int>();
    try {
      final v = native.pdf_page_get_rotation(h, page, err);
      _check(err);
      return v;
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: encrypted save ─────────────────────────────────────────

  Uint8List editorSaveEncrypted(
    ffi.Pointer<ffi.Void> h, {
    required String userPassword,
    required String ownerPassword,
  }) {
    final userPtr = userPassword.toNativeUtf8(allocator: calloc);
    final ownerPtr = ownerPassword.toNativeUtf8(allocator: calloc);
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.document_editor_save_encrypted_to_bytes(
        h, userPtr.cast(), ownerPtr.cast(), outLen, err);
      _check(err);
      return _readFreeBytes(ptr, outLen.value);
    } finally {
      calloc.free(userPtr);
      calloc.free(ownerPtr);
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  // ── Editor: encrypted save with algorithm + permissions ────────

  Uint8List editorSaveEncryptedFull(
    ffi.Pointer<ffi.Void> h, {
    required String userPassword,
    required String ownerPassword,
    int algorithm = 3,
    bool allowPrint = true,
    bool allowPrintHq = true,
    bool allowModify = true,
    bool allowCopy = true,
    bool allowAnnotate = true,
    bool allowFillForms = true,
    bool allowAccessibility = true,
    bool allowAssemble = true,
  }) {
    final userPtr = userPassword.toNativeUtf8(allocator: calloc);
    final ownerPtr = ownerPassword.toNativeUtf8(allocator: calloc);
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.document_editor_save_encrypted_full(
        h, userPtr.cast(), ownerPtr.cast(),
        algorithm,
        allowPrint, allowPrintHq,
        allowModify, allowCopy,
        allowAnnotate, allowFillForms,
        allowAccessibility, allowAssemble,
        outLen, err,
      );
      _check(err);
      return _readFreeBytes(ptr, outLen.value);
    } finally {
      calloc.free(userPtr);
      calloc.free(ownerPtr);
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  // ── Editor: positioned watermark ──────────────────────────────

  void editorAddWatermarkPositioned(
    ffi.Pointer<ffi.Void> h,
    int page,
    String text, {
    required double x,
    required double y,
    required double width,
    required double height,
    double fontSize = 48.0,
    String? fontName,
    double rotation = 45.0,
    double opacity = 0.3,
    double r = 0.5,
    double g = 0.5,
    double b = 0.5,
    bool fixedPrint = false,
    double fixedPrintH = 0.0,
    double fixedPrintV = 0.0,
  }) {
    final textPtr = text.toNativeUtf8(allocator: calloc);
    final fontPtr = fontName != null
        ? fontName.toNativeUtf8(allocator: calloc)
        : ffi.nullptr.cast<Utf8>();
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_add_watermark_positioned(
        h, page, textPtr.cast(),
        x, y, width, height,
        fontSize, fontPtr.cast(),
        rotation, opacity, r, g, b,
        fixedPrint, fixedPrintH, fixedPrintV,
        err,
      );
      _check(err);
    } finally {
      calloc.free(textPtr);
      if (fontName != null) calloc.free(fontPtr);
      calloc.free(err);
    }
  }

  // ── Editor: stamp annotation ──────────────────────────────────

  void editorAddStamp(
    ffi.Pointer<ffi.Void> h,
    int page, {
    required int stampType,
    String? customName,
    required double x,
    required double y,
    required double width,
    required double height,
    double opacity = 1.0,
  }) {
    final namePtr = customName != null
        ? customName.toNativeUtf8(allocator: calloc)
        : ffi.nullptr.cast<Utf8>();
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_add_stamp(
        h, page, stampType, namePtr.cast(),
        x, y, width, height, opacity, err,
      );
      _check(err);
    } finally {
      if (customName != null) calloc.free(namePtr);
      calloc.free(err);
    }
  }

  // ── Editor: image stamp ────────────────────────────────────────

  void editorAddImageStamp(
    ffi.Pointer<ffi.Void> h,
    int page,
    Uint8List imageBytes, {
    required double x,
    required double y,
    required double width,
    required double height,
    double opacity = 1.0,
  }) {
    final imgPtr = calloc<ffi.Uint8>(imageBytes.length);
    imgPtr.asTypedList(imageBytes.length).setAll(0, imageBytes);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_add_image_stamp(
        h, page, imgPtr.cast(), imageBytes.length,
        x, y, width, height, opacity, err,
      );
      _check(err);
    } finally {
      calloc.free(imgPtr);
      calloc.free(err);
    }
  }

  // ── Editor: image resize ──────────────────────────────────────

  void editorResizeImage(
    ffi.Pointer<ffi.Void> h,
    int page,
    String imageName,
    double newWidth,
    double newHeight,
  ) {
    final namePtr = imageName.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_resize_image(
        h, page, namePtr.cast(), newWidth, newHeight, err,
      );
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(err);
    }
  }

  // ── Editor: form field value ──────────────────────────────────────

  void editorSetFormFieldValue(
      ffi.Pointer<ffi.Void> h, String fieldName, String value) {
    final namePtr = fieldName.toNativeUtf8(allocator: calloc);
    final valPtr = value.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_set_form_field_value(
          h, namePtr.cast(), valPtr.cast(), err);
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(valPtr);
      calloc.free(err);
    }
  }

  // ── Editor: annotations ───────────────────────────────────────────

  void editorFlattenAllAnnotations(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_flatten_all_annotations(h, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: watermark ──────────────────────────────────────────────

  void editorAddWatermark(
    ffi.Pointer<ffi.Void> h,
    int page,
    String text, {
    double fontSize = 48.0,
    double rotation = 45.0,
    double opacity = 0.3,
    double r = 0.5,
    double g = 0.5,
    double b = 0.5,
  }) {
    final textPtr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_add_watermark(
        h, page, textPtr.cast(),
        fontSize, rotation, opacity, r, g, b, err,
      );
      _check(err);
    } finally {
      calloc.free(textPtr);
      calloc.free(err);
    }
  }

  // ── Editor: image optimization ─────────────────────────────────────

  /// Optimize images by converting non-JPEG to JPEG when smaller.
  /// Returns the number of images optimized.
  int editorOptimizeImages(ffi.Pointer<ffi.Void> h, {int quality = 75}) {
    final err = calloc<ffi.Int>();
    try {
      final count = native.document_editor_optimize_images(h, quality, err);
      _check(err);
      return count;
    } finally {
      calloc.free(err);
    }
  }

  int editorUnembedStandardFonts(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final count = native.document_editor_unembed_standard_fonts(h, err);
      _check(err);
      return count;
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: crop margins ──────────────────────────────────────────

  void editorCropMargins(
    ffi.Pointer<ffi.Void> h, {
    required double left,
    required double right,
    required double top,
    required double bottom,
  }) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_crop_margins(h, left, right, top, bottom, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ── Editor: PDF/A conversion ──────────────────────────────────────

  void editorConvertToPdfA(ffi.Pointer<ffi.Void> h, int level) {
    final err = calloc<ffi.Int>();
    try {
      native.document_editor_convert_to_pdf_a(h, level, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // BUILDER — create PDFs, watermark, images-to-PDF
  // ══════════════════════════════════════════════════════════════════

  ffi.Pointer<ffi.Void> builderCreate() {
    final err = calloc<ffi.Int>();
    try {
      final handle = native.pdf_document_builder_create(err);
      _check(err);
      return handle;
    } finally {
      calloc.free(err);
    }
  }

  void builderFree(ffi.Pointer<ffi.Void> h) {
    native.pdf_document_builder_free(h);
  }

  void builderSetTitle(ffi.Pointer<ffi.Void> h, String v) {
    final ptr = v.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_document_builder_set_title(h, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void builderSetAuthor(ffi.Pointer<ffi.Void> h, String v) {
    final ptr = v.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_document_builder_set_author(h, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void builderSetSubject(ffi.Pointer<ffi.Void> h, String v) {
    final ptr = v.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_document_builder_set_subject(h, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void builderSetKeywords(ffi.Pointer<ffi.Void> h, String v) {
    final ptr = v.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_document_builder_set_keywords(h, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  ffi.Pointer<ffi.Void> builderAddA4Page(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final page = native.pdf_document_builder_a4_page(h, err);
      _check(err);
      return page;
    } finally {
      calloc.free(err);
    }
  }

  ffi.Pointer<ffi.Void> builderAddLetterPage(ffi.Pointer<ffi.Void> h) {
    final err = calloc<ffi.Int>();
    try {
      final page = native.pdf_document_builder_letter_page(h, err);
      _check(err);
      return page;
    } finally {
      calloc.free(err);
    }
  }

  ffi.Pointer<ffi.Void> builderAddPage(ffi.Pointer<ffi.Void> h, double width, double height) {
    final err = calloc<ffi.Int>();
    try {
      final page = native.pdf_document_builder_page(h, width, height, err);
      _check(err);
      return page;
    } finally {
      calloc.free(err);
    }
  }

  void pageBuilderFont(ffi.Pointer<ffi.Void> page, String name, double size) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_font(page, namePtr.cast(), size, err); _check(err); }
    finally { calloc.free(namePtr); calloc.free(err); }
  }

  void pageBuilderAt(ffi.Pointer<ffi.Void> page, double x, double y) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_at(page, x, y, err); _check(err); }
    finally { calloc.free(err); }
  }

  void pageBuilderText(ffi.Pointer<ffi.Void> page, String text) {
    final ptr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_text(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderHeading(ffi.Pointer<ffi.Void> page, int level, String text) {
    final ptr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_heading(page, level, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderParagraph(ffi.Pointer<ffi.Void> page, String text) {
    final ptr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_paragraph(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderSpace(ffi.Pointer<ffi.Void> page, double points) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_space(page, points, err); _check(err); }
    finally { calloc.free(err); }
  }

  void pageBuilderHorizontalRule(ffi.Pointer<ffi.Void> page) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_horizontal_rule(page, err); _check(err); }
    finally { calloc.free(err); }
  }

  void pageBuilderDone(ffi.Pointer<ffi.Void> page) {
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_done(page, err);
      _check(err);
    } finally {
      calloc.free(err);
    }
  }

  void pageBuilderWatermark(ffi.Pointer<ffi.Void> page, String text) {
    final textPtr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_watermark(page, textPtr.cast(), err);
      _check(err);
    } finally {
      calloc.free(textPtr);
      calloc.free(err);
    }
  }

  void pageBuilderImage(
    ffi.Pointer<ffi.Void> page,
    Uint8List imageBytes,
    double x,
    double y,
    double width,
    double height, {
    String altText = '',
  }) {
    final imgPtr = _allocBytes(imageBytes);
    final altPtr = altText.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_image_with_alt(
        page,
        imgPtr,
        imageBytes.length,
        x,
        y,
        width,
        height,
        altPtr.cast(),
        err,
      );
      _check(err);
    } finally {
      calloc.free(imgPtr);
      calloc.free(altPtr);
      calloc.free(err);
    }
  }

  Uint8List builderBuild(ffi.Pointer<ffi.Void> h) {
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_builder_build(h, outLen, err);
      _check(err);
      return _readFreeBytes(ptr, outLen.value);
    } finally {
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  Uint8List builderBuildEncrypted(
    ffi.Pointer<ffi.Void> h, {
    required String userPassword,
    required String ownerPassword,
  }) {
    final userPtr = userPassword.toNativeUtf8(allocator: calloc);
    final ownerPtr = ownerPassword.toNativeUtf8(allocator: calloc);
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final ptr = native.pdf_document_builder_to_bytes_encrypted(
        h, userPtr.cast(), ownerPtr.cast(), outLen, err);
      _check(err);
      return _readFreeBytes(ptr, outLen.value);
    } finally {
      calloc.free(userPtr);
      calloc.free(ownerPtr);
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // RENDERING — page → image
  // ══════════════════════════════════════════════════════════════════

  RenderedPage renderPage(ffi.Pointer<ffi.Void> docHandle, int pageIndex,
      {int format = 0}) {
    final err = calloc<ffi.Int>();
    try {
      final imgHandle = native.pdf_render_page(docHandle, pageIndex, format, err);
      _check(err);
      try {
        return _extractRenderedImage(imgHandle);
      } finally {
        native.pdf_rendered_image_free(imgHandle);
      }
    } finally {
      calloc.free(err);
    }
  }

  RenderedPage renderPageFit(ffi.Pointer<ffi.Void> docHandle, int pageIndex,
      {required int fitWidth, required int fitHeight, int format = 0}) {
    final err = calloc<ffi.Int>();
    try {
      final imgHandle = native.pdf_render_page_fit(
          docHandle, pageIndex, fitWidth, fitHeight, format, err);
      _check(err);
      try {
        return _extractRenderedImage(imgHandle);
      } finally {
        native.pdf_rendered_image_free(imgHandle);
      }
    } finally {
      calloc.free(err);
    }
  }

  RenderedPage renderPageThumbnail(ffi.Pointer<ffi.Void> docHandle,
      int pageIndex, {required int thumbnailSize, int format = 0}) {
    final err = calloc<ffi.Int>();
    try {
      final imgHandle = native.pdf_render_page_thumbnail(
          docHandle, pageIndex, thumbnailSize, format, err);
      _check(err);
      try {
        return _extractRenderedImage(imgHandle);
      } finally {
        native.pdf_rendered_image_free(imgHandle);
      }
    } finally {
      calloc.free(err);
    }
  }

  RenderedPage _extractRenderedImage(ffi.Pointer<ffi.Void> imgHandle) {
    final errW = calloc<ffi.Int>();
    final errH = calloc<ffi.Int>();
    final dataLen = calloc<ffi.Int32>();
    final errD = calloc<ffi.Int>();
    try {
      final w = native.pdf_get_rendered_image_width(imgHandle, errW);
      _check(errW);
      final h = native.pdf_get_rendered_image_height(imgHandle, errH);
      _check(errH);
      final dataPtr = native.pdf_get_rendered_image_data(imgHandle, dataLen, errD);
      _check(errD);

      final len = dataLen.value;
      final data = Uint8List(len);
      data.setAll(0, dataPtr.cast<ffi.Uint8>().asTypedList(len));

      return RenderedPage(width: w, height: h, data: data);
    } finally {
      calloc.free(errW);
      calloc.free(errH);
      calloc.free(dataLen);
      calloc.free(errD);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // IMAGE EXTRACTION — embedded images from pages
  // ══════════════════════════════════════════════════════════════════

  List<PdfImage> docGetEmbeddedImages(
      ffi.Pointer<ffi.Void> docHandle, int pageIndex) {
    final err = calloc<ffi.Int>();
    try {
      final listHandle =
          native.pdf_document_get_embedded_images(docHandle, pageIndex, err);
      _check(err);
      if (listHandle == ffi.nullptr) return const [];

      try {
        final count = native.pdf_oxide_image_count(listHandle);
        final images = <PdfImage>[];

        for (var i = 0; i < count; i++) {
          final errI = calloc<ffi.Int>();
          try {
            final w = native.pdf_oxide_image_get_width(listHandle, i, errI);
            _check(errI);
            final h = native.pdf_oxide_image_get_height(listHandle, i, errI);
            _check(errI);

            final fmtPtr = native.pdf_oxide_image_get_format(listHandle, i, errI);
            _check(errI);
            final fmt = _readFreeString(fmtPtr);

            final csPtr =
                native.pdf_oxide_image_get_colorspace(listHandle, i, errI);
            _check(errI);
            final cs = _readFreeString(csPtr);

            final bpc =
                native.pdf_oxide_image_get_bits_per_component(listHandle, i, errI);
            _check(errI);

            final dataLen = calloc<ffi.Int>();
            try {
              final dataPtr =
                  native.pdf_oxide_image_get_data(listHandle, i, dataLen, errI);
              _check(errI);

              final len = dataLen.value;
              final data = Uint8List(len);
              data.setAll(0, dataPtr.cast<ffi.Uint8>().asTypedList(len));

              images.add(PdfImage(
                width: w,
                height: h,
                format: fmt,
                colorSpace: cs,
                bitsPerComponent: bpc,
                data: data,
              ));
            } finally {
              calloc.free(dataLen);
            }
          } finally {
            calloc.free(errI);
          }
        }

        return images;
      } finally {
        native.pdf_oxide_image_list_free(listHandle);
      }
    } finally {
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // DIGITAL SIGNATURES
  // ══════════════════════════════════════════════════════════════════

  int docGetSignatureCount(ffi.Pointer<ffi.Void> docHandle) {
    final err = calloc<ffi.Int>();
    try {
      final count = native.pdf_document_get_signature_count(docHandle, err);
      _check(err);
      return count;
    } finally {
      calloc.free(err);
    }
  }

  PdfSignatureInfo docGetSignature(ffi.Pointer<ffi.Void> docHandle, int index) {
    final err = calloc<ffi.Int>();
    try {
      final sigHandle = native.pdf_document_get_signature(docHandle, index, err);
      _check(err);
      try {
        return _extractSignatureInfo(sigHandle);
      } finally {
        native.pdf_signature_free(sigHandle);
      }
    } finally {
      calloc.free(err);
    }
  }

  bool docVerifyAllSignatures(ffi.Pointer<ffi.Void> docHandle) {
    final count = docGetSignatureCount(docHandle);
    if (count == 0) return true;
    final err = calloc<ffi.Int>();
    try {
      final result = native.pdf_document_verify_all_signatures(docHandle, err);
      _check(err);
      return result != 0;
    } finally {
      calloc.free(err);
    }
  }

  Uint8List signBytes(Uint8List pdfData, Uint8List certBytes, String certPassword,
      {String? reason, String? location}) {
    final pdfPtr = _allocBytes(pdfData);
    final certPtr = _allocBytes(certBytes);
    final pwPtr = certPassword.toNativeUtf8(allocator: calloc);
    final reasonPtr = (reason ?? '').toNativeUtf8(allocator: calloc);
    final locationPtr = (location ?? '').toNativeUtf8(allocator: calloc);
    final outLen = calloc<ffi.Size>();
    final err = calloc<ffi.Int>();
    try {
      final certHandle = native.pdf_certificate_load_from_bytes(
          certPtr, certBytes.length, pwPtr.cast(), err);
      _check(err);
      try {
        final ptr = native.pdf_sign_bytes(
            pdfPtr, pdfData.length, certHandle,
            reasonPtr.cast(), locationPtr.cast(), outLen, err);
        _check(err);
        return _readFreeBytes(ptr, outLen.value);
      } finally {
        native.pdf_certificate_free(certHandle);
      }
    } finally {
      calloc.free(pdfPtr);
      calloc.free(certPtr);
      calloc.free(pwPtr);
      calloc.free(reasonPtr);
      calloc.free(locationPtr);
      calloc.free(outLen);
      calloc.free(err);
    }
  }

  PdfSignatureInfo _extractSignatureInfo(ffi.Pointer<ffi.Void> sigHandle) {
    final err = calloc<ffi.Int>();
    try {
      final verifyResult = native.pdf_signature_verify(sigHandle, err);
      _check(err);

      final name = _readFreeString(native.pdf_signature_get_signer_name(sigHandle, err));
      final timeMs = native.pdf_signature_get_signing_time(sigHandle, err);
      final reason = _readFreeString(native.pdf_signature_get_signing_reason(sigHandle, err));
      final location = _readFreeString(native.pdf_signature_get_signing_location(sigHandle, err));

      PdfCertificateInfo? certInfo;
      final certErr = calloc<ffi.Int>();
      try {
        final certHandle = native.pdf_signature_get_certificate(sigHandle, certErr);
        if (certHandle != ffi.nullptr) {
          try {
            certInfo = _extractCertificateInfo(certHandle);
          } finally {
            native.pdf_certificate_free(certHandle);
          }
        }
      } finally {
        calloc.free(certErr);
      }

      return PdfSignatureInfo(
        signerName: name.isEmpty ? null : name,
        signingTime: timeMs > 0 ? DateTime.fromMillisecondsSinceEpoch(timeMs * 1000) : null,
        reason: reason.isEmpty ? null : reason,
        location: location.isEmpty ? null : location,
        isValid: verifyResult != 0,
        certificate: certInfo,
      );
    } finally {
      calloc.free(err);
    }
  }

  PdfCertificateInfo _extractCertificateInfo(ffi.Pointer<ffi.Void> certHandle) {
    final err = calloc<ffi.Int>();
    final notBefore = calloc<ffi.Int64>();
    final notAfter = calloc<ffi.Int64>();
    try {
      final subject = _readFreeString(native.pdf_certificate_get_subject(certHandle, err));
      final issuer = _readFreeString(native.pdf_certificate_get_issuer(certHandle, err));
      final serial = _readFreeString(native.pdf_certificate_get_serial(certHandle, err));
      native.pdf_certificate_get_validity(certHandle, notBefore, notAfter, err);
      final isValid = native.pdf_certificate_is_valid(certHandle, err);

      return PdfCertificateInfo(
        subject: subject.isEmpty ? null : subject,
        issuer: issuer.isEmpty ? null : issuer,
        serial: serial.isEmpty ? null : serial,
        notBefore: notBefore.value > 0
            ? DateTime.fromMillisecondsSinceEpoch(notBefore.value * 1000)
            : null,
        notAfter: notAfter.value > 0
            ? DateTime.fromMillisecondsSinceEpoch(notAfter.value * 1000)
            : null,
        isValid: isValid != 0,
      );
    } finally {
      calloc.free(err);
      calloc.free(notBefore);
      calloc.free(notAfter);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // PDF/A VALIDATION
  // ══════════════════════════════════════════════════════════════════

  ({bool compliant, int errors, int warnings}) docValidatePdfA(
      ffi.Pointer<ffi.Void> docHandle, {int level = 2}) {
    final err = calloc<ffi.Int>();
    try {
      final results = native.pdf_validate_pdf_a_level(docHandle, level, err);
      _check(err);
      if (results == ffi.nullptr) {
        return (compliant: false, errors: 0, warnings: 0);
      }
      try {
        final compErr = calloc<ffi.Int>();
        try {
          final compliant = native.pdf_pdf_a_is_compliant(results, compErr);
          _check(compErr);
          final errorCount = native.pdf_pdf_a_error_count(results);
          final warnCount = native.pdf_pdf_a_warning_count(results);
          return (compliant: compliant, errors: errorCount, warnings: warnCount);
        } finally {
          calloc.free(compErr);
        }
      } finally {
        native.pdf_pdf_a_results_free(results);
      }
    } finally {
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // PDF/UA VALIDATION
  // ══════════════════════════════════════════════════════════════════

  bool docValidatePdfUa(ffi.Pointer<ffi.Void> docHandle, {int level = 1}) {
    final err = calloc<ffi.Int>();
    try {
      final results = native.pdf_validate_pdf_ua(docHandle, level, err);
      _check(err);
      if (results == ffi.nullptr) return false;
      try {
        final errCheck = calloc<ffi.Int>();
        try {
          final accessible = native.pdf_pdf_ua_is_accessible(results, errCheck);
          _check(errCheck);
          return accessible;
        } finally {
          calloc.free(errCheck);
        }
      } finally {
        // pdf_oxide frees the results handle internally
      }
    } finally {
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // READ ENCRYPTION INFO
  // ══════════════════════════════════════════════════════════════════

  ({bool print, bool printHq, bool modify, bool copy, bool annotate,
    bool fillForms, bool accessibility, bool assemble})
  docGetPermissions(ffi.Pointer<ffi.Void> docHandle) {
    final p = calloc<ffi.Bool>(8);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_document_get_permissions(
        docHandle,
        p + 0, p + 1, p + 2, p + 3,
        p + 4, p + 5, p + 6, p + 7,
        err,
      );
      _check(err);
      return (
        print: p[0], printHq: p[1], modify: p[2], copy: p[3],
        annotate: p[4], fillForms: p[5], accessibility: p[6], assemble: p[7],
      );
    } finally {
      calloc.free(p);
      calloc.free(err);
    }
  }

  int docGetEncryptionAlgorithm(ffi.Pointer<ffi.Void> docHandle) {
    final err = calloc<ffi.Int>();
    try {
      final result = native.pdf_document_get_encryption_algorithm(docHandle, err);
      _check(err);
      return result;
    } finally {
      calloc.free(err);
    }
  }

  // ══════════════════════════════════════════════════════════════════
  // FORM FIELD CREATION (via PageBuilder)
  // ══════════════════════════════════════════════════════════════════

  void pageBuilderTextField(ffi.Pointer<ffi.Void> page, String name,
      double x, double y, double w, double h, {String? defaultValue}) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final valPtr = defaultValue != null
        ? defaultValue.toNativeUtf8(allocator: calloc)
        : ffi.nullptr.cast<Utf8>();
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_text_field(page, namePtr.cast(), x, y, w, h, valPtr.cast(), err);
      _check(err);
    } finally {
      calloc.free(namePtr);
      if (defaultValue != null) calloc.free(valPtr);
      calloc.free(err);
    }
  }

  void pageBuilderCheckbox(ffi.Pointer<ffi.Void> page, String name,
      double x, double y, double w, double h, {bool checked = false}) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_checkbox(page, namePtr.cast(), x, y, w, h, checked ? 1 : 0, err);
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(err);
    }
  }

  void pageBuilderComboBox(ffi.Pointer<ffi.Void> page, String name,
      double x, double y, double w, double h,
      List<String> options, {String? selected}) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final optionPtrs = options.map((o) => o.toNativeUtf8(allocator: calloc)).toList();
    final optionsArray = calloc<ffi.Pointer<ffi.Char>>(options.length);
    for (var i = 0; i < options.length; i++) {
      optionsArray[i] = optionPtrs[i].cast();
    }
    final selPtr = selected != null
        ? selected.toNativeUtf8(allocator: calloc)
        : ffi.nullptr.cast<Utf8>();
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_combo_box(
        page, namePtr.cast(), x, y, w, h,
        optionsArray, options.length, selPtr.cast(), err,
      );
      _check(err);
    } finally {
      calloc.free(namePtr);
      for (final p in optionPtrs) {
        calloc.free(p);
      }
      calloc.free(optionsArray);
      if (selected != null) {
        calloc.free(selPtr);
      }
      calloc.free(err);
    }
  }

  void pageBuilderPushButton(ffi.Pointer<ffi.Void> page, String name,
      double x, double y, double w, double h, String caption) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final capPtr = caption.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_push_button(page, namePtr.cast(), x, y, w, h, capPtr.cast(), err);
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(capPtr);
      calloc.free(err);
    }
  }

  void pageBuilderSignatureField(ffi.Pointer<ffi.Void> page, String name,
      double x, double y, double w, double h) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_signature_field(page, namePtr.cast(), x, y, w, h, err);
      _check(err);
    } finally {
      calloc.free(namePtr);
      calloc.free(err);
    }
  }

  void pageBuilderRadioGroup(ffi.Pointer<ffi.Void> page, String name,
      List<String> values,
      List<double> xs, List<double> ys, List<double> ws, List<double> hs,
      {String? selected}) {
    final namePtr = name.toNativeUtf8(allocator: calloc);
    final valPtrs = values.map((v) => v.toNativeUtf8(allocator: calloc)).toList();
    final valsArray = calloc<ffi.Pointer<ffi.Char>>(values.length);
    for (var i = 0; i < values.length; i++) {
      valsArray[i] = valPtrs[i].cast();
    }
    final xArr = calloc<ffi.Float>(xs.length);
    final yArr = calloc<ffi.Float>(ys.length);
    final wArr = calloc<ffi.Float>(ws.length);
    final hArr = calloc<ffi.Float>(hs.length);
    for (var i = 0; i < xs.length; i++) {
      xArr[i] = xs[i]; yArr[i] = ys[i]; wArr[i] = ws[i]; hArr[i] = hs[i];
    }
    final selPtr = selected != null
        ? selected.toNativeUtf8(allocator: calloc)
        : ffi.nullptr.cast<Utf8>();
    final err = calloc<ffi.Int>();
    try {
      native.pdf_page_builder_radio_group(
        page, namePtr.cast(), valsArray, xArr, yArr, wArr, hArr,
        values.length, selPtr.cast(), err,
      );
      _check(err);
    } finally {
      calloc.free(namePtr);
      for (final p in valPtrs) {
        calloc.free(p);
      }
      calloc.free(valsArray);
      calloc.free(xArr); calloc.free(yArr);
      calloc.free(wArr); calloc.free(hArr);
      if (selected != null) calloc.free(selPtr);
      calloc.free(err);
    }
  }

  // ── Field validation scripts ──────────────────────────────────

  void pageBuilderFieldKeystroke(ffi.Pointer<ffi.Void> page, String script) {
    final ptr = script.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_field_keystroke(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderFieldFormat(ffi.Pointer<ffi.Void> page, String script) {
    final ptr = script.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_field_format(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderFieldValidate(ffi.Pointer<ffi.Void> page, String script) {
    final ptr = script.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_field_validate(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderFieldCalculate(ffi.Pointer<ffi.Void> page, String script) {
    final ptr = script.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_field_calculate(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  // ── Page builder: links ───────────────────────────────────────

  void pageBuilderLinkUrl(ffi.Pointer<ffi.Void> page, String url) {
    final ptr = url.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_link_url(page, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderLinkPage(ffi.Pointer<ffi.Void> page, int targetPage) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_link_page(page, targetPage, err); _check(err); }
    finally { calloc.free(err); }
  }

  // ── Page builder: layout ──────────────────────────────────────

  void pageBuilderFootnote(ffi.Pointer<ffi.Void> page, String refMark, String noteText) {
    final refPtr = refMark.toNativeUtf8(allocator: calloc);
    final notePtr = noteText.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_footnote(page, refPtr.cast(), notePtr.cast(), err); _check(err); }
    finally { calloc.free(refPtr); calloc.free(notePtr); calloc.free(err); }
  }

  void pageBuilderColumns(ffi.Pointer<ffi.Void> page, int columnCount, double gapPt, String text) {
    final ptr = text.toNativeUtf8(allocator: calloc);
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_columns(page, columnCount, gapPt, ptr.cast(), err); _check(err); }
    finally { calloc.free(ptr); calloc.free(err); }
  }

  void pageBuilderNewline(ffi.Pointer<ffi.Void> page) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_newline(page, err); _check(err); }
    finally { calloc.free(err); }
  }

  void pageBuilderNewPageSameSize(ffi.Pointer<ffi.Void> page) {
    final err = calloc<ffi.Int>();
    try { native.pdf_page_builder_new_page_same_size(page, err); _check(err); }
    finally { calloc.free(err); }
  }
}

