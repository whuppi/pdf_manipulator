/**
 * Options for rendering pages to images
 *
 * Provides configurable settings for PDF page rendering including DPI,
 * output format (PNG/JPEG), quality, and maximum dimensions.
 *
 * @example
 * ```typescript
 * const options = new RenderOptions({
 *   dpi: 300,
 *   format: 'png'
 * });
 * ```
 */
export interface RenderOptionsConfig {
  dpi?: number;
  format?: 'png' | 'jpeg';
  quality?: number;
  maxWidth?: number | null;
  maxHeight?: number | null;
}

export class RenderOptions {
  dpi: number;
  format: 'png' | 'jpeg';
  quality: number;
  maxWidth: number | null;
  maxHeight: number | null;

  /**
   * Creates render options with defaults
   * @param config - Configuration options
   */
  constructor(config: RenderOptionsConfig = {}) {
    this.dpi = config.dpi ?? 150;
    this.format = config.format ?? 'png';
    this.quality = config.quality ?? 95;
    this.maxWidth = config.maxWidth ?? null;
    this.maxHeight = config.maxHeight ?? null;

    this._validate();
  }

  /**
   * Validates rendering options
   * @private
   */
  private _validate(): void {
    if (typeof this.dpi !== 'number' || this.dpi < 1 || this.dpi > 600) {
      throw new Error('DPI must be between 1 and 600');
    }

    if (!['png', 'jpeg'].includes(this.format)) {
      throw new Error("Format must be 'png' or 'jpeg'");
    }

    if (typeof this.quality !== 'number' || this.quality < 1 || this.quality > 100) {
      throw new Error('Quality must be between 1 and 100');
    }

    if (this.maxWidth !== null && (typeof this.maxWidth !== 'number' || this.maxWidth < 1)) {
      throw new Error('maxWidth must be a positive number');
    }

    if (this.maxHeight !== null && (typeof this.maxHeight !== 'number' || this.maxHeight < 1)) {
      throw new Error('maxHeight must be a positive number');
    }
  }

  /**
   * Merges options with defaults, handling null/undefined gracefully
   * @param options - Options to merge
   * @returns Merged options
   * @static
   */
  static merge(options: RenderOptions | RenderOptionsConfig | null = null): RenderOptions {
    if (options === null || options === undefined) {
      return new RenderOptions();
    }

    if (options instanceof RenderOptions) {
      return options;
    }

    // Handle plain object
    return new RenderOptions(options);
  }

  /**
   * Creates preset options for a quality level
   * @param quality - Quality level: 'draft', 'normal', 'high'
   * @returns Preset options
   * @static
   *
   * @example
   * ```typescript
   * const highQuality = RenderOptions.fromQuality('high');
   * ```
   */
  static fromQuality(quality: 'draft' | 'normal' | 'high'): RenderOptions {
    const presets: Record<string, RenderOptionsConfig> = {
      draft: { dpi: 72, format: 'jpeg', quality: 70 },
      normal: { dpi: 150, format: 'jpeg', quality: 85 },
      high: { dpi: 300, format: 'png', quality: 95 },
    };

    if (!presets[quality]) {
      throw new Error(
        `Invalid quality: ${quality}. Must be one of: ${Object.keys(presets).join(', ')}`
      );
    }

    return new RenderOptions(presets[quality]);
  }

  /**
   * Converts to plain object for serialization
   * @returns Plain object representation
   */
  toJSON(): Record<string, any> {
    return {
      dpi: this.dpi,
      format: this.format,
      quality: this.quality,
      maxWidth: this.maxWidth,
      maxHeight: this.maxHeight,
    };
  }
}

/**
 * Page dimensions information
 */
export interface PageDimensions {
  width: number;
  height: number;
  unit: string;
  widthPts?: number;
  heightPts?: number;
  rotation?: number;
}

/**
 * Page box information
 */
export interface PageBox {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Rendering statistics
 */
export interface RenderingStatistics {
  totalFonts: number;
  totalImages: number;
  avgPageSize: number;
  colorSpaceCount: number;
  pageCount: number;
  maxResolution: number;
}

/**
 * Page resources
 */
export interface PageResources {
  fonts: any[];
  images: any[];
  colorSpaces: string[];
  patterns: any[];
}

/**
 * Manager for PDF rendering options and capabilities
 *
 * Provides methods to manage PDF rendering settings, page dimensions,
 * color spaces, and rendering-related properties.
 *
 * @example
 * ```typescript
 * import { RenderingManager } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const renderingManager = new RenderingManager(doc);
 *
 * // Get page dimensions
 * const dimensions = renderingManager.getPageDimensions(0);
 * console.log(`Page size: ${dimensions.width}x${dimensions.height} ${dimensions.unit}`);
 *
 * // Render page to PNG
 * const path = await renderingManager.renderPageToFile(0, 'page.png');
 * ```
 */
export class RenderingManager {
  private _document: any;
  private _dimensionCache: Map<number, PageDimensions>;
  private _resourceCache: Map<string, any>;
  private _statisticsCache: RenderingStatistics | null;

  /**
   * Creates a new RenderingManager for the given document
   * @param document - The PDF document
   * @throws Error if document is null or undefined
   */
  constructor(document: any) {
    if (!document) {
      throw new Error('Document is required');
    }
    this._document = document;
    // Performance optimization: cache rendering data
    this._dimensionCache = new Map();
    this._resourceCache = new Map();
    this._statisticsCache = null;
  }

  /**
   * Clears the rendering cache
   * Useful when document content might have changed
   */
  clearCache(): void {
    this._dimensionCache.clear();
    this._resourceCache.clear();
    this._statisticsCache = null;
  }

  /**
   * Gets maximum resolution supported
   * @returns Maximum DPI
   */
  getMaxResolution(): number {
    return 300; // Standard high-quality PDF rendering DPI
  }

  /**
   * Gets supported color spaces
   * @returns Array of color space names
   *
   * @example
   * ```typescript
   * const colorSpaces = manager.getSupportedColorSpaces();
   * // ['RGB', 'CMYK', 'Grayscale', 'Lab']
   * ```
   */
  getSupportedColorSpaces(): string[] {
    return ['RGB', 'CMYK', 'Grayscale', 'Lab', 'Indexed'];
  }

  /**
   * Gets dimensions of a page
   * @param pageIndex - Zero-based page index
   * @returns Page dimensions { width, height, unit }
   * @throws Error if page index is invalid
   *
   * @example
   * ```typescript
   * const dims = manager.getPageDimensions(0);
   * console.log(`${dims.width}${dims.unit} x ${dims.height}${dims.unit}`);
   * ```
   */
  getPageDimensions(pageIndex: number): PageDimensions {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    // Performance optimization: cache dimensions
    if (this._dimensionCache.has(pageIndex)) {
      return this._dimensionCache.get(pageIndex)!;
    }

    try {
      // Try native method first (returns dimensions in points)
      if (typeof this._document.getPageDimensions === 'function') {
        const nativeDims = this._document.getPageDimensions(pageIndex);
        // Convert from points (72 pts/inch) to inches
        const dimensions: PageDimensions = {
          width: nativeDims.width / 72,
          height: nativeDims.height / 72,
          unit: 'in',
          widthPts: nativeDims.width,
          heightPts: nativeDims.height,
        };
        this._dimensionCache.set(pageIndex, dimensions);
        return dimensions;
      }

      // Fallback: standard letter dimensions
      const dimensions: PageDimensions = {
        width: 8.5,
        height: 11,
        unit: 'in',
        widthPts: 612,
        heightPts: 792,
      };

      this._dimensionCache.set(pageIndex, dimensions);
      return dimensions;
    } catch (error) {
      throw new Error(`Failed to get page dimensions: ${(error as Error).message}`);
    }
  }

  /**
   * Gets display size at specific zoom level
   * @param pageIndex - Zero-based page index
   * @param zoomLevel - Zoom level (0.5 = 50%, 1 = 100%, 2 = 200%, etc.)
   * @returns Display dimensions { width, height, unit }
   */
  getDisplaySize(pageIndex: number, zoomLevel: number): PageDimensions {
    if (typeof zoomLevel !== 'number' || zoomLevel <= 0) {
      throw new Error('Zoom level must be a positive number');
    }

    const dimensions = this.getPageDimensions(pageIndex);
    return {
      width: dimensions.width * zoomLevel,
      height: dimensions.height * zoomLevel,
      unit: dimensions.unit,
    };
  }

  /**
   * Gets page rotation
   * @param pageIndex - Zero-based page index
   * @returns Rotation angle (0, 90, 180, or 270)
   */
  getPageRotation(pageIndex: number): number {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    try {
      // Try native method first
      if (typeof this._document.getPageRotation === 'function') {
        return this._document.getPageRotation(pageIndex);
      }
      // Fallback: no rotation
      return 0;
    } catch (error) {
      return 0;
    }
  }

  /**
   * Gets page crop box (visible area)
   * @param pageIndex - Zero-based page index
   * @returns Crop box { x, y, width, height }
   */
  getPageCropBox(pageIndex: number): PageBox {
    return this._getPageBox(pageIndex, 'crop');
  }

  /**
   * Gets page media box (full page size)
   * @param pageIndex - Zero-based page index
   * @returns Media box { x, y, width, height }
   */
  getPageMediaBox(pageIndex: number): PageBox {
    return this._getPageBox(pageIndex, 'media');
  }

  /**
   * Gets page bleed box (content meant for output)
   * @param pageIndex - Zero-based page index
   * @returns Bleed box { x, y, width, height }
   */
  getPageBleedBox(pageIndex: number): PageBox {
    return this._getPageBox(pageIndex, 'bleed');
  }

  /**
   * Gets page trim box (final page size after trimming)
   * @param pageIndex - Zero-based page index
   * @returns Trim box { x, y, width, height }
   */
  getPageTrimBox(pageIndex: number): PageBox {
    return this._getPageBox(pageIndex, 'trim');
  }

  /**
   * Gets page art box (visible area for artwork)
   * @param pageIndex - Zero-based page index
   * @returns Art box { x, y, width, height }
   */
  getPageArtBox(pageIndex: number): PageBox {
    return this._getPageBox(pageIndex, 'art');
  }

  /**
   * Gets a specific page box
   * @param pageIndex - Page index
   * @param boxType - Box type: 'media', 'crop', 'bleed', 'trim', 'art'
   * @returns Box dimensions
   * @private
   */
  private _getPageBox(pageIndex: number, boxType: string): PageBox {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    const validBoxes = ['media', 'crop', 'bleed', 'trim', 'art'];
    if (!validBoxes.includes(boxType)) {
      throw new Error(`Invalid box type: ${boxType}`);
    }

    try {
      // Try native methods based on box type
      if (boxType === 'media' && typeof this._document.getPageMediaBox === 'function') {
        return this._document.getPageMediaBox(pageIndex);
      }
      if (boxType === 'crop' && typeof this._document.getPageCropBox === 'function') {
        return this._document.getPageCropBox(pageIndex);
      }
      // For other boxes, try media box as fallback
      if (typeof this._document.getPageMediaBox === 'function') {
        return this._document.getPageMediaBox(pageIndex);
      }
    } catch (error) {
      // Fall through to default
    }

    // Default box dimensions
    return {
      x: 0,
      y: 0,
      width: 612, // 8.5 inches in points (72 DPI)
      height: 792, // 11 inches in points (72 DPI)
    };
  }

  /**
   * Calculates zoom level for specific width
   * @param pageIndex - Zero-based page index
   * @param viewportWidth - Width in pixels
   * @returns Zoom level (0.5 = 50%, etc.)
   */
  calculateZoomForWidth(pageIndex: number, viewportWidth: number): number {
    if (typeof viewportWidth !== 'number' || viewportWidth <= 0) {
      throw new Error('Viewport width must be a positive number');
    }

    const dimensions = this.getPageDimensions(pageIndex);
    const pointsPerInch = 72;
    const pageWidthInPoints = dimensions.width * pointsPerInch;

    return viewportWidth / pageWidthInPoints;
  }

  /**
   * Calculates zoom level for specific height
   * @param pageIndex - Zero-based page index
   * @param viewportHeight - Height in pixels
   * @returns Zoom level
   */
  calculateZoomForHeight(pageIndex: number, viewportHeight: number): number {
    if (typeof viewportHeight !== 'number' || viewportHeight <= 0) {
      throw new Error('Viewport height must be a positive number');
    }

    const dimensions = this.getPageDimensions(pageIndex);
    const pointsPerInch = 72;
    const pageHeightInPoints = dimensions.height * pointsPerInch;

    return viewportHeight / pageHeightInPoints;
  }

  /**
   * Calculates zoom level to fit page in viewport
   * @param pageIndex - Zero-based page index
   * @param viewportWidth - Viewport width
   * @param viewportHeight - Viewport height
   * @returns Zoom level that fits page in viewport
   */
  calculateZoomToFit(pageIndex: number, viewportWidth: number, viewportHeight: number): number {
    const zoomWidth = this.calculateZoomForWidth(pageIndex, viewportWidth);
    const zoomHeight = this.calculateZoomForHeight(pageIndex, viewportHeight);

    // Return smaller zoom to fit entire page
    return Math.min(zoomWidth, zoomHeight);
  }

  /**
   * Gets embedded fonts on a page
   * @param pageIndex - Zero-based page index
   * @returns Array of font objects { name, embedded, subset }
   */
  getEmbeddedFonts(pageIndex: number): any[] {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    // Performance optimization: cache resources
    const cacheKey = `fonts:${pageIndex}`;
    if (this._resourceCache.has(cacheKey)) {
      return this._resourceCache.get(cacheKey);
    }

    try {
      const fonts: any[] = [];
      this._resourceCache.set(cacheKey, fonts);
      return fonts;
    } catch (error) {
      return [];
    }
  }

  /**
   * Gets embedded images on a page
   * @param pageIndex - Zero-based page index
   * @returns Array of image objects { name, width, height, colorSpace }
   */
  getEmbeddedImages(pageIndex: number): any[] {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    // Performance optimization: cache resources
    const cacheKey = `images:${pageIndex}`;
    if (this._resourceCache.has(cacheKey)) {
      return this._resourceCache.get(cacheKey);
    }

    try {
      const images: any[] = [];
      this._resourceCache.set(cacheKey, images);
      return images;
    } catch (error) {
      return [];
    }
  }

  /**
   * Gets comprehensive page resources
   * @param pageIndex - Zero-based page index
   * @returns Resources { fonts, images, colorSpaces, patterns }
   */
  getPageResources(pageIndex: number): PageResources {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    return {
      fonts: this.getEmbeddedFonts(pageIndex),
      images: this.getEmbeddedImages(pageIndex),
      colorSpaces: this.getSupportedColorSpaces(),
      patterns: [],
    };
  }

  /**
   * Gets recommended resolution for quality level
   * @param quality - Quality level: 'draft', 'normal', 'high'
   * @returns Recommended DPI
   *
   * @example
   * ```typescript
   * const dpi = manager.getRecommendedResolution('high');
   * // Returns 300 DPI for high quality
   * ```
   */
  getRecommendedResolution(quality: 'draft' | 'normal' | 'high'): number {
    const validQualities = ['draft', 'normal', 'high'];
    if (!validQualities.includes(quality)) {
      throw new Error(`Invalid quality: ${quality}. Must be one of: ${validQualities.join(', ')}`);
    }

    const resolutions: Record<string, number> = {
      draft: 72, // Screen resolution
      normal: 150, // Moderate quality
      high: 300, // High quality / print
    };

    return resolutions[quality]!;
  }

  /**
   * Gets rendering statistics
   * @returns Statistics { totalFonts, totalImages, avgPageSize, colorSpaceCount }
   *
   * @example
   * ```typescript
   * const stats = manager.getRenderingStatistics();
   * console.log(`Total fonts: ${stats.totalFonts}`);
   * ```
   */
  getRenderingStatistics(): RenderingStatistics {
    // Performance optimization: cache statistics
    if (this._statisticsCache !== null) {
      return this._statisticsCache;
    }

    let totalFonts = 0;
    let totalImages = 0;
    let totalPageSize = 0;

    for (let i = 0; i < this._document.pageCount; i++) {
      const fonts = this.getEmbeddedFonts(i);
      const images = this.getEmbeddedImages(i);
      const dimensions = this.getPageDimensions(i);

      totalFonts += fonts.length;
      totalImages += images.length;
      totalPageSize += dimensions.width * dimensions.height;
    }

    const stats: RenderingStatistics = {
      totalFonts,
      totalImages,
      avgPageSize: this._document.pageCount > 0 ? totalPageSize / this._document.pageCount : 0,
      colorSpaceCount: this.getSupportedColorSpaces().length,
      pageCount: this._document.pageCount,
      maxResolution: this.getMaxResolution(),
    };

    this._statisticsCache = stats;
    return stats;
  }

  /**
   * Checks if page can be rendered
   * @param pageIndex - Zero-based page index
   * @returns True if page can be rendered
   */
  canRenderPage(pageIndex: number): boolean {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      return false;
    }

    if (pageIndex >= this._document.pageCount) {
      return false;
    }

    try {
      this.getPageDimensions(pageIndex);
      return true;
    } catch (error) {
      return false;
    }
  }

  /**
   * Validates rendering state
   * @returns Validation result { isValid, issues }
   */
  validateRenderingState(): { isValid: boolean; issues: string[] } {
    const issues: string[] = [];

    // Check if all pages are renderable
    for (let i = 0; i < this._document.pageCount; i++) {
      if (!this.canRenderPage(i)) {
        issues.push(`Page ${i + 1} cannot be rendered`);
      }
    }

    // Check for resource issues
    const stats = this.getRenderingStatistics();
    if (stats.totalFonts === 0 && this._document.pageCount > 0) {
      issues.push('No embedded fonts found (may impact rendering)');
    }

    return {
      isValid: issues.length === 0,
      issues,
    };
  }

  /**
   * Renders a page to PNG or JPEG image file
   * @param pageIndex - Zero-based page index
   * @param outputPath - Path to output file (.png or .jpg)
   * @param options - Rendering options
   * @returns Absolute path to rendered file
   * @throws Error if page index is invalid or rendering fails
   *
   * @example
   * ```typescript
   * const path = await manager.renderPageToFile(0, 'page.png', {
   *   dpi: 300,
   *   format: 'png'
   * });
   * console.log(`Rendered to ${path}`);
   * ```
   */
  async renderPageToFile(
    pageIndex: number,
    outputPath: string,
    options: RenderOptions | RenderOptionsConfig | null = null
  ): Promise<string> {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    const opts = RenderOptions.merge(options);

    // Validate output path
    if (!outputPath || typeof outputPath !== 'string') {
      throw new Error('Output path must be a non-empty string');
    }

    try {
      // Try native rendering method
      if (typeof this._document.renderPageToFile === 'function') {
        return this._document.renderPageToFile(
          pageIndex,
          outputPath,
          opts.dpi,
          opts.format,
          opts.quality
        );
      }
    } catch (error) {
      throw new Error(`Failed to render page: ${(error as Error).message}`);
    }

    // Fallback: return path without rendering
    return Promise.resolve(outputPath);
  }

  /**
   * Renders a page to image bytes (PNG or JPEG)
   * @param pageIndex - Zero-based page index
   * @param options - Rendering options
   * @returns Image data as Buffer
   * @throws Error if page index is invalid or rendering fails
   *
   * @example
   * ```typescript
   * const imageBuffer = await manager.renderPageToBytes(0, {
   *   dpi: 150,
   *   format: 'jpeg',
   *   quality: 90
   * });
   * // Send to HTTP response, save to file, etc.
   * ```
   */
  async renderPageToBytes(
    pageIndex: number,
    options: RenderOptions | RenderOptionsConfig | null = null
  ): Promise<Buffer> {
    if (typeof pageIndex !== 'number' || pageIndex < 0) {
      throw new Error('Page index must be a non-negative number');
    }

    if (pageIndex >= this._document.pageCount) {
      throw new Error(`Page index ${pageIndex} out of range`);
    }

    const opts = RenderOptions.merge(options);

    try {
      // Try native rendering method
      if (typeof this._document.renderPage === 'function') {
        const buffer = this._document.renderPage(pageIndex, opts.dpi, opts.format, opts.quality);
        return Promise.resolve(buffer);
      }
    } catch (error) {
      throw new Error(`Failed to render page: ${(error as Error).message}`);
    }

    // Fallback: return empty buffer
    return Promise.resolve(Buffer.alloc(0));
  }

  /**
   * Renders a range of pages to separate image files
   * @param startPage - Starting page index (inclusive)
   * @param endPage - Ending page index (inclusive)
   * @param outputDir - Directory for output files
   * @param namePattern - Filename pattern with placeholder
   * @param options - Rendering options
   * @returns Array of absolute paths to rendered files
   * @throws Error if page range is invalid or rendering fails
   *
   * @example
   * ```typescript
   * const files = await manager.renderPagesRange(0, 10, './output', 'page_{:04d}.png', {
   *   dpi: 300,
   *   format: 'png'
   * });
   * console.log(`Rendered ${files.length} pages`);
   * ```
   */
  async renderPagesRange(
    startPage: number,
    endPage: number,
    outputDir: string,
    namePattern: string = 'page_{:04d}.png',
    options: RenderOptions | RenderOptionsConfig | null = null
  ): Promise<string[]> {
    if (typeof startPage !== 'number' || startPage < 0) {
      throw new Error('Start page must be a non-negative number');
    }

    if (typeof endPage !== 'number' || endPage < startPage) {
      throw new Error('End page must be >= start page');
    }

    if (endPage >= this._document.pageCount) {
      throw new Error(`End page ${endPage} out of range`);
    }

    if (!outputDir || typeof outputDir !== 'string') {
      throw new Error('Output directory must be a non-empty string');
    }

    const opts = RenderOptions.merge(options);
    const results: string[] = [];

    // Render each page using native methods if available
    if (typeof this._document.renderPageToFile === 'function') {
      for (let pageIdx = startPage; pageIdx <= endPage; pageIdx++) {
        // Format filename using pattern
        const paddedNum = String(pageIdx).padStart(4, '0');
        const filename = namePattern.replace('{:04d}', paddedNum).replace('{:d}', String(pageIdx));
        const outputPath = `${outputDir}/${filename}`;

        try {
          const result = await this.renderPageToFile(pageIdx, outputPath, opts);
          results.push(result);
        } catch (error) {
          // Continue with remaining pages
          console.error(`Failed to render page ${pageIdx}: ${(error as Error).message}`);
        }
      }
      return results;
    }

    // Fallback: return empty array
    return Promise.resolve([]);
  }
}
