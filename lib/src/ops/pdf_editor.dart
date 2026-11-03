// PdfEditor — the open editor handle. Parse once, mutate many, save once.
//
// Every edit is declared once on PdfEdits. What is here is what an edit
// object of one call cannot offer: reads of the edited document, the queued
// redaction protocol, the per-field editor and the save.

part of 'pdf.dart';

/// Mutable PDF editor — parse once, mutate many times, save once.
base class PdfEditor extends PdfEdits implements _Handle {
  /// Internal constructor — use [Pdf.edit] to create instances.
  @internal
  PdfEditor.internal(PdfBridge _, BridgeEditorHandle handle)
    : _handle = handle,
      super._() {
    HandleBackstop.attach(this, 'PdfEditor', handle.dispose);
  }

  final BridgeEditorHandle _handle;
  bool _disposed = false;

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

  @override
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeEditorHandle handle) apply) {
    _check();
    return apply(_handle);
  }

  void _check() {
    if (_disposed) {
      throw const PdfDisposed('This PdfEditor has been disposed');
    }
  }

  // ── Properties ──

  /// Current number of pages in the document.
  PdfTask<int> pageCount() {
    _check();
    return _handle.pageCount();
  }

  /// PDF version string of the document.
  PdfTask<String> version() {
    _check();
    return _handle.version();
  }

  /// Whether unsaved modifications exist.
  PdfTask<bool> isModified() {
    _check();
    return _handle.isModified();
  }

  // ── Metadata ──

  /// Returns the document's standard info fields as the edited document
  /// has them now; a field it does not carry is null.
  PdfTask<PdfMetadata> metadata() {
    _check();
    return _handle.metadata();
  }

  // ── Pages ──

  /// Returns the facts of [pages] (every page when null) as the edited
  /// document has them now: boxes, rotation and label. A label is the
  /// catalog's page-label entry for the page's position in the edited
  /// document.
  PdfTask<List<PdfPageInfo>> pageInfo({PdfPages? pages}) {
    _check();
    return _handle.pageInfo(pages: pages);
  }

  /// Appends pages from [source] at the end of this document: every page,
  /// or only [pages] (0-based, in the order given). [password] opens an
  /// encrypted [source]; a missing or wrong one throws the typed password
  /// error. An index past the other document's last page throws
  /// [PdfEngineError].
  PdfTask<void> mergeFrom(
    DataSource source, {
    PdfPages? pages,
    String? password,
  }) {
    _check();
    return _handle.mergeFrom(source, pages: pages, password: password);
  }

  /// Lists the images placed on [page] (0-based): each one's resource
  /// [PdfPageImage.name], its placement [PdfPageImage.bounds] in points
  /// and the full [PdfPageImage.transform]. The name is what
  /// [PdfEdits.setImageBounds] takes. A page with no images returns an empty
  /// list; a page index out of range throws [PdfEngineError].
  PdfTask<List<PdfPageImage>> pageImages(int page) {
    _check();
    return _handle.pageImages(page);
  }

  // ── Forms ──

  /// The edits of the form field [name], its fully qualified (dotted) name.
  /// Nothing is looked up until an edit runs: a name the document has no
  /// field for fails that edit with [PdfInvalidArgument].
  PdfFieldEditor field(String name) =>
      PdfFieldEditor._((editor: this, name: name));

  // ── Redaction ──

  /// Marks [region] on [page] for redaction: nothing changes until
  /// [applyRedactions]. [overlay] is what is drawn over the region then, a
  /// box by default; `PdfRedactionOverlay.none()` draws nothing, so the mark
  /// erases. The engine draws no text over a redaction.
  PdfTask<void> markRedaction(
    int page,
    PdfRect region, {
    PdfRedactionOverlay overlay = const PdfRedactionOverlay.fill(
      PdfColor.black,
    ),
  }) {
    KeepRecord.op('render');
    _check();
    return _handle.markRedaction(page, region, overlay);
  }

  /// Drops every mark queued by [markRedaction], on every page, without
  /// applying it. A `/Redact` annotation the document carries is not a queued
  /// mark and stays.
  PdfTask<void> clearRedactions() {
    _check();
    return _handle.clearRedactions();
  }

  /// Returns the number of pending redaction marks on [page]: the queued
  /// ones and the `/Redact` annotations the document carries.
  PdfTask<int> redactionCount(int page) {
    _check();
    return _handle.redactionCount(page);
  }

  /// Permanently applies all pending redaction marks, removing content,
  /// and reports what was removed.
  PdfTask<PdfRedactionReport> applyRedactions() {
    KeepRecord.op('render');
    _check();
    return _handle.applyRedactions();
  }

  // ── Save ──

  /// Writes the modified document to [output] with the given save [options].
  PdfTask<void> save(
    DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  }) {
    _check();
    return _handle.save(output, options: options);
  }

  // ── Lifecycle ──

  /// Releases editor resources and the underlying doc handle. Idempotent.
  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    HandleBackstop.detach(this);
    await _handle.dispose();
  }
}

/// The edits of one form field, from `editor.field(name)`. Each edit is one
/// engine call on the open editor and is saved with it.
extension type PdfFieldEditor._(({PdfEditor editor, String name}) _field) {
  BridgeEditorHandle get _handle {
    _field.editor._check();
    return _field.editor._handle;
  }

  String get _name => _field.name;

  /// Sets the field's value. [PdfFormFieldValue.text] fills a text field, or
  /// names the state of a checkbox or radio group; [PdfFormFieldValue.checked]
  /// checks or clears a checkbox; [PdfFormFieldValue.choice] selects one
  /// option of a combo box, list box or radio group by its export value,
  /// and [PdfFormFieldValue.choices] the options of a multi-select list box;
  /// [PdfFormFieldValue.none] clears the value. A choice the field does not
  /// offer, and a value of a kind the field cannot hold, throw
  /// [PdfInvalidArgument] and change nothing. The widget is drawn from the
  /// new value.
  PdfTask<void> setValue(PdfFormFieldValue value) => switch (value) {
    PdfTextValue(:final text) => _handle.setFormFieldValue(_name, text),
    PdfCheckedValue(:final checked) => _handle.setCheckboxFieldValue(
      _name,
      checked,
    ),
    PdfChoiceValue(:final choice) => _handle.setFormFieldChoice(_name, choice),
    PdfMultiChoiceValue(:final choices) => _handle.setFormFieldChoices(
      _name,
      choices,
    ),
    PdfNoValue() => _handle.clearFormFieldValue(_name),
  };

  /// Sets whether the user can edit the field.
  PdfTask<void> setReadOnly(bool readOnly) =>
      _handle.setFormFieldReadOnly(_name, readOnly);

  /// Sets whether the field must have a value on submit.
  PdfTask<void> setRequired(bool required) =>
      _handle.setFormFieldRequired(_name, required);

  /// Sets the tooltip a viewer shows when it hovers the field; null removes
  /// it.
  PdfTask<void> setTooltip(String? tooltip) =>
      _handle.setFormFieldTooltip(_name, tooltip);

  /// Sets the bounding rectangle of the field's first widget.
  PdfTask<void> setBounds(PdfRect bounds) =>
      _handle.setFormFieldBounds(_name, bounds);

  /// Sets the maximum number of characters the text field accepts; null
  /// removes the limit.
  PdfTask<void> setMaxLength(int? maxLength) =>
      _handle.setFormFieldMaxLength(_name, maxLength);

  /// Sets the alignment of the text field's content.
  PdfTask<void> setAlignment(PdfAlignment alignment) =>
      _handle.setFormFieldAlignment(_name, alignment);

  /// Sets the widget's background color; null removes it.
  PdfTask<void> setBackgroundColor(PdfColor? color) =>
      _handle.setFormFieldBackgroundColor(_name, color);

  /// Sets the widget's border color; null removes it.
  PdfTask<void> setBorderColor(PdfColor? color) =>
      _handle.setFormFieldBorderColor(_name, color);

  /// Sets the widget's border width, in points.
  PdfTask<void> setBorderWidth(double width) =>
      _handle.setFormFieldBorderWidth(_name, width);

  /// Sets the field's default appearance: [font] is a resource name such as
  /// `Helv`, [fontSize] in points, and text [color].
  PdfTask<void> setAppearance({
    required String font,
    required double fontSize,
    PdfColor color = PdfColor.black,
  }) => _handle.setFormFieldAppearance(
    _name,
    font: font,
    fontSize: fontSize,
    color: color,
  );

  /// Sets the field's `/Ff` bits to the OR of [flags].
  PdfTask<void> setFlags(Set<PdfFormFieldFlag> flags) =>
      _handle.setFormFieldFlags(_name, flags);

  /// Removes the field and its widget from the document.
  PdfTask<void> remove() => _handle.removeFormField(_name);
}
