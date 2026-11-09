/**
 * Builder for document metadata configuration
 *
 * Configures document information like title, author, subject, keywords,
 * creation date, and custom properties.
 *
 * @example
 * ```typescript
 * import { MetadataBuilder } from 'pdf_oxide';
 *
 * const metadata = MetadataBuilder.create()
 *   .title('My Document')
 *   .author('John Doe')
 *   .subject('Important Information')
 *   .keywords(['document', 'important', 'example'])
 *   .creator('MyApp v1.0')
 *   .build();
 *
 * pdf.setMetadata(metadata);
 * ```
 */

export interface Metadata {
  title?: string;
  author?: string;
  subject?: string;
  keywords: string[];
  creator?: string;
  producer: string;
  creationDate: Date;
  modificationDate: Date;
  customProperties: Record<string, string>;
}

export class MetadataBuilder {
  private _title?: string;
  private _author?: string;
  private _subject?: string;
  private _keywords: string[] = [];
  private _creator?: string;
  private _producer: string = 'PDF Oxide';
  private _creationDate: Date = new Date();
  private _modificationDate: Date = new Date();
  private _customProperties: Record<string, string> = {};

  /**
   * Creates a new MetadataBuilder instance
   * @private
   */
  private constructor() {}

  /**
   * Creates a new MetadataBuilder instance
   * @returns New builder instance
   */
  static create(): MetadataBuilder {
    return new MetadataBuilder();
  }

  /**
   * Sets the document title
   * @param title - The document title
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.title('Project Report 2024');
   * ```
   */
  title(title: string): this {
    if (typeof title !== 'string') {
      throw new Error('Title must be a string');
    }
    this._title = title.length > 0 ? title : undefined;
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
    if (typeof author !== 'string') {
      throw new Error('Author must be a string');
    }
    this._author = author.length > 0 ? author : undefined;
    return this;
  }

  /**
   * Sets the document subject
   * @param subject - The document subject
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.subject('Annual Report');
   * ```
   */
  subject(subject: string): this {
    if (typeof subject !== 'string') {
      throw new Error('Subject must be a string');
    }
    this._subject = subject.length > 0 ? subject : undefined;
    return this;
  }

  /**
   * Sets document keywords
   * @param keywords - Array of keywords
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.keywords(['report', 'annual', 'financial']);
   * ```
   */
  keywords(keywords: string[]): this {
    if (!Array.isArray(keywords)) {
      throw new Error('Keywords must be an array');
    }
    if (!keywords.every((k) => typeof k === 'string')) {
      throw new Error('All keywords must be strings');
    }
    this._keywords = [...keywords];
    return this;
  }

  /**
   * Adds a single keyword
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
    if (!this._keywords.includes(keyword)) {
      this._keywords.push(keyword);
    }
    return this;
  }

  /**
   * Sets the creator application name
   * @param creator - Name of the application that created the document
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.creator('MyApp v2.1.0');
   * ```
   */
  creator(creator: string): this {
    if (typeof creator !== 'string') {
      throw new Error('Creator must be a string');
    }
    this._creator = creator.length > 0 ? creator : undefined;
    return this;
  }

  /**
   * Sets the PDF producer (usually the library/tool that saved it)
   * @param producer - Name of the PDF producer
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.producer('PDF Oxide v0.3.2');
   * ```
   */
  producer(producer: string): this {
    if (typeof producer !== 'string') {
      throw new Error('Producer must be a string');
    }
    this._producer = producer.length > 0 ? producer : 'PDF Oxide';
    return this;
  }

  /**
   * Sets the document creation date
   * @param date - The creation date
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.creationDate(new Date('2024-01-15'));
   * ```
   */
  creationDate(date: Date): this {
    if (!(date instanceof Date)) {
      throw new Error('creationDate must be a Date object');
    }
    this._creationDate = new Date(date);
    return this;
  }

  /**
   * Sets the document modification date
   * @param date - The modification date
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.modificationDate(new Date());
   * ```
   */
  modificationDate(date: Date): this {
    if (!(date instanceof Date)) {
      throw new Error('modificationDate must be a Date object');
    }
    this._modificationDate = new Date(date);
    return this;
  }

  /**
   * Sets a custom metadata property
   * @param key - Property key
   * @param value - Property value
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.customProperty('Department', 'Engineering');
   * builder.customProperty('Classification', 'Confidential');
   * ```
   */
  customProperty(key: string, value: string): this {
    if (typeof key !== 'string' || key.length === 0) {
      throw new Error('Property key must be a non-empty string');
    }
    if (typeof value !== 'string') {
      throw new Error('Property value must be a string');
    }
    this._customProperties[key] = value;
    return this;
  }

  /**
   * Sets multiple custom metadata properties
   * @param properties - Object with key-value pairs
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.customProperties({
   *   Department: 'Engineering',
   *   Classification: 'Confidential',
   *   ProjectCode: 'PROJ-2024-001'
   * });
   * ```
   */
  customProperties(properties: Record<string, string>): this {
    if (typeof properties !== 'object' || properties === null) {
      throw new Error('customProperties must be an object');
    }
    for (const [key, value] of Object.entries(properties)) {
      if (typeof value !== 'string') {
        throw new Error(`Custom property "${key}" value must be a string`);
      }
      this._customProperties[key] = value;
    }
    return this;
  }

  /**
   * Builds and returns the metadata object
   * @returns Immutable metadata object
   *
   * @example
   * ```typescript
   * const metadata = builder.build();
   * ```
   */
  build(): Metadata {
    return {
      title: this._title,
      author: this._author,
      subject: this._subject,
      keywords: [...this._keywords],
      creator: this._creator,
      producer: this._producer,
      creationDate: new Date(this._creationDate),
      modificationDate: new Date(this._modificationDate),
      customProperties: { ...this._customProperties },
    };
  }

  /**
   * Creates metadata with current timestamp
   * @returns This builder with current modification date
   */
  withCurrentDate(): this {
    this._modificationDate = new Date();
    return this;
  }
}

/**
 * Create a new MetadataBuilder with static factory
 * @deprecated Use MetadataBuilder.create() instead
 * @returns New builder instance
 */
export function createMetadataBuilder(): MetadataBuilder {
  return MetadataBuilder.create();
}
