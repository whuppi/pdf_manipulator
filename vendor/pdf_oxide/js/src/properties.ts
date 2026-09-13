/**
 * PDF Oxide Property Getters for Node.js
 *
 * Adds JavaScript property getters to native classes for idiomatic API.
 * Converts Java-style getters (getPageCount()) to JavaScript properties (pageCount).
 * Caches results where appropriate for performance.
 */

import type { DocumentInfo, Metadata, Rect } from './types/common';

/**
 * Cache structure for PdfDocument properties
 */
interface DocumentPropertyCache {
  version?: [number, number];
  pageCount?: number;
  documentInfo?: DocumentInfo;
  metadata?: Metadata;
  forms?: any;
  pageLabels?: string[];
  embeddedFiles?: any[];
}

/**
 * Cache structure for Pdf properties
 */
interface PdfPropertyCache {
  version?: [number, number];
  pageCount?: number;
  documentInfo?: DocumentInfo;
  metadata?: Metadata;
  forms?: any;
}

/**
 * Cache structure for PdfPage properties
 */
interface PagePropertyCache {
  bounds?: Rect;
}

/**
 * Adds property getters to PdfDocument class
 * Converts methods to idiomatic JavaScript properties
 *
 * @param PdfDocument - The native PdfDocument class
 * @returns Enhanced PdfDocument class with property getters
 *
 * @example
 * ```typescript
 * const { PdfDocument } = require('pdf_oxide');
 * const doc = PdfDocument.open('file.pdf');
 * console.log(doc.pageCount);  // Instead of doc.get_page_count()
 * console.log(doc.version);     // Instead of doc.get_version()
 * ```
 */
export function addPdfDocumentProperties<T extends { prototype: any }>(PdfDocument: T): T {
  if (!PdfDocument || !PdfDocument.prototype) {
    return PdfDocument;
  }

  const prototype = PdfDocument.prototype;

  // Cache for frequently accessed properties
  const cache = new WeakMap<any, DocumentPropertyCache>();

  /**
   * Gets the PDF version as [major, minor]
   * Caches result for performance
   */
  Object.defineProperty(prototype, 'version', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.version) {
        obj.version = this.get_version ? this.get_version() : [1, 4];
      }
      return obj.version;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the number of pages
   * Caches result for performance
   */
  Object.defineProperty(prototype, 'pageCount', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (obj.pageCount === undefined) {
        try {
          obj.pageCount = this.get_page_count ? this.get_page_count() : 0;
        } catch (err) {
          throw err;
        }
      }
      return obj.pageCount;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Checks if document has structure tree (Tagged PDF)
   */
  Object.defineProperty(prototype, 'hasStructureTree', {
    get(this: any) {
      return this.has_structure_tree ? this.has_structure_tree() : false;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document information metadata
   */
  Object.defineProperty(prototype, 'documentInfo', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.documentInfo) {
        try {
          obj.documentInfo = this.get_document_info ? this.get_document_info() : {};
        } catch (err) {
          throw err;
        }
      }
      return obj.documentInfo;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document XMP metadata
   */
  Object.defineProperty(prototype, 'metadata', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.metadata) {
        try {
          obj.metadata = this.get_metadata ? this.get_metadata() : {};
        } catch (err) {
          throw err;
        }
      }
      return obj.metadata;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document forms (AcroForm)
   */
  Object.defineProperty(prototype, 'forms', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.forms) {
        try {
          obj.forms = this.get_forms ? this.get_forms() : null;
        } catch (err) {
          throw err;
        }
      }
      return obj.forms;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets page labels
   */
  Object.defineProperty(prototype, 'pageLabels', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.pageLabels) {
        try {
          obj.pageLabels = this.get_page_labels ? this.get_page_labels() : [];
        } catch (err) {
          throw err;
        }
      }
      return obj.pageLabels;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets embedded files
   */
  Object.defineProperty(prototype, 'embeddedFiles', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.embeddedFiles) {
        try {
          obj.embeddedFiles = this.get_embedded_files ? this.get_embedded_files() : [];
        } catch (err) {
          throw err;
        }
      }
      return obj.embeddedFiles;
    },
    configurable: true,
    enumerable: true,
  });

  return PdfDocument;
}

/**
 * Adds property getters to Pdf class
 *
 * @param Pdf - The native Pdf class
 * @returns Enhanced Pdf class with property getters
 *
 * @example
 * ```typescript
 * const { Pdf } = require('pdf_oxide');
 * const doc = Pdf.fromMarkdown('# Hello');
 * console.log(doc.pageCount);  // Instead of doc.get_page_count()
 * console.log(doc.version);     // Instead of doc.get_version()
 * ```
 */
export function addPdfProperties<T extends { prototype: any }>(Pdf: T): T {
  if (!Pdf || !Pdf.prototype) {
    return Pdf;
  }

  const prototype = Pdf.prototype;
  const cache = new WeakMap<any, PdfPropertyCache>();

  /**
   * Gets the PDF version as [major, minor]
   */
  Object.defineProperty(prototype, 'version', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.version) {
        obj.version = this.get_version ? this.get_version() : [1, 4];
      }
      return obj.version;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the number of pages
   */
  Object.defineProperty(prototype, 'pageCount', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (obj.pageCount === undefined) {
        try {
          obj.pageCount = this.get_page_count ? this.get_page_count() : 0;
        } catch (err) {
          throw err;
        }
      }
      return obj.pageCount;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document information metadata
   */
  Object.defineProperty(prototype, 'documentInfo', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.documentInfo) {
        try {
          obj.documentInfo = this.get_document_info ? this.get_document_info() : {};
        } catch (err) {
          throw err;
        }
      }
      return obj.documentInfo;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document XMP metadata
   */
  Object.defineProperty(prototype, 'metadata', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.metadata) {
        try {
          obj.metadata = this.get_metadata ? this.get_metadata() : {};
        } catch (err) {
          throw err;
        }
      }
      return obj.metadata;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets document forms
   */
  Object.defineProperty(prototype, 'forms', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.forms) {
        try {
          obj.forms = this.get_forms ? this.get_forms() : null;
        } catch (err) {
          throw err;
        }
      }
      return obj.forms;
    },
    configurable: true,
    enumerable: true,
  });

  return Pdf;
}

/**
 * Adds property getters to PdfPage class
 *
 * @param PdfPage - The native PdfPage class
 * @returns Enhanced PdfPage class with property getters
 *
 * @example
 * ```typescript
 * const page = doc.page(0);
 * console.log(page.width);      // Instead of page.get_width()
 * console.log(page.height);     // Instead of page.get_height()
 * console.log(page.pageIndex);  // Instead of page.get_page_index()
 * ```
 */
export function addPdfPageProperties<T extends { prototype: any }>(PdfPage: T): T {
  if (!PdfPage || !PdfPage.prototype) {
    return PdfPage;
  }

  const prototype = PdfPage.prototype;
  const cache = new WeakMap<any, PagePropertyCache>();

  /**
   * Gets the page index (zero-based)
   */
  Object.defineProperty(prototype, 'pageIndex', {
    get(this: any) {
      return this.get_page_index ? this.get_page_index() : 0;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the page width in points
   */
  Object.defineProperty(prototype, 'width', {
    get(this: any) {
      return this.get_width ? this.get_width() : 612.0;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the page height in points
   */
  Object.defineProperty(prototype, 'height', {
    get(this: any) {
      return this.get_height ? this.get_height() : 792.0;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the page bounds as a Rect
   */
  Object.defineProperty(prototype, 'bounds', {
    get(this: any) {
      if (!cache.has(this)) {
        cache.set(this, {});
      }
      const obj = cache.get(this)!;

      if (!obj.bounds) {
        if (this.get_bounds) {
          obj.bounds = this.get_bounds();
        } else {
          // Return default bounds [0, 0, width, height]
          obj.bounds = {
            x: 0,
            y: 0,
            width: this.width || 612.0,
            height: this.height || 792.0,
          };
        }
      }
      return obj.bounds;
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the page orientation (portrait or landscape)
   */
  Object.defineProperty(prototype, 'orientation', {
    get(this: any) {
      const width = this.width || 612.0;
      const height = this.height || 792.0;
      return height >= width ? 'portrait' : 'landscape';
    },
    configurable: true,
    enumerable: true,
  });

  /**
   * Gets the page aspect ratio (width / height)
   */
  Object.defineProperty(prototype, 'aspectRatio', {
    get(this: any) {
      const width = this.width || 612.0;
      const height = this.height || 792.0;
      return width / height;
    },
    configurable: true,
    enumerable: true,
  });

  return PdfPage;
}

/**
 * Adds property getters to all supported classes
 *
 * @param classes - Object with class references
 * @returns Object with enhanced classes
 *
 * @example
 * ```typescript
 * const classes = {
 *   PdfDocument,
 *   Pdf,
 *   PdfPage,
 *   PdfElement,
 *   PdfText,
 *   PdfImage,
 * };
 * const enhanced = addPropertiesToAll(classes);
 * ```
 */
export function addPropertiesToAll(classes: Record<string, any>): Record<string, any> {
  if (classes.PdfDocument) {
    classes.PdfDocument = addPdfDocumentProperties(classes.PdfDocument);
  }
  if (classes.Pdf) {
    classes.Pdf = addPdfProperties(classes.Pdf);
  }
  if (classes.PdfPage) {
    classes.PdfPage = addPdfPageProperties(classes.PdfPage);
  }

  return classes;
}
