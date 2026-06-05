/**
 * DocumentEditor — thin TS wrapper around the N-API `editor*` exports
 * in `binding.cc`. Mirrors the C# `DocumentEditor` / Go
 * `DocumentEditor` surface.
 *
 * Every mutation is a synchronous call into the Rust core; the same
 * handle is carried until {@link DocumentEditor.close}. Throws plain
 * `Error` with the native message on failure.
 *
 * ```ts
 * import { DocumentEditor } from 'pdf-oxide';
 *
 * const editor = DocumentEditor.open('in.pdf');
 * try {
 *   editor.mergeFrom('other.pdf');
 *   editor.deletePage(0);
 *   editor.movePage(0, 2);
 *   editor.setPageRotation(0, 90);
 *   editor.save('out.pdf');
 * } finally {
 *   editor.close();
 * }
 * ```
 */
// Load the native addon via the shared prebuild-aware loader.
// Importing `./index.js` would create an ESM cycle (index.js imports
// us back), so we go through `./native.js` — same resolver, no cycle,
// resolves against `prebuilds/<triple>/pdf_oxide.node` in the
// published package.
import { loadNative } from './native.js';

const native = loadNative();

/**
 * Page rotation angles valid for {@link DocumentEditor.setPageRotation}.
 */
export type PageRotation = 0 | 90 | 180 | 270;

/**
 * PDF editor bound to a concrete file on disk. Open via
 * {@link DocumentEditor.open}; always pair with {@link close} (or use
 * `using` / the explicit-resource-management protocol once it's
 * stable in your toolchain).
 */
export class DocumentEditor {
  private _handle: any;
  private _closed = false;

  private constructor(handle: any) {
    this._handle = handle;
  }

  /** Open a PDF file for editing. */
  static open(path: string): DocumentEditor {
    if (typeof path !== 'string' || path.length === 0) {
      throw new TypeError('path must be a non-empty string');
    }
    const handle = native.editorOpen(path);
    return new DocumentEditor(handle);
  }

  /** Open a PDF from an in-memory buffer for editing. */
  static openFromBytes(data: Buffer | Uint8Array): DocumentEditor {
    if (!data || data.length === 0) {
      throw new TypeError('data must be a non-empty Buffer or Uint8Array');
    }
    const handle = native.editorOpenFromBytes(data);
    return new DocumentEditor(handle);
  }

  /** True if the editor has been closed. Subsequent calls will throw. */
  get closed(): boolean {
    return this._closed;
  }

  private _throwIfClosed(): void {
    if (this._closed) throw new Error('DocumentEditor is closed');
  }

  /** Current page count. */
  pageCount(): number {
    this._throwIfClosed();
    return native.editorGetPageCount(this._handle);
  }

  /** True if the editor has unsaved modifications. */
  isModified(): boolean {
    this._throwIfClosed();
    return native.editorIsModified(this._handle);
  }

  // ----- metadata ---------------------------------------------------

  setTitle(title: string): void {
    this._throwIfClosed();
    native.editorSetTitle(this._handle, title);
  }

  setAuthor(author: string): void {
    this._throwIfClosed();
    native.editorSetAuthor(this._handle, author);
  }

  setSubject(subject: string): void {
    this._throwIfClosed();
    native.editorSetSubject(this._handle, subject);
  }

  getKeywords(): string | null {
    this._throwIfClosed();
    return native.editorGetKeywords(this._handle);
  }

  setKeywords(keywords: string): void {
    this._throwIfClosed();
    native.editorSetKeywords(this._handle, keywords);
  }

  getProducer(): string {
    this._throwIfClosed();
    return native.editorGetProducer(this._handle);
  }

  setProducer(producer: string): void {
    this._throwIfClosed();
    native.editorSetProducer(this._handle, producer);
  }

  getCreationDate(): string {
    this._throwIfClosed();
    return native.editorGetCreationDate(this._handle);
  }

  setCreationDate(date: string): void {
    this._throwIfClosed();
    native.editorSetCreationDate(this._handle, date);
  }

  // ----- page mutations ---------------------------------------------

  /** Delete the page at `pageIndex` (zero-based). */
  deletePage(pageIndex: number): void {
    this._throwIfClosed();
    native.editorDeletePage(this._handle, pageIndex);
  }

  /** Move a page. Indices refer to positions before the move. */
  movePage(fromIndex: number, toIndex: number): void {
    this._throwIfClosed();
    native.editorMovePage(this._handle, fromIndex, toIndex);
  }

  /** Set rotation on a page (0/90/180/270). */
  setPageRotation(pageIndex: number, degrees: PageRotation): void {
    this._throwIfClosed();
    native.editorSetPageRotation(this._handle, pageIndex, degrees);
  }

  // ----- document-level mutations -----------------------------------

  /**
   * Append every page of another PDF to the end of this document.
   */
  mergeFrom(sourcePath: string): void {
    this._throwIfClosed();
    if (typeof sourcePath !== 'string' || sourcePath.length === 0) {
      throw new TypeError('sourcePath must be a non-empty string');
    }
    native.editorMergeFrom(this._handle, sourcePath);
  }

  /** Flatten form fields across the entire document. */
  flattenForms(): void {
    this._throwIfClosed();
    native.editorFlattenForms(this._handle);
  }

  /**
   * Return warnings collected during the last form-flattening save.
   * Each entry names a widget field that had no `/AP` appearance stream;
   * flattening it produces a blank rectangle.
   */
  flattenWarnings(): string[] {
    this._throwIfClosed();
    return native.editorFlattenWarnings(this._handle) as string[];
  }

  /** Flatten annotations. If `pageIndex` is omitted, flattens all pages. */
  flattenAnnotations(pageIndex?: number): void {
    this._throwIfClosed();
    if (pageIndex === undefined) {
      native.editorFlattenAnnotations(this._handle);
    } else {
      native.editorFlattenAnnotations(this._handle, pageIndex);
    }
  }

  /** Set a form field value by fully-qualified field name. */
  setFormFieldValue(fieldName: string, value: string): void {
    this._throwIfClosed();
    native.editorSetFormFieldValue(this._handle, fieldName, value);
  }

  /** Import an FDF file (bytes) into the document's form. */
  importFdfBytes(fdf: Buffer | Uint8Array): void {
    this._throwIfClosed();
    native.editorImportFdfBytes(this._handle, fdf);
  }

  /** Import an XFDF file (bytes) into the document's form. */
  importXfdfBytes(xfdf: Buffer | Uint8Array): void {
    this._throwIfClosed();
    native.editorImportXfdfBytes(this._handle, xfdf);
  }

  // ----- byte-level merge / embed -----------------------------------

  /**
   * Append every page of another PDF (supplied as bytes) to this document.
   * Returns the number of pages added.
   */
  mergeFromBytes(data: Buffer | Uint8Array): number {
    this._throwIfClosed();
    return native.editorMergeFromBytes(this._handle, data) as number;
  }

  /** Embed a file attachment into the document. */
  embedFile(name: string, data: Buffer | Uint8Array): void {
    this._throwIfClosed();
    native.editorEmbedFile(this._handle, name, data);
  }

  // ----- redactions -------------------------------------------------

  /** Burn in redaction annotations on a single page (zero-based). */
  applyPageRedactions(pageIndex: number): void {
    this._throwIfClosed();
    native.editorApplyPageRedactions(this._handle, pageIndex);
  }

  /** Burn in all pending redaction annotations across the document. */
  applyAllRedactions(): void {
    this._throwIfClosed();
    native.editorApplyAllRedactions(this._handle);
  }

  // ----- rotation (additive) ----------------------------------------

  /** Rotate all pages by `degrees` (additive). */
  rotateAllPages(degrees: number): void {
    this._throwIfClosed();
    native.editorRotateAllPages(this._handle, degrees);
  }

  /** Rotate a single page by `degrees` (additive). */
  rotatePageBy(pageIndex: number, degrees: number): void {
    this._throwIfClosed();
    native.editorRotatePageBy(this._handle, pageIndex, degrees);
  }

  // ----- page boxes -------------------------------------------------

  /** Get the MediaBox of a page as `{x, y, width, height}`. */
  getPageMediaBox(pageIndex: number): { x: number; y: number; width: number; height: number } {
    this._throwIfClosed();
    return native.editorGetPageMediaBox(this._handle, pageIndex);
  }

  /** Set the MediaBox of a page. */
  setPageMediaBox(pageIndex: number, x: number, y: number, width: number, height: number): void {
    this._throwIfClosed();
    native.editorSetPageMediaBox(this._handle, pageIndex, x, y, width, height);
  }

  /** Get the CropBox of a page. Returns `{x:0,y:0,width:0,height:0}` if none set. */
  getPageCropBox(pageIndex: number): { x: number; y: number; width: number; height: number } {
    this._throwIfClosed();
    return native.editorGetPageCropBox(this._handle, pageIndex);
  }

  /** Set the CropBox of a page. */
  setPageCropBox(pageIndex: number, x: number, y: number, width: number, height: number): void {
    this._throwIfClosed();
    native.editorSetPageCropBox(this._handle, pageIndex, x, y, width, height);
  }

  // ----- erase regions ----------------------------------------------

  /**
   * Erase rectangular regions on a page.
   * `rects` is an array of `[x, y, w, h]` tuples.
   */
  eraseRegions(pageIndex: number, rects: [number, number, number, number][]): void {
    this._throwIfClosed();
    native.editorEraseRegions(this._handle, pageIndex, rects);
  }

  /** Clear all pending erase-region entries for a page. */
  clearEraseRegions(pageIndex: number): void {
    this._throwIfClosed();
    native.editorClearEraseRegions(this._handle, pageIndex);
  }

  // ----- form flattening on single page ------------------------------

  /** Flatten form fields on a single page. */
  flattenFormsOnPage(pageIndex: number): void {
    this._throwIfClosed();
    native.editorFlattenFormsOnPage(this._handle, pageIndex);
  }

  // ----- page-mark state queries ------------------------------------

  /** True if the page is marked for annotation-flatten. */
  isPageMarkedForFlatten(pageIndex: number): boolean {
    this._throwIfClosed();
    return native.editorIsPageMarkedForFlatten(this._handle, pageIndex) as boolean;
  }

  /** Remove the flatten mark from a page. */
  unmarkPageForFlatten(pageIndex: number): void {
    this._throwIfClosed();
    native.editorUnmarkPageForFlatten(this._handle, pageIndex);
  }

  /** True if the page is marked for redaction. */
  isPageMarkedForRedaction(pageIndex: number): boolean {
    this._throwIfClosed();
    return native.editorIsPageMarkedForRedaction(this._handle, pageIndex) as boolean;
  }

  /** Remove the redaction mark from a page. */
  unmarkPageForRedaction(pageIndex: number): void {
    this._throwIfClosed();
    native.editorUnmarkPageForRedaction(this._handle, pageIndex);
  }

  // ----- save paths -------------------------------------------------

  /** Save the document to `path`. */
  save(path: string): void {
    this._throwIfClosed();
    if (typeof path !== 'string' || path.length === 0) {
      throw new TypeError('path must be a non-empty string');
    }
    native.editorSave(this._handle, path);
  }

  /** Save with AES-256 encryption (user + owner passwords). */
  saveEncrypted(path: string, userPassword: string, ownerPassword: string): void {
    this._throwIfClosed();
    native.editorSaveEncrypted(this._handle, path, userPassword, ownerPassword);
  }

  /** Extract specific pages (by 0-based index) into a new PDF returned as a Buffer. */
  extractPagesToBytes(pageIndices: number[]): Buffer {
    this._throwIfClosed();
    return native.editorExtractPagesToBytes(this._handle, pageIndices) as Buffer;
  }

  /** Save the document to an in-memory Buffer. */
  saveToBytes(): Buffer {
    this._throwIfClosed();
    return native.editorSaveToBytes(this._handle) as Buffer;
  }

  /** Save to an in-memory Buffer with explicit compression / GC / linearize flags. */
  saveToBytesWithOptions(compress: boolean, garbageCollect: boolean, linearize: boolean): Buffer {
    this._throwIfClosed();
    return native.editorSaveToBytesWithOptions(
      this._handle,
      compress,
      garbageCollect,
      linearize
    ) as Buffer;
  }

  // ----- lifecycle --------------------------------------------------

  /** Release the native handle. Safe to call multiple times. */
  close(): void {
    if (!this._closed) {
      if (native.editorFree) {
        native.editorFree(this._handle);
      }
      this._closed = true;
    }
  }
}

export default DocumentEditor;
