// EngineOp — the wire names for every operation the engine can execute.
// SharedBridge uses these. bridge_api.rs matches on the .wire string.
// Adding a new op: add it here, both platforms pick it up.
// Renaming an op: rename here, compiler breaks both platforms until fixed.

/// Wire-level operation identifiers matching the Rust engine's op names.
enum EngineOp {
  // ── Document handle ops ──

  /// Open a PDF document.
  open('open'),

  /// Dispose a document handle.
  docDispose('docDispose'),

  /// Extract text content.
  extract('extract'),

  /// Full-text search across pages.
  search('search'),

  /// Render pages to pixel buffers.
  render('render'),

  /// Extract embedded images.
  extractImages('extractImages'),

  /// Get digital signature metadata.
  getSignatures('getSignatures'),

  /// Verify all digital signatures.
  verifySignatures('verifySignatures'),

  /// Validate PDF/A conformance.
  validatePdfA('validatePdfA'),

  /// Validate PDF/UA accessibility.
  validatePdfUa('validatePdfUa'),

  /// Plan bookmark-based split points.
  planSplitByBookmarks('planSplitByBookmarks'),

  /// Classify a single page.
  classifyPage('classifyPage'),

  /// Classify the entire document.
  classifyDocument('classifyDocument'),

  // ── Standalone write ──

  /// Digitally sign a PDF.
  sign('sign'),

  /// Convert PDF to another format.
  convertTo('convertTo'),

  /// Convert a document to PDF.
  convertToPdf('convertToPdf'),

  /// Register a runtime fallback font (form-value baking).
  registerFallbackFont('registerFallbackFont'),

  // ── Editor handle ops ──

  /// Open a PDF for editing.
  editorOpen('editorOpen'),

  /// Dispose an editor handle.
  editorDispose('editorDispose'),

  /// Execute a mutation on the editor.
  editorMutate('editorMutate'),

  /// Save the edited document.
  editorSave('editorSave'),

  /// Get editor metadata (page count, version, title, etc.).
  editorGetMetadata('editorGetMetadata'),

  /// Check whether the editor has unsaved modifications.
  editorIsModified('editorIsModified'),

  /// Get a page's media box rectangle.
  editorPageMediaBox('editorPageMediaBox'),

  /// Count pending redaction marks on a page.
  editorRedactionCount('editorRedactionCount'),

  /// Merge pages from another PDF into the editor.
  editorMergeFrom('editorMergeFrom'),

  /// Extract pages without modifying the editor.
  editorExtractPages('editorExtractPages'),

  // ── Builder handle ops ──

  /// Create a new PDF builder session.
  builderCreate('builderCreate'),

  /// Dispose a builder handle.
  builderDispose('builderDispose'),

  /// Set builder-level metadata.
  builderSetMetadata('builderSetMetadata'),

  /// Add a new page to the builder.
  builderAddPage('builderAddPage'),

  /// Execute a page-builder operation.
  builderPageOp('builderPageOp'),

  /// Finalise the current page.
  builderPageDone('builderPageDone'),

  /// Save the built PDF.
  builderSave('builderSave');

  /// Creates an op with the given [wire] name.
  const EngineOp(this.wire);

  /// The string sent on the wire, matching the Rust side.
  final String wire;
}
