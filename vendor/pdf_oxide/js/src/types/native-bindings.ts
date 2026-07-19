/**
 * Type definitions for PDF Oxide native bindings (C++ module via NAPI)
 *
 * These interfaces describe the structure of objects returned from the native module.
 * They are used for type checking and IDE auto-completion when working with the binding layer.
 */

// ===== Geometry Types =====

/**
 * Represents a rectangular area with coordinates and dimensions
 */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Represents a 2D point coordinate
 */
export interface Point {
  x: number;
  y: number;
}

/**
 * Represents an RGB color value
 */
export interface Color {
  red: number;
  green: number;
  blue: number;
  alpha?: number;
}

// ===== Document Info Types =====

/**
 * Document metadata and properties
 */
export interface DocumentInfo {
  title?: string;
  author?: string;
  subject?: string;
  keywords?: string[];
  creator?: string;
  producer?: string;
  creationDate?: Date;
  modificationDate?: Date;
  encrypted?: boolean;
  pages?: number;
}

/**
 * PDF document metadata
 */
export interface Metadata {
  [key: string]: string | number | boolean | Date | undefined;
}

/**
 * Embedded file information
 */
export interface EmbeddedFile {
  filename: string;
  mimeType?: string;
  size?: number;
  data?: Buffer;
}

// ===== Search Types =====

/**
 * Result of a text search operation
 */
export interface SearchResult {
  pageIndex: number;
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
  confidence?: number;
}

/**
 * Options for text search operations
 */
export interface SearchOptions {
  caseSensitive?: boolean;
  wholeWords?: boolean;
  regex?: boolean;
  startPage?: number;
  endPage?: number;
  maxResults?: number;
}

/**
 * Options for document conversion
 */
export interface ConversionOptions {
  format?: 'markdown' | 'text' | 'html';
  includeImages?: boolean;
  includeMetadata?: boolean;
  preserveLayout?: boolean;
  encoding?: string;
}

// ===== PDF Element Types =====

/**
 * Base class for PDF elements (text, images, paths, etc.)
 */
export interface PdfElement {
  type: string;
  bounds: Rect;
  rotation?: number;
  opacity?: number;
  blendMode?: string;
}

/**
 * Text element extracted from PDF
 */
export interface PdfText extends PdfElement {
  type: 'text';
  content: string;
  fontSize?: number;
  fontName?: string;
  fontFamily?: string;
  bold?: boolean;
  italic?: boolean;
  color?: Color;
}

/**
 * Image element from PDF
 */
export interface PdfImage extends PdfElement {
  type: 'image';
  width: number;
  height: number;
  colorSpace?: string;
  bitsPerComponent?: number;
  data?: Buffer;
}

/**
 * Vector path (graphics)
 */
export interface PdfPath extends PdfElement {
  type: 'path';
  points: Point[];
  strokeColor?: Color;
  fillColor?: Color;
  lineWidth?: number;
}

/**
 * Table extracted from PDF
 */
export interface PdfTable extends PdfElement {
  type: 'table';
  rows: number;
  columns: number;
  cells: PdfTableCell[][];
}

/**
 * Individual table cell
 */
export interface PdfTableCell {
  text: string;
  rowSpan?: number;
  colSpan?: number;
  backgroundColor?: Color;
  borderColor?: Color;
}

// ===== Annotation Types =====

/**
 * Base annotation interface
 */
export interface Annotation {
  type: string;
  page: number;
  bounds: Rect;
  author?: string;
  content?: string;
  creationDate?: Date;
  modificationDate?: Date;
  flags?: number;
}

/**
 * Text annotation (sticky note style)
 */
export interface TextAnnotation extends Annotation {
  type: 'text';
  icon?: string;
  color?: Color;
}

/**
 * Highlight annotation
 */
export interface HighlightAnnotation extends Annotation {
  type: 'highlight';
  color?: Color;
  quadPoints?: Point[][];
}

/**
 * Link annotation
 */
export interface LinkAnnotation extends Annotation {
  type: 'link';
  url?: string;
  destination?: {
    page: number;
    x?: number;
    y?: number;
    zoom?: number;
  };
  action?: string;
}

/**
 * Ink annotation (drawing/handwriting)
 */
export interface InkAnnotation extends Annotation {
  type: 'ink';
  strokes: Point[][];
  color?: Color;
  thickness?: number;
}

/**
 * Square annotation
 */
export interface SquareAnnotation extends Annotation {
  type: 'square';
  fillColor?: Color;
  strokeColor?: Color;
  lineWidth?: number;
}

/**
 * Circle annotation
 */
export interface CircleAnnotation extends Annotation {
  type: 'circle';
  fillColor?: Color;
  strokeColor?: Color;
  lineWidth?: number;
}

/**
 * Line annotation
 */
export interface LineAnnotation extends Annotation {
  type: 'line';
  startPoint: Point;
  endPoint: Point;
  color?: Color;
  lineWidth?: number;
  startLineStyle?: string;
  endLineStyle?: string;
}

/**
 * Polygon annotation
 */
export interface PolygonAnnotation extends Annotation {
  type: 'polygon';
  vertices: Point[];
  fillColor?: Color;
  strokeColor?: Color;
  lineWidth?: number;
}

// ===== Native Class Interfaces =====

/**
 * Native PdfDocument class interface
 */
export interface NativePdfDocument {
  // Version info
  getVersion(): string;
  getPdfOxideVersion(): string;

  // Document properties
  getPageCount(): number;
  getTitle(): string | null;
  getAuthor(): string | null;
  getSubject(): string | null;
  getKeywords(): string[] | null;
  getCreationDate(): Date | null;
  getModificationDate(): Date | null;
  isEncrypted(): boolean;
  isLinearized(): boolean;

  // Metadata operations
  getMetadata(): Metadata;
  setMetadata(metadata: Metadata): void;
  getDocumentInfo(): DocumentInfo;

  // Text extraction
  extractText(pageIndex: number): string;
  extractTextAsync(pageIndex: number): Promise<string>;
  toMarkdown(): string;
  toMarkdownAsync(): Promise<string>;
  toHtml(): string;

  // Search operations
  search(text: string, options?: SearchOptions): SearchResult[];
  hasText(text: string, caseSensitive?: boolean): boolean;

  // Page access
  getPage(pageIndex: number): NativePdfPage;
  getPages(): NativePdfPage[];

  // Document manipulation
  saveToPath(path: string): void;
  saveToPdf(format?: string): Buffer;
  save_async(path: string): Promise<void>;
  saveToStream(stream: NodeJS.WritableStream): void;

  // Form operations
  getForms(): any;
  fillForms(values: Record<string, string>): void;

  // Embedded files
  getEmbeddedFiles(): EmbeddedFile[];
  extractEmbeddedFile(filename: string): Buffer;

  // Security operations
  encrypt(userPassword: string, ownerPassword?: string, permissions?: number): void;
  decrypt(password: string): boolean;
  setPermissions(permissions: number): void;

  // Outline/Bookmarks
  getOutline(): any[];
  setOutline(outline: any[]): void;

  // Document structure
  getStructure(): any;
  getPageLabels(): string[];
  setPageLabels(labels: string[]): void;

  // Layers (Optional Content)
  getLayers(): any[];
  setLayerVisibility(layerId: string, visible: boolean): void;

  // Cleanup
  close(): void;
}

/**
 * Native Pdf class interface (represents a PDF file)
 */
export interface NativePdf {
  // Properties
  getPageCount(): number;
  getPageSizes(): Array<{ width: number; height: number }>;
  getPageSize(pageIndex: number): { width: number; height: number };

  // Page access
  getPage(pageIndex: number): NativePdfPage;

  // Rendering
  renderPage(pageIndex: number, options?: any): Buffer;
  renderPageToCanvas(pageIndex: number, canvas: any): void;

  // Manipulation
  addPage(width: number, height: number): NativePdfPage;
  removePage(pageIndex: number): void;
  insertPage(pageIndex: number, width: number, height: number): NativePdfPage;
  rotatePage(pageIndex: number, rotation: number): void;

  // Saving
  save_async(path: string): Promise<void>;
  saveToBuffer(): Buffer;

  // Cleanup
  close(): void;
}

/**
 * Native PdfPage class interface
 */
export interface NativePdfPage {
  // Properties
  getIndex(): number;
  getWidth(): number;
  getHeight(): number;
  getRotation(): number;
  getMediaBox(): Rect;
  getCropBox(): Rect;

  // Content extraction
  extractText(): string;
  extractElements(): PdfElement[];
  getImages(): PdfImage[];
  getTables(): PdfTable[];

  // Rendering
  render(options?: any): Buffer;
  renderToCanvas(canvas: any): void;

  // Annotations
  getAnnotations(): Annotation[];
  addAnnotation(annotation: Annotation): void;
  removeAnnotation(annotationIndex: number): void;

  // Manipulation
  setMediaBox(rect: Rect): void;
  setCropBox(rect: Rect): void;
  rotate(rotation: number): void;

  // Drawing
  drawText(text: string, x: number, y: number, options?: any): void;
  drawImage(image: Buffer, x: number, y: number, width: number, height: number): void;
  drawPath(points: Point[], options?: any): void;

  // Search
  searchText(text: string, options?: SearchOptions): SearchResult[];

  // Cleanup
  close(): void;
}

/**
 * Native builder class interface
 */
export interface NativePdfBuilder {
  // Fluent methods
  title(title: string): NativePdfBuilder;
  author(author: string): NativePdfBuilder;
  subject(subject: string): NativePdfBuilder;
  keywords(keywords: string[]): NativePdfBuilder;
  creator(creator: string): NativePdfBuilder;
  producer(producer: string): NativePdfBuilder;

  // Build method
  build(): NativePdf;
}

/**
 * Native text searcher interface
 */
export interface NativeTextSearcher {
  search(text: string, options?: SearchOptions): SearchResult[];
  replaceAll(searchText: string, replaceText: string): number;
  getMatches(text: string): SearchResult[];
}

/**
 * Page size constants
 */
export interface PageSizeConstants {
  A0: { width: number; height: number };
  A1: { width: number; height: number };
  A2: { width: number; height: number };
  A3: { width: number; height: number };
  A4: { width: number; height: number };
  A5: { width: number; height: number };
  Letter: { width: number; height: number };
  Legal: { width: number; height: number };
  Tabloid: { width: number; height: number };
}

/**
 * Complete native module interface
 */
export interface NativeModule {
  // Version functions
  getVersion(): string;
  getPdfOxideVersion(): string;

  // Classes
  PdfDocument: new (
    ...args: any[]
  ) => NativePdfDocument;
  Pdf: new (...args: any[]) => NativePdf;
  PdfPage: new (...args: any[]) => NativePdfPage;
  PdfBuilder: new (...args: any[]) => NativePdfBuilder;
  TextSearcher: new (...args: any[]) => NativeTextSearcher;

  // Element types
  PdfElement: new (
    ...args: any[]
  ) => PdfElement;
  PdfText: new (...args: any[]) => PdfText;
  PdfImage: new (...args: any[]) => PdfImage;
  PdfPath: new (...args: any[]) => PdfPath;
  PdfTable: new (...args: any[]) => PdfTable;
  PdfStructure: new (...args: any[]) => any;

  // Annotation types
  Annotation: new (
    ...args: any[]
  ) => Annotation;
  TextAnnotation: new (...args: any[]) => TextAnnotation;
  HighlightAnnotation: new (...args: any[]) => HighlightAnnotation;
  LinkAnnotation: new (...args: any[]) => LinkAnnotation;

  // Type constants
  PageSize: PageSizeConstants;
  Rect: new (x: number, y: number, width: number, height: number) => Rect;
  Point: new (x: number, y: number) => Point;
  Color: new (red: number, green: number, blue: number, alpha?: number) => Color;

  // Options
  ConversionOptions: new (
    ...args: any[]
  ) => ConversionOptions;
  SearchOptions: new (...args: any[]) => SearchOptions;
  SearchResult: new (...args: any[]) => SearchResult;

  [key: string]: any;
}
