// Operation codes for the message-based dispatch protocol.
// Used by both _native.dart (isolate) and _web.dart (Web Worker).
// No closures cross boundaries — only typed messages with transferable bytes.

enum Op {
  // Inspect
  open,
  probe,

  // Structural
  merge,
  split,
  splitBySize,
  extractPages,
  deletePages,
  reorderPages,
  movePage,
  rotatePages,
  rotateAllPages,

  // Content
  flattenForms,
  applyRedactions,
  embedFile,
  eraseRegions,
  compress,

  // Extraction
  extractText,
  toMarkdown,
  toHtml,
  toPlainText,

  // Search
  searchPage,
  searchAll,

  // Security
  watermark,
  watermarkPositioned,
  encrypt,
  encryptFull,
  decrypt,
  sign,

  // Creation
  imagesToPdf,

  // Rendering
  renderPage,
  renderPageFit,
  renderPageThumbnail,
  renderAllPages,

  // Image extraction
  extractImages,
  extractAllImages,

  // Signatures
  getSignatureCount,
  getSignatures,
  verifySignatures,

  // Validation
  validatePdfA,
  validatePdfUa,

  // Editor handle ops
  editorOpen,
  editorDispose,
  editorPageCount,
  editorVersion,
  editorIsModified,
  editorGetTitle,
  editorSetTitle,
  editorGetAuthor,
  editorSetAuthor,
  editorGetSubject,
  editorSetSubject,
  editorGetKeywords,
  editorSetKeywords,
  editorRotatePage,
  editorRotateAllPages,
  editorGetPageMediaBox,
  editorDeletePage,
  editorMovePage,
  editorExtractPages,
  editorMergeFrom,
  editorOptimizeImages,
  editorUnembedStandardFonts,
  editorAddWatermark,
  editorEmbedFile,
  editorEraseRegions,
  editorFlattenForms,
  editorFlattenAllAnnotations,
  editorApplyAllRedactions,
  editorSetFormFieldValue,
  editorCropMargins,
  editorConvertToPdfA,
  editorSave,
  editorSaveWithOptions,
  editorSaveEncrypted,
  editorSaveEncryptedFull,
  editorAddWatermarkPositioned,
  editorAddStamp,
  editorAddImageStamp,
  editorResizeImage,

  // Builder handle ops
  builderCreate,
  builderDispose,
  builderSetTitle,
  builderSetAuthor,
  builderSetSubject,
  builderSetKeywords,
  builderAddA4Page,
  builderAddLetterPage,
  builderAddPage,
  builderBuild,
  builderBuildEncrypted,

  // Page builder ops
  pageFont,
  pageAt,
  pageText,
  pageHeading,
  pageParagraph,
  pageSpace,
  pageHorizontalRule,
  pageImage,
  pageWatermark,
  pageDone,

  // Page builder form field ops
  pageTextField,
  pageCheckbox,
  pageComboBox,
  pagePushButton,
  pageSignatureField,
  pageRadioGroup,
  pageFieldKeystroke,
  pageFieldFormat,
  pageFieldValidate,
  pageFieldCalculate,

  // Page builder link ops
  pageLinkUrl,
  pageLinkPage,

  // Page builder layout ops
  pageFootnote,
  pageColumns,
  pageNewline,
  pageNewPageSameSize,

  // Encryption info
  getPermissions,
  getEncryptionAlgorithm,

  // Lifecycle
  dispose,
}
