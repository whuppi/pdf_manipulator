/**
 * Comprehensive test suite for OCRManager in Node.js/TypeScript
 *
 * Tests validate:
 * - OCR engine lifecycle management
 * - Page recognition and text extraction
 * - Bounding box extraction with confidence scores
 * - Result caching behavior
 * - Multi-page processing
 * - Error handling and validation
 * - Resource cleanup
 */

const { PdfDocument, OCRManager, OCRConfig } = require('../src');
const path = require('path');
const fs = require('fs');

const SAMPLE_PDF_PATH = path.join(__dirname, '../test_fixtures/sample-multi-page.pdf');

describe('OCRManager Test Suite', () => {
  let doc;
  let manager;

  beforeAll(() => {
    if (!fs.existsSync(SAMPLE_PDF_PATH)) {
      throw new Error(`Test fixture not found: ${SAMPLE_PDF_PATH}`);
    }
  });

  beforeEach(() => {
    doc = PdfDocument.open(SAMPLE_PDF_PATH);
    manager = new OCRManager(doc);
  });

  afterEach(async () => {
    if (manager) {
      await manager.close();
    }
    if (doc) {
      doc.close();
    }
  });

  describe('OCREngine Lifecycle', () => {
    test('Manager initializes with document', () => {
      expect(manager).not.toBeNull();
      expect(manager.document).toEqual(doc);
    });

    test('Engine creation with default config', async () => {
      const engine = await manager.createEngine();
      expect(engine).not.toBeNull();
      expect(engine.isReady()).toBe(true);
    });

    test('Engine creation with custom config', async () => {
      const config = new OCRConfig({
        detectionThreshold: 0.4,
        recognitionThreshold: 0.6,
        maxSideLen: 1280,
        useGPU: false,
      });

      const engine = await manager.createEngine(config);
      expect(engine).not.toBeNull();
      expect(engine.isReady()).toBe(true);
    });

    test('Engine status is available', async () => {
      const engine = await manager.createEngine();
      const status = engine.getStatus();

      expect(status).not.toBeNull();
      expect(status).toHaveProperty('model_loaded');
      expect(status.model_loaded).toBe(true);
    });

    test('Engine can be closed', async () => {
      const engine = await manager.createEngine();
      engine.close();
      expect(engine.isReady()).toBe(false);
    });
  });

  describe('Page Recognition', () => {
    beforeEach(async () => {
      await manager.createEngine();
    });

    test('Recognize page returns OCRResult', async () => {
      const result = await manager.recognizePage(0);

      expect(result).not.toBeNull();
      expect(result.pageIndex).toBe(0);
      expect(result.text).not.toBeUndefined();
      expect(result.spans).not.toBeUndefined();
      expect(result.averageConfidence).toBeGreaterThanOrEqual(0);
      expect(result.averageConfidence).toBeLessThanOrEqual(1);
    });

    test('Extract text from page', async () => {
      const text = await manager.extractText(0);
      expect(typeof text).toBe('string');
    });

    test('Extract spans with bounding boxes', async () => {
      const spans = await manager.extractSpans(0);

      expect(Array.isArray(spans)).toBe(true);
      for (const span of spans) {
        expect(span.text).toBeDefined();
        expect(span.confidence).toBeGreaterThanOrEqual(0);
        expect(span.confidence).toBeLessThanOrEqual(1);
        expect(span.boundingBox).toBeDefined();
      }
    });

    test('Invalid page index raises error', async () => {
      await expect(manager.recognizePage(999)).rejects.toThrow();
    });

    test('Negative page index raises error', async () => {
      await expect(manager.recognizePage(-1)).rejects.toThrow();
    });

    test('Needs OCR check returns boolean', async () => {
      const needsOcr = await manager.needsOcr(0);
      expect(typeof needsOcr).toBe('boolean');
    });

    test('Detect page returns region map', async () => {
      const regions = await manager.detectPage(0);
      expect(regions).toBeDefined();
      expect(regions).toHaveProperty('regions');
    });
  });

  describe('Result Caching', () => {
    beforeEach(async () => {
      await manager.createEngine();
    });

    test('Results are cached after recognition', async () => {
      const result1 = await manager.recognizePage(0);

      const stats = manager.getCacheStats();
      expect(stats.size).toBeGreaterThan(0);
      expect(stats.pages).toContain(0);

      const result2 = await manager.recognizePage(0);
      expect(result1.text).toEqual(result2.text);
    });

    test('Cache statistics track cached pages', async () => {
      await manager.recognizePage(0);

      const stats = manager.getCacheStats();
      expect(stats).toHaveProperty('size');
      expect(stats).toHaveProperty('pages');
      expect(stats.size).toBeGreaterThan(0);
    });

    test('Cache can be cleared', async () => {
      await manager.recognizePage(0);

      const statsBefore = manager.getCacheStats();
      expect(statsBefore.size).toBeGreaterThan(0);

      manager.clearCache();

      const statsAfter = manager.getCacheStats();
      expect(statsAfter.size).toBe(0);
    });

    test('Extract text uses cache', async () => {
      await manager.recognizePage(0);
      const text = await manager.extractText(0);
      expect(typeof text).toBe('string');
    });

    test('Extract spans uses cache', async () => {
      await manager.recognizePage(0);
      const spans = await manager.extractSpans(0);
      expect(Array.isArray(spans)).toBe(true);
    });
  });

  describe('Multi-Page Processing', () => {
    beforeEach(async () => {
      await manager.createEngine();
    });

    test('Extract multiple pages', async () => {
      const pageCount = doc.pageCount;
      if (pageCount >= 2) {
        const results = await manager.extractPages(0, Math.min(1, pageCount - 1));
        expect(results.length).toBeGreaterThan(0);

        for (const result of results) {
          expect(result.text).toBeDefined();
          expect(result.spans).toBeDefined();
        }
      }
    });

    test('Invalid page range raises error', async () => {
      await expect(manager.extractPages(5, 1)).rejects.toThrow();
    });

    test('Out-of-bounds page range raises error', async () => {
      await expect(manager.extractPages(0, 999)).rejects.toThrow();
    });
  });

  describe('Configuration Validation', () => {
    test('Invalid detection threshold raises error', () => {
      expect(() => {
        new OCRConfig({ detectionThreshold: 1.5 });
      }).toThrow();
    });

    test('Invalid recognition threshold raises error', () => {
      expect(() => {
        new OCRConfig({ recognitionThreshold: -0.1 });
      }).toThrow();
    });

    test('Invalid max side len raises error', () => {
      expect(() => {
        new OCRConfig({ maxSideLen: 50 });
      }).toThrow();
    });

    test('Valid configuration is accepted', () => {
      const config = new OCRConfig({
        detectionThreshold: 0.5,
        recognitionThreshold: 0.5,
        maxSideLen: 960,
        useGPU: false,
      });

      expect(config).not.toBeNull();
      expect(config.detectionThreshold).toBe(0.5);
      expect(config.recognitionThreshold).toBe(0.5);
      expect(config.maxSideLen).toBe(960);
      expect(config.useGPU).toBe(false);
    });
  });

  describe('Error Handling', () => {
    test('Uninitialized engine raises error', async () => {
      // Don't create engine
      await expect(manager.recognizePage(0)).rejects.toThrow();
    });

    test('Error propagates correctly', async () => {
      await manager.createEngine();
      await expect(manager.recognizePage(-5)).rejects.toThrow();
    });
  });

  describe('Resource Management', () => {
    test('Manager cleans up resources', async () => {
      const engine = await manager.createEngine();
      await manager.recognizePage(0);

      const statsBefore = manager.getCacheStats();
      expect(statsBefore.size).toBeGreaterThan(0);

      await manager.close();

      const statsAfter = manager.getCacheStats();
      expect(statsAfter.size).toBe(0);
    });
  });

  describe('Result Structure Validation', () => {
    beforeEach(async () => {
      await manager.createEngine();
    });

    test('OCRResult has all required fields', async () => {
      const result = await manager.recognizePage(0);

      expect(result).toHaveProperty('pageIndex');
      expect(result).toHaveProperty('text');
      expect(result).toHaveProperty('spans');
      expect(result).toHaveProperty('averageConfidence');
      expect(result).toHaveProperty('processingTimeMs');
    });

    test('OCRSpan has all required fields', async () => {
      const spans = await manager.extractSpans(0);

      if (spans.length > 0) {
        const span = spans[0];
        expect(span).toHaveProperty('text');
        expect(span).toHaveProperty('confidence');
        expect(span).toHaveProperty('boundingBox');

        expect(span.confidence).toBeGreaterThanOrEqual(0);
        expect(span.confidence).toBeLessThanOrEqual(1);
      }
    });
  });
});
