/* tslint:disable */
/* eslint-disable */

/**
 * Horizontal-alignment enum shared by `textInRect`, buffered `table`, and
 * `streamingTable`. Maps 1:1 onto [`crate::writer::TextAlign`] /
 * [`crate::writer::CellAlign`]. Exported to JS as `Align` via `js_name`.
 */
export enum Align {
    /**
     * Align to the left edge.
     */
    Left = 0,
    /**
     * Center horizontally.
     */
    Center = 1,
    /**
     * Align to the right edge.
     */
    Right = 2,
}

/**
 * Style configuration for header/footer text.
 */
export class ArtifactStyle {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Set bold font for the artifact.
     */
    bold(): ArtifactStyle;
    /**
     * Set color for the artifact.
     */
    color(r: number, g: number, b: number): ArtifactStyle;
    /**
     * Set font for the artifact.
     */
    font(name: string, size: number): ArtifactStyle;
    /**
     * Create a new artifact style.
     */
    constructor();
}

/**
 * Chroma subsampling format
 */
export enum ChromaSampling {
    /**
     * Both vertically and horizontally subsampled.
     */
    Cs420 = 0,
    /**
     * Horizontally subsampled.
     */
    Cs422 = 1,
    /**
     * Not subsampled.
     */
    Cs444 = 2,
    /**
     * Monochrome.
     */
    Cs400 = 3,
}

/**
 * A parsed Document Security Store (`/DSS`, ISO 32000-2 §12.8.4.3).
 * Count + index accessors mirror `WasmCertificate`'s flat shape
 * (wasm-bindgen cannot return `Uint8Array[]` directly).
 */
export class Dss {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * The `i`-th DER certificate, or `undefined` if out of range.
     */
    getCert(i: number): Uint8Array | undefined;
    /**
     * The `i`-th DER CRL, or `undefined` if out of range.
     */
    getCrl(i: number): Uint8Array | undefined;
    /**
     * The `i`-th DER OCSP response, or `undefined` if out of range.
     */
    getOcsp(i: number): Uint8Array | undefined;
    /**
     * Number of DER X.509 certificates in the DSS.
     */
    readonly certCount: number;
    /**
     * Number of DER CRLs in the DSS.
     */
    readonly crlCount: number;
    /**
     * Number of DER OCSP responses in the DSS.
     */
    readonly ocspCount: number;
    /**
     * Per-signature VRI keys (uppercase-hex SHA-1 of `/Contents`).
     */
    readonly vri: string[];
}

/**
 * PAdES baseline level. Frozen integer mapping (BB=0, BT=1, BLt=2,
 * BLta=3) shared with the C ABI and every binding — never renumber.
 */
export enum PadesLevel {
    /**
     * B-B: signed attrs incl. the ESS signing-certificate-v2.
     */
    BB = 0,
    /**
     * B-T: B-B + an RFC 3161 signature-time-stamp unsigned attr.
     */
    BT = 1,
    /**
     * B-LT: B-T + a Document Security Store (DSS/VRI).
     */
    BLt = 2,
    /**
     * B-LTA: B-LT + a document-scoped `/DocTimeStamp`.
     */
    BLta = 3,
}

/**
 * Offline B-LT validation material (DER certs / CRLs / OCSP
 * responses). Build with `new()` then `addCert`/`addCrl`/`addOcsp`.
 */
export class RevocationMaterial {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Add a DER X.509 certificate.
     */
    addCert(der: Uint8Array): void;
    /**
     * Add a DER CRL.
     */
    addCrl(der: Uint8Array): void;
    /**
     * Add a DER OCSP response.
     */
    addOcsp(der: Uint8Array): void;
    /**
     * Create an empty revocation-material set.
     */
    constructor();
}

/**
 * WASM handle to a streaming-table building session. Created by
 * `FluentPageBuilder.streamingTable()`; rows are pushed via `pushRow`,
 * and the session is sealed with `finish()`.
 *
 * Single-use: `finish()` twice throws, and `pushRow` after `finish()`
 * throws. The rows are buffered and replayed against the real Rust
 * `StreamingTable` at `WasmFluentPageBuilder.done()` commit time —
 * preserving the FluentPageBuilder borrow-lifetime invariant that can't
 * cross the wasm-bindgen boundary.
 */
export class StreamingTable {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Number of fully-completed batches waiting for finish().
     */
    batchCount(): number;
    /**
     * Number of columns configured on this streaming table.
     */
    columnCount(): number;
    /**
     * Seal the streaming table — the buffered rows are flushed onto the
     * parent page's op queue, to be replayed against the real Rust
     * `StreamingTable` at `done()` commit time. Calling `finish()` twice
     * throws.
     */
    finish(): void;
    /**
     * Explicitly flush the current batch to `completed_batches`.
     * Called automatically when `batch_size` rows accumulate.
     */
    flush(): void;
    /**
     * Number of rows in the current (not-yet-flushed) batch.
     */
    pendingRowCount(): number;
    /**
     * Push one row as an array of cell strings (all rowspan=1). Returns an
     * error if the table has already been finished or if the row's cell count
     * does not match the column count. Auto-flushes the batch when full.
     */
    pushRow(cells: string[]): void;
    /**
     * Push one row with per-cell rowspan values. `cells` is a JS array of
     * `[text, rowspan]` two-element arrays. `rowspan == 1` is a normal cell.
     * Auto-flushes the batch when full.
     */
    pushRowSpan(cells: any): void;
}

/**
 * A header or footer artifact definition.
 */
export class WasmArtifact {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a center-aligned artifact.
     */
    static center(text: string): WasmArtifact;
    /**
     * Create a left-aligned artifact.
     */
    static left(text: string): WasmArtifact;
    /**
     * Create a new artifact.
     */
    constructor();
    /**
     * Create a right-aligned artifact.
     */
    static right(text: string): WasmArtifact;
    /**
     * Set vertical offset for the artifact.
     */
    withOffset(offset: number): WasmArtifact;
    /**
     * Set style for the artifact.
     */
    withStyle(style: ArtifactStyle): WasmArtifact;
}

/**
 * X.509 certificate parsed from a raw DER blob. Mirrors the C#,
 * Node, Python, and Go `Certificate` surfaces — `subject` / `issuer`
 * / `serial` / `validity` / `isValid` getters only.
 */
export class WasmCertificate {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Load a certificate from a DER-encoded X.509 blob. Throws if
     * the DER doesn't parse.
     */
    static load(data: Uint8Array): WasmCertificate;
    /**
     * Load a signer certificate + private key from PEM strings.
     * `certPem` must begin `-----BEGIN CERTIFICATE-----`.
     * `keyPem` must begin `-----BEGIN PRIVATE KEY-----` or `-----BEGIN RSA PRIVATE KEY-----`.
     */
    static loadPem(cert_pem: string, key_pem: string): WasmCertificate;
    /**
     * Load a signer certificate + private key from a PKCS#12 (.p12/.pfx) blob.
     * `password` is the passphrase protecting the key bag.
     */
    static loadPkcs12(data: Uint8Array, password: string): WasmCertificate;
    /**
     * Whether the certificate is currently within its validity
     * window. Does NOT verify chain, trust-root, or revocation.
     */
    readonly isValid: boolean;
    /**
     * Issuer distinguished name.
     */
    readonly issuer: string;
    /**
     * Serial number as a hex string (no `0x` prefix).
     */
    readonly serial: string;
    /**
     * Subject distinguished name.
     */
    readonly subject: string;
    /**
     * Validity window as `[notBefore, notAfter]` Unix epoch seconds.
     * JavaScript: `new Date(notBefore * 1000)` for a Date.
     */
    readonly validity: BigInt64Array;
}

/**
 * WASM wrapper for [`crate::writer::DocumentBuilder`]. Fluent API for
 * programmatic multi-page PDF construction with embedded fonts and
 * annotations.
 *
 * The terminal methods (`build`, `toBytesEncrypted`) **consume** the
 * builder; subsequent calls throw `Error: DocumentBuilder already
 * consumed`.
 */
export class WasmDocumentBuilder {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Start a new A4 page. Returns a `FluentPageBuilder` that must be
     * committed with `.done()` before calling another page method or a
     * terminal (`build`, etc.).
     */
    a4Page(): WasmFluentPageBuilder;
    /**
     * Set document author.
     */
    author(author: string): void;
    /**
     * Build the PDF and return it as a `Uint8Array`. **Consumes** the
     * builder.
     */
    build(): Uint8Array;
    /**
     * Commit a completed `FluentPageBuilder` back to this builder.
     * Takes the place of the Rust `page.done()` re-parenting.
     *
     * JS users typically don't call this directly — the ergonomic
     * pattern is `builder.a4Page();` for each page, then
     * `builder.commitPage(page)` once ops are queued. For more fluent
     * code, see the `FluentPageBuilder.done(builder)` helper which
     * delegates to this method.
     */
    commitPage(page: WasmFluentPageBuilder): void;
    /**
     * Set the creator application name recorded in the PDF.
     */
    creator(creator: string): void;
    /**
     * Set document keywords (comma-separated per PDF convention).
     */
    keywords(keywords: string): void;
    /**
     * Set the document's natural language tag (e.g. `"en-US"`).
     *
     * Emitted as `/Lang` in the catalog when `taggedPdfUa1()` is set.
     */
    language(lang: string): void;
    /**
     * Start a new US Letter page.
     */
    letterPage(): WasmFluentPageBuilder;
    /**
     * Create a new empty document builder. Equivalent to the Rust
     * [`crate::writer::DocumentBuilder::new`] — every other method
     * chains off the instance returned here.
     */
    constructor();
    /**
     * Run a JavaScript script when the document is opened (`/OpenAction`).
     */
    onOpen(script: string): void;
    /**
     * Start a new page with custom dimensions in PDF points
     * (72 pt = 1 inch).
     */
    page(width: number, height: number): WasmFluentPageBuilder;
    /**
     * Register a TTF / OTF font the pages can reference by name.
     * **Consumes** `font` — reusing the `WasmEmbeddedFont` throws.
     */
    registerEmbeddedFont(name: string, font: WasmEmbeddedFont): void;
    /**
     * Add a role-map entry: custom structure type → standard PDF structure type.
     *
     * Emitted in `/RoleMap` inside the StructTreeRoot when `taggedPdfUa1()`
     * is set. Multiple calls accumulate entries.
     */
    roleMap(custom: string, standard: string): void;
    /**
     * Set document subject.
     */
    subject(subject: string): void;
    /**
     * Enable PDF/UA-1 tagged PDF mode.
     *
     * When enabled, `build()` emits `/MarkInfo`, `/StructTreeRoot`, `/Lang`,
     * and `/ViewerPreferences` in the catalog. Opt-in — no effect unless called.
     */
    taggedPdfUa1(): void;
    /**
     * Set document title.
     */
    title(title: string): void;
    /**
     * Build the PDF with AES-256 encryption and return it as a
     * `Uint8Array`. Granted permissions default to all. **Consumes**
     * the builder.
     */
    toBytesEncrypted(user_password: string, owner_password: string): Uint8Array;
}

/**
 * Embedded TTF/OTF font usable by `WasmDocumentBuilder`. Single-use: once
 * passed to `registerEmbeddedFont`, the underlying Rust `EmbeddedFont` is
 * moved into the builder and this handle becomes empty.
 */
export class WasmEmbeddedFont {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Load an embedded font from raw TTF/OTF bytes. Pass `name` to
     * override the PostScript name baked into the font file.
     */
    static fromBytes(data: Uint8Array, name?: string | null): WasmEmbeddedFont;
    /**
     * The font's PostScript name (or the override). Empty once consumed.
     */
    readonly name: string;
}

/**
 * Per-page fluent builder. Buffers operations until `done(builder)` is
 * called, which commits them to the parent `WasmDocumentBuilder`. Each
 * instance is single-use — `done()` twice throws.
 */
export class WasmFluentPageBuilder {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    at(x: number, y: number): void;
    /**
     * Place a 1-D barcode image at `(x, y, w, h)` on the page.
     * `barcodeType`: 0=Code128 1=Code39 2=EAN13 3=EAN8 4=UPCA 5=ITF
     * 6=Code93 7=Codabar.
     */
    barcode1d(barcode_type: number, data: string, x: number, y: number, w: number, h: number): void;
    /**
     * Place a QR-code image at `(x, y, size, size)` on the page.
     */
    barcodeQr(data: string, x: number, y: number, size: number): void;
    checkbox(name: string, x: number, y: number, w: number, h: number, checked: boolean): void;
    /**
     * Lay out `text` as balanced multi-column flow (`columnCount` columns,
     * `gapPt` points between columns). Paragraphs in `text` are separated by `"\n\n"`.
     */
    columns(column_count: number, gap_pt: number, text: string): void;
    /**
     * Add a dropdown combo-box.
     */
    comboBox(name: string, x: number, y: number, w: number, h: number, options: string[], selected?: string | null): void;
    /**
     * Convenience: commit this page's buffered ops to `builder`. Same
     * as `builder.commitPage(this)` but lets JS users keep the
     * chain-like flow:
     *
     * ```javascript
     * const page = builder.a4Page();
     * page.at(72, 720); page.text("Hi");
     * page.done(builder);
     * ```
     */
    done(builder: WasmDocumentBuilder): void;
    fieldCalculate(script: string): void;
    fieldFormat(script: string): void;
    fieldKeystroke(script: string): void;
    fieldValidate(script: string): void;
    /**
     * Draw a filled rectangle. RGB channels in 0.0-1.0.
     */
    filledRect(x: number, y: number, w: number, h: number, r: number, g: number, b: number): void;
    font(name: string, size: number): void;
    /**
     * Add a footnote: inline `refMark` at the cursor and `noteText` body
     * near the page bottom with a separator artifact line.
     */
    footnote(ref_mark: string, note_text: string): void;
    freeText(x: number, y: number, w: number, h: number, text: string): void;
    heading(level: number, text: string): void;
    highlight(r: number, g: number, b: number): void;
    horizontalRule(): void;
    /**
     * Embed a decorative image (JPEG/PNG/WebP bytes) as an /Artifact (no alt text).
     */
    imageArtifact(bytes: Uint8Array, x: number, y: number, w: number, h: number): void;
    /**
     * Embed an image (JPEG/PNG/WebP bytes) with an accessibility alt text.
     */
    imageWithAlt(bytes: Uint8Array, x: number, y: number, w: number, h: number, alt_text: string): void;
    /**
     * Emit `text` inline (advances cursorX only, not cursorY).
     */
    inline(text: string): void;
    /**
     * Inline bold run.
     */
    inlineBold(text: string): void;
    /**
     * Inline colored run (RGB 0.0–1.0).
     */
    inlineColor(r: number, g: number, b: number, text: string): void;
    /**
     * Inline italic run.
     */
    inlineItalic(text: string): void;
    /**
     * Draw a line from (x1, y1) to (x2, y2) with 1pt black stroke.
     */
    line(x1: number, y1: number, x2: number, y2: number): void;
    linkJavascript(script: string): void;
    linkNamed(destination: string): void;
    linkPage(page: number): void;
    linkUrl(url: string): void;
    /**
     * Measure the rendered width of `text` in the builder's current font
     * and size, in PDF points. Pure query — does not mutate state.
     *
     * Thin JS view over [`crate::writer::FluentPageBuilder::measure`]. The
     * WASM class tracks the current font/size independently of the
     * buffered ops so this query is served without a live builder.
     */
    measure(text: string): number;
    /**
     * Finish the current page and start a new one with the same page
     * size. Cursor resets to the top-left margin (72, height-72). The
     * builder's font carries over.
     */
    newPageSameSize(): void;
    /**
     * Advance cursorY by one line-height and reset cursorX to 72 pt.
     */
    newline(): void;
    onClose(script: string): void;
    onOpen(script: string): void;
    paragraph(text: string): void;
    /**
     * Add a clickable push button with a visible caption.
     */
    pushButton(name: string, x: number, y: number, w: number, h: number, caption: string): void;
    /**
     * Add a radio-button group. `values`, `xs`, `ys`, `ws`, `hs` are
     * parallel arrays of length N describing each option's export
     * value and rectangle. `selected` picks the initial value.
     */
    radioGroup(name: string, values: string[], xs: Float32Array, ys: Float32Array, ws: Float32Array, hs: Float32Array, selected?: string | null): void;
    /**
     * Draw a stroked rectangle outline (1pt black).
     */
    rect(x: number, y: number, w: number, h: number): void;
    /**
     * Points remaining on the current page below the cursor (down to the
     * 72 pt bottom margin). Mirrors
     * [`crate::writer::FluentPageBuilder::remaining_space`] using the WASM
     * class's independently-tracked cursor — accurate after `at`, `text`,
     * `space`, `newPageSameSize`, `textInRect` and the table helpers.
     */
    remainingSpace(): number;
    /**
     * Add an unsigned signature placeholder field at the given bounds.
     */
    signatureField(name: string, x: number, y: number, w: number, h: number): void;
    space(points: number): void;
    squiggly(r: number, g: number, b: number): void;
    stamp(name: string): void;
    stickyNote(text: string): void;
    stickyNoteAt(x: number, y: number, text: string): void;
    /**
     * Open a streaming table. Returns a `StreamingTable` handle the caller
     * pushes rows into; call `finish()` when done. The streamed rows are
     * buffered per-table and replayed against the real Rust
     * `StreamingTable` at `done()` commit time — avoiding the
     * FluentPageBuilder-lifetime problem that otherwise can't cross the
     * wasm-bindgen boundary.
     */
    streamingTable(spec: any): StreamingTable;
    strikeout(r: number, g: number, b: number): void;
    /**
     * Draw a straight line with explicit stroke width and RGB colour.
     */
    strokeLine(x1: number, y1: number, x2: number, y2: number, width: number, r: number, g: number, b: number): void;
    /**
     * Draw a dashed line. `dash` is alternating on/off lengths in points; `phase` is the starting offset.
     */
    strokeLineDashed(x1: number, y1: number, x2: number, y2: number, width: number, r: number, g: number, b: number, dash: Float32Array, phase: number): void;
    /**
     * Draw a stroked rectangle with explicit stroke width and RGB colour.
     */
    strokeRect(x: number, y: number, w: number, h: number, width: number, r: number, g: number, b: number): void;
    /**
     * Draw a dashed rectangle border. `dash` is alternating on/off lengths in points; `phase` is the starting offset.
     */
    strokeRectDashed(x: number, y: number, w: number, h: number, width: number, r: number, g: number, b: number, dash: Float32Array, phase: number): void;
    /**
     * Render a buffered table from a JS object:
     *
     * ```javascript
     * page.table({
     *   columns: [
     *     { header: "SKU", width: 100, align: Align.Left },
     *     { header: "Qty", width: 60,  align: Align.Right },
     *   ],
     *   rows: [["A-1","12"], ["B-2","3"]],
     *   hasHeader: true,
     * });
     * ```
     *
     * Uses `serde-wasm-bindgen` for deserialisation. Replays against the
     * Rust `Table` builder at `done()` commit time.
     */
    table(spec: any): void;
    text(text: string): void;
    textField(name: string, x: number, y: number, w: number, h: number, default_value?: string | null): void;
    /**
     * Place wrapped text inside a rectangle with horizontal alignment.
     * `align` is the `Align` enum (0 = Left, 1 = Center, 2 = Right) — also
     * accepts a raw integer for JS callers that pre-date the enum import.
     */
    textInRect(x: number, y: number, w: number, h: number, text: string, align: number): void;
    underline(r: number, g: number, b: number): void;
    watermark(text: string): void;
    watermarkConfidential(): void;
    watermarkDraft(): void;
}

/**
 * A footer definition.
 */
export class WasmFooter {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a center-aligned footer.
     */
    static center(text: string): WasmFooter;
    /**
     * Create a left-aligned footer.
     */
    static left(text: string): WasmFooter;
    /**
     * Create a new empty footer.
     */
    constructor();
    /**
     * Create a right-aligned footer.
     */
    static right(text: string): WasmFooter;
}

/**
 * A header definition.
 */
export class WasmHeader {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a center-aligned header.
     */
    static center(text: string): WasmHeader;
    /**
     * Create a left-aligned header.
     */
    static left(text: string): WasmHeader;
    /**
     * Create a new empty header.
     */
    constructor();
    /**
     * Create a right-aligned header.
     */
    static right(text: string): WasmHeader;
}

/**
 * OCR configuration for WebAssembly. (Currently a marker — the engine
 * uses tuned defaults; knobs are exposed as the WASM OCR surface
 * matures, #524.)
 */
export class WasmOcrConfig {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create a new OCR configuration.
     */
    constructor();
}

/**
 * OCR engine for WebAssembly (#524).
 *
 * OCR runs entirely in-WASM via the pure-Rust `tract` backend — no
 * native ONNX Runtime, no JS bridge. Model **delivery is host-side**:
 * the browser/Deno/edge host fetches the detector + recognizer ONNX
 * files and the char dictionary (see `modelManifest()` for the URLs)
 * — typically `fetch()` + the Cache API / IndexedDB for the
 * tens-of-MB models — then hands the bytes to the constructor. This
 * only works in the `wasm-ocr` build of `pdf-oxide`; the default
 * `pdf-oxide-wasm` has no OCR (the constructor returns an error
 * explaining this).
 */
export class WasmOcrEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Not available in this build. OCR needs the `wasm-ocr` build of
     * `pdf-oxide` (the pure-Rust tract backend); the default
     * `pdf-oxide-wasm` ships without it.
     */
    constructor(_det_model: Uint8Array, _rec_model: Uint8Array, _dict: string, _config?: WasmOcrConfig | null);
}

/**
 * A complete page template with header and footer.
 */
export class WasmPageTemplate {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Set footer artifact.
     */
    footer(footer: WasmArtifact): WasmPageTemplate;
    /**
     * Set header artifact.
     */
    header(header: WasmArtifact): WasmPageTemplate;
    /**
     * Create a new page template.
     */
    constructor();
    /**
     * Skip rendering template on the first page.
     */
    skipFirstPage(): WasmPageTemplate;
}

/**
 * Create new PDF documents from Markdown, HTML, or plain text.
 *
 * ```javascript
 * const pdf = WasmPdf.fromMarkdown("# Hello\n\nWorld");
 * const bytes = pdf.toBytes(); // Uint8Array
 * console.log(`PDF size: ${pdf.size} bytes`);
 * ```
 */
export class WasmPdf {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Open an existing PDF from bytes for editing.
     *
     * @param data - PDF file contents as Uint8Array
     * @returns WasmPdf for editing
     */
    static fromBytes(data: Uint8Array): WasmPdf;
    /**
     * Create a PDF from HTML content.
     *
     * @param content - HTML string
     * @param title - Optional document title
     * @param author - Optional document author
     */
    static fromHtml(content: string, title?: string | null, author?: string | null): WasmPdf;
    /**
     * Render `html` with `css` applied, embedding `font_bytes` for the
     * body text. The font must cover every codepoint used by `html` or
     * unknown glyphs fall back to `.notdef`. See
     * [`Self::from_html_css_with_fonts`] for a multi-font cascade.
     */
    static fromHtmlCss(html: string, css: string, font_bytes: Uint8Array): WasmPdf;
    /**
     * Render `html` + `css` with a multi-font cascade. `families` and
     * `fonts` are parallel arrays: `families[i]` names the CSS
     * `font-family` that resolves to `fonts[i]` bytes. The first entry
     * is the default used whenever a CSS `font-family` doesn't match a
     * registered family.
     */
    static fromHtmlCssWithFonts(html: string, css: string, families: string[], fonts: Uint8Array[]): WasmPdf;
    /**
     * Create a PDF from image bytes (PNG, JPEG, etc.).
     *
     * @param data - Image file contents as a Uint8Array
     */
    static fromImageBytes(data: Uint8Array): WasmPdf;
    /**
     * Create a PDF from Markdown content.
     *
     * @param content - Markdown string
     * @param title - Optional document title
     * @param author - Optional document author
     */
    static fromMarkdown(content: string, title?: string | null, author?: string | null): WasmPdf;
    /**
     * Create a PDF from multiple image byte arrays.
     *
     * Each image becomes a separate page. Pass an array of Uint8Arrays.
     *
     * @param images_array - Array of Uint8Arrays, each containing image file bytes (PNG/JPEG)
     */
    static fromMultipleImageBytes(images_array: any): WasmPdf;
    /**
     * Create a PDF from plain text.
     *
     * @param content - Plain text string
     * @param title - Optional document title
     * @param author - Optional document author
     */
    static fromText(content: string, title?: string | null, author?: string | null): WasmPdf;
    /**
     * Merge multiple PDF byte arrays into a single PDF.
     *
     * @param pdfs - Array of Uint8Array, each containing a PDF
     * @returns WasmPdf containing all pages
     */
    static merge(pdfs: Uint8Array[]): WasmPdf;
    /**
     * Merge multiple PDFs from reader callbacks — each input reads on demand.
     *
     * @param readers - Array of [readFn, lengthFn] pairs
     * @param write_fn - JS function for streaming output: (chunk: Uint8Array) => void
     * @returns void (output streamed through write_fn)
     */
    static mergeFromReaders(readers: any[], write_fn: Function): void;
    /**
     * Get the PDF as a Uint8Array.
     */
    toBytes(): Uint8Array;
    /**
     * Get the size of the PDF in bytes.
     */
    readonly size: number;
}

/**
 * A PDF document loaded from bytes for use in WebAssembly.
 *
 * Create an instance by passing PDF file bytes to the constructor.
 * Call `.free()` when done to release memory.
 */
export class WasmPdfDocument {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add an image stamp annotation (logo watermark) to a page.
     * The image bytes (JPEG/PNG) are embedded as an XObject in the appearance stream.
     * Removal trigger: upstream adds image stamp to WasmPdfDocument.
     */
    addImageStamp(page_index: number, image_data: Uint8Array, x: number, y: number, width: number, height: number, opacity: number): void;
    /**
     * Queue an explicit destructive redaction rectangle on a page
     * (page user space; `fill` is an optional DeviceRGB `[r,g,b]`).
     */
    addRedaction(page: number, x0: number, y0: number, x1: number, y1: number, fill?: Float32Array | null): void;
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a stamp annotation to a page.
     * Removal trigger: upstream adds stamp annotations to WasmPdfDocument.
     */
    addStamp(page_index: number, stamp_type: number, custom_name: string | null | undefined, x: number, y: number, width: number, height: number, opacity: number): void;
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a text watermark to an existing page.
     * Removal trigger: upstream adds watermark to WasmPdfDocument.
     */
    addWatermark(page_index: number, text: string, font_size: number, rotation: number, opacity: number, r: number, g: number, b: number): void;
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Add a positioned watermark (text at specific coordinates) to a page.
     * Removal trigger: upstream adds positioned watermark to WasmPdfDocument.
     */
    addWatermarkPositioned(page_index: number, text: string, x: number, y: number, width: number, height: number, font_size: number, font_name: string | null | undefined, rotation: number, opacity: number, r: number, g: number, b: number): void;
    /**
     * Apply all redactions in the document.
     */
    applyAllRedactions(): void;
    /**
     * Apply redactions on a page (removes redacted content permanently).
     */
    applyPageRedactions(page_index: number): void;
    /**
     * Destructively apply all queued redactions (true content removal,
     * ISO 32000-1:2008 §12.5.6.23). Returns a `RedactionReport` object.
     */
    applyRedactionsDestructive(scrub_metadata?: boolean | null): any;
    /**
     * Authenticate with a password to decrypt an encrypted PDF.
     *
     * @param password - The password string
     * @returns true if authentication succeeded
     */
    authenticate(password: string): boolean;
    /**
     * Cheap per-page text-vs-OCR classification → JSON
     * `DocumentClassification`.
     */
    classifyDocument(): string;
    /**
     * Cheap per-page classification → JSON `PageClassification`.
     */
    classifyPage(page_index: number): string;
    /**
     * Clear all pending erase operations for a page.
     */
    clearEraseRegions(page_index: number): void;
    /**
     * Convert the document to PDF/A compliance.
     *
     * Level must be one of: `"1a"`, `"1b"`, `"2a"`, `"2b"`, `"2u"`, `"3a"`, `"3b"`, `"3u"`.
     * Returns a JS object with `success`, `level`, `actions`, and `errors` fields.
     */
    convertToPdfA(level: string): any;
    /**
     * Crop margins from all pages.
     */
    cropMargins(left: number, right: number, top: number, bottom: number): void;
    /**
     * Delete a page by index (0-based).
     */
    deletePage(index: number): void;
    /**
     * The document's Document Security Store (`/DSS`) as a `Dss`, or
     * `undefined` if absent. Mirrors Rust `signatures::read_dss`.
     */
    dss(): Dss | undefined;
    /**
     * Deprecated: Use eraseFooter instead.
     */
    editFooter(page_index: number): void;
    /**
     * Deprecated: Use eraseHeader instead.
     */
    editHeader(page_index: number): void;
    /**
     * Open a PDF for editing from a callback-based reader.
     *
     * Uses `DocumentEditor::from_document` — the editor wraps the
     * reader-opened PdfDocument directly. No full-file buffer.
     * `source_bytes` is empty — `convertToPdfA` won't work.
     *
     * @param read_fn - JS function: (offset: number, count: number) => Uint8Array
     * @param length_fn - JS function: () => number (total file size)
     * @param password - Optional password for encrypted PDFs
     */
    static editorFromReader(read_fn: Function, length_fn: Function, password?: string | null): WasmPdfDocument;
    /**
     * Embed a file into the PDF document.
     *
     * @param name - Display name for the embedded file
     * @param data - File contents as a Uint8Array
     */
    embedFile(name: string, data: Uint8Array): void;
    /**
     * Erase both header and footer content.
     *
     * @param page_index - Zero-based page number
     */
    eraseArtifacts(page_index: number): void;
    /**
     * Erase existing footer content.
     *
     * Identifies existing text in the footer area (bottom 15%) and marks it for erasure.
     *
     * @param page_index - Zero-based page number
     */
    eraseFooter(page_index: number): void;
    /**
     * Erase existing header content.
     *
     * Identifies existing text in the header area (top 15%) and marks it for erasure.
     *
     * @param page_index - Zero-based page number
     */
    eraseHeader(page_index: number): void;
    /**
     * Erase (whiteout) a rectangular region on a page.
     */
    eraseRegion(page_index: number, llx: number, lly: number, urx: number, ury: number): void;
    /**
     * Erase multiple rectangular regions on a page.
     *
     * @param page_index - Zero-based page number
     * @param rects - Flat array of coordinates [llx1,lly1,urx1,ury1, llx2,lly2,urx2,ury2, ...]
     */
    eraseRegions(page_index: number, rects: Float32Array): void;
    /**
     * Export form field data as FDF or XFDF bytes.
     *
     * @param format - "fdf" or "xfdf" (default: "fdf")
     * @returns Uint8Array containing the exported form data
     */
    exportFormData(format?: string | null): Uint8Array;
    /**
     * Extract plain text from all pages, separated by form feed characters.
     */
    extractAllText(): string;
    /**
     * Extract character-level data from a page.
     *
     * Returns an array of objects with: char, bbox {x, y, width, height},
     * font_name, font_size, font_weight, is_italic, color {r, g, b}, etc.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     */
    extractChars(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract image bytes from a page as PNG data.
     *
     * Returns an array of objects with: width, height, data (Uint8Array of PNG bytes), format ("png").
     */
    extractImageBytes(page_index: number): any;
    /**
     * Extract image metadata from a page.
     *
     * Returns an array of objects with: width, height, color_space,
     * bits_per_component, bbox (if available). Does NOT return raw image bytes.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     */
    extractImages(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract only straight lines from a page (v0.3.14).
     *
     * Identifies paths that form a single straight line segment.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects
     */
    extractLines(page_index: number, region?: Float32Array | null): any;
    /**
     * Rich per-page extraction → JSON `PageExtraction` (per-region
     * bbox + typed reason). `optionsJson` is `{}`-tolerant
     * `AutoExtractOptions`; undefined/empty → defaults.
     */
    extractPageAuto(page_index: number, options_json?: string | null): string;
    /**
     * Extract complete page text data in a single call.
     *
     * Returns `{ spans, chars, page_width, page_height }`.
     * The `chars` are derived from spans using font-metric widths when available.
     *
     * Optional `reading_order`: `"column_aware"` for XY-Cut column detection,
     * or `"top_to_bottom"` (default).
     */
    extractPageText(page_index: number, reading_order?: string | null): any;
    /**
     * Extract specific pages — removes all OTHER pages from the editor.
     * Same logic as native FFI: direct editor mutation via shared editor_ops.
     */
    extractPages(pages: Uint32Array): void;
    /**
     * Extract vector paths (lines, curves, shapes) from a page.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects with bbox, stroke_color, fill_color, etc.
     */
    extractPaths(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract only rectangles from a page (v0.3.14).
     *
     * Identifies paths that form axis-aligned rectangles.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     * @returns Array of path objects
     */
    extractRects(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract span-level data from a page.
     *
     * Returns an array of objects with: text, bbox, font_name, font_size,
     * font_weight, is_italic, color, etc.
     *
     * Optional `reading_order`: `"column_aware"` for XY-Cut column detection,
     * or `"top_to_bottom"` (default).
     */
    extractSpans(page_index: number, region?: Float32Array | null, reading_order?: string | null): any;
    /**
     * Extract tables from a page (v0.3.14).
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     */
    extractTables(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract plain text from a single page.
     *
     * @param page_index - Zero-based page number
     * @param region - Optional [x, y, width, height] to filter by
     */
    extractText(page_index: number, region: any): string;
    /**
     * One-shot auto text extraction — graceful native fallback (never
     * the opaque OCR error #513).
     */
    extractTextAuto(page_index: number): string;
    /**
     * Extract text lines from a page.
     *
     * Returns an array of objects with: text, bbox, words (array of Word objects).
     */
    extractTextLines(page_index: number, region?: Float32Array | null): any;
    /**
     * Extract text using OCR. Not available in this build — OCR needs
     * the `wasm-ocr` build of `pdf-oxide`.
     */
    extractTextOcr(_page_index: number, _engine: WasmOcrEngine): string;
    /**
     * Extract word-level data from a page.
     *
     * Returns an array of objects with: text, bbox, font_name, font_size,
     * font_weight, is_italic, is_bold.
     */
    extractWords(page_index: number, region?: Float32Array | null): any;
    /**
     * Flatten all annotations in the document into page content.
     */
    flattenAllAnnotations(): void;
    /**
     * Flatten all form fields into page content.
     *
     * After flattening, form field values become static text and are no longer editable.
     */
    flattenForms(): void;
    /**
     * Flatten form fields on a specific page.
     *
     * @param page_index - Zero-based page number
     */
    flattenFormsOnPage(page_index: number): void;
    /**
     * Flatten annotations on a page into the page content.
     */
    flattenPageAnnotations(page_index: number): void;
    /**
     * Create a flattened PDF where each page is rendered as an image.
     * Burns in all annotations, form fields, and overlays.
     * Returns the flattened PDF as bytes.
     */
    flattenToImages(dpi?: number | null): Uint8Array;
    /**
     * Return warnings collected during the last form-flattening save.
     *
     * Each entry names a widget field that had no `/AP` appearance stream;
     * flattening such a field produces a blank rectangle.
     *
     * @returns Array of warning strings
     */
    flattenWarnings(): string[];
    /**
     * Load a PDF document from a callback-based reader (OPFS).
     *
     * @param read_fn - JS function: (offset: number, count: number) => Uint8Array
     * @param length_fn - JS function: () => number (total file size)
     * @param password - Optional password for encrypted PDFs
     */
    static fromReader(read_fn: Function, length_fn: Function, password?: string | null): WasmPdfDocument;
    /**
     * Get annotations from a page.
     *
     * @param page_index - Zero-based page number
     * @returns Array of annotation objects with fields like subtype, rect, contents, etc.
     */
    getAnnotations(page_index: number): any;
    /**
     * Read the document author from the Info dictionary.
     */
    getAuthor(): string;
    /**
     * Get the value of a specific form field by name.
     *
     * @param name - Full qualified field name (e.g., "name" or "topmostSubform[0].Page1[0].f1_01[0]")
     * @returns The field value: string for text, boolean for checkbox, null if not found
     */
    getFormFieldValue(name: string): any;
    /**
     * Get all form fields from the document.
     *
     * Returns an array of form field objects, each with:
     * - name: Full qualified field name
     * - field_type: "text", "button", "choice", "signature", or "unknown"
     * - value: string, boolean, array of strings, or null
     * - tooltip: string or null
     * - bounds: [x1, y1, x2, y2] or null
     * - flags: number or null
     * - max_length: number or null
     * - is_readonly: boolean
     * - is_required: boolean
     */
    getFormFields(): any;
    /**
     * Read the document keywords from the Info dictionary.
     */
    getKeywords(): string;
    /**
     * Get the document outline (bookmarks / table of contents).
     *
     * @returns Array of outline items or null if no outline exists.
     * Each item has: { title, page (number|null), dest_name (string, optional), children (array) }
     */
    getOutline(): any;
    /**
     * Read the document subject from the Info dictionary.
     */
    getSubject(): string;
    /**
     * Read the document title from the Info dictionary.
     */
    getTitle(): string;
    /**
     * Check if the document has a structure tree (Tagged PDF).
     */
    hasStructureTree(): boolean;
    /**
     * Check if the document contains XFA form data.
     *
     * @returns true if the document has XFA form data
     */
    hasXfa(): boolean;
    /**
     * Merge another PDF (provided as bytes) into this document.
     *
     * @param data - The PDF file contents to merge as a Uint8Array
     * @returns Number of pages merged
     */
    mergeFrom(data: Uint8Array): number;
    /**
     * Move a page within the document. Zero-based; `from_index` and
     * `to_index` refer to positions **before** the move, matching the
     * Python (`PyPdfDocument.move_page`) / Go (`DocumentEditor.MovePage`) /
     * C# contracts.
     */
    movePage(from_index: number, to_index: number): void;
    /**
     * Load a PDF document from raw bytes.
     */
    constructor(data: Uint8Array, password?: string | null);
    /**
     * Open a PDF from DOCX bytes.
     */
    static openFromDocxBytes(data: Uint8Array): WasmPdfDocument;
    /**
     * Open a PDF from PPTX bytes.
     */
    static openFromPptxBytes(data: Uint8Array): WasmPdfDocument;
    /**
     * Open a PDF from XLSX bytes.
     */
    static openFromXlsxBytes(data: Uint8Array): WasmPdfDocument;
    /**
     * Get the number of pages in the document.
     */
    pageCount(): number;
    /**
     * Get the CropBox of a page as [llx, lly, urx, ury], or null if not set.
     */
    pageCropBox(page_index: number): any;
    /**
     * Get information about images on a page.
     *
     * Returns an array of {name, bounds: [x, y, width, height], matrix: [a, b, c, d, e, f]}.
     */
    pageImages(page_index: number): any;
    /**
     * Get page label ranges from the document.
     *
     * @returns Array of {start_page, style, prefix, start_value} objects, or empty array
     */
    pageLabels(): any;
    /**
     * Get the MediaBox of a page as [llx, lly, urx, ury].
     */
    pageMediaBox(page_index: number): Float32Array;
    /**
     * Get the rotation of a page in degrees (0, 90, 180, 270).
     */
    pageRotation(page_index: number): number;
    /**
     * Number of redaction regions queued for `page`.
     */
    redactionCount(page: number): number;
    /**
     * Identify and remove both headers and footers.
     *
     * Prioritizes ISO 32000 spec-compliant /Artifact tags, with a heuristic
     * fallback for untagged PDFs.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     */
    removeArtifacts(threshold: number): number;
    /**
     * Identify and remove footers.
     *
     * Uses spec-compliant /Artifact tags when available (100% accuracy), or
     * falls back to heuristic analysis of the bottom 15% of pages.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     */
    removeFooters(threshold: number): number;
    /**
     * Identify and remove headers.
     *
     * Uses spec-compliant /Artifact tags when available (100% accuracy), or
     * falls back to heuristic analysis of the top 15% of pages.
     *
     * @param threshold - Fraction of pages (0.0-1.0) where text must repeat (heuristic mode)
     */
    removeHeaders(threshold: number): number;
    /**
     * Render a page to an image (PNG).
     *
     * Requires the `rendering` feature.
     *
     * @param page_index - Zero-based page number
     * @param dpi - Dots per inch (default: 150)
     * @returns Uint8Array containing the PNG image data
     */
    renderPage(page_index: number, dpi?: number | null): Uint8Array;
    /**
     * Render a page fitted to a pixel bounding box.
     * Returns a JS object: { width: number, height: number, data: Uint8Array }
     * where data is raw image bytes (PNG format, same as renderPage).
     */
    renderPageFit(page_index: number, fit_width: number, fit_height: number): any;
    /**
     * Reposition an image on a page.
     */
    repositionImage(page_index: number, name: string, x: number, y: number): void;
    /**
     * Resize an image on a page.
     */
    resizeImage(page_index: number, name: string, width: number, height: number): void;
    /**
     * Rotate all pages by the given degrees.
     */
    rotateAllPages(degrees: number): void;
    /**
     * Rotate a page by the given degrees (adds to current rotation).
     */
    rotatePage(page_index: number, degrees: number): void;
    /**
     * Standalone document sanitization (#231 T10): strip `/Info`,
     * catalog XMP `/Metadata`, document JavaScript and embedded files
     * without geometric redaction. Returns a `RedactionReport` object.
     */
    sanitizeDocument(scrub_metadata?: boolean | null, remove_javascript?: boolean | null, remove_embedded_files?: boolean | null): any;
    /**
     * Save all edits and return the resulting PDF as bytes.
     *
     * @returns Uint8Array containing the modified PDF
     */
    save(): Uint8Array;
    /**
     * Save with encryption and return the resulting PDF as bytes.
     */
    saveEncryptedToBytes(user_password: string, owner_password?: string | null, allow_print?: boolean | null, allow_copy?: boolean | null, allow_modify?: boolean | null, allow_annotate?: boolean | null): Uint8Array;
    /**
     * Save the modified PDF and return as bytes.
     * `saveToBytes()` is the original method; `save()` is a convenience alias.
     *
     * @returns Uint8Array containing the modified PDF
     */
    saveToBytes(): Uint8Array;
    /**
     * Save by streaming output chunks through a JS callback function.
     *
     * @param write_fn - JS function: (chunk: Uint8Array) => void
     * @param options - Optional save options (compress, garbageCollect, linearize)
     */
    saveToWriter(write_fn: Function, compress?: boolean | null, garbage_collect?: boolean | null, linearize?: boolean | null): void;
    /**
     * Save with options (compress, garbage_collect, linearize) and return bytes.
     *
     * @param {Object} [options] - Optional save options.
     * @param {boolean} [options.compress=true] - Compress raw streams with FlateDecode.
     * @param {boolean} [options.garbageCollect=true] - Remove unreachable objects.
     * @param {boolean} [options.linearize=false] - Linearize (reserved, no-op).
     * @returns Uint8Array containing the modified PDF
     */
    saveWithOptions(compress?: boolean | null, garbage_collect?: boolean | null, linearize?: boolean | null): Uint8Array;
    /**
     * Search for text across all pages.
     *
     * @param pattern - Regex pattern or literal text to search for
     * @param case_insensitive - Case insensitive search (default: false)
     * @param literal - Treat pattern as literal text, not regex (default: false)
     * @param whole_word - Match whole words only (default: false)
     * @param max_results - Maximum results to return, 0 = unlimited (default: 0)
     *
     * Returns an array of {page, text, bbox, start_index, end_index, span_boxes}.
     */
    search(pattern: string, case_insensitive?: boolean | null, literal?: boolean | null, whole_word?: boolean | null, max_results?: number | null): any;
    /**
     * Search for text on a specific page.
     */
    searchPage(page_index: number, pattern: string, case_insensitive?: boolean | null, literal?: boolean | null, whole_word?: boolean | null, max_results?: number | null): any;
    /**
     * Set the document author.
     */
    setAuthor(author: string): void;
    /**
     * Set the value of a form field.
     *
     * @param name - Full qualified field name
     * @param value - New value: string for text fields, boolean for checkboxes
     */
    setFormFieldValue(name: string, value: any): void;
    /**
     * Set the complete bounds of an image on a page.
     */
    setImageBounds(page_index: number, name: string, x: number, y: number, width: number, height: number): void;
    /**
     * Set the document keywords.
     */
    setKeywords(keywords: string): void;
    /**
     * Set the CropBox of a page.
     */
    setPageCropBox(page_index: number, llx: number, lly: number, urx: number, ury: number): void;
    /**
     * Set the MediaBox of a page.
     */
    setPageMediaBox(page_index: number, llx: number, lly: number, urx: number, ury: number): void;
    /**
     * Set the rotation of a page (0, 90, 180, or 270 degrees).
     */
    setPageRotation(page_index: number, degrees: number): void;
    /**
     * Set the document subject.
     */
    setSubject(subject: string): void;
    /**
     * Set the document title.
     */
    setTitle(title: string): void;
    /**
     * Count existing PDF signatures. Returns 0 when the document has
     * no AcroForm or no signed signature fields.
     */
    signatureCount(): number;
    /**
     * Enumerate existing PDF signatures. Each entry is a
     * `WasmSignature` (inspection-only) mirroring the C# and Python
     * Signature surfaces.
     */
    signatures(): WasmSignature[];
    /**
     * Convert the entire PDF to DOCX bytes (Uint8Array).
     */
    toDocxBytes(): Uint8Array;
    /**
     * Convert a single page to HTML.
     *
     * @param page_index - Zero-based page number
     * @param preserve_layout - Use CSS positioning to preserve layout (default: false)
     * @param detect_headings - Whether to detect headings (default: true)
     */
    toHtml(page_index: number, preserve_layout?: boolean | null, detect_headings?: boolean | null, include_form_fields?: boolean | null): string;
    /**
     * Convert all pages to HTML.
     */
    toHtmlAll(preserve_layout?: boolean | null, detect_headings?: boolean | null, include_form_fields?: boolean | null): string;
    /**
     * Convert a single page to Markdown.
     *
     * @param page_index - Zero-based page number
     * @param detect_headings - Whether to detect headings (default: true)
     * @param include_images - Whether to include images (default: false)
     */
    toMarkdown(page_index: number, detect_headings?: boolean | null, include_images?: boolean | null, include_form_fields?: boolean | null): string;
    /**
     * Convert all pages to Markdown.
     */
    toMarkdownAll(detect_headings?: boolean | null, include_images?: boolean | null, include_form_fields?: boolean | null): string;
    /**
     * Convert a single page to plain text (with layout preservation options).
     */
    toPlainText(page_index: number): string;
    /**
     * Convert all pages to plain text.
     */
    toPlainTextAll(): string;
    /**
     * Convert the entire PDF to PPTX bytes (Uint8Array).
     */
    toPptxBytes(): Uint8Array;
    /**
     * Convert the entire PDF to XLSX bytes (Uint8Array).
     */
    toXlsxBytes(): Uint8Array;
    /**
     * LOCAL PATCH — pdf_manipulator/0.3.47-patches
     * Unembed Standard 14 fonts.
     * Removal trigger: upstream adds font optimization to WasmPdfDocument.
     */
    unembedStandardFonts(): number;
    /**
     * Validate PDF/A compliance. Level: "1b", "2b", etc.
     */
    validatePdfA(level: string): any;
    /**
     * Validate PDF/UA accessibility compliance.
     */
    validatePdfUa(level?: string | null): any;
    /**
     * Validate PDF/X print production compliance.
     */
    validatePdfX(level?: string | null): any;
    /**
     * Get the PDF version as [major, minor].
     */
    version(): Uint8Array;
    /**
     * Focus extraction on a specific rectangular region of a page (v0.3.14).
     *
     * @param page_index - Zero-based page number
     * @param region - [x, y, width, height] in points
     */
    within(page_index: number, region: Float32Array): WasmPdfPageRegion;
    /**
     * Get XMP metadata from the document.
     *
     * @returns Object with XMP fields (dc_title, dc_creator, etc.) or null if no XMP
     */
    xmpMetadata(): any;
}

/**
 * A focused view of a PDF page region for scoped extraction (v0.3.14).
 */
export class WasmPdfPageRegion {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Extract character-level data from this region.
     */
    extractChars(): any;
    /**
     * Extract images from this region.
     */
    extractImages(): any;
    /**
     * Extract straight lines from this region.
     */
    extractLines(): any;
    /**
     * Extract vector paths from this region.
     */
    extractPaths(): any;
    /**
     * Extract rectangles from this region.
     */
    extractRects(): any;
    /**
     * Extract tables from this region.
     */
    extractTables(): any;
    /**
     * Extract text from this region.
     */
    extractText(): string;
    /**
     * Extract text lines from this region.
     */
    extractTextLines(): any;
    /**
     * Extract text using OCR from this region.
     *
     * Region-scoped OCR is not wired yet; use the page-level
     * `WasmPdfDocument.extractTextOcr(pageIndex, engine)` for now
     * (#524 follow-up).
     */
    extractTextOcr(_engine?: WasmOcrEngine | null): string;
    /**
     * Extract words from this region.
     */
    extractWords(): any;
}

/**
 * A single existing PDF signature surfaced by
 * `WasmPdfDocument.signatures()`. `verify()` runs the signer-attributes
 * check; `verifyDetached()` adds the `messageDigest` content-hash check.
 * Supported: RSA-PKCS#1 v1.5, RSA-PSS, ECDSA P-256/P-384.
 */
export class WasmSignature {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Run the RFC 5652 §5.4 signer-attributes crypto check. Today
     * this covers RSA-PKCS#1 v1.5 over SHA-1/256/384/512 — the
     * padding used by essentially every PDF signature.
     *
     * A `true` return proves the signer held the private key matching
     * the embedded certificate and that the signed-attribute bundle
     * is authentic. It does **not** verify the `messageDigest`
     * attribute against the document's byte-range content hash —
     * call `verifyDetached()` for that end-to-end check.
     *
     * Throws for RSA-PSS, ECDSA, unknown digest OIDs, or signatures
     * without signed_attrs.
     */
    verify(): boolean;
    /**
     * End-to-end detached-signature verification. Runs both the
     * signer-attributes RSA-PKCS#1 v1.5 crypto check AND the RFC 5652
     * §11.2 `messageDigest` check against the segment of `pdfData`
     * this signature covers (extracted via `/ByteRange`).
     *
     * `pdfData` must be the full PDF file. A `true` result proves
     * both the signer is authentic and that the document's byte-range
     * content has not been altered since signing. `false` means one
     * of the two checks failed (wrong key or tampered content).
     *
     * Throws for RSA-PSS, ECDSA, unknown digest OIDs, or CMS blobs
     * missing `signed_attrs` / `messageDigest`.
     */
    verifyDetached(pdf_data: Uint8Array): boolean;
    /**
     * `/ContactInfo` entry from the signature dictionary, if present.
     */
    readonly contactInfo: string | undefined;
    /**
     * True iff `/ByteRange` is a 4-element array covering the whole
     * document (i.e. the signature protects every byte of the file).
     */
    readonly coversWholeDocument: boolean;
    /**
     * `/Location` entry from the signature dictionary, if present.
     */
    readonly location: string | undefined;
    /**
     * PAdES baseline level from this signature's CMS attributes alone
     * (`BB` vs `BT`). `BLt` additionally needs the document `/DSS` —
     * read it via `WasmPdfDocument.dss()` and re-classify there.
     */
    readonly padesLevel: PadesLevel;
    /**
     * `/Reason` entry from the signature dictionary, if present.
     */
    readonly reason: string | undefined;
    /**
     * `/Name` entry from the signature dictionary, if present.
     */
    readonly signerName: string | undefined;
    /**
     * Unix epoch (seconds). `None` if the `/M` entry is missing or
     * unparseable.
     */
    readonly signingTime: bigint | undefined;
}

/**
 * RFC 3161 timestamp parsed from a DER TimeStampToken or bare
 * TSTInfo. Mirrors the C#, Go, and Python `Timestamp` surfaces.
 */
export class WasmTimestamp {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Parse a DER blob that may be either a full TimeStampToken or
     * the bare TSTInfo SEQUENCE.
     */
    static parse(data: Uint8Array): WasmTimestamp;
    /**
     * Cryptographically verify this TimeStampToken.
     *
     * Returns `true` when the TSA's signature and `messageDigest` both pass.
     * Returns `false` when a crypto check fails (tampered token or wrong key).
     * Throws when the token is not CMS-wrapped or uses an unsupported algorithm.
     */
    verify(): boolean;
    /**
     * Hash algorithm enum value (1=SHA1, 2=SHA256, 3=SHA384,
     * 4=SHA512, 0=unknown).
     */
    readonly hashAlgorithm: number;
    /**
     * Raw message-imprint hash bytes.
     */
    readonly messageImprint: Uint8Array;
    /**
     * TSA policy OID in dotted-decimal form.
     */
    readonly policyOid: string;
    /**
     * Serial number as a hex string (no `0x` prefix).
     */
    readonly serial: string;
    /**
     * Generation time as Unix epoch seconds.
     */
    readonly time: bigint;
    /**
     * TSA name from the token (may be empty).
     */
    readonly tsaName: string;
}

/**
 * A CycloneDX 1.6 Cryptographic Bill of Materials (JSON string) of the
 * algorithms exercised so far this process (#230 Phase F).
 */
export function cryptoCbom(): string;

/**
 * The cryptographic algorithm tokens exercised so far this process
 * (governance report), as a JSON string array.
 */
export function cryptoInventory(): any;

/**
 * The active crypto policy as its canonical grammar string.
 */
export function cryptoPolicy(): string;

/**
 * Disable all pdf_oxide log output — convenience wrapper for
 * `setLogLevel("off")`.
 */
export function disableLogging(): void;

/**
 * Generate a 1D barcode as an SVG string.
 *
 * `barcodeType`: 0=Code128, 1=Code39, 2=EAN13, 3=EAN8, 4=UPCA, 5=ITF, 6=Code93, 7=Codabar.
 */
export function generateBarcodeSvg(barcode_type: number, data: string): string;

/**
 * Generate a QR code as an SVG string.
 *
 * `errorCorrection`: 0=Low, 1=Medium, 2=Quartile, 3=High. `size`: advisory pixel size.
 */
export function generateQrSvg(data: string, error_correction: number, size: number): string;

/**
 * Whether `pdf_data` carries a document-scoped RFC 3161
 * `/DocTimeStamp` archival timestamp (PAdES-B-LTA). This is the
 * document-level reader signal; a `WasmSignature`'s `padesLevel`
 * getter is signature-scoped and tops out at B-LT by design.
 */
export function hasDocumentTimestamp(pdf_data: Uint8Array): boolean;

/**
 * #519: Air-gapped OCR model manifest — JSON (detector + every
 * supported language's cache filenames and source URLs).
 *
 * WASM provisioning is **host-side**: browser/WASM has no filesystem
 * or network-to-disk, so a download-to-cache prefetch cannot run
 * here. This manifest is informational — it lets the JS host learn
 * which model files/URLs to fetch and bundle (or ship out of band)
 * before driving OCR. There is intentionally no `prefetchModels` in
 * the WASM surface (see `prefetchAvailable`, which always returns
 * `false`).
 */
export function modelManifest(): string;

/**
 * Plan a bookmark split without producing PDFs. Returns a JSON array
 * of segment objects (`index, startPage…` shape from
 * `BookmarkSegment`). `level`: 0 = all depths, 1 = top-level.
 */
export function planSplitByBookmarks(src_bytes: Uint8Array, title_prefix: string | null | undefined, ignore_case: boolean, level: number, include_front_matter: boolean): any;

/**
 * #519: Whether this build can download OCR models to a local cache.
 * Always `false` in WASM — provisioning is host-side (see
 * `modelManifest`).
 */
export function prefetchAvailable(): boolean;

/**
 * Install the process-wide runtime crypto policy from its grammar
 * string (`"compat"|"strict"|"fips-strict"[;…]`). Fail-closed:
 * throws on an unparseable spec (policy NOT installed) or if a
 * policy is already set. Default (never set) is `compat`.
 */
export function setCryptoPolicy(spec: string): void;

/**
 * Set the maximum log level for pdf_oxide messages.
 *
 * Accepts one of: `"off"`, `"error"`, `"warn"` / `"warning"`, `"info"`,
 * `"debug"`, `"trace"`. Case-insensitive. Default is `"off"` — the library
 * is silent unless explicitly enabled.
 *
 * Logs are forwarded to the browser console (`console.log`, `console.warn`,
 * `console.error`, etc.). Fixes issue #280.
 *
 * @example
 * ```javascript
 * import init, { setLogLevel } from "pdf-oxide-wasm";
 * await init();
 * setLogLevel("warn");
 * ```
 */
export function setLogLevel(level: string): void;

/**
 * Sign raw PDF bytes and return the signed PDF as a `Uint8Array`.
 *
 * `cert` must carry a private key (loaded via `Certificate.loadPem` or
 * `Certificate.loadPkcs12`).
 */
export function signPdfBytes(pdf_data: Uint8Array, cert: WasmCertificate, reason?: string | null, location?: string | null): Uint8Array;

/**
 * Sign raw PDF bytes at a PAdES baseline level and return the signed
 * PDF as a `Uint8Array`.
 *
 * `level` `BLTA` is reserved (→ error). For `BT`/`BLt` pass a
 * pre-fetched RFC 3161 `timestampToken` (DER): WASM intentionally
 * omits the online TSA client (same `ureq`-incompat carve-out as
 * v0.3.38) — without a token the core fail-closes with `Unsupported`.
 * `revocation` supplies the B-LT DSS material.
 */
export function signPdfBytesPades(pdf_data: Uint8Array, cert: WasmCertificate, level: PadesLevel, timestamp_token?: Uint8Array | null, revocation?: RevocationMaterial | null, reason?: string | null, location?: string | null): Uint8Array;

/**
 * Sign PDF bytes with PEM certificate + key. All data stays inside WASM.
 */
export function signPdfWithPem(pdf_data: Uint8Array, cert_pem: string, key_pem: string, reason?: string | null, location?: string | null): Uint8Array;

/**
 * Sign PDF bytes with a PKCS#12 certificate. All data stays inside WASM —
 * no cross-boundary borrows that can be invalidated by memory growth.
 */
export function signPdfWithPkcs12(pdf_data: Uint8Array, pkcs12_data: Uint8Array, password: string, reason?: string | null, location?: string | null): Uint8Array;

/**
 * Split at bookmark boundaries. Returns a JSON array of
 * `[segment, bytes]` pairs (bytes as a number array; source
 * unmodified).
 */
export function splitByBookmarks(src_bytes: Uint8Array, title_prefix: string | null | undefined, ignore_case: boolean, level: number, include_front_matter: boolean): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly setLogLevel: (a: number, b: number, c: number) => void;
    readonly generateBarcodeSvg: (a: number, b: number, c: number, d: number) => void;
    readonly generateQrSvg: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly planSplitByBookmarks: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly splitByBookmarks: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly setCryptoPolicy: (a: number, b: number, c: number) => void;
    readonly cryptoPolicy: (a: number) => void;
    readonly cryptoInventory: (a: number) => void;
    readonly cryptoCbom: (a: number) => void;
    readonly modelManifest: (a: number) => void;
    readonly prefetchAvailable: () => number;
    readonly __wbg_wasmpdfdocument_free: (a: number, b: number) => void;
    readonly wasmpdfdocument_new: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_fromReader: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_editorFromReader: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_pageCount: (a: number, b: number) => void;
    readonly wasmpdfdocument_signatureCount: (a: number, b: number) => void;
    readonly wasmpdfdocument_signatures: (a: number, b: number) => void;
    readonly wasmpdfdocument_dss: (a: number, b: number) => void;
    readonly wasmpdfdocument_version: (a: number, b: number) => void;
    readonly wasmpdfdocument_authenticate: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_hasStructureTree: (a: number, b: number) => void;
    readonly wasmpdfdocument_extractText: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_extractAllText: (a: number, b: number) => void;
    readonly wasmpdfdocument_removeHeaders: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_removeFooters: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_removeArtifacts: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_eraseHeader: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_editHeader: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_eraseFooter: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_editFooter: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_eraseArtifacts: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_within: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_renderPage: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_renderPageFit: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_toMarkdown: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpdfdocument_toMarkdownAll: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_toHtml: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpdfdocument_toHtmlAll: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_toPlainText: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_toPlainTextAll: (a: number, b: number) => void;
    readonly wasmpdfdocument_toDocxBytes: (a: number, b: number) => void;
    readonly wasmpdfdocument_toPptxBytes: (a: number, b: number) => void;
    readonly wasmpdfdocument_toXlsxBytes: (a: number, b: number) => void;
    readonly wasmpdfdocument_openFromDocxBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_openFromPptxBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_openFromXlsxBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_extractChars: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractSpans: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_extractPageText: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractWords: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractTextLines: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractTables: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_search: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly wasmpdfdocument_searchPage: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmpdfdocument_extractImages: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_getOutline: (a: number, b: number) => void;
    readonly wasmpdfdocument_getAnnotations: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_extractPaths: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractRects: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractLines: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly __wbg_wasmcertificate_free: (a: number, b: number) => void;
    readonly wasmcertificate_load: (a: number, b: number, c: number) => void;
    readonly wasmcertificate_loadPem: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmcertificate_loadPkcs12: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmcertificate_subject: (a: number, b: number) => void;
    readonly wasmcertificate_issuer: (a: number, b: number) => void;
    readonly wasmcertificate_serial: (a: number, b: number) => void;
    readonly wasmcertificate_validity: (a: number, b: number) => void;
    readonly wasmcertificate_isValid: (a: number, b: number) => void;
    readonly signPdfBytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly __wbg_revocationmaterial_free: (a: number, b: number) => void;
    readonly wasmrevocationmaterial_new: () => number;
    readonly wasmrevocationmaterial_addCert: (a: number, b: number, c: number) => void;
    readonly wasmrevocationmaterial_addCrl: (a: number, b: number, c: number) => void;
    readonly wasmrevocationmaterial_addOcsp: (a: number, b: number, c: number) => void;
    readonly __wbg_dss_free: (a: number, b: number) => void;
    readonly wasmdss_certCount: (a: number) => number;
    readonly wasmdss_getCert: (a: number, b: number, c: number) => void;
    readonly wasmdss_getCrl: (a: number, b: number, c: number) => void;
    readonly wasmdss_getOcsp: (a: number, b: number, c: number) => void;
    readonly wasmdss_vri: (a: number, b: number) => void;
    readonly hasDocumentTimestamp: (a: number, b: number) => number;
    readonly signPdfBytesPades: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly signPdfWithPkcs12: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly signPdfWithPem: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly __wbg_wasmtimestamp_free: (a: number, b: number) => void;
    readonly wasmtimestamp_parse: (a: number, b: number, c: number) => void;
    readonly wasmtimestamp_time: (a: number) => bigint;
    readonly wasmtimestamp_serial: (a: number, b: number) => void;
    readonly wasmtimestamp_policyOid: (a: number, b: number) => void;
    readonly wasmtimestamp_tsaName: (a: number, b: number) => void;
    readonly wasmtimestamp_hashAlgorithm: (a: number) => number;
    readonly wasmtimestamp_messageImprint: (a: number, b: number) => void;
    readonly wasmtimestamp_verify: (a: number, b: number) => void;
    readonly __wbg_wasmsignature_free: (a: number, b: number) => void;
    readonly wasmsignature_signerName: (a: number, b: number) => void;
    readonly wasmsignature_reason: (a: number, b: number) => void;
    readonly wasmsignature_location: (a: number, b: number) => void;
    readonly wasmsignature_contactInfo: (a: number, b: number) => void;
    readonly wasmsignature_signingTime: (a: number, b: number) => void;
    readonly wasmsignature_coversWholeDocument: (a: number) => number;
    readonly wasmsignature_padesLevel: (a: number) => number;
    readonly wasmsignature_verify: (a: number, b: number) => void;
    readonly wasmsignature_verifyDetached: (a: number, b: number, c: number, d: number) => void;
    readonly __wbg_wasmpdfpageregion_free: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractText: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractChars: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractWords: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractTextLines: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractTables: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractImages: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractPaths: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractRects: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractLines: (a: number, b: number) => void;
    readonly wasmpdfpageregion_extractTextOcr: (a: number, b: number, c: number) => void;
    readonly __wbg_wasmocrconfig_free: (a: number, b: number) => void;
    readonly wasmocrengine_new: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly wasmpdfdocument_extractTextOcr: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_getFormFields: (a: number, b: number) => void;
    readonly wasmpdfdocument_hasXfa: (a: number, b: number) => void;
    readonly wasmpdfdocument_exportFormData: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_getFormFieldValue: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_setFormFieldValue: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_extractImageBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_flattenForms: (a: number, b: number) => void;
    readonly wasmpdfdocument_flattenFormsOnPage: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_flattenWarnings: (a: number, b: number) => void;
    readonly wasmpdfdocument_mergeFrom: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_embedFile: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpdfdocument_pageLabels: (a: number, b: number) => void;
    readonly wasmpdfdocument_xmpMetadata: (a: number, b: number) => void;
    readonly wasmpdfdocument_getTitle: (a: number, b: number) => void;
    readonly wasmpdfdocument_getAuthor: (a: number, b: number) => void;
    readonly wasmpdfdocument_getSubject: (a: number, b: number) => void;
    readonly wasmpdfdocument_getKeywords: (a: number, b: number) => void;
    readonly wasmpdfdocument_setTitle: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_setAuthor: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_setSubject: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_setKeywords: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_pageRotation: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_setPageRotation: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_rotatePage: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_rotateAllPages: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_pageMediaBox: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_setPageMediaBox: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_pageCropBox: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_setPageCropBox: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_cropMargins: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpdfdocument_eraseRegion: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_eraseRegions: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_clearEraseRegions: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_flattenPageAnnotations: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_flattenAllAnnotations: (a: number, b: number) => void;
    readonly wasmpdfdocument_addWatermark: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly wasmpdfdocument_unembedStandardFonts: (a: number, b: number) => void;
    readonly wasmpdfdocument_addStamp: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => void;
    readonly wasmpdfdocument_addImageStamp: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmpdfdocument_addWatermarkPositioned: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number, q: number) => void;
    readonly wasmpdfdocument_applyPageRedactions: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_applyAllRedactions: (a: number, b: number) => void;
    readonly wasmpdfdocument_addRedaction: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmpdfdocument_redactionCount: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_applyRedactionsDestructive: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_sanitizeDocument: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly __wbg_artifactstyle_free: (a: number, b: number) => void;
    readonly wasmartifactstyle_new: () => number;
    readonly wasmartifactstyle_font: (a: number, b: number, c: number, d: number) => number;
    readonly wasmartifactstyle_bold: (a: number) => number;
    readonly wasmartifactstyle_color: (a: number, b: number, c: number, d: number) => number;
    readonly __wbg_wasmartifact_free: (a: number, b: number) => void;
    readonly wasmartifact_new: () => number;
    readonly wasmartifact_left: (a: number, b: number) => number;
    readonly wasmartifact_center: (a: number, b: number) => number;
    readonly wasmartifact_right: (a: number, b: number) => number;
    readonly wasmartifact_withStyle: (a: number, b: number) => number;
    readonly wasmartifact_withOffset: (a: number, b: number) => number;
    readonly __wbg_wasmpagetemplate_free: (a: number, b: number) => void;
    readonly wasmpagetemplate_new: () => number;
    readonly wasmpagetemplate_header: (a: number, b: number) => number;
    readonly wasmpagetemplate_footer: (a: number, b: number) => number;
    readonly wasmpagetemplate_skipFirstPage: (a: number) => number;
    readonly wasmpdfdocument_pageImages: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_repositionImage: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_resizeImage: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdfdocument_setImageBounds: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmpdfdocument_save: (a: number, b: number) => void;
    readonly wasmpdfdocument_saveWithOptions: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmpdfdocument_saveToWriter: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpdfdocument_saveEncryptedToBytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmpdfdocument_validatePdfA: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_convertToPdfA: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_validatePdfUa: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_validatePdfX: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_deletePage: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_movePage: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_extractPages: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdfdocument_flattenToImages: (a: number, b: number, c: number) => void;
    readonly __wbg_wasmpdf_free: (a: number, b: number) => void;
    readonly wasmpdf_fromBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdf_mergeFromReaders: (a: number, b: number, c: number, d: number) => void;
    readonly wasmpdf_merge: (a: number, b: number, c: number) => void;
    readonly wasmpdf_fromMarkdown: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdf_fromHtml: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdf_fromText: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdf_fromImageBytes: (a: number, b: number, c: number) => void;
    readonly wasmpdf_fromMultipleImageBytes: (a: number, b: number) => void;
    readonly wasmpdf_toBytes: (a: number, b: number) => void;
    readonly wasmpdf_size: (a: number) => number;
    readonly wasmpdf_fromHtmlCss: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmpdf_fromHtmlCssWithFonts: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly __wbg_wasmembeddedfont_free: (a: number, b: number) => void;
    readonly wasmembeddedfont_fromBytes: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmembeddedfont_name: (a: number, b: number) => void;
    readonly __wbg_wasmdocumentbuilder_free: (a: number, b: number) => void;
    readonly wasmdocumentbuilder_new: () => number;
    readonly wasmdocumentbuilder_title: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_author: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_subject: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_keywords: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_creator: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_onOpen: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_taggedPdfUa1: (a: number, b: number) => void;
    readonly wasmdocumentbuilder_language: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_roleMap: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmdocumentbuilder_registerEmbeddedFont: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmdocumentbuilder_a4Page: (a: number, b: number) => void;
    readonly wasmdocumentbuilder_letterPage: (a: number, b: number) => void;
    readonly wasmdocumentbuilder_page: (a: number, b: number, c: number, d: number) => void;
    readonly wasmdocumentbuilder_commitPage: (a: number, b: number, c: number) => void;
    readonly wasmdocumentbuilder_build: (a: number, b: number) => void;
    readonly wasmdocumentbuilder_toBytesEncrypted: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly __wbg_wasmfluentpagebuilder_free: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_font: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_at: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_text: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_heading: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_paragraph: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_space: (a: number, b: number, c: number) => void;
    readonly wasmfluentpagebuilder_horizontalRule: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_linkUrl: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_linkPage: (a: number, b: number, c: number) => void;
    readonly wasmfluentpagebuilder_linkNamed: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_linkJavascript: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_onOpen: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_onClose: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_fieldKeystroke: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_fieldFormat: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_fieldValidate: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_fieldCalculate: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_highlight: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_underline: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_strikeout: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_squiggly: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmfluentpagebuilder_stickyNote: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_stickyNoteAt: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmfluentpagebuilder_watermark: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_watermarkConfidential: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_watermarkDraft: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_stamp: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_freeText: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly wasmfluentpagebuilder_textField: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmfluentpagebuilder_checkbox: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmfluentpagebuilder_comboBox: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => void;
    readonly wasmfluentpagebuilder_radioGroup: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number, o: number, p: number) => void;
    readonly wasmfluentpagebuilder_pushButton: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmfluentpagebuilder_signatureField: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly wasmfluentpagebuilder_footnote: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmfluentpagebuilder_columns: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmfluentpagebuilder_inline: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_inlineBold: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_inlineItalic: (a: number, b: number, c: number, d: number) => void;
    readonly wasmfluentpagebuilder_inlineColor: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmfluentpagebuilder_newline: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_barcode1d: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmfluentpagebuilder_barcodeQr: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmfluentpagebuilder_imageWithAlt: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmfluentpagebuilder_imageArtifact: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly wasmfluentpagebuilder_rect: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmfluentpagebuilder_filledRect: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmfluentpagebuilder_line: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmfluentpagebuilder_measure: (a: number, b: number, c: number) => number;
    readonly wasmfluentpagebuilder_remainingSpace: (a: number) => number;
    readonly wasmfluentpagebuilder_textInRect: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => void;
    readonly wasmfluentpagebuilder_strokeRect: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmfluentpagebuilder_strokeRectDashed: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
    readonly wasmfluentpagebuilder_strokeLine: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => void;
    readonly wasmfluentpagebuilder_strokeLineDashed: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => void;
    readonly wasmfluentpagebuilder_newPageSameSize: (a: number, b: number) => void;
    readonly wasmfluentpagebuilder_table: (a: number, b: number, c: number) => void;
    readonly wasmfluentpagebuilder_streamingTable: (a: number, b: number, c: number) => void;
    readonly wasmfluentpagebuilder_done: (a: number, b: number, c: number) => void;
    readonly __wbg_streamingtable_free: (a: number, b: number) => void;
    readonly streamingtable_columnCount: (a: number) => number;
    readonly streamingtable_pendingRowCount: (a: number) => number;
    readonly streamingtable_batchCount: (a: number) => number;
    readonly streamingtable_pushRow: (a: number, b: number, c: number, d: number) => void;
    readonly streamingtable_pushRowSpan: (a: number, b: number, c: number) => void;
    readonly streamingtable_flush: (a: number) => void;
    readonly streamingtable_finish: (a: number, b: number) => void;
    readonly wasmpdfdocument_classifyDocument: (a: number, b: number) => void;
    readonly wasmpdfdocument_classifyPage: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_extractTextAuto: (a: number, b: number, c: number) => void;
    readonly wasmpdfdocument_extractPageAuto: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmdss_crlCount: (a: number) => number;
    readonly wasmdss_ocspCount: (a: number) => number;
    readonly wasmheader_left: (a: number, b: number) => number;
    readonly wasmheader_center: (a: number, b: number) => number;
    readonly wasmheader_right: (a: number, b: number) => number;
    readonly wasmfooter_left: (a: number, b: number) => number;
    readonly wasmfooter_center: (a: number, b: number) => number;
    readonly wasmfooter_right: (a: number, b: number) => number;
    readonly wasmheader_new: () => number;
    readonly wasmfooter_new: () => number;
    readonly disableLogging: () => void;
    readonly __wbg_wasmocrengine_free: (a: number, b: number) => void;
    readonly wasmpdfdocument_saveToBytes: (a: number, b: number) => void;
    readonly wasmocrconfig_new: () => number;
    readonly __wbg_wasmheader_free: (a: number, b: number) => void;
    readonly __wbg_wasmfooter_free: (a: number, b: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
