/**
 * Fluent document builder — the programmatic multi-page construction API
 * exposed through the C FFI.
 *
 * Mirrors the Python / WASM / C# / Go equivalents. The same handle-lifetime
 * contract applies: terminal methods (`build`, `save`, `saveEncrypted`,
 * `toBytesEncrypted`) CONSUME the builder, and only one `PageBuilder` may
 * be open at a time.
 *
 * @example
 * ```typescript
 * import { DocumentBuilder, EmbeddedFont } from 'pdf-oxide';
 *
 * const font = EmbeddedFont.fromFile('DejaVuSans.ttf');
 * const builder = DocumentBuilder.create()
 *   .title('Hello')
 *   .registerEmbeddedFont('DejaVu', font);   // consumes `font`
 * builder.a4Page()
 *   .font('DejaVu', 12)
 *   .at(72, 720).text('Привет, мир!')
 *   .at(72, 700).text('Καλημέρα κόσμε')
 *   .done();
 * const bytes = builder.build();            // consumes the builder
 * ```
 */

// Load the addon via the shared prebuild-aware loader — resolves
// against `prebuilds/<triple>/pdf_oxide.node` in the published
// package and the in-tree `build/Release/` output in dev mode.
import { loadNative } from '../native.js';
import {
  Align,
  type Column,
  type SpanCell,
  type StreamingTableConfig,
  type TableMode,
  type TableSpec,
} from '../types/common.js';
import { StreamingTable } from './streaming-table.js';

const native = loadNative();

/**
 * TTF/OTF font handle registerable with {@link DocumentBuilder}. Single-use:
 * after `registerEmbeddedFont` the native handle is moved into the builder
 * and this object becomes disposed.
 */
export class EmbeddedFont {
  private _handle: unknown;
  private _consumed = false;

  private constructor(handle: unknown) {
    this._handle = handle;
  }

  /** Load a TTF / OTF font from disk. */
  static fromFile(path: string): EmbeddedFont {
    return new EmbeddedFont(native.embeddedFontFromFile(path));
  }

  /** Load a font from a byte buffer; pass `name` to override the PostScript name. */
  static fromBytes(data: Uint8Array | Buffer, name?: string): EmbeddedFont {
    return new EmbeddedFont(native.embeddedFontFromBytes(data, name));
  }

  /** @internal — used by {@link DocumentBuilder.registerEmbeddedFont} */
  get handle(): unknown {
    if (this._consumed) {
      throw new Error('EmbeddedFont already consumed');
    }
    return this._handle;
  }

  /** @internal — called by the builder after the FFI transfers ownership. */
  markConsumed(): void {
    this._consumed = true;
    this._handle = null;
  }

  /** Release the native font handle if it hasn't been consumed. */
  close(): void {
    if (!this._consumed && this._handle != null) {
      native.embeddedFontFree(this._handle);
      this._consumed = true;
      this._handle = null;
    }
  }

  /** Symbol.dispose support for `using` declarations. */
  [Symbol.dispose](): void {
    this.close();
  }
}

/**
 * Fluent top-level API for multi-page PDF construction.
 * Use {@link DocumentBuilder.create} to start a new builder.
 */
export class DocumentBuilder {
  private _handle: unknown;
  private _consumed = false;
  private _openPage: PageBuilder | null = null;

  private constructor(handle: unknown) {
    this._handle = handle;
  }

  /** Create a fresh empty builder. */
  static create(): DocumentBuilder {
    return new DocumentBuilder(native.documentBuilderCreate());
  }

  /** @internal — used by PageBuilder.done */
  clearOpenPage(): void {
    this._openPage = null;
  }

  private checkUsable(): unknown {
    if (this._consumed || this._handle == null) {
      throw new Error('DocumentBuilder has been consumed');
    }
    if (this._openPage != null) {
      throw new Error('A PageBuilder is already open; call done() first.');
    }
    return this._handle;
  }

  /** Set the document title. */
  title(title: string): this {
    native.documentBuilderSetTitle(this.checkUsable(), title);
    return this;
  }

  /** Set the document author. */
  author(author: string): this {
    native.documentBuilderSetAuthor(this.checkUsable(), author);
    return this;
  }

  /** Set the document subject. */
  subject(subject: string): this {
    native.documentBuilderSetSubject(this.checkUsable(), subject);
    return this;
  }

  /** Set the document keywords (comma-separated per PDF convention). */
  keywords(keywords: string): this {
    native.documentBuilderSetKeywords(this.checkUsable(), keywords);
    return this;
  }

  /** Set the creator application name. */
  creator(creator: string): this {
    native.documentBuilderSetCreator(this.checkUsable(), creator);
    return this;
  }

  /** Run a JavaScript script when the document is opened (/OpenAction). */
  onOpen(script: string): this {
    native.documentBuilderOnOpen(this.checkUsable(), script);
    return this;
  }

  /**
   * Enable PDF/UA-1 tagged PDF mode.
   *
   * When enabled, `build()` emits `/MarkInfo`, `/StructTreeRoot`, `/Lang`, and
   * `/ViewerPreferences` in the catalog. Opt-in — no effect unless called.
   * Bundle F-1/F-2.
   */
  taggedPdfUa1(): this {
    native.documentBuilderTaggedPdfUa1(this.checkUsable());
    return this;
  }

  /**
   * Set the document's natural language tag, e.g. `"en-US"`.
   *
   * Emitted as `/Lang` in the catalog when `taggedPdfUa1()` is set. Bundle F-2.
   */
  language(lang: string): this {
    native.documentBuilderLanguage(this.checkUsable(), lang);
    return this;
  }

  /**
   * Add a role-map entry: custom structure type → standard PDF structure type.
   *
   * Emitted in `/RoleMap` inside the StructTreeRoot when `taggedPdfUa1()` is
   * set. Multiple calls accumulate entries. Bundle F-4.
   */
  roleMap(custom: string, standard: string): this {
    native.documentBuilderRoleMap(this.checkUsable(), custom, standard);
    return this;
  }

  /**
   * Register a TTF / OTF font under `name`. CONSUMES `font` on success —
   * do not call `close()` on the font afterwards.
   */
  registerEmbeddedFont(name: string, font: EmbeddedFont): this {
    const builderHandle = this.checkUsable();
    // font.handle getter throws if already consumed, so validation is done.
    native.documentBuilderRegisterEmbeddedFont(builderHandle, name, font.handle);
    font.markConsumed();
    return this;
  }

  /** Start a new A4 page. Only one page may be outstanding per builder. */
  a4Page(): PageBuilder {
    const h = this.checkUsable();
    const pageHandle = native.documentBuilderA4Page(h);
    this._openPage = new PageBuilder(this, pageHandle);
    return this._openPage;
  }

  /** Start a new US Letter page. */
  letterPage(): PageBuilder {
    const h = this.checkUsable();
    const pageHandle = native.documentBuilderLetterPage(h);
    this._openPage = new PageBuilder(this, pageHandle);
    return this._openPage;
  }

  /** Start a page with custom dimensions in PDF points (72 pt = 1 inch). */
  page(width: number, height: number): PageBuilder {
    const h = this.checkUsable();
    const pageHandle = native.documentBuilderPage(h, width, height);
    this._openPage = new PageBuilder(this, pageHandle);
    return this._openPage;
  }

  private consumeHandle(): unknown {
    const h = this.checkUsable();
    this._consumed = true;
    this._handle = null;
    return h;
  }

  /** Build the PDF and return the bytes. CONSUMES the builder. */
  build(): Buffer {
    const h = this.consumeHandle();
    try {
      return native.documentBuilderBuild(h);
    } finally {
      native.documentBuilderFree(h);
    }
  }

  /** Save the PDF to a file. CONSUMES the builder. */
  save(path: string): void {
    const h = this.consumeHandle();
    try {
      native.documentBuilderSave(h, path);
    } finally {
      native.documentBuilderFree(h);
    }
  }

  /** Save the PDF with AES-256 encryption. CONSUMES the builder. */
  saveEncrypted(path: string, userPassword: string, ownerPassword: string): void {
    const h = this.consumeHandle();
    try {
      native.documentBuilderSaveEncrypted(h, path, userPassword, ownerPassword);
    } finally {
      native.documentBuilderFree(h);
    }
  }

  /** Return the PDF as encrypted bytes (AES-256). CONSUMES the builder. */
  toBytesEncrypted(userPassword: string, ownerPassword: string): Buffer {
    const h = this.consumeHandle();
    try {
      return native.documentBuilderToBytesEncrypted(h, userPassword, ownerPassword);
    } finally {
      native.documentBuilderFree(h);
    }
  }

  /** Release native resources if the builder wasn't consumed. */
  close(): void {
    if (!this._consumed && this._handle != null) {
      native.documentBuilderFree(this._handle);
      this._consumed = true;
      this._handle = null;
    }
  }

  /** Symbol.dispose support for `using` declarations. */
  [Symbol.dispose](): void {
    this.close();
  }
}

/**
 * Fluent per-page builder returned by `DocumentBuilder.a4Page` etc.
 * Single-use — `done()` commits the page and invalidates this builder.
 */
export class PageBuilder {
  private _parent: DocumentBuilder;
  private _handle: unknown;
  private _done = false;

  /** @internal — constructed by DocumentBuilder */
  constructor(parent: DocumentBuilder, handle: unknown) {
    this._parent = parent;
    this._handle = handle;
  }

  private h(): unknown {
    if (this._done || this._handle == null) {
      throw new Error('PageBuilder already committed');
    }
    return this._handle;
  }

  // --- content --------------------------------------------------------

  /** Set font + size for subsequent text. */
  font(name: string, size: number): this {
    native.pageBuilderFont(this.h(), name, size);
    this._lastFontSize = size;
    return this;
  }

  /** Move the cursor to absolute coordinates. */
  at(x: number, y: number): this {
    native.pageBuilderAt(this.h(), x, y);
    return this;
  }

  /** Emit a line of text at the current cursor position. */
  text(text: string): this {
    native.pageBuilderText(this.h(), text);
    return this;
  }

  /** Emit a heading (level 1-6). */
  heading(level: number, text: string): this {
    native.pageBuilderHeading(this.h(), level, text);
    return this;
  }

  /** Emit a paragraph with automatic line wrapping. */
  paragraph(text: string): this {
    native.pageBuilderParagraph(this.h(), text);
    return this;
  }

  /** Advance the cursor by the given number of points. */
  space(points: number): this {
    native.pageBuilderSpace(this.h(), points);
    return this;
  }

  /** Draw a horizontal rule across the page. */
  horizontalRule(): this {
    native.pageBuilderHorizontalRule(this.h());
    return this;
  }

  // --- annotations (Phase 3) -----------------------------------------

  /** Attach a URL link to the previous text element. */
  linkUrl(url: string): this {
    native.pageBuilderLinkUrl(this.h(), url);
    return this;
  }

  /** Link the previous text to an internal page (0-based). */
  linkPage(pageIndex: number): this {
    native.pageBuilderLinkPage(this.h(), pageIndex);
    return this;
  }

  /** Link the previous text to a named destination. */
  linkNamed(destination: string): this {
    native.pageBuilderLinkNamed(this.h(), destination);
    return this;
  }

  /** Link the previous text to a JavaScript action. */
  linkJavascript(script: string): this {
    native.pageBuilderLinkJavascript(this.h(), script);
    return this;
  }

  /** Run JavaScript when this page is opened (/AA /O). */
  onOpen(script: string): this {
    native.pageBuilderOnOpen(this.h(), script);
    return this;
  }

  /** Run JavaScript when this page is closed (/AA /C). */
  onClose(script: string): this {
    native.pageBuilderOnClose(this.h(), script);
    return this;
  }

  /** Set a keystroke JS action (/AA /K) on the last form field. */
  fieldKeystroke(script: string): this {
    native.pageBuilderFieldKeystroke(this.h(), script);
    return this;
  }

  /** Set a format JS action (/AA /F) on the last form field. */
  fieldFormat(script: string): this {
    native.pageBuilderFieldFormat(this.h(), script);
    return this;
  }

  /** Set a validate JS action (/AA /V) on the last form field. */
  fieldValidate(script: string): this {
    native.pageBuilderFieldValidate(this.h(), script);
    return this;
  }

  /** Set a calculate JS action (/AA /C) on the last form field. */
  fieldCalculate(script: string): this {
    native.pageBuilderFieldCalculate(this.h(), script);
    return this;
  }

  /** Highlight the previous text with an RGB colour (channels 0-1). */
  highlight(r: number, g: number, b: number): this {
    native.pageBuilderHighlight(this.h(), r, g, b);
    return this;
  }

  /** Underline the previous text. */
  underline(r: number, g: number, b: number): this {
    native.pageBuilderUnderline(this.h(), r, g, b);
    return this;
  }

  /** Strike through the previous text. */
  strikeout(r: number, g: number, b: number): this {
    native.pageBuilderStrikeout(this.h(), r, g, b);
    return this;
  }

  /** Squiggly-underline the previous text. */
  squiggly(r: number, g: number, b: number): this {
    native.pageBuilderSquiggly(this.h(), r, g, b);
    return this;
  }

  /** Attach a sticky-note annotation to the previous text. */
  stickyNote(text: string): this {
    native.pageBuilderStickyNote(this.h(), text);
    return this;
  }

  /** Place a sticky-note at an absolute position. */
  stickyNoteAt(x: number, y: number, text: string): this {
    native.pageBuilderStickyNoteAt(this.h(), x, y, text);
    return this;
  }

  /** Apply a text watermark to the page. */
  watermark(text: string): this {
    native.pageBuilderWatermark(this.h(), text);
    return this;
  }

  /** Apply the standard "CONFIDENTIAL" diagonal watermark. */
  watermarkConfidential(): this {
    native.pageBuilderWatermarkConfidential(this.h());
    return this;
  }

  /** Apply the standard "DRAFT" diagonal watermark. */
  watermarkDraft(): this {
    native.pageBuilderWatermarkDraft(this.h());
    return this;
  }

  /**
   * Attach a standard stamp annotation at the cursor (150×50 default).
   * `typeName` matches the PDF spec's standard stamps (Approved,
   * NotApproved, Draft, Confidential, Final, Experimental, Expired,
   * ForPublicRelease, NotForPublicRelease, AsIs, Sold, Departmental,
   * ForComment, TopSecret) — any other name becomes a custom stamp.
   */
  stamp(typeName: string): this {
    native.pageBuilderStamp(this.h(), typeName);
    return this;
  }

  /** Place a free-flowing text annotation inside the rectangle (x, y, w, h). */
  freeText(x: number, y: number, w: number, h: number, text: string): this {
    native.pageBuilderFreetext(this.h(), x, y, w, h, text);
    return this;
  }

  // --- Form-field widgets --------------------------------------------

  /**
   * Add a single-line text form field at the rectangle (x, y, w, h).
   * `name` is the unique field identifier used for form submission;
   * `defaultValue` is the initial text (pass undefined for blank).
   */
  textField(name: string, x: number, y: number, w: number, h: number, defaultValue?: string): this {
    native.pageBuilderTextField(this.h(), name, x, y, w, h, defaultValue);
    return this;
  }

  /**
   * Add a checkbox form field at the rectangle (x, y, w, h).
   * `checked` sets the initial state.
   */
  checkbox(
    name: string,
    x: number,
    y: number,
    w: number,
    h: number,
    checked: boolean = false
  ): this {
    native.pageBuilderCheckbox(this.h(), name, x, y, w, h, checked);
    return this;
  }

  /**
   * Add a dropdown combo-box form field. `options` are the user-
   * visible choices; `selected` picks the initial value.
   */
  comboBox(
    name: string,
    x: number,
    y: number,
    w: number,
    h: number,
    options: string[],
    selected?: string
  ): this {
    native.pageBuilderComboBox(this.h(), name, x, y, w, h, options, selected);
    return this;
  }

  /**
   * Add a radio-button group. `buttons` is an array of
   * `[exportValue, x, y, w, h]` tuples — one per option. `selected`
   * picks the initial value by export value.
   */
  radioGroup(
    name: string,
    buttons: Array<[string, number, number, number, number]>,
    selected?: string
  ): this {
    native.pageBuilderRadioGroup(this.h(), name, buttons, selected);
    return this;
  }

  /** Add a clickable push button with a visible caption. */
  pushButton(name: string, x: number, y: number, w: number, h: number, caption: string): this {
    native.pageBuilderPushButton(this.h(), name, x, y, w, h, caption);
    return this;
  }

  /** Add an unsigned signature placeholder field (/FT /Sig) at the given bounds. */
  signatureField(name: string, x: number, y: number, w: number, h: number): this {
    native.pageBuilderSignatureField(this.h(), name, x, y, w, h);
    return this;
  }

  /**
   * Add a footnote: inline `refMark` emitted at the cursor position, and
   * `noteText` placed near the page bottom with a separator artifact line.
   */
  footnote(refMark: string, noteText: string): this {
    native.pageBuilderFootnote(this.h(), refMark, noteText);
    return this;
  }

  /**
   * Lay out `text` as balanced multi-column flow.
   * `columnCount` columns separated by `gapPt` points.
   * Paragraphs in `text` are delimited by `"\n\n"`.
   */
  columns(columnCount: number, gapPt: number, text: string): this {
    native.pageBuilderColumns(this.h(), columnCount, gapPt, text);
    return this;
  }

  /**
   * Emit `text` inline at the current cursor position without advancing
   * to a new line. The cursor advances horizontally so the next `inline`
   * call follows on the same line.
   */
  inline(text: string): this {
    native.pageBuilderInline(this.h(), text);
    return this;
  }

  /** Emit `text` inline in bold weight. */
  inlineBold(text: string): this {
    native.pageBuilderInlineBold(this.h(), text);
    return this;
  }

  /** Emit `text` inline in italic style. */
  inlineItalic(text: string): this {
    native.pageBuilderInlineItalic(this.h(), text);
    return this;
  }

  /** Emit `text` inline in an RGB colour (channels 0–1). */
  inlineColor(r: number, g: number, b: number, text: string): this {
    native.pageBuilderInlineColor(this.h(), r, g, b, text);
    return this;
  }

  /** Advance the cursor to the start of the next line. */
  newline(): this {
    native.pageBuilderNewline(this.h());
    return this;
  }

  // --- Barcode / QR-code placement ------------------------------------

  /**
   * Place a 1-D barcode image on the page at `(x, y, w, h)`.
   * `barcodeType`: 0=Code128 1=Code39 2=EAN13 3=EAN8 4=UPCA 5=ITF
   * 6=Code93 7=Codabar.
   */
  barcode1d(barcodeType: number, data: string, x: number, y: number, w: number, h: number): this {
    native.pageBuilderBarcode1d(this.h(), barcodeType, data, x, y, w, h);
    return this;
  }

  /** Place a QR-code image on the page (square: `size × size` pt). */
  barcodeQr(data: string, x: number, y: number, size: number): this {
    native.pageBuilderBarcodeQr(this.h(), data, x, y, size);
    return this;
  }

  /**
   * Embed a JPEG/PNG image at (x, y, w, h) in PDF points.
   * No alt text or /Artifact wrapper. Use `imageWithAlt` or `imageArtifact`
   * for PDF/UA-1 accessibility requirements.
   */
  image(bytes: Buffer | Uint8Array, x: number, y: number, w: number, h: number): this {
    native.pageBuilderImage(this.h(), bytes, x, y, w, h);
    return this;
  }

  /**
   * Embed an image with an accessibility alt text (PDF/UA-1 §Figure).
   * `bytes` must contain raw JPEG/PNG/WebP image data.
   */
  imageWithAlt(
    bytes: Buffer | Uint8Array,
    x: number,
    y: number,
    w: number,
    h: number,
    altText: string
  ): this {
    native.pageBuilderImageWithAlt(this.h(), bytes, x, y, w, h, altText);
    return this;
  }

  /**
   * Embed a decorative image as an /Artifact (no alt text, PDF/UA-1 §Artifact).
   * `bytes` must contain raw JPEG/PNG/WebP image data.
   */
  imageArtifact(bytes: Buffer | Uint8Array, x: number, y: number, w: number, h: number): this {
    native.pageBuilderImageArtifact(this.h(), bytes, x, y, w, h);
    return this;
  }

  // --- Low-level graphics primitives ---------------------------------

  /** Draw a stroked rectangle outline (1pt black). */
  rect(x: number, y: number, w: number, h: number): this {
    native.pageBuilderRect(this.h(), x, y, w, h);
    return this;
  }

  /** Draw a filled rectangle in RGB colour (channels 0–1). */
  filledRect(x: number, y: number, w: number, h: number, r: number, g: number, b: number): this {
    native.pageBuilderFilledRect(this.h(), x, y, w, h, r, g, b);
    return this;
  }

  /** Draw a line from `(x1, y1)` to `(x2, y2)` with 1pt black stroke. */
  line(x1: number, y1: number, x2: number, y2: number): this {
    native.pageBuilderLine(this.h(), x1, y1, x2, y2);
    return this;
  }

  // --- v0.3.39 primitives (#393) -------------------------------------

  /**
   * Draw a stroked rectangle outline with caller-supplied width + RGB
   * colour (channels 0–1). Underlies the Table surface.
   */
  strokeRect(
    x: number,
    y: number,
    w: number,
    h: number,
    style?: { width?: number; color?: [number, number, number] }
  ): this {
    const width = style?.width ?? 1;
    const [r, g, b] = style?.color ?? [0, 0, 0];
    native.pageBuilderStrokeRect(this.h(), x, y, w, h, width, r, g, b);
    return this;
  }

  /**
   * Draw a straight line with caller-supplied width + RGB colour.
   */
  strokeLine(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    style?: { width?: number; color?: [number, number, number] }
  ): this {
    const width = style?.width ?? 1;
    const [r, g, b] = style?.color ?? [0, 0, 0];
    native.pageBuilderStrokeLine(this.h(), x1, y1, x2, y2, width, r, g, b);
    return this;
  }

  /**
   * Draw a dashed rectangle outline. `dash` is the on/off lengths in PDF
   * points (e.g. `[3, 2]` → 3 pt on, 2 pt off). `phase` offsets the dash
   * start within the pattern.
   */
  strokeRectDashed(
    x: number,
    y: number,
    w: number,
    h: number,
    dash: number[],
    phase: number = 0,
    style?: { width?: number; color?: [number, number, number] }
  ): this {
    const width = style?.width ?? 1;
    const [r, g, b] = style?.color ?? [0, 0, 0];
    native.pageBuilderStrokeRectDashed(this.h(), x, y, w, h, width, r, g, b, dash, phase);
    return this;
  }

  /**
   * Draw a dashed straight line. `dash` / `phase` semantics are the same as
   * {@link strokeRectDashed}.
   */
  strokeLineDashed(
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    dash: number[],
    phase: number = 0,
    style?: { width?: number; color?: [number, number, number] }
  ): this {
    const width = style?.width ?? 1;
    const [r, g, b] = style?.color ?? [0, 0, 0];
    native.pageBuilderStrokeLineDashed(this.h(), x1, y1, x2, y2, width, r, g, b, dash, phase);
    return this;
  }

  /**
   * Place wrapped text inside the rectangle (x, y, w, h) with the
   * given horizontal alignment. Uses the current font + size. Text
   * that does not fit is clipped to the rectangle height.
   */
  textInRect(
    x: number,
    y: number,
    w: number,
    h: number,
    text: string,
    align: Align = Align.Left
  ): this {
    native.pageBuilderTextInRect(this.h(), x, y, w, h, text, align);
    return this;
  }

  /**
   * Start a new page with the *same* dimensions as the current one.
   * Text config (font + size) carries over; the cursor resets to the
   * top-left margin. Callers wanting header-repeat-on-break must
   * re-emit the header explicitly.
   */
  newPageSameSize(): this {
    native.pageBuilderNewPageSameSize(this.h());
    return this;
  }

  /**
   * Measure the width of `text` in the current font and size.
   *
   * Note: v0.3.39 ships a JS-side approximation (0.55em per glyph for
   * ASCII / typical Latin-1 characters). A true per-glyph measurement
   * FFI is pending. The result is in PDF points.
   */
  measure(text: string): number {
    // Retrieve current font size if the user tracked it via `font(...)`.
    // We have no FFI query for the size; fall back to the 10pt default.
    const size = this._lastFontSize ?? 10;
    // ~0.55 em is a reasonable average for proportional fonts.
    return text.length * size * 0.55;
  }

  /**
   * Estimate the remaining vertical space on the page at the current
   * cursor position. v0.3.39 returns `null` when the native cursor is
   * unknown — callers should treat `null` as "unknown; assume fresh
   * page". A real FFI hook is pending in a follow-up release.
   */
  remainingSpace(): number | null {
    return null;
  }

  /**
   * Emit a buffered table at the current cursor. All rows are
   * marshalled into a single FFI call — memory scales with the row
   * count. For million-row streams use {@link streamingTable}.
   */
  table(spec: TableSpec): this {
    const columns = spec.columns;
    if (!columns || columns.length === 0) {
      throw new Error('table spec must contain at least one column');
    }
    const widths = columns.map((c) => c.width);
    const aligns = columns.map((c) => (c.align ?? Align.Left) as number);
    const hasHeader = spec.hasHeader !== false;
    const rows = spec.rows ?? [];
    const cells: Array<string | null> = [];
    if (hasHeader) {
      for (const c of columns) cells.push(c.header);
    }
    const bodyRowCount = rows.length;
    for (const row of rows) {
      if (row.length !== columns.length) {
        throw new Error(`row width ${row.length} does not match column count ${columns.length}`);
      }
      for (const cell of row) cells.push(cell ?? null);
    }
    const totalRows = (hasHeader ? 1 : 0) + bodyRowCount;
    native.pageBuilderTable(this.h(), widths, aligns, totalRows, cells, hasHeader);
    return this;
  }

  /**
   * Begin a streaming table. Uses the native row-at-a-time FFI
   * (`pdf_page_builder_streaming_table_begin_v2`). Pass a `mode` in
   * `config` to control column-sizing strategy (default: fixed widths).
   */
  streamingTable(config: StreamingTableConfig): StreamingTable {
    return new StreamingTable(this, config);
  }

  /** @internal — open streaming table FFI handle on this page. */
  _streamingTableBeginV2(
    headers: string[],
    widths: number[],
    aligns: number[],
    repeatHeader: boolean,
    mode: TableMode | undefined,
    maxRowspan: number
  ): void {
    let modeInt = 0;
    let sampleRows = 20;
    let minW = 0;
    let maxW = 9999;
    if (mode?.kind === 'sample') {
      modeInt = 1;
      if (mode.sampleRows != null) sampleRows = mode.sampleRows;
      if (mode.minColWidthPt != null) minW = mode.minColWidthPt;
      if (mode.maxColWidthPt != null) maxW = mode.maxColWidthPt;
    }
    native.pageBuilderStreamingTableBeginV2(
      this.h(),
      headers,
      widths,
      aligns,
      repeatHeader,
      modeInt,
      sampleRows,
      minW,
      maxW,
      maxRowspan
    );
  }

  /** @internal — push one row into the open streaming table (all rowspan=1). */
  _streamingTablePushRow(cells: Array<string | null>): void {
    native.pageBuilderStreamingTablePushRow(this.h(), cells);
  }

  /** @internal — push one row with per-cell rowspan values. */
  _streamingTablePushRowV2(cells: Array<[string | null, number]>): void {
    native.pageBuilderStreamingTablePushRowV2(this.h(), cells);
  }

  /** @internal — set the batch size for the open streaming table. */
  _streamingTableSetBatchSize(batchSize: number): void {
    native.pageBuilderStreamingTableSetBatchSize(this.h(), batchSize);
  }

  /** @internal — return pending row count from native layer. */
  _streamingTablePendingRowCount(): number {
    return native.pageBuilderStreamingTablePendingRowCount(this.h());
  }

  /** @internal — return batch count from native layer. */
  _streamingTableBatchCount(): number {
    return native.pageBuilderStreamingTableBatchCount(this.h());
  }

  /** @internal — flush (mark batch boundary) in native layer. */
  _streamingTableFlush(): void {
    native.pageBuilderStreamingTableFlush(this.h());
  }

  /** @internal — close the open streaming table. */
  _streamingTableFinish(): void {
    native.pageBuilderStreamingTableFinish(this.h());
  }

  /** @internal — track the last font size for JS-side `measure()`. */
  _lastFontSize?: number;

  /**
   * Commit the page's buffered operations to the parent builder and
   * return the parent for chaining. After `done()` this PageBuilder is
   * invalid.
   */
  done(): DocumentBuilder {
    if (this._done) {
      throw new Error('PageBuilder already committed');
    }
    native.pageBuilderDone(this._handle);
    this._done = true;
    this._handle = null;
    this._parent.clearOpenPage();
    return this._parent;
  }

  /**
   * Drop an uncommitted page. Use only for error recovery — the parent's
   * open-page slot is released so the next `a4Page()` etc. succeeds.
   */
  close(): void {
    if (!this._done && this._handle != null) {
      native.pageBuilderFree(this._handle);
      this._parent.clearOpenPage();
      this._done = true;
      this._handle = null;
    }
  }

  /** Symbol.dispose support for `using`. */
  [Symbol.dispose](): void {
    this.close();
  }
}

export { StreamingTable } from './streaming-table.js';
// Re-export the v0.3.39 table surface so users can `import { Align,
// StreamingTable } from 'pdf-oxide'` without reaching into ./types or
// ./builders/streaming-table.
export {
  Align,
  type Column,
  type SpanCell,
  type StreamingTableConfig,
  type TableMode,
  type TableSpec,
};
