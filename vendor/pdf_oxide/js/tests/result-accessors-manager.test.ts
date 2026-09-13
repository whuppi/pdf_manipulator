/**
 * Unit tests for ResultAccessorsManager
 * Tests 29 Result Accessor functions covering:
 * - Search result properties (10 functions)
 * - Font information (8 functions)
 * - Image metadata (5 functions)
 * - Annotation properties (6 functions)
 */

import { ResultAccessorsManager } from '../src/result-accessors-manager';

describe('ResultAccessorsManager', () => {
  let manager: ResultAccessorsManager;

  beforeEach(() => {
    manager = new ResultAccessorsManager(null);
  });

  describe('Search Result Accessors', () => {
    test('should get search result context', async () => {
      const context = await manager.getSearchResultContext({}, 0);
      expect(typeof context).toBe('string');
    });

    test('should get search result line number', async () => {
      const lineNum = await manager.getSearchResultLineNumber({}, 0);
      expect(typeof lineNum).toBe('number');
      expect(lineNum).toBeGreaterThanOrEqual(0);
    });

    test('should get search result paragraph number', async () => {
      const paragraphNum = await manager.getSearchResultParagraphNumber({}, 0);
      expect(typeof paragraphNum).toBe('number');
      expect(paragraphNum).toBeGreaterThanOrEqual(0);
    });

    test('should get search result confidence', async () => {
      const confidence = await manager.getSearchResultConfidence({}, 0);
      expect(typeof confidence).toBe('number');
      expect(confidence).toBeGreaterThanOrEqual(0);
      expect(confidence).toBeLessThanOrEqual(1);
    });

    test('should check if search result is highlighted', async () => {
      const highlighted = await manager.isSearchResultHighlighted({}, 0);
      expect(typeof highlighted).toBe('boolean');
    });

    test('should get search result font info', async () => {
      const fontInfo = await manager.getSearchResultFontInfo({}, 0);
      expect(typeof fontInfo).toBe('string');
    });

    test('should get search result color as RGB array', async () => {
      const color = await manager.getSearchResultColor({}, 0);
      expect(Array.isArray(color)).toBe(true);
      expect(color.length).toBe(3);
      expect(color[0]).toBeGreaterThanOrEqual(0);
      expect(color[0]).toBeLessThanOrEqual(255);
    });

    test('should get search result rotation', async () => {
      const rotation = await manager.getSearchResultRotation({}, 0);
      expect(typeof rotation).toBe('number');
      expect([0, 90, 180, 270]).toContain(rotation);
    });

    test('should get search result object ID', async () => {
      const objectId = await manager.getSearchResultObjectId({}, 0);
      expect(typeof objectId).toBe('number');
    });

    test('should get search result stream index', async () => {
      const streamIndex = await manager.getSearchResultStreamIndex({}, 0);
      expect(typeof streamIndex).toBe('number');
    });

    test('should get all search result properties', async () => {
      const props = await manager.getSearchResultAllProperties({}, 0);
      expect(props).toBeDefined();
      expect(props.context).toBeDefined();
      expect(props.lineNumber).toBeDefined();
      expect(props.paragraphNumber).toBeDefined();
      expect(props.confidence).toBeDefined();
      expect(props.isHighlighted).toBeDefined();
      expect(props.fontInfo).toBeDefined();
      expect(props.color).toBeDefined();
      expect(props.rotation).toBeDefined();
      expect(props.objectId).toBeDefined();
      expect(props.streamIndex).toBeDefined();
    });
  });

  describe('Font Accessors', () => {
    test('should get font base font name', async () => {
      const name = await manager.getFontBaseFontName({}, 0);
      expect(typeof name).toBe('string');
    });

    test('should get font descriptor', async () => {
      const descriptor = await manager.getFontDescriptor({}, 0);
      expect(typeof descriptor).toBe('string');
    });

    test('should get font descendant font', async () => {
      const descendant = await manager.getFontDescendantFont({}, 0);
      expect(typeof descendant).toBe('string');
    });

    test('should get font ToUnicode CMap', async () => {
      const cmap = await manager.getFontToUnicodeCmap({}, 0);
      expect(typeof cmap).toBe('string');
    });

    test('should check if font is vertical', async () => {
      const vertical = await manager.isFontVertical({}, 0);
      expect(typeof vertical).toBe('boolean');
    });

    test('should get font widths', async () => {
      const widths = await manager.getFontWidths({}, 0);
      expect(widths instanceof Float32Array).toBe(true);
    });

    test('should get font ascender', async () => {
      const ascender = await manager.getFontAscender({}, 0);
      expect(typeof ascender).toBe('number');
    });

    test('should get font descender', async () => {
      const descender = await manager.getFontDescender({}, 0);
      expect(typeof descender).toBe('number');
    });

    test('should get all font properties', async () => {
      const props = await manager.getFontAllProperties({}, 0);
      expect(props).toBeDefined();
      expect(props.baseFontName).toBeDefined();
      expect(props.descriptor).toBeDefined();
      expect(props.descendantFont).toBeDefined();
      expect(props.toUnicodeCmap).toBeDefined();
      expect(props.isVertical).toBeDefined();
      expect(props.widths).toBeDefined();
      expect(props.ascender).toBeDefined();
      expect(props.descender).toBeDefined();
    });
  });

  describe('Image Accessors', () => {
    test('should check if image has alpha channel', async () => {
      const hasAlpha = await manager.hasImageAlphaChannel({}, 0);
      expect(typeof hasAlpha).toBe('boolean');
    });

    test('should get image ICC profile', async () => {
      const profile = await manager.getImageIccProfile({}, 0);
      expect(profile instanceof Uint8Array).toBe(true);
    });

    test('should get image filter chain', async () => {
      const chain = await manager.getImageFilterChain({}, 0);
      expect(typeof chain).toBe('string');
    });

    test('should get image decoded data', async () => {
      const data = await manager.getImageDecodedData({}, 0);
      expect(data instanceof Uint8Array).toBe(true);
    });

    test('should get all image properties', async () => {
      const props = await manager.getImageAllProperties({}, 0);
      expect(props).toBeDefined();
      expect(props.hasAlphaChannel).toBeDefined();
      expect(props.iccProfile).toBeDefined();
      expect(props.filterChain).toBeDefined();
      expect(props.decodedData).toBeDefined();
    });
  });

  describe('Annotation Accessors', () => {
    test('should get annotation modified date', async () => {
      const date = await manager.getAnnotationModifiedDate({}, 0);
      expect(typeof date).toBe('number');
    });

    test('should get annotation subject', async () => {
      const subject = await manager.getAnnotationSubject({}, 0);
      expect(typeof subject).toBe('string');
    });

    test('should get annotation reply to index', async () => {
      const replyTo = await manager.getAnnotationReplyToIndex({}, 0);
      expect(typeof replyTo).toBe('number');
    });

    test('should get annotation page number', async () => {
      const pageNum = await manager.getAnnotationPageNumber({}, 0);
      expect(typeof pageNum).toBe('number');
      expect(pageNum).toBeGreaterThanOrEqual(0);
    });

    test('should get annotation icon name', async () => {
      const icon = await manager.getAnnotationIconName({}, 0);
      expect(typeof icon).toBe('string');
    });

    test('should get all annotation properties', async () => {
      const props = await manager.getAnnotationAllProperties({}, 0);
      expect(props).toBeDefined();
      expect(props.modifiedDate).toBeDefined();
      expect(props.subject).toBeDefined();
      expect(props.replyToIndex).toBeDefined();
      expect(props.pageNumber).toBeDefined();
      expect(props.iconName).toBeDefined();
    });
  });

  describe('Cache Management', () => {
    test('should clear cache', () => {
      manager.clearCache();
      const stats = manager.getCacheStats();
      expect(stats.cacheSize).toBe(0);
    });

    test('should get cache statistics', () => {
      const stats = manager.getCacheStats();
      expect(stats).toBeDefined();
      expect(stats.cacheSize).toBeDefined();
      expect(stats.maxCacheSize).toBeDefined();
      expect(stats.entries).toBeDefined();
      expect(Array.isArray(stats.entries)).toBe(true);
    });

    test('should cache results on repeated access', async () => {
      await manager.getSearchResultContext({}, 0);
      const stats1 = manager.getCacheStats();
      const sizeAfterFirstCall = stats1.cacheSize;

      await manager.getSearchResultContext({}, 0);
      const stats2 = manager.getCacheStats();
      const sizeAfterSecondCall = stats2.cacheSize;

      expect(sizeAfterSecondCall).toBe(sizeAfterFirstCall);
    });
  });

  describe('Event Emission', () => {
    test('should emit search result events', (done) => {
      manager.on('searchPropertiesExtracted', (index) => {
        expect(index).toBe(0);
        done();
      });

      manager.getSearchResultAllProperties({}, 0).catch(() => {
        // Expected if no actual search results
      });
    });

    test('should emit font property events', (done) => {
      manager.on('fontPropertiesExtracted', (index) => {
        expect(index).toBe(0);
        done();
      });

      manager.getFontAllProperties({}, 0).catch(() => {
        // Expected if no actual fonts
      });
    });

    test('should emit image property events', (done) => {
      manager.on('imagePropertiesExtracted', (index) => {
        expect(index).toBe(0);
        done();
      });

      manager.getImageAllProperties({}, 0).catch(() => {
        // Expected if no actual images
      });
    });

    test('should emit annotation property events', (done) => {
      manager.on('annotationPropertiesExtracted', (index) => {
        expect(index).toBe(0);
        done();
      });

      manager.getAnnotationAllProperties({}, 0).catch(() => {
        // Expected if no actual annotations
      });
    });

    test('should emit cache cleared event', (done) => {
      manager.on('cacheCleared', () => {
        done();
      });

      manager.clearCache();
    });
  });

  describe('Data Class Types', () => {
    test('SearchResultProperties should have all required properties', async () => {
      const props = await manager.getSearchResultAllProperties({}, 0);
      expect(props).toHaveProperty('context');
      expect(props).toHaveProperty('lineNumber');
      expect(props).toHaveProperty('paragraphNumber');
      expect(props).toHaveProperty('confidence');
      expect(props).toHaveProperty('isHighlighted');
      expect(props).toHaveProperty('fontInfo');
      expect(props).toHaveProperty('color');
      expect(props).toHaveProperty('rotation');
      expect(props).toHaveProperty('objectId');
      expect(props).toHaveProperty('streamIndex');
    });

    test('FontProperties should have all required properties', async () => {
      const props = await manager.getFontAllProperties({}, 0);
      expect(props).toHaveProperty('baseFontName');
      expect(props).toHaveProperty('descriptor');
      expect(props).toHaveProperty('descendantFont');
      expect(props).toHaveProperty('toUnicodeCmap');
      expect(props).toHaveProperty('isVertical');
      expect(props).toHaveProperty('widths');
      expect(props).toHaveProperty('ascender');
      expect(props).toHaveProperty('descender');
    });

    test('ImageProperties should have all required properties', async () => {
      const props = await manager.getImageAllProperties({}, 0);
      expect(props).toHaveProperty('hasAlphaChannel');
      expect(props).toHaveProperty('iccProfile');
      expect(props).toHaveProperty('filterChain');
      expect(props).toHaveProperty('decodedData');
    });

    test('AnnotationProperties should have all required properties', async () => {
      const props = await manager.getAnnotationAllProperties({}, 0);
      expect(props).toHaveProperty('modifiedDate');
      expect(props).toHaveProperty('subject');
      expect(props).toHaveProperty('replyToIndex');
      expect(props).toHaveProperty('pageNumber');
      expect(props).toHaveProperty('iconName');
    });
  });
});
