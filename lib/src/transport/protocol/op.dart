// EngineOp — the wire names for every operation the engine can execute.
// SharedBridge uses these. bridge_api.rs matches on the .wire string.
// Adding a new op: add it here, both platforms pick it up.
// Renaming an op: rename here, compiler breaks both platforms until fixed.

enum EngineOp {
  // ── Document handle ops ──
  open('open'),
  docDispose('docDispose'),
  extract('extract'),
  search('search'),
  render('render'),
  extractImages('extractImages'),
  getSignatures('getSignatures'),
  verifySignatures('verifySignatures'),
  validatePdfA('validatePdfA'),
  validatePdfUa('validatePdfUa'),
  planSplitByBookmarks('planSplitByBookmarks'),
  classifyPage('classifyPage'),
  classifyDocument('classifyDocument'),

  // ── Standalone write ──
  sign('sign'),
  convertTo('convertTo'),
  convertToPdf('convertToPdf'),

  // ── Editor handle ops ──
  editorOpen('editorOpen'),
  editorDispose('editorDispose'),
  editorMutate('editorMutate'),
  editorSave('editorSave'),
  editorGetMetadata('editorGetMetadata'),
  editorIsModified('editorIsModified'),
  editorPageMediaBox('editorPageMediaBox'),
  editorRedactionCount('editorRedactionCount'),
  editorMergeFrom('editorMergeFrom'),
  editorExtractPages('editorExtractPages'),

  // ── Builder handle ops ──
  builderCreate('builderCreate'),
  builderDispose('builderDispose'),
  builderSetMetadata('builderSetMetadata'),
  builderAddPage('builderAddPage'),
  builderPageOp('builderPageOp'),
  builderPageDone('builderPageDone'),
  builderSave('builderSave');

  const EngineOp(this.wire);
  final String wire;
}
