// Abstract bridge — SharedBridge implements this.
// Only FFI methods that go to the engine. No one-shot sugar.
// No algorithms. No helpers. Just the wire.
// INTERNAL — not exported from the package.

import 'dart:async';

import 'package:pdf_manipulator/src/types/data_sink.dart';
import 'package:pdf_manipulator/src/types/data_source.dart';
import 'package:pdf_manipulator/src/types/pdf_annotation.dart';
import 'package:pdf_manipulator/src/types/pdf_attachment.dart';
import 'package:pdf_manipulator/src/types/pdf_enums.dart';
import 'package:pdf_manipulator/src/types/pdf_field_scripts.dart';
import 'package:pdf_manipulator/src/types/pdf_link.dart';
import 'package:pdf_manipulator/src/types/pdf_point.dart';
import 'package:pdf_manipulator/src/types/pdf_radio_option.dart';
import 'package:pdf_manipulator/src/types/pdf_form_field.dart';
import 'package:pdf_manipulator/src/types/pdf_image_page.dart';
import 'package:pdf_manipulator/src/types/pdf_metadata.dart';
import 'package:pdf_manipulator/src/types/pdf_page_size.dart';
import 'package:pdf_manipulator/src/types/pdf_pages.dart';
import 'package:pdf_manipulator/src/types/pdf_params.dart';
import 'package:pdf_manipulator/src/types/pdf_image_info.dart';
import 'package:pdf_manipulator/src/types/pdf_image_policy.dart';
import 'package:pdf_manipulator/src/types/pdf_image_report.dart';
import 'package:pdf_manipulator/src/types/pdf_image_placement.dart';
import 'package:pdf_manipulator/src/types/pdf_page_info.dart';
import 'package:pdf_manipulator/src/types/pdf_rect.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_overlay.dart';
import 'package:pdf_manipulator/src/types/pdf_redaction_report.dart';
import 'package:pdf_manipulator/src/types/pdf_render.dart';
import 'package:pdf_manipulator/src/types/pdf_signature.dart';
import 'package:pdf_manipulator/src/types/pdf_stroke.dart';
import 'package:pdf_manipulator/src/types/pdf_table.dart';
import 'package:pdf_manipulator/src/types/pdf_task.dart';
import 'package:pdf_manipulator/src/types/pdf_text_style.dart';
import 'package:pdf_manipulator/src/types/pdf_xfa_info.dart';
import 'package:pdf_manipulator/src/types/pdf_search_hit.dart';

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
  PdfTask<void> convertToOffice(
    DataSource source,
    DataSink output, {
    required PdfOfficeFormat format,
    String? password,
  });

  /// Converts an XFA document into a plain AcroForm document.
  PdfTask<void> convertXfaToAcroForm(
    DataSource source,
    DataSink output, {
    String? password,
  });

  /// Converts a document to PDF format.
  PdfTask<void> convertFromOffice(
    DataSource document,
    DataSink output, {
    required PdfOfficeFormat format,
  });

  /// Register a runtime fallback font for form-value baking.
  /// [kind] is the engine wire name ('cjk' or 'emoji'); [font] is read whole.
  PdfTask<void> registerFallbackFont(String kind, DataSource font);

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
  /// The hook of the combinator that owns this handle, or null for a handle
  /// the caller opened. Every op issued on the handle registers its cancel
  /// with it while it runs; an op issued after the cancel resolves cancelled
  /// without reaching the engine.
  CancelHook? scope;

  // ── Metadata (populated at open time, cached) ──

  /// Raw key-value pairs returned by the engine at open time.
  Map<String, Object?> get openResult;

  // ── Read ops (reuse already-parsed document via handleId) ──

  /// The per-page facts of the pages named; every page when [pages] is all.
  PdfTask<List<PdfPageInfo>> pageInfo({required PdfPages pages});

  /// Writes the text of the pages named to [output]; every page when
  /// [pages] is null.
  PdfTask<void> extractText(
    DataSink output, {
    required PdfPages pages,
    required PdfExtractionFormat format,
  });

  /// Searches for [query] across the pages named; every page when [pages]
  /// is null.
  PdfTask<List<PdfSearchHit>> search({
    required String query,
    required PdfPages pages,
  });

  /// Renders the specified [pages], writing each to the sink [sinkFor]
  /// returns for its descriptor.
  PdfTask<List<PdfRenderedPage>> render(
    FutureOr<DataSink> Function(PdfRenderedPage page) sinkFor, {
    required PdfPages pages,
    required PdfRenderSize size,
    required PdfRenderFormat format,
  });

  /// Extracts the images of the specified [pages], writing each to the sink
  /// [sinkFor] returns for its descriptor.
  PdfTask<List<PdfImageInfo>> extractImages(
    FutureOr<DataSink> Function(PdfImageInfo image) sinkFor, {
    required PdfPages pages,
  });

  /// Returns signature info for all signatures in the document.
  PdfTask<List<PdfSignatureInfo>> signatures();

  /// Verifies all digital signatures, returning true if all valid.
  PdfTask<bool> verifySignatures();

  /// Validates the document against PDF/A conformance.
  PdfTask<PdfValidationReport> validatePdfA({required PdfALevel level});

  /// Validates the document against PDF/UA accessibility.
  PdfTask<PdfValidationReport> validatePdfUa({required PdfUaLevel level});

  /// The page ranges of the document's top-level bookmarks.
  PdfTask<List<PdfBookmarkRange>> bookmarkRanges();

  /// Classifies the pages named by what they hold; every page when [pages]
  /// is null.
  PdfTask<List<PdfPageClassification>> classifyPages({required PdfPages pages});

  /// Classifies the entire document by content type.
  PdfTask<PdfDocumentKind> classifyDocument();

  /// Reads every AcroForm field of the document.
  PdfTask<List<PdfFormField>> formFields();

  /// Exports AcroForm field values as FDF or XFDF to [output].
  PdfTask<void> exportFormData(DataSink output, {required String format});

  /// Reads the document's XFA structure, or null when it has none.
  PdfTask<PdfXfaInfo?> xfa();

  /// Lists the files embedded in the document.
  PdfTask<List<PdfAttachment>> attachments();

  /// Lists the annotations on one page.
  PdfTask<List<PdfAnnotationInfo>> annotations(int page);

  /// Streams the embedded file named [name] to [output].
  PdfTask<void> extractAttachment(String name, DataSink output);

  // ── Lifecycle ──

  /// Releases the document handle and any held resources.
  Future<void> dispose();
}

/// Handle to an open PDF editor session.
abstract class BridgeEditorHandle {
  /// The hook of the combinator that owns this handle (see
  /// [BridgeDocHandle.scope]).
  CancelHook? scope;

  /// Number of pages in the document.
  PdfTask<int> pageCount();

  /// PDF version string.
  PdfTask<String> version();

  /// Whether the document has unsaved modifications.
  PdfTask<bool> isModified();

  // ── Metadata ──

  /// Returns the document's standard info fields as the edited document
  /// has them now.
  PdfTask<PdfMetadata> metadata();

  /// Replaces the document's standard info fields with [metadata]; a null
  /// field is removed.
  PdfTask<void> setMetadata(PdfMetadata metadata);

  // ── Pages ──

  /// Turns every page in [pages] by [by] on top of its current rotation.
  PdfTask<void> rotatePages(PdfPages pages, {required PdfRotation by});

  /// Sets the absolute rotation of every page in [pages].
  PdfTask<void> setRotation(PdfPages pages, PdfRotation rotation);

  /// Returns the facts of [pages] (every page when all) as the edited
  /// document has them now.
  PdfTask<List<PdfPageInfo>> pageInfo({required PdfPages pages});

  /// Sets the media box of every page in [pages].
  PdfTask<void> setMediaBox(PdfPages pages, PdfRect box);

  /// Sets the crop box of every page in [pages]; a null [box] removes it.
  PdfTask<void> setCropBox(PdfPages pages, PdfRect? box);

  /// Deletes every page in [pages].
  PdfTask<void> removePages(PdfPages pages);

  /// Moves a page from index [from] to index [to].
  PdfTask<void> movePage({required int from, required int to});

  /// Retains only the pages in [order], in that order, removing all others.
  PdfTask<void> keepPages(List<int> order);

  /// Appends pages from [source] into this document; [pages] selects
  /// which ones of it, [password] opens it.
  PdfTask<void> appendPages(
    DataSource source, {
    required PdfPages pages,
    String? password,
  });

  // ── Optimization ──

  /// Re-encodes and downsamples images under [policy]; one row per image.
  PdfTask<PdfImageReport> reduceImages(PdfImagePolicy policy);

  /// Removes embedded standard fonts, returning count unembedded.
  PdfTask<int> unembedStandardFonts();

  // ── Watermark + stamps ──

  /// Adds a text watermark to every page in [pages].
  PdfTask<void> addWatermark(
    PdfPages pages,
    String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition position = const PdfWatermarkPosition.center(),
    PdfWatermarkLayer layer = PdfWatermarkLayer.foreground,
  });

  /// Adds a predefined stamp to every page in [pages].
  PdfTask<void> addStamp(
    PdfPages pages, {
    required PdfStamp stamp,
    required PdfRect bounds,
    required double opacity,
  });

  /// Adds an image stamp to every page in [pages] from [imageData].
  PdfTask<void> addImageStamp(
    PdfPages pages,
    DataSource imageData, {
    required PdfRect bounds,
    required double opacity,
  });

  // ── Content ──

  /// Embeds a file attachment with the given [name].
  PdfTask<void> addAttachment(
    String name,
    DataSource data, {
    String? description,
    String? mimeType,
    PdfAttachmentRelationship? relationship,
  });

  /// Flattens the interactive form fields on every page in [pages] into
  /// static content.
  PdfTask<void> flattenForms(PdfPages pages);

  /// Flattens the annotations on every page in [pages] into page content.
  PdfTask<void> flattenAnnotations(PdfPages pages);

  /// Sets a form field's value by [fieldName].
  PdfTask<void> setFormFieldValue(String fieldName, String value);

  /// Checks or clears the checkbox field identified by [fieldName].
  PdfTask<void> setCheckboxFieldValue(String fieldName, bool checked);

  /// Selects the option [value] of the choice field [fieldName].
  PdfTask<void> setFormFieldChoice(String fieldName, String value);

  /// Selects the options [values] of the multi-select list box [fieldName].
  PdfTask<void> setFormFieldChoices(String fieldName, List<String> values);

  /// Clears the value of the form field [fieldName].
  PdfTask<void> clearFormFieldValue(String fieldName);

  /// Crops margins from all pages.
  PdfTask<void> cropMargins({
    double left = 0,
    double right = 0,
    double top = 0,
    double bottom = 0,
  });

  /// Converts the document to PDF/A conformance at [level].
  PdfTask<void> convertToPdfA({PdfALevel level = PdfALevel.a2});

  /// Lists the image XObjects placed on [page].
  PdfTask<List<PdfImagePlacement>> pageImages(int page);

  /// Moves and resizes an embedded image on [page] by [imageName].
  PdfTask<void> setImageBounds(int page, String imageName, PdfRect bounds);

  // ── Form field properties ──

  /// Removes the form field [name] and its widget.
  PdfTask<void> removeFormField(String name);

  /// Sets the form field [name]'s read-only flag.
  PdfTask<void> setFormFieldReadOnly(String name, bool readOnly);

  /// Sets the form field [name]'s required flag.
  PdfTask<void> setFormFieldRequired(String name, bool required);

  /// Sets the form field [name]'s tooltip; null removes it.
  PdfTask<void> setFormFieldTooltip(String name, String? tooltip);

  /// Sets the form field [name]'s bounding rectangle.
  PdfTask<void> setFormFieldBounds(String name, PdfRect bounds);

  /// Sets the form field [name]'s maximum text length; null removes it.
  PdfTask<void> setFormFieldMaxLength(String name, int? maxLength);

  /// Sets the form field [name]'s text alignment.
  PdfTask<void> setFormFieldAlignment(String name, PdfAlignment alignment);

  /// Sets the form field [name]'s background color; null removes it.
  PdfTask<void> setFormFieldBackgroundColor(String name, PdfColor? color);

  /// Sets the form field [name]'s border color; null removes it.
  PdfTask<void> setFormFieldBorderColor(String name, PdfColor? color);

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

  /// Marks a [region] on [page] for redaction, to be drawn over with
  /// [overlay] when it is applied.
  PdfTask<void> markRedaction(
    int page,
    PdfRect region,
    PdfRedactionOverlay overlay,
  );

  /// Drops every queued redaction mark.
  PdfTask<void> clearRedactions();

  /// Redacts [regions] on [page] at once: marks and applies them, and only
  /// them, leaving the marks queued before as they were.
  PdfTask<PdfRedactionReport> redact(
    int page,
    List<PdfRect> regions,
    PdfRedactionOverlay overlay,
  );

  /// Returns the number of pending redaction marks on [page].
  PdfTask<int> redactionCount(int page);

  /// Permanently applies all pending redactions, removing content, and
  /// reports what was removed.
  PdfTask<PdfRedactionReport> applyRedactions();

  /// Strips metadata, JavaScript and/or attachments as asked.
  PdfTask<void> sanitize({
    required bool stripMetadata,
    required bool stripJavaScript,
    required bool stripAttachments,
  });

  // ── Save ──

  /// Saves the edited document to [output].
  PdfTask<void> save(DataSink output, {PdfSave save = const PdfSave.full()});

  /// Runs the save of [save] into no sink and returns the bytes the engine
  /// wrote: the size the file would have.
  PdfTask<int> measureSave({PdfSave save = const PdfSave.full()});

  // ── Extract pages (select → save → restore, editor unchanged) ──

  /// Extracts [pages] into [output] without modifying the editor.
  PdfTask<void> extractPages(List<int> pages, DataSink output);

  /// Runs the extract of [extractPages] into no sink and returns the bytes
  /// the engine wrote: the size the extracted file would have.
  PdfTask<int> measurePages(List<int> pages);

  // ── Lifecycle ──

  /// Releases the editor handle and any held resources.
  Future<void> dispose();
}

/// Handle to a PDF builder session.
abstract class BridgeBuilderHandle {
  /// The hook of the combinator that owns this handle (see
  /// [BridgeDocHandle.scope]); the page builders it returns inherit it.
  CancelHook? scope;

  /// Sets the document metadata.
  PdfTask<void> setMetadata(PdfMetadata metadata);

  /// Adds a page of [size] and returns a page builder.
  PdfTask<BridgePageBuilderHandle> addPage(PdfPageSize size);

  /// Adds a page holding [imageData] and returns a page builder.
  PdfTask<BridgePageBuilderHandle> addImagePage(
    DataSource imageData, {
    PdfImagePage page = const PdfImagePage(),
  });

  /// Saves the built document to [output]. Streams are Flate-compressed
  /// unless [deflate] is false.
  PdfTask<void> save(DataSink output, {bool deflate = true});

  /// Releases the builder handle.
  Future<void> dispose();
}

/// Handle to a single page being built.
abstract class BridgePageBuilderHandle {
  /// Sets the current font by [name] and [size].
  PdfTask<void> setFont(String name, double size);

  /// Moves the cursor to [point].
  PdfTask<void> moveTo(PdfPoint point);

  /// Writes [text] at the current position, in [style]'s font, size,
  /// weight, slant and colour, then anchors [link] and [markup] on it.
  PdfTask<void> text(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  });

  /// Writes a heading of [level] with [text].
  PdfTask<void> heading(int level, String text);

  /// Writes a wrapped paragraph of [text].
  PdfTask<void> paragraph(String text);

  /// Inserts vertical space of [points].
  PdfTask<void> space(double points);

  /// Draws a horizontal rule across the page.
  PdfTask<void> horizontalRule();

  /// Places an image from [imageData] within [bounds]. A null [altText]
  /// places it as an artifact, which assistive technology ignores.
  PdfTask<void> image(DataSource imageData, PdfRect bounds, {String? altText});

  /// Adds a diagonal watermark [text] across the page.
  PdfTask<void> watermark(String text);

  /// Creates a text input field with [name] in [bounds], then its [scripts].
  PdfTask<void> textField(
    String name,
    PdfRect bounds, {
    String? value,
    required PdfFieldScripts scripts,
  });

  /// Creates a checkbox with [name] in [bounds], then its [scripts].
  PdfTask<void> checkbox(
    String name,
    PdfRect bounds, {
    required bool isChecked,
    required PdfFieldScripts scripts,
  });

  /// Creates a combo box with [name] in [bounds], then its [scripts].
  PdfTask<void> comboBox(
    String name,
    PdfRect bounds,
    List<String> options, {
    String? selected,
    required PdfFieldScripts scripts,
  });

  /// Creates a radio button group with [name], then its [scripts].
  PdfTask<void> radioGroup(
    String name,
    List<PdfRadioOption> options, {
    String? selected,
    required PdfFieldScripts scripts,
  });

  /// Creates a push button with [name] in [bounds].
  PdfTask<void> pushButton(String name, PdfRect bounds, String caption);

  /// Creates a digital signature field with [name] in [bounds].
  PdfTask<void> signatureField(String name, PdfRect bounds);

  /// Adds a footnote with [mark] and [note].
  PdfTask<void> footnote(String mark, String note);

  /// Writes inline [text] at the cursor without moving to a new line, in
  /// [style]'s font, size, weight, slant and colour, then anchors [link] and
  /// [markup] on it.
  PdfTask<void> appendText(
    String text, {
    PdfTextStyle? style,
    PdfLink? link,
    PdfMarkup? markup,
  });

  /// Wraps [text] inside [bounds] with the given [align].
  PdfTask<void> textIn(
    PdfRect bounds,
    String text, {
    PdfAlignment align = PdfAlignment.left,
  });

  /// Draws [rect] with [fill] inside it and [stroke] around it. A null
  /// [fill] leaves the rectangle unfilled, a null [stroke] leaves it
  /// unstroked.
  PdfTask<void> rect(PdfRect rect, {PdfColor? fill, PdfStroke? stroke});

  /// Draws a straight line from [from] to [to].
  PdfTask<void> line(
    PdfPoint from,
    PdfPoint to, {
    PdfStroke stroke = const PdfStroke(),
  });

  /// Adds a sticky note with [text] at [at], or at the cursor when [at]
  /// is null.
  PdfTask<void> stickyNote(String text, {PdfPoint? at});

  /// Adds a stamp of [stamp] at the cursor.
  PdfTask<void> stamp(PdfStamp stamp);

  /// Adds a free-text annotation inside [bounds].
  PdfTask<void> freeText(PdfRect bounds, String text);

  /// Lays out a table from [columns] and [rows] with row spans (within the
  /// options' max row span) and column sizing; the fixed sizing needs a
  /// width on every column, the measuring sizings size the columns
  /// themselves.
  PdfTask<void> table(
    List<PdfTableColumn> columns,
    List<List<PdfTableCell>> rows, {
    PdfTableOptions options = const PdfTableOptions(),
  });

  /// Lays out [text] across [count] columns with [gap] between them.
  PdfTask<void> columns(int count, double gap, String text);

  /// Inserts a line break.
  PdfTask<void> newline();

  /// Runs [script] when the page opens.
  PdfTask<void> onOpen(String script);

  /// Runs [script] when the page closes.
  PdfTask<void> onClose(String script);
}
