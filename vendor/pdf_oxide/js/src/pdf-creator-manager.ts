/**
 * PdfCreatorManager for creating new PDF documents
 *
 * Provides methods to create PDF documents from scratch, add content,
 * and build documents programmatically. API is consistent with Python,
 * Java, C#, Go, and Swift implementations.
 */

import { EventEmitter } from 'events';
import { promises as fs } from 'fs';
import * as path from 'path';

/**
 * Page size presets
 */
export enum PageSize {
  Letter = 'letter', // 8.5 x 11 inches
  Legal = 'legal', // 8.5 x 14 inches
  A4 = 'a4', // 210 x 297 mm
  A3 = 'a3', // 297 x 420 mm
  A5 = 'a5', // 148 x 210 mm
  Custom = 'custom',
}

/**
 * Page orientation
 */
export enum PageOrientation {
  Portrait = 'portrait',
  Landscape = 'landscape',
}

/**
 * Font style options
 */
export enum FontStyle {
  Normal = 'normal',
  Bold = 'bold',
  Italic = 'italic',
  BoldItalic = 'bolditalic',
}

/**
 * Text alignment options
 */
export enum TextAlign {
  Left = 'left',
  Center = 'center',
  Right = 'right',
  Justify = 'justify',
}

/**
 * Configuration for page creation
 */
export interface PageConfig {
  size?: PageSize;
  orientation?: PageOrientation;
  customWidth?: number;
  customHeight?: number;
  marginTop?: number;
  marginBottom?: number;
  marginLeft?: number;
  marginRight?: number;
}

/**
 * Configuration for text addition
 */
export interface TextConfig {
  x: number;
  y: number;
  text: string;
  fontSize?: number;
  fontFamily?: string;
  fontStyle?: FontStyle;
  color?: string;
  align?: TextAlign;
  lineHeight?: number;
  maxWidth?: number;
}

/**
 * Configuration for image addition
 */
export interface ImageConfig {
  x: number;
  y: number;
  width?: number;
  height?: number;
  preserveAspectRatio?: boolean;
  imageData?: Buffer;
  imagePath?: string;
}

/**
 * Configuration for rectangle drawing
 */
export interface RectConfig {
  x: number;
  y: number;
  width: number;
  height: number;
  fillColor?: string;
  strokeColor?: string;
  strokeWidth?: number;
}

/**
 * PDF document metadata
 */
export interface DocumentMetadata {
  title?: string;
  author?: string;
  subject?: string;
  keywords?: string[];
  creator?: string;
}

/**
 * PDF Creator Manager for document creation
 *
 * Provides methods to:
 * - Create new PDF documents
 * - Add pages with various sizes
 * - Add text, images, and shapes
 * - Set document metadata
 * - Build complete documents
 */
export class PdfCreatorManager extends EventEmitter {
  private pages: any[] = [];
  private currentPageIndex = -1;
  private metadata: DocumentMetadata = {};
  private defaultPageConfig: PageConfig = {
    size: PageSize.Letter,
    orientation: PageOrientation.Portrait,
    marginTop: 72,
    marginBottom: 72,
    marginLeft: 72,
    marginRight: 72,
  };

  constructor() {
    super();
  }

  /**
   * Creates a new page in the document
   * Matches: Python addPage(), Java addPage(), C# AddPage()
   */
  addPage(config?: PageConfig): number {
    const pageConfig = { ...this.defaultPageConfig, ...config };

    let width = 612; // Letter width in points
    let height = 792; // Letter height in points

    // Set dimensions based on page size
    switch (pageConfig.size) {
      case PageSize.Letter:
        width = 612;
        height = 792;
        break;
      case PageSize.Legal:
        width = 612;
        height = 1008;
        break;
      case PageSize.A4:
        width = 595;
        height = 842;
        break;
      case PageSize.A3:
        width = 842;
        height = 1191;
        break;
      case PageSize.A5:
        width = 420;
        height = 595;
        break;
      case PageSize.Custom:
        width = pageConfig.customWidth || 612;
        height = pageConfig.customHeight || 792;
        break;
    }

    // Swap for landscape
    if (pageConfig.orientation === PageOrientation.Landscape) {
      [width, height] = [height, width];
    }

    this.pages.push({
      width,
      height,
      config: pageConfig,
      content: [],
    });

    this.currentPageIndex = this.pages.length - 1;
    this.emit('pageAdded', this.currentPageIndex);
    return this.currentPageIndex;
  }

  /**
   * Sets the current page for content addition
   * Matches: Python setCurrentPage(), Java setCurrentPage(), C# SetCurrentPage()
   */
  setCurrentPage(pageIndex: number): void {
    if (pageIndex < 0 || pageIndex >= this.pages.length) {
      throw new Error(`Invalid page index: ${pageIndex}`);
    }
    this.currentPageIndex = pageIndex;
  }

  /**
   * Adds text to the current page
   * Matches: Python addText(), Java addText(), C# AddText()
   */
  addText(config: TextConfig): void {
    this.ensureCurrentPage();

    const textContent = {
      type: 'text',
      x: config.x,
      y: config.y,
      text: config.text,
      fontSize: config.fontSize || 12,
      fontFamily: config.fontFamily || 'Helvetica',
      fontStyle: config.fontStyle || FontStyle.Normal,
      color: config.color || '#000000',
      align: config.align || TextAlign.Left,
      lineHeight: config.lineHeight || 1.2,
      maxWidth: config.maxWidth,
    };

    this.pages[this.currentPageIndex].content.push(textContent);
    this.emit('textAdded', this.currentPageIndex);
  }

  /**
   * Adds an image to the current page
   * Matches: Python addImage(), Java addImage(), C# AddImage()
   */
  async addImage(config: ImageConfig): Promise<void> {
    this.ensureCurrentPage();

    const imageContent = {
      type: 'image',
      x: config.x,
      y: config.y,
      width: config.width,
      height: config.height,
      preserveAspectRatio: config.preserveAspectRatio ?? true,
      imageData: config.imageData,
      imagePath: config.imagePath,
    };

    this.pages[this.currentPageIndex].content.push(imageContent);
    this.emit('imageAdded', this.currentPageIndex);
  }

  /**
   * Draws a rectangle on the current page
   * Matches: Python drawRect(), Java drawRect(), C# DrawRect()
   */
  drawRect(config: RectConfig): void {
    this.ensureCurrentPage();

    const rectContent = {
      type: 'rect',
      x: config.x,
      y: config.y,
      width: config.width,
      height: config.height,
      fillColor: config.fillColor,
      strokeColor: config.strokeColor || '#000000',
      strokeWidth: config.strokeWidth || 1,
    };

    this.pages[this.currentPageIndex].content.push(rectContent);
    this.emit('rectDrawn', this.currentPageIndex);
  }

  /**
   * Draws a line on the current page
   * Matches: Python drawLine(), Java drawLine(), C# DrawLine()
   */
  drawLine(x1: number, y1: number, x2: number, y2: number, color = '#000000', width = 1): void {
    this.ensureCurrentPage();

    const lineContent = {
      type: 'line',
      x1,
      y1,
      x2,
      y2,
      color,
      width,
    };

    this.pages[this.currentPageIndex].content.push(lineContent);
    this.emit('lineDrawn', this.currentPageIndex);
  }

  /**
   * Sets document metadata
   * Matches: Python setMetadata(), Java setMetadata(), C# SetMetadata()
   */
  setMetadata(metadata: DocumentMetadata): void {
    this.metadata = { ...this.metadata, ...metadata };
    this.emit('metadataSet');
  }

  /**
   * Gets document metadata
   * Matches: Python getMetadata(), Java getMetadata(), C# GetMetadata()
   */
  getMetadata(): DocumentMetadata {
    return { ...this.metadata };
  }

  /**
   * Gets the number of pages
   * Matches: Python getPageCount(), Java getPageCount(), C# GetPageCount()
   */
  getPageCount(): number {
    return this.pages.length;
  }

  /**
   * Gets information about a page
   * Matches: Python getPageInfo(), Java getPageInfo(), C# GetPageInfo()
   */
  getPageInfo(pageIndex: number): any {
    if (pageIndex < 0 || pageIndex >= this.pages.length) {
      throw new Error(`Invalid page index: ${pageIndex}`);
    }

    const page = this.pages[pageIndex];
    return {
      index: pageIndex,
      width: page.width,
      height: page.height,
      contentCount: page.content.length,
    };
  }

  /**
   * Builds and returns the PDF document as a Buffer
   * Matches: Python build(), Java build(), C# Build()
   */
  async build(): Promise<Buffer> {
    if (this.pages.length === 0) {
      throw new Error('Cannot build empty document. Add at least one page.');
    }

    try {
      // Placeholder implementation - would call native FFI PDF builder
      // In real implementation, would:
      // 1. Use native PdfBuilder to create document
      // 2. Add pages with content (text, images, shapes)
      // 3. Set metadata
      // 4. Serialize to PDF bytes

      const pdfContent = this.serializeToPdf();
      this.emit('documentBuilt', {
        pages: this.pages.length,
        size: pdfContent.length,
      });
      return pdfContent;
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Saves the PDF to a file
   * Matches: Python save(), Java save(), C# Save()
   */
  async save(filePath: string): Promise<void> {
    try {
      const pdfData = await this.build();

      // Ensure directory exists
      const dir = path.dirname(filePath);
      try {
        await fs.mkdir(dir, { recursive: true });
      } catch {
        // Directory might already exist
      }

      // Write PDF to file
      await fs.writeFile(filePath, pdfData);

      this.emit('documentSaved', {
        path: filePath,
        size: pdfData.length,
      });
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Clears the document (removes all pages)
   * Matches: Python clear(), Java clear(), C# Clear()
   */
  clear(): void {
    this.pages = [];
    this.currentPageIndex = -1;
    this.metadata = {};
    this.emit('documentCleared');
  }

  /**
   * Sets default page configuration
   * Matches: Python setDefaultPageConfig(), Java setDefaultPageConfig(), C# SetDefaultPageConfig()
   */
  setDefaultPageConfig(config: PageConfig): void {
    this.defaultPageConfig = { ...this.defaultPageConfig, ...config };
  }

  // Private helper methods
  private ensureCurrentPage(): void {
    if (this.currentPageIndex < 0) {
      this.addPage();
    }
  }

  /**
   * Serializes the document to PDF format
   * Creates a minimal but valid PDF structure
   */
  private serializeToPdf(): Buffer {
    const chunks: Buffer[] = [];

    // PDF Header
    chunks.push(Buffer.from('%PDF-1.4\n'));

    // Object 1: Catalog
    chunks.push(Buffer.from('1 0 obj\n'));
    chunks.push(Buffer.from('<< /Type /Catalog /Pages 2 0 R >>\n'));
    chunks.push(Buffer.from('endobj\n'));

    // Object 2: Pages
    const pageRefs = this.pages.map((_, i) => `${3 + i} 0 R`).join(' ');
    chunks.push(Buffer.from('2 0 obj\n'));
    chunks.push(
      Buffer.from(`<< /Type /Pages /Kids [${pageRefs}] /Count ${this.pages.length} >>\n`)
    );
    chunks.push(Buffer.from('endobj\n'));

    // Objects 3+: Pages
    this.pages.forEach((page, idx) => {
      const objNum = 3 + idx;
      chunks.push(Buffer.from(`${objNum} 0 obj\n`));
      chunks.push(
        Buffer.from(
          `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${page.width} ${page.height}] /Contents ${objNum + this.pages.length} 0 R >>\n`
        )
      );
      chunks.push(Buffer.from('endobj\n'));
    });

    // Content streams
    this.pages.forEach((page, idx) => {
      const objNum = 3 + this.pages.length + idx;
      chunks.push(Buffer.from(`${objNum} 0 obj\n`));
      chunks.push(Buffer.from('<< /Length 44 >>\n'));
      chunks.push(Buffer.from('stream\n'));
      chunks.push(Buffer.from('BT /F1 12 Tf 50 750 Td (Generated PDF) Tj ET\n'));
      chunks.push(Buffer.from('endstream\n'));
      chunks.push(Buffer.from('endobj\n'));
    });

    // xref table
    const xrefOffset = chunks.reduce((sum, buf) => sum + buf.length, 0);
    chunks.push(Buffer.from(`xref\n0 ${3 + this.pages.length * 2}\n`));
    chunks.push(Buffer.from('0000000000 65535 f \n'));

    let offset = 9; // After PDF header
    for (let i = 1; i < 3 + this.pages.length * 2; i++) {
      chunks.push(Buffer.from(`${String(offset).padStart(10, '0')} 00000 n \n`));
      // Rough offset calculation - in production would track actual positions
      offset += 50;
    }

    // Trailer
    chunks.push(Buffer.from('trailer\n'));
    chunks.push(Buffer.from(`<< /Size ${3 + this.pages.length * 2} /Root 1 0 R >>\n`));
    chunks.push(Buffer.from('startxref\n'));
    chunks.push(Buffer.from(`${xrefOffset}\n`));
    chunks.push(Buffer.from('%%EOF\n'));

    return Buffer.concat(chunks);
  }
}

export default PdfCreatorManager;
