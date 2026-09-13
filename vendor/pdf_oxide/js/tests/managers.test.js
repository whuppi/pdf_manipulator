/**
 * Manager Classes Tests - Phase 2.7
 *
 * Comprehensive test suite for manager pattern classes covering:
 * - OutlineManager: Bookmark navigation
 * - MetadataManager: Document metadata access
 * - ExtractionManager: Content extraction
 * - SearchManager: Text search
 * - SecurityManager: Encryption and permissions
 * - AnnotationManager: Page annotations
 */

import assert from 'node:assert';
import { describe, it } from 'node:test';
import {
  AnnotationManager,
  ExtractionManager,
  MetadataManager,
  OutlineManager,
  SearchManager,
  SecurityManager,
} from '../lib/managers/index.js';

describe('Managers - Phase 2.7', () => {
  describe('OutlineManager', () => {
    it('should create manager instance', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      assert.ok(manager instanceof OutlineManager);
    });

    it('should throw on null document', () => {
      assert.throws(() => new OutlineManager(null), /Document is required/);
      assert.throws(() => new OutlineManager(undefined), /Document is required/);
    });

    it('should check for outlines', () => {
      const mockDoc = { hasStructureTree: true };
      const manager = new OutlineManager(mockDoc);
      assert.ok(manager.hasOutlines() === true);
    });

    it('should return outline count', () => {
      const mockDoc = { hasStructureTree: true };
      const manager = new OutlineManager(mockDoc);
      assert.strictEqual(typeof manager.getOutlineCount(), 'number');
    });

    it('should return empty array for outlines', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      const outlines = manager.getOutlines();
      assert.ok(Array.isArray(outlines));
    });

    it('should find outline by title', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      const result = manager.findByTitle('test');
      assert.ok(result === null);
    });

    it('should validate title fragment', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      assert.throws(() => manager.findByTitle(''), /non-empty string/);
      assert.throws(() => manager.findByTitle(null), /non-empty string/);
    });

    it('should find all matching outlines', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      const results = manager.findAllByTitle('test');
      assert.ok(Array.isArray(results));
    });

    it('should get outlines for page', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      const results = manager.getOutlinesForPage(0);
      assert.ok(Array.isArray(results));
    });

    it('should validate page index', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      assert.throws(() => manager.getOutlinesForPage(-1), /non-negative number/);
      assert.throws(() => manager.getOutlinesForPage('0'), /non-negative number/);
    });

    it('should check page has outlines', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      assert.strictEqual(manager.pageHasOutlines(0), false);
    });

    it('should get outline at index', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      const result = manager.getOutlineAt(0);
      assert.ok(result === null);
    });

    it('should check contains page number', () => {
      const mockDoc = {};
      const manager = new OutlineManager(mockDoc);
      assert.strictEqual(manager.containsPageNumber(1), false);
    });
  });

  describe('MetadataManager', () => {
    it('should create manager instance', () => {
      const mockDoc = { documentInfo: {} };
      const manager = new MetadataManager(mockDoc);
      assert.ok(manager instanceof MetadataManager);
    });

    it('should throw on null document', () => {
      assert.throws(() => new MetadataManager(null), /Document is required/);
    });

    it('should get title', () => {
      const mockDoc = { documentInfo: { title: 'Test Doc' } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getTitle(), 'Test Doc');
    });

    it('should get author', () => {
      const mockDoc = { documentInfo: { author: 'John Doe' } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getAuthor(), 'John Doe');
    });

    it('should get subject', () => {
      const mockDoc = { documentInfo: { subject: 'Test Subject' } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getSubject(), 'Test Subject');
    });

    it('should get keywords array', () => {
      const mockDoc = { documentInfo: { keywords: ['test', 'doc'] } };
      const manager = new MetadataManager(mockDoc);
      assert.deepStrictEqual(manager.getKeywords(), ['test', 'doc']);
    });

    it('should return empty array for missing keywords', () => {
      const mockDoc = { documentInfo: {} };
      const manager = new MetadataManager(mockDoc);
      assert.deepStrictEqual(manager.getKeywords(), []);
    });

    it('should get creator', () => {
      const mockDoc = { documentInfo: { creator: 'Adobe' } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getCreator(), 'Adobe');
    });

    it('should get producer', () => {
      const mockDoc = { documentInfo: { producer: 'Test Producer' } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getProducer(), 'Test Producer');
    });

    it('should get creation date', () => {
      const date = new Date();
      const mockDoc = { documentInfo: { creationDate: date } };
      const manager = new MetadataManager(mockDoc);
      assert.ok(manager.getCreationDate() instanceof Date);
    });

    it('should get modification date', () => {
      const date = new Date();
      const mockDoc = { documentInfo: { modificationDate: date } };
      const manager = new MetadataManager(mockDoc);
      assert.ok(manager.getModificationDate() instanceof Date);
    });

    it('should get all metadata', () => {
      const mockDoc = { documentInfo: { title: 'Test' } };
      const manager = new MetadataManager(mockDoc);
      const meta = manager.getAllMetadata();
      assert.ok(typeof meta === 'object');
    });

    it('should check has metadata', () => {
      const mockDoc = { documentInfo: { title: 'Test' } };
      const manager = new MetadataManager(mockDoc);
      assert.ok(manager.hasMetadata());
    });

    it('should get metadata summary', () => {
      const mockDoc = { documentInfo: { title: 'Test' }, pageCount: 10 };
      const manager = new MetadataManager(mockDoc);
      const summary = manager.getMetadataSummary();
      assert.ok(typeof summary === 'string');
    });

    it('should check has keyword', () => {
      const mockDoc = { documentInfo: { keywords: ['test', 'doc'] } };
      const manager = new MetadataManager(mockDoc);
      assert.ok(manager.hasKeyword('test'));
      assert.ok(!manager.hasKeyword('missing'));
    });

    it('should get keyword count', () => {
      const mockDoc = { documentInfo: { keywords: ['a', 'b', 'c'] } };
      const manager = new MetadataManager(mockDoc);
      assert.strictEqual(manager.getKeywordCount(), 3);
    });

    it('should compare with another document', () => {
      const mockDoc1 = { documentInfo: { title: 'Test1' } };
      const mockDoc2 = { documentInfo: { title: 'Test2' } };
      const manager = new MetadataManager(mockDoc1);
      const comparison = manager.compareWith(mockDoc2);
      assert.ok(comparison.matching);
      assert.ok(comparison.differing);
    });

    it('should validate metadata', () => {
      const mockDoc = { documentInfo: { title: 'Test' } };
      const manager = new MetadataManager(mockDoc);
      const validation = manager.validate();
      assert.ok('isComplete' in validation);
      assert.ok(Array.isArray(validation.issues));
    });
  });

  describe('ExtractionManager', () => {
    it('should create manager instance', () => {
      const mockDoc = { pageCount: 10, extractText: () => 'test' };
      const manager = new ExtractionManager(mockDoc);
      assert.ok(manager instanceof ExtractionManager);
    });

    it('should throw on null document', () => {
      assert.throws(() => new ExtractionManager(null), /Document is required/);
    });

    it('should extract text from page', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'Hello World',
      };
      const manager = new ExtractionManager(mockDoc);
      const text = manager.extractText(0);
      assert.strictEqual(text, 'Hello World');
    });

    it('should validate page index for extraction', () => {
      const mockDoc = { pageCount: 1, extractText: () => '' };
      const manager = new ExtractionManager(mockDoc);
      assert.throws(() => manager.extractText(-1), /non-negative number/);
      assert.throws(() => manager.extractText(10), /out of range/);
    });

    it('should extract all text', () => {
      const mockDoc = {
        pageCount: 2,
        extractText: () => 'text',
      };
      const manager = new ExtractionManager(mockDoc);
      const text = manager.extractAllText();
      assert.ok(typeof text === 'string');
    });

    it('should extract text range', () => {
      const mockDoc = {
        pageCount: 5,
        extractText: () => 'text',
      };
      const manager = new ExtractionManager(mockDoc);
      const text = manager.extractTextRange(0, 2);
      assert.ok(typeof text === 'string');
    });

    it('should get page word count', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'one two three',
      };
      const manager = new ExtractionManager(mockDoc);
      const count = manager.getPageWordCount(0);
      assert.strictEqual(count, 3);
    });

    it('should get total word count', () => {
      const mockDoc = {
        pageCount: 2,
        extractText: () => 'a b c',
      };
      const manager = new ExtractionManager(mockDoc);
      const count = manager.getTotalWordCount();
      assert.ok(typeof count === 'number');
    });

    it('should get page character count', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'hello',
      };
      const manager = new ExtractionManager(mockDoc);
      const count = manager.getPageCharacterCount(0);
      assert.strictEqual(count, 5);
    });

    it('should get total character count', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'hello',
      };
      const manager = new ExtractionManager(mockDoc);
      const count = manager.getTotalCharacterCount();
      assert.strictEqual(count, 5);
    });

    it('should get page line count', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'line1\nline2\nline3',
      };
      const manager = new ExtractionManager(mockDoc);
      const count = manager.getPageLineCount(0);
      assert.strictEqual(count, 3);
    });

    it('should get content statistics', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'hello world',
      };
      const manager = new ExtractionManager(mockDoc);
      const stats = manager.getContentStatistics();
      assert.ok(stats.pageCount > 0);
      assert.ok(stats.wordCount > 0);
      assert.ok(stats.characterCount > 0);
    });

    it('should search content', () => {
      const mockDoc = {
        pageCount: 1,
        extractText: () => 'test content with test keyword',
      };
      const manager = new ExtractionManager(mockDoc);
      const results = manager.searchContent('test');
      assert.ok(Array.isArray(results));
      assert.ok(results.length > 0);
    });
  });

  describe('SearchManager', () => {
    it('should create manager instance', () => {
      const mockDoc = { pageCount: 1, search: () => [] };
      const manager = new SearchManager(mockDoc);
      assert.ok(manager instanceof SearchManager);
    });

    it('should throw on null document', () => {
      assert.throws(() => new SearchManager(null), /Document is required/);
    });

    it('should search on page', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [],
      };
      const manager = new SearchManager(mockDoc);
      const results = manager.search('test', 0);
      assert.ok(Array.isArray(results));
    });

    it('should validate search parameters', () => {
      const mockDoc = { pageCount: 1, search: () => [] };
      const manager = new SearchManager(mockDoc);
      assert.throws(() => manager.search('', 0), /non-empty string/);
      assert.throws(() => manager.search('test', -1), /non-negative number/);
      assert.throws(() => manager.search('test', 10), /out of range/);
    });

    it('should search all pages', () => {
      const mockDoc = {
        pageCount: 2,
        search: () => [],
      };
      const manager = new SearchManager(mockDoc);
      const results = manager.searchAll('test');
      assert.ok(Array.isArray(results));
    });

    it('should count occurrences', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [1, 2, 3],
      };
      const manager = new SearchManager(mockDoc);
      const count = manager.countOccurrences('test', 0);
      assert.strictEqual(count, 3);
    });

    it('should count all occurrences', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [{ position: 0 }, { position: 10 }],
      };
      const manager = new SearchManager(mockDoc);
      const count = manager.countAllOccurrences('test');
      assert.strictEqual(count, 2);
    });

    it('should check contains', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [1],
      };
      const manager = new SearchManager(mockDoc);
      assert.ok(manager.contains('test', 0));
    });

    it('should check contains anywhere', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [{ position: 0 }],
      };
      const manager = new SearchManager(mockDoc);
      assert.ok(manager.containsAnywhere('test'));
    });

    it('should get pages containing text', () => {
      const mockDoc = {
        pageCount: 2,
        search: () => [{ position: 0 }],
      };
      const manager = new SearchManager(mockDoc);
      const pages = manager.getPagesContaining('test');
      assert.ok(Array.isArray(pages));
    });

    it('should get search statistics', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [],
      };
      const manager = new SearchManager(mockDoc);
      const stats = manager.getSearchStatistics('test');
      assert.ok(stats.searchText);
      assert.ok('totalOccurrences' in stats);
      assert.ok('pagesContaining' in stats);
    });

    it('should search with regex', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [],
      };
      const manager = new SearchManager(mockDoc);
      const results = manager.searchRegex(/test/i);
      assert.ok(Array.isArray(results));
    });

    it('should find first match', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [{ position: 0 }],
      };
      const manager = new SearchManager(mockDoc);
      const first = manager.findFirst('test');
      assert.ok(first !== null);
    });

    it('should find last match', () => {
      const mockDoc = {
        pageCount: 1,
        search: () => [{ position: 0 }],
      };
      const manager = new SearchManager(mockDoc);
      const last = manager.findLast('test');
      assert.ok(last !== null);
    });

    it('should get search capabilities', () => {
      const mockDoc = { pageCount: 1, search: () => [] };
      const manager = new SearchManager(mockDoc);
      const caps = manager.getCapabilities();
      assert.ok(caps.caseSensitiveSearch);
      assert.ok(caps.wholeWordSearch);
      assert.ok(caps.regexSearch);
    });
  });

  describe('SecurityManager', () => {
    it('should create manager instance', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.ok(manager instanceof SecurityManager);
    });

    it('should throw on null document', () => {
      assert.throws(() => new SecurityManager(null), /Document is required/);
    });

    it('should check is encrypted', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.isEncrypted(), false);
    });

    it('should check requires password', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.requiresPassword(), false);
    });

    it('should get encryption algorithm', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      const algo = manager.getEncryptionAlgorithm();
      assert.ok(algo === null || typeof algo === 'string');
    });

    it('should check can print', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.canPrint(), true);
    });

    it('should check can copy', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.canCopy(), true);
    });

    it('should check can modify', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.canModify(), true);
    });

    it('should check can annotate', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.canAnnotate(), true);
    });

    it('should check can fill forms', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.canFillForms(), true);
    });

    it('should check is view only', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      assert.strictEqual(manager.isViewOnly(), false);
    });

    it('should get permissions summary', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      const perms = manager.getPermissionsSummary();
      assert.ok(typeof perms === 'object');
      assert.ok('canPrint' in perms);
      assert.ok('canCopy' in perms);
      assert.ok('canModify' in perms);
    });

    it('should get security level', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      const level = manager.getSecurityLevel();
      assert.ok(level.level);
      assert.ok(level.description);
    });

    it('should validate accessibility', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      const validation = manager.validateAccessibility();
      assert.ok('canExtractText' in validation);
      assert.ok('isAccessible' in validation);
      assert.ok(Array.isArray(validation.issues));
    });

    it('should generate security report', () => {
      const mockDoc = {};
      const manager = new SecurityManager(mockDoc);
      const report = manager.generateSecurityReport();
      assert.ok(typeof report === 'string');
      assert.ok(report.includes('Security'));
    });
  });

  describe('AnnotationManager', () => {
    it('should create manager instance', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      assert.ok(manager instanceof AnnotationManager);
    });

    it('should throw on null page', () => {
      assert.throws(() => new AnnotationManager(null), /Page is required/);
    });

    it('should get annotations', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const annotations = manager.getAnnotations();
      assert.ok(Array.isArray(annotations));
    });

    it('should get annotations by type', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const annotations = manager.getAnnotationsByType('highlight');
      assert.ok(Array.isArray(annotations));
    });

    it('should validate annotation type', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      assert.throws(() => manager.getAnnotationsByType(''), /non-empty string/);
      assert.throws(() => manager.getAnnotationsByType('invalid'), /Invalid annotation type/);
    });

    it('should get annotation count', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      assert.strictEqual(manager.getAnnotationCount(), 0);
    });

    it('should get annotations by author', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const annotations = manager.getAnnotationsByAuthor('John');
      assert.ok(Array.isArray(annotations));
    });

    it('should get annotation authors', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const authors = manager.getAnnotationAuthors();
      assert.ok(Array.isArray(authors));
    });

    it('should get highlights', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const highlights = manager.getHighlights();
      assert.ok(Array.isArray(highlights));
    });

    it('should get comments', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const comments = manager.getComments();
      assert.ok(Array.isArray(comments));
    });

    it('should get underlines', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const underlines = manager.getUnderlines();
      assert.ok(Array.isArray(underlines));
    });

    it('should get annotation statistics', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const stats = manager.getAnnotationStatistics();
      assert.ok('total' in stats);
      assert.ok('byType' in stats);
      assert.ok('byAuthor' in stats);
    });

    it('should get recent annotations', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const recent = manager.getRecentAnnotations(7);
      assert.ok(Array.isArray(recent));
    });

    it('should validate days parameter', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      assert.throws(() => manager.getRecentAnnotations(-1), /non-negative number/);
    });

    it('should generate annotation summary', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const summary = manager.generateAnnotationSummary();
      assert.ok(typeof summary === 'string');
    });

    it('should validate annotation', () => {
      const mockPage = { pageIndex: 0 };
      const manager = new AnnotationManager(mockPage);
      const annotation = { type: 'highlight' };
      const validation = manager.validateAnnotation(annotation);
      assert.ok('isValid' in validation);
      assert.ok(Array.isArray(validation.issues));
    });
  });

  describe('Manager integration', () => {
    it('should work with all manager types', () => {
      const mockDoc = {
        pageCount: 1,
        hasStructureTree: false,
        documentInfo: {},
        extractText: () => 'test',
        search: () => [],
      };

      const managers = [
        new OutlineManager(mockDoc),
        new MetadataManager(mockDoc),
        new ExtractionManager(mockDoc),
        new SearchManager(mockDoc),
        new SecurityManager(mockDoc),
      ];

      managers.forEach((manager) => {
        assert.ok(manager instanceof Object);
      });
    });

    it('should chain manager operations', () => {
      const mockDoc = {
        pageCount: 1,
        documentInfo: { title: 'Test' },
      };

      const metadataManager = new MetadataManager(mockDoc);
      const title = metadataManager.getTitle();
      const keywords = metadataManager.getKeywords();
      const summary = metadataManager.getMetadataSummary();

      assert.strictEqual(title, 'Test');
      assert.ok(Array.isArray(keywords));
      assert.ok(typeof summary === 'string');
    });

    it('should handle manager errors gracefully', () => {
      const mockDoc = {
        pageCount: 0,
        documentInfo: null,
      };

      const metadataManager = new MetadataManager(mockDoc);
      const title = metadataManager.getTitle();
      const meta = metadataManager.getAllMetadata();

      assert.strictEqual(title, null);
      assert.ok(typeof meta === 'object');
    });
  });
});
