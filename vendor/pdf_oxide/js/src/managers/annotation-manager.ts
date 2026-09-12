/**
 * Manager for PDF page annotations (comments, highlights, etc.)
 *
 * Provides methods to work with annotations on PDF pages including
 * comments, highlights, underlines, and other markup.
 *
 * @example
 * ```typescript
 * import { AnnotationManager } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 * const page = doc.getPage(0);
 * const annotationManager = new AnnotationManager(page);
 *
 * // Get annotations on the page
 * const annotations = annotationManager.getAnnotations();
 * console.log(`Page has ${annotations.length} annotations`);
 * ```
 */

export interface Annotation {
  type: string;
  content?: string;
  author?: string;
  modificationDate?: Date;
  bounds?: {
    x: number;
    y: number;
    width: number;
    height: number;
  };
  opacity?: number;
  [key: string]: any;
}

export interface AnnotationStatistics {
  total: number;
  byType: Record<string, number>;
  byAuthor: Record<string, number>;
  authors: string[];
  types: string[];
  hasComments: boolean;
  hasHighlights: boolean;
  averageOpacity: number;
  recentModifications: number;
}

export interface AnnotationValidation {
  isValid: boolean;
  issues: string[];
}

export class AnnotationManager {
  private _page: any;
  private _annotationCache: Annotation[] | null;
  private _statisticsCache: AnnotationStatistics | null;

  /**
   * Creates a new AnnotationManager for the given page
   * @param page - The PDF page
   * @throws Error if page is null or undefined
   */
  constructor(page: any) {
    if (!page) {
      throw new Error('Page is required');
    }
    this._page = page;
    // Performance optimization: cache annotations to avoid redundant fetches
    this._annotationCache = null;
    this._statisticsCache = null;
  }

  /**
   * Clears the annotation cache
   * Useful when page content might have changed
   */
  clearCache(): void {
    this._annotationCache = null;
    this._statisticsCache = null;
  }

  /**
   * Gets all annotations on the page
   * Performance optimization: caches results to avoid redundant fetches
   * @returns Array of annotations
   *
   * @example
   * ```typescript
   * const annotations = manager.getAnnotations();
   * annotations.forEach(ann => {
   *   console.log(`${ann.type}: ${ann.content}`);
   * });
   * ```
   */
  getAnnotations(): Annotation[] {
    // Performance optimization: return cached annotations if available
    if (this._annotationCache !== null) {
      return this._annotationCache;
    }

    try {
      // This would require native API support for getting annotations
      const annotations: Annotation[] = [];
      // Cache the result
      this._annotationCache = annotations;
      return annotations;
    } catch (error) {
      return [];
    }
  }

  /**
   * Gets annotations by type
   * @param type - Annotation type ('text', 'highlight', 'underline', 'strikeout', 'squiggly')
   * @returns Matching annotations
   *
   * @example
   * ```typescript
   * const highlights = manager.getAnnotationsByType('highlight');
   * console.log(`Page has ${highlights.length} highlights`);
   * ```
   */
  getAnnotationsByType(type: string): Annotation[] {
    if (!type || typeof type !== 'string') {
      throw new Error('Type must be a non-empty string');
    }

    const validTypes = ['text', 'highlight', 'underline', 'strikeout', 'squiggly', 'link', 'ink'];
    if (!validTypes.includes(type.toLowerCase())) {
      throw new Error(`Invalid annotation type: ${type}`);
    }

    const annotations = this.getAnnotations();
    return annotations.filter((ann) => ann.type === type.toLowerCase());
  }

  /**
   * Gets number of annotations on page
   * @returns Annotation count
   */
  getAnnotationCount(): number {
    return this.getAnnotations().length;
  }

  /**
   * Gets annotations by author
   * @param author - Author name to filter
   * @returns Annotations by specified author
   *
   * @example
   * ```typescript
   * const johnDoeAnnotations = manager.getAnnotationsByAuthor('John Doe');
   * ```
   */
  getAnnotationsByAuthor(author: string): Annotation[] {
    if (!author || typeof author !== 'string') {
      throw new Error('Author must be a non-empty string');
    }

    const annotations = this.getAnnotations();
    return annotations.filter((ann) => ann.author === author);
  }

  /**
   * Gets unique authors of annotations
   * @returns Array of author names
   */
  getAnnotationAuthors(): string[] {
    const annotations = this.getAnnotations();
    const authors = new Set<string>();

    annotations.forEach((ann) => {
      if (ann.author) {
        authors.add(ann.author);
      }
    });

    return Array.from(authors).sort();
  }

  /**
   * Gets annotations modified after a date
   * @param date - Filter date
   * @returns Annotations modified after date
   *
   * @example
   * ```typescript
   * const recentAnnotations = manager.getAnnotationsAfter(new Date('2024-01-01'));
   * ```
   */
  getAnnotationsAfter(date: Date): Annotation[] {
    if (!(date instanceof Date)) {
      throw new Error('Date must be a Date object');
    }

    const annotations = this.getAnnotations();
    return annotations.filter(
      (ann) => ann.modificationDate && new Date(ann.modificationDate) > date
    );
  }

  /**
   * Gets annotations modified before a date
   * @param date - Filter date
   * @returns Annotations modified before date
   */
  getAnnotationsBefore(date: Date): Annotation[] {
    if (!(date instanceof Date)) {
      throw new Error('Date must be a Date object');
    }

    const annotations = this.getAnnotations();
    return annotations.filter(
      (ann) => ann.modificationDate && new Date(ann.modificationDate) < date
    );
  }

  /**
   * Gets annotations with specific content
   * @param contentFragment - Text fragment to search for
   * @returns Matching annotations
   *
   * @example
   * ```typescript
   * const reviewComments = manager.getAnnotationsWithContent('review');
   * ```
   */
  getAnnotationsWithContent(contentFragment: string): Annotation[] {
    if (!contentFragment || typeof contentFragment !== 'string') {
      throw new Error('Content fragment must be a non-empty string');
    }

    const annotations = this.getAnnotations();
    const fragment = contentFragment.toLowerCase();

    return annotations.filter((ann) => ann.content && ann.content.toLowerCase().includes(fragment));
  }

  /**
   * Gets highlights (most common annotation type)
   * @returns Array of highlight annotations
   */
  getHighlights(): Annotation[] {
    return this.getAnnotationsByType('highlight');
  }

  /**
   * Gets text comments/notes
   * @returns Array of text annotations
   */
  getComments(): Annotation[] {
    return this.getAnnotationsByType('text');
  }

  /**
   * Gets underlines
   * @returns Array of underline annotations
   */
  getUnderlines(): Annotation[] {
    return this.getAnnotationsByType('underline');
  }

  /**
   * Gets strikeouts
   * @returns Array of strikeout annotations
   */
  getStrikeouts(): Annotation[] {
    return this.getAnnotationsByType('strikeout');
  }

  /**
   * Gets squiggly underlines
   * @returns Array of squiggly annotations
   */
  getSquigglies(): Annotation[] {
    return this.getAnnotationsByType('squiggly');
  }

  /**
   * Gets annotations statistics
   * Performance optimization: caches computed statistics
   * @returns Statistics about annotations
   *
   * @example
   * ```typescript
   * const stats = manager.getAnnotationStatistics();
   * console.log(`Total: ${stats.total}, Highlights: ${stats.byType.highlight}`);
   * ```
   */
  getAnnotationStatistics(): AnnotationStatistics {
    // Performance optimization: return cached statistics if available
    if (this._statisticsCache !== null) {
      return this._statisticsCache;
    }

    const annotations = this.getAnnotations();
    const byType: Record<string, number> = {};
    const byAuthor: Record<string, number> = {};

    annotations.forEach((ann) => {
      // Count by type
      byType[ann.type] = (byType[ann.type] || 0) + 1;

      // Count by author
      if (ann.author) {
        byAuthor[ann.author] = (byAuthor[ann.author] || 0) + 1;
      }
    });

    const stats: AnnotationStatistics = {
      total: annotations.length,
      byType,
      byAuthor,
      authors: Object.keys(byAuthor),
      types: Object.keys(byType),
      hasComments: annotations.some((ann) => ann.type === 'text'),
      hasHighlights: annotations.some((ann) => ann.type === 'highlight'),
      averageOpacity: this.getAverageOpacity(),
      recentModifications: this.getRecentAnnotations(7).length,
    };

    // Cache the results
    this._statisticsCache = stats;
    return stats;
  }

  /**
   * Gets average opacity of annotations
   * @returns Average opacity value (0-1)
   * @private
   */
  private getAverageOpacity(): number {
    const annotations = this.getAnnotations();
    if (annotations.length === 0) return 1;

    const sum = annotations.reduce((acc, ann) => acc + (ann.opacity || 1), 0);
    return sum / annotations.length;
  }

  /**
   * Gets annotations modified within last N days
   * @param days - Number of days to look back
   * @returns Recent annotations
   *
   * @example
   * ```typescript
   * const lastWeek = manager.getRecentAnnotations(7);
   * console.log(`${lastWeek.length} annotations modified in last 7 days`);
   * ```
   */
  getRecentAnnotations(days: number): Annotation[] {
    if (typeof days !== 'number' || days < 0) {
      throw new Error('Days must be a non-negative number');
    }

    const cutoffDate = new Date();
    cutoffDate.setDate(cutoffDate.getDate() - days);

    return this.getAnnotationsAfter(cutoffDate);
  }

  /**
   * Generates annotation summary
   * @returns Human-readable annotation summary
   *
   * @example
   * ```typescript
   * console.log(manager.generateAnnotationSummary());
   * ```
   */
  generateAnnotationSummary(): string {
    const stats = this.getAnnotationStatistics();
    const lines: string[] = [];

    lines.push(`Annotation Summary (Page ${(this._page as any).pageIndex + 1}):`);
    lines.push(`Total Annotations: ${stats.total}`);

    if (stats.total > 0) {
      lines.push('\nBy Type:');
      Object.entries(stats.byType).forEach(([type, count]) => {
        lines.push(`  ${type}: ${count}`);
      });

      if (stats.authors.length > 0) {
        lines.push('\nBy Author:');
        Object.entries(stats.byAuthor).forEach(([author, count]) => {
          lines.push(`  ${author}: ${count}`);
        });
      }
    }

    return lines.join('\n');
  }

  /**
   * Validates annotation bounds
   * @param annotation - Annotation to validate
   * @returns Validation result
   *
   * @example
   * ```typescript
   * const annotation = manager.getAnnotations()[0];
   * const validation = manager.validateAnnotation(annotation);
   * if (!validation.isValid) {
   *   console.log('Invalid:', validation.issues);
   * }
   * ```
   */
  validateAnnotation(annotation: any): AnnotationValidation {
    const issues: string[] = [];

    if (!annotation) {
      issues.push('Annotation is null or undefined');
    } else {
      if (!annotation.type) issues.push('Missing annotation type');
      if (!['text', 'highlight', 'underline', 'strikeout', 'squiggly'].includes(annotation.type)) {
        issues.push(`Unknown annotation type: ${annotation.type}`);
      }

      if (annotation.bounds) {
        if (annotation.bounds.x < 0) issues.push('Invalid bounds: x must be non-negative');
        if (annotation.bounds.y < 0) issues.push('Invalid bounds: y must be non-negative');
        if (annotation.bounds.width < 0) issues.push('Invalid bounds: width must be non-negative');
        if (annotation.bounds.height < 0)
          issues.push('Invalid bounds: height must be non-negative');
      }

      if (annotation.opacity && (annotation.opacity < 0 || annotation.opacity > 1)) {
        issues.push('Invalid opacity: must be between 0 and 1');
      }
    }

    return {
      isValid: issues.length === 0,
      issues,
    };
  }
}
