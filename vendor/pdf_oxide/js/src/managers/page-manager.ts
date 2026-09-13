/**
 * Manager for PDF page-level operations
 *
 * Provides methods to query page count, dimensions, and validate indices.
 *
 * @example
 * ```typescript
 * import { PageManager } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const pageManager = new PageManager(doc);
 *
 * console.log(`Document has ${pageManager.getPageCount()} pages`);
 *
 * if (pageManager.isValidPageIndex(5)) {
 *   const info = pageManager.getPageInfo(5);
 *   console.log(`Page 5: ${info.width} x ${info.height} points`);
 * }
 * ```
 */

export interface PageInfo {
  index: number;
  width: number;
  height: number;
}

export interface PageRange {
  firstPage: number;
  lastPage: number;
}

export interface PageStatistics {
  count: number;
  minWidth: number;
  maxWidth: number;
  minHeight: number;
  maxHeight: number;
  averageWidth: number;
  averageHeight: number;
  hasVariableSizes: boolean;
}

export class PageManager {
  private _document: any;
  private _cache: Map<string, any>;

  /**
   * Creates a new PageManager for the given document
   * @param document - The PDF document
   * @throws Error if document is null or undefined
   */
  constructor(document: any) {
    if (!document) {
      throw new Error('Document is required');
    }
    this._document = document;
    this._cache = new Map();
  }

  /**
   * Clears the page cache
   */
  clearCache(): void {
    this._cache.clear();
  }

  /**
   * Gets the total number of pages in the document
   * @returns Number of pages
   */
  getPageCount(): number {
    const cacheKey = 'page:count';
    if (this._cache.has(cacheKey)) {
      return this._cache.get(cacheKey);
    }

    try {
      const count = this._document.pageCount || 0;
      this._cache.set(cacheKey, count);
      return count;
    } catch (error) {
      return 0;
    }
  }

  /**
   * Checks if a page index is valid for this document
   * @param pageIndex - Page index to validate (0-based)
   * @returns True if the page index is valid
   */
  isValidPageIndex(pageIndex: number): boolean {
    if (pageIndex < 0) {
      return false;
    }
    return pageIndex < this.getPageCount();
  }

  /**
   * Gets information about a specific page
   * @param pageIndex - Page index (0-based)
   * @returns PageInfo object with page dimensions
   * @throws Error if page index is invalid
   *
   * @example
   * ```typescript
   * const info = manager.getPageInfo(0);
   * console.log(`Page 0: ${info.width} x ${info.height} points`);
   * ```
   */
  getPageInfo(pageIndex: number): PageInfo {
    const cacheKey = `page:info:${pageIndex}`;
    if (this._cache.has(cacheKey)) {
      return this._cache.get(cacheKey);
    }

    if (!this.isValidPageIndex(pageIndex)) {
      throw new Error(`Invalid page index: ${pageIndex}`);
    }

    try {
      const page = this._document.getPage(pageIndex);
      const info: PageInfo = {
        index: pageIndex,
        width: page?.width || 0,
        height: page?.height || 0,
      };
      this._cache.set(cacheKey, info);
      return info;
    } catch (error) {
      return {
        index: pageIndex,
        width: 0,
        height: 0,
      };
    }
  }

  /**
   * Gets information about all pages in the document
   * @returns Array of PageInfo objects
   *
   * @example
   * ```typescript
   * const pages = manager.getAllPageInfo();
   * pages.forEach(page => {
   *   console.log(`Page ${page.index}: ${page.width} x ${page.height}`);
   * });
   * ```
   */
  getAllPageInfo(): PageInfo[] {
    const cacheKey = 'page:info:all';
    if (this._cache.has(cacheKey)) {
      return this._cache.get(cacheKey);
    }

    const count = this.getPageCount();
    const pages: PageInfo[] = [];
    for (let i = 0; i < count; i++) {
      pages.push(this.getPageInfo(i));
    }

    this._cache.set(cacheKey, pages);
    return pages;
  }

  /**
   * Checks if the document has no pages
   * @returns True if the document has no pages
   */
  isEmpty(): boolean {
    return this.getPageCount() === 0;
  }

  /**
   * Checks if the document has more than one page
   * @returns True if document has multiple pages
   */
  hasMultiplePages(): boolean {
    return this.getPageCount() > 1;
  }

  /**
   * Gets the valid page range
   * @returns Object with firstPage and lastPage indices
   *
   * @example
   * ```typescript
   * const range = manager.getPageRange();
   * console.log(`Page range: ${range.firstPage} to ${range.lastPage}`);
   * ```
   */
  getPageRange(): PageRange {
    const count = this.getPageCount();
    if (count === 0) {
      return { firstPage: 0, lastPage: -1 };
    }
    return { firstPage: 0, lastPage: count - 1 };
  }

  /**
   * Gets page dimension statistics
   * @returns Statistics about page dimensions
   *
   * @example
   * ```typescript
   * const stats = manager.getPageStatistics();
   * console.log(`Average width: ${stats.averageWidth}`);
   * console.log(`Pages vary in size: ${stats.hasVariableSizes}`);
   * ```
   */
  getPageStatistics(): PageStatistics {
    const pages = this.getAllPageInfo();

    if (pages.length === 0) {
      return {
        count: 0,
        minWidth: 0,
        maxWidth: 0,
        minHeight: 0,
        maxHeight: 0,
        averageWidth: 0,
        averageHeight: 0,
        hasVariableSizes: false,
      };
    }

    const widths = pages.map((p) => p.width);
    const heights = pages.map((p) => p.height);

    const minWidth = Math.min(...widths);
    const maxWidth = Math.max(...widths);
    const minHeight = Math.min(...heights);
    const maxHeight = Math.max(...heights);

    const averageWidth = widths.reduce((a, b) => a + b, 0) / widths.length;
    const averageHeight = heights.reduce((a, b) => a + b, 0) / heights.length;

    return {
      count: pages.length,
      minWidth,
      maxWidth,
      minHeight,
      maxHeight,
      averageWidth,
      averageHeight,
      hasVariableSizes: minWidth !== maxWidth || minHeight !== maxHeight,
    };
  }

  /**
   * Gets pages within a specific size range
   * @param minWidth - Minimum width
   * @param maxWidth - Maximum width
   * @param minHeight - Minimum height
   * @param maxHeight - Maximum height
   * @returns Matching PageInfo objects
   */
  getPagesInSizeRange(
    minWidth: number,
    maxWidth: number,
    minHeight: number,
    maxHeight: number
  ): PageInfo[] {
    const pages = this.getAllPageInfo();
    return pages.filter(
      (p) =>
        p.width >= minWidth && p.width <= maxWidth && p.height >= minHeight && p.height <= maxHeight
    );
  }

  /**
   * Gets landscape pages
   * @returns Array of landscape PageInfo objects
   */
  getLandscapePages(): PageInfo[] {
    const pages = this.getAllPageInfo();
    return pages.filter((p) => p.width > p.height);
  }

  /**
   * Gets portrait pages
   * @returns Array of portrait PageInfo objects
   */
  getPortraitPages(): PageInfo[] {
    const pages = this.getAllPageInfo();
    return pages.filter((p) => p.height > p.width);
  }
}
