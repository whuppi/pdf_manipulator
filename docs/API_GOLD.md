# API Gold Standard — The Final Design

> The definitive public API for pdf_manipulator. Every type earned.
> Every method earned. Every parameter typed. Every scope explicit.
> No null-means-all. No magic numbers. No duplicate methods. No
> functionality lost. Fully scalable for future operations.
>
> This doc supersedes API_SURFACE.md.
>
> ## Naming convention
>
> **Every public type is prefixed with `Pdf`.** No exceptions.
> No judgment calls about "is this name generic enough to conflict."
> The prefix is the namespace. The consumer never needs
> `import ... as pdf` to resolve a collision.
>
> This matches the industry standard: Syncfusion's `PdfDocument`,
> `PdfPage`, `PdfColor`; DavBfr's `PdfDocument`, `PdfPage`,
> `PdfGraphics`. Every PDF package in the Dart ecosystem prefixes
> with `Pdf`. We follow the same convention.
>
> **Derivation rule for future names:** take the concept name
> (e.g., "table extraction result") → PascalCase it
> (`TableExtractionResult`) → prefix with `Pdf`
> (`PdfTableExtractionResult`). Apply to every new class, enum,
> sealed type, and typedef.

---

## 1. Core principle: sealed types for scope, not null

Every operation that can target "one page," "some pages," or "all pages"
uses a sealed type. The compiler enforces exhaustive handling. The call
site reads like English. No null, no empty list, no guessing.

```dart
sealed class PdfPages {
  const PdfPages();

  /// Every page in the document.
  const factory PdfPages.all() = PdfAllPages;

  /// A single page by index (0-based).
  const factory PdfPages.single(int index) = PdfSinglePage;

  /// A list of specific pages by index (0-based).
  const factory PdfPages.list(List<int> indices) = PdfPageList;

  /// A range of pages (inclusive start, exclusive end).
  const factory PdfPages.range(int start, int end) = PdfPageRange;
}

class PdfAllPages extends PdfPages { const PdfAllPages(); }
class PdfSinglePage extends PdfPages {
  final int index;
  const PdfSinglePage(this.index);
}
class PdfPageList extends PdfPages {
  final List<int> indices;
  const PdfPageList(this.indices);
}
class PdfPageRange extends PdfPages {
  final int start;
  final int end;
  const PdfPageRange(this.start, this.end);
}
```

Usage:
```dart
// Render one page
await for (final page in pdf.render(source, pages: PdfPages.single(0))) { ... }

// Render pages 0, 3, 7
await for (final page in pdf.render(source, pages: PdfPages.list([0, 3, 7]))) { ... }

// Render all pages
await for (final page in pdf.render(source, pages: PdfPages.all())) { ... }

// Render pages 5 through 9
await for (final page in pdf.render(source, pages: PdfPages.range(5, 10))) { ... }
```

The engine resolves the sealed type:
```dart
switch (pages) {
  case AllPages():      // iterate 0..pageCount
  case SinglePage(:final index): // just that one
  case PageList(:final indices): // iterate the list
  case PageRange(:final start, :final end): // iterate start..<end
}
```

Adding a future scope (e.g., `PdfPages.odd()`, `PdfPages.bookmarked('Chapter 1')`)
is one new subclass + one new switch arm. No existing code breaks — the
compiler tells every consumer to handle the new case.

---

## 2. The two interfaces

```dart
/// Random-access byte source. The engine reads targeted ranges —
/// xref (few KB), page objects (few KB each). Never the full file.
///
/// The consumer implements this with their backing store:
/// file (pread), memory (sublistView), HTTP (Range header),
/// IndexedDB (getAll with key range), etc.
abstract interface class PdfSource {
  /// Total size in bytes. Required — PDF parsing needs it for xref lookup.
  int get length;

  /// Read [count] bytes at [offset]. Returns fewer bytes only at end-of-source.
  FutureOr<Uint8List> readAt(int offset, int count);
}

/// Sequential byte sink. The engine writes chunks as it produces output.
/// Chunks are typically 4KB–256KB.
///
/// The consumer implements this with their destination:
/// file (write), memory (BytesBuilder), upload stream, etc.
abstract interface class PdfSink {
  /// Write a chunk. Called sequentially, never concurrently.
  FutureOr<void> write(Uint8List chunk);
}
```

---

## 3. Typed enums — no magic numbers anywhere

```dart
/// PDF encryption algorithm.
enum PdfEncryptionAlgorithm {
  /// RC4, 40-bit key. Legacy, weak. PDF 1.1+.
  rc4_40,
  /// RC4, 128-bit key. PDF 1.4+.
  rc4_128,
  /// AES, 128-bit key. PDF 1.5+.
  aes128,
  /// AES, 256-bit key. Recommended. PDF 1.7+.
  aes256,
}

/// Text/content extraction format.
enum PdfExtractionFormat {
  /// Auto-detect the best method for the content.
  auto,
  /// Structured text preserving reading order and layout.
  text,
  /// Markdown with headings, lists, tables, images.
  markdown,
  /// HTML with styled elements.
  html,
  /// Flat plain text, no structure.
  plainText,
}

/// Office document format for conversion.
enum PdfDocumentFormat { docx, pptx, xlsx }

/// Standard stamp annotation type.
enum PdfStampType {
  approved, experimental, notApproved, asIs, expired,
  notForPublicRelease, confidential, final_, sold,
  departmental, forComment, topSecret, draft, forPublicRelease,
}
```

---

## 4. Typed parameter groups — no loose parameters

### PdfColor

```dart
class PdfColor {
  final double r, g, b;
  const PdfColor(this.r, this.g, this.b);

  static const black = PdfColor(0, 0, 0);
  static const white = PdfColor(1, 1, 1);
  static const gray = PdfColor(0.5, 0.5, 0.5);
  static const red = PdfColor(1, 0, 0);
}
```

### PdfPermissions

```dart
class PdfPermissions {
  final bool print, printHq, modify, copy;
  final bool annotate, fillForms, accessibility, assemble;

  const PdfPermissions({
    this.print = true, this.printHq = true,
    this.modify = true, this.copy = true,
    this.annotate = true, this.fillForms = true,
    this.accessibility = true, this.assemble = true,
  });

  const PdfPermissions.all() : this();
  const PdfPermissions.readOnly()
      : print = false, printHq = false, modify = false, copy = false,
        annotate = false, fillForms = false, accessibility = true,
        assemble = false;
}
```

### PdfEncryptionConfig

```dart
class PdfEncryptionConfig {
  final String ownerPassword;
  final String userPassword;
  final PdfEncryptionAlgorithm algorithm;
  final PdfPermissions permissions;

  const PdfEncryptionConfig({
    required this.ownerPassword,
    this.userPassword = '',
    this.algorithm = PdfEncryptionAlgorithm.aes256,
    this.permissions = const PdfPermissions.all(),
  });
}
```

### PdfSaveOptions

```dart
class PdfSaveOptions {
  final bool compress;
  final bool garbageCollect;
  final bool linearize;
  final PdfEncryptionConfig? encryption;

  const PdfSaveOptions({
    this.compress = true,
    this.garbageCollect = true,
    this.linearize = false,
    this.encryption,
  });
}
```

### PdfWatermarkStyle

```dart
class PdfWatermarkStyle {
  final double fontSize;
  final String? fontName;
  final double opacity;
  final double rotation;
  final PdfColor color;

  const PdfWatermarkStyle({
    this.fontSize = 48,
    this.fontName,
    this.opacity = 0.3,
    this.rotation = 45,
    this.color = PdfColor.gray,
  });
}
```

### PdfWatermarkPosition

```dart
class PdfWatermarkPosition {
  final double x, y, width, height;
  final bool fixedPrint;
  final double fixedPrintH, fixedPrintV;

  const PdfWatermarkPosition({
    required this.x, required this.y,
    required this.width, required this.height,
    this.fixedPrint = false,
    this.fixedPrintH = 0, this.fixedPrintV = 0,
  });
}
```

### PdfRenderSize

```dart
/// Output size constraint for rendering.
/// Null = native resolution. Both set = fit preserving aspect ratio.
class PdfRenderSize {
  final int maxWidth;
  final int maxHeight;
  const PdfRenderSize({required this.maxWidth, required this.maxHeight});

  /// Square thumbnail.
  const PdfRenderSize.thumbnail(int size) : maxWidth = size, maxHeight = size;
}
```

### PdfRadioOption

```dart
class PdfRadioOption {
  final String value;
  final PdfRect rect;
  const PdfRadioOption(this.value, this.rect);
}
```

### PdfValidationResult

```dart
class PdfValidationResult {
  final bool compliant;
  final int errors;
  final int warnings;
  const PdfValidationResult({
    required this.compliant,
    required this.errors,
    required this.warnings,
  });
}
```

### PdfPageClassification

```dart
class PdfPageClassification {
  final String primaryType;
  final double confidence;
  final Map<String, dynamic> details;
  const PdfPageClassification({...});
}
```

### PdfDocumentClassification

```dart
class PdfDocumentClassification {
  final String primaryType;
  final List<PdfPageClassification> pages;
  final Map<String, dynamic> details;
  const PdfDocumentClassification({...});
}
```

### PdfBookmarkSplit

```dart
class PdfBookmarkSplit {
  final String title;
  final int startPage;
  final int endPage;
  const PdfBookmarkSplit({...});
}
```

### PdfConfig

```dart
class PdfConfig {
  /// Custom web worker URL. Ignored on native.
  final String? webWorkerUrl;
  const PdfConfig({this.webWorkerUrl});
}
```

---

## 5. Return types — what comes back

### PdfDoc

```dart
class PdfDoc {
  final int pageCount;
  final String version;
  final List<PdfPageInfo> pages;
  final String? title, author, subject, keywords;
  final bool isEncrypted;
  final bool requiresPassword;
  final bool isTagged;
  final PdfEncryptionAlgorithm? encryptionAlgorithm;
  final PdfPermissions? permissions;
  const PdfDoc({...});
}
```

### PdfPageInfo

```dart
class PdfPageInfo {
  final int index;
  final double width, height;
  final int rotation;
  final String? label;
  const PdfPageInfo({...});

  double get effectiveWidth => (rotation == 90 || rotation == 270) ? height : width;
  double get effectiveHeight => (rotation == 90 || rotation == 270) ? width : height;
}
```

### PdfRenderedPage, PdfImage, PdfSearchResult, PdfSignatureInfo, PdfRect

Unchanged from current — these types are clean already.

### PdfCancelled error (NEW)

```dart
class PdfCancelled extends PdfError {
  const PdfCancelled() : super('Operation cancelled');
}
```

---

## 6. Pdf — every one-shot operation

```dart
class Pdf {
  /// One instance = one worker + one thread pool.
  /// Most apps need one instance.
  Pdf({PdfConfig? config});

  /// Open for batch editing. Shares this instance's pool.
  Future<PdfEditor> edit(PdfSource source, {String? password});

  /// Create from scratch. Shares this instance's pool.
  Future<PdfBuilder> build();

  // ── Inspect ──────────────────────────────────────────────────

  /// Parse and inspect. Returns metadata, pages, encryption status.
  /// Throws PdfCorrupted, PdfPasswordRequired.
  Future<PdfDoc> open(PdfSource source, {String? password});

  // ── Structural ───────────────────────────────────────────────

  Future<void> merge(List<PdfSource> inputs, PdfSink output);

  Future<void> split(PdfSource source,
      PdfSink Function(int index) sinkFactory, {required int every});

  Future<int> splitBySize(PdfSource source,
      PdfSink Function(int index) sinkFactory, {required int maxBytes});

  Future<List<PdfBookmarkSplit>> planSplitByBookmarks(
      PdfSource source, {String? password});

  Future<void> splitByBookmarks(PdfSource source,
      PdfSink Function(int index) sinkFactory, {String? password});

  Future<void> extractPages(PdfSource source, PdfSink output,
      {required List<int> pages});

  Future<void> deletePages(PdfSource source, PdfSink output,
      {required List<int> pages});

  Future<void> reorderPages(PdfSource source, PdfSink output,
      {required List<int> order});

  Future<void> movePage(PdfSource source, PdfSink output,
      {required int from, required int to});

  Future<void> rotatePages(PdfSource source, PdfSink output,
      {required Map<int, int> pages});

  Future<void> rotateAllPages(PdfSource source, PdfSink output,
      {required int degrees});

  // ── Content ──────────────────────────────────────────────────

  Future<void> flattenForms(PdfSource source, PdfSink output);
  Future<void> applyRedactions(PdfSource source, PdfSink output);

  Future<void> embedFile(PdfSource source, PdfSink output,
      {required String name, required Uint8List fileData});

  Future<void> eraseRegions(PdfSource source, PdfSink output,
      {required int page, required List<PdfRect> regions});

  Future<void> compress(PdfSource source, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true,
       bool linearize = false});

  // ── Extraction ───────────────────────────────────────────────

  /// Extract text/markdown/html/plainText from pages.
  Future<String> extract(PdfSource source, {
    required Pages pages,
    String? password,
    PdfExtractionFormat format = PdfExtractionFormat.auto,
  });

  // ── Search ───────────────────────────────────────────────────

  /// Search for text across pages.
  Future<List<PdfSearchResult>> search(PdfSource source, {
    required String query,
    required Pages pages,
    String? password,
  });

  // ── Security ─────────────────────────────────────────────────

  Future<void> watermark(PdfSource source, PdfSink output, {
    required String text,
    Pages pages = const PdfPages.all(),
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  });

  Future<void> encrypt(PdfSource source, PdfSink output, {
    required PdfEncryptionConfig encryption,
  });

  Future<void> decrypt(PdfSource source, PdfSink output, {
    required String password,
  });

  Future<void> sign(PdfSource source, PdfSink output, {
    required Uint8List certificate,
    required String certificatePassword,
    String? reason,
    String? location,
  });

  // ── Stamps ───────────────────────────────────────────────────

  Future<void> addStamp(PdfSource source, PdfSink output, {
    required int page,
    required PdfStampType type,
    required PdfRect rect,
    String? customName,
    double opacity = 1.0,
  });

  Future<void> addImageStamp(PdfSource source, PdfSink output, {
    required int page,
    required Uint8List imageBytes,
    required PdfRect rect,
    double opacity = 1.0,
  });

  // ── Creation ─────────────────────────────────────────────────

  Future<void> imagesToPdf(Stream<Uint8List> images, PdfSink output);

  // ── Rendering ────────────────────────────────────────────────

  /// Render pages to RGBA pixels, yielding one at a time.
  Stream<PdfRenderedPage> render(PdfSource source, {
    required Pages pages,
    PdfRenderSize? size,
    String? password,
  });

  // ── Image extraction ─────────────────────────────────────────

  /// Extract embedded images, yielding one at a time.
  Stream<PdfImage> extractImages(PdfSource source, {
    required Pages pages,
    String? password,
  });

  // ── Conversion ───────────────────────────────────────────────

  /// Convert PDF to office format.
  Future<void> convertTo(PdfSource source, PdfSink output, {
    required PdfDocumentFormat format,
    String? password,
  });

  /// Convert office document to PDF.
  Future<void> convertToPdf(PdfSource document, PdfSink output, {
    required PdfDocumentFormat format,
  });

  // ── Classification ───────────────────────────────────────────

  Future<PdfPageClassification> classifyPage(PdfSource source, int page, {
    String? password,
  });

  Future<PdfDocumentClassification> classifyDocument(PdfSource source, {
    String? password,
  });

  // ── Signatures ───────────────────────────────────────────────

  Future<List<PdfSignatureInfo>> getSignatures(PdfSource source, {
    String? password,
  });

  Future<bool> verifySignatures(PdfSource source, {
    String? password,
  });

  // ── Validation ───────────────────────────────────────────────

  Future<PdfValidationResult> validatePdfA(PdfSource source, {
    int level = 2, String? password,
  });

  Future<bool> validatePdfUa(PdfSource source, {
    int level = 1, String? password,
  });

  // ── Lifecycle ────────────────────────────────────────────────

  /// Cancel all running ops. Free all resources. Instant.
  Future<void> dispose();
}
```

---

## 7. PdfEditor — batch editing

```dart
class PdfEditor {

  // ── Properties ───────────────────────────────────────────────

  Future<int> get pageCount;
  Future<String> get version;
  Future<bool> get isModified;

  // ── Metadata ─────────────────────────────────────────────────

  Future<String> getTitle();
  Future<void> setTitle(String value);
  Future<String> getAuthor();
  Future<void> setAuthor(String value);
  Future<String> getSubject();
  Future<void> setSubject(String value);
  Future<String> getKeywords();
  Future<void> setKeywords(String value);

  // ── Pages ────────────────────────────────────────────────────

  Future<void> rotatePage(int page, {required int degrees});
  Future<void> rotateAllPages({required int degrees});
  Future<PdfRect> getPageMediaBox(int page);
  Future<void> deletePage(int page);
  Future<void> movePage({required int from, required int to});
  Future<void> extractPages(List<int> pages, PdfSink output);
  Future<void> mergeFrom(PdfSource otherPdf);

  // ── Optimization ─────────────────────────────────────────────

  Future<int> optimizeImages({int quality = 75});
  Future<int> unembedStandardFonts();

  // ── Watermark + stamps ───────────────────────────────────────

  Future<void> addWatermark(int page, String text, {
    PdfWatermarkStyle style = const PdfWatermarkStyle(),
    PdfWatermarkPosition? position,
  });

  Future<void> addStamp(int page, {
    required PdfStampType type,
    required PdfRect rect,
    String? customName,
    double opacity = 1.0,
  });

  Future<void> addImageStamp(int page, Uint8List imageBytes, {
    required PdfRect rect,
    double opacity = 1.0,
  });

  // ── Content ──────────────────────────────────────────────────

  Future<void> embedFile(String name, Uint8List data);
  Future<void> eraseRegions(int page, List<PdfRect> regions);
  Future<void> flattenForms();
  Future<void> flattenAllAnnotations();
  Future<void> setFormFieldValue(String fieldName, String value);
  Future<void> cropMargins({
    double left = 0, double right = 0,
    double top = 0, double bottom = 0,
  });
  Future<void> convertToPdfA({int level = 1});
  Future<void> resizeImage(int page, String imageName, {
    required double width, required double height,
  });

  // ── Redaction ────────────────────────────────────────────────

  Future<void> addRedaction(int page, PdfRect region, {String? overlayText});
  Future<int> get redactionCount;
  Future<void> applyRedactions();
  Future<void> scrubMetadata();

  // ── Save ─────────────────────────────────────────────────────

  /// One save method. Options cover compression, GC, linearization,
  /// and optional encryption. No separate saveEncrypted/saveWithOptions.
  Future<void> save(PdfSink output, {
    PdfSaveOptions options = const PdfSaveOptions(),
  });

  // ── Lifecycle ────────────────────────────────────────────────

  /// Frees editor handle. Does NOT dispose the parent Pdf's pool.
  void dispose();
}
```

---

## 8. PdfBuilder — create from scratch

```dart
class PdfBuilder {

  Future<void> setTitle(String value);
  Future<void> setAuthor(String value);
  Future<void> setSubject(String value);
  Future<void> setKeywords(String value);

  Future<PdfPageBuilder> addA4Page();
  Future<PdfPageBuilder> addLetterPage();
  Future<PdfPageBuilder> addPage({required double width, required double height});

  Future<void> save(PdfSink output, {
    PdfSaveOptions options = const PdfSaveOptions(),
  });

  void dispose();
}
```

---

## 9. PdfPageBuilder — build a single page

```dart
class PdfPageBuilder {
  // ── Text ─────────────────────────────────────────────────────

  Future<void> font(String name, double size);
  Future<void> at(double x, double y);
  Future<void> text(String text);
  Future<void> heading(int level, String text);
  Future<void> paragraph(String text);
  Future<void> space(double points);
  Future<void> horizontalRule();
  Future<void> newline();
  Future<void> newPageSameSize();

  // ── Media ────────────────────────────────────────────────────

  Future<void> image(Uint8List imageBytes, PdfRect rect, {String altText = ''});
  Future<void> watermark(String text);

  // ── Form fields (all use PdfRect) ────────────────────────────

  Future<void> textField(String name, PdfRect rect, {String? defaultValue});
  Future<void> checkbox(String name, PdfRect rect, {bool checked = false});
  Future<void> comboBox(String name, PdfRect rect, List<String> options,
      {String? selected});
  Future<void> pushButton(String name, PdfRect rect, String caption);
  Future<void> signatureField(String name, PdfRect rect);
  Future<void> radioGroup(String name, List<PdfRadioOption> options,
      {String? selected});

  // ── Field scripts ────────────────────────────────────────────

  Future<void> fieldKeystroke(String script);
  Future<void> fieldFormat(String script);
  Future<void> fieldValidate(String script);
  Future<void> fieldCalculate(String script);

  // ── Links ────────────────────────────────────────────────────

  Future<void> linkUrl(String url);
  Future<void> linkPage(int targetPage);

  // ── Layout ───────────────────────────────────────────────────

  Future<void> footnote(String refMark, String noteText);
  Future<void> columns(int columnCount, double gapPt, String text);

  Future<void> done();
}
```

---

## 10. The sealed Pages type in action — every usage

```dart
// ── Extract text from page 0 ──
final text = await pdf.extract(source,
    pages: PdfPages.single(0), format: PdfExtractionFormat.text);

// ── Extract markdown from all pages ──
final md = await pdf.extract(source,
    pages: PdfPages.all(), format: PdfExtractionFormat.markdown);

// ── Extract auto-detected from pages 5-10 ──
final auto = await pdf.extract(source,
    pages: PdfPages.range(5, 11), format: PdfExtractionFormat.auto);

// ── Search one page ──
final hits = await pdf.search(source,
    query: 'revenue', pages: PdfPages.single(0));

// ── Search all pages ──
final allHits = await pdf.search(source,
    query: 'revenue', pages: PdfPages.all());

// ── Render one page at native resolution ──
final page = await pdf.render(source, pages: PdfPages.single(0)).first;

// ── Render one page as thumbnail ──
final thumb = await pdf.render(source,
    pages: PdfPages.single(0), size: PdfRenderSize.thumbnail(150)).first;

// ── Render all pages fitted ──
await for (final p in pdf.render(source,
    pages: PdfPages.all(), size: PdfRenderSize(maxWidth: 800, maxHeight: 600))) {
  processPage(p);
}

// ── Render pages 3, 5, 9 ──
await for (final p in pdf.render(source,
    pages: PdfPages.list([3, 5, 9]))) {
  processPage(p);
}

// ── Extract images from page 0 ──
await for (final img in pdf.extractImages(source, pages: PdfPages.single(0))) {
  saveImage(img);
}

// ── Extract images from all pages ──
await for (final img in pdf.extractImages(source, pages: PdfPages.all())) {
  saveImage(img);
}

// ── Watermark specific pages ──
await pdf.watermark(source, output,
    text: 'DRAFT', pages: PdfPages.list([0, 1, 2]));

// ── Watermark all pages ──
await pdf.watermark(source, output,
    text: 'CONFIDENTIAL', pages: PdfPages.all());
```

The compiler enforces: you MUST specify a `Pages` variant. You can't
pass `null`. You can't pass an ambiguous empty list. The call site
reads what it does.

---

## 11. What the sealed type prevents (compile-time)

```dart
// WON'T COMPILE — pages is required:
pdf.render(source);

// WON'T COMPILE — null is not a Pages:
pdf.render(source, pages: null);

// WON'T COMPILE — int is not a Pages:
pdf.render(source, pages: 0);

// COMPILES — explicit intent:
pdf.render(source, pages: PdfPages.all());
pdf.render(source, pages: PdfPages.single(0));
```

And on the engine side, the switch is exhaustive:

```dart
int resolvePageCount(Pages pages, int totalPages) => switch (pages) {
  AllPages()   => totalPages,
  SinglePage(:final index) => 1,
  PageList(:final indices) => indices.length,
  PageRange(:final start, :final end) => end - start,
};
// If we add PdfPages.odd() later, this switch won't compile
// until every consumer handles it. Zero runtime surprises.
```

---

## 12. Method count

| Category | Count | Methods |
|---|---|---|
| Inspect | 1 | open |
| Structural | 10 | merge, split, splitBySize, planSplitByBookmarks, splitByBookmarks, extractPages, deletePages, reorderPages, movePage, rotatePages, rotateAllPages |
| Content | 5 | flattenForms, applyRedactions, embedFile, eraseRegions, compress |
| Extraction | 1 | extract (format + pages) |
| Search | 1 | search (query + pages) |
| Security | 4 | watermark, encrypt, decrypt, sign |
| Stamps | 2 | addStamp, addImageStamp |
| Rendering | 1 | render (pages + size) |
| Image extraction | 1 | extractImages (pages) |
| Conversion | 2 | convertTo, convertToPdf |
| Classification | 2 | classifyPage, classifyDocument |
| Signatures | 2 | getSignatures, verifySignatures |
| Validation | 2 | validatePdfA, validatePdfUa |
| Creation | 1 | imagesToPdf |
| Lifecycle | 1 | dispose |
| **TOTAL on Pdf** | **36** | |
| Editor metadata | 8 | get/set title, author, subject, keywords |
| Editor pages | 7 | rotate, rotateAll, getMediaBox, delete, move, extract, mergeFrom |
| Editor optimization | 2 | optimizeImages, unembedStandardFonts |
| Editor watermark+stamps | 3 | addWatermark, addStamp, addImageStamp |
| Editor content | 7 | embedFile, eraseRegions, flattenForms, flattenAllAnnotations, setFormFieldValue, cropMargins, convertToPdfA, resizeImage |
| Editor redaction | 4 | addRedaction, redactionCount, applyRedactions, scrubMetadata |
| Editor save | 1 | save (with PdfSaveOptions) |
| Editor lifecycle | 1 | dispose |
| **TOTAL on PdfEditor** | **33** | |

---

## 13. Scalability — how future ops fit

| Future operation | How it fits |
|---|---|
| New extraction format (e.g., `json`) | Add `PdfExtractionFormat.json`. One enum value. Zero method changes. |
| New document format (e.g., `odt`) | Add `PdfDocumentFormat.odt`. One enum value. |
| New page scope (e.g., odd pages) | Add `PdfPages.odd()`. Compiler forces handler updates. |
| New stamp type | Add `PdfStampType.xxx`. One enum value. |
| New encryption algorithm | Add `PdfEncryptionAlgorithm.xxx`. One enum value. |
| OCR-based extraction | `PdfExtractionFormat.ocr` or a new method. Format enum scales. |
| Table extraction | New method `extractTables(source, pages: PdfPages.all())`. Same `Pages` type. |
| Form field reading | New method on PdfEditor or PdfDoc. Existing types. |
| Annotation reading | New method returning typed annotations. |
| Digital timestamp | New field on PdfSignatureInfo. |

Every new capability is either a new enum value (no method change) or a new method
using existing types (`Pages`, `PdfSource`, `PdfSink`, `PdfRect`). The type system
scales. The method count grows linearly with genuinely new operations, not with
parameter combinations.

---

## 14. Companion doc

[`BRIDGE_ARCHITECTURE.md`](BRIDGE_ARCHITECTURE.md) — how data moves
between the consumer's Dart code and the Rust/WASM engine. Thread
pools, condvars, OPFS, arena allocators, cancellation, dispose. The
plumbing that makes this API work without full-file buffers.

This doc (API_GOLD) defines WHAT the consumer sees.
BRIDGE_ARCHITECTURE defines HOW it works underneath.
Zero duplication between them.

---

## 15. The one-line summary

> **Sealed `PdfPages` type for every page-scoped operation — no null,
> no guessing, compiler-enforced exhaustive handling. Every public type
> prefixed with `Pdf` — zero namespace conflicts. One `render()` with
> `PdfPages` + `PdfRenderSize`. One `extract()` with `PdfPages` +
> `PdfExtractionFormat`. One `search()` with `PdfPages`. One `save()`
> with `PdfSaveOptions`. `Stream<Uint8List>` for `imagesToPdf`. Every
> enum is typed. Every parameter group is a class. Every scope is
> explicit. Zero duplicate methods. Zero lost functionality. Scales
> by adding enum values and sealed subtypes, not by adding methods.**
