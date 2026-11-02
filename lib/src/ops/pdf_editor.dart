// PdfEditor — mutations only. Parse once, mutate many, save once.
//
// Every method here modifies the document. Read/export ops that don't
// modify belong on PdfDoc (queries) or PdfStandalone (one-shot export).
// Even if the Rust engine implements a non-mutating op on DocumentEditor,
// it must NOT be exposed here — wrap it as standalone or doc query instead.

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

  // ── Optimization ──

  /// Re-encodes and downsamples every image drawn on the document's pages
  /// under [policy] — `PdfImagePolicy.screen`, `.ebook`, `.print`,
  /// `.lossless`, or your own numbers — and reports one row per image.
  ///
  /// Placement never changes: [pageImages] reads the same names, bounds
  /// and transforms afterwards. A replacement is written only when it is
  /// smaller than the original; soft masks are resampled with their
  /// image; JPEG 2000, JBIG2, spot-colour and colour-key-masked images
  /// are reported as kept. Only images drawn by page content (directly or
  /// through Form XObjects) are visited: an image referenced solely from an
  /// annotation appearance stream or a pattern, and inline `BI … EI`
  /// images, are left as stored and do not appear in the report.
  PdfTask<PdfImageReport> reduceImages(PdfImagePolicy policy) {
    KeepRecord.op('render');
    _check();
    return _handle.reduceImages(policy);
  }

  /// Removes embedded copies of standard PDF fonts to reduce file size.
  PdfTask<int> unembedStandardFonts() {
    _check();
    return _handle.unembedStandardFonts();
  }

  // ── Watermark + stamps ──

  /// Adds a predefined stamp annotation to the specified [page].
  PdfTask<void> addStamp(
    int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addStamp(page, type: type, rect: rect, opacity: opacity);
  }

  /// Adds a custom image stamp to the specified [page].
  PdfTask<void> addImageStamp(
    int page,
    DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  }) {
    _check();
    return _handle.addImageStamp(page, imageData, rect: rect, opacity: opacity);
  }

  // ── Content ──

  /// Embeds [data] as an attached file with the given [name].
  ///
  /// [description] and [mimeType] go into the file spec, where
  /// `PdfDoc.attachments` reads them back; [relationship] states how the
  /// file relates to this document.
  PdfTask<void> embedFile(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  }) {
    _check();
    return _handle.embedFile(
      name,
      data,
      description: description,
      mimeType: mimeType,
      relationship: relationship,
    );
  }

  /// Erases content within the specified [regions] on [page].
  PdfTask<void> eraseRegions(int page, List<PdfRect> regions) {
    _check();
    return _handle.eraseRegions(page, regions);
  }

  /// Clears the destructive-erase regions queued on [page] by
  /// [eraseRegions] without applying them.
  PdfTask<void> clearEraseRegions(int page) {
    _check();
    return _handle.clearEraseRegions(page);
  }

  /// Flattens interactive form fields into static content. With no
  /// [page], every page is flattened; with [page], only that page's
  /// widgets are, leaving the rest of the form fillable.
  PdfTask<void> flattenForms({int? page}) {
    _check();
    return _handle.flattenForms(page: page);
  }

  /// Flattens annotations (forms, comments, stamps) into static content.
  /// With no [page], every page is flattened; with [page], only that
  /// page's annotations are.
  PdfTask<void> flattenAnnotations({int? page}) {
    _check();
    return _handle.flattenAnnotations(page: page);
  }

  /// Sets the value of the form field identified by [fieldName].
  ///
  /// On a button field (checkbox or radio) the value is an appearance-state
  /// name — `'Yes'`, `'On'`, or whichever name that widget uses — not text to
  /// draw. Pass `'Off'` to clear it. Use [setCheckboxFieldValue] for a
  /// checkbox whose state name you do not know.
  PdfTask<void> setFormFieldValue(String fieldName, String value) {
    _check();
    return _handle.setFormFieldValue(fieldName, value);
  }

  /// Checks or clears the checkbox field identified by [fieldName].
  ///
  /// Selects whichever on-state the widget offers, so this works on a box
  /// named `/On` or `/1` as well as the common `/Yes`.
  PdfTask<void> setCheckboxFieldValue(String fieldName, bool checked) {
    _check();
    return _handle.setCheckboxFieldValue(fieldName, checked);
  }

  /// Crops all pages by the specified margins (in points).
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  }) {
    _check();
    return _handle.cropMargins(
      left: left,
      right: right,
      top: top,
      bottom: bottom,
    );
  }

  /// Lists the images placed on [page] (0-based): each one's resource
  /// [PdfPageImage.name], its placement [PdfPageImage.bounds] in points
  /// and the full [PdfPageImage.transform]. The name is what [setImageBounds]
  /// takes. A page with no images returns an empty list; a page index out
  /// of range throws [PdfEngineError].
  PdfTask<List<PdfPageImage>> pageImages(int page) {
    _check();
    return _handle.pageImages(page);
  }

  /// Moves and resizes the named image on [page] to [bounds] in points in
  /// one edit. Names come from [pageImages], whose `bounds` this mirrors.
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds) {
    _check();
    return _handle.setImageBounds(page, imageName, bounds);
  }

  // ── Form field properties ──

  /// Removes the form field [name], and its widget annotation, from the
  /// document.
  PdfTask<void> removeFormField(String name) {
    _check();
    return _handle.removeFormField(name);
  }

  /// Sets whether the form field [name] can be edited by the user.
  PdfTask<void> setFormFieldReadOnly(String name, bool readOnly) {
    _check();
    return _handle.setFormFieldReadOnly(name, readOnly);
  }

  /// Sets whether the form field [name] must have a value on submit.
  PdfTask<void> setFormFieldRequired(String name, bool required) {
    _check();
    return _handle.setFormFieldRequired(name, required);
  }

  /// Sets the form field [name]'s tooltip, shown when a viewer hovers it.
  PdfTask<void> setFormFieldTooltip(String name, String tooltip) {
    _check();
    return _handle.setFormFieldTooltip(name, tooltip);
  }

  /// Sets the bounding rectangle of the form field [name]'s first widget.
  PdfTask<void> setFormFieldBounds(String name, PdfRect bounds) {
    _check();
    return _handle.setFormFieldBounds(name, bounds);
  }

  /// Sets the maximum number of characters the text field [name] accepts.
  PdfTask<void> setFormFieldMaxLength(String name, int maxLength) {
    _check();
    return _handle.setFormFieldMaxLength(name, maxLength);
  }

  /// Sets the text field [name]'s content alignment.
  PdfTask<void> setFormFieldAlignment(String name, PdfAlignment alignment) {
    _check();
    return _handle.setFormFieldAlignment(name, alignment);
  }

  /// Sets the form field [name]'s widget background color.
  PdfTask<void> setFormFieldBackgroundColor(String name, PdfColor color) {
    _check();
    return _handle.setFormFieldBackgroundColor(name, color);
  }

  /// Sets the form field [name]'s widget border color.
  PdfTask<void> setFormFieldBorderColor(String name, PdfColor color) {
    _check();
    return _handle.setFormFieldBorderColor(name, color);
  }

  /// Sets the form field [name]'s widget border width, in points.
  PdfTask<void> setFormFieldBorderWidth(String name, double width) {
    _check();
    return _handle.setFormFieldBorderWidth(name, width);
  }

  /// Sets the form field [name]'s default appearance: [font] is a resource
  /// name such as `Helv`, [fontSize] in points, and text [color].
  PdfTask<void> setFormFieldAppearance(
    String name, {
    required String font,
    required double fontSize,
    PdfColor color = PdfColor.black,
  }) {
    _check();
    return _handle.setFormFieldAppearance(
      name,
      font: font,
      fontSize: fontSize,
      color: color,
    );
  }

  /// Sets the form field [name]'s `/Ff` bits to the OR of [flags].
  PdfTask<void> setFormFieldFlags(String name, Set<PdfFormFieldFlag> flags) {
    _check();
    return _handle.setFormFieldFlags(name, flags);
  }

  /// Converts the document to PDF/A at the given conformance [level].
  PdfTask<void> convertToPdfA({PdfALevel level = PdfALevel.a2}) {
    KeepRecord.op('pdfa');
    _check();
    return _handle.convertToPdfA(level: level);
  }

  // ── Redaction ──

  /// Marks a [region] on [page] for redaction (applied with [applyRedactions]).
  PdfTask<void> addRedaction(int page, PdfRect region, {String? overlayText}) {
    KeepRecord.op('render');
    _check();
    return _handle.addRedaction(page, region, overlayText: overlayText);
  }

  /// Returns the number of pending redaction marks on [page].
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

  /// Removes all document metadata (title, author, timestamps, etc.).
  /// Equivalent to `sanitize(const PdfSanitizeOptions(javascript: false))`.
  PdfTask<void> scrubMetadata() {
    _check();
    return _handle.scrubMetadata();
  }

  /// Strips metadata, JavaScript and/or embedded files per [options].
  PdfTask<void> sanitize(PdfSanitizeOptions options) {
    _check();
    return _handle.sanitize(options);
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
