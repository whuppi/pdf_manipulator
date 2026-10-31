// Members of PdfEditor and PdfDoc that are not on PdfEdits / PdfReads, each
// with the reason it has no one-shot. The list shrinks as the render, image
// and sweep tasks move members onto the base classes; the membership test
// fails on a stale entry.

/// Reason: an editor read is not an edit, so the one-shot of edits has no use
/// for it (`Pdf.read` and `PdfDoc` answer it).
const _editorRead = 'a read on the editor, not an edit';

/// Reason: the door saves.
const _save = 'the one-shot saves its own result';

/// Reason: `merge` is its one-shot.
const _merge = 'Pdf.merge is its one-shot';

/// Reason: the redaction protocol steps are one pass in `redact`.
const _redactionStep = 'a redaction protocol step; redact is the composite';

/// Reason: not yet moved onto the base class.
const _moves = 'moves onto the base class in T5d, T7 or the API sweep';

/// Reason: the doc's sync facts are `Pdf.info`.
const _docFact = 'a sync fact of the open doc; Pdf.info answers it';

/// Reason: lifecycle of an open handle.
const _lifecycle = 'closes the handle; a one-shot closes its own';

/// Excluded members by class, then member name, to the reason.
const oneShotExclusions = <String, Map<String, String>>{
  'PdfEditor': {
    'pageCount': _editorRead,
    'version': _editorRead,
    'isModified': _editorRead,
    'title': _editorRead,
    'setTitle': _moves,
    'author': _editorRead,
    'setAuthor': _moves,
    'subject': _editorRead,
    'setSubject': _moves,
    'keywords': _editorRead,
    'setKeywords': _moves,
    'producer': _editorRead,
    'setProducer': _moves,
    'creationDate': _editorRead,
    'setCreationDate': _moves,
    'rotateAllPages': _moves,
    'pageMediaBox': _editorRead,
    'pageCropBox': _editorRead,
    'setPageMediaBox': _moves,
    'setPageCropBox': _moves,
    'setPageRotation': _moves,
    'deletePage': _moves,
    'movePage': _moves,
    'selectPages': _moves,
    'mergeFrom': _merge,
    'reduceImages': _moves,
    'unembedStandardFonts': _moves,
    'addStamp': _moves,
    'addImageStamp': _moves,
    'embedFile': _moves,
    'eraseRegions': _moves,
    'clearEraseRegions': _moves,
    'flattenForms': _moves,
    'flattenAnnotations': _moves,
    'setFormFieldValue': _moves,
    'setCheckboxFieldValue': _moves,
    'cropMargins': _moves,
    'pageImages': _editorRead,
    'resizeImage': _moves,
    'repositionImage': _moves,
    'setImageBounds': _moves,
    'removeFormField': _moves,
    'setFormFieldReadOnly': _moves,
    'setFormFieldRequired': _moves,
    'setFormFieldTooltip': _moves,
    'setFormFieldBounds': _moves,
    'setFormFieldMaxLength': _moves,
    'setFormFieldAlignment': _moves,
    'setFormFieldBackgroundColor': _moves,
    'setFormFieldBorderColor': _moves,
    'setFormFieldBorderWidth': _moves,
    'setFormFieldAppearance': _moves,
    'setFormFieldFlags': _moves,
    'convertToPdfA': _moves,
    'addRedaction': _redactionStep,
    'redactionCount': _editorRead,
    'applyRedactions': _redactionStep,
    'scrubMetadata': _moves,
    'sanitize': _moves,
    'save': _save,
    'dispose': _lifecycle,
  },
  'PdfDoc': {
    'pageCount': _docFact,
    'version': _docFact,
    'pages': _moves,
    'title': _docFact,
    'author': _docFact,
    'subject': _docFact,
    'keywords': _docFact,
    'producer': _docFact,
    'creator': _docFact,
    'creationDate': _docFact,
    'isEncrypted': _docFact,
    'requiresPassword': _docFact,
    'isTagged': _docFact,
    'encryptionAlgorithm': _docFact,
    'permissions': _docFact,
    'extract': _moves,
    'signatures': _moves,
    'verifySignatures': _moves,
    'validatePdfA': _moves,
    'validatePdfUa': _moves,
    'planSplitByBookmarks': _moves,
    'classifyPage': _moves,
    'classifyDocument': _moves,
    'formFields': _moves,
    'formField': _moves,
    'exportFormData': _moves,
    'xfa': _moves,
    'attachments': _moves,
    'annotations': _moves,
    'extractAttachment': _moves,
    'dispose': _lifecycle,
  },
};
