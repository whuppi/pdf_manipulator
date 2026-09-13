// Abstract bridge — SharedBridge implements this.
// Only FFI methods that go to the engine. No one-shot sugar.
// No algorithms. No helpers. Just the wire.
// INTERNAL — not exported from the package.

import 'dart:typed_data';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_attachment.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_form_field.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_image.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_page_image.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_sanitize_options.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/search_result.dart';

/// Abstract bridge. Only FFI-level methods.
abstract class PdfBridge {
  /// Detected I/O mode. Null before first op or [ensureInitialized].
  PdfIoMode? get ioMode;

  /// Eagerly initialize and return the detected I/O mode. Idempotent.
  Future<PdfIoMode> ensureInitialized();

  // ── Document handle ──

  /// Opens a PDF document for read-only queries.
  PdfTask<BridgeDocHandle> open(DataSource source, {String? password});

  // ── One-shot ops (no handle — source consumed in one call) ──

  /// Digitally signs a PDF document.
  PdfTask<void> sign(
    DataSource source,
    DataSink output, {
    required PdfSigningCredentials credentials,
    String? reason,
    String? location,
  });

  /// Converts a PDF to another document format.
  PdfTask<void> convertTo(
    DataSource source,
    DataSink output, {
    required PdfDocumentFormat format,
    String? password,
  });

  /// Converts an XFA document into a plain AcroForm document.
  PdfTask<void> convertXfaToAcroForm(
    DataSource source,
    DataSink output, {
    String? password,
  });

  /// Converts a document to PDF format.
  PdfTask<void> convertToPdf(
    DataSource document,
    DataSink output, {
    required PdfDocumentFormat format,
  });

  /// Register a runtime fallback font for form-value baking.
  /// [kind] is the engine wire name ('cjk' or 'emoji').
  PdfTask<void> registerFallbackFont(String kind, Uint8List bytes);

  // ── Editor handle ──

  /// Opens a PDF for editing, returning a mutable handle.
  PdfTask<BridgeEditorHandle> openEditor(DataSource source, {String? password});

  // ── Builder handle ──

  /// Creates a new empty PDF builder session.
  PdfTask<BridgeBuilderHandle> createBuilder();

  // ── Lifecycle ──

  /// Releases all transport resources.
  Future<void> dispose();
}

/// Handle to an open PDF document — read-only queries.
abstract class BridgeDocHandle {
  // ── Metadata (populated at open time, cached) ──

  /// Raw key-value pairs returned by the engine at open time.
  Map<String, Object?> get openResult;

  // ── Read ops (reuse already-parsed document via handleId) ──

  /// Extracts text content from the specified [pages].
  PdfTask<String> extract({
    required PdfPages pages,
    PdfExtractionFormat format,
  });

  /// Searches for [query] across the specified [pages].
  PdfTask<List<SearchResult>> search({
    required String query,
    required PdfPages pages,
  });

  /// Renders the specified [pages] as pixel buffers.
  Stream<RenderedPage> render({required PdfPages pages, PdfRenderSize? size});

  /// Extracts embedded images from the specified [pages].
  Stream<PdfImage> extractImages({required PdfPages pages});

  /// Returns signature info for all signatures in the document.
  PdfTask<List<PdfSignatureInfo>> get signatures;

  /// Verifies all digital signatures, returning true if all valid.
  PdfTask<bool> verifySignatures();

  /// Validates the document against PDF/A conformance.
  PdfTask<PdfValidationResult> validatePdfA({int level});

  /// Validates the document against PDF/UA accessibility.
  PdfTask<bool> validatePdfUa({int level});

  /// Plans split points based on top-level bookmarks.
  PdfTask<List<PdfBookmarkSplit>> planSplitByBookmarks();

  /// Classifies a single page by content type.
  PdfTask<PdfPageClassification> classifyPage(int page);

  /// Classifies the entire document by content type.
  PdfTask<PdfDocumentClassification> classifyDocument();

  /// Reads every AcroForm field of the document.
  PdfTask<List<PdfFormField>> get formFields;

  /// Exports AcroForm field values as FDF or XFDF to [output].
  PdfTask<void> exportFormData(DataSink output, {required String format});

  /// Reads the document's XFA structure, or null when it has none.
  PdfTask<PdfXfaInfo?> get xfa;

  /// Lists the files embedded in the document.
  PdfTask<List<PdfAttachment>> get attachments;

  /// Streams the embedded file named [name] to [output].
  PdfTask<void> extractAttachment(String name, DataSink output);

  // ── Lifecycle ──

  /// Releases the document handle and any held resources.
  Future<void> dispose();
}

/// Handle to an open PDF editor session.
abstract class BridgeEditorHandle {
  /// Number of pages in the document.
  PdfTask<int> get pageCount;

  /// PDF version string.
  PdfTask<String> get version;

  /// Whether the document has unsaved modifications.
  PdfTask<bool> get isModified;

  // ── Metadata ──

  /// Returns the document title.
  PdfTask<String> get title;

  /// Sets the document title.
  PdfTask<void> setTitle(String value);

  /// Returns the document author.
  PdfTask<String> get author;

  /// Sets the document author.
  PdfTask<void> setAuthor(String value);

  /// Returns the document subject.
  PdfTask<String> get subject;

  /// Sets the document subject.
  PdfTask<void> setSubject(String value);

  /// Returns the document keywords.
  PdfTask<String> get keywords;

  /// Sets the document keywords.
  PdfTask<void> setKeywords(String value);

  /// Returns the document producer.
  PdfTask<String> get producer;

  /// Sets the document producer.
  PdfTask<void> setProducer(String value);

  /// Returns the document creation date (raw PDF date string).
  PdfTask<String> get creationDate;

  /// Sets the document creation date (raw PDF date string).
  PdfTask<void> setCreationDate(String value);

  // ── Pages ──

  /// Rotates a single [page] by [degrees] (90, 180, 270).
  PdfTask<void> rotatePage(int page, {required int degrees});

  /// Rotates all pages by [degrees].
  PdfTask<void> rotateAllPages({required int degrees});

  /// Returns the media box rectangle for [page].
  PdfTask<PdfRect> pageMediaBox(int page);

  /// Returns the crop box rectangle for [page], or `null` when unset.
  PdfTask<PdfRect?> pageCropBox(int page);

  /// Sets the media box rectangle for [page].
  PdfTask<void> setPageMediaBox(int page, PdfRect box);

  /// Sets the crop box rectangle for [page].
  PdfTask<void> setPageCropBox(int page, PdfRect box);

  /// Sets the absolute rotation of [page] in degrees.
  PdfTask<void> setPageRotation(int page, {required int degrees});

  /// Deletes a single [page].
  PdfTask<void> deletePage(int page);

  /// Moves a page from index [from] to index [to].
  PdfTask<void> movePage({required int from, required int to});

  /// Retains only the specified [pages], removing all others.
  PdfTask<void> selectPages(List<int> pages);

  /// Appends pages from [otherPdf] into this document; [pages] selects
  /// which ones, in the order given.
  PdfTask<void> mergeFrom(DataSource otherPdf, {List<int>? pages});

  // ── Optimization ──

  /// Re-encodes and downsamples images under [policy]; one row per image.
  PdfTask<PdfImageReport> reduceImages(PdfImagePolicy policy);

  /// Removes embedded standard fonts, returning count unembedded.
  PdfTask<int> unembedStandardFonts();

  // ── Watermark + stamps ──

  /// Adds a text watermark to [page].
  PdfTask<void> addWatermark(
    int page,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  });

  /// Adds a predefined stamp to [page].
  PdfTask<void> addStamp(
    int page, {
    required PdfStampType type,
    required PdfRect rect,
    double opacity = 1.0,
  });

  /// Adds an image stamp to [page] from [imageData].
  PdfTask<void> addImageStamp(
    int page,
    DataSource imageData, {
    required PdfRect rect,
    double opacity = 1.0,
  });

  // ── Content ──

  /// Embeds a file attachment with the given [name].
  PdfTask<void> embedFile(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  });

  /// Erases content within [regions] on [page].
  PdfTask<void> eraseRegions(int page, List<PdfRect> regions);

  /// Flattens interactive form fields into static content. `page == null`
  /// flattens every page; otherwise only that page's widgets.
  PdfTask<void> flattenForms({int? page});

  /// Flattens annotations into page content. `page == null` flattens every
  /// page; otherwise only that page's annotations.
  PdfTask<void> flattenAnnotations({int? page});

  /// Clears the queued destructive-erase regions on [page] without
  /// applying them.
  PdfTask<void> clearEraseRegions(int page);

  /// Sets a form field's value by [fieldName].
  PdfTask<void> setFormFieldValue(String fieldName, String value);

  /// Checks or clears the checkbox field identified by [fieldName].
  PdfTask<void> setCheckboxFieldValue(String fieldName, bool checked);

  /// Crops margins from all pages.
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  });

  /// Converts the document to PDF/A conformance at [level].
  PdfTask<void> convertToPdfA({int level = 1});

  /// Lists the image XObjects placed on [page].
  PdfTask<List<PdfPageImage>> pageImages(int page);

  /// Resizes an embedded image on [page] by [imageName].
  PdfTask<void> resizeImage(
    int page,
    String imageName, {
    required double width,
    required double height,
  });

  /// Moves an embedded image on [page] by [imageName] to ([x], [y]).
  PdfTask<void> repositionImage(
    int page,
    String imageName, {
    required double x,
    required double y,
  });

  /// Moves and resizes an embedded image on [page] by [imageName].
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds);

  // ── Form field properties ──

  /// Removes the form field [name] and its widget.
  PdfTask<void> removeFormField(String name);

  /// Sets the form field [name]'s read-only flag.
  PdfTask<void> setFormFieldReadOnly(String name, bool readOnly);

  /// Sets the form field [name]'s required flag.
  PdfTask<void> setFormFieldRequired(String name, bool required);

  /// Sets the form field [name]'s tooltip.
  PdfTask<void> setFormFieldTooltip(String name, String tooltip);

  /// Sets the form field [name]'s bounding rectangle.
  PdfTask<void> setFormFieldBounds(String name, PdfRect bounds);

  /// Sets the form field [name]'s maximum text length.
  PdfTask<void> setFormFieldMaxLength(String name, int maxLength);

  /// Sets the form field [name]'s text alignment.
  PdfTask<void> setFormFieldAlignment(String name, PdfTextAlignment alignment);

  /// Sets the form field [name]'s background color.
  PdfTask<void> setFormFieldBackgroundColor(String name, PdfColor color);

  /// Sets the form field [name]'s border color.
  PdfTask<void> setFormFieldBorderColor(String name, PdfColor color);

  /// Sets the form field [name]'s border width.
  PdfTask<void> setFormFieldBorderWidth(String name, double width);

  /// Sets the form field [name]'s default appearance.
  PdfTask<void> setFormFieldAppearance(
    String name, {
    required String font,
    required double fontSize,
    PdfColor color = PdfColor.black,
  });

  /// Sets the form field [name]'s flag bits.
  PdfTask<void> setFormFieldFlags(String name, Set<PdfFormFieldFlag> flags);

  // ── Redaction ──

  /// Marks a [region] on [page] for redaction.
  PdfTask<void> addRedaction(int page, PdfRect region, {String? overlayText});

  /// Returns the number of pending redaction marks on [page].
  PdfTask<int> redactionCount(int page);

  /// Permanently applies all pending redactions, removing content, and
  /// reports what was removed.
  PdfTask<PdfRedactionReport> applyRedactions();

  /// Removes all metadata from the document.
  PdfTask<void> scrubMetadata();

  /// Strips metadata, JavaScript and/or embedded files per [options].
  PdfTask<void> sanitize(PdfSanitizeOptions options);

  // ── Save ──

  /// Saves the edited document to [output].
  PdfTask<void> save(
    DataSink output, {
    PdfSaveOptions options = const PdfSaveOptions.fullRewrite(),
  });

  // ── Extract pages (select → save → restore, editor unchanged) ──

  /// Extracts [pages] into [output] without modifying the editor.
  PdfTask<void> extractPages(List<int> pages, DataSink output);

  // ── Lifecycle ──

  /// Releases the editor handle and any held resources.
  Future<void> dispose();
}

/// Handle to a PDF builder session.
abstract class BridgeBuilderHandle {
  /// Sets the document title.
  PdfTask<void> setTitle(String value);

  /// Sets the document author.
  PdfTask<void> setAuthor(String value);

  /// Sets the document subject.
  PdfTask<void> setSubject(String value);

  /// Sets the document keywords.
  PdfTask<void> setKeywords(String value);

  /// Adds a new A4-sized page and returns a page builder.
  PdfTask<BridgePageBuilderHandle> addA4Page();

  /// Adds a new US Letter-sized page and returns a page builder.
  PdfTask<BridgePageBuilderHandle> addLetterPage();

  /// Adds a page with custom [width] and [height] in points.
  PdfTask<BridgePageBuilderHandle> addPage({
    required double width,
    required double height,
  });

  /// Saves the built document to [output].
  PdfTask<void> save(DataSink output);

  /// Releases the builder handle.
  Future<void> dispose();
}

/// Handle to a single page being built.
abstract class BridgePageBuilderHandle {
  /// Sets the current font by [name] and [size].
  PdfTask<void> font(String name, double size);

  /// Moves the cursor to ([x], [y]).
  PdfTask<void> at(double x, double y);

  /// Writes inline [text] at the current position.
  PdfTask<void> text(String text);

  /// Writes a heading of [level] with [text].
  PdfTask<void> heading(int level, String text);

  /// Writes a wrapped paragraph of [text].
  PdfTask<void> paragraph(String text);

  /// Inserts vertical space of [points].
  PdfTask<void> space(double points);

  /// Draws a horizontal rule across the page.
  PdfTask<void> horizontalRule();

  /// Places an image from [imageData] within [rect].
  PdfTask<void> image(
    DataSource imageData,
    PdfRect rect, {
    String altText = '',
  });

  /// Adds a diagonal watermark [text] across the page.
  PdfTask<void> watermark(String text);

  /// Creates a text input field with [name] at [rect].
  PdfTask<void> textField(String name, PdfRect rect, {String? defaultValue});

  /// Creates a checkbox with [name] at [rect].
  PdfTask<void> checkbox(String name, PdfRect rect, {bool checked = false});

  /// Creates a combo box with [name] at [rect].
  PdfTask<void> comboBox(
    String name,
    PdfRect rect,
    List<String> options, {
    String? selected,
  });

  /// Creates a push button with [name] at [rect].
  PdfTask<void> pushButton(String name, PdfRect rect, String caption);

  /// Creates a digital signature field with [name] at [rect].
  PdfTask<void> signatureField(String name, PdfRect rect);

  /// Creates a radio button group with [name].
  PdfTask<void> radioGroup(
    String name,
    List<({String value, PdfRect rect})> options, {
    String? selected,
  });

  /// Attaches a keystroke JavaScript action to the most-recently-
  /// added field. A page with no field yet ignores the call.
  PdfTask<void> fieldKeystroke(String script);

  /// Attaches a format JavaScript action to the most-recently-
  /// added field. A page with no field yet ignores the call.
  PdfTask<void> fieldFormat(String script);

  /// Attaches a validate JavaScript action to the most-recently-
  /// added field. A page with no field yet ignores the call.
  PdfTask<void> fieldValidate(String script);

  /// Attaches a calculate JavaScript action to the most-recently-
  /// added field. A page with no field yet ignores the call.
  PdfTask<void> fieldCalculate(String script);

  /// Creates a hyperlink to an external [url], anchored on the
  /// last-written text. A page with no text yet ignores the call.
  PdfTask<void> linkUrl(String url);

  /// Creates a link to [targetPage], anchored on the last-written
  /// text. A page with no text yet ignores the call.
  PdfTask<void> linkPage(int targetPage);

  /// Adds a footnote with [refMark] and [noteText].
  PdfTask<void> footnote(String refMark, String noteText);

  /// Lays out [text] across [columnCount] columns with [gapPt] gap.
  PdfTask<void> columns(int columnCount, double gapPt, String text);

  /// Inserts a line break.
  PdfTask<void> newline();

  /// Starts a new page with the same dimensions.
  PdfTask<void> newPageSameSize();

  /// Finalises the page — no further operations allowed.
  PdfTask<void> done();
}
