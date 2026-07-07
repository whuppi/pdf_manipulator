/**
 * PdfDocument - TypeScript type definitions for pdf_oxide
 *
 * Provides complete type definitions for the PdfDocument class and related types.
 * These types ensure type safety when using the pdf_oxide library in TypeScript.
 *
 * @example
 * ```typescript
 * import { PdfDocument } from 'pdf_oxide';
 *
 * const doc = await PdfDocument.open('document.pdf');
 * const pageCount = await doc.pageCount();
 * const text = await doc.extractText(0);
 * ```
 */

// ============================================================================
// Core Types
// ============================================================================

/**
 * Represents a rectangle in PDF coordinate space (origin at bottom-left)
 */
export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/**
 * Represents a point in PDF coordinate space
 */
export interface Point {
  readonly x: number;
  readonly y: number;
}

/**
 * Represents a color (RGBA)
 */
export interface Color {
  readonly r: number;
  readonly g: number;
  readonly b: number;
  readonly a?: number;
}

/**
 * Standard page sizes
 */
export enum PageSizeType {
  LETTER = 'letter',
  LEGAL = 'legal',
  A4 = 'a4',
  A3 = 'a3',
  A5 = 'a5',
  B4 = 'b4',
  TABLOID = 'tabloid',
}

/**
 * Page size dimensions
 */
export interface PageSize {
  readonly type?: PageSizeType;
  readonly width: number;
  readonly height: number;
}

// ============================================================================
// Document Metadata
// ============================================================================

/**
 * PDF document metadata
 */
export interface DocumentMetadata {
  readonly title?: string;
  readonly author?: string;
  readonly subject?: string;
  readonly keywords?: string;
  readonly creator?: string;
  readonly producer?: string;
  readonly creationDate?: Date;
  readonly modificationDate?: Date;
  readonly pageCount: number;
  readonly format?: string;
  readonly version?: string;
  readonly isEncrypted?: boolean;
  readonly hasXfa?: boolean;
}

// ============================================================================
// Form Fields
// ============================================================================

/**
 * Form field type enumeration
 */
export enum FormFieldType {
  TEXT = 'text',
  CHECKBOX = 'checkbox',
  RADIO = 'radio',
  COMBOBOX = 'combobox',
  LISTBOX = 'listbox',
  SIGNATURE = 'signature',
  DATE = 'date',
  BUTTON = 'button',
}

/**
 * Form field information
 */
export interface FormField {
  readonly name: string;
  readonly type: FormFieldType;
  readonly value: string;
  readonly pageIndex: number;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly required?: boolean;
  readonly readonly?: boolean;
}

// ============================================================================
// Page Content
// ============================================================================

/**
 * Text content on a page
 */
export interface TextContent {
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly fontName?: string;
  readonly fontSize?: number;
  readonly confidence?: number;
}

/**
 * Image content on a page
 */
export interface ImageContent {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly imageType?: string;
  readonly bitsPerComponent?: number;
  readonly colorSpace?: string;
}

// ============================================================================
// Annotations
// ============================================================================

/**
 * Annotation type enumeration
 */
export enum AnnotationType {
  TEXT = 'text',
  LINK = 'link',
  HIGHLIGHT = 'highlight',
  UNDERLINE = 'underline',
  STRIKEOUT = 'strikeout',
  SQUIGGLY = 'squiggly',
  STAMP = 'stamp',
  CARET = 'caret',
  INK = 'ink',
  POPUP = 'popup',
  FILE_ATTACHMENT = 'file_attachment',
  SOUND = 'sound',
  MOVIE = 'movie',
  WIDGET = 'widget',
  SCREEN = 'screen',
  PRINTER_MARK = 'printer_mark',
  TRAP_NET = 'trap_net',
  WATERMARK = 'watermark',
  REDACT = 'redact',
}

/**
 * Annotation information
 */
export interface Annotation {
  readonly type: AnnotationType;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly contents?: string;
  readonly author?: string;
  readonly creationDate?: Date;
  readonly modificationDate?: Date;
  readonly color?: Color;
}

// ============================================================================
// Search
// ============================================================================

/**
 * Search options
 */
export interface SearchOptions {
  readonly caseSensitive?: boolean;
  readonly wholeWord?: boolean;
  readonly regex?: boolean;
  readonly maxResults?: number;
  readonly pageRange?: { start: number; end: number };
}

/**
 * Search result
 */
export interface SearchResult {
  readonly pageIndex: number;
  readonly text: string;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
  readonly context?: string;
}

// ============================================================================
// Conversion Options
// ============================================================================

/**
 * Markdown conversion options
 */
export interface MarkdownOptions {
  readonly includeImages?: boolean;
  readonly includeLinks?: boolean;
  readonly preserveLayout?: boolean;
  readonly pageRange?: { start: number; end: number };
}

/**
 * HTML conversion options
 */
export interface HtmlOptions {
  readonly includeStyles?: boolean;
  readonly includeImages?: boolean;
  readonly embedImages?: boolean;
  readonly pageRange?: { start: number; end: number };
}

// ============================================================================
// Page Interface
// ============================================================================

/**
 * Represents a single page in a PDF document
 */
export interface PdfPage {
  readonly index: number;
  readonly width: number;
  readonly height: number;
  readonly rotation: number;
  readonly mediaBox: Rect;
  readonly cropBox?: Rect;
  readonly bleedBox?: Rect;
  readonly trimBox?: Rect;
  readonly artBox?: Rect;
}

// ============================================================================
// PdfDocument Class
// ============================================================================

/**
 * Main PDF document class
 *
 * Provides access to PDF document content, metadata, and structure.
 * All operations are async to support non-blocking I/O.
 *
 * @example
 * ```typescript
 * // Open a document
 * const doc = await PdfDocument.open('file.pdf');
 *
 * // Get page count
 * const pages = await doc.pageCount();
 *
 * // Extract text from first page
 * const text = await doc.extractText(0);
 *
 * // Get metadata
 * const metadata = await doc.getMetadata();
 *
 * // Convert to markdown
 * const markdown = await doc.toMarkdown();
 * ```
 */
export interface PdfDocument {
  // ==========================================================================
  // Static Methods
  // ==========================================================================

  /**
   * Open a PDF document from a file path
   */
  // open(path: string, password?: string): Promise<PdfDocument>;

  /**
   * Open a PDF document from a buffer
   */
  // openBuffer(buffer: Buffer, password?: string): Promise<PdfDocument>;

  // ==========================================================================
  // Document Properties
  // ==========================================================================

  /**
   * Get the number of pages in the document
   */
  pageCount(): Promise<number>;

  /**
   * Get document metadata
   */
  getMetadata(): Promise<DocumentMetadata>;

  /**
   * Get page information
   */
  getPage(pageIndex: number): Promise<PdfPage>;

  // ==========================================================================
  // Text Extraction
  // ==========================================================================

  /**
   * Extract text from a specific page
   */
  extractText(pageIndex: number): Promise<string>;

  /**
   * Extract text from a range of pages
   */
  extractTextRange(startPage: number, endPage: number): Promise<string[]>;

  /**
   * Extract all text from the document
   */
  extractAllText(): Promise<string>;

  // ==========================================================================
  // Form Fields
  // ==========================================================================

  /**
   * Get all form fields in the document
   */
  extractFormFields(): Promise<FormField[]>;

  /**
   * Get form field by name
   */
  getFormField(fieldName: string): Promise<FormField | undefined>;

  /**
   * Set form field value
   */
  setFormFieldValue(fieldName: string, value: string): Promise<boolean>;

  // ==========================================================================
  // Annotations
  // ==========================================================================

  /**
   * Get annotations on a specific page
   */
  getAnnotations(pageIndex: number): Promise<Annotation[]>;

  /**
   * Get all annotations in the document
   */
  getAllAnnotations(): Promise<Annotation[]>;

  // ==========================================================================
  // Search
  // ==========================================================================

  /**
   * Search for text in the document
   */
  search(query: string, options?: SearchOptions): Promise<SearchResult[]>;

  // ==========================================================================
  // Conversion
  // ==========================================================================

  /**
   * Convert document to markdown
   */
  toMarkdown(options?: MarkdownOptions): Promise<string>;

  /**
   * Convert document to HTML
   */
  toHtml(options?: HtmlOptions): Promise<string>;

  // ==========================================================================
  // Resource Management
  // ==========================================================================

  /**
   * Close the document and release resources
   */
  close(): Promise<void>;

  /**
   * Check if document is open
   */
  isOpen(): boolean;
}

/**
 * PdfDocument static methods (factory pattern)
 */
export interface PdfDocumentStatic {
  /**
   * Open a PDF document from a file path
   */
  open(path: string, password?: string): Promise<PdfDocument>;

  /**
   * Open a PDF document from a buffer
   */
  openBuffer(buffer: Buffer, password?: string): Promise<PdfDocument>;

  /**
   * Create a new empty PDF document
   */
  create(): Promise<PdfDocument>;
}

// ============================================================================
// Export PdfDocument (dynamically loaded from native module)
// ============================================================================

/**
 * Re-export PdfDocument from the main nodejs module
 * This provides the actual implementation with full type safety
 */
export { PdfDocument } from '../index.js';

export default PdfDocument;
