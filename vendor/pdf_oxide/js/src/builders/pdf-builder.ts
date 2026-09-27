/**
 * Fluent builder for creating PDF documents with configuration
 *
 * Provides a fluent API for configuring PDF document metadata and options
 * before document creation.
 *
 * @example
 * ```typescript
 * import { PdfBuilder } from 'pdf_oxide';
 *
 * const pdf = PdfBuilder.create()
 *   .title('My Document')
 *   .author('John Doe')
 *   .subject('PDF Creation')
 *   .keywords(['pdf', 'document', 'example'])
 *   .fromMarkdown('# Content\n\nMarkdown text here');
 *
 * pdf.save('output.pdf');
 * ```
 */

interface PdfBuilderConfig {
  title?: string;
  author?: string;
  subject?: string;
  keywords: string[];
  pageSize?: string;
  margins: {
    top: number;
    right: number;
    bottom: number;
    left: number;
  };
}

export class PdfBuilder {
  private _title?: string;
  private _author?: string;
  private _subject?: string;
  private _keywords: string[] = [];
  private _pageSize?: string;
  private _margins: { top: number; right: number; bottom: number; left: number } = {
    top: 36,
    bottom: 36,
    left: 36,
    right: 36,
  };

  /**
   * Creates a new PdfBuilder instance
   * @private
   */
  private constructor() {}

  /**
   * Creates a new PdfBuilder instance
   * @returns New builder instance
   *
   * @example
   * ```typescript
   * const builder = PdfBuilder.create();
   * ```
   */
  static create(): PdfBuilder {
    return new PdfBuilder();
  }

  /**
   * Sets the document title
   * @param title - The document title
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.title('My Document Title');
   * ```
   */
  title(title: string): this {
    if (typeof title !== 'string' || title.length === 0) {
      throw new Error('Title must be a non-empty string');
    }
    this._title = title;
    return this;
  }

  /**
   * Sets the document author
   * @param author - The author name
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.author('Jane Doe');
   * ```
   */
  author(author: string): this {
    if (typeof author !== 'string' || author.length === 0) {
      throw new Error('Author must be a non-empty string');
    }
    this._author = author;
    return this;
  }

  /**
   * Sets the document subject
   * @param subject - The document subject
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.subject('Technical Documentation');
   * ```
   */
  subject(subject: string): this {
    if (typeof subject !== 'string' || subject.length === 0) {
      throw new Error('Subject must be a non-empty string');
    }
    this._subject = subject;
    return this;
  }

  /**
   * Sets the document keywords
   * @param keywords - Array of keywords
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.keywords(['PDF', 'Document', 'Generation']);
   * ```
   */
  keywords(keywords: string[]): this {
    if (!Array.isArray(keywords)) {
      throw new Error('Keywords must be an array');
    }
    if (!keywords.every((k) => typeof k === 'string')) {
      throw new Error('All keywords must be strings');
    }
    this._keywords = keywords;
    return this;
  }

  /**
   * Adds a single keyword to the document
   * @param keyword - A keyword to add
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.addKeyword('Important').addKeyword('Urgent');
   * ```
   */
  addKeyword(keyword: string): this {
    if (typeof keyword !== 'string' || keyword.length === 0) {
      throw new Error('Keyword must be a non-empty string');
    }
    this._keywords.push(keyword);
    return this;
  }

  /**
   * Sets the default page size
   * @param pageSize - Page size name (e.g., 'Letter', 'A4', 'Legal')
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.pageSize('A4');
   * ```
   */
  pageSize(pageSize: string): this {
    const validSizes = ['Letter', 'Legal', 'A4', 'A3', 'A5', 'B4', 'B5'];
    if (!validSizes.includes(pageSize)) {
      throw new Error(`Invalid page size. Must be one of: ${validSizes.join(', ')}`);
    }
    this._pageSize = pageSize;
    return this;
  }

  /**
   * Sets page margins
   * @param top - Top margin in points
   * @param right - Right margin in points
   * @param bottom - Bottom margin in points
   * @param left - Left margin in points
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.margins(36, 36, 36, 36); // 0.5 inches on all sides
   * ```
   */
  margins(top: number, right: number, bottom: number, left: number): this {
    if (![top, right, bottom, left].every((m) => typeof m === 'number' && m >= 0)) {
      throw new Error('All margins must be non-negative numbers');
    }
    this._margins = { top, right, bottom, left };
    return this;
  }

  /**
   * Creates a PDF document from Markdown content
   * @param markdown - Markdown formatted content
   * @returns The created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = builder.fromMarkdown('# Title\n\nContent here');
   * ```
   */
  fromMarkdown(markdown: string): any {
    // Note: Using any for Pdf type to avoid circular dependency
    const { Pdf } = require('../index.js');

    if (typeof markdown !== 'string') {
      throw new Error('Markdown must be a string');
    }

    const pdf = Pdf.fromMarkdown(markdown);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Creates a PDF document from HTML content
   * @param html - HTML formatted content
   * @returns The created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = builder.fromHtml('<h1>Title</h1><p>Content</p>');
   * ```
   */
  fromHtml(html: string): any {
    const { Pdf } = require('../index.js');

    if (typeof html !== 'string') {
      throw new Error('HTML must be a string');
    }

    const pdf = Pdf.fromHtml(html);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Creates a PDF document from plain text content
   * @param text - Plain text content
   * @returns The created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = builder.fromText('Plain text content');
   * ```
   */
  fromText(text: string): any {
    const { Pdf } = require('../index.js');

    if (typeof text !== 'string') {
      throw new Error('Text must be a string');
    }

    const pdf = Pdf.fromText(text);
    this._applyConfiguration(pdf);
    return pdf;
  }

  fromHtmlCss(html: string, css: string, fontBytes: Buffer | Uint8Array): any {
    const { Pdf } = require('../index.js');
    const pdf = Pdf.fromHtmlCss(html, css, fontBytes);
    this._applyConfiguration(pdf);
    return pdf;
  }

  fromHtmlCssWithFonts(
    html: string,
    css: string,
    families: string[],
    fonts: (Buffer | Uint8Array)[]
  ): any {
    const { Pdf } = require('../index.js');
    const pdf = Pdf.fromHtmlCssWithFonts(html, css, families, fonts);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Asynchronously creates a PDF document from Markdown content
   * @param markdown - Markdown formatted content
   * @returns Promise that resolves to the created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = await builder.fromMarkdownAsync('# Title\n\nContent');
   * ```
   */
  async fromMarkdownAsync(markdown: string): Promise<any> {
    const { Pdf } = require('../index.js');

    if (typeof markdown !== 'string') {
      throw new Error('Markdown must be a string');
    }

    const pdf = await Pdf.fromMarkdownAsync(markdown);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Asynchronously creates a PDF document from HTML content
   * @param html - HTML formatted content
   * @returns Promise that resolves to the created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = await builder.fromHtmlAsync('<h1>Title</h1>');
   * ```
   */
  async fromHtmlAsync(html: string): Promise<any> {
    const { Pdf } = require('../index.js');

    if (typeof html !== 'string') {
      throw new Error('HTML must be a string');
    }

    const pdf = await Pdf.fromHtmlAsync(html);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Asynchronously creates a PDF document from plain text content
   * @param text - Plain text content
   * @returns Promise that resolves to the created PDF document
   * @throws PdfError if PDF creation fails
   *
   * @example
   * ```typescript
   * const pdf = await builder.fromTextAsync('Plain text');
   * ```
   */
  async fromTextAsync(text: string): Promise<any> {
    const { Pdf } = require('../index.js');

    if (typeof text !== 'string') {
      throw new Error('Text must be a string');
    }

    const pdf = await Pdf.fromTextAsync(text);
    this._applyConfiguration(pdf);
    return pdf;
  }

  /**
   * Gets the current configuration as a plain object
   * @returns Configuration object with title, author, subject, keywords
   *
   * @private
   */
  private _getConfiguration(): PdfBuilderConfig {
    return {
      title: this._title,
      author: this._author,
      subject: this._subject,
      keywords: this._keywords,
      pageSize: this._pageSize,
      margins: this._margins,
    };
  }

  /**
   * Applies builder configuration to a PDF document
   * @param pdf - The PDF document to configure
   * @private
   */
  private _applyConfiguration(pdf: any): void {
    // Apply metadata properties if set
    if (this._title !== undefined) {
      pdf.title = this._title;
    }
    if (this._author !== undefined) {
      pdf.author = this._author;
    }
    if (this._subject !== undefined) {
      pdf.subject = this._subject;
    }
    if (this._keywords.length > 0) {
      pdf.keywords = [...this._keywords];
    }
  }
}

/**
 * Create a new PdfBuilder with static factory
 * @deprecated Use PdfBuilder.create() instead
 * @returns New builder instance
 */
export function createPdfBuilder(): PdfBuilder {
  return PdfBuilder.create();
}
