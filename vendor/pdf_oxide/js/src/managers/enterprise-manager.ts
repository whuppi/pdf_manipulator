/**
 * EnterpriseManager - Enterprise PDF Operations
 *
 * Provides enterprise-level PDF operations including:
 * - Bates numbering (standard and advanced)
 * - Page-level and document-level comparison
 * - Header and footer stamping
 *
 * @since 1.0.0
 */

import { EventEmitter } from 'events';
import { mapFfiErrorCode, PdfException } from '../errors';

// =============================================================================
// Enums
// =============================================================================

/**
 * Position for Bates number placement.
 */
export enum BatesPosition {
  TOP_LEFT = 0,
  TOP_CENTER = 1,
  TOP_RIGHT = 2,
  BOTTOM_LEFT = 3,
  BOTTOM_CENTER = 4,
  BOTTOM_RIGHT = 5,
}

/**
 * Text alignment for header/footer stamps.
 */
export enum StampAlignment {
  LEFT = 0,
  CENTER = 1,
  RIGHT = 2,
}

/**
 * Types of differences found in page comparison.
 */
export enum DifferenceType {
  TEXT_ADDED = 0,
  TEXT_REMOVED = 1,
  TEXT_CHANGED = 2,
  IMAGE_ADDED = 3,
  IMAGE_REMOVED = 4,
  IMAGE_CHANGED = 5,
  LAYOUT_CHANGED = 6,
  ANNOTATION_CHANGED = 7,
}

// =============================================================================
// Interfaces
// =============================================================================

/**
 * A single difference found between two pages.
 */
export interface Difference {
  /** Type of difference */
  readonly type: DifferenceType;
  /** Description of the difference */
  readonly description: string;
  /** Page region where the difference was found (x, y, width, height) */
  readonly bounds?: { x: number; y: number; width: number; height: number };
}

/**
 * Result of comparing two pages.
 */
export interface PageComparisonResult {
  /** Similarity score (0.0 - 1.0, where 1.0 means identical) */
  readonly similarity: number;
  /** Number of differences found */
  readonly diffCount: number;
  /** List of differences */
  readonly differences: readonly Difference[];
}

/**
 * Result of comparing two documents.
 */
export interface DocumentComparisonResult {
  /** Overall similarity score (0.0 - 1.0) */
  readonly similarity: number;
  /** Per-page comparison results */
  readonly pageResults: readonly PageComparisonResult[];
  /** Number of pages in document A */
  readonly pagesA: number;
  /** Number of pages in document B */
  readonly pagesB: number;
  /** Total differences found across all pages */
  readonly totalDifferences: number;
}

// =============================================================================
// EnterpriseManager
// =============================================================================

/**
 * Manager for enterprise PDF operations.
 *
 * Provides methods for Bates numbering, document comparison,
 * and header/footer stamping.
 *
 * @example
 * ```typescript
 * const enterprise = new EnterpriseManager(document);
 *
 * // Apply Bates numbering
 * await enterprise.applyBates('DOC', 1, 6, BatesPosition.BOTTOM_RIGHT);
 *
 * // Compare pages
 * const result = await enterprise.comparePages(docA, 0, docB, 0);
 * console.log(`Pages are ${(result.similarity * 100).toFixed(1)}% similar`);
 *
 * // Stamp headers and footers
 * await enterprise.stampHeader('CONFIDENTIAL', StampAlignment.CENTER, 12, 36);
 * ```
 */
export class EnterpriseManager extends EventEmitter {
  private document: any;
  private native: any;

  constructor(document: any) {
    super();
    if (!document) {
      throw new Error('Document cannot be null or undefined');
    }
    this.document = document;
    try {
      this.native = require('../../index.node');
    } catch {
      this.native = null;
    }
  }

  // ===========================================================================
  // Bates Numbering
  // ===========================================================================

  /**
   * Applies Bates numbering to the document.
   *
   * @param prefix - Text prefix before the number (e.g., 'DOC')
   * @param startNumber - Starting number
   * @param numDigits - Number of digits (zero-padded)
   * @param position - Position on the page
   * @throws PdfException if the operation fails
   */
  async applyBates(
    prefix: string,
    startNumber: number,
    numDigits: number,
    position: BatesPosition
  ): Promise<void> {
    if (!this.native?.pdf_bates_apply) {
      throw new PdfException('9900', 'Native enterprise not available: pdf_bates_apply not found');
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_bates_apply(
      this.document._handle ?? this.document,
      prefix,
      startNumber,
      numDigits,
      position,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to apply Bates numbering');
    }

    this.emit('bates-applied', { prefix, startNumber, numDigits, position });
  }

  /**
   * Applies advanced Bates numbering with full configuration.
   *
   * @param prefix - Text prefix before the number
   * @param suffix - Text suffix after the number
   * @param startNumber - Starting number
   * @param numDigits - Number of digits (zero-padded)
   * @param position - Position on the page
   * @param fontSize - Font size in points
   * @param margin - Margin from page edge in points
   * @param startPage - Starting page index (0-based)
   * @param endPage - Ending page index (0-based, -1 for last page)
   * @throws PdfException if the operation fails
   */
  async applyBatesAdvanced(
    prefix: string,
    suffix: string,
    startNumber: number,
    numDigits: number,
    position: BatesPosition,
    fontSize: number,
    margin: number,
    startPage: number,
    endPage: number
  ): Promise<void> {
    if (!this.native?.pdf_bates_apply_advanced) {
      throw new PdfException(
        '9900',
        'Native enterprise not available: pdf_bates_apply_advanced not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_bates_apply_advanced(
      this.document._handle ?? this.document,
      prefix,
      suffix,
      startNumber,
      numDigits,
      position,
      fontSize,
      margin,
      startPage,
      endPage,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to apply advanced Bates numbering');
    }

    this.emit('bates-applied-advanced', { prefix, suffix, startNumber, numDigits, position });
  }

  // ===========================================================================
  // Document Comparison
  // ===========================================================================

  /**
   * Compares two pages from potentially different documents.
   *
   * @param docA - First document handle
   * @param pageA - Page index in first document (0-based)
   * @param docB - Second document handle
   * @param pageB - Page index in second document (0-based)
   * @returns Comparison result with similarity score and differences
   * @throws PdfException if the comparison fails
   */
  async comparePages(
    docA: any,
    pageA: number,
    docB: any,
    pageB: number
  ): Promise<PageComparisonResult> {
    if (!this.native?.pdf_compare_pages) {
      throw new PdfException(
        '9900',
        'Native enterprise not available: pdf_compare_pages not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const comparisonPtr = this.native.pdf_compare_pages(
      docA._handle ?? docA,
      pageA,
      docB._handle ?? docB,
      pageB,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to compare pages');
    }

    try {
      const similarity = this.native.pdf_comparison_get_similarity?.(comparisonPtr) ?? 0;
      const diffCount = this.native.pdf_comparison_get_diff_count?.(comparisonPtr) ?? 0;

      const differences: Difference[] = [];
      for (let i = 0; i < diffCount; i++) {
        const diffPtr = this.native.pdf_comparison_get_diff?.(comparisonPtr, i);
        if (diffPtr) {
          const diffType = this.native.pdf_comparison_get_diff_type?.(diffPtr) ?? 0;
          differences.push({
            type: diffType as DifferenceType,
            description: `Difference ${i + 1}`,
          });
        }
      }

      const result: PageComparisonResult = { similarity, diffCount, differences };
      this.emit('pages-compared', { pageA, pageB, similarity });
      return result;
    } finally {
      if (this.native.pdf_comparison_free) {
        this.native.pdf_comparison_free(comparisonPtr);
      }
    }
  }

  /**
   * Compares two entire documents page by page.
   *
   * @param docA - First document handle
   * @param docB - Second document handle
   * @returns Document-level comparison result
   * @throws PdfException if the comparison fails
   */
  async compareDocuments(docA: any, docB: any): Promise<DocumentComparisonResult> {
    if (!this.native?.pdf_compare_documents) {
      throw new PdfException(
        '9900',
        'Native enterprise not available: pdf_compare_documents not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_compare_documents(
      docA._handle ?? docA,
      docB._handle ?? docB,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to compare documents');
    }

    try {
      let result: DocumentComparisonResult;
      if (typeof resultPtr === 'string') {
        result = JSON.parse(resultPtr);
      } else {
        result = resultPtr ?? {
          similarity: 0,
          pageResults: [],
          pagesA: 0,
          pagesB: 0,
          totalDifferences: 0,
        };
      }

      this.emit('documents-compared', {
        similarity: result.similarity,
        totalDifferences: result.totalDifferences,
      });
      return result;
    } finally {
      if (this.native.pdf_document_comparison_free && typeof resultPtr !== 'string') {
        this.native.pdf_document_comparison_free(resultPtr);
      }
    }
  }

  // ===========================================================================
  // Header / Footer Stamping
  // ===========================================================================

  /**
   * Stamps a header on all pages of the document.
   *
   * @param text - Header text (supports placeholders: {page}, {pages}, {date})
   * @param align - Text alignment
   * @param size - Font size in points
   * @param margin - Top margin in points
   * @throws PdfException if the operation fails
   */
  async stampHeader(
    text: string,
    align: StampAlignment,
    size: number,
    margin: number
  ): Promise<void> {
    if (!this.native?.pdf_stamp_header) {
      throw new PdfException('9900', 'Native enterprise not available: pdf_stamp_header not found');
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_stamp_header(
      this.document._handle ?? this.document,
      text,
      align,
      size,
      margin,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to stamp header');
    }

    this.emit('header-stamped', { text, align, size, margin });
  }

  /**
   * Stamps a footer on all pages of the document.
   *
   * @param text - Footer text (supports placeholders: {page}, {pages}, {date})
   * @param align - Text alignment
   * @param size - Font size in points
   * @param margin - Bottom margin in points
   * @throws PdfException if the operation fails
   */
  async stampFooter(
    text: string,
    align: StampAlignment,
    size: number,
    margin: number
  ): Promise<void> {
    if (!this.native?.pdf_stamp_footer) {
      throw new PdfException('9900', 'Native enterprise not available: pdf_stamp_footer not found');
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_stamp_footer(
      this.document._handle ?? this.document,
      text,
      align,
      size,
      margin,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to stamp footer');
    }

    this.emit('footer-stamped', { text, align, size, margin });
  }

  /**
   * Stamps both header and footer on all pages of the document.
   *
   * @param headerText - Header text
   * @param footerText - Footer text
   * @param align - Text alignment for both
   * @param size - Font size in points
   * @param margin - Margin from page edge in points
   * @throws PdfException if the operation fails
   */
  async stampHeaderFooter(
    headerText: string,
    footerText: string,
    align: StampAlignment,
    size: number,
    margin: number
  ): Promise<void> {
    if (!this.native?.pdf_stamp_header_footer) {
      throw new PdfException(
        '9900',
        'Native enterprise not available: pdf_stamp_header_footer not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_stamp_header_footer(
      this.document._handle ?? this.document,
      headerText,
      footerText,
      align,
      size,
      margin,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to stamp header and footer');
    }

    this.emit('header-footer-stamped', { headerText, footerText, align, size, margin });
  }

  // ===========================================================================
  // Cleanup
  // ===========================================================================

  /**
   * Releases resources held by this manager.
   */
  destroy(): void {
    this.removeAllListeners();
  }
}

export default EnterpriseManager;
