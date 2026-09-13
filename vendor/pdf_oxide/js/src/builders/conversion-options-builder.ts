/**
 * Builder for conversion options when converting PDF to other formats
 *
 * Configures how PDFs are converted to Markdown, HTML, or other text formats
 * with options for formatting, image handling, and content extraction.
 *
 * @example
 * ```typescript
 * import { ConversionOptionsBuilder } from 'pdf_oxide';
 *
 * const options = ConversionOptionsBuilder.create()
 *   .preserveFormatting(true)
 *   .includeImages(true)
 *   .detectHeadings(true)
 *   .detectTables(true)
 *   .build();
 *
 * const doc = PdfDocument.open('file.pdf');
 * const markdown = doc.toMarkdown(0, options);
 * ```
 */

interface PageRangeOptions {
  start: number;
  end: number;
}

export interface ConversionOptions {
  preserveFormatting: boolean;
  detectHeadings: boolean;
  detectTables: boolean;
  detectLists: boolean;
  includeImages: boolean;
  imageFormat: string;
  imageQuality: number;
  maxImageDimension: number;
  outputEncoding: string;
  normalizeWhitespace: boolean;
  extractAnnotations: boolean;
  useStructureTree: boolean;
  pageRange?: PageRangeOptions;
}

export class ConversionOptionsBuilder {
  private _preserveFormatting: boolean = true;
  private _detectHeadings: boolean = true;
  private _detectTables: boolean = true;
  private _detectLists: boolean = true;
  private _includeImages: boolean = true;
  private _imageFormat: string = 'png';
  private _imageQuality: number = 85;
  private _maxImageDimension: number = 2048;
  private _outputEncoding: string = 'utf-8';
  private _normalizeWhitespace: boolean = true;
  private _extractAnnotations: boolean = false;
  private _useStructureTree: boolean = true;
  private _pageRange?: PageRangeOptions;

  /**
   * Creates a new ConversionOptionsBuilder instance
   * @private
   */
  private constructor() {}

  /**
   * Creates a new ConversionOptionsBuilder instance
   * @returns New builder instance
   */
  static create(): ConversionOptionsBuilder {
    return new ConversionOptionsBuilder();
  }

  /**
   * Creates options with default settings optimized for readability
   * @returns Conversion options with default preset
   */
  static default(): ConversionOptions {
    return ConversionOptionsBuilder.create().build();
  }

  /**
   * Creates options optimized for text-only extraction
   * @returns Conversion options with text-only preset
   */
  static textOnly(): ConversionOptions {
    return ConversionOptionsBuilder.create()
      .preserveFormatting(false)
      .detectHeadings(true)
      .detectTables(false)
      .detectLists(false)
      .includeImages(false)
      .build();
  }

  /**
   * Creates options optimized for maximum quality and detail preservation
   * @returns Conversion options with high-quality preset
   */
  static highQuality(): ConversionOptions {
    return ConversionOptionsBuilder.create()
      .preserveFormatting(true)
      .detectHeadings(true)
      .detectTables(true)
      .detectLists(true)
      .includeImages(true)
      .imageQuality(95)
      .normalizeWhitespace(false)
      .build();
  }

  /**
   * Creates options for fast, basic conversion
   * @returns Conversion options with fast preset
   */
  static fast(): ConversionOptions {
    return ConversionOptionsBuilder.create()
      .preserveFormatting(false)
      .detectHeadings(false)
      .detectTables(false)
      .detectLists(false)
      .includeImages(false)
      .normalizeWhitespace(true)
      .build();
  }

  preserveFormatting(preserve: boolean): this {
    if (typeof preserve !== 'boolean') {
      throw new Error('preserveFormatting must be a boolean');
    }
    this._preserveFormatting = preserve;
    return this;
  }

  detectHeadings(detect: boolean): this {
    if (typeof detect !== 'boolean') {
      throw new Error('detectHeadings must be a boolean');
    }
    this._detectHeadings = detect;
    return this;
  }

  detectTables(detect: boolean): this {
    if (typeof detect !== 'boolean') {
      throw new Error('detectTables must be a boolean');
    }
    this._detectTables = detect;
    return this;
  }

  detectLists(detect: boolean): this {
    if (typeof detect !== 'boolean') {
      throw new Error('detectLists must be a boolean');
    }
    this._detectLists = detect;
    return this;
  }

  includeImages(include: boolean): this {
    if (typeof include !== 'boolean') {
      throw new Error('includeImages must be a boolean');
    }
    this._includeImages = include;
    return this;
  }

  imageFormat(format: string): this {
    const validFormats = ['png', 'jpg', 'jpeg', 'webp'];
    if (!validFormats.includes(format.toLowerCase())) {
      throw new Error(`Invalid image format. Must be one of: ${validFormats.join(', ')}`);
    }
    this._imageFormat = format.toLowerCase();
    return this;
  }

  imageQuality(quality: number): this {
    if (typeof quality !== 'number' || quality < 0 || quality > 100) {
      throw new Error('imageQuality must be a number between 0 and 100');
    }
    this._imageQuality = quality;
    return this;
  }

  maxImageDimension(maxDimension: number): this {
    if (typeof maxDimension !== 'number' || maxDimension <= 0) {
      throw new Error('maxImageDimension must be a positive number');
    }
    this._maxImageDimension = maxDimension;
    return this;
  }

  outputEncoding(encoding: string): this {
    if (typeof encoding !== 'string' || encoding.length === 0) {
      throw new Error('outputEncoding must be a non-empty string');
    }
    this._outputEncoding = encoding;
    return this;
  }

  normalizeWhitespace(normalize: boolean): this {
    if (typeof normalize !== 'boolean') {
      throw new Error('normalizeWhitespace must be a boolean');
    }
    this._normalizeWhitespace = normalize;
    return this;
  }

  extractAnnotations(extract: boolean): this {
    if (typeof extract !== 'boolean') {
      throw new Error('extractAnnotations must be a boolean');
    }
    this._extractAnnotations = extract;
    return this;
  }

  useStructureTree(use: boolean): this {
    if (typeof use !== 'boolean') {
      throw new Error('useStructureTree must be a boolean');
    }
    this._useStructureTree = use;
    return this;
  }

  pageRange(start: number, end: number): this {
    if (typeof start !== 'number' || typeof end !== 'number' || start < 0 || end < start) {
      throw new Error('pageRange must have valid start and end indices');
    }
    this._pageRange = { start, end };
    return this;
  }

  build(): ConversionOptions {
    return {
      preserveFormatting: this._preserveFormatting,
      detectHeadings: this._detectHeadings,
      detectTables: this._detectTables,
      detectLists: this._detectLists,
      includeImages: this._includeImages,
      imageFormat: this._imageFormat,
      imageQuality: this._imageQuality,
      maxImageDimension: this._maxImageDimension,
      outputEncoding: this._outputEncoding,
      normalizeWhitespace: this._normalizeWhitespace,
      extractAnnotations: this._extractAnnotations,
      useStructureTree: this._useStructureTree,
      pageRange: this._pageRange,
    };
  }
}

/**
 * Create a new ConversionOptionsBuilder with static factory
 * @deprecated Use ConversionOptionsBuilder.create() instead
 * @returns New builder instance
 */
export function createConversionOptionsBuilder(): ConversionOptionsBuilder {
  return ConversionOptionsBuilder.create();
}
