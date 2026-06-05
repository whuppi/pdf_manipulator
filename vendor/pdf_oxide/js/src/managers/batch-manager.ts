/**
 * Batch Processing Manager for Parallel Document Operations
 *
 * Enables efficient parallel processing of multiple PDF documents with:
 * - Configurable concurrency control
 * - Real-time progress tracking with ETA calculation
 * - Memory-aware backpressure handling
 * - Per-document timeout support
 * - Error resilience and detailed reporting
 *
 * @example
 * ```typescript
 * import { BatchManager } from 'pdf-oxide';
 * import { PdfDocument } from 'pdf-oxide';
 *
 * const batch = new BatchManager([
 *   { path: 'doc1.pdf' },
 *   { path: 'doc2.pdf' },
 *   { path: 'doc3.pdf' }
 * ]);
 *
 * // Extract text from multiple documents in parallel
 * const results = await batch.extractTextBatch({
 *   maxParallel: 4,
 *   timeout: 30000,
 *   onProgress: (progress) => {
 *     console.log('Progress: ' + Math.round(progress.progress * 100) + '%');
 *     console.log('ETA: ' + progress.eta + 'ms');
 *   }
 * });
 *
 * results.forEach(result => {
 *   if (result.success) {
 *     console.log(result.document.path + ': ' + result.data.length + ' chars');
 *   } else {
 *     console.error(result.document.path + ': ' + result.error.message);
 *   }
 * });
 * ```
 */

import os from 'os';

/**
 * Represents a document to be processed in a batch
 */
export interface BatchDocument {
  /** File path to the PDF document */
  path: string;
  /** Optional unique identifier */
  id?: string;
  /** Priority (1-10, default 5) */
  priority?: number;
  /** Optional metadata */
  metadata?: Record<string, any>;
}

/**
 * Progress information for batch operations
 */
export interface BatchProgress {
  /** Total documents in batch */
  total: number;
  /** Number of successfully completed documents */
  completed: number;
  /** Number of failed documents */
  failed: number;
  /** Current document being processed (index) */
  current: number;
  /** Progress percentage (0.0-1.0) */
  progress: number;
  /** Estimated time remaining in milliseconds */
  eta: number;
  /** Number of currently active operations */
  activeOperations: number;
  /** Batch start time (milliseconds since epoch) */
  startTime: number;
  /** Elapsed time since start (milliseconds) */
  elapsedTime: number;
}

/**
 * Result of processing a single document in a batch
 */
export interface BatchResult<T = any> {
  /** The document that was processed */
  document: BatchDocument;
  /** Whether the operation succeeded */
  success: boolean;
  /** The result data if successful */
  data?: T;
  /** Error if operation failed */
  error?: Error;
  /** Time to process this document (milliseconds) */
  duration: number;
}

/**
 * Options for batch processing
 */
export interface BatchOptions {
  /** Maximum number of parallel operations (default: CPU count) */
  maxParallel?: number;
  /** Timeout per document in milliseconds (default: 30000) */
  timeout?: number;
  /** Progress callback invoked on each document completion */
  onProgress?: (progress: BatchProgress) => void;
  /** Backpressure configuration for memory management */
  backpressure?: {
    /** Maximum memory usage in MB (default: 500) */
    maxMemoryMB?: number;
    /** Interval to check memory in ms (default: 1000) */
    checkInterval?: number;
  };
}

/**
 * Statistics for completed batch operations
 */
export interface BatchStatistics {
  /** Total documents processed */
  total: number;
  /** Successfully completed documents */
  completed: number;
  /** Failed documents */
  failed: number;
  /** Total time elapsed (milliseconds) */
  totalTime: number;
  /** Average time per document (milliseconds) */
  averageTime: number;
  /** Documents per second throughput */
  throughput: number;
  /** Peak memory usage (MB) */
  peakMemory: number;
}

/**
 * Batch processor for parallel document operations
 */
export class BatchManager {
  private documents: BatchDocument[];
  private stats: BatchStatistics = {
    total: 0,
    completed: 0,
    failed: 0,
    totalTime: 0,
    averageTime: 0,
    throughput: 0,
    peakMemory: 0,
  };

  /**
   * Creates a new BatchManager
   * @param documents - Array of documents to process
   * @throws Error if documents array is empty or invalid
   */
  constructor(documents: BatchDocument[]) {
    if (!Array.isArray(documents) || documents.length === 0) {
      throw new Error('Documents array must not be empty');
    }

    for (const doc of documents) {
      if (!doc.path || typeof doc.path !== 'string') {
        throw new Error('Each document must have a valid path property');
      }
    }

    this.documents = documents;
    this.stats.total = documents.length;
  }

  /**
   * Get current statistics
   */
  getStatistics(): BatchStatistics {
    return { ...this.stats };
  }

  /**
   * Process documents in a queue with concurrency control
   * @private
   */
  private async processQueue<T>(
    processor: (doc: BatchDocument, index: number) => Promise<BatchResult<T>>,
    options: BatchOptions = {}
  ): Promise<BatchResult<T>[]> {
    const maxParallel = options.maxParallel || os.cpus().length;
    const timeout = options.timeout || 30000;
    const backpressure = options.backpressure || {
      maxMemoryMB: 500,
      checkInterval: 1000,
    };

    const results: BatchResult<T>[] = [];
    const startTime = Date.now();
    let completed = 0;
    let failed = 0;
    let active = 0;

    // Progress tracking helper
    const reportProgress = () => {
      const elapsedTime = Date.now() - startTime;
      const completedDocs = completed + failed;
      const avgTimePerDoc = completedDocs > 0 ? elapsedTime / completedDocs : 0;
      const eta = completedDocs > 0 ? (this.documents.length - completedDocs) * avgTimePerDoc : 0;

      if (options.onProgress) {
        options.onProgress({
          total: this.documents.length,
          completed,
          failed,
          current: completedDocs,
          progress: this.documents.length > 0 ? completedDocs / this.documents.length : 0,
          eta: Math.max(0, eta),
          activeOperations: active,
          startTime,
          elapsedTime,
        });
      }
    };

    // Memory monitoring helper
    const checkMemory = async (): Promise<void> => {
      const memUsageMB = process.memoryUsage().heapUsed / 1024 / 1024;
      this.stats.peakMemory = Math.max(this.stats.peakMemory, memUsageMB);

      if (memUsageMB > (backpressure.maxMemoryMB || 500)) {
        // Wait a bit for garbage collection
        await new Promise((resolve) => setTimeout(resolve, 100));
      }
    };

    // Process all documents with concurrency control
    let index = 0;
    const queue: Promise<void>[] = [];

    while (index < this.documents.length || queue.length > 0) {
      // Check memory before starting new operations
      await checkMemory();

      // Start new operations while under concurrency limit
      while (active < maxParallel && index < this.documents.length) {
        const docIndex = index++;
        const doc = this.documents[docIndex];

        active++;

        const promise = (async () => {
          const docStartTime = Date.now();
          try {
            const result = await Promise.race([
              processor(doc!, docIndex),
              new Promise<BatchResult<T>>((_, reject) =>
                setTimeout(() => reject(new Error('Timeout after ' + timeout + 'ms')), timeout)
              ),
            ]);

            result.duration = Date.now() - docStartTime;
            results[docIndex] = result;

            if (result.success) {
              completed++;
            } else {
              failed++;
            }
          } catch (error) {
            const duration = Date.now() - docStartTime;
            results[docIndex] = {
              document: doc!,
              success: false,
              error: error instanceof Error ? error : new Error(String(error)),
              duration,
            };
            failed++;
          } finally {
            active--;
            reportProgress();
          }
        })();

        queue.push(promise);
      }

      // Wait for at least one operation to complete
      if (queue.length > 0) {
        await Promise.race(queue);
        const idx = queue.findIndex((p) => p !== undefined);
        if (idx >= 0) {
          queue.splice(idx, 1);
        }
      }
    }

    // Update final statistics
    const totalTime = Date.now() - startTime;
    this.stats.totalTime = totalTime;
    this.stats.completed = completed;
    this.stats.failed = failed;
    this.stats.averageTime = completed > 0 ? totalTime / completed : 0;
    this.stats.throughput = totalTime > 0 ? (completed / totalTime) * 1000 : 0;

    return results.filter((r) => r !== undefined);
  }

  /**
   * Extract text from multiple documents in parallel
   * @param options - Batch processing options
   * @returns Array of extraction results
   */
  async extractTextBatch(options: BatchOptions = {}): Promise<BatchResult<string>[]> {
    return this.processQueue(async (doc, _index) => {
      try {
        // Dynamic import to avoid circular dependencies
        const { PdfDocument } = await import('../index.js');
        const pdfDoc = PdfDocument.open(doc.path);
        const extractionMgr = (pdfDoc as any).createExtractionManager?.();
        if (!extractionMgr) {
          throw new Error('Failed to create extraction manager');
        }
        const text = extractionMgr.extractAllText();
        if (typeof pdfDoc.close === 'function') {
          pdfDoc.close();
        }

        return {
          document: doc,
          success: true,
          data: text,
          duration: 0,
        };
      } catch (error) {
        return {
          document: doc,
          success: false,
          error: error instanceof Error ? error : new Error(String(error)),
          duration: 0,
        };
      }
    }, options);
  }

  /**
   * Extract markdown from multiple documents in parallel
   * @param options - Batch processing options
   * @returns Array of extraction results
   */
  async extractMarkdownBatch(options: BatchOptions = {}): Promise<BatchResult<string>[]> {
    return this.processQueue(async (doc, _index) => {
      try {
        const { PdfDocument } = await import('../index.js');
        const pdfDoc = PdfDocument.open(doc.path);
        const extractionMgr = (pdfDoc as any).createExtractionManager?.();
        if (!extractionMgr) {
          throw new Error('Failed to create extraction manager');
        }

        // Extract markdown from all pages
        let markdown = '';
        const pageCount = (pdfDoc as any).pageCount || 0;
        for (let i = 0; i < pageCount; i++) {
          markdown += extractionMgr.extractMarkdown(i) || '';
          if (i < pageCount - 1) {
            markdown += '\n\n---\n\n';
          }
        }
        if (typeof pdfDoc.close === 'function') {
          pdfDoc.close();
        }

        return {
          document: doc,
          success: true,
          data: markdown,
          duration: 0,
        };
      } catch (error) {
        return {
          document: doc,
          success: false,
          error: error instanceof Error ? error : new Error(String(error)),
          duration: 0,
        };
      }
    }, options);
  }

  /**
   * Extract HTML from multiple documents in parallel
   * @param options - Batch processing options
   * @returns Array of extraction results
   */
  async extractHtmlBatch(options: BatchOptions = {}): Promise<BatchResult<string>[]> {
    return this.processQueue(async (doc, _index) => {
      try {
        const { PdfDocument } = await import('../index.js');
        const pdfDoc = PdfDocument.open(doc.path);
        const extractionMgr = (pdfDoc as any).createExtractionManager?.();
        if (!extractionMgr) {
          throw new Error('Failed to create extraction manager');
        }

        // Extract HTML from all pages
        let html = '<html><body>';
        const pageCount = (pdfDoc as any).pageCount || 0;
        for (let i = 0; i < pageCount; i++) {
          html += '<div class="page page-' + (i + 1) + '">';
          html += extractionMgr.extractHtml(i) || '';
          html += '</div>';
        }
        html += '</body></html>';
        if (typeof pdfDoc.close === 'function') {
          pdfDoc.close();
        }

        return {
          document: doc,
          success: true,
          data: html,
          duration: 0,
        };
      } catch (error) {
        return {
          document: doc,
          success: false,
          error: error instanceof Error ? error : new Error(String(error)),
          duration: 0,
        };
      }
    }, options);
  }

  /**
   * Search for a term in multiple documents in parallel
   * @param searchTerm - Term to search for
   * @param options - Batch processing options
   * @returns Array of search results
   */
  async searchBatch(
    searchTerm: string,
    options: BatchOptions = {}
  ): Promise<BatchResult<Array<{ page: number; count: number }>>[]> {
    if (!searchTerm || typeof searchTerm !== 'string') {
      throw new Error('Search term must be a non-empty string');
    }

    return this.processQueue(async (doc, _index) => {
      try {
        const { PdfDocument } = await import('../index.js');
        const pdfDoc = PdfDocument.open(doc.path);
        const searchMgr = (pdfDoc as any).createSearchManager?.();
        if (!searchMgr) {
          throw new Error('Failed to create search manager');
        }

        const results: Array<{ page: number; count: number }> = [];
        const pageCount = (pdfDoc as any).pageCount || 0;
        for (let i = 0; i < pageCount; i++) {
          const matches = searchMgr.search(searchTerm, i) || [];
          if (matches.length > 0) {
            results.push({ page: i, count: matches.length });
          }
        }
        if (typeof pdfDoc.close === 'function') {
          pdfDoc.close();
        }

        return {
          document: doc,
          success: true,
          data: results,
          duration: 0,
        };
      } catch (error) {
        return {
          document: doc,
          success: false,
          error: error instanceof Error ? error : new Error(String(error)),
          duration: 0,
        };
      }
    }, options);
  }

  /**
   * Generic batch processor for custom operations
   * @param processor - Function to process each document
   * @param options - Batch processing options
   * @returns Array of results
   */
  async processBatch<T>(
    processor: (doc: BatchDocument, pdfDoc: any) => Promise<T>,
    options: BatchOptions = {}
  ): Promise<BatchResult<T>[]> {
    return this.processQueue(async (doc, _index) => {
      try {
        const { PdfDocument } = await import('../index.js');
        const pdfDoc = PdfDocument.open(doc.path);
        const data = await processor(doc, pdfDoc);
        if (typeof pdfDoc.close === 'function') {
          pdfDoc.close();
        }

        return {
          document: doc,
          success: true,
          data,
          duration: 0,
        };
      } catch (error) {
        return {
          document: doc,
          success: false,
          error: error instanceof Error ? error : new Error(String(error)),
          duration: 0,
        };
      }
    }, options);
  }
}
