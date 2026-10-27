// EngineOp — the wire names for every operation the engine can execute.
// SharedBridge uses these. The host registry (host/ops/registry.rs)
// matches on the .wire string.
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

  /// Read every AcroForm field of a document.
  formFields('formFields'),

  /// Export AcroForm field values as FDF or XFDF.
  exportFormData('exportFormData'),

  /// Read the document's XFA structure, or its absence.
  xfa('xfa'),

  /// List the files embedded in the document.
  attachments('attachments'),

  /// List one page's annotations.
  annotations('annotations'),

  /// Stream one embedded file's bytes to a sink.
  extractAttachment('extractAttachment'),

  // ── Standalone write ──

  /// Digitally sign a PDF.
  sign('sign'),

  /// Convert PDF to another format.
  convertTo('convertTo'),

  /// Convert a document to PDF.
  convertToPdf('convertToPdf'),

  /// Convert an XFA document into a plain AcroForm document.
  convertXfaToAcroForm('convertXfaToAcroForm'),

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

  /// Get a page's crop box rectangle, or absence when unset.
  editorPageCropBox('editorPageCropBox'),

  /// List the image XObjects placed on a page.
  editorPageImages('editorPageImages'),

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

/// Wire names of the page-builder sub-operations carried by
/// [EngineOp.builderPageOp] in its `pageOp` field. The host matches
/// each one in `host/ops/builder.rs`.
enum PageOp {
  /// Select the font and size for following text.
  font('font'),

  /// Move the text cursor.
  at('at'),

  /// Write a line of text.
  text('text'),

  /// Write a heading.
  heading('heading'),

  /// Write a wrapped paragraph.
  paragraph('paragraph'),

  /// Add vertical space.
  space('space'),

  /// Draw a horizontal rule.
  horizontalRule('horizontalRule'),

  /// Add a text watermark.
  watermark('watermark'),

  /// Place an image.
  image('image'),

  /// Add a text form field.
  textField('textField'),

  /// Add a checkbox form field.
  checkbox('checkbox'),

  /// Add a combo box form field.
  comboBox('comboBox'),

  /// Add a push button form field.
  pushButton('pushButton'),

  /// Add a signature form field.
  signatureField('signatureField'),

  /// Add a radio button group.
  radioGroup('radioGroup'),

  /// Attach a keystroke script to the last field.
  fieldKeystroke('fieldKeystroke'),

  /// Attach a format script to the last field.
  fieldFormat('fieldFormat'),

  /// Attach a validate script to the last field.
  fieldValidate('fieldValidate'),

  /// Attach a calculate script to the last field.
  fieldCalculate('fieldCalculate'),

  /// Link the last text to a URL.
  linkUrl('linkUrl'),

  /// Link the last text to a page.
  linkPage('linkPage'),

  /// Add a footnote.
  footnote('footnote'),

  /// Lay text out in columns.
  columns('columns'),

  /// Move to the next line.
  newline('newline'),

  /// Start a new page of the same size.
  newPageSameSize('newPageSameSize'),

  /// Write an inline text run.
  inline('inline'),

  /// Write a bold inline text run.
  inlineBold('inlineBold'),

  /// Write an italic inline text run.
  inlineItalic('inlineItalic'),

  /// Write a coloured inline text run.
  inlineColor('inlineColor'),

  /// Write text inside a rectangle.
  textInRect('textInRect'),

  /// Stroke a rectangle outline.
  rect('rect'),

  /// Fill a rectangle.
  filledRect('filledRect'),

  /// Draw a line.
  line('line'),

  /// Stroke a styled rectangle outline.
  strokeRect('strokeRect'),

  /// Draw a styled line.
  strokeLine('strokeLine'),

  /// Highlight the last text.
  highlight('highlight'),

  /// Underline the last text.
  underline('underline'),

  /// Strike out the last text.
  strikeout('strikeout'),

  /// Squiggle-underline the last text.
  squiggly('squiggly'),

  /// Add a sticky note annotation.
  stickyNote('stickyNote'),

  /// Add a stamp annotation.
  stamp('stamp'),

  /// Add a free-text annotation.
  freeText('freeText'),

  /// Add the CONFIDENTIAL watermark.
  watermarkConfidential('watermarkConfidential'),

  /// Add the DRAFT watermark.
  watermarkDraft('watermarkDraft'),

  /// Link the last text to a named destination.
  linkNamed('linkNamed'),

  /// Link the last text to a JavaScript action.
  linkJavaScript('linkJavaScript'),

  /// Run a script when the document opens.
  onOpen('onOpen'),

  /// Run a script when the document closes.
  onClose('onClose'),

  /// Place an image as a page artifact.
  imageArtifact('imageArtifact'),

  /// Add a buffered table.
  table('table'),

  /// Add a streaming table block.
  streamingTable('streamingTable');

  /// Creates a page op with the given [wire] name.
  const PageOp(this.wire);

  /// The string sent on the wire, matching the Rust side.
  final String wire;
}
