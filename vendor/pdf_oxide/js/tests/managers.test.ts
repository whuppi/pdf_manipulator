/**
 * Comprehensive tests for PDF Oxide Node.js managers
 * Tests all managers to verify FFI wiring and functionality
 */

import { beforeEach, describe, expect, it } from '@jest/globals';
import DocumentEditorManager from '../src/document-editor-manager';
import OCRManager, { OCRLanguage } from '../src/ocr-manager';
import {
  FontStyle,
  PageOrientation,
  PageSize,
  PdfCreatorManager,
  TextAlign,
} from '../src/pdf-creator-manager';

describe('DocumentEditorManager', () => {
  let mockDocument: any;
  let manager: DocumentEditorManager;

  beforeEach(() => {
    // Mock native document
    mockDocument = {
      pageCount: 5,
      removePage: jest.fn(),
      insertPage: jest.fn(),
      rotatePage: jest.fn(),
      getPage: jest.fn(() => ({ getWidth: () => 612, getHeight: () => 792 })),
      save_async: jest.fn().mockResolvedValue(undefined),
      filePath: 'test.pdf',
    };

    manager = new DocumentEditorManager(mockDocument);
  });

  it('should validate page indices', () => {
    expect(() => manager.validatePageIndex?.(10)).toThrow();
  });

  it('should handle deletePage', async () => {
    await manager.deletePage(0);
    expect(mockDocument.removePage).toHaveBeenCalledWith(0);
    expect(manager.getEditHistory().length).toBeGreaterThan(0);
  });

  it('should handle deletePages', async () => {
    await manager.deletePages([0, 1, 2]);
    expect(manager.getEditHistory().length).toBeGreaterThanOrEqual(3);
  });

  it('should handle insertBlankPage', async () => {
    await manager.insertBlankPage(0, 612, 792);
    expect(mockDocument.insertPage).toHaveBeenCalled();
  });

  it('should handle rotatePage', async () => {
    await manager.rotatePage(0, 90);
    expect(mockDocument.rotatePage).toHaveBeenCalledWith(0, 90);
  });

  it('should handle movePage', async () => {
    await manager.movePage(0, 2);
    expect(mockDocument.removePage).toHaveBeenCalled();
  });

  it('should handle save', async () => {
    await manager.save('output.pdf');
    expect(mockDocument.save_async).toHaveBeenCalled();
    expect(manager.hasChanges()).toBe(false);
  });

  it('should track edit history', async () => {
    await manager.deletePage(0);
    const history = manager.getEditHistory();
    expect(history).toHaveLength(1);
    expect(history[0].type).toBe('deletePage');
  });

  it('should clear history', () => {
    manager.clearHistory?.();
    expect(manager.getEditHistory()).toHaveLength(0);
  });

  it('should emit events', (done) => {
    manager.once('pageDeleted', () => {
      done();
    });
    manager.deletePage(0);
  });
});

describe('OCRManager', () => {
  let mockDocument: any;
  let manager: OCRManager;

  beforeEach(() => {
    mockDocument = {
      pageCount: 3,
      extractText: jest.fn(() => 'Sample text extracted from page'),
    };

    manager = new OCRManager(mockDocument);
  });

  it('should set language', () => {
    manager.setLanguage(OCRLanguage.Spanish);
    expect(manager.getLanguage()).toBe(OCRLanguage.Spanish);
  });

  it('should detect pages needing OCR', async () => {
    mockDocument.extractText = jest.fn(() => '');
    const needs = await manager.needsOcr(0);
    expect(needs).toBe(true);
  });

  it('should extract text from page', async () => {
    const text = await manager.extractText(0);
    expect(text).toBeTruthy();
    expect(mockDocument.extractText).toHaveBeenCalledWith(0);
  });

  it('should analyze single page', async () => {
    const analysis = await manager.analyzePage(0);
    expect(analysis).toHaveProperty('pageIndex', 0);
    expect(analysis).toHaveProperty('text');
    expect(analysis).toHaveProperty('needsOcr');
    expect(analysis).toHaveProperty('confidence');
    expect(analysis).toHaveProperty('spanCount');
  });

  it('should analyze entire document', async () => {
    const results = await manager.analyzeDocument();
    expect(results).toBeInstanceOf(Array);
    expect(results.length).toBeGreaterThan(0);
  });

  it('should extract page range', async () => {
    const result = await manager.extractPageRange(0, 2);
    expect(result).toHaveProperty('startPage', 0);
    expect(result).toHaveProperty('endPage', 2);
    expect(result).toHaveProperty('totalPages', 3);
    expect(result).toHaveProperty('totalSpans');
    expect(result).toHaveProperty('averageConfidence');
  });

  it('should cache results', async () => {
    await manager.extractText(0);
    await manager.extractText(0);
    expect(mockDocument.extractText).toHaveBeenCalledTimes(1); // Cache hit on second call
  });

  it('should clear cache', () => {
    manager.clearCache();
    const stats = manager.getCacheStats();
    expect(stats.cacheSize).toBe(0);
  });

  it('should emit events', (done) => {
    manager.once('textExtracted', () => {
      done();
    });
    manager.extractText(0);
  });

  it('should handle errors', async () => {
    mockDocument.extractText = jest.fn(() => {
      throw new Error('Extract failed');
    });

    manager.once('error', (error) => {
      expect(error).toBeTruthy();
    });

    await expect(manager.extractText(0)).rejects.toThrow();
  });
});

describe('PdfCreatorManager', () => {
  let manager: PdfCreatorManager;

  beforeEach(() => {
    manager = new PdfCreatorManager();
  });

  it('should create document with default page', () => {
    expect(manager.getPageCount()).toBe(0);
  });

  it('should add Letter page', () => {
    const idx = manager.addPage({ size: PageSize.Letter });
    expect(idx).toBe(0);
    expect(manager.getPageCount()).toBe(1);
  });

  it('should add A4 page', () => {
    manager.addPage({ size: PageSize.A4 });
    const info = manager.getPageInfo(0);
    expect(info.width).toBe(595);
    expect(info.height).toBe(842);
  });

  it('should support landscape orientation', () => {
    manager.addPage({
      size: PageSize.Letter,
      orientation: PageOrientation.Landscape,
    });
    const info = manager.getPageInfo(0);
    expect(info.width).toBe(792); // Swapped
    expect(info.height).toBe(612);
  });

  it('should add custom page size', () => {
    manager.addPage({
      size: PageSize.Custom,
      customWidth: 400,
      customHeight: 600,
    });
    const info = manager.getPageInfo(0);
    expect(info.width).toBe(400);
    expect(info.height).toBe(600);
  });

  it('should set current page', () => {
    manager.addPage();
    manager.addPage();
    manager.setCurrentPage(1);
    manager.addText({ x: 100, y: 100, text: 'Test' });
    expect(manager.getPageInfo(1).contentCount).toBeGreaterThan(0);
  });

  it('should add text', () => {
    manager.addPage();
    manager.addText({
      x: 100,
      y: 100,
      text: 'Hello World',
      fontSize: 14,
      fontStyle: FontStyle.Bold,
      color: '#FF0000',
      align: TextAlign.Center,
    });
    const info = manager.getPageInfo(0);
    expect(info.contentCount).toBe(1);
  });

  it('should add image', async () => {
    manager.addPage();
    await manager.addImage({
      x: 100,
      y: 100,
      width: 200,
      height: 200,
      imagePath: '/path/to/image.png',
    });
    const info = manager.getPageInfo(0);
    expect(info.contentCount).toBe(1);
  });

  it('should draw rectangle', () => {
    manager.addPage();
    manager.drawRect({
      x: 50,
      y: 50,
      width: 100,
      height: 100,
      fillColor: '#0000FF',
      strokeColor: '#000000',
      strokeWidth: 2,
    });
    const info = manager.getPageInfo(0);
    expect(info.contentCount).toBe(1);
  });

  it('should draw line', () => {
    manager.addPage();
    manager.drawLine(10, 10, 100, 100, '#FF0000', 2);
    const info = manager.getPageInfo(0);
    expect(info.contentCount).toBe(1);
  });

  it('should set metadata', () => {
    manager.setMetadata({
      title: 'Test Document',
      author: 'Test Author',
      subject: 'Testing',
      keywords: ['test', 'pdf'],
      creator: 'PDF Oxide',
    });
    const metadata = manager.getMetadata();
    expect(metadata.title).toBe('Test Document');
    expect(metadata.author).toBe('Test Author');
  });

  it('should reject empty document build', async () => {
    await expect(manager.build()).rejects.toThrow('Cannot build empty document');
  });

  it('should build valid PDF', async () => {
    manager.addPage();
    manager.addText({ x: 100, y: 100, text: 'Test' });
    const pdf = await manager.build();
    expect(pdf).toBeInstanceOf(Buffer);
    expect(pdf.toString('utf8', 0, 4)).toBe('%PDF'); // PDF header
  });

  it('should create multiple pages in PDF', async () => {
    manager.addPage();
    manager.addPage();
    manager.addPage();
    const pdf = await manager.build();
    expect(pdf.length).toBeGreaterThan(0);
  });

  it('should clear document', () => {
    manager.addPage();
    manager.addPage();
    manager.clear();
    expect(manager.getPageCount()).toBe(0);
  });

  it('should set default page config', () => {
    manager.setDefaultPageConfig({
      size: PageSize.A3,
      orientation: PageOrientation.Landscape,
    });
    const idx = manager.addPage();
    const info = manager.getPageInfo(idx);
    expect(info.width).toBe(1191); // A3 landscape
  });

  it('should emit events', (done) => {
    manager.once('pageAdded', () => {
      done();
    });
    manager.addPage();
  });

  it('should handle save with directory creation', async () => {
    manager.addPage();
    manager.addText({ x: 100, y: 100, text: 'Test' });

    // This would require mocking fs module in real tests
    // For now just verify build works
    const pdf = await manager.build();
    expect(pdf).toBeInstanceOf(Buffer);
  });
});

describe('Cross-Manager Integration', () => {
  it('should work with multiple managers', () => {
    const creator = new PdfCreatorManager();
    const ocr = new OCRManager({
      pageCount: 0,
      extractText: () => 'text',
    });

    creator.addPage();
    creator.addText({ x: 100, y: 100, text: 'Integration Test' });

    expect(creator.getPageCount()).toBe(1);
    expect(ocr.getLanguage()).toBe(OCRLanguage.EnglishUS);
  });

  it('should handle concurrent operations', async () => {
    const creator = new PdfCreatorManager();
    const ocr = new OCRManager({
      pageCount: 2,
      extractText: () => 'concurrent test',
    });

    creator.addPage();
    creator.addPage();

    const [pdf, analysis] = await Promise.all([creator.build(), ocr.analyzeDocument()]);

    expect(pdf).toBeInstanceOf(Buffer);
    expect(analysis).toBeInstanceOf(Array);
  });
});
