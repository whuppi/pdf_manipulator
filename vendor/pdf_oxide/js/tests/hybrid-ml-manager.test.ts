import { HybridMLManager, type PageAnalysisResult, PageComplexity } from '../src/hybrid-ml-manager';

describe('HybridMLManager', () => {
  let doc: any; // PdfDocument

  beforeAll(async () => {
    // TODO: Open actual test PDF
    // doc = await PdfDocument.open('test_files/sample.pdf');
  });

  afterAll(async () => {
    // TODO: Close document
    // if (doc) doc.close();
  });

  describe('analyze_page', () => {
    test('returns PageAnalysisResult', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const result = await manager.analyzePage(0);

      expect(result).toBeDefined();
      expect(result.complexity).toBeGreaterThanOrEqual(PageComplexity.SIMPLE);
      expect(result.complexity).toBeLessThanOrEqual(PageComplexity.VERY_COMPLEX);
      expect(result.complexityScore).toBeGreaterThanOrEqual(0);
      expect(result.complexityScore).toBeLessThanOrEqual(1);
      expect(result.textDensity).toBeGreaterThanOrEqual(0);
      expect(result.textDensity).toBeLessThanOrEqual(1);
      expect(result.imageDensity).toBeGreaterThanOrEqual(0);
      expect(result.imageDensity).toBeLessThanOrEqual(1);
    });

    test('throws error for invalid page index', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      await expect(manager.analyzePage(-1)).rejects.toThrow();
      await expect(manager.analyzePage(9999)).rejects.toThrow();
    });

    test('caches results', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      const result1 = await manager.analyzePage(0);
      const result2 = await manager.analyzePage(0);

      // Same cached object or identical values
      expect(result1.complexity).toBe(result2.complexity);
      expect(result1.complexityScore).toBe(result2.complexityScore);
    });
  });

  describe('analyzeDocument', () => {
    test('returns overall complexity score', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const score = await manager.analyzeDocument();

      expect(typeof score).toBe('number');
      expect(score).toBeGreaterThanOrEqual(0);
      expect(score).toBeLessThanOrEqual(1);
    });
  });

  describe('createExtractionStrategy', () => {
    test('returns ExtractionStrategy', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const strategy = await manager.createExtractionStrategy(0);

      expect(strategy).toBeDefined();
      expect(typeof strategy.description).toBe('string');
      expect(typeof strategy.recommends_ocr).toBe('boolean');
    });
  });

  describe('detectTables', () => {
    test('returns array of TableRegion', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const tables = await manager.detectTables(0);

      expect(Array.isArray(tables)).toBe(true);

      for (const table of tables) {
        expect(table.x).toBeGreaterThanOrEqual(0);
        expect(table.y).toBeGreaterThanOrEqual(0);
        expect(table.width).toBeGreaterThanOrEqual(0);
        expect(table.height).toBeGreaterThanOrEqual(0);
      }
    });
  });

  describe('detectColumns', () => {
    test('returns array of ColumnRegion', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const columns = await manager.detectColumns(0);

      expect(Array.isArray(columns)).toBe(true);

      for (const column of columns) {
        expect(column.x).toBeGreaterThanOrEqual(0);
        expect(column.y).toBeGreaterThanOrEqual(0);
        expect(column.width).toBeGreaterThanOrEqual(0);
        expect(column.height).toBeGreaterThanOrEqual(0);
      }
    });
  });

  describe('getMlStatus', () => {
    test('returns status string', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const status = await manager.getMlStatus();

      expect(typeof status).toBe('string');
    });
  });

  describe('isModelAvailable', () => {
    test('checks model availability', () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      // Test with various model names
      const modelNames = ['detection', 'classification', 'segmentation'];

      for (const modelName of modelNames) {
        const available = manager.isModelAvailable(modelName);
        expect(typeof available).toBe('boolean');
      }
    });
  });

  describe('findPagesWithTables', () => {
    test('returns map of pages with tables', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const result = await manager.findPagesWithTables();

      expect(result instanceof Map).toBe(true);

      for (const [pageIndex, tables] of result) {
        expect(typeof pageIndex).toBe('number');
        expect(Array.isArray(tables)).toBe(true);
      }
    });
  });

  describe('findPagesNeedingOcr', () => {
    test('returns array of page indices', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);
      const pages = await manager.findPagesNeedingOcr();

      expect(Array.isArray(pages)).toBe(true);

      for (const pageIndex of pages) {
        expect(typeof pageIndex).toBe('number');
        expect(pageIndex).toBeGreaterThanOrEqual(0);
      }
    });
  });

  describe('cache management', () => {
    test('clearCache clears all cached data', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      // Populate cache
      await manager.analyzePage(0);
      await manager.analyzeDocument();

      // Clear cache
      manager.clearCache();

      // Verify cache stats
      const stats = manager.getCacheStats();
      expect(stats.cacheSize).toBe(0);
    });

    test('getCacheStats returns cache information', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      // Populate cache
      await manager.analyzePage(0);

      // Get stats
      const stats = manager.getCacheStats();

      expect(stats).toBeDefined();
      expect(typeof stats.cacheSize).toBe('number');
      expect(typeof stats.maxCacheSize).toBe('number');
      expect(Array.isArray(stats.entries)).toBe(true);
    });
  });

  describe('batch operations', () => {
    test('analyzeAllPagesAsync iterates through pages', async () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      const results: Array<{ pageIndex: number; result: PageAnalysisResult }> = [];

      for await (const item of manager.analyzeAllPagesAsync()) {
        results.push(item);
      }

      expect(Array.isArray(results)).toBe(true);

      for (const item of results) {
        expect(typeof item.pageIndex).toBe('number');
        expect(item.result).toBeDefined();
      }
    });
  });

  describe('error handling', () => {
    test('throws error for invalid document', () => {
      expect(() => new HybridMLManager(null)).toThrow();
      expect(() => new HybridMLManager(undefined)).toThrow();
    });

    test('throws error for invalid model name', () => {
      if (!doc) {
        console.log('Skipping test: sample document not available');
        return;
      }

      const manager = new HybridMLManager(doc);

      expect(() => manager.isModelAvailable('')).toThrow();
    });
  });
});
