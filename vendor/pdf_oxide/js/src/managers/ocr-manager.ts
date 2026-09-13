/**
 * OcrManager - Canonical OCR Manager (merged from 3 implementations)
 *
 * Consolidates:
 * - src/ocr-manager.ts (simple API with setLanguage, extractText, analyzePage)
 * - src/managers/ocr-compliance-cache.ts OCRManager (engine lifecycle)
 * - src/managers/ocr-manager-typed.ts OCRManager (full TypeScript, FFI-wired)
 *
 * Provides optical character recognition operations with complete type safety,
 * proper error handling, and full FFI integration.
 */

import { promises as fs } from 'fs';
import { dirname } from 'path';
import {
  BaseManager,
  type ManagerOptions,
  type OcrBatchResult,
  OcrLanguage,
  type OcrResult,
  type PdfDocumentHandle,
  type TextRegion,
} from '../types/manager-types.js';

export type { OcrBatchResult, OcrResult, TextRegion };
// Re-export types for convenience
export { OcrLanguage };

/**
 * OCR detection modes for accuracy/speed tradeoff
 */
export enum OcrDetectionMode {
  Accurate = 'accurate',
  Fast = 'fast',
  Balanced = 'balanced',
}

/**
 * Configuration for OCR operations
 */
export interface OcrConfig {
  language?: OcrLanguage;
  detectionMode?: OcrDetectionMode;
  detectionThreshold?: number;
  recognitionThreshold?: number;
  maxSideLen?: number;
  useGpu?: boolean;
  gpuDeviceId?: number;
}

/**
 * A recognized text span with position and confidence
 */
export interface OcrSpan {
  text: string;
  confidence: number;
  x: number;
  y: number;
  width: number;
  height: number;
  charCount: number;
}

/**
 * Analysis result for a single page
 */
export interface OcrPageAnalysis {
  pageIndex: number;
  needsOcr: boolean;
  confidence: number;
  spanCount: number;
  text: string;
}

/**
 * Canonical OcrManager - Comprehensive OCR with full TypeScript support
 *
 * Features:
 * - Full text recognition with confidence scoring
 * - Batch page processing with skip optimization
 * - Text region detection with coordinates
 * - Multi-language support
 * - Comprehensive event emission
 * - Automatic resource cleanup
 * - Legacy API compatibility (setLanguage, extractText, analyzePage, etc.)
 */
export class OcrManager extends BaseManager<PdfDocumentHandle> {
  private ocrEngine: unknown | null = null;
  private currentLanguage: OcrLanguage = OcrLanguage.ENGLISH;
  private preprocessingType: string = 'auto';
  private native: any;

  constructor(document: PdfDocumentHandle, options?: ManagerOptions) {
    super(document, options);
    try {
      this.native = require('../../index.node');
    } catch {
      this.native = null;
    }
  }

  // ==========================================================================
  // Engine Lifecycle (from typed version)
  // ==========================================================================

  /**
   * Initialize OCR engine with specified configuration
   */
  async initializeEngine(
    detectionThreshold: number = 0.5,
    recognitionThreshold: number = 0.5,
    maxSideLen: number = 960,
    useGpu: boolean = false,
    gpuDeviceId: number = 0
  ): Promise<boolean> {
    try {
      this.recordOperation();

      if (this.ocrEngine) {
        return true;
      }

      this.ocrEngine = await (this.document as any)?.createOcrEngine(
        detectionThreshold,
        recognitionThreshold,
        maxSideLen,
        useGpu,
        gpuDeviceId
      );

      if (this.ocrEngine) {
        this.emit('ocr-engine-initialized', {
          useGpu,
          gpuDeviceId,
          detectionThreshold,
          recognitionThreshold,
        });
        return true;
      }

      return false;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Destroy OCR engine and free resources
   */
  async destroyOcrEngine(): Promise<void> {
    try {
      this.recordOperation();

      if (this.ocrEngine) {
        await (this.document as any)?.destroyOcrEngine(this.ocrEngine);
        this.ocrEngine = null;
        this.emit('ocr-engine-destroyed', { timestamp: Date.now() });
      }
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  // ==========================================================================
  // Core Recognition (from typed version)
  // ==========================================================================

  /**
   * Check if page needs OCR processing
   */
  async pageNeedsOcr(pageIndex: number): Promise<boolean> {
    try {
      this.recordOperation();
      return (await (this.document as any)?.pageNeedsOcr(pageIndex)) || false;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Recognize text on a page with full confidence scoring
   */
  async recognizePage(pageIndex: number): Promise<string> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        throw new Error('OCR engine not initialized. Call initializeEngine() first.');
      }

      const text = await (this.document as any)?.recognizePage(pageIndex, this.ocrEngine);

      this.emit('page-recognized', {
        pageIndex,
        textLength: text?.length || 0,
        timestamp: Date.now(),
      });

      return text || '';
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Get OCR confidence score for a page
   */
  async getOcrConfidence(pageIndex: number): Promise<number> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        return 0;
      }

      return (await (this.document as any)?.getOcrConfidence(pageIndex)) || 0;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Detect text regions on a page with bounding boxes
   */
  async detectTextRegions(pageIndex: number): Promise<TextRegion[]> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        return [];
      }

      const regions = await (this.document as any)?.detectTextRegions(pageIndex, this.ocrEngine);

      return regions || [];
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  // ==========================================================================
  // Language Configuration (from typed + root versions)
  // ==========================================================================

  /**
   * Set OCR language for recognition (FFI-wired)
   */
  async setOcrLanguage(language: OcrLanguage | string): Promise<boolean> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        throw new Error('OCR engine not initialized');
      }

      const result = await (this.document as any)?.setOcrLanguage(this.ocrEngine, language);

      if (result) {
        this.currentLanguage = (language as OcrLanguage) || OcrLanguage.ENGLISH;
        this.emit('language-changed', {
          language,
          timestamp: Date.now(),
        });
      }

      return !!result;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Sets the OCR language (convenience alias for setOcrLanguage)
   * From root-level OCRManager
   */
  setLanguage(language: OcrLanguage): void {
    this.currentLanguage = language;
    this.invalidateCache('ocr');
    this.emit('languageChanged', language);
  }

  /**
   * Gets the current OCR language
   * From root-level OCRManager
   */
  getLanguage(): OcrLanguage {
    return this.currentLanguage;
  }

  /**
   * Get available OCR languages
   */
  async getAvailableLanguages(): Promise<OcrLanguage[]> {
    try {
      this.recordOperation();

      const languages =
        (await (this.document as any)?.getAvailableLanguages()) || Object.values(OcrLanguage);

      return languages;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  // ==========================================================================
  // Processing & Export (from typed version)
  // ==========================================================================

  /**
   * Preprocess page before OCR for better recognition
   */
  async preprocessPage(pageIndex: number, preprocessingType: string = 'auto'): Promise<boolean> {
    try {
      this.recordOperation();

      const result = await (this.document as any)?.preprocessPage(pageIndex, preprocessingType);

      this.preprocessingType = preprocessingType;
      this.emit('page-preprocessed', {
        pageIndex,
        type: preprocessingType,
        timestamp: Date.now(),
      });

      return !!result;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Export OCR text to file
   */
  async exportOcrText(
    pageIndex: number,
    filePath: string,
    format: 'txt' | 'json' | 'xml' = 'txt'
  ): Promise<boolean> {
    try {
      this.recordOperation();

      const text = await this.recognizePage(pageIndex);

      await fs.mkdir(dirname(filePath), { recursive: true });

      let content: string;
      switch (format) {
        case 'json':
          content = JSON.stringify({ pageIndex, text, timestamp: Date.now() }, null, 2);
          break;
        case 'xml':
          content = `<?xml version="1.0"?>\n<page index="${pageIndex}">\n${text
            .split('\n')
            .map((line) => `  <line>${line}</line>`)
            .join('\n')}\n</page>`;
          break;
        default:
          content = text;
      }

      await fs.writeFile(filePath, content, 'utf8');

      this.emit('text-exported', {
        pageIndex,
        filePath,
        format,
        size: content.length,
        timestamp: Date.now(),
      });

      return true;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  // ==========================================================================
  // Statistics & Batch (from typed version)
  // ==========================================================================

  /**
   * Get comprehensive OCR statistics for a page
   */
  async getOcrStatistics(pageIndex: number): Promise<OcrResult> {
    try {
      this.recordOperation();

      const text = await this.recognizePage(pageIndex);
      const confidence = await this.getOcrConfidence(pageIndex);
      const regions = await this.detectTextRegions(pageIndex);

      return {
        pageIndex,
        text,
        confidence,
        regionCount: regions.length,
      };
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Batch recognize multiple pages
   */
  async batchRecognizePages(startPage: number, endPage: number): Promise<Map<number, string>> {
    try {
      this.recordOperation();

      const results = new Map<number, string>();

      for (let i = startPage; i <= endPage; i++) {
        const text = await this.recognizePage(i);
        results.set(i, text);
      }

      this.emit('batch-recognized', {
        startPage,
        endPage,
        pageCount: endPage - startPage + 1,
        totalCharacters: Array.from(results.values()).reduce((s, t) => s + t.length, 0),
        timestamp: Date.now(),
      });

      return results;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Extract OCR text with aggregated statistics from page range (FFI-wired)
   */
  async extractPageRange(
    startPage: number,
    endPage: number,
    skipNonScanned: boolean = true
  ): Promise<OcrBatchResult> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        throw new Error('OCR engine not initialized');
      }

      let totalSpans = 0;
      let confidenceSum = 0;
      let skippedPages = 0;

      for (let pageIdx = startPage; pageIdx <= endPage; pageIdx++) {
        try {
          if (skipNonScanned) {
            const needsOcr = await this.pageNeedsOcr(pageIdx);
            if (!needsOcr) {
              skippedPages++;
              continue;
            }
          }

          const text = await this.recognizePage(pageIdx);
          const confidence = await this.getOcrConfidence(pageIdx);
          const regions = await this.detectTextRegions(pageIdx);
          totalSpans += Math.max(regions.length, text ? 1 : 0);
          confidenceSum += confidence;
        } catch {}
      }

      const processedPages = endPage - startPage + 1 - skippedPages;
      const avgConfidence = processedPages > 0 ? confidenceSum / processedPages : 0;

      const result: OcrBatchResult = {
        startPage,
        endPage,
        totalPages: endPage - startPage + 1,
        totalSpans,
        averageConfidence: avgConfidence,
        skippedPages,
      };

      this.emit('page-range-extracted', {
        ...result,
        timestamp: Date.now(),
      });

      this.setCached(`ocr-batch:${startPage}-${endPage}`, result);

      return result;
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  // ==========================================================================
  // Engine Status & Configuration (from typed version)
  // ==========================================================================

  /**
   * Get OCR engine status and configuration
   */
  async getEngineStatus(): Promise<string> {
    try {
      this.recordOperation();

      if (!this.ocrEngine) {
        return 'not_initialized';
      }

      return (await (this.document as any)?.getEngineStatus(this.ocrEngine)) || 'unknown';
    } catch (error) {
      this.recordError(error instanceof Error ? error : new Error(String(error)));
      throw error;
    }
  }

  /**
   * Get current OCR configuration
   */
  getConfiguration(): {
    language: OcrLanguage;
    preprocessingType: string;
    engineInitialized: boolean;
  } {
    return {
      language: this.currentLanguage,
      preprocessingType: this.preprocessingType,
      engineInitialized: !!this.ocrEngine,
    };
  }

  // ==========================================================================
  // Methods from root-level OCRManager
  // ==========================================================================

  /**
   * Extracts text from a page (convenience alias for recognizePage)
   * From root-level OCRManager
   */
  async extractText(pageIndex: number, config?: OcrConfig): Promise<string> {
    const cacheKey = `ocr:text:${pageIndex}:${this.currentLanguage}`;
    const cached = this.getCached<string>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    let result = '';
    if ((this.document as any)?.extractText) {
      result = (this.document as any).extractText(pageIndex) || '';
    }
    this.setCached(cacheKey, result);
    this.emit('textExtracted', pageIndex, result.length);
    return result;
  }

  /**
   * Analyzes a page and returns detailed results
   * From root-level OCRManager
   */
  async analyzePage(pageIndex: number, config?: OcrConfig): Promise<OcrPageAnalysis> {
    const cacheKey = `ocr:analysis:${pageIndex}:${this.currentLanguage}`;
    const cached = this.getCached<OcrPageAnalysis>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    let text = '';
    let needsOcr = false;

    if ((this.document as any)?.extractText) {
      text = (this.document as any).extractText(pageIndex) || '';
      needsOcr = !text || text.trim().length < 10;
    }

    const result: OcrPageAnalysis = {
      pageIndex,
      needsOcr,
      confidence: needsOcr ? 0.0 : 0.95,
      spanCount: text.split(' ').length || 0,
      text,
    };
    this.setCached(cacheKey, result);
    this.emit('pageAnalyzed', pageIndex, result);
    return result;
  }

  /**
   * Performs OCR analysis on all pages in the document
   * From root-level OCRManager
   */
  async analyzeDocument(config?: OcrConfig): Promise<OcrPageAnalysis[]> {
    const cacheKey = `ocr:document:${this.currentLanguage}`;
    const cached = this.getCached<OcrPageAnalysis[]>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    const results: OcrPageAnalysis[] = [];
    const pageCount = (this.document as any)?.pageCount || 0;

    for (let i = 0; i < pageCount; i++) {
      const analysis = await this.analyzePage(i, config);
      results.push(analysis);
      this.emit('pageProcessed', i + 1, pageCount);
    }

    this.setCached(cacheKey, results);
    this.emit('documentAnalyzed', results.length);
    return results;
  }

  /**
   * Extracts text spans with bounding boxes for a page
   * From root-level OCRManager
   */
  async extractSpans(pageIndex: number, config?: OcrConfig): Promise<OcrSpan[]> {
    const cacheKey = `ocr:spans:${pageIndex}:${this.currentLanguage}`;
    const cached = this.getCached<OcrSpan[]>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    let spans: OcrSpan[] = [];
    if (this.native?.extract_spans) {
      try {
        const spansJson = this.native.extract_spans(pageIndex) ?? [];
        spans = spansJson.length > 0 ? spansJson.map((json: string) => JSON.parse(json)) : [];
      } catch {
        spans = [];
      }
    }

    this.setCached(cacheKey, spans);
    this.emit('spansExtracted', { page: pageIndex, count: spans.length });
    return spans;
  }

  /**
   * Checks if OCR is available/installed
   * From root-level OCRManager
   */
  async isAvailable(): Promise<boolean> {
    const cacheKey = 'ocr:available';
    const cached = this.getCached<boolean>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    const result = this.native ? true : false;
    this.setCached(cacheKey, result);
    return result;
  }

  /**
   * Gets OCR engine version
   * From root-level OCRManager
   */
  async getVersion(): Promise<string> {
    const cacheKey = 'ocr:version';
    const cached = this.getCached<string>(cacheKey);
    if (cached !== undefined) {
      return cached;
    }

    let version = '0.0.0';
    if (this.native?.get_ocr_version) {
      try {
        version = this.native.get_ocr_version() ?? '0.0.0';
      } catch {
        version = '0.0.0';
      }
    }

    this.setCached(cacheKey, version);
    return version;
  }

  // ==========================================================================
  // Cache Operations (from root-level OCRManager)
  // ==========================================================================

  /**
   * Clears the result cache
   */
  clearCache(): void {
    this.invalidateCache();
    this.emit('cacheCleared');
  }

  /**
   * Gets cache statistics
   */
  getCacheStats(): Record<string, any> {
    return {
      cacheSize: this.cache.size,
      entries: Array.from(this.cache.keys()),
    };
  }

  // ==========================================================================
  // Cleanup
  // ==========================================================================

  /**
   * Cleanup on destroy
   */
  async destroy(): Promise<void> {
    try {
      await this.destroyOcrEngine();
      this.invalidateCache();
      this.removeAllListeners();
      this.initialized = false;
    } catch (error) {
      console.error('Error during OCR manager cleanup:', error);
    }
  }
}

/** @deprecated Use OcrManager instead */
export const OCRManager = OcrManager;

export default OcrManager;
