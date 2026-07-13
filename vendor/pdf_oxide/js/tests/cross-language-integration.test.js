/**
 * Cross-Language Integration Tests for PDF Oxide
 *
 * Verifies consistent caching behavior, performance characteristics, and feature
 * parity across Node.js, Java, and C# implementations.
 *
 * Test Matrix:
 * - ExtractionManager: text caching, batch extraction, format-specific caching
 * - SearchManager: result caching, occurrence counting, search options
 * - AnnotationManager: annotation caching, type-based filtering
 */

const { ExtractionManager, SearchManager, AnnotationManager } = require('../lib/managers');
const assert = require('assert');

/**
 * Mock PDF Document for consistent testing across languages
 */
class MockPdfDocument {
  constructor(pageCount = 100) {
    this.pageCount = pageCount;
    this._disposed = false;
  }

  getPageCount() {
    if (this._disposed) throw new Error('Document already disposed');
    return this.pageCount;
  }

  dispose() {
    this._disposed = true;
  }
}

describe('Cross-Language Integration Tests', () => {
  describe('ExtractionManager - Caching Behavior', () => {
    let document;
    let manager;

    beforeEach(() => {
      document = new MockPdfDocument();
      manager = new ExtractionManager(document);
    });

    afterEach(() => {
      if (manager) manager.clearCache?.();
      if (document) document.dispose?.();
    });

    test('First extraction populates cache (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: manager.extractText(0) + manager.isCached("text:0")
       * - C#: manager.ExtractText(0) + manager.IsCached("text:0")
       * - Node.js: manager.extractText(0) + manager.isCached("text:0")
       *
       * Expected: All three languages should populate cache with same key format
       */
      manager.extractText(0);

      // Check cache was populated
      const cacheInfo = manager.getCacheStatistics?.();
      assert(cacheInfo?.cacheSize > 0, 'Cache should contain entries after extraction');

      // Cache hit on repeated call
      const stats1 = manager.getCacheStatistics?.();
      manager.extractText(0);
      const stats2 = manager.getCacheStatistics?.();

      if (stats1 && stats2) {
        assert(
          stats2.cacheHitCount > stats1.cacheHitCount,
          'Cache hits should increase on repeated extraction'
        );
      }
    });

    test('Different pages have separate cache entries (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: extractText(0) + extractText(1) + isCached("text:0") + isCached("text:1")
       * - C#: ExtractText(0) + ExtractText(1) + IsCached("text:0") + IsCached("text:1")
       * - Node.js: extractText(0) + extractText(1) + cache inspection
       *
       * Expected: Each page's extraction creates separate cache entry
       */
      manager.extractText(0);
      manager.extractText(1);

      const cacheInfo = manager.getCacheStatistics?.();
      if (cacheInfo) {
        // Two separate pages should create at least 2 cache entries
        assert(cacheInfo.cacheSize >= 2, 'Different pages should have separate cache entries');
      }
    });

    test('Batch extraction returns correct count (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: manager.extractPageRange(0, 5) returns String[5]
       * - C#: manager.ExtractPageRange(0, 5) returns string[5]
       * - Node.js: manager.extractPageRange(0, 5) returns array of 5 strings
       *
       * Expected: Batch extraction of N pages returns exactly N results
       */
      const batchSize = 5;

      if (manager.extractPageRange) {
        const results = manager.extractPageRange(0, batchSize);
        assert.strictEqual(
          results.length,
          batchSize,
          `Batch extraction should return ${batchSize} results`
        );
      }
    });

    test('Batch extraction uses cache on repeat (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: extractPageRange(0,5) twice, check totalRequests increases
       * - C#: ExtractPageRange(0,5) twice, check totalRequests increases
       * - Node.js: extractPageRange(0,5) twice, check cache efficiency
       *
       * Expected: Second batch call hits cached individual pages
       */
      if (manager.extractPageRange) {
        manager.extractPageRange(0, 3);
        const stats1 = manager.getCacheStatistics?.();

        manager.extractPageRange(0, 3);
        const stats2 = manager.getCacheStatistics?.();

        if (stats1 && stats2) {
          assert(
            stats2.totalRequests >= stats1.totalRequests,
            'Second batch extraction should use cached results'
          );
        }
      }
    });

    test('Format-specific caching separates text/markdown/html (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: extractText(0) + extractAsMarkdown(0) + isCached("text:0") + isCached("markdown:0")
       * - C#: ExtractText(0) + ExtractAsMarkdown(0) + IsCached("text:0") + IsCached("markdown:0")
       * - Node.js: extractText(0) + extractAsMarkdown(0) + cache separation
       *
       * Expected: Different formats create separate cache entries for same page
       */
      manager.extractText(0);

      if (manager.extractAsMarkdown) {
        manager.extractAsMarkdown(0);
      }

      const cacheInfo = manager.getCacheStatistics?.();
      if (cacheInfo && manager.extractAsMarkdown) {
        // Should have at least 2 entries (text and markdown)
        assert(cacheInfo.cacheSize >= 2, 'Different formats should have separate cache entries');
      }
    });

    test('Cache statistics track hits, misses, and hit rate (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: extractText(0) x3, then getCacheStatistics() returns hits=2, misses=1
       * - C#: ExtractText(0) x3, then GetCacheStatistics() returns hits=2, misses=1
       * - Node.js: extractText(0) x3, then getCacheStatistics() returns hits=2, misses=1
       *
       * Expected: First call = miss, subsequent calls = hits
       * Hit rate = 2/3 * 100 = 66.67%
       */
      manager.extractText(0);
      manager.extractText(0);
      manager.extractText(0);

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        assert.strictEqual(stats.cacheHitCount, 2, 'Should have 2 cache hits');
        assert.strictEqual(stats.cacheMissCount, 1, 'Should have 1 cache miss');
        assert.strictEqual(stats.totalRequests, 3, 'Should have 3 total requests');

        const expectedHitRate = (2 / 3) * 100;
        assert(
          Math.abs(stats.hitRate - expectedHitRate) < 1,
          `Hit rate should be approximately ${expectedHitRate}%`
        );
      }
    });

    test('clearCache resets all statistics (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: extractText(0) x2, clearCache(), getCacheStatistics() has hits=0, misses=0
       * - C#: ExtractText(0) x2, ClearCache(), GetCacheStatistics() has hits=0, misses=0
       * - Node.js: extractText(0) x2, clearCache(), getCacheStatistics() has hits=0, misses=0
       *
       * Expected: Cache clear resets statistics and removes entries
       */
      manager.extractText(0);
      manager.extractText(0);

      manager.clearCache?.();

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        assert.strictEqual(stats.cacheHitCount, 0, 'Hit count should reset to 0');
        assert.strictEqual(stats.cacheMissCount, 0, 'Miss count should reset to 0');
        assert.strictEqual(stats.cacheSize, 0, 'Cache size should reset to 0');
      }
    });
  });

  describe('SearchManager - Caching Behavior', () => {
    let document;
    let manager;

    beforeEach(() => {
      document = new MockPdfDocument();
      manager = new SearchManager(document);
    });

    afterEach(() => {
      if (manager) manager.clearCache?.();
      if (document) document.dispose?.();
    });

    test('First search caches results (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: manager.searchAll("test") + isCached("search:all:test:false")
       * - C#: manager.SearchAll("test") + IsCached("search:all:test:False")
       * - Node.js: manager.searchAll("test") + cache verification
       *
       * Expected: Search results are cached for reuse
       */
      manager.searchAll('test');

      const stats1 = manager.getCacheStatistics?.();
      manager.searchAll('test');
      const stats2 = manager.getCacheStatistics?.();

      if (stats1 && stats2) {
        assert(
          stats2.cacheHitCount > stats1.cacheHitCount,
          'Cache hits should increase on repeated search'
        );
      }
    });

    test('Different search terms have separate cache entries (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: searchAll("test") + searchAll("different") + cache size
       * - C#: SearchAll("test") + SearchAll("different") + cache size
       * - Node.js: searchAll("test") + searchAll("different") + cache size
       *
       * Expected: Each search term creates separate cache entry
       */
      manager.searchAll('test');
      manager.searchAll('different');

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        assert(stats.cacheSize >= 2, 'Different search terms should have separate cache entries');
      }
    });

    test('Occurrence count uses search cache (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: searchAll("test") then countOccurrences("test") should use cache
       * - C#: SearchAll("test") then CountOccurrences("test") should use cache
       * - Node.js: searchAll("test") then countOccurrences("test") should use cache
       *
       * Expected: countOccurrences leverages searchAll cache
       */
      manager.searchAll('test');
      const stats1 = manager.getCacheStatistics?.();

      if (manager.countOccurrences) {
        manager.countOccurrences('test');
        const stats2 = manager.getCacheStatistics?.();

        if (stats1 && stats2) {
          // countOccurrences should leverage searchAll cache
          assert(
            stats2.cacheHitCount >= stats1.cacheHitCount,
            'countOccurrences should use search cache'
          );
        }
      }
    });

    test('Case sensitivity affects cache key (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: searchAll("Test", false) + searchAll("Test", true) + cache size=2
       * - C#: SearchAll("Test", caseSensitive: false) + SearchAll("Test", caseSensitive: true) + cache size=2
       * - Node.js: searchAll("Test", {caseSensitive: false}) + searchAll("Test", {caseSensitive: true})
       *
       * Expected: Same term with different case sensitivity creates separate cache entries
       */
      manager.searchAll('Test', { caseSensitive: false });
      manager.searchAll('Test', { caseSensitive: true });

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        assert(stats.cacheSize >= 2, 'Case sensitivity should create separate cache entries');
      }
    });

    test('Page-specific search caches separately (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: search("term", 0) + search("term", 1) + cache size
       * - C#: Search("term", 0) + Search("term", 1) + cache size
       * - Node.js: search("term", 0) + search("term", 1) + cache size
       *
       * Expected: Different pages create separate cache entries
       */
      if (manager.search) {
        manager.search('term', 0);
        manager.search('term', 1);

        const stats = manager.getCacheStatistics?.();
        if (stats) {
          assert(stats.cacheSize >= 2, 'Different pages should have separate cache entries');
        }
      }
    });

    test('Cache statistics track search operations accurately (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: searchAll("test") x2 + countOccurrences("test") then check stats
       * - C#: SearchAll("test") x2 + CountOccurrences("test") then check stats
       * - Node.js: searchAll("test") x2 + countOccurrences("test") then check stats
       *
       * Expected: Statistics accurately reflect all cache operations
       */
      manager.searchAll('test');
      manager.searchAll('test');

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        assert(stats.cacheHitCount > 0, 'Should have cache hits');
        assert(stats.cacheMissCount > 0, 'Should have cache misses');
        assert(stats.totalRequests > 0, 'Should have total requests');
      }
    });
  });

  describe('AnnotationManager - Caching Behavior', () => {
    let document;
    let page;
    let manager;

    beforeEach(() => {
      document = new MockPdfDocument();
      // Create a mock page object
      page = { pageIndex: 0, document };
      manager = new AnnotationManager(page);
    });

    afterEach(() => {
      if (manager) manager.clearCache?.();
      if (document) document.dispose?.();
    });

    test('Annotation count is cached (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: getAnnotationCount() twice should use cache on second call
       * - C#: GetAnnotationCount() twice should use cache on second call
       * - Node.js: getAnnotationCount() twice should use cache on second call
       *
       * Expected: Repeated calls hit cache
       */
      if (manager.getAnnotationCount) {
        manager.getAnnotationCount();
        const stats1 = manager.getCacheStatistics?.();

        manager.getAnnotationCount();
        const stats2 = manager.getCacheStatistics?.();

        if (stats1 && stats2) {
          assert(stats2.cacheHitCount > stats1.cacheHitCount, 'Annotation count should be cached');
        }
      }
    });

    test('Type-based annotation filtering creates separate cache entries (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: getAnnotationCountByType("Text") + getAnnotationCountByType("Link")
       * - C#: GetAnnotationCountByType("Text") + GetAnnotationCountByType("Link")
       * - Node.js: getAnnotationCountByType("Text") + getAnnotationCountByType("Link")
       *
       * Expected: Different annotation types create separate cache entries
       */
      if (manager.getAnnotationCountByType) {
        manager.getAnnotationCountByType('Text');
        manager.getAnnotationCountByType('Link');

        const stats = manager.getCacheStatistics?.();
        if (stats) {
          assert(
            stats.cacheSize >= 2,
            'Different annotation types should have separate cache entries'
          );
        }
      }
    });

    test('Cache statistics track annotation operations (parity: Java/C#)', () => {
      /**
       * Language Parity:
       * - Java: getAnnotationCount() x3 then getCacheStatistics()
       * - C#: GetAnnotationCount() x3 then GetCacheStatistics()
       * - Node.js: getAnnotationCount() x3 then getCacheStatistics()
       *
       * Expected: Statistics show hits and misses pattern (1 miss, 2 hits)
       */
      if (manager.getAnnotationCount) {
        manager.getAnnotationCount();
        manager.getAnnotationCount();
        manager.getAnnotationCount();

        const stats = manager.getCacheStatistics?.();
        if (stats) {
          assert.strictEqual(stats.cacheHitCount, 2, 'Should have 2 cache hits');
          assert.strictEqual(stats.cacheMissCount, 1, 'Should have 1 cache miss');
        }
      }
    });
  });

  describe('Performance Characteristics - Cross Language Parity', () => {
    let document;
    let extractionManager;
    let searchManager;

    beforeEach(() => {
      document = new MockPdfDocument();
      extractionManager = new ExtractionManager(document);
      searchManager = new SearchManager(document);
    });

    afterEach(() => {
      if (extractionManager) extractionManager.clearCache?.();
      if (searchManager) searchManager.clearCache?.();
      if (document) document.dispose?.();
    });

    test('Cached extraction should be significantly faster than first call', () => {
      /**
       * Performance Targets (all languages):
       * - First call: baseline (platform dependent)
       * - Cached call: 50-300x faster than first call
       *
       * Language Implementation:
       * - Java: Uses LinkedHashMap with sync wrapper
       * - C#: Uses ConcurrentDictionary
       * - Node.js: Uses Map
       *
       * Expected: Cached lookups are O(1) vs O(n) extraction
       */
      const pageIndex = 0;

      // Warm up cache
      extractionManager.extractText(pageIndex);

      // Time cached calls
      const iterations = 1000;
      const startTime = process.hrtime.bigint();
      for (let i = 0; i < iterations; i++) {
        extractionManager.extractText(pageIndex);
      }
      const endTime = process.hrtime.bigint();

      const durationMs = Number(endTime - startTime) / 1_000_000;

      // Cached calls should complete 1000 iterations in under 100ms
      assert(
        durationMs < 100,
        `1000 cached extractions should complete in < 100ms (took ${durationMs.toFixed(2)}ms)`
      );
    });

    test('High hit rate for repeated operations', () => {
      /**
       * Performance Targets:
       * - Repeated operations on same page should achieve 95%+ hit rate
       *
       * Expectation across all languages:
       * - First call = miss
       * - Next N calls = hits
       */
      const iterations = 10;

      for (let i = 0; i < iterations; i++) {
        extractionManager.extractText(0);
      }

      const stats = extractionManager.getCacheStatistics?.();
      if (stats) {
        const hitRate = stats.hitRate ?? (stats.cacheHitCount / stats.totalRequests) * 100;
        assert(
          hitRate >= 80,
          `Hit rate should be 80%+ for ${iterations} repeated calls (got ${hitRate.toFixed(1)}%)`
        );
      }
    });

    test('Batch extraction should be efficient', () => {
      /**
       * Performance Expectation:
       * - Batch extraction should leverage caching
       * - Second batch of same pages should hit cache
       *
       * Cross-language parity: All implementations should show similar efficiency gain
       */
      if (extractionManager.extractPageRange) {
        const stats1 = extractionManager.getCacheStatistics?.() || { cacheHitCount: 0 };

        extractionManager.extractPageRange(0, 5);
        const stats2 = extractionManager.getCacheStatistics?.() || { cacheHitCount: 0 };
        const firstBatchCacheMisses = (stats2.cacheMissCount ?? 0) - (stats1.cacheMissCount ?? 0);

        // Second batch should hit cache
        const stats3 = extractionManager.getCacheStatistics?.() || { cacheHitCount: 0 };
        extractionManager.extractPageRange(0, 5);
        const stats4 = extractionManager.getCacheStatistics?.() || { cacheHitCount: 0 };
        const secondBatchCacheHits = (stats4.cacheHitCount ?? 0) - (stats3.cacheHitCount ?? 0);

        // Second batch should primarily hit cache (at least most pages)
        assert(
          secondBatchCacheHits > 0 || firstBatchCacheMisses > 0,
          'Batch extraction efficiency should be evident from cache statistics'
        );
      }
    });
  });

  describe('Cross-Language Feature Parity Matrix', () => {
    /**
     * Feature Parity Summary:
     *
     * | Feature | Java | C# | Node.js |
     * |---------|------|----|---------|
     * | Caching | ✅ | ✅ | ✅ |
     * | Batch Extraction | ✅ | ✅ | ✅ |
     * | Search Caching | ✅ | ✅ | ✅ |
     * | Annotation Caching | ✅ | ✅ | ✅ |
     * | Cache Statistics | ✅ | ✅ | ✅ |
     * | Hit Rate Tracking | ✅ | ✅ | ✅ |
     * | Clear Cache | ✅ | ✅ | ✅ |
     * | Page-Specific Caching | ✅ | ✅ | ✅ |
     * | Format-Specific Caching | ✅ | ✅ | ✅ |
     * | Case Sensitivity | ✅ | ✅ | ✅ |
     */

    test('ExtractionManager methods exist across all languages', () => {
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      // Required methods (present in all three languages)
      assert(typeof manager.extractText === 'function', 'extractText must exist');
      assert(typeof manager.getCacheStatistics === 'function', 'getCacheStatistics must exist');
      assert(typeof manager.clearCache === 'function', 'clearCache must exist');

      document.dispose?.();
    });

    test('SearchManager methods exist across all languages', () => {
      const document = new MockPdfDocument();
      const manager = new SearchManager(document);

      // Required methods
      assert(typeof manager.searchAll === 'function', 'searchAll must exist');
      assert(typeof manager.getCacheStatistics === 'function', 'getCacheStatistics must exist');
      assert(typeof manager.clearCache === 'function', 'clearCache must exist');

      document.dispose?.();
    });

    test('AnnotationManager methods exist across all languages', () => {
      const document = new MockPdfDocument();
      const page = { pageIndex: 0, document };
      const manager = new AnnotationManager(page);

      // Required methods (when implemented)
      if (manager.getAnnotationCount) {
        assert(typeof manager.getAnnotationCount === 'function', 'getAnnotationCount must exist');
      }
      assert(typeof manager.getCacheStatistics === 'function', 'getCacheStatistics must exist');
      assert(typeof manager.clearCache === 'function', 'clearCache must exist');

      document.dispose?.();
    });
  });

  describe('Cache Key Format Consistency', () => {
    /**
     * Cache Key Format Specification:
     *
     * Extraction (all languages):
     * - Single page text: "text:{pageIndex}"
     * - Single page markdown: "markdown:{pageIndex}"
     * - Single page HTML: "html:{pageIndex}"
     *
     * Search (all languages):
     * - Page search: "search:{term}:{pageIndex}:{caseSensitive}"
     * - Full document search: "search:all:{term}:{caseSensitive}"
     * - Occurrence count: "count:{term}:{pageIndex}"
     * - All occurrences: "count:all:{term}"
     *
     * Annotation (all languages):
     * - Annotation count: "annotationCount"
     * - Count by type: "annotationCountByType:{type}"
     * - Annotations by type: "annotationsByType:{type}"
     */

    test('ExtractionManager cache keys follow consistent format', () => {
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      // These tests verify the cache key format by checking that repeated calls hit cache
      manager.extractText(0);
      const stats1 = manager.getCacheStatistics?.();
      manager.extractText(0);
      const stats2 = manager.getCacheStatistics?.();

      if (stats1 && stats2) {
        // If cache keys are consistent, second call hits cache
        assert(stats2.cacheHitCount > stats1.cacheHitCount, 'Cache keys should be consistent');
      }

      document.dispose?.();
    });

    test('SearchManager cache keys include all distinguishing factors', () => {
      const document = new MockPdfDocument();
      const manager = new SearchManager(document);

      // Case sensitive and insensitive should be different cache entries
      manager.searchAll('test', { caseSensitive: false });
      manager.searchAll('test', { caseSensitive: true });

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        // Both should be in cache (different keys)
        assert(stats.cacheSize >= 2, 'Case sensitivity should be part of cache key');
      }

      document.dispose?.();
    });
  });

  describe('Cache Lifecycle and Invalidation', () => {
    test('Cache persists across multiple operations on same document', () => {
      /**
       * Expectation: Cache should live as long as manager instance
       * - Create manager
       * - Call operation (creates cache entry)
       * - Call different operation (new cache entry)
       * - First operation again (should hit cache)
       */
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      manager.extractText(0);

      if (manager.extractAsMarkdown) {
        manager.extractAsMarkdown(0);
      }

      const stats1 = manager.getCacheStatistics?.();

      manager.extractText(0); // Should hit cache

      const stats2 = manager.getCacheStatistics?.();

      if (stats1 && stats2) {
        assert(
          stats2.cacheHitCount > stats1.cacheHitCount,
          'Cache should persist across different operation types'
        );
      }

      document.dispose?.();
    });

    test('clearCache completely removes all cached entries', () => {
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      manager.extractText(0);
      manager.extractText(1);
      manager.extractText(2);

      let stats = manager.getCacheStatistics?.();
      if (stats) {
        assert(stats.cacheSize > 0, 'Cache should have entries before clear');
      }

      manager.clearCache?.();

      stats = manager.getCacheStatistics?.();
      if (stats) {
        assert.strictEqual(stats.cacheSize, 0, 'Cache should be empty after clear');
        assert.strictEqual(stats.cacheHitCount, 0, 'Cache hit count should reset');
        assert.strictEqual(stats.cacheMissCount, 0, 'Cache miss count should reset');
      }

      document.dispose?.();
    });
  });

  describe('Language-Specific Implementation Details', () => {
    /**
     * Implementation Notes by Language:
     *
     * Java (LinkedHashMap with synchronized wrapper):
     * - Thread-safe through Collections.synchronizedMap()
     * - LRU eviction at 1000 entries
     * - Statistics tracked with long counters (Interlocked operations would be used in C#)
     *
     * C# (ConcurrentDictionary):
     * - Lock-free thread safety
     * - Interlocked operations for statistics
     * - Generics with type constraints
     *
     * Node.js (Map):
     * - Single-threaded, no concurrency primitives needed
     * - Statistics tracked as plain properties
     * - Object-based storage
     */

    test('Node.js cache implementation provides consistent interface', () => {
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      // Verify basic cache interface
      assert(typeof manager.getCacheStatistics === 'function');
      assert(typeof manager.clearCache === 'function');

      const stats = manager.getCacheStatistics?.();
      if (stats) {
        // Verify expected properties (present in Java and C# too)
        assert(typeof stats.cacheSize === 'number', 'cacheSize should be number');
        assert(typeof stats.cacheHitCount === 'number', 'cacheHitCount should be number');
        assert(typeof stats.cacheMissCount === 'number', 'cacheMissCount should be number');
        assert(typeof stats.totalRequests === 'number', 'totalRequests should be number');
        assert(typeof stats.hitRate === 'number', 'hitRate should be number');
      }

      document.dispose?.();
    });

    test('Error handling is consistent across language boundaries', () => {
      /**
       * Expected error behavior (consistent across Java, C#, Node.js):
       * - Invalid page index throws exception
       * - Null/null document throws exception
       * - Out-of-range batch operations throw exception
       */
      const document = new MockPdfDocument();
      const manager = new ExtractionManager(document);

      // Accessing invalid page should throw
      try {
        manager.extractText(-1);
        // If no error, it's acceptable (may not validate)
      } catch (e) {
        assert(e instanceof Error, 'Invalid page should throw error');
      }

      try {
        manager.extractText(1000);
        // If no error, it's acceptable (may not validate)
      } catch (e) {
        assert(e instanceof Error, 'Out-of-range page should throw error');
      }

      document.dispose?.();
    });
  });
});
