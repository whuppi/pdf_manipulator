// Members of PdfEditor and PdfDoc that are not on PdfEdits / PdfReads, each
// with the reason it has no one-shot. The membership test fails on a stale
// entry.

/// Reason: an editor read is not an edit, so the one-shot of edits has no use
/// for it (`Pdf.read` and `PdfDoc` answer it).
const _editorRead = 'a read on the editor, not an edit';

/// Reason: the door saves.
const _save = 'the one-shot saves its own result';

/// Reason: `merge` is its one-shot.
const _merge = 'Pdf.merge is its one-shot';

/// Reason: the redaction protocol steps are one pass in `redact`.
const _redactionStep = 'a redaction protocol step; redact is the composite';

/// Reason: the doc's sync facts are `Pdf.info`.
const _docFact = 'a sync fact of the open doc; Pdf.info answers it';

/// Reason: lifecycle of an open handle.
const _lifecycle = 'closes the handle; a one-shot closes its own';

/// Reason: `field` returns the edits of one field on the open editor, and a
/// one-shot object runs a single call.
const _field =
    'the edits of one field on the open editor; a one-shot runs one call';

/// Excluded members by class, then member name, to the reason.
const oneShotExclusions = <String, Map<String, String>>{
  'PdfEditor': {
    'pageCount': _editorRead,
    'version': _editorRead,
    'isModified': _editorRead,
    'metadata': _editorRead,
    'pageInfo': _editorRead,
    'pageImages': _editorRead,
    'mergeFrom': _merge,
    'field': _field,
    'markRedaction': _redactionStep,
    'clearRedactions': _redactionStep,
    'redactionCount': _editorRead,
    'applyRedactions': _redactionStep,
    'save': _save,
    'dispose': _lifecycle,
  },
  'PdfDoc': {
    'pageCount': _docFact,
    'version': _docFact,
    'isTagged': _docFact,
    'metadata': _docFact,
    'encryption': _docFact,
    'dispose': _lifecycle,
  },
};
