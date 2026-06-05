/**
 * OptimizationManager - PDF Optimization Operations
 *
 * Provides document optimization capabilities including:
 * - Font subsetting
 * - Image downsampling
 * - Object deduplication
 * - Full optimization pipeline
 *
 * @since 1.0.0
 */

import { EventEmitter } from 'events';
import { mapFfiErrorCode, OptimizationException } from '../errors';

// =============================================================================
// Type Definitions
// =============================================================================

/**
 * Result of an optimization operation.
 */
export interface OptimizationResult {
  /** Whether the optimization succeeded */
  readonly success: boolean;
  /** Number of bytes saved */
  readonly bytesSaved: number;
  /** Original document size in bytes */
  readonly originalSize: number;
  /** Optimized document size in bytes */
  readonly optimizedSize: number;
  /** Compression ratio (0.0 - 1.0) */
  readonly compressionRatio: number;
}

// =============================================================================
// OptimizationManager
// =============================================================================

/**
 * Manager for PDF optimization operations.
 *
 * Provides methods for reducing PDF file size through font subsetting,
 * image downsampling, object deduplication, and combined optimization.
 *
 * @example
 * ```typescript
 * const optimizer = new OptimizationManager(document);
 *
 * // Subset fonts to remove unused glyphs
 * const fontResult = await optimizer.subsetFonts();
 * console.log(`Font subsetting saved ${fontResult.bytesSaved} bytes`);
 *
 * // Downsample high-resolution images
 * const imageResult = await optimizer.downsampleImages(150, 80);
 *
 * // Full optimization pipeline
 * const fullResult = await optimizer.optimizeFull(150, 80);
 * console.log(`Total savings: ${fullResult.bytesSaved} bytes`);
 * ```
 */
export class OptimizationManager extends EventEmitter {
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
  // Optimization Operations
  // ===========================================================================

  /**
   * Subsets all embedded fonts in the document.
   *
   * Removes unused glyphs from embedded fonts, reducing file size
   * while preserving visual fidelity for the characters actually used.
   *
   * @returns Optimization result with bytes saved
   * @throws OptimizationException if the operation fails
   */
  async subsetFonts(): Promise<OptimizationResult> {
    if (!this.native?.pdf_optimize_subset_fonts) {
      throw new OptimizationException(
        'Native optimization not available: pdf_optimize_subset_fonts not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_optimize_subset_fonts(
      this.document._handle ?? this.document,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to subset fonts');
    }

    const result = this.parseOptimizationResult(resultPtr);
    this.emit('fonts-subsetted', { bytesSaved: result.bytesSaved });

    this.freeOptimizationResult(resultPtr);
    return result;
  }

  /**
   * Downsamples images in the document to reduce file size.
   *
   * @param dpi - Target resolution in dots per inch (default: 150)
   * @param quality - JPEG quality for recompression (1-100, default: 80)
   * @returns Optimization result with bytes saved
   * @throws OptimizationException if the operation fails
   */
  async downsampleImages(dpi?: number, quality?: number): Promise<OptimizationResult> {
    if (!this.native?.pdf_optimize_downsample_images) {
      throw new OptimizationException(
        'Native optimization not available: pdf_optimize_downsample_images not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_optimize_downsample_images(
      this.document._handle ?? this.document,
      dpi ?? 150,
      quality ?? 80,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to downsample images');
    }

    const result = this.parseOptimizationResult(resultPtr);
    this.emit('images-downsampled', {
      dpi: dpi ?? 150,
      quality: quality ?? 80,
      bytesSaved: result.bytesSaved,
    });

    this.freeOptimizationResult(resultPtr);
    return result;
  }

  /**
   * Deduplicates identical objects in the document.
   *
   * Identifies and merges duplicate fonts, images, and other resources
   * that appear multiple times in the document.
   *
   * @returns Optimization result with bytes saved
   * @throws OptimizationException if the operation fails
   */
  async deduplicate(): Promise<OptimizationResult> {
    if (!this.native?.pdf_optimize_deduplicate) {
      throw new OptimizationException(
        'Native optimization not available: pdf_optimize_deduplicate not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_optimize_deduplicate(
      this.document._handle ?? this.document,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to deduplicate objects');
    }

    const result = this.parseOptimizationResult(resultPtr);
    this.emit('deduplicated', { bytesSaved: result.bytesSaved });

    this.freeOptimizationResult(resultPtr);
    return result;
  }

  /**
   * Runs the full optimization pipeline.
   *
   * Combines font subsetting, image downsampling, and object deduplication
   * into a single operation for maximum file size reduction.
   *
   * @param dpi - Target image resolution in dots per inch (default: 150)
   * @param quality - JPEG quality for recompression (1-100, default: 80)
   * @returns Optimization result with total bytes saved
   * @throws OptimizationException if the operation fails
   */
  async optimizeFull(dpi?: number, quality?: number): Promise<OptimizationResult> {
    if (!this.native?.pdf_optimize_full) {
      throw new OptimizationException(
        'Native optimization not available: pdf_optimize_full not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_optimize_full(
      this.document._handle ?? this.document,
      dpi ?? 150,
      quality ?? 80,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to run full optimization');
    }

    const result = this.parseOptimizationResult(resultPtr);
    this.emit('optimized-full', {
      dpi: dpi ?? 150,
      quality: quality ?? 80,
      bytesSaved: result.bytesSaved,
    });

    this.freeOptimizationResult(resultPtr);
    return result;
  }

  // ===========================================================================
  // Private Helpers
  // ===========================================================================

  private parseOptimizationResult(resultPtr: any): OptimizationResult {
    if (!resultPtr) {
      return {
        success: true,
        bytesSaved: 0,
        originalSize: 0,
        optimizedSize: 0,
        compressionRatio: 0,
      };
    }

    if (typeof resultPtr === 'string') {
      try {
        return JSON.parse(resultPtr);
      } catch {
        return {
          success: true,
          bytesSaved: 0,
          originalSize: 0,
          optimizedSize: 0,
          compressionRatio: 0,
        };
      }
    }

    // Handle native result handle
    const bytesSaved = this.native?.pdf_optimization_result_bytes_saved?.(resultPtr) ?? 0;
    return {
      success: true,
      bytesSaved,
      originalSize: 0,
      optimizedSize: 0,
      compressionRatio: 0,
    };
  }

  private freeOptimizationResult(resultPtr: any): void {
    if (resultPtr && typeof resultPtr !== 'string' && this.native?.pdf_optimization_result_free) {
      this.native.pdf_optimization_result_free(resultPtr);
    }
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

export default OptimizationManager;
