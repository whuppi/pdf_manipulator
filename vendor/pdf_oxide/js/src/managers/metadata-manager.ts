/**
 * Manager for PDF document metadata
 *
 * Provides access to document metadata properties such as title, author,
 * subject, keywords, creation and modification dates, and custom properties.
 *
 * @example
 * ```typescript
 * import { MetadataManager } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const metadataManager = new MetadataManager(doc);
 *
 * console.log(`Title: ${metadataManager.getTitle()}`);
 * console.log(`Author: ${metadataManager.getAuthor()}`);
 * console.log(`Created: ${metadataManager.getCreationDate()}`);
 * ```
 */

export interface MetadataComparison {
  matching: Record<string, any>;
  differing: Record<string, { document1: any; document2: any }>;
}

export interface ValidationResult {
  isComplete: boolean;
  issues: string[];
  missingFieldCount: number;
}

export class MetadataManager {
  private _document: any;

  /**
   * Creates a new MetadataManager for the given document
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
   * Gets the document title
   * @returns Document title or null if not set
   */
  getTitle(): string | null {
    try {
      return this._document.documentInfo?.title || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document author
   * @returns Document author or null if not set
   */
  getAuthor(): string | null {
    try {
      return this._document.documentInfo?.author || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document subject
   * @returns Document subject or null if not set
   */
  getSubject(): string | null {
    try {
      return this._document.documentInfo?.subject || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document keywords
   * @returns Array of document keywords (empty if none)
   */
  getKeywords(): string[] {
    try {
      return this._document.documentInfo?.keywords || [];
    } catch (error) {
      return [];
    }
  }

  /**
   * Gets the document creator application
   * @returns Creator application or null if not set
   */
  getCreator(): string | null {
    try {
      return this._document.documentInfo?.creator || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document producer application
   * @returns Producer application or null if not set
   */
  getProducer(): string | null {
    try {
      return this._document.documentInfo?.producer || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document creation date
   * @returns Creation date or null if not set
   */
  getCreationDate(): Date | null {
    try {
      return this._document.documentInfo?.creationDate || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets the document modification date
   * @returns Modification date or null if not set
   */
  getModificationDate(): Date | null {
    try {
      return this._document.documentInfo?.modificationDate || null;
    } catch (error) {
      return null;
    }
  }

  /**
   * Gets all metadata as a single object
   * @returns Complete metadata object
   *
   * @example
   * ```typescript
   * const metadata = manager.getAllMetadata();
   * console.log(JSON.stringify(metadata, null, 2));
   * ```
   */
  getAllMetadata(): Record<string, any> {
    try {
      return this._document.documentInfo || {};
    } catch (error) {
      return {};
    }
  }

  /**
   * Checks if metadata has been set
   * @returns True if any metadata is present
   */
  hasMetadata(): boolean {
    const meta = this.getAllMetadata();
    return Object.keys(meta).length > 0;
  }

  /**
   * Gets a summary of key metadata fields
   * @returns Formatted metadata summary
   *
   * @example
   * ```typescript
   * console.log(manager.getMetadataSummary());
   * // Output: Title: My Document, Author: John Doe, Pages: 42
   * ```
   */
  getMetadataSummary(): string {
    const parts: string[] = [];

    const title = this.getTitle();
    if (title) parts.push(`Title: ${title}`);

    const author = this.getAuthor();
    if (author) parts.push(`Author: ${author}`);

    try {
      parts.push(`Pages: ${this._document.pageCount}`);
    } catch (e) {
      // Ignore
    }

    const subject = this.getSubject();
    if (subject) parts.push(`Subject: ${subject}`);

    const keywords = this.getKeywords();
    if (keywords.length > 0) parts.push(`Keywords: ${keywords.join(', ')}`);

    const created = this.getCreationDate();
    if (created) parts.push(`Created: ${(created as any).toISOString()}`);

    return parts.join(' | ');
  }

  /**
   * Checks if a specific keyword is present
   * @param keyword - Keyword to search for
   * @returns True if keyword is found
   */
  hasKeyword(keyword: string): boolean {
    if (!keyword || typeof keyword !== 'string') {
      throw new Error('Keyword must be a non-empty string');
    }

    const keywords = this.getKeywords();
    return keywords.includes(keyword);
  }

  /**
   * Gets the number of keywords
   * @returns Keyword count
   */
  getKeywordCount(): number {
    return this.getKeywords().length;
  }

  /**
   * Gets metadata comparison with another document
   * @param otherDocument - Document to compare with
   * @returns Comparison object with matching and differing fields
   *
   * @example
   * ```typescript
   * const doc1 = PdfDocument.open('file1.pdf');
   * const doc2 = PdfDocument.open('file2.pdf');
   * const mgr1 = new MetadataManager(doc1);
   * const comparison = mgr1.compareWith(doc2);
   * console.log(comparison.matching); // Fields that match
   * console.log(comparison.differing); // Fields that differ
   * ```
   */
  compareWith(otherDocument: any): MetadataComparison {
    if (!otherDocument) {
      throw new Error('Document is required for comparison');
    }

    const otherMgr = new MetadataManager(otherDocument);
    const matching: Record<string, any> = {};
    const differing: Record<string, any> = {};

    const fields = [
      { getter: 'getTitle' as const, field: 'title' },
      { getter: 'getAuthor' as const, field: 'author' },
      { getter: 'getSubject' as const, field: 'subject' },
      { getter: 'getCreator' as const, field: 'creator' },
      { getter: 'getProducer' as const, field: 'producer' },
    ];

    fields.forEach(({ getter, field }) => {
      const val1 = this[getter]();
      const val2 = otherMgr[getter]();

      if (val1 === val2) {
        matching[field] = val1;
      } else {
        differing[field] = { document1: val1, document2: val2 };
      }
    });

    return { matching, differing };
  }

  /**
   * Validates metadata completeness
   * @returns Validation result with issues array
   *
   * @example
   * ```typescript
   * const validation = manager.validate();
   * if (validation.isComplete) {
   *   console.log('Metadata is complete');
   * } else {
   *   console.log('Issues:', validation.issues);
   * }
   * ```
   */
  validate(): ValidationResult {
    const issues: string[] = [];

    if (!this.getTitle()) issues.push('Title is missing');
    if (!this.getAuthor()) issues.push('Author is missing');
    if (!this.getSubject()) issues.push('Subject is missing');
    if (this.getKeywordCount() === 0) issues.push('Keywords are missing');
    if (!this.getCreationDate()) issues.push('Creation date is missing');

    return {
      isComplete: issues.length === 0,
      issues,
      missingFieldCount: issues.length,
    };
  }
}
