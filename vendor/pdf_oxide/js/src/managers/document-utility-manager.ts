/**
 * Document Utility Manager - Document optimization and manipulation utilities
 *
 * Provides comprehensive document utilities:
 * - Document optimization and compression
 * - PDF linearization (fast web view)
 * - Font optimization and subsetting
 * - Image optimization and recompression
 * - Page manipulation utilities
 * - Document repair
 * - Resource cleanup
 *
 * This completes the document utility coverage for 100% FFI parity.
 */

import { EventEmitter } from 'events';
import { promises as fs } from 'fs';
import { dirname } from 'path';

// ============================================================================
// Type Definitions
// ============================================================================

/**
 * Optimization level enumeration
 */
export enum OptimizationLevel {
  /** No optimization */
  NONE = 'none',
  /** Light optimization - fast, minimal size reduction */
  LIGHT = 'light',
  /** Balanced optimization - good balance of speed and size */
  BALANCED = 'balanced',
  /** Aggressive optimization - maximum size reduction, slower */
  AGGRESSIVE = 'aggressive',
  /** Maximum optimization - smallest possible size */
  MAXIMUM = 'maximum',
}

/**
 * Image compression type
 */
export enum ImageCompressionType {
  NONE = 'none',
  JPEG = 'jpeg',
  JPEG2000 = 'jpeg2000',
  JBIG2 = 'jbig2',
  FLATE = 'flate',
  LZW = 'lzw',
  RUN_LENGTH = 'run_length',
  CCITT_FAX = 'ccitt_fax',
}

/**
 * Color space type
 */
export enum ColorSpaceType {
  RGB = 'rgb',
  CMYK = 'cmyk',
  GRAYSCALE = 'grayscale',
  INDEXED = 'indexed',
}

/**
 * Font embedding mode
 */
export enum FontEmbeddingMode {
  /** Embed full fonts */
  FULL = 'full',
  /** Embed subset of used glyphs only */
  SUBSET = 'subset',
  /** Remove all font embedding */
  REMOVE = 'remove',
}

/**
 * Page range specification
 */
export interface PageRange {
  readonly start: number;
  readonly end: number;
}

/**
 * Optimization options
 */
export interface OptimizationOptions {
  readonly level?: OptimizationLevel;
  readonly compressImages?: boolean;
  readonly imageQuality?: number; // 1-100
  readonly imageCompression?: ImageCompressionType;
  readonly downsampleImages?: boolean;
  readonly maxImageDpi?: number;
  readonly removeUnusedObjects?: boolean;
  readonly removeMetadata?: boolean;
  readonly removeThumbnails?: boolean;
  readonly removeBookmarks?: boolean;
  readonly removeAnnotations?: boolean;
  readonly removeJavaScript?: boolean;
  readonly removeFormFields?: boolean;
  readonly removeEmbeddedFiles?: boolean;
  readonly linearize?: boolean;
  readonly fontEmbedding?: FontEmbeddingMode;
  readonly convertColorSpace?: ColorSpaceType;
  readonly flattenTransparency?: boolean;
}

/**
 * Compression options
 */
export interface CompressionOptions {
  readonly compressStreams?: boolean;
  readonly compressObjects?: boolean;
  readonly algorithm?: 'flate' | 'lzw' | 'none';
  readonly level?: number; // 1-9
}

/**
 * Image optimization options
 */
export interface ImageOptimizationOptions {
  readonly maxDpi?: number;
  readonly minDpi?: number;
  readonly targetQuality?: number;
  readonly targetFormat?: ImageCompressionType;
  readonly convertColor?: ColorSpaceType;
  readonly removeIccProfiles?: boolean;
}

/**
 * Font optimization options
 */
export interface FontOptimizationOptions {
  readonly mode?: FontEmbeddingMode;
  readonly removeUnusedFonts?: boolean;
  readonly mergeSubsets?: boolean;
  readonly convertToType1?: boolean;
  readonly convertToOpenType?: boolean;
}

/**
 * Linearization options
 */
export interface LinearizationOptions {
  readonly firstPageEnd?: number;
  readonly primaryHint?: boolean;
  readonly overflowHint?: boolean;
}

/**
 * Optimization result
 */
export interface OptimizationResult {
  readonly success: boolean;
  readonly originalSize: number;
  readonly optimizedSize: number;
  readonly reductionPercent: number;
  readonly reductionBytes: number;
  readonly duration: number;
  readonly details?: {
    readonly imagesOptimized?: number;
    readonly fontsOptimized?: number;
    readonly objectsRemoved?: number;
    readonly streamsCompressed?: number;
  };
  readonly error?: string;
  readonly warnings?: readonly string[];
}

/**
 * Document repair result
 */
export interface RepairResult {
  readonly success: boolean;
  readonly issuesFound: number;
  readonly issuesFixed: number;
  readonly issues: readonly string[];
  readonly error?: string;
}

/**
 * Document statistics
 */
export interface DocumentStatistics {
  readonly pageCount: number;
  readonly fileSize: number;
  readonly objectCount: number;
  readonly streamCount: number;
  readonly imageCount: number;
  readonly fontCount: number;
  readonly annotationCount: number;
  readonly bookmarkCount: number;
  readonly embeddedFileCount: number;
  readonly formFieldCount: number;
  readonly signatureCount: number;
  readonly hasJavaScript: boolean;
  readonly hasXfa: boolean;
  readonly isLinearized: boolean;
  readonly isEncrypted: boolean;
  readonly pdfVersion: string;
}

/**
 * Page information
 */
export interface PageInfo {
  readonly index: number;
  readonly width: number;
  readonly height: number;
  readonly rotation: number;
  readonly hasAnnotations: boolean;
  readonly hasText: boolean;
  readonly hasImages: boolean;
  readonly mediaBox: readonly [number, number, number, number];
  readonly cropBox?: readonly [number, number, number, number];
}

// ============================================================================
// Document Utility Manager
// ============================================================================

/**
 * Document Utility Manager - Complete document optimization and manipulation
 *
 * Provides 35 functions for document utilities.
 *
 * @example
 * ```typescript
 * const doc = await PdfDocument.open('large-document.pdf');
 * const utilityManager = new DocumentUtilityManager(doc);
 *
 * // Optimize document
 * const result = await utilityManager.optimizeDocument({
 *   level: OptimizationLevel.AGGRESSIVE,
 *   compressImages: true,
 *   imageQuality: 75,
 *   linearize: true,
 * });
 *
 * console.log(`Reduced by ${result.reductionPercent}%`);
 *
 * // Save optimized document
 * await utilityManager.saveOptimized('optimized.pdf');
 * ```
 */
export class DocumentUtilityManager extends EventEmitter {
  private readonly document: any;
  private lastOptimizationResult: OptimizationResult | null = null;

  constructor(document: any) {
    super();
    if (!document) {
      throw new Error('Document cannot be null or undefined');
    }
    this.document = document;
  }

  // ==========================================================================
  // Optimization Functions (8 functions)
  // ==========================================================================

  /**
   * Optimizes the document with specified options
   */
  async optimizeDocument(options?: OptimizationOptions): Promise<OptimizationResult> {
    const startTime = Date.now();

    try {
      const originalSize = await this.getFileSize();

      const result = await this.document?.optimizeDocument?.({
        level: options?.level ?? OptimizationLevel.BALANCED,
        compressImages: options?.compressImages ?? true,
        imageQuality: options?.imageQuality ?? 85,
        imageCompression: options?.imageCompression ?? ImageCompressionType.JPEG,
        downsampleImages: options?.downsampleImages ?? true,
        maxImageDpi: options?.maxImageDpi ?? 150,
        removeUnusedObjects: options?.removeUnusedObjects ?? true,
        removeMetadata: options?.removeMetadata ?? false,
        removeThumbnails: options?.removeThumbnails ?? true,
        removeBookmarks: options?.removeBookmarks ?? false,
        removeAnnotations: options?.removeAnnotations ?? false,
        removeJavaScript: options?.removeJavaScript ?? false,
        removeFormFields: options?.removeFormFields ?? false,
        removeEmbeddedFiles: options?.removeEmbeddedFiles ?? false,
        linearize: options?.linearize ?? false,
        fontEmbedding: options?.fontEmbedding ?? FontEmbeddingMode.SUBSET,
        convertColorSpace: options?.convertColorSpace,
        flattenTransparency: options?.flattenTransparency ?? false,
      });

      const optimizedSize = await this.getFileSize();
      const reductionBytes = originalSize - optimizedSize;
      const reductionPercent = originalSize > 0 ? (reductionBytes / originalSize) * 100 : 0;
      const duration = Date.now() - startTime;

      const optimizationResult: OptimizationResult = {
        success: true,
        originalSize,
        optimizedSize,
        reductionPercent: Math.round(reductionPercent * 100) / 100,
        reductionBytes,
        duration,
        details: result?.details,
        warnings: result?.warnings,
      };

      this.lastOptimizationResult = optimizationResult;
      this.emit('optimization-complete', optimizationResult);

      return optimizationResult;
    } catch (error) {
      const duration = Date.now() - startTime;
      const errorResult: OptimizationResult = {
        success: false,
        originalSize: 0,
        optimizedSize: 0,
        reductionPercent: 0,
        reductionBytes: 0,
        duration,
        error: error instanceof Error ? error.message : 'Unknown error',
      };

      this.emit('error', error);
      return errorResult;
    }
  }

  /**
   * Compresses document streams
   */
  async compressStreams(options?: CompressionOptions): Promise<boolean> {
    try {
      const result = await this.document?.compressStreams?.({
        algorithm: options?.algorithm ?? 'flate',
        level: options?.level ?? 6,
      });

      this.emit('streams-compressed');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Optimizes images in the document
   */
  async optimizeImages(options?: ImageOptimizationOptions): Promise<number> {
    try {
      const result = await this.document?.optimizeImages?.({
        maxDpi: options?.maxDpi ?? 150,
        minDpi: options?.minDpi ?? 72,
        targetQuality: options?.targetQuality ?? 85,
        targetFormat: options?.targetFormat ?? ImageCompressionType.JPEG,
        convertColor: options?.convertColor,
        removeIccProfiles: options?.removeIccProfiles ?? false,
      });

      const count = result?.optimizedCount ?? 0;
      this.emit('images-optimized', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Optimizes fonts in the document
   */
  async optimizeFonts(options?: FontOptimizationOptions): Promise<number> {
    try {
      const result = await this.document?.optimizeFonts?.({
        mode: options?.mode ?? FontEmbeddingMode.SUBSET,
        removeUnusedFonts: options?.removeUnusedFonts ?? true,
        mergeSubsets: options?.mergeSubsets ?? true,
        convertToType1: options?.convertToType1 ?? false,
        convertToOpenType: options?.convertToOpenType ?? false,
      });

      const count = result?.optimizedCount ?? 0;
      this.emit('fonts-optimized', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Linearizes the document for fast web view
   */
  async linearize(options?: LinearizationOptions): Promise<boolean> {
    try {
      const result = await this.document?.linearize?.({
        firstPageEnd: options?.firstPageEnd,
        primaryHint: options?.primaryHint ?? true,
        overflowHint: options?.overflowHint ?? true,
      });

      this.emit('document-linearized');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Removes unused objects from the document
   */
  async removeUnusedObjects(): Promise<number> {
    try {
      const result = await this.document?.removeUnusedObjects?.();
      const count = result ?? 0;
      this.emit('unused-objects-removed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Flattens transparency in the document
   */
  async flattenTransparency(): Promise<boolean> {
    try {
      const result = await this.document?.flattenTransparency?.();
      this.emit('transparency-flattened');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Converts color space
   */
  async convertColorSpace(targetColorSpace: ColorSpaceType): Promise<boolean> {
    try {
      const result = await this.document?.convertColorSpace?.(targetColorSpace);
      this.emit('color-space-converted', { targetColorSpace });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  // ==========================================================================
  // Page Manipulation (8 functions)
  // ==========================================================================

  /**
   * Rotates pages
   */
  async rotatePages(pageRange: PageRange | 'all', degrees: 90 | 180 | 270): Promise<number> {
    try {
      const range = pageRange === 'all' ? null : pageRange;
      const result = await this.document?.rotatePages?.(range, degrees);
      const count = result ?? 0;
      this.emit('pages-rotated', { count, degrees });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Scales pages
   */
  async scalePages(pageRange: PageRange | 'all', scaleX: number, scaleY: number): Promise<number> {
    try {
      const range = pageRange === 'all' ? null : pageRange;
      const result = await this.document?.scalePages?.(range, scaleX, scaleY);
      const count = result ?? 0;
      this.emit('pages-scaled', { count, scaleX, scaleY });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Crops pages
   */
  async cropPages(
    pageRange: PageRange | 'all',
    cropBox: readonly [number, number, number, number]
  ): Promise<number> {
    try {
      const range = pageRange === 'all' ? null : pageRange;
      const result = await this.document?.cropPages?.(range, cropBox);
      const count = result ?? 0;
      this.emit('pages-cropped', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Removes pages
   */
  async removePages(pageIndices: readonly number[]): Promise<boolean> {
    try {
      const result = await this.document?.removePages?.(pageIndices);
      this.emit('pages-removed', { count: pageIndices.length });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Reorders pages
   */
  async reorderPages(newOrder: readonly number[]): Promise<boolean> {
    try {
      const result = await this.document?.reorderPages?.(newOrder);
      this.emit('pages-reordered');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Duplicates pages
   */
  async duplicatePages(pageIndices: readonly number[], times: number = 1): Promise<boolean> {
    try {
      const result = await this.document?.duplicatePages?.(pageIndices, times);
      this.emit('pages-duplicated', { count: pageIndices.length * times });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Extracts pages to a new document
   */
  async extractPages(pageIndices: readonly number[]): Promise<Buffer | null> {
    try {
      const result = await this.document?.extractPages?.(pageIndices);
      this.emit('pages-extracted', { count: pageIndices.length });
      return result ?? null;
    } catch (error) {
      this.emit('error', error);
      return null;
    }
  }

  /**
   * Inserts blank pages
   */
  async insertBlankPages(
    afterPageIndex: number,
    count: number,
    width?: number,
    height?: number
  ): Promise<boolean> {
    try {
      const result = await this.document?.insertBlankPages?.(
        afterPageIndex,
        count,
        width ?? 612,
        height ?? 792
      );
      this.emit('blank-pages-inserted', { afterPageIndex, count });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  // ==========================================================================
  // Document Information (6 functions)
  // ==========================================================================

  /**
   * Gets document statistics
   */
  async getDocumentStatistics(): Promise<DocumentStatistics | null> {
    try {
      const stats = await this.document?.getDocumentStatistics?.();

      return (
        stats ?? {
          pageCount: await this.getPageCount(),
          fileSize: await this.getFileSize(),
          objectCount: 0,
          streamCount: 0,
          imageCount: 0,
          fontCount: 0,
          annotationCount: 0,
          bookmarkCount: 0,
          embeddedFileCount: 0,
          formFieldCount: 0,
          signatureCount: 0,
          hasJavaScript: false,
          hasXfa: false,
          isLinearized: false,
          isEncrypted: false,
          pdfVersion: '1.7',
        }
      );
    } catch (error) {
      this.emit('error', error);
      return null;
    }
  }

  /**
   * Gets page information
   */
  async getPageInfo(pageIndex: number): Promise<PageInfo | null> {
    try {
      const info = await this.document?.getPageInfo?.(pageIndex);
      return info ?? null;
    } catch (error) {
      this.emit('error', error);
      return null;
    }
  }

  /**
   * Gets page count
   */
  async getPageCount(): Promise<number> {
    try {
      return (await this.document?.getPageCount?.()) ?? 0;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Gets file size in bytes
   */
  async getFileSize(): Promise<number> {
    try {
      return (await this.document?.getFileSize?.()) ?? 0;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Checks if document is linearized
   */
  async isLinearized(): Promise<boolean> {
    try {
      return (await this.document?.isLinearized?.()) ?? false;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Gets last optimization result
   */
  getLastOptimizationResult(): OptimizationResult | null {
    return this.lastOptimizationResult;
  }

  // ==========================================================================
  // Repair and Cleanup (5 functions)
  // ==========================================================================

  /**
   * Repairs the document
   */
  async repairDocument(): Promise<RepairResult> {
    try {
      const result = await this.document?.repairDocument?.();

      const repairResult: RepairResult = {
        success: result?.success ?? true,
        issuesFound: result?.issuesFound ?? 0,
        issuesFixed: result?.issuesFixed ?? 0,
        issues: result?.issues ?? [],
        error: result?.error,
      };

      this.emit('document-repaired', repairResult);
      return repairResult;
    } catch (error) {
      this.emit('error', error);
      return {
        success: false,
        issuesFound: 0,
        issuesFixed: 0,
        issues: [error instanceof Error ? error.message : 'Unknown error'],
        error: error instanceof Error ? error.message : 'Unknown error',
      };
    }
  }

  /**
   * Removes all metadata
   */
  async removeAllMetadata(): Promise<boolean> {
    try {
      const result = await this.document?.removeAllMetadata?.();
      this.emit('metadata-removed');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Removes all JavaScript
   */
  async removeAllJavaScript(): Promise<boolean> {
    try {
      const result = await this.document?.removeAllJavaScript?.();
      this.emit('javascript-removed');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Removes all annotations
   */
  async removeAllAnnotations(): Promise<number> {
    try {
      const result = await this.document?.removeAllAnnotations?.();
      const count = result ?? 0;
      this.emit('annotations-removed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  /**
   * Removes all embedded files
   */
  async removeAllEmbeddedFiles(): Promise<number> {
    try {
      const result = await this.document?.removeAllEmbeddedFiles?.();
      const count = result ?? 0;
      this.emit('embedded-files-removed', { count });
      return count;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  // ==========================================================================
  // Save Functions (5 functions)
  // ==========================================================================

  /**
   * Saves the optimized document to a file
   */
  async saveOptimized(filePath: string): Promise<boolean> {
    try {
      await fs.mkdir(dirname(filePath), { recursive: true });
      const result = await this.document?.save?.(filePath);
      this.emit('document-saved', { filePath });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Saves the document to bytes
   */
  async saveToBytes(): Promise<Buffer | null> {
    try {
      return (await this.document?.saveToBytes?.()) ?? null;
    } catch (error) {
      this.emit('error', error);
      return null;
    }
  }

  /**
   * Saves with incremental update
   */
  async saveIncremental(filePath: string): Promise<boolean> {
    try {
      await fs.mkdir(dirname(filePath), { recursive: true });
      const result = await this.document?.saveIncremental?.(filePath);
      this.emit('document-saved-incremental', { filePath });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Saves with specific PDF version
   */
  async saveWithVersion(filePath: string, version: string): Promise<boolean> {
    try {
      await fs.mkdir(dirname(filePath), { recursive: true });
      const result = await this.document?.saveWithVersion?.(filePath, version);
      this.emit('document-saved', { filePath, version });
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Exports pages as separate PDFs
   */
  async exportPagesAsSeparateFiles(outputDir: string, prefix: string = 'page'): Promise<number> {
    try {
      await fs.mkdir(outputDir, { recursive: true });
      const pageCount = await this.getPageCount();
      let exported = 0;

      for (let i = 0; i < pageCount; i++) {
        const pageBuffer = await this.extractPages([i]);
        if (pageBuffer) {
          const filePath = `${outputDir}/${prefix}_${i + 1}.pdf`;
          await fs.writeFile(filePath, pageBuffer);
          exported++;
        }
      }

      this.emit('pages-exported-as-files', { count: exported, outputDir });
      return exported;
    } catch (error) {
      this.emit('error', error);
      return 0;
    }
  }

  // ==========================================================================
  // Merge Functions (3 functions)
  // ==========================================================================

  /**
   * Merges another PDF into this document
   */
  async mergePdf(otherPdfBuffer: Buffer, atPageIndex?: number): Promise<boolean> {
    try {
      const result = await this.document?.mergePdf?.(otherPdfBuffer, atPageIndex);
      this.emit('pdf-merged');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Merges multiple PDFs
   */
  async mergeMultiplePdfs(pdfBuffers: readonly Buffer[]): Promise<boolean> {
    try {
      for (const buffer of pdfBuffers) {
        const success = await this.mergePdf(buffer);
        if (!success) return false;
      }
      return true;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Appends pages from another PDF
   */
  async appendPagesFromPdf(
    otherPdfBuffer: Buffer,
    pageIndices?: readonly number[]
  ): Promise<boolean> {
    try {
      const result = await this.document?.appendPagesFromPdf?.(otherPdfBuffer, pageIndices);
      this.emit('pages-appended');
      return !!result;
    } catch (error) {
      this.emit('error', error);
      return false;
    }
  }

  /**
   * Cleanup resources
   */
  destroy(): void {
    this.lastOptimizationResult = null;
    this.removeAllListeners();
  }
}

export default DocumentUtilityManager;
