/**
 * DocumentEditorManager for PDF document editing operations
 *
 * Provides methods to edit and modify PDF documents including page operations,
 * content manipulation, and document merging. API is consistent with Python,
 * Java, C#, Go, and Swift implementations.
 */

import { EventEmitter } from 'events';

/**
 * Page rotation options
 */
export enum PageRotation {
  None = 0,
  Rotate90 = 90,
  Rotate180 = 180,
  Rotate270 = 270,
}

/**
 * Configuration for page insertion
 */
export interface InsertConfig {
  pageIndex: number;
  width?: number;
  height?: number;
  content?: Buffer;
}

/**
 * Configuration for page extraction
 */
export interface ExtractConfig {
  startPage: number;
  endPage: number;
  outputPath?: string;
}

/**
 * Configuration for document merging
 */
export interface MergeConfig {
  documents: any[];
  outputPath?: string;
  preserveOutlines?: boolean;
  preserveAnnotations?: boolean;
}

/**
 * Edit operation record
 */
export interface EditOperation {
  type: string;
  timestamp: Date;
  details: Record<string, any>;
}

/**
 * Document Editor Manager for document manipulation
 *
 * Provides methods to:
 * - Delete, insert, and reorder pages
 * - Rotate pages
 * - Extract page ranges to new documents
 * - Merge multiple documents
 * - Track edit history
 */
export class DocumentEditorManager extends EventEmitter {
  private document: any;
  private editHistory: EditOperation[] = [];
  private hasUnsavedChanges = false;

  constructor(document: any) {
    super();
    if (!document) {
      throw new Error('Document is required');
    }
    this.document = document;
  }

  /**
   * Deletes a page from the document
   * Matches: Python deletePage(), Java deletePage(), C# DeletePage()
   */
  async deletePage(pageIndex: number): Promise<void> {
    this.validatePageIndex(pageIndex);

    try {
      if (this.document?.removePage) {
        this.document.removePage(pageIndex);
      }
      this.recordOperation('deletePage', { pageIndex });
      this.emit('pageDeleted', pageIndex);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Deletes multiple pages from the document
   * Matches: Python deletePages(), Java deletePages(), C# DeletePages()
   */
  async deletePages(pageIndices: number[]): Promise<void> {
    // Sort in descending order to avoid index shifting issues
    const sorted = [...pageIndices].sort((a, b) => b - a);

    for (const pageIndex of sorted) {
      await this.deletePage(pageIndex);
    }
  }

  /**
   * Inserts a blank page at the specified position
   * Matches: Python insertBlankPage(), Java insertBlankPage(), C# InsertBlankPage()
   */
  async insertBlankPage(pageIndex: number, width = 612, height = 792): Promise<void> {
    try {
      if (this.document?.insertPage) {
        this.document.insertPage(pageIndex, width, height);
      }
      this.recordOperation('insertBlankPage', { pageIndex, width, height });
      this.emit('pageInserted', pageIndex);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Rotates a page by the specified angle
   * Matches: Python rotatePage(), Java rotatePage(), C# RotatePage()
   */
  async rotatePage(pageIndex: number, rotation: PageRotation): Promise<void> {
    this.validatePageIndex(pageIndex);

    try {
      if (this.document?.rotatePage) {
        this.document.rotatePage(pageIndex, rotation);
      }
      this.recordOperation('rotatePage', { pageIndex, rotation });
      this.emit('pageRotated', pageIndex, rotation);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Rotates multiple pages
   * Matches: Python rotatePages(), Java rotatePages(), C# RotatePages()
   */
  async rotatePages(pageIndices: number[], rotation: PageRotation): Promise<void> {
    for (const pageIndex of pageIndices) {
      await this.rotatePage(pageIndex, rotation);
    }
  }

  /**
   * Moves a page to a new position
   * Matches: Python movePage(), Java movePage(), C# MovePage()
   */
  async movePage(fromIndex: number, toIndex: number): Promise<void> {
    this.validatePageIndex(fromIndex);
    this.validatePageIndex(toIndex);

    try {
      // Implement move by extracting and re-inserting
      if (this.document?.getPage && this.document?.removePage && this.document?.insertPage) {
        const page = this.document.getPage(fromIndex);
        this.document.removePage(fromIndex);
        // Note: After removal, toIndex may need adjustment
        const adjustedIndex = toIndex > fromIndex ? toIndex - 1 : toIndex;
        if (page) {
          this.document.insertPage(adjustedIndex, page.getWidth?.(), page.getHeight?.());
        }
      }
      this.recordOperation('movePage', { fromIndex, toIndex });
      this.emit('pageMoved', fromIndex, toIndex);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Extracts pages to a new document
   * Matches: Python extractPages(), Java extractPages(), C# ExtractPages()
   */
  async extractPages(config: ExtractConfig): Promise<any> {
    const { startPage, endPage } = config;
    this.validatePageIndex(startPage);
    this.validatePageIndex(endPage);

    // FFI call placeholder
    // const newDoc = await FFI.extractPages(this.document.handle, startPage, endPage);

    this.recordOperation('extractPages', { startPage, endPage });
    this.emit('pagesExtracted', startPage, endPage);

    return null; // Placeholder - would return new document
  }

  /**
   * Merges multiple documents into one
   * Matches: Python mergeDocuments(), Java mergeDocuments(), C# MergeDocuments()
   */
  static async mergeDocuments(config: MergeConfig): Promise<any> {
    const { documents, preserveOutlines = true, preserveAnnotations = true } = config;

    if (!documents || documents.length === 0) {
      throw new Error('At least one document is required for merging');
    }

    // FFI call placeholder
    // const merged = await FFI.mergeDocuments(documents, preserveOutlines, preserveAnnotations);

    return null; // Placeholder - would return merged document
  }

  /**
   * Saves the document with all modifications
   * Matches: Python save(), Java save(), C# Save()
   */
  async save(outputPath?: string): Promise<void> {
    try {
      if (outputPath) {
        // Save to file path
        if (this.document?.save_async) {
          await this.document.save_async(outputPath);
        } else if (this.document?.saveToPath) {
          this.document.saveToPath(outputPath);
        }
      } else {
        // Save changes to current document
        if (this.document?.save_async) {
          await this.document.save_async(this.document.filePath || 'output.pdf');
        }
      }
      this.hasUnsavedChanges = false;
      this.emit('saved', outputPath);
    } catch (error) {
      this.emit('error', error);
      throw error;
    }
  }

  /**
   * Gets the edit history
   * Matches: Python getEditHistory(), Java getEditHistory(), C# GetEditHistory()
   */
  getEditHistory(): EditOperation[] {
    return [...this.editHistory];
  }

  /**
   * Checks if there are unsaved changes
   * Matches: Python hasUnsavedChanges(), Java hasUnsavedChanges(), C# HasUnsavedChanges()
   */
  hasChanges(): boolean {
    return this.hasUnsavedChanges;
  }

  /**
   * Undoes the last operation (if supported)
   * Matches: Python undo(), Java undo(), C# Undo()
   */
  async undo(): Promise<boolean> {
    if (this.editHistory.length === 0) {
      return false;
    }

    // Placeholder - would implement undo logic
    this.editHistory.pop();
    this.emit('undone');
    return true;
  }

  /**
   * Clears the edit history
   * Matches: Python clearHistory(), Java clearHistory(), C# ClearHistory()
   */
  clearHistory(): void {
    this.editHistory = [];
    this.emit('historyCleared');
  }

  /**
   * Gets the page count after modifications
   */
  getPageCount(): number {
    try {
      return this.document.pageCount || 0;
    } catch {
      return 0;
    }
  }

  // Private helper methods
  private validatePageIndex(pageIndex: number): void {
    const count = this.getPageCount();
    if (pageIndex < 0 || pageIndex >= count) {
      throw new Error(`Invalid page index: ${pageIndex}. Document has ${count} pages.`);
    }
  }

  private recordOperation(type: string, details: Record<string, any>): void {
    this.editHistory.push({
      type,
      timestamp: new Date(),
      details,
    });
    this.hasUnsavedChanges = true;
  }
}

export default DocumentEditorManager;
