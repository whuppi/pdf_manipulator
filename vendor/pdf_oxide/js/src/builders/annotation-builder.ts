/**
 * Builder for creating PDF annotations
 *
 * Configures annotation properties like content, appearance, author, and behavior.
 *
 * @example
 * ```typescript
 * import { AnnotationBuilder } from 'pdf_oxide';
 *
 * const annotation = AnnotationBuilder.create()
 *   .type('highlight')
 *   .content('Important section')
 *   .author('Reviewer')
 *   .color([1, 1, 0]) // Yellow
 *   .build();
 *
 * pdf.addAnnotation(annotation);
 * ```
 */

interface AnnotationBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Annotation {
  type: string;
  content: string;
  author?: string;
  subject?: string;
  color: number[];
  opacity: number;
  bounds?: AnnotationBounds;
  creationDate: Date;
  modificationDate: Date;
  flags: number;
  reply?: string;
}

export class AnnotationBuilder {
  private _type: string = 'text';
  private _content: string = '';
  private _author?: string;
  private _subject?: string;
  private _color: number[] = [1, 0, 0]; // Default: red (RGB normalized 0-1)
  private _opacity: number = 1.0;
  private _bounds?: AnnotationBounds;
  private _creationDate: Date = new Date();
  private _modificationDate: Date = new Date();
  private _flags: number = 0;
  private _reply?: string;

  /**
   * Creates a new AnnotationBuilder instance
   * @private
   */
  private constructor() {}

  /**
   * Creates a new AnnotationBuilder instance
   * @returns New builder instance
   */
  static create(): AnnotationBuilder {
    return new AnnotationBuilder();
  }

  /**
   * Creates a text annotation (comment/note)
   * @returns This builder for chaining
   */
  asText(): this {
    this._type = 'text';
    return this;
  }

  /**
   * Creates a highlight annotation
   * @returns This builder for chaining
   */
  asHighlight(): this {
    this._type = 'highlight';
    return this;
  }

  /**
   * Creates an underline annotation
   * @returns This builder for chaining
   */
  asUnderline(): this {
    this._type = 'underline';
    return this;
  }

  /**
   * Creates a strikeout annotation
   * @returns This builder for chaining
   */
  asStrikeout(): this {
    this._type = 'strikeout';
    return this;
  }

  /**
   * Creates a squiggly (wavy underline) annotation
   * @returns This builder for chaining
   */
  asSquiggly(): this {
    this._type = 'squiggly';
    return this;
  }

  /**
   * Sets the annotation type
   * @param type - Annotation type ('text', 'highlight', 'underline', 'strikeout', 'squiggly')
   * @returns This builder for chaining
   */
  type(type: string): this {
    const validTypes = ['text', 'highlight', 'underline', 'strikeout', 'squiggly', 'note'];
    if (!validTypes.includes(type)) {
      throw new Error(`Invalid annotation type. Must be one of: ${validTypes.join(', ')}`);
    }
    this._type = type;
    return this;
  }

  /**
   * Sets the annotation content/text
   * @param content - The annotation content
   * @returns This builder for chaining
   */
  content(content: string): this {
    if (typeof content !== 'string') {
      throw new Error('Content must be a string');
    }
    this._content = content;
    return this;
  }

  /**
   * Sets the author of the annotation
   * @param author - The author name
   * @returns This builder for chaining
   */
  author(author: string): this {
    if (typeof author !== 'string') {
      throw new Error('Author must be a string');
    }
    this._author = author.length > 0 ? author : undefined;
    return this;
  }

  /**
   * Sets the subject/title of the annotation
   * @param subject - The annotation subject
   * @returns This builder for chaining
   */
  subject(subject: string): this {
    if (typeof subject !== 'string') {
      throw new Error('Subject must be a string');
    }
    this._subject = subject.length > 0 ? subject : undefined;
    return this;
  }

  /**
   * Sets the color of the annotation (RGB, normalized 0-1)
   * @param rgb - RGB color array [r, g, b] with values 0-1
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.color([1, 1, 0]); // Yellow
   * builder.color([1, 0, 0]); // Red
   * builder.color([0, 1, 0]); // Green
   * ```
   */
  color(rgb: number[]): this {
    if (!Array.isArray(rgb) || rgb.length !== 3) {
      throw new Error('Color must be an array of 3 RGB values [r, g, b]');
    }
    if (!rgb.every((c) => typeof c === 'number' && c >= 0 && c <= 1)) {
      throw new Error('RGB values must be numbers between 0 and 1');
    }
    this._color = [...rgb];
    return this;
  }

  /**
   * Sets the color using common color names
   * @param colorName - Color name (e.g., 'red', 'yellow', 'green', 'blue')
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.colorName('yellow');
   * builder.colorName('red');
   * ```
   */
  colorName(colorName: string): this {
    const colors: Record<string, number[]> = {
      red: [1, 0, 0],
      green: [0, 1, 0],
      blue: [0, 0, 1],
      yellow: [1, 1, 0],
      cyan: [0, 1, 1],
      magenta: [1, 0, 1],
      white: [1, 1, 1],
      black: [0, 0, 0],
      gray: [0.5, 0.5, 0.5],
      orange: [1, 0.5, 0],
      purple: [0.5, 0, 0.5],
    };

    const lowerColorName = colorName.toLowerCase();
    if (!colors[lowerColorName]) {
      const available = Object.keys(colors).join(', ');
      throw new Error(`Unknown color. Available colors: ${available}`);
    }

    this._color = [...colors[lowerColorName]];
    return this;
  }

  /**
   * Sets the opacity/transparency (0-1)
   * @param opacity - Opacity value (0=transparent, 1=opaque)
   * @returns This builder for chaining
   */
  opacity(opacity: number): this {
    if (typeof opacity !== 'number' || opacity < 0 || opacity > 1) {
      throw new Error('Opacity must be a number between 0 and 1');
    }
    this._opacity = opacity;
    return this;
  }

  /**
   * Sets the bounding box for the annotation
   * @param bounds - Bounding box {x, y, width, height}
   * @returns This builder for chaining
   *
   * @example
   * ```typescript
   * builder.bounds({x: 100, y: 200, width: 150, height: 30});
   * ```
   */
  bounds(bounds: AnnotationBounds): this {
    if (typeof bounds !== 'object' || bounds === null) {
      throw new Error('Bounds must be an object');
    }
    const { x, y, width, height } = bounds;
    if (![x, y, width, height].every((v) => typeof v === 'number' && v >= 0)) {
      throw new Error('Bounds must have numeric x, y, width, height values >= 0');
    }
    this._bounds = { x, y, width, height };
    return this;
  }

  /**
   * Sets the creation date
   * @param date - The creation date
   * @returns This builder for chaining
   */
  creationDate(date: Date): this {
    if (!(date instanceof Date)) {
      throw new Error('creationDate must be a Date object');
    }
    this._creationDate = new Date(date);
    return this;
  }

  /**
   * Sets the modification date
   * @param date - The modification date
   * @returns This builder for chaining
   */
  modificationDate(date: Date): this {
    if (!(date instanceof Date)) {
      throw new Error('modificationDate must be a Date object');
    }
    this._modificationDate = new Date(date);
    return this;
  }

  /**
   * Sets the annotation to be printed
   * @returns This builder for chaining
   */
  printable(): this {
    this._flags |= 4; // Print flag
    return this;
  }

  /**
   * Sets the annotation to NOT be printed
   * @returns This builder for chaining
   */
  notPrintable(): this {
    this._flags &= ~4; // Clear print flag
    return this;
  }

  /**
   * Sets whether the annotation is locked (read-only)
   * @param locked - Whether to lock the annotation
   * @returns This builder for chaining
   */
  locked(locked: boolean): this {
    if (typeof locked !== 'boolean') {
      throw new Error('locked must be a boolean');
    }
    if (locked) {
      this._flags |= 128; // Locked flag
    } else {
      this._flags &= ~128;
    }
    return this;
  }

  /**
   * Sets a reply to this annotation
   * @param replyContent - Content of the reply
   * @returns This builder for chaining
   */
  reply(replyContent: string): this {
    if (typeof replyContent !== 'string') {
      throw new Error('Reply content must be a string');
    }
    this._reply = replyContent;
    return this;
  }

  /**
   * Builds and returns the annotation object
   * @returns Immutable annotation object
   */
  build(): Annotation {
    if (!this._bounds && this._type !== 'text') {
      throw new Error(`Annotation type "${this._type}" requires bounds to be set`);
    }

    return {
      type: this._type,
      content: this._content,
      author: this._author,
      subject: this._subject,
      color: [...this._color],
      opacity: this._opacity,
      bounds: this._bounds ? { ...this._bounds } : undefined,
      creationDate: new Date(this._creationDate),
      modificationDate: new Date(this._modificationDate),
      flags: this._flags,
      reply: this._reply,
    };
  }
}

/**
 * Create a new AnnotationBuilder with static factory
 * @deprecated Use AnnotationBuilder.create() instead
 * @returns New builder instance
 */
export function createAnnotationBuilder(): AnnotationBuilder {
  return AnnotationBuilder.create();
}
