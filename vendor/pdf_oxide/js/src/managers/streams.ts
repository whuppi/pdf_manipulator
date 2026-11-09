/**
 * Stream API support for PDF Oxide Node.js
 *
 * Provides Readable streams for search results, text extraction, and page metadata.
 * Supports backpressure handling and proper Node.js stream semantics.
 *
 * Phase 2.4 implementation for idiomatic Node.js patterns with Stream API.
 */

import { Readable } from 'node:stream';

/**
 * SearchResult emitted by SearchStream
 */
export interface SearchResultData {
  text?: string;
  pageIndex?: number;
  position?: number;
  boundingBox?: Record<string, number>;
}

/**
 * ExtractionProgress emitted by ExtractionStream
 */
export interface ExtractionProgressData {
  pageIndex: number;
  totalPages: number;
  extractedText: string;
  extractionType: 'text' | 'markdown' | 'html';
  progress: number;
}

/**
 * PageMetadata emitted by MetadataStream
 */
export interface PageMetadataData {
  pageIndex: number;
  width: number;
  height: number;
  fontCount: number;
  imageCount: number;
  rotation: number;
}

/**
 * Readable stream for search results
 *
 * Emits search results one at a time with proper backpressure handling.
 * Supports searching either a specific page or the entire document.
 *
 * Supports both traditional stream API (.on('data')) and async iteration (for await...of).
 *
 * @example
 * ```typescript
 * // Traditional stream API
 * const stream = new SearchStream(searchManager, 'keyword');
 * stream.on('data', (result) => {
 *   console.log(`Found on page ${result.pageIndex}: ${result.text}`);
 * });
 *
 * // Async iteration
 * const stream = new SearchStream(searchManager, 'keyword');
 * for await (const result of stream) {
 *   console.log(`Found on page ${result.pageIndex}: ${result.text}`);
 * }
 * ```
 */
export class SearchStream extends Readable {
  private searchManager: any;
  private searchTerm: string;
  private options: Record<string, any>;
  private pageIndex: number | undefined;
  private caseSensitive: boolean;
  private wholeWords: boolean;
  private maxResults: number;
  private _results: any[] | null;
  private _currentIndex: number;
  private _resultCount: number;
  private _initialized: boolean;
  private _ended: boolean;

  /**
   * Creates a new SearchStream
   * @param searchManager - The search manager instance
   * @param searchTerm - Text to search for
   * @param options - Search options
   * @throws Error if parameters are invalid
   */
  constructor(searchManager: any, searchTerm: string, options: Record<string, any> = {}) {
    super({ objectMode: true });

    if (!searchManager) {
      throw new Error('SearchManager is required');
    }
    if (!searchTerm || typeof searchTerm !== 'string') {
      throw new Error('Search term must be a non-empty string');
    }

    this.searchManager = searchManager;
    this.searchTerm = searchTerm;
    this.options = options;
    this.pageIndex = options.pageIndex;
    this.caseSensitive = options.caseSensitive ?? false;
    this.wholeWords = options.wholeWords ?? false;
    this.maxResults = options.maxResults ?? Infinity;

    this._results = null;
    this._currentIndex = 0;
    this._resultCount = 0;
    this._initialized = false;
    this._ended = false;
  }

  /**
   * Initialize results (lazy initialization)
   * @private
   */
  private _initialize(): void {
    if (this._initialized) return;
    this._initialized = true;

    try {
      // Perform search
      if (this.pageIndex !== undefined) {
        this._results = (this.searchManager.search(this.searchTerm, this.pageIndex, {
          caseSensitive: this.caseSensitive,
          wholeWords: this.wholeWords,
        }) || []) as any[];
      } else {
        this._results = (this.searchManager.searchAll(this.searchTerm, {
          caseSensitive: this.caseSensitive,
          wholeWords: this.wholeWords,
        }) || []) as any[];
      }

      // Apply max results limit
      if (this._results && this._results.length > this.maxResults) {
        this._results = this._results.slice(0, this.maxResults);
      }
    } catch (error) {
      this.destroy(error as Error);
    }
  }

  /**
   * Implement _read() for readable stream
   * @private
   */
  _read(): void {
    // Initialize on first read
    if (!this._initialized) {
      this._initialize();
    }

    // Check if we have results to emit
    if (!this._results || this._currentIndex >= this._results.length) {
      // All results emitted
      if (!this._ended) {
        this._ended = true;
        this.push(null);
      }
      return;
    }

    // Emit next result
    const result = this._results[this._currentIndex];
    this._currentIndex++;

    // Format the result
    const data: SearchResultData = {
      text: result.text || result.getText?.(),
      pageIndex: result.pageIndex || result.page || 0,
      position: result.position || 0,
      boundingBox: result.boundingBox,
    };

    this.push(data);
  }

  /**
   * Implement async iteration protocol for `for await...of` support
   * @returns AsyncIterator for iterating over search results
   */
  async *[Symbol.asyncIterator](): AsyncGenerator<SearchResultData, undefined, unknown> {
    // Initialize on first iteration
    if (!this._initialized) {
      this._initialize();
    }

    // Yield results one by one
    while (this._results && this._currentIndex < this._results.length) {
      const result = this._results[this._currentIndex];
      this._currentIndex++;

      const data: SearchResultData = {
        text: result.text || result.getText?.(),
        pageIndex: result.pageIndex || result.page || 0,
        position: result.position || 0,
        boundingBox: result.boundingBox,
      };

      yield data;
    }

    if (!this._ended) {
      this._ended = true;
      this.destroy();
    }
  }
}

/**
 * Readable stream for text extraction with progress tracking
 *
 * Emits extraction progress for each page with progress percentage.
 * Supports multiple extraction formats: text, markdown, html.
 * Supports both traditional stream API and async iteration.
 *
 * @example
 * ```typescript
 * // Traditional stream API
 * const stream = new ExtractionStream(extractionManager, 0, 10, 'markdown');
 * stream.on('data', (progress) => {
 *   console.log(`Progress: ${Math.round(progress.progress * 100)}%`);
 *   console.log(`Page ${progress.pageIndex + 1}: ${progress.extractedText.length} chars`);
 * });
 *
 * // Async iteration
 * const stream = new ExtractionStream(extractionManager, 0, 10, 'markdown');
 * for await (const progress of stream) {
 *   console.log(`Progress: ${Math.round(progress.progress * 100)}%`);
 * }
 * ```
 */
export class ExtractionStream extends Readable {
  private extractionManager: any;
  private startPage: number;
  private endPage: number;
  private extractionType: 'text' | 'markdown' | 'html';
  private options: Record<string, any>;
  private _currentPage: number;
  private _totalPages: number;
  private _ended: boolean;

  /**
   * Creates a new ExtractionStream
   * @param extractionManager - The extraction manager instance
   * @param startPage - Starting page index (inclusive)
   * @param endPage - Ending page index (exclusive)
   * @param extractionType - 'text', 'markdown', or 'html'
   * @param options - Additional extraction options
   * @throws Error if parameters are invalid
   */
  constructor(
    extractionManager: any,
    startPage: number,
    endPage: number,
    extractionType: 'text' | 'markdown' | 'html' = 'text',
    options: Record<string, any> = {}
  ) {
    super({ objectMode: true });

    if (!extractionManager) {
      throw new Error('ExtractionManager is required');
    }
    if (typeof startPage !== 'number' || startPage < 0) {
      throw new Error('Start page must be a non-negative number');
    }
    if (typeof endPage !== 'number' || endPage <= startPage) {
      throw new Error('End page must be greater than start page');
    }
    if (!['text', 'markdown', 'html'].includes(extractionType)) {
      throw new Error("Extraction type must be 'text', 'markdown', or 'html'");
    }

    this.extractionManager = extractionManager;
    this.startPage = startPage;
    this.endPage = endPage;
    this.extractionType = extractionType;
    this.options = options;

    this._currentPage = startPage;
    this._totalPages = endPage - startPage;
    this._ended = false;
  }

  /**
   * Implement _read() for readable stream
   * @private
   */
  _read(): void {
    // Check if we've processed all pages
    if (this._currentPage >= this.endPage) {
      if (!this._ended) {
        this._ended = true;
        this.push(null);
      }
      return;
    }

    try {
      // Extract current page
      let extractedText: string;
      if (this.extractionType === 'markdown') {
        extractedText = this.extractionManager.extractMarkdown(this._currentPage, this.options);
      } else if (this.extractionType === 'html') {
        extractedText = this.extractionManager.extractHtml(this._currentPage, this.options);
      } else {
        extractedText = this.extractionManager.extractText(this._currentPage, this.options);
      }

      // Emit progress object
      const progress: ExtractionProgressData = {
        pageIndex: this._currentPage,
        totalPages: this._totalPages,
        extractedText: extractedText || '',
        extractionType: this.extractionType,
        progress: (this._currentPage - this.startPage + 1) / this._totalPages,
      };

      this._currentPage++;
      this.push(progress);
    } catch (error) {
      this.destroy(error as Error);
    }
  }

  /**
   * Implement async iteration protocol for `for await...of` support
   * @returns AsyncGenerator for iterating over extraction progress
   */
  async *[Symbol.asyncIterator](): AsyncGenerator<ExtractionProgressData, undefined, unknown> {
    // Process each page
    while (this._currentPage < this.endPage) {
      try {
        // Extract current page
        let extractedText: string;
        if (this.extractionType === 'markdown') {
          extractedText = this.extractionManager.extractMarkdown(this._currentPage, this.options);
        } else if (this.extractionType === 'html') {
          extractedText = this.extractionManager.extractHtml(this._currentPage, this.options);
        } else {
          extractedText = this.extractionManager.extractText(this._currentPage, this.options);
        }

        // Create progress object
        const progress: ExtractionProgressData = {
          pageIndex: this._currentPage,
          totalPages: this._totalPages,
          extractedText: extractedText || '',
          extractionType: this.extractionType,
          progress: (this._currentPage - this.startPage + 1) / this._totalPages,
        };

        this._currentPage++;

        yield progress;
      } catch (error) {
        this.destroy(error as Error);
        return;
      }
    }

    if (!this._ended) {
      this._ended = true;
      this.destroy();
    }
  }
}

/**
 * Readable stream for page metadata retrieval
 *
 * Emits page metadata (dimensions, fonts, images) for each page in range.
 * Supports lazy loading of metadata per page.
 * Supports both traditional stream API and async iteration.
 *
 * @example
 * ```typescript
 * // Traditional stream API
 * const stream = new MetadataStream(renderingManager, 0, 10);
 * stream.on('data', (metadata) => {
 *   console.log(`Page ${metadata.pageIndex + 1}: ${metadata.width}x${metadata.height}`);
 *   console.log(`  Fonts: ${metadata.fontCount}, Images: ${metadata.imageCount}`);
 * });
 *
 * // Async iteration
 * const stream = new MetadataStream(renderingManager, 0, 10);
 * for await (const metadata of stream) {
 *   console.log(`Page ${metadata.pageIndex + 1}: ${metadata.width}x${metadata.height}`);
 * }
 * ```
 */
export class MetadataStream extends Readable {
  private renderingManager: any;
  private startPage: number;
  private endPage: number;
  private _currentPage: number;
  private _ended: boolean;

  /**
   * Creates a new MetadataStream
   * @param renderingManager - The rendering manager instance
   * @param startPage - Starting page index (inclusive)
   * @param endPage - Ending page index (exclusive)
   * @throws Error if parameters are invalid
   */
  constructor(renderingManager: any, startPage: number, endPage: number) {
    super({ objectMode: true });

    if (!renderingManager) {
      throw new Error('RenderingManager is required');
    }
    if (typeof startPage !== 'number' || startPage < 0) {
      throw new Error('Start page must be a non-negative number');
    }
    if (typeof endPage !== 'number' || endPage <= startPage) {
      throw new Error('End page must be greater than start page');
    }

    this.renderingManager = renderingManager;
    this.startPage = startPage;
    this.endPage = endPage;

    this._currentPage = startPage;
    this._ended = false;
  }

  /**
   * Implement _read() for readable stream
   * @private
   */
  _read(): void {
    // Check if we've processed all pages
    if (this._currentPage >= this.endPage) {
      if (!this._ended) {
        this._ended = true;
        this.push(null);
      }
      return;
    }

    try {
      // Get page dimensions
      const dimensions = this.renderingManager.getPageDimensions(this._currentPage);

      // Get embedded resources
      const fonts = this.renderingManager.getEmbeddedFonts?.(this._currentPage) || [];
      const images = this.renderingManager.getEmbeddedImages?.(this._currentPage) || [];

      // Get rotation
      const rotation = dimensions?.rotation || 0;

      // Emit metadata object
      const metadata: PageMetadataData = {
        pageIndex: this._currentPage,
        width: dimensions?.width || 0,
        height: dimensions?.height || 0,
        fontCount: Array.isArray(fonts) ? fonts.length : 0,
        imageCount: Array.isArray(images) ? images.length : 0,
        rotation: rotation,
      };

      this._currentPage++;
      this.push(metadata);
    } catch (error) {
      this.destroy(error as Error);
    }
  }

  /**
   * Implement async iteration protocol for `for await...of` support
   * @returns AsyncGenerator for iterating over page metadata
   */
  async *[Symbol.asyncIterator](): AsyncGenerator<PageMetadataData, undefined, unknown> {
    // Process each page
    while (this._currentPage < this.endPage) {
      try {
        // Get page dimensions
        const dimensions = this.renderingManager.getPageDimensions(this._currentPage);

        // Get embedded resources
        const fonts = this.renderingManager.getEmbeddedFonts?.(this._currentPage) || [];
        const images = this.renderingManager.getEmbeddedImages?.(this._currentPage) || [];

        // Get rotation
        const rotation = dimensions?.rotation || 0;

        // Create metadata object
        const metadata: PageMetadataData = {
          pageIndex: this._currentPage,
          width: dimensions?.width || 0,
          height: dimensions?.height || 0,
          fontCount: Array.isArray(fonts) ? fonts.length : 0,
          imageCount: Array.isArray(images) ? images.length : 0,
          rotation: rotation,
        };

        this._currentPage++;

        yield metadata;
      } catch (error) {
        this.destroy(error as Error);
        return;
      }
    }

    if (!this._ended) {
      this._ended = true;
      this.destroy();
    }
  }
}

/**
 * Creates a readable stream for search results
 *
 * Convenience function to create a SearchStream instance.
 *
 * @param searchManager - The search manager
 * @param searchTerm - Text to search for
 * @param options - Search options
 * @returns A readable stream of search results
 *
 * @example
 * ```typescript
 * createSearchStream(manager, 'error')
 *   .pipe(through2.obj((result, enc, cb) => {
 *     console.log(`Found: ${result.text}`);
 *     cb();
 *   }));
 * ```
 */
export function createSearchStream(
  searchManager: any,
  searchTerm: string,
  options: Record<string, any> = {}
): SearchStream {
  return new SearchStream(searchManager, searchTerm, options);
}

/**
 * Creates a readable stream for extraction with progress
 *
 * Convenience function to create an ExtractionStream instance.
 *
 * @param extractionManager - The extraction manager
 * @param startPage - Starting page index
 * @param endPage - Ending page index
 * @param extractionType - Extraction format
 * @param options - Additional options
 * @returns A readable stream of extraction progress
 *
 * @example
 * ```typescript
 * createExtractionStream(manager, 0, 10, 'markdown')
 *   .pipe(through2.obj((progress, enc, cb) => {
 *     console.log(`${Math.round(progress.progress * 100)}% complete`);
 *     cb();
 *   }));
 * ```
 */
export function createExtractionStream(
  extractionManager: any,
  startPage: number,
  endPage: number,
  extractionType: 'text' | 'markdown' | 'html' = 'text',
  options: Record<string, any> = {}
): ExtractionStream {
  return new ExtractionStream(extractionManager, startPage, endPage, extractionType, options);
}

/**
 * Creates a readable stream for page metadata
 *
 * Convenience function to create a MetadataStream instance.
 *
 * @param renderingManager - The rendering manager
 * @param startPage - Starting page index
 * @param endPage - Ending page index
 * @returns A readable stream of page metadata
 *
 * @example
 * ```typescript
 * createMetadataStream(manager, 0, 10)
 *   .pipe(through2.obj((metadata, enc, cb) => {
 *     console.log(`Page ${metadata.pageIndex}: ${metadata.width}x${metadata.height}`);
 *     cb();
 *   }));
 * ```
 */
export function createMetadataStream(
  renderingManager: any,
  startPage: number,
  endPage: number
): MetadataStream {
  return new MetadataStream(renderingManager, startPage, endPage);
}
