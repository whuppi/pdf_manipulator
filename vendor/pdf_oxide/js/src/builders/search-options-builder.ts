/**
 * Builder for text search options
 *
 * Configures how text search is performed in PDF documents,
 * including case sensitivity, whole word matching, and regex support.
 *
 * @example
 * ```typescript
 * import { SearchOptionsBuilder } from 'pdf_oxide';
 *
 * const options = SearchOptionsBuilder.create()
 *   .caseSensitive(false)
 *   .wholeWords(true)
 *   .useRegex(false)
 *   .build();
 *
 * const results = doc.search('pattern', 0, options);
 * ```
 */

export interface SearchOptions {
  caseSensitive: boolean;
  wholeWords: boolean;
  useRegex: boolean;
  ignoreAccents: boolean;
  maxResults: number;
  searchAnnotations: boolean;
}

export class SearchOptionsBuilder {
  private _caseSensitive: boolean = false;
  private _wholeWords: boolean = false;
  private _useRegex: boolean = false;
  private _ignoreAccents: boolean = false;
  private _maxResults: number = 1000;
  private _searchAnnotations: boolean = false;

  /**
   * Creates a new SearchOptionsBuilder instance
   * @private
   */
  private constructor() {}

  /**
   * Creates a new SearchOptionsBuilder instance
   * @returns New builder instance
   */
  static create(): SearchOptionsBuilder {
    return new SearchOptionsBuilder();
  }

  /**
   * Creates options with default search settings (case-insensitive, no regex)
   * @returns Search options with default preset
   */
  static default(): SearchOptions {
    return SearchOptionsBuilder.create().build();
  }

  /**
   * Creates options with strict search settings (case-sensitive, whole words, no regex)
   * @returns Search options with strict preset
   */
  static strict(): SearchOptions {
    return SearchOptionsBuilder.create()
      .caseSensitive(true)
      .wholeWords(true)
      .useRegex(false)
      .build();
  }

  /**
   * Creates options with regex search settings
   * @returns Search options with regex preset
   */
  static regex(): SearchOptions {
    return SearchOptionsBuilder.create()
      .useRegex(true)
      .caseSensitive(false)
      .wholeWords(false)
      .build();
  }

  caseSensitive(sensitive: boolean): this {
    if (typeof sensitive !== 'boolean') {
      throw new Error('caseSensitive must be a boolean');
    }
    this._caseSensitive = sensitive;
    return this;
  }

  wholeWords(wholeOnly: boolean): this {
    if (typeof wholeOnly !== 'boolean') {
      throw new Error('wholeWords must be a boolean');
    }
    this._wholeWords = wholeOnly;
    return this;
  }

  useRegex(regex: boolean): this {
    if (typeof regex !== 'boolean') {
      throw new Error('useRegex must be a boolean');
    }
    this._useRegex = regex;
    return this;
  }

  ignoreAccents(ignore: boolean): this {
    if (typeof ignore !== 'boolean') {
      throw new Error('ignoreAccents must be a boolean');
    }
    this._ignoreAccents = ignore;
    return this;
  }

  maxResults(max: number): this {
    if (typeof max !== 'number' || max <= 0) {
      throw new Error('maxResults must be a positive number');
    }
    this._maxResults = Math.floor(max);
    return this;
  }

  searchAnnotations(search: boolean): this {
    if (typeof search !== 'boolean') {
      throw new Error('searchAnnotations must be a boolean');
    }
    this._searchAnnotations = search;
    return this;
  }

  build(): SearchOptions {
    return {
      caseSensitive: this._caseSensitive,
      wholeWords: this._wholeWords,
      useRegex: this._useRegex,
      ignoreAccents: this._ignoreAccents,
      maxResults: this._maxResults,
      searchAnnotations: this._searchAnnotations,
    };
  }
}

/**
 * Create a new SearchOptionsBuilder with static factory
 * @deprecated Use SearchOptionsBuilder.create() instead
 * @returns New builder instance
 */
export function createSearchOptionsBuilder(): SearchOptionsBuilder {
  return SearchOptionsBuilder.create();
}
