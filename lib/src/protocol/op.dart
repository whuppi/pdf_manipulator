// Shared operation protocol — the single source of truth for op names,
// arg shapes, and result parsing between native and web bridges.
//
// Both NativeBridge and WebBridge use these builders to construct ops.
// The native worker_entry.dart and web wasm_worker.js both receive
// the same op names and arg shapes.
//
// Adding a new op: add it here, both platforms pick it up.
// Renaming an op: rename here, compiler breaks both platforms until fixed.

import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';

/// Every operation the engine can execute. String values are the wire names
/// used by both native worker_entry.dart and web wasm_worker.js.
enum EngineOp {
  // ── Inspect ──
  open('open'),

  // ── Structural (source → edit → sink) ──
  merge('merge'),
  extractPages('extractPages'),
  deletePages('deletePages'),
  reorderPages('reorderPages'),
  movePage('movePage'),
  rotatePages('rotatePages'),
  rotateAllPages('rotateAllPages'),

  // ── Content (source → edit → sink) ──
  flattenForms('flattenForms'),
  applyRedactions('applyRedactions'),
  embedFile('embedFile'),
  eraseRegions('eraseRegions'),
  compress('compress'),

  // ── Extraction (source → read → result) ──
  extract('extract'),
  search('search'),

  // ── Security (source → edit → sink) ──
  watermark('watermark'),
  encrypt('encrypt'),
  decrypt('decrypt'),
  sign('sign'),

  // ── Stamps (source → edit → sink) ──
  addStamp('addStamp'),
  addImageStamp('addImageStamp'),

  // ── Creation (no source) ──
  imagesToPdf('imagesToPdf'),

  // ── Streaming reads (source → stream items) ──
  render('render'),
  extractImages('extractImages'),

  // ── Signatures / Validation (source → read → result) ──
  getSignatures('getSignatures'),
  verifySignatures('verifySignatures'),
  validatePdfA('validatePdfA'),
  validatePdfUa('validatePdfUa'),

  // ── Editor handle ops ──
  editorOpen('editorOpen'),
  editorDispose('editorDispose'),
  editorMutate('editorMutate'),
  editorSave('editorSave'),
  editorGetMetadata('editorGetMetadata'),
  editorPageMediaBox('editorPageMediaBox'),
  editorExtractPages('editorExtractPages'),
  editorMergeFrom('editorMergeFrom'),

  // ── Builder handle ops ──
  builderCreate('builderCreate'),
  builderDispose('builderDispose'),
  builderSetMetadata('builderSetMetadata'),
  builderAddPage('builderAddPage'),
  builderPageOp('builderPageOp'),
  builderPageDone('builderPageDone'),
  builderSave('builderSave');

  const EngineOp(this.wire);

  /// The string sent over the wire (isolate message / postMessage).
  final String wire;
}

/// Resolved page indices from sealed PdfPages.
List<int> resolvePageIndices(PdfPages pages, int pageCount) {
  return switch (pages) {
    PdfAllPages() => List.generate(pageCount, (i) => i),
    PdfSinglePage(:final index) => [index],
    PdfPageList(:final indices) => indices,
    PdfPageRange(:final start, :final end) => List.generate(end - start, (i) => start + i),
  };
}

/// Build args map for edit ops (shared between native and web).
Map<String, Object?> buildEditArgs(EngineOp op, Map<String, Object?> extra) {
  return {'op': op.wire, ...extra};
}

/// Encode regions as a transferable list of maps.
List<Map<String, double>> encodeRegions(List<PdfRect> regions) {
  return regions.map((r) => {
    'x': r.x, 'y': r.y, 'width': r.width, 'height': r.height,
  }).toList();
}

/// Encode watermark style as flat args.
Map<String, Object?> encodeWatermarkArgs(String text, PdfWatermarkStyle style) {
  return {
    'text': text,
    'opacity': style.opacity,
    'fontSize': style.fontSize,
    'rotation': style.rotation,
    'r': style.color.r,
    'g': style.color.g,
    'b': style.color.b,
  };
}

/// Encode stamp rect as flat args.
Map<String, Object?> encodeRectArgs(PdfRect rect) {
  return {'x': rect.x, 'y': rect.y, 'width': rect.width, 'height': rect.height};
}

/// Encode save options as flat args.
Map<String, Object?> encodeSaveArgs(PdfSaveOptions options) {
  return {
    'compress': options.compress,
    'garbageCollect': options.garbageCollect,
    'linearize': options.linearize,
  };
}

/// Extraction format to wire string.
String encodeExtractionFormat(PdfExtractionFormat format) {
  return switch (format) {
    PdfExtractionFormat.markdown => 'markdown',
    PdfExtractionFormat.html => 'html',
    PdfExtractionFormat.plainText => 'plainText',
    _ => 'plainText',
  };
}
