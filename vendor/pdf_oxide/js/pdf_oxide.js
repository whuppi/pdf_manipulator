const binding = require('./build/Release/pdf_oxide');

/**
 * Represents an open PDF document
 */
class PdfDocument {
  /**
   * Opens a PDF document from file path
   * @param {string} path - Path to the PDF file
   * @throws {Error} If file cannot be opened or is not a valid PDF
   */
  constructor(path) {
    if (typeof path !== 'string') {
      throw new TypeError('Path must be a string');
    }
    this._handle = binding.openDocument(path);
    this._closed = false;
  }

  /**
   * Opens a PDF document from a Buffer or Uint8Array
   * @param {Buffer|Uint8Array} data - PDF file contents
   * @returns {PdfDocument} An open PDF document
   * @throws {TypeError} If data is not a Buffer or Uint8Array
   * @throws {Error} If data is not a valid PDF
   */
  static openFromBuffer(data) {
    if (!Buffer.isBuffer(data) && !(data instanceof Uint8Array)) {
      throw new TypeError('data must be a Buffer or Uint8Array');
    }
    const doc = Object.create(PdfDocument.prototype);
    doc._handle = binding.openFromBuffer(data);
    doc._closed = false;
    return doc;
  }

  /**
   * Opens a password-protected PDF document
   * @param {string} path - Path to the PDF file
   * @param {string} password - Document password
   * @returns {PdfDocument} An open PDF document
   * @throws {Error} If file cannot be opened or password is wrong
   */
  static openWithPassword(path, password) {
    if (typeof path !== 'string') throw new TypeError('path must be a string');
    if (typeof password !== 'string') throw new TypeError('password must be a string');
    const doc = Object.create(PdfDocument.prototype);
    doc._handle = binding.openWithPassword(path, password);
    doc._closed = false;
    return doc;
  }

  /**
   * Returns the number of pages in the document
   * @returns {number} Page count
   * @throws {Error} If document is closed or operation fails
   */
  getPageCount() {
    this._checkClosed();
    return binding.getPageCount(this._handle);
  }

  /**
   * Gets the PDF version
   * @returns {{major: number, minor: number}} Version tuple
   * @throws {Error} If document is closed
   */
  getVersion() {
    this._checkClosed();
    return binding.getVersion(this._handle);
  }

  /**
   * Checks if document has a structure tree (Tagged PDF)
   * @returns {boolean} Whether document has structure tree
   * @throws {Error} If document is closed
   */
  hasStructureTree() {
    this._checkClosed();
    return binding.hasStructureTree(this._handle);
  }

  /**
   * Extracts plain text from a page
   * @param {number} pageIndex - Page index (0-based)
   * @returns {string} Extracted text
   * @throws {Error} If page index is invalid or extraction fails
   */
  extractText(pageIndex) {
    this._checkClosed();
    if (!Number.isInteger(pageIndex) || pageIndex < 0) {
      throw new RangeError('pageIndex must be a non-negative integer');
    }
    return binding.extractText(this._handle, pageIndex);
  }

  /**
   * Converts a page to Markdown format
   * @param {number} pageIndex - Page index (0-based)
   * @returns {string} Markdown formatted text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toMarkdown(pageIndex) {
    this._checkClosed();
    if (!Number.isInteger(pageIndex) || pageIndex < 0) {
      throw new RangeError('pageIndex must be a non-negative integer');
    }
    return binding.toMarkdown(this._handle, pageIndex);
  }

  /**
   * Converts a page to HTML format
   * @param {number} pageIndex - Page index (0-based)
   * @returns {string} HTML formatted text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toHtml(pageIndex) {
    this._checkClosed();
    if (!Number.isInteger(pageIndex) || pageIndex < 0) {
      throw new RangeError('pageIndex must be a non-negative integer');
    }
    return binding.toHtml(this._handle, pageIndex);
  }

  /**
   * Converts a page to plain text format
   * @param {number} pageIndex - Page index (0-based)
   * @returns {string} Plain text
   * @throws {Error} If page index is invalid or conversion fails
   */
  toPlainText(pageIndex) {
    this._checkClosed();
    if (!Number.isInteger(pageIndex) || pageIndex < 0) {
      throw new RangeError('pageIndex must be a non-negative integer');
    }
    return binding.toPlainText(this._handle, pageIndex);
  }

  /**
   * Converts all pages to Markdown format
   * @returns {string} Markdown formatted text for all pages
   * @throws {Error} If conversion fails
   */
  toMarkdownAll() {
    this._checkClosed();
    return binding.toMarkdownAll(this._handle);
  }

  /**
   * Validate PDF/A conformance.
   * @param {string} level - "1a"|"1b"|"2a"|"2b"|"2u"|"3a"|"3b"|"3u" (default "2b")
   * @returns {{compliant: boolean, errors: string[], warnings: string[]}}
   */
  validatePdfA(level = '2b') {
    this._checkClosed();
    const levelMap = { '1b': 0, '1a': 1, '2b': 2, '2a': 3, '2u': 4, '3b': 5, '3a': 6, '3u': 7 };
    const levelInt = levelMap[level];
    if (levelInt === undefined) throw new RangeError(`Unknown PDF/A level: "${level}"`);
    return binding.validatePdfALevel(this._handle, levelInt);
  }

  /**
   * Convert document to PDF/A conformance in-place.
   * @param {string} level - "1b"|"2b"|"2u"|"3b" etc. (default "2b")
   * @returns {boolean} true on success
   */
  convertToPdfA(level = '2b') {
    this._checkClosed();
    const levelMap = { '1b': 0, '1a': 1, '2b': 2, '2a': 3, '2u': 4, '3b': 5, '3a': 6, '3u': 7 };
    const levelInt = levelMap[level];
    if (levelInt === undefined) throw new RangeError(`Unknown PDF/A level: "${level}"`);
    return binding.convertToPdfA(this._handle, levelInt);
  }

  /**
   * Validate PDF/X conformance.
   * @param {string} level - "1a_2001"|"1a_2003"|"3_2003"|"4"|"5"|"6" (default "4")
   * @returns {{compliant: boolean, errors: string[], warnings: string[]}}
   */
  validatePdfX(level = '4') {
    this._checkClosed();
    const levelMap = { '1a_2001': 0, '1a_2003': 1, '3_2003': 2, '4': 3, '5': 4, '6': 5 };
    const levelInt = levelMap[level];
    if (levelInt === undefined) throw new RangeError(`Unknown PDF/X level: "${level}"`);
    return binding.validatePdfXLevel(this._handle, levelInt);
  }

  /**
   * Validate PDF/UA accessibility conformance.
   * @param {string} level - "ua1" (default and only supported level)
   * @returns {{accessible: boolean, errors: string[], warnings: string[], stats: object}}
   */
  validatePdfUA(level = 'ua1') {
    this._checkClosed();
    const levelMap = { 'ua1': 0 };
    const levelInt = levelMap[level];
    if (levelInt === undefined) throw new RangeError(`Unknown PDF/UA level: "${level}"`);
    return binding.validatePdfUA(this._handle, levelInt);
  }

  /**
   * Closes the document and releases resources
   */
  close() {
    if (!this._closed && this._handle) {
      binding.closeDocument(this._handle);
      this._closed = true;
      this._handle = null;
    }
  }

  /**
   * Returns whether the document is closed
   * @returns {boolean}
   */
  isClosed() {
    return this._closed;
  }

  /**
   * Automatically closes document when garbage collected
   */
  [Symbol.dispose]() {
    this.close();
  }

  /**
   * Check if document is still open
   * @private
   */
  _checkClosed() {
    if (this._closed) {
      throw new Error('Document is closed');
    }
  }
}

/**
 * Log level constants
 * @enum {number}
 */
const LogLevel = {
  OFF: 0,
  ERROR: 1,
  WARN: 2,
  INFO: 3,
  DEBUG: 4,
  TRACE: 5,
};

/**
 * Sets the global log level for the pdf_oxide library
 * @param {number} level - Log level (0=Off, 1=Error, 2=Warn, 3=Info, 4=Debug, 5=Trace)
 */
function setLogLevel(level) {
  if (typeof level !== 'number' || level < 0 || level > 5) {
    throw new RangeError('level must be a number between 0 (Off) and 5 (Trace)');
  }
  binding.setLogLevel(level);
}

/**
 * Gets the current log level
 * @returns {number} Current log level (0-5)
 */
function getLogLevel() {
  return binding.getLogLevel();
}

module.exports = { PdfDocument, LogLevel, setLogLevel, getLogLevel };
