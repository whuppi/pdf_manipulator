/**
 * ResultAccessorsManager for extracting extended properties from PDF operations
 *
 * Provides detailed metadata from search results, fonts, images, and annotations.
 * Enables advanced features like context extraction, font metrics analysis, and annotation tracking.
 * API is consistent with Python, Java, C#, Go, and Swift implementations.
 */

import { EventEmitter } from 'events';

/**
 * Extended properties from a search result
 */
export interface SearchResultProperties {
  context: string;
  lineNumber: number;
  paragraphNumber: number;
  confidence: number;
  isHighlighted: boolean;
  fontInfo: string; // JSON format
  color: [number, number, number]; // RGB
  rotation: number;
  objectId: number;
  streamIndex: number;
}

/**
 * Extended font metric information
 */
export interface FontProperties {
  baseFontName: string;
  descriptor: string; // JSON format
  descendantFont: string;
  toUnicodeCmap: string;
  isVertical: boolean;
  widths: Float32Array;
  ascender: number;
  descender: number;
}

/**
 * Extended image metadata
 */
export interface ImageProperties {
  hasAlphaChannel: boolean;
  iccProfile: Uint8Array;
  filterChain: string; // JSON format
  decodedData: Uint8Array;
  width: number;
  height: number;
  colorSpace: string;
}

/**
 * Extended annotation properties
 */
export interface AnnotationProperties {
  modifiedDate: number; // timestamp in milliseconds
  subject: string;
  replyToIndex: number;
  pageNumber: number;
  iconName: string;
  author: string;
}

/**
 * Result Accessors Manager for extracting extended properties
 *
 * Provides methods to:
 * - Extract context from search results
 * - Get detailed font metrics
 * - Inspect image metadata and ICC profiles
 * - Track annotation relationships and metadata
 * - Filter and analyze results by properties
 *
 * Matches: Python ResultAccessorsManager, Java ResultAccessorsManager, etc.
 */
export class ResultAccessorsManager extends EventEmitter {
  private document: any;
  private resultCache = new Map<string, any>();
  private maxCacheSize = 100;
  private native: any;

  constructor(document: any) {
    super();
    this.document = document;
    try {
      this.native = require('../index.node');
    } catch {
      // Fall back to framework defaults if native module not available
      this.native = null;
    }
  }

  private setCached(key: string, value: any): void {
    if (this.resultCache.size >= this.maxCacheSize) {
      // Remove oldest entry
      const firstKey = this.resultCache.keys().next().value;
      if (firstKey) this.resultCache.delete(firstKey);
    }
    this.resultCache.set(key, value);
  }

  // ========== Search Result Accessors (10 functions) ==========

  /**
   * Gets context text around a search result
   * Includes words before and after the match
   * @param results Search results handle
   * @param index Index of the result
   * @param contextWidth Number of characters for context
   * @returns Context text with highlighted match
   */
  async getSearchResultContext(
    results: any,
    index: number,
    contextWidth: number = 50
  ): Promise<string> {
    const cacheKey = `search:context:${index}:${contextWidth}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const context = this.native?.search_result_context?.(index, contextWidth) ?? '';
    this.setCached(cacheKey, context);
    this.emit('searchContextExtracted', index);
    return context;
  }

  /**
   * Gets the line number of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns Line number (0-based)
   */
  async getSearchResultLineNumber(results: any, index: number): Promise<number> {
    const cacheKey = `search:linenum:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const lineNumber = this.native?.search_result_line_number?.(index) ?? 0;
    this.setCached(cacheKey, lineNumber);
    return lineNumber;
  }

  /**
   * Gets the paragraph number of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns Paragraph number (0-based)
   */
  async getSearchResultParagraphNumber(results: any, index: number): Promise<number> {
    const cacheKey = `search:paragraphnum:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const paragraphNumber = this.native?.search_result_paragraph_number?.(index) ?? 0;
    this.setCached(cacheKey, paragraphNumber);
    return paragraphNumber;
  }

  /**
   * Gets the confidence score of a search result
   * Useful for OCR results where confidence varies
   * @param results Search results handle
   * @param index Index of the result
   * @returns Confidence score (0.0 to 1.0)
   */
  async getSearchResultConfidence(results: any, index: number): Promise<number> {
    const cacheKey = `search:confidence:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const confidence = this.native?.search_result_confidence?.(index) ?? 1.0;
    this.setCached(cacheKey, confidence);
    return confidence;
  }

  /**
   * Checks if a search result is highlighted in the document
   * @param results Search results handle
   * @param index Index of the result
   * @returns True if the result is highlighted
   */
  async isSearchResultHighlighted(results: any, index: number): Promise<boolean> {
    const cacheKey = `search:highlighted:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const highlighted = this.native?.search_result_is_highlighted?.(index) ?? false;
    this.setCached(cacheKey, highlighted);
    return highlighted;
  }

  /**
   * Gets font information for a search result
   * Returns JSON with font name, size, family, etc.
   * @param results Search results handle
   * @param index Index of the result
   * @returns Font info as JSON string
   */
  async getSearchResultFontInfo(results: any, index: number): Promise<string> {
    const cacheKey = `search:fontinfo:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const fontInfo = this.native?.search_result_font_info?.(index) ?? '{}';
    this.setCached(cacheKey, fontInfo);
    return fontInfo;
  }

  /**
   * Gets RGB color of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns Color as [R, G, B] array (0-255)
   */
  async getSearchResultColor(results: any, index: number): Promise<[number, number, number]> {
    const cacheKey = `search:color:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    let color: [number, number, number] = [0, 0, 0];
    if (this.native?.search_result_color) {
      try {
        const colorJson = this.native.search_result_color(index);
        const parsed = JSON.parse(colorJson);
        color = [parsed.r ?? 0, parsed.g ?? 0, parsed.b ?? 0];
      } catch {
        color = [0, 0, 0];
      }
    }
    this.setCached(cacheKey, color);
    return color;
  }

  /**
   * Gets the rotation angle of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns Rotation in degrees (0, 90, 180, 270)
   */
  async getSearchResultRotation(results: any, index: number): Promise<number> {
    const cacheKey = `search:rotation:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const rotation = this.native?.search_result_rotation?.(index) ?? 0;
    this.setCached(cacheKey, rotation);
    return rotation;
  }

  /**
   * Gets the object ID of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns PDF object ID
   */
  async getSearchResultObjectId(results: any, index: number): Promise<number> {
    const cacheKey = `search:objectid:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const objectId = this.native?.search_result_object_id?.(index) ?? 0;
    this.setCached(cacheKey, objectId);
    return objectId;
  }

  /**
   * Gets the stream index of a search result
   * @param results Search results handle
   * @param index Index of the result
   * @returns Stream index in the content
   */
  async getSearchResultStreamIndex(results: any, index: number): Promise<number> {
    const cacheKey = `search:streamindex:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const streamIndex = this.native?.search_result_stream_index?.(index) ?? 0;
    this.setCached(cacheKey, streamIndex);
    return streamIndex;
  }

  /**
   * Gets all properties of a search result at once
   * More efficient than individual property calls
   * @param results Search results handle
   * @param index Index of the result
   * @returns Object with all properties
   */
  async getSearchResultAllProperties(results: any, index: number): Promise<SearchResultProperties> {
    const cacheKey = `search:all:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Aggregate all properties by calling individual native functions
    const context = this.native?.search_result_context?.(index, 50) ?? '';
    const lineNumber = this.native?.search_result_line_number?.(index) ?? 0;
    const paragraphNumber = this.native?.search_result_paragraph_number?.(index) ?? 0;
    const confidence = this.native?.search_result_confidence?.(index) ?? 1.0;
    const isHighlighted = this.native?.search_result_is_highlighted?.(index) ?? false;
    const fontInfo = this.native?.search_result_font_info?.(index) ?? '{}';

    let color: [number, number, number] = [0, 0, 0];
    if (this.native?.search_result_color) {
      try {
        const colorJson = this.native.search_result_color(index);
        const parsed = JSON.parse(colorJson);
        color = [parsed.r ?? 0, parsed.g ?? 0, parsed.b ?? 0];
      } catch {
        color = [0, 0, 0];
      }
    }

    const rotation = this.native?.search_result_rotation?.(index) ?? 0;
    const objectId = this.native?.search_result_object_id?.(index) ?? 0;
    const streamIndex = this.native?.search_result_stream_index?.(index) ?? 0;

    const props: SearchResultProperties = {
      context,
      lineNumber,
      paragraphNumber,
      confidence,
      isHighlighted,
      fontInfo,
      color,
      rotation,
      objectId,
      streamIndex,
    };
    this.setCached(cacheKey, props);
    this.emit('searchPropertiesExtracted', index);
    return props;
  }

  // ========== Font Accessors (8 functions) ==========

  /**
   * Gets the base font name
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Font name (e.g., "Helvetica", "Arial")
   */
  async getFontBaseFontName(fonts: any, index: number): Promise<string> {
    const cacheKey = `font:basename:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const name = this.native?.font_get_base_font_name?.(index) ?? '';
    this.setCached(cacheKey, name);
    return name;
  }

  /**
   * Gets the font descriptor JSON
   * Contains details about font metrics and characteristics
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Font descriptor as JSON string
   */
  async getFontDescriptor(fonts: any, index: number): Promise<string> {
    const cacheKey = `font:descriptor:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const descriptor = this.native?.font_get_descriptor?.(index) ?? '{}';
    this.setCached(cacheKey, descriptor);
    return descriptor;
  }

  /**
   * Gets the descendant font name (for composite fonts)
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Descendant font name or empty string
   */
  async getFontDescendantFont(fonts: any, index: number): Promise<string> {
    const cacheKey = `font:descendant:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const descendant = this.native?.font_get_descendant_font?.(index) ?? '';
    this.setCached(cacheKey, descendant);
    return descendant;
  }

  /**
   * Gets the ToUnicode CMap for character to Unicode mapping
   * @param fonts Font handle
   * @param index Index of the font
   * @returns ToUnicode CMap as string
   */
  async getFontToUnicodeCmap(fonts: any, index: number): Promise<string> {
    const cacheKey = `font:tounicode:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const cmap = this.native?.font_get_to_unicode_cmap?.(index) ?? '';
    this.setCached(cacheKey, cmap);
    return cmap;
  }

  /**
   * Checks if font is vertical (top-to-bottom layout)
   * @param fonts Font handle
   * @param index Index of the font
   * @returns True if font is vertical
   */
  async isFontVertical(fonts: any, index: number): Promise<boolean> {
    const cacheKey = `font:isvertical:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const vertical = this.native?.font_is_vertical?.(index) ?? false;
    this.setCached(cacheKey, vertical);
    return vertical;
  }

  /**
   * Gets character widths for the font
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Array of character widths
   */
  async getFontWidths(fonts: any, index: number): Promise<Float32Array> {
    const cacheKey = `font:widths:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const widths = this.native?.font_get_widths?.(index) ?? new Float32Array();
    this.setCached(cacheKey, widths);
    return widths;
  }

  /**
   * Gets the ascender metric (height above baseline)
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Ascender value in font units
   */
  async getFontAscender(fonts: any, index: number): Promise<number> {
    const cacheKey = `font:ascender:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const ascender = this.native?.font_get_ascender?.(index) ?? 0;
    this.setCached(cacheKey, ascender);
    return ascender;
  }

  /**
   * Gets the descender metric (depth below baseline)
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Descender value in font units (usually negative)
   */
  async getFontDescender(fonts: any, index: number): Promise<number> {
    const cacheKey = `font:descender:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const descender = this.native?.font_get_descender?.(index) ?? 0;
    this.setCached(cacheKey, descender);
    return descender;
  }

  /**
   * Gets all font properties at once
   * More efficient than individual property calls
   * @param fonts Font handle
   * @param index Index of the font
   * @returns Object with all font properties
   */
  async getFontAllProperties(fonts: any, index: number): Promise<FontProperties> {
    const cacheKey = `font:all:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Aggregate all font properties by calling individual native functions
    const baseFontName = this.native?.font_get_base_font_name?.(index) ?? '';
    const descriptor = this.native?.font_get_descriptor?.(index) ?? '{}';
    const descendantFont = this.native?.font_get_descendant_font?.(index) ?? '';
    const toUnicodeCmap = this.native?.font_get_to_unicode_cmap?.(index) ?? '';
    const isVertical = this.native?.font_is_vertical?.(index) ?? false;
    const widths = this.native?.font_get_widths?.(index) ?? new Float32Array();
    const ascender = this.native?.font_get_ascender?.(index) ?? 0;
    const descender = this.native?.font_get_descender?.(index) ?? 0;

    const props: FontProperties = {
      baseFontName,
      descriptor,
      descendantFont,
      toUnicodeCmap,
      isVertical,
      widths,
      ascender,
      descender,
    };
    this.setCached(cacheKey, props);
    this.emit('fontPropertiesExtracted', index);
    return props;
  }

  // ========== Image Accessors (5 functions) ==========

  /**
   * Checks if image has an alpha channel
   * @param images Image handle
   * @param index Index of the image
   * @returns True if alpha channel is present
   */
  async hasImageAlphaChannel(images: any, index: number): Promise<boolean> {
    const cacheKey = `image:hasalpha:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const hasAlpha = this.native?.image_has_alpha_channel?.(index) ?? false;
    this.setCached(cacheKey, hasAlpha);
    return hasAlpha;
  }

  /**
   * Gets the ICC color profile
   * @param images Image handle
   * @param index Index of the image
   * @returns ICC profile as binary data
   */
  async getImageIccProfile(images: any, index: number): Promise<Uint8Array> {
    const cacheKey = `image:iccprofile:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const profile = this.native?.image_get_icc_profile?.(index) ?? new Uint8Array();
    this.setCached(cacheKey, profile);
    return profile;
  }

  /**
   * Gets the filter chain applied to the image
   * (e.g., ["FlateDecode", "DCTDecode"])
   * @param images Image handle
   * @param index Index of the image
   * @returns Filter chain as JSON string
   */
  async getImageFilterChain(images: any, index: number): Promise<string> {
    const cacheKey = `image:filterchain:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const filterChain = this.native?.image_get_filter_chain?.(index) ?? '[]';
    this.setCached(cacheKey, filterChain);
    return filterChain;
  }

  /**
   * Gets the decoded image data
   * @param images Image handle
   * @param index Index of the image
   * @returns Decoded image data as binary
   */
  async getImageDecodedData(images: any, index: number): Promise<Uint8Array> {
    const cacheKey = `image:decoded:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const data = this.native?.image_get_decoded_data?.(index) ?? new Uint8Array();
    this.setCached(cacheKey, data);
    return data;
  }

  /**
   * Gets the image width in pixels
   * @param images Image handle
   * @param index Index of the image
   * @returns Width in pixels
   */
  async getImageWidth(images: any, index: number): Promise<number> {
    const cacheKey = `image:width:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const width = this.native?.image_get_width?.(index) ?? 0;
    this.setCached(cacheKey, width);
    return width;
  }

  /**
   * Gets the image height in pixels
   * @param images Image handle
   * @param index Index of the image
   * @returns Height in pixels
   */
  async getImageHeight(images: any, index: number): Promise<number> {
    const cacheKey = `image:height:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const height = this.native?.image_get_height?.(index) ?? 0;
    this.setCached(cacheKey, height);
    return height;
  }

  /**
   * Gets the color space of the image
   * @param images Image handle
   * @param index Index of the image
   * @returns Color space name (e.g., "RGB", "CMYK", "Gray")
   */
  async getImageColorSpace(images: any, index: number): Promise<string> {
    const cacheKey = `image:colorspace:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const colorSpace = this.native?.image_get_color_space?.(index) ?? 'RGB';
    this.setCached(cacheKey, colorSpace);
    return colorSpace;
  }

  /**
   * Gets all image properties at once
   * More efficient than individual property calls
   * @param images Image handle
   * @param index Index of the image
   * @returns Object with all image properties
   */
  async getImageAllProperties(images: any, index: number): Promise<ImageProperties> {
    const cacheKey = `image:all:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Aggregate all image properties by calling individual native functions
    const hasAlphaChannel = this.native?.image_has_alpha_channel?.(index) ?? false;
    const iccProfile = this.native?.image_get_icc_profile?.(index) ?? new Uint8Array();
    const filterChain = this.native?.image_get_filter_chain?.(index) ?? '[]';
    const decodedData = this.native?.image_get_decoded_data?.(index) ?? new Uint8Array();
    const width = this.native?.image_get_width?.(index) ?? 0;
    const height = this.native?.image_get_height?.(index) ?? 0;
    const colorSpace = this.native?.image_get_color_space?.(index) ?? 'RGB';

    const props: ImageProperties = {
      hasAlphaChannel,
      iccProfile,
      filterChain,
      decodedData,
      width,
      height,
      colorSpace,
    };
    this.setCached(cacheKey, props);
    this.emit('imagePropertiesExtracted', index);
    return props;
  }

  // ========== Annotation Accessors (6 functions) ==========

  /**
   * Gets the modified date of an annotation
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Timestamp in milliseconds
   */
  async getAnnotationModifiedDate(annotations: any, index: number): Promise<number> {
    const cacheKey = `annotation:modifieddate:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const timestamp = this.native?.annotation_get_modified_date?.(index) ?? 0;
    this.setCached(cacheKey, timestamp);
    return timestamp;
  }

  /**
   * Gets the subject/title of an annotation
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Subject text
   */
  async getAnnotationSubject(annotations: any, index: number): Promise<string> {
    const cacheKey = `annotation:subject:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const subject = this.native?.annotation_get_subject?.(index) ?? '';
    this.setCached(cacheKey, subject);
    return subject;
  }

  /**
   * Gets the index of the annotation this is replying to
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Index of parent annotation, or -1 if not a reply
   */
  async getAnnotationReplyToIndex(annotations: any, index: number): Promise<number> {
    const cacheKey = `annotation:replyto:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const replyToIndex = this.native?.annotation_get_reply_to?.(index) ?? -1;
    this.setCached(cacheKey, replyToIndex);
    return replyToIndex;
  }

  /**
   * Gets the page number where annotation appears
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Page number (0-based)
   */
  async getAnnotationPageNumber(annotations: any, index: number): Promise<number> {
    const cacheKey = `annotation:pagenum:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const pageNumber = this.native?.annotation_get_page_number?.(index) ?? 0;
    this.setCached(cacheKey, pageNumber);
    return pageNumber;
  }

  /**
   * Gets the icon name for the annotation
   * (e.g., "Comment", "Note", "Help")
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Icon name
   */
  async getAnnotationIconName(annotations: any, index: number): Promise<string> {
    const cacheKey = `annotation:icon:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const icon = this.native?.annotation_get_icon_name?.(index) ?? '';
    this.setCached(cacheKey, icon);
    return icon;
  }

  /**
   * Gets the author/creator of the annotation
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Author name
   */
  async getAnnotationAuthor(annotations: any, index: number): Promise<string> {
    const cacheKey = `annotation:author:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    const author = this.native?.annotation_get_author?.(index) ?? '';
    this.setCached(cacheKey, author);
    return author;
  }

  /**
   * Gets all annotation properties at once
   * More efficient than individual property calls
   * @param annotations Annotation handle
   * @param index Index of the annotation
   * @returns Object with all annotation properties
   */
  async getAnnotationAllProperties(annotations: any, index: number): Promise<AnnotationProperties> {
    const cacheKey = `annotation:all:${index}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Aggregate all annotation properties by calling individual native functions
    const modifiedDate = this.native?.annotation_get_modified_date?.(index) ?? 0;
    const subject = this.native?.annotation_get_subject?.(index) ?? '';
    const replyToIndex = this.native?.annotation_get_reply_to?.(index) ?? -1;
    const pageNumber = this.native?.annotation_get_page_number?.(index) ?? 0;
    const iconName = this.native?.annotation_get_icon_name?.(index) ?? '';
    const author = this.native?.annotation_get_author?.(index) ?? '';

    const props: AnnotationProperties = {
      modifiedDate,
      subject,
      replyToIndex,
      pageNumber,
      iconName,
      author,
    };
    this.setCached(cacheKey, props);
    this.emit('annotationPropertiesExtracted', index);
    return props;
  }

  // ========== Cache Management ==========

  /**
   * Clears the result cache
   */
  clearCache(): void {
    this.resultCache.clear();
    this.emit('cacheCleared');
  }

  /**
   * Gets cache statistics
   * @returns Object with cache information
   */
  getCacheStats(): Record<string, any> {
    return {
      cacheSize: this.resultCache.size,
      maxCacheSize: this.maxCacheSize,
      entries: Array.from(this.resultCache.keys()),
    };
  }

  private clearCachePattern(pattern: string): void {
    const regex = new RegExp(pattern);
    const keysToDelete = Array.from(this.resultCache.keys()).filter((key) => regex.test(key));
    keysToDelete.forEach((key) => {
      this.resultCache.delete(key);
    });
  }
}

export default ResultAccessorsManager;
