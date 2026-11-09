/**
 * PDF Oxide - Node.js/TypeScript Bindings
 */

/**
 * Version information
 */
export interface Version {
  major: number;
  minor: number;
}

/**
 * Log level constants
 */
export declare const LogLevel: {
  /** Disable all logging */
  readonly OFF: 0;
  /** Error messages only */
  readonly ERROR: 1;
  /** Warnings and errors */
  readonly WARN: 2;
  /** Informational messages */
  readonly INFO: 3;
  /** Debug messages */
  readonly DEBUG: 4;
  /** Verbose trace messages */
  readonly TRACE: 5;
};

/**
 * Sets the global log level for the pdf_oxide library.
 * @param level - Log level (0=Off, 1=Error, 2=Warn, 3=Info, 4=Debug, 5=Trace)
 */
export declare function setLogLevel(level: number): void;

/**
 * Gets the current log level.
 * @returns Current log level (0-5)
 */
export declare function getLogLevel(): number;

/**
 * Represents an open PDF document
 */
export declare class PdfDocument {
  /**
   * Opens a PDF document from file path
   * @param path Path to the PDF file
   * @throws {Error} If file cannot be opened or is not a valid PDF
   */
  constructor(path: string);

  /**
   * Opens a PDF document from a Buffer or Uint8Array
   * @param data - PDF file contents as bytes
   * @returns An open PDF document
   * @throws {TypeError} If data is not a Buffer or Uint8Array
   * @throws {Error} If data is not a valid PDF
   */
  static openFromBuffer(data: Buffer | Uint8Array): PdfDocument;

  /**
   * Opens a password-protected PDF document
   * @param path - Path to the PDF file
   * @param password - Document password
   * @returns An open PDF document
   * @throws {Error} If file cannot be opened or password is wrong
   */
  static openWithPassword(path: string, password: string): PdfDocument;

  /**
   * Returns the number of pages in the document
   * @returns Page count
   * @throws {Error} If document is closed or operation fails
   */
  getPageCount(): number;

  /**
   * Gets the PDF version
   * @returns Version tuple with major and minor version numbers
   * @throws {Error} If document is closed
   */
  getVersion(): Version;

  /**
   * Checks if document has a structure tree (Tagged PDF)
   * @returns Whether document has structure tree
   * @throws {Error} If document is closed
   */
  hasStructureTree(): boolean;

  /**
   * Extracts plain text from a page
   * @param pageIndex Page index (0-based)
   * @returns Extracted text
   * @throws {Error} If page index is invalid or extraction fails
   */
  extractText(pageIndex: number): string;

  /**
   * Converts a page to Markdown format
   * @param pageIndex Page index (0-based)
   * @returns Markdown formatted text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toMarkdown(pageIndex: number): string;

  /**
   * Converts a page to HTML format
   * @param pageIndex Page index (0-based)
   * @returns HTML formatted text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toHtml(pageIndex: number): string;

  /**
   * Converts a page to plain text format
   * @param pageIndex Page index (0-based)
   * @returns Plain text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toPlainText(pageIndex: number): string;

  /**
   * Converts all pages to Markdown format
   * @returns Markdown formatted text for all pages
   * @throws {Error} If conversion fails
   */
  toMarkdownAll(): string;

  /**
   * Validate PDF/A conformance.
   * @param level - "1a"|"1b"|"2a"|"2b"|"2u"|"3a"|"3b"|"3u" (default "2b")
   * @returns Validation result with errors and warnings arrays
   */
  validatePdfA(
    level?: '1a' | '1b' | '2a' | '2b' | '2u' | '3a' | '3b' | '3u'
  ): { compliant: boolean; errors: string[]; warnings: string[] };

  convertToPdfA(
    level?: '1a' | '1b' | '2a' | '2b' | '2u' | '3a' | '3b' | '3u'
  ): boolean;

  /**
   * Validate PDF/X conformance.
   * @param level - "1a_2001"|"1a_2003"|"3_2003"|"4"|"5"|"6" (default "4")
   * @returns Validation result with errors array
   */
  validatePdfX(
    level?: '1a_2001' | '1a_2003' | '3_2003' | '4' | '5' | '6'
  ): { compliant: boolean; errors: string[]; warnings: string[] };

  /**
   * Validate PDF/UA accessibility conformance.
   * @param level - "ua1" (default, only currently supported level)
   * @returns Accessibility result with errors, warnings, and stats
   */
  validatePdfUA(level?: 'ua1'): {
    accessible: boolean;
    errors: string[];
    warnings: string[];
    stats: {
      structureElements: number;
      images: number;
      tables: number;
      formFields: number;
      annotations: number;
      pages: number;
    };
  };

  /**
   * Closes the document and releases resources
   */
  close(): void;

  /**
   * Returns whether the document is closed
   */
  isClosed(): boolean;

  /**
   * Disposable resource cleanup
   */
  [Symbol.dispose](): void;
}
