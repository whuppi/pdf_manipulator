/**
 * Common type definitions and utilities
 */

/** A table extracted from a PDF page. */
export interface Table {
  /** Number of rows. */
  rows: number;
  /** Number of columns. */
  cols: number;
  /** True if the first row is a header row. */
  hasHeader: boolean;
  /** Cell text: cells[row][col]. Individual cells may be `null` when the
   *  native binding has no text for that position (missing cell, decoding
   *  failure, etc.). */
  cells: (string | null)[][];
}

// Re-export commonly used native types
export type {
  Annotation,
  CircleAnnotation,
  Color,
  DocumentInfo,
  EmbeddedFile,
  HighlightAnnotation,
  InkAnnotation,
  LineAnnotation,
  LinkAnnotation,
  Metadata,
  NativePdf,
  NativePdfDocument,
  NativePdfPage,
  PdfElement,
  PdfImage,
  PdfPath,
  PdfTable,
  PdfTableCell,
  PdfText,
  Point,
  PolygonAnnotation,
  Rect,
  SearchOptions,
  SearchResult,
  SquareAnnotation,
  TextAnnotation,
} from './native-bindings';

/**
 * Page range specification for document operations
 */
export interface PageRange {
  startPage?: number;
  endPage?: number;
  pages?: number[];
}

/**
 * Generic extraction result with metadata
 */
export interface ExtractionResult<T> {
  data: T;
  pageIndex: number;
  timestamp: Date;
  processingTimeMs?: number;
}

/**
 * Async operation callback function type
 */
export type AsyncOperationCallback<T> = (err: Error | null, result?: T) => void;

/**
 * Manager configuration interface for all managers
 */
export interface ManagerConfig {
  maxCacheSize?: number;
  cacheExpirationMs?: number;
  enableCaching?: boolean;
  timeout?: number;
  retryAttempts?: number;
  retryDelayMs?: number;
}

/**
 * Batch operation options
 */
export interface BatchOptions {
  batchSize?: number;
  parallel?: boolean;
  maxParallel?: number;
  progressCallback?: (processed: number, total: number) => void;
  continueOnError?: boolean;
}

/**
 * Error details for exception context
 */
export interface PdfErrorDetails {
  timestamp?: string;
  operation?: string;
  context?: Record<string, any>;
  originalError?: Error;
  stack?: string;
}

/**
 * Optional content (layers) information
 */
export interface OptionalContent {
  id: string;
  name: string;
  visible: boolean;
  locked?: boolean;
  printable?: boolean;
  exportable?: boolean;
  viewState?: string;
}

/**
 * Form field value map for filling forms
 */
export type FormFieldValues = Record<string, string | number | boolean | string[]>;

/**
 * Type for validation result
 */
export interface ValidationResult {
  isValid: boolean;
  errors: string[];
  warnings: string[];
}

/**
 * Stream operation callback
 */
export type StreamCallback<T> = (data: T) => void;

/**
 * Stream error callback
 */
export type StreamErrorCallback = (error: Error) => void;

/**
 * Stream end callback
 */
export type StreamEndCallback = () => void;

// ============================================================================
// DocumentBuilder — table primitives (v0.3.39, issue #393)
// ============================================================================

/**
 * Horizontal alignment for wrapped text and table cells.
 * Matches the C FFI integer encoding used by
 * `pdf_page_builder_text_in_rect` and `pdf_page_builder_table`.
 */
export enum Align {
  Left = 0,
  Center = 1,
  Right = 2,
}

/**
 * Column descriptor for {@link TableSpec} / {@link StreamingTableConfig}.
 */
export interface Column {
  /** Header label rendered in bold (used only when `hasHeader`/`repeatHeader`). */
  header: string;
  /** Column width in PDF points. */
  width: number;
  /** Cell alignment (default {@link Align.Left}). */
  align?: Align;
}

/**
 * Buffered-table spec passed to `PageBuilder.table(...)`.
 *
 * All rows are held in JS memory and flushed to the native
 * `pdf_page_builder_table` call in a single step.
 */
export interface TableSpec {
  /** Column layout — widths, alignments, and header labels. */
  columns: Column[];
  /** Body rows, each row has `columns.length` cells (nullable = empty). */
  rows: Array<Array<string | null | undefined>>;
  /** Promote the column headers to a styled first row. Defaults to true. */
  hasHeader?: boolean;
}

/**
 * Column-sizing strategy for streaming tables (issue #400).
 *
 * - `fixed` — use the `width` from each `Column` as-is (default).
 * - `sample` — buffer the first N rows, measure content, then freeze
 *   column widths automatically.  Supply `sampleRows`, `minColWidth`,
 *   and `maxColWidth` to tune.
 */
export type TableMode =
  | { kind: 'fixed' }
  | { kind: 'sample'; sampleRows?: number; minColWidthPt?: number; maxColWidthPt?: number };

/**
 * Configuration for the managed streaming-table adapter.
 */
export interface StreamingTableConfig {
  /** Column layout — widths, alignments, and header labels. */
  columns: Column[];
  /**
   * Whether to repeat the header row on every page break. Defaults to true.
   */
  repeatHeader?: boolean;
  /**
   * Column-sizing mode. Defaults to `{ kind: 'fixed' }`.
   */
  mode?: TableMode;
  /**
   * Maximum rowspan a cell may carry.  0 or 1 (default) disables rowspan.
   * Set to ≥2 to allow `pushRowSpan` cells to span multiple rows.
   */
  maxRowspan?: number;
  /**
   * Maximum rows to buffer before an automatic batch flush in the native
   * layer. 0 or unset defaults to 256.
   */
  batchSize?: number;
}

/**
 * A single cell value for `StreamingTable.pushRowSpan`.
 * `rowspan == 1` is equivalent to a normal cell.
 */
export interface SpanCell {
  text: string;
  rowspan: number;
}
