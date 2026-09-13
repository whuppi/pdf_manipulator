/**
 * Manager for content extraction from PDF documents
 *
 * Caching is handled automatically at the Rust FFI layer, eliminating
 * the need for duplicate cache implementations in the binding.
 *
 * @example
 * ```typescript
 * import { ExtractionManager, ConversionOptionsBuilder } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const extractionManager = new ExtractionManager(doc);
 *
 * // Extract text from a single page
 * const text = extractionManager.extractText(0);
 * console.log(text);
 *
 * // Extract all text
 * const allText = extractionManager.extractAllText();
 *
 * // Extract with custom options
 * const options = ConversionOptionsBuilder.highQuality().build();
 * const markdown = extractionManager.extractMarkdown(0, options);
 * ```
 */

export interface ContentStatistics {
  pageCount: number;
  wordCount: number;
  characterCount: number;
  averageWordsPerPage: number;
  averageCharactersPerPage: number;
}

export interface SearchMatch {
  pageIndex: number;
  pageNumber: number;
  matchIndex: number;
  snippet: string;
  matchText: string;
}

export class ExtractionManager {
  private _document: any;

  /**
   * Creates a new ExtractionManager for the given document
   * @param document - The PDF document
   * @throws Error if document is null or undefined
   */
  constructor(document: any) {
    if (!document) {
      throw new Error('Document is required');
    }
    this._document = document;
  }

  /**
   * Extracts text from a single page.
   *
   * The native layer produces UTF-8 bytes, which Node decodes into a JS
   * `string` (UTF-16 code units internally). As a result,
   * `text.length` reports UTF-16 code units, not bytes — so a 648-byte
   * UTF-8 string containing two accented letters reads as 646 in JS. Use
   * `Buffer.byteLength(text, 'utf8')` if you need the byte count (e.g. to
   * compare against Go's `len(string)` or Rust's `String::len()`).
   *
   * Results are automatically cached at the FFI layer.
   *
   * @param pageIndex - Zero-based page index
   * @param options - Conversion options
   * @returns Extracted text (UTF-16 code units)
   * @throws Error if page index is invalid
   *
   * @example
   * ```typescript
   * const text = manager.extractText(0);
   * console.log(`Page 1: ${text.length} UTF-16 code units`);
   * console.log(`         ${Buffer.byteLength(text, 'utf8')} UTF-8 bytes`);
   * ```
   */
  extractText(pageIndex: number, options?: Record<string, any>): string {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    try {
      return this._document.extractText(pageIndex);
    } catch (error) {
      throw new Error(`Failed to extract text from page ${pageIndex}: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts text from all pages
   * @param options - Conversion options
   * @returns All extracted text concatenated
   *
   * @example
   * ```typescript
   * const allText = manager.extractAllText();
   * console.log(`Total characters: ${allText.length}`);
   * ```
   */
  extractAllText(options?: Record<string, any>): string {
    try {
      const parts: string[] = [];
      for (let i = 0; i < this._document.pageCount; i++) {
        parts.push(this.extractText(i, options));
      }
      return parts.join('\n');
    } catch (error) {
      throw new Error(`Failed to extract all text: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts text from a range of pages
   * @param startPageIndex - Zero-based start page index
   * @param endPageIndex - Zero-based end page index (inclusive)
   * @param options - Conversion options
   * @returns Extracted text from pages in range
   *
   * @example
   * ```typescript
   * const text = manager.extractTextRange(0, 10);
   * console.log(`Text from pages 1-11: ${text}`);
   * ```
   */
  extractTextRange(
    startPageIndex: number,
    endPageIndex: number,
    options?: Record<string, any>
  ): string {
    if (typeof startPageIndex !== 'number' || startPageIndex < 0) {
      throw new Error('Start page index must be a non-negative number');
    }

    if (typeof endPageIndex !== 'number' || endPageIndex < startPageIndex) {
      throw new Error('End page index must be >= start page index');
    }

    if (endPageIndex >= this._document.pageCount) {
      throw new Error(`End page index ${endPageIndex} out of range`);
    }

    try {
      const parts: string[] = [];
      for (let i = startPageIndex; i <= endPageIndex; i++) {
        parts.push(this.extractText(i, options));
      }
      return parts.join('\n');
    } catch (error) {
      throw new Error(`Failed to extract text range: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts text from specific page indices (non-contiguous)
   * @param pageIndices - Array of zero-based page indices
   * @param options - Conversion options
   * @returns Extracted text from specified pages concatenated with newlines
   * @throws Error if page indices are invalid
   *
   * @example
   * ```typescript
   * const text = manager.extractTextBatch([0, 2, 5]); // Extract pages 1, 3, 6
   * console.log(text);
   * ```
   */
  extractTextBatch(pageIndices: number[], options?: Record<string, any>): string {
    if (!Array.isArray(pageIndices)) {
      throw new Error('Page indices must be an array');
    }

    if (pageIndices.length === 0) {
      return '';
    }

    try {
      const parts: string[] = [];
      for (const pageIndex of pageIndices) {
        if (
          typeof pageIndex !== 'number' ||
          pageIndex < 0 ||
          pageIndex >= this._document.pageCount
        ) {
          throw new Error(`Invalid page index: ${pageIndex}`);
        }
        parts.push(this.extractText(pageIndex, options));
      }
      return parts.join('\n');
    } catch (error) {
      throw new Error(`Failed to extract text batch: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts text from pages as an array (one entry per page)
   * @param startPageIndex - Zero-based start page index
   * @param endPageIndex - Zero-based end page index (inclusive)
   * @param options - Conversion options
   * @returns Array of extracted text, one per page
   *
   * @example
   * ```typescript
   * const pages = manager.extractTextArray(0, 5);
   * pages.forEach((text, i) => console.log(`Page ${i}: ${text.length} chars`));
   * ```
   */
  extractTextArray(
    startPageIndex: number,
    endPageIndex: number,
    options?: Record<string, any>
  ): string[] {
    if (typeof startPageIndex !== 'number' || startPageIndex < 0) {
      throw new Error('Start page index must be a non-negative number');
    }

    if (typeof endPageIndex !== 'number' || endPageIndex < startPageIndex) {
      throw new Error('End page index must be >= start page index');
    }

    if (endPageIndex >= this._document.pageCount) {
      throw new Error(`End page index ${endPageIndex} out of range`);
    }

    try {
      const results: string[] = [];
      for (let i = startPageIndex; i <= endPageIndex; i++) {
        results.push(this.extractText(i, options));
      }
      return results;
    } catch (error) {
      throw new Error(`Failed to extract text array: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts page as Markdown.
   * Results are automatically cached at the FFI layer.
   * @param pageIndex - Zero-based page index
   * @param options - Conversion options
   * @returns Page content as Markdown
   * @throws Error if page index is invalid
   *
   * @example
   * ```typescript
   * const markdown = manager.extractMarkdown(0);
   * console.log(markdown); // Markdown formatted content
   * ```
   */
  extractMarkdown(pageIndex: number, options?: Record<string, any>): string {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    try {
      return this._document.toMarkdown(pageIndex, options);
    } catch (error) {
      throw new Error(
        `Failed to extract markdown from page ${pageIndex}: ${(error as Error).message}`
      );
    }
  }

  /**
   * Extracts all pages as Markdown
   * @param options - Conversion options
   * @returns All pages as Markdown
   *
   * @example
   * ```typescript
   * const markdown = manager.extractAllMarkdown();
   * // Write to file
   * fs.writeFileSync('output.md', markdown);
   * ```
   */
  extractAllMarkdown(options?: Record<string, any>): string {
    try {
      const parts: string[] = [];
      for (let i = 0; i < this._document.pageCount; i++) {
        const heading = `\n## Page ${i + 1}\n`;
        const content = this.extractMarkdown(i, options);
        parts.push(heading + content);
      }
      return parts.join('\n');
    } catch (error) {
      throw new Error(`Failed to extract all markdown: ${(error as Error).message}`);
    }
  }

  /**
   * Extracts markdown from a range of pages
   * @param startPageIndex - Zero-based start page index
   * @param endPageIndex - Zero-based end page index (inclusive)
   * @param options - Conversion options
   * @returns Extracted markdown from pages in range
   */
  extractMarkdownRange(
    startPageIndex: number,
    endPageIndex: number,
    options?: Record<string, any>
  ): string {
    if (typeof startPageIndex !== 'number' || startPageIndex < 0) {
      throw new Error('Start page index must be a non-negative number');
    }

    if (typeof endPageIndex !== 'number' || endPageIndex < startPageIndex) {
      throw new Error('End page index must be >= start page index');
    }

    if (endPageIndex >= this._document.pageCount) {
      throw new Error(`End page index ${endPageIndex} out of range`);
    }

    try {
      const parts: string[] = [];
      for (let i = startPageIndex; i <= endPageIndex; i++) {
        const heading = `\n## Page ${i + 1}\n`;
        const content = this.extractMarkdown(i, options);
        parts.push(heading + content);
      }
      return parts.join('\n');
    } catch (error) {
      throw new Error(`Failed to extract markdown range: ${(error as Error).message}`);
    }
  }

  /**
   * Gets word count for a page
   * @param pageIndex - Zero-based page index
   * @returns Estimated word count
   */
  getPageWordCount(pageIndex: number): number {
    const text = this.extractText(pageIndex);
    return text.trim().split(/\s+/).length;
  }

  /**
   * Gets total word count for all pages
   * @returns Total word count across all pages
   */
  getTotalWordCount(): number {
    const allText = this.extractAllText();
    return allText
      .trim()
      .split(/\s+/)
      .filter((word) => word.length > 0).length;
  }

  /**
   * Gets character count for a page
   * @param pageIndex - Zero-based page index
   * @returns Character count (including whitespace)
   */
  getPageCharacterCount(pageIndex: number): number {
    const text = this.extractText(pageIndex);
    return text.length;
  }

  /**
   * Gets total character count for all pages
   * @returns Total character count
   */
  getTotalCharacterCount(): number {
    let total = 0;
    for (let i = 0; i < this._document.pageCount; i++) {
      total += this.getPageCharacterCount(i);
    }
    return total;
  }

  /**
   * Gets line count for a page
   * @param pageIndex - Zero-based page index
   * @returns Estimated line count
   */
  getPageLineCount(pageIndex: number): number {
    const text = this.extractText(pageIndex);
    return text.split('\n').length;
  }

  /**
   * Gets statistics for extracted content
   * @returns Content statistics object
   *
   * @example
   * ```typescript
   * const stats = manager.getContentStatistics();
   * console.log(`Total pages: ${stats.pageCount}`);
   * console.log(`Total words: ${stats.wordCount}`);
   * console.log(`Average page length: ${stats.averagePageLength}`);
   * ```
   */
  getContentStatistics(): ContentStatistics {
    try {
      const pageCount = this._document.pageCount;
      const totalWords = this.getTotalWordCount();
      const totalCharacters = this.getTotalCharacterCount();

      return {
        pageCount,
        wordCount: totalWords,
        characterCount: totalCharacters,
        averageWordsPerPage: Math.round(totalWords / pageCount),
        averageCharactersPerPage: Math.round(totalCharacters / pageCount),
      };
    } catch (error) {
      throw new Error(`Failed to get content statistics: ${(error as Error).message}`);
    }
  }

  /**
   * Searches for text across all pages and returns matching snippets
   * @param searchText - Text to search for
   * @param contextLength - Characters of context around match
   * @returns Array of match objects with page and snippet
   *
   * @example
   * ```typescript
   * const matches = manager.searchContent('keyword', 50);
   * matches.forEach(match => {
   *   console.log(`Page ${match.pageIndex + 1}: ...${match.snippet}...`);
   * });
   * ```
   */
  searchContent(searchText: string, contextLength: number = 100): SearchMatch[] {
    if (!searchText || typeof searchText !== 'string') {
      throw new Error('Search text must be a non-empty string');
    }

    const results: SearchMatch[] = [];
    const searchRegex = new RegExp(searchText, 'gi');

    for (let i = 0; i < this._document.pageCount; i++) {
      try {
        const text = this.extractText(i);
        let match;

        while ((match = searchRegex.exec(text)) !== null) {
          const start = Math.max(0, match.index - contextLength);
          const end = Math.min(text.length, match.index + searchText.length + contextLength);
          const snippet = text.substring(start, end);

          results.push({
            pageIndex: i,
            pageNumber: i + 1,
            matchIndex: match.index,
            snippet: snippet.replace(/\n/g, ' '),
            matchText: match[0],
          });
        }

        // Reset regex for next iteration
        searchRegex.lastIndex = 0;
      } catch (e) {
        // Skip pages that fail extraction
      }
    }

    return results;
  }

  /**
   * Extract text from a page in a worker thread (non-blocking)
   * @param documentPath - Path to the PDF document
   * @param pageIndex - Page index to extract from
   * @param options - Optional extraction options
   * @param timeout - Optional timeout in milliseconds
   * @returns Promise resolving to extracted text
   */
  async extractTextInWorker(
    documentPath: string,
    pageIndex: number,
    options?: Record<string, any>,
    timeout?: number
  ): Promise<string> {
    const { workerPool } = await import('../workers/index.js');

    const result = await workerPool.runTask(
      {
        operation: 'extract',
        documentPath,
        params: {
          type: 'text',
          pageIndex,
          options: options || {},
        },
      },
      timeout
    );

    if (!result.success) {
      throw new Error(
        `Worker extraction failed: ${
          result.error instanceof Error ? result.error.message : String(result.error)
        }`
      );
    }

    return result.data as string;
  }

  /**
   * Extract markdown from a page in a worker thread (non-blocking)
   * @param documentPath - Path to the PDF document
   * @param pageIndex - Page index to extract from
   * @param options - Optional extraction options
   * @param timeout - Optional timeout in milliseconds
   * @returns Promise resolving to extracted markdown
   */
  async extractMarkdownInWorker(
    documentPath: string,
    pageIndex: number,
    options?: Record<string, any>,
    timeout?: number
  ): Promise<string> {
    const { workerPool } = await import('../workers/index.js');

    const result = await workerPool.runTask(
      {
        operation: 'extract',
        documentPath,
        params: {
          type: 'markdown',
          pageIndex,
          options: options || {},
        },
      },
      timeout
    );

    if (!result.success) {
      throw new Error(
        `Worker extraction failed: ${
          result.error instanceof Error ? result.error.message : String(result.error)
        }`
      );
    }

    return result.data as string;
  }

  /**
   * Extract HTML from a page in a worker thread (non-blocking)
   * @param documentPath - Path to the PDF document
   * @param pageIndex - Page index to extract from
   * @param options - Optional extraction options
   * @param timeout - Optional timeout in milliseconds
   * @returns Promise resolving to extracted HTML
   */
  async extractHtmlInWorker(
    documentPath: string,
    pageIndex: number,
    options?: Record<string, any>,
    timeout?: number
  ): Promise<string> {
    const { workerPool } = await import('../workers/index.js');

    const result = await workerPool.runTask(
      {
        operation: 'extract',
        documentPath,
        params: {
          type: 'html',
          pageIndex,
          options: options || {},
        },
      },
      timeout
    );

    if (!result.success) {
      throw new Error(
        `Worker extraction failed: ${
          result.error instanceof Error ? result.error.message : String(result.error)
        }`
      );
    }

    return result.data as string;
  }
}
