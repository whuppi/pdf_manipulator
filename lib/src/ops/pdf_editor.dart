// PdfEditor — the editor a scope body gets, and PdfEditorHandle, the one
// `Pdf.edit` returns. Parse once, mutate many, save once.
//
// Every edit is declared once on PdfEdits. What is here is what an edit
// object of one call cannot offer: reads of the edited document, the queued
// redaction protocol and the per-field editor. Only the handle saves and
// disposes.

part of 'pdf.dart';

/// A mutable PDF editor — what a [Pdf.rewrite] body edits: every edit of
/// [PdfEdits] plus reads of the document as the edits have left it. The
/// library saves it when the body returns.
base class PdfEditor extends PdfEdits {
  /// Internal constructor — use [Pdf.edit] to create instances.
  @internal
  PdfEditor.internal(this._pdf, this._handle) : super._();

  final Pdf _pdf;
  final BridgeEditorHandle _handle;
  bool _disposed = false;

  @override
  PdfTask<T> _run<T>(PdfTask<T> Function(BridgeEditorHandle handle) apply) {
    _check();
    return apply(_handle);
  }

  void _check() {
    if (_disposed) throw StateError('This PdfEditor has been disposed');
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

  /// Returns the facts of [pages] (every page by default) as the edited
  /// document has them now: boxes, rotation and label. A label is the
  /// catalog's page-label entry for the page's position in the edited
  /// document.
  PdfTask<List<PdfPageInfo>> pageInfo({PdfPages pages = const PdfPages.all()}) {
    _check();
    return _handle.pageInfo(pages: pages);
  }

  /// Appends pages from [from] at the end of this document: every page, or
  /// only [pages] (0-based; the pages of the selection are appended in
  /// document order). [PdfInput.password] opens an encrypted [from]; a
  /// missing or wrong one throws the typed password error. A page past the
  /// other document's last page fails with [PdfEngineFailed].
  PdfTask<void> appendPages(
    PdfInput from, {
    PdfPages pages = const PdfPages.all(),
  }) {
    _check();
    return PdfTask.group((hook) async {
      final lease = await from.opened;
      try {
        final selection = await _resolveFrom(hook, lease.source, from, pages);
        await hook.guard(
          _handle.appendPages(
            lease.source,
            pages: selection,
            password: from.password,
          ),
        );
      } finally {
        await lease.release();
      }
    });
  }

  /// A selection from a page to the last needs the page count of the
  /// document being appended, which is read by opening it once.
  Future<PdfPages> _resolveFrom(
    CancelHook hook,
    DataSource source,
    PdfInput from,
    PdfPages pages,
  ) async {
    if (pages is! PdfPagesFrom) return pages;
    final count = await hook.guard(
      _own(
        _pdf.open(PdfInput(source, password: from.password)),
        (doc, _) async => doc.pageCount,
      ),
    );
    return pages.start < count
        ? PdfPagesRange(pages.start, count)
        : PdfPagesSingle(pages.start);
  }

  /// Lists the images placed on [page] (0-based): each one's resource
  /// [PdfImagePlacement.name], its placement [PdfImagePlacement.bounds] in points
  /// and the full [PdfImagePlacement.transform]. The name is what
  /// [PdfEdits.setImageBounds] takes. A page with no images returns an empty
  /// list; a page index out of range throws [PdfPageOutOfRange].
  PdfTask<List<PdfImagePlacement>> pageImages(int page) {
    _check();
    return _handle.pageImages(page);
  }

  // ── Forms ──

  /// The edits of the form field [name], its fully qualified (dotted) name.
  /// Nothing is looked up until an edit runs: a name the document has no
  /// field for fails that edit with [PdfEngineFailed].
  PdfFormFieldEditor field(String name) =>
      PdfFormFieldEditor._((editor: this, name: name));

  // ── Redaction ──

  /// Marks each of [regions] on [page] for redaction: nothing changes until
  /// [applyRedactions]. [overlay] is what is drawn over each region then, a
  /// box by default; `PdfRedactionOverlay.none()` draws nothing, so the mark
  /// erases. The engine draws no text over a redaction. An empty [regions]
  /// throws [ArgumentError].
  PdfTask<void> markRedaction(
    int page,
    List<PdfRect> regions, {
    PdfRedactionOverlay overlay = const PdfRedactionOverlay.fill(
      PdfColor.black,
    ),
  }) {
    if (regions.isEmpty) {
      throw ArgumentError.value(regions, 'regions', 'must not be empty');
    }
    KeepRecord.op('render');
    _check();
    return PdfTask.group((hook) async {
      for (final region in regions) {
        await hook.guard(_handle.markRedaction(page, region, overlay));
      }
    });
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
}

/// A [PdfEditor] you own: the handle [Pdf.edit] returns.
///
/// Call [dispose] when you are done, on every path including errors. For one
/// edit pass, use [Pdf.rewrite] (several edits, one save) or [Pdf.editOnce]
/// (one edit), which open, save and always close. A handle you forget is
/// released when the garbage collector frees it, and in debug mode the leak
/// is logged with where it was opened; that is a backstop, not a plan.
final class PdfEditorHandle extends PdfEditor implements _Handle {
  /// Internal constructor — use [Pdf.edit] to create instances.
  @internal
  PdfEditorHandle.internal(
    super.pdf,
    super.handle, {
    required Future<void> Function() release,
  }) : _close = _closing(handle.dispose, release),
       super.internal() {
    HandleBackstop.attach(this, 'PdfEditorHandle', _close);
  }

  final Future<void> Function() _close;

  @override
  void _scopeTo(CancelHook hook) => _handle.scope = hook;

  /// Writes the modified document to [output] with the given [save]
  /// strategy. The sink stays the caller's: it is aborted when the save
  /// fails or is cancelled, and never closed.
  PdfTask<void> save(DataSink output, {PdfSave save = const PdfSave.full()}) {
    _check();
    return _handle.save(output, save: save);
  }

  /// Releases the editor, and the source when the input owns it.
  /// Idempotent.
  @override
  Future<void> dispose() async {
    if (_disposed) return;
    _disposed = true;
    HandleBackstop.detach(this);
    await _close();
  }
}

/// The edits of one form field, from `editor.field(name)`. Each edit is one
/// engine call on the open editor and is saved with it.
extension type PdfFormFieldEditor._(({PdfEditor editor, String name}) _field) {
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
  /// [PdfEngineFailed] and change nothing. The widget is drawn from the
  /// new value.
  PdfTask<void> setValue(PdfFormFieldValue value) => switch (value) {
    PdfTextValue(:final text) => _handle.setFormFieldValue(_name, text),
    PdfCheckedValue(:final isChecked) => _handle.setCheckboxFieldValue(
      _name,
      isChecked,
    ),
    PdfChoiceValue(:final choice) => _handle.setFormFieldChoice(_name, choice),
    PdfMultiChoiceValue(:final choices) => _handle.setFormFieldChoices(
      _name,
      choices,
    ),
    PdfNoValue() => _handle.clearFormFieldValue(_name),
  };

  /// Sets whether the user can edit the field.
  PdfTask<void> setReadOnly(bool isReadOnly) =>
      _handle.setFormFieldReadOnly(_name, isReadOnly);

  /// Sets whether the field must have a value on submit.
  PdfTask<void> setRequired(bool isRequired) =>
      _handle.setFormFieldRequired(_name, isRequired);

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
