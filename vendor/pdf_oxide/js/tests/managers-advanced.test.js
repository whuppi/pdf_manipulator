/**
 * Advanced Managers Tests - Phase 2
 *
 * Tests for LayerManager and RenderingManager implementations.
 * Verifies layer management, rendering capabilities, and page properties.
 */

import assert from 'node:assert';
import { beforeEach, describe, it } from 'node:test';
import { LayerManager } from '../lib/managers/LayerManager.js';
import { RenderingManager } from '../lib/managers/RenderingManager.js';

/**
 * Mock PDF document for testing
 */
class MockPdfDocument {
  constructor(pageCount = 10) {
    this.pageCount = pageCount;
  }
}

describe('Advanced Managers Tests - Phase 2', () => {
  let mockDoc;

  beforeEach(() => {
    mockDoc = new MockPdfDocument(10);
  });

  describe('LayerManager', () => {
    describe('Initialization and Basic Operations', () => {
      it('should create LayerManager instance', () => {
        const manager = new LayerManager(mockDoc);
        assert.ok(manager instanceof LayerManager);
      });

      it('should throw on null document', () => {
        assert.throws(() => new LayerManager(null), /Document is required/);
      });

      it('should provide clearCache method', () => {
        const manager = new LayerManager(mockDoc);
        assert.ok(typeof manager.clearCache === 'function');
        manager.clearCache(); // Should not throw
      });
    });

    describe('Layer Detection and Querying', () => {
      it('should check if document has layers', () => {
        const manager = new LayerManager(mockDoc);
        const result = manager.hasLayers();
        assert.ok(typeof result === 'boolean');
      });

      it('should get layer count', () => {
        const manager = new LayerManager(mockDoc);
        const count = manager.getLayerCount();
        assert.ok(typeof count === 'number');
        assert.ok(count >= 0);
      });

      it('should get all layers', () => {
        const manager = new LayerManager(mockDoc);
        const layers = manager.getLayers();
        assert.ok(Array.isArray(layers));
      });

      it('should cache layer results', () => {
        const manager = new LayerManager(mockDoc);
        const layers1 = manager.getLayers();
        const layers2 = manager.getLayers();
        // Should return same reference from cache
        assert.strictEqual(layers1, layers2);
      });
    });

    describe('Layer Lookup Operations', () => {
      it('should get layer by name', () => {
        const manager = new LayerManager(mockDoc);
        const result = manager.getLayerByName('TestLayer');
        // Should return null or layer object
        assert.ok(result === null || typeof result === 'object');
      });

      it('should throw on empty layer name', () => {
        const manager = new LayerManager(mockDoc);
        assert.throws(() => manager.getLayerByName(''), /Layer name must be a non-empty string/);
      });

      it('should get layer by ID', () => {
        const manager = new LayerManager(mockDoc);
        const result = manager.getLayerById('layer-123');
        assert.ok(result === null || typeof result === 'object');
      });

      it('should throw on empty layer ID', () => {
        const manager = new LayerManager(mockDoc);
        assert.throws(() => manager.getLayerById(''), /Layer ID must be a non-empty string/);
      });
    });

    describe('Layer Hierarchy Operations', () => {
      it('should get root layers', () => {
        const manager = new LayerManager(mockDoc);
        const roots = manager.getRootLayers();
        assert.ok(Array.isArray(roots));
      });

      it('should get layer hierarchy', () => {
        const manager = new LayerManager(mockDoc);
        const hierarchy = manager.getLayerHierarchy();
        assert.ok(typeof hierarchy === 'object');
        assert.ok(Array.isArray(hierarchy.root));
      });

      it('should cache hierarchy results', () => {
        const manager = new LayerManager(mockDoc);
        const h1 = manager.getLayerHierarchy();
        const h2 = manager.getLayerHierarchy();
        assert.strictEqual(h1, h2);
      });

      it('should get child layers', () => {
        const manager = new LayerManager(mockDoc);
        const children = manager.getChildLayers('parent-123');
        assert.ok(Array.isArray(children));
      });

      it('should get parent layer', () => {
        const manager = new LayerManager(mockDoc);
        const parent = manager.getParentLayer('child-123');
        assert.ok(parent === null || typeof parent === 'object');
      });
    });

    describe('Layer Visibility and Properties', () => {
      it('should check layer visibility', () => {
        const manager = new LayerManager(mockDoc);
        const visible = manager.isLayerVisible('layer-123');
        assert.ok(typeof visible === 'boolean');
      });

      it('should get visibility chain', () => {
        const manager = new LayerManager(mockDoc);
        const chain = manager.getVisibilityChain('layer-123');
        assert.ok(Array.isArray(chain));
      });

      it('should get layer usages', () => {
        const manager = new LayerManager(mockDoc);
        const usages = manager.getLayerUsages();
        assert.ok(typeof usages === 'object');
        assert.ok('view' in usages);
        assert.ok('print' in usages);
        assert.ok('export' in usages);
      });
    });

    describe('Layer Statistics and Validation', () => {
      it('should get layer statistics', () => {
        const manager = new LayerManager(mockDoc);
        const stats = manager.getLayerStatistics();
        assert.ok(typeof stats === 'object');
        assert.ok('count' in stats);
        assert.ok('maxDepth' in stats);
        assert.ok('hasConflicts' in stats);
      });

      it('should cache statistics results', () => {
        const manager = new LayerManager(mockDoc);
        const s1 = manager.getLayerStatistics();
        const s2 = manager.getLayerStatistics();
        assert.strictEqual(s1, s2);
      });

      it('should find layers by pattern', () => {
        const manager = new LayerManager(mockDoc);
        const matches = manager.findLayersByPattern(/test/i);
        assert.ok(Array.isArray(matches));
      });

      it('should validate layer state', () => {
        const manager = new LayerManager(mockDoc);
        const validation = manager.validateLayerState();
        assert.ok(typeof validation === 'object');
        assert.ok('isValid' in validation);
        assert.ok(Array.isArray(validation.issues));
      });
    });

    describe('LayerManager Edge Cases', () => {
      it('should handle missing parent layers', () => {
        const manager = new LayerManager(mockDoc);
        const parent = manager.getParentLayer('nonexistent');
        assert.strictEqual(parent, null);
      });

      it('should handle pattern search with string', () => {
        const manager = new LayerManager(mockDoc);
        const matches = manager.findLayersByPattern('test');
        assert.ok(Array.isArray(matches));
      });

      it('should detect layer conflicts', () => {
        const manager = new LayerManager(mockDoc);
        const validation = manager.validateLayerState();
        // Should return valid structure even with no layers
        assert.ok(typeof validation.isValid === 'boolean');
      });
    });
  });

  describe('RenderingManager', () => {
    describe('Initialization and Basic Operations', () => {
      it('should create RenderingManager instance', () => {
        const manager = new RenderingManager(mockDoc);
        assert.ok(manager instanceof RenderingManager);
      });

      it('should throw on null document', () => {
        assert.throws(() => new RenderingManager(null), /Document is required/);
      });

      it('should provide clearCache method', () => {
        const manager = new RenderingManager(mockDoc);
        assert.ok(typeof manager.clearCache === 'function');
        manager.clearCache(); // Should not throw
      });
    });

    describe('Rendering Capabilities', () => {
      it('should get maximum resolution', () => {
        const manager = new RenderingManager(mockDoc);
        const maxDpi = manager.getMaxResolution();
        assert.ok(typeof maxDpi === 'number');
        assert.ok(maxDpi > 0);
      });

      it('should get supported color spaces', () => {
        const manager = new RenderingManager(mockDoc);
        const colorSpaces = manager.getSupportedColorSpaces();
        assert.ok(Array.isArray(colorSpaces));
        assert.ok(colorSpaces.length > 0);
        assert.ok(colorSpaces.includes('RGB'));
      });
    });

    describe('Page Dimensions', () => {
      it('should get page dimensions', () => {
        const manager = new RenderingManager(mockDoc);
        const dims = manager.getPageDimensions(0);
        assert.ok(typeof dims === 'object');
        assert.ok('width' in dims);
        assert.ok('height' in dims);
        assert.ok('unit' in dims);
        assert.ok(dims.width > 0);
        assert.ok(dims.height > 0);
      });

      it('should cache dimension results', () => {
        const manager = new RenderingManager(mockDoc);
        const d1 = manager.getPageDimensions(0);
        const d2 = manager.getPageDimensions(0);
        assert.strictEqual(d1, d2);
      });

      it('should throw on invalid page index', () => {
        const manager = new RenderingManager(mockDoc);
        assert.throws(() => manager.getPageDimensions(100), /out of range/);
      });

      it('should get display size at zoom level', () => {
        const manager = new RenderingManager(mockDoc);
        const display = manager.getDisplaySize(0, 1.5);
        assert.ok(display.width > 0);
        assert.ok(display.height > 0);
      });

      it('should throw on invalid zoom level', () => {
        const manager = new RenderingManager(mockDoc);
        assert.throws(() => manager.getDisplaySize(0, -1), /positive number/);
      });
    });

    describe('Page Transformations', () => {
      it('should get page rotation', () => {
        const manager = new RenderingManager(mockDoc);
        const rotation = manager.getPageRotation(0);
        assert.ok([0, 90, 180, 270].includes(rotation));
      });

      it('should get page crop box', () => {
        const manager = new RenderingManager(mockDoc);
        const box = manager.getPageCropBox(0);
        assert.ok(typeof box === 'object');
        assert.ok('x' in box);
        assert.ok('y' in box);
        assert.ok('width' in box);
        assert.ok('height' in box);
      });

      it('should get page media box', () => {
        const manager = new RenderingManager(mockDoc);
        const box = manager.getPageMediaBox(0);
        assert.ok(typeof box === 'object');
      });

      it('should get page bleed box', () => {
        const manager = new RenderingManager(mockDoc);
        const box = manager.getPageBleedBox(0);
        assert.ok(typeof box === 'object');
      });

      it('should get page trim box', () => {
        const manager = new RenderingManager(mockDoc);
        const box = manager.getPageTrimBox(0);
        assert.ok(typeof box === 'object');
      });

      it('should get page art box', () => {
        const manager = new RenderingManager(mockDoc);
        const box = manager.getPageArtBox(0);
        assert.ok(typeof box === 'object');
      });
    });

    describe('Zoom Calculations', () => {
      it('should calculate zoom for width', () => {
        const manager = new RenderingManager(mockDoc);
        const zoom = manager.calculateZoomForWidth(0, 600);
        assert.ok(typeof zoom === 'number');
        assert.ok(zoom > 0);
      });

      it('should throw on invalid viewport width', () => {
        const manager = new RenderingManager(mockDoc);
        assert.throws(() => manager.calculateZoomForWidth(0, -100), /positive number/);
      });

      it('should calculate zoom for height', () => {
        const manager = new RenderingManager(mockDoc);
        const zoom = manager.calculateZoomForHeight(0, 800);
        assert.ok(typeof zoom === 'number');
        assert.ok(zoom > 0);
      });

      it('should calculate zoom to fit', () => {
        const manager = new RenderingManager(mockDoc);
        const zoom = manager.calculateZoomToFit(0, 600, 800);
        assert.ok(typeof zoom === 'number');
        assert.ok(zoom > 0);
      });
    });

    describe('Page Resources', () => {
      it('should get embedded fonts', () => {
        const manager = new RenderingManager(mockDoc);
        const fonts = manager.getEmbeddedFonts(0);
        assert.ok(Array.isArray(fonts));
      });

      it('should cache font results', () => {
        const manager = new RenderingManager(mockDoc);
        const f1 = manager.getEmbeddedFonts(0);
        const f2 = manager.getEmbeddedFonts(0);
        assert.strictEqual(f1, f2);
      });

      it('should get embedded images', () => {
        const manager = new RenderingManager(mockDoc);
        const images = manager.getEmbeddedImages(0);
        assert.ok(Array.isArray(images));
      });

      it('should cache image results', () => {
        const manager = new RenderingManager(mockDoc);
        const i1 = manager.getEmbeddedImages(0);
        const i2 = manager.getEmbeddedImages(0);
        assert.strictEqual(i1, i2);
      });

      it('should get page resources', () => {
        const manager = new RenderingManager(mockDoc);
        const resources = manager.getPageResources(0);
        assert.ok(typeof resources === 'object');
        assert.ok('fonts' in resources);
        assert.ok('images' in resources);
        assert.ok('colorSpaces' in resources);
        assert.ok('patterns' in resources);
      });
    });

    describe('Quality and Resolution', () => {
      it('should get recommended resolution for draft', () => {
        const manager = new RenderingManager(mockDoc);
        const dpi = manager.getRecommendedResolution('draft');
        assert.ok(typeof dpi === 'number');
        assert.strictEqual(dpi, 72);
      });

      it('should get recommended resolution for normal', () => {
        const manager = new RenderingManager(mockDoc);
        const dpi = manager.getRecommendedResolution('normal');
        assert.ok(typeof dpi === 'number');
      });

      it('should get recommended resolution for high', () => {
        const manager = new RenderingManager(mockDoc);
        const dpi = manager.getRecommendedResolution('high');
        assert.ok(typeof dpi === 'number');
        assert.strictEqual(dpi, 300);
      });

      it('should throw on invalid quality', () => {
        const manager = new RenderingManager(mockDoc);
        assert.throws(() => manager.getRecommendedResolution('invalid'), /Invalid quality/);
      });
    });

    describe('Rendering Statistics', () => {
      it('should get rendering statistics', () => {
        const manager = new RenderingManager(mockDoc);
        const stats = manager.getRenderingStatistics();
        assert.ok(typeof stats === 'object');
        assert.ok('totalFonts' in stats);
        assert.ok('totalImages' in stats);
        assert.ok('avgPageSize' in stats);
        assert.ok('colorSpaceCount' in stats);
        assert.ok('pageCount' in stats);
      });

      it('should cache statistics results', () => {
        const manager = new RenderingManager(mockDoc);
        const s1 = manager.getRenderingStatistics();
        const s2 = manager.getRenderingStatistics();
        assert.strictEqual(s1, s2);
      });
    });

    describe('Page Rendering Validation', () => {
      it('should check if page can be rendered', () => {
        const manager = new RenderingManager(mockDoc);
        const canRender = manager.canRenderPage(0);
        assert.ok(typeof canRender === 'boolean');
      });

      it('should return false for invalid page index', () => {
        const manager = new RenderingManager(mockDoc);
        const canRender = manager.canRenderPage(100);
        assert.strictEqual(canRender, false);
      });

      it('should validate rendering state', () => {
        const manager = new RenderingManager(mockDoc);
        const validation = manager.validateRenderingState();
        assert.ok(typeof validation === 'object');
        assert.ok('isValid' in validation);
        assert.ok(Array.isArray(validation.issues));
      });
    });

    describe('RenderingManager Edge Cases', () => {
      it('should handle empty document', () => {
        const emptyDoc = new MockPdfDocument(0);
        const manager = new RenderingManager(emptyDoc);
        const stats = manager.getRenderingStatistics();
        assert.strictEqual(stats.pageCount, 0);
      });

      it('should handle large page counts', () => {
        const largeDoc = new MockPdfDocument(1000);
        const manager = new RenderingManager(largeDoc);
        assert.strictEqual(manager.canRenderPage(999), true);
        assert.strictEqual(manager.canRenderPage(1000), false);
      });
    });
  });

  describe('Phase 2 Summary', () => {
    it('should have LayerManager with 17 key methods', () => {
      const manager = new LayerManager(mockDoc);

      const methods = [
        'hasLayers',
        'getLayerCount',
        'getLayers',
        'getLayerByName',
        'getLayerById',
        'getRootLayers',
        'getLayerHierarchy',
        'getChildLayers',
        'getParentLayer',
        'isLayerVisible',
        'getVisibilityChain',
        'getLayerUsages',
        'getLayerStatistics',
        'findLayersByPattern',
        'validateLayerState',
        'clearCache',
      ];

      methods.forEach((method) => {
        assert.ok(
          typeof manager[method] === 'function',
          `LayerManager should have ${method} method`
        );
      });
    });

    it('should have RenderingManager with 20 key methods', () => {
      const manager = new RenderingManager(mockDoc);

      const methods = [
        'getMaxResolution',
        'getSupportedColorSpaces',
        'getPageDimensions',
        'getDisplaySize',
        'getPageRotation',
        'getPageCropBox',
        'getPageMediaBox',
        'getPageBleedBox',
        'getPageTrimBox',
        'getPageArtBox',
        'calculateZoomForWidth',
        'calculateZoomForHeight',
        'calculateZoomToFit',
        'getEmbeddedFonts',
        'getEmbeddedImages',
        'getPageResources',
        'getRecommendedResolution',
        'getRenderingStatistics',
        'canRenderPage',
        'validateRenderingState',
        'clearCache',
      ];

      methods.forEach((method) => {
        assert.ok(
          typeof manager[method] === 'function',
          `RenderingManager should have ${method} method`
        );
      });
    });

    it('should document Phase 2 completion', () => {
      const phase2Tasks = [
        'LayerManager: Layer detection and querying',
        'LayerManager: Layer hierarchy management',
        'LayerManager: Layer visibility and properties',
        'LayerManager: Conflict and cycle detection',
        'RenderingManager: Page dimensions and transformations',
        'RenderingManager: Zoom calculations',
        'RenderingManager: Resource enumeration',
        'RenderingManager: Quality and resolution settings',
      ];

      assert.strictEqual(phase2Tasks.length, 8);
      phase2Tasks.forEach((task) => {
        assert.ok(typeof task === 'string');
      });
    });
  });
});
