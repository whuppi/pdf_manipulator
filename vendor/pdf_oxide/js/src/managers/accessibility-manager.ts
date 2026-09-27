/**
 * AccessibilityManager - PDF Accessibility Operations
 *
 * Provides accessibility analysis and remediation capabilities including:
 * - Tagged PDF detection
 * - Structure tree extraction
 * - Auto-tagging
 * - Alt text management
 * - Language and title metadata
 *
 * @since 1.0.0
 */

import { EventEmitter } from 'events';
import { AccessibilityException, mapFfiErrorCode } from '../errors';

// =============================================================================
// Type Definitions
// =============================================================================

/**
 * Represents a structure element in a tagged PDF.
 */
export interface StructureElement {
  /** Structure element type (e.g., 'P', 'H1', 'Figure', 'Table') */
  readonly type: string;
  /** Alt text for the element, if set */
  readonly altText?: string;
  /** Actual text content of the element */
  readonly actualText?: string;
  /** Language of the element */
  readonly language?: string;
  /** Page index where this element appears */
  readonly pageIndex: number;
  /** Marked content identifier */
  readonly mcid: number;
  /** Child elements */
  readonly children: readonly StructureElement[];
}

/**
 * Represents the full structure tree of a tagged PDF.
 */
export interface StructureTree {
  /** Whether the document is tagged */
  readonly isTagged: boolean;
  /** Root-level structure elements */
  readonly elements: readonly StructureElement[];
  /** Total element count across all levels */
  readonly totalElements: number;
  /** Document language from the structure tree root */
  readonly language?: string;
}

/**
 * Result of an auto-tag operation.
 */
export interface AutoTagResult {
  /** Whether auto-tagging succeeded */
  readonly success: boolean;
  /** Number of elements tagged */
  readonly elementsTagged: number;
  /** Number of images found */
  readonly imagesFound: number;
  /** Number of headings detected */
  readonly headingsDetected: number;
  /** Warnings generated during tagging */
  readonly warnings: readonly string[];
}

// =============================================================================
// AccessibilityManager
// =============================================================================

/**
 * Manager for PDF accessibility operations.
 *
 * Provides methods for inspecting and improving the accessibility of
 * PDF documents, including tagged PDF detection, structure tree analysis,
 * auto-tagging, and alt text management.
 *
 * @example
 * ```typescript
 * const accessibility = new AccessibilityManager(document);
 *
 * // Check if document is tagged
 * const tagged = await accessibility.isTagged();
 *
 * // Auto-tag an untagged document
 * if (!tagged) {
 *   const result = await accessibility.autoTag('en');
 *   console.log(`Tagged ${result.elementsTagged} elements`);
 * }
 *
 * // Set alt text on images
 * await accessibility.setAltText(0, 1, 'Company logo');
 * ```
 */
export class AccessibilityManager extends EventEmitter {
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
  // Accessibility Analysis
  // ===========================================================================

  /**
   * Checks whether the PDF document is tagged.
   *
   * A tagged PDF contains a structure tree that defines the logical
   * reading order and semantic structure of the content.
   *
   * @returns True if the document is tagged
   * @throws AccessibilityException if the check fails
   */
  async isTagged(): Promise<boolean> {
    if (!this.native?.pdf_accessibility_is_tagged) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_is_tagged not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const result = this.native.pdf_accessibility_is_tagged(
      this.document._handle ?? this.document,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to check if document is tagged');
    }

    this.emit('tagged-checked', { isTagged: result });
    return !!result;
  }

  /**
   * Gets the full structure tree of the document.
   *
   * Returns the hierarchical structure tree that defines the logical
   * organization and reading order of the document content.
   *
   * @returns The document structure tree
   * @throws AccessibilityException if the extraction fails
   */
  async getStructureTree(): Promise<StructureTree> {
    if (!this.native?.pdf_accessibility_get_structure_tree) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_get_structure_tree not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_accessibility_get_structure_tree(
      this.document._handle ?? this.document,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to get structure tree');
    }

    try {
      const tree: StructureTree = typeof resultPtr === 'string' ? JSON.parse(resultPtr) : resultPtr;

      this.emit('structure-tree-retrieved', { totalElements: tree.totalElements });

      // Free native handle if needed
      if (this.native.pdf_structure_tree_free && typeof resultPtr !== 'string') {
        this.native.pdf_structure_tree_free(resultPtr);
      }

      return tree;
    } catch {
      return {
        isTagged: false,
        elements: [],
        totalElements: 0,
      };
    }
  }

  /**
   * Automatically tags an untagged PDF document.
   *
   * Uses heuristic analysis to detect document structure and apply
   * appropriate tags for headings, paragraphs, images, tables, and lists.
   *
   * @param language - Optional BCP-47 language tag (e.g., 'en', 'fr', 'de')
   * @returns Result of the auto-tagging operation
   * @throws AccessibilityException if auto-tagging fails
   */
  async autoTag(language?: string): Promise<AutoTagResult> {
    if (!this.native?.pdf_accessibility_auto_tag) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_auto_tag not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    const resultPtr = this.native.pdf_accessibility_auto_tag(
      this.document._handle ?? this.document,
      language ?? null,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to auto-tag document');
    }

    let result: AutoTagResult;
    try {
      result =
        typeof resultPtr === 'string'
          ? JSON.parse(resultPtr)
          : (resultPtr ?? {
              success: true,
              elementsTagged: 0,
              imagesFound: 0,
              headingsDetected: 0,
              warnings: [],
            });
    } catch {
      result = {
        success: true,
        elementsTagged: 0,
        imagesFound: 0,
        headingsDetected: 0,
        warnings: [],
      };
    }

    this.emit('auto-tagged', { language, elementsTagged: result.elementsTagged });
    return result;
  }

  // ===========================================================================
  // Alt Text Management
  // ===========================================================================

  /**
   * Sets alt text for a structure element identified by page and MCID.
   *
   * @param page - Zero-based page index
   * @param mcid - Marked content identifier of the element
   * @param text - Alt text to set
   * @throws AccessibilityException if the operation fails
   */
  async setAltText(page: number, mcid: number, text: string): Promise<void> {
    if (!this.native?.pdf_accessibility_set_alt_text) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_set_alt_text not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_accessibility_set_alt_text(
      this.document._handle ?? this.document,
      page,
      mcid,
      text,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, `Failed to set alt text on page ${page}, mcid ${mcid}`);
    }

    this.emit('alt-text-set', { page, mcid, text });
  }

  // ===========================================================================
  // Document-Level Accessibility Metadata
  // ===========================================================================

  /**
   * Sets the document language.
   *
   * @param lang - BCP-47 language tag (e.g., 'en', 'en-US', 'fr')
   * @throws AccessibilityException if the operation fails
   */
  async setLanguage(lang: string): Promise<void> {
    if (!this.native?.pdf_accessibility_set_language) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_set_language not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_accessibility_set_language(
      this.document._handle ?? this.document,
      lang,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, `Failed to set document language to '${lang}'`);
    }

    this.emit('language-set', { lang });
  }

  /**
   * Sets the document title for accessibility.
   *
   * @param title - Document title
   * @throws AccessibilityException if the operation fails
   */
  async setTitle(title: string): Promise<void> {
    if (!this.native?.pdf_accessibility_set_title) {
      throw new AccessibilityException(
        'Native accessibility not available: pdf_accessibility_set_title not found'
      );
    }

    const errorCode = Buffer.alloc(4);
    this.native.pdf_accessibility_set_title(
      this.document._handle ?? this.document,
      title,
      errorCode
    );
    const code = errorCode.readInt32LE(0);

    if (code !== 0) {
      throw mapFfiErrorCode(code, 'Failed to set document title');
    }

    this.emit('title-set', { title });
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

export default AccessibilityManager;
