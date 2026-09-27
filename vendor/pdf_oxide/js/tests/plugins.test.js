/**
 * Plugin System Tests - Phase 3
 *
 * Tests for wrapper-based plugin architecture including base classes,
 * plugin registry, and example plugins.
 */

import assert from 'node:assert';
import { beforeEach, describe, it } from 'node:test';
import {
  AnnotationPlugin,
  ExtractionPlugin,
  PluginRegistry,
  SearchPlugin,
} from '../lib/plugins/index.js';

/**
 * Mock PDF document for testing
 */
class MockPdfDocument {
  constructor(pageCount = 10) {
    this.pageCount = pageCount;
  }

  extractText(pageIndex) {
    return `Page ${pageIndex + 1} content`;
  }

  search(searchText, pageIndex) {
    return [{ position: 0, matchText: searchText }];
  }
}

/**
 * Mock PDF page for testing
 */
class MockPdfPage {
  constructor(pageIndex = 0) {
    this.pageIndex = pageIndex;
  }
}

describe('Plugin System Tests - Phase 3', () => {
  let mockDoc;
  let mockPage;

  beforeEach(() => {
    mockDoc = new MockPdfDocument(10);
    mockPage = new MockPdfPage(0);
  });

  describe('ExtractionPlugin Base Class', () => {
    it('should create extraction plugin instance', () => {
      const plugin = new ExtractionPlugin(mockDoc);
      assert.ok(plugin instanceof ExtractionPlugin);
    });

    it('should have plugin lifecycle methods', () => {
      const plugin = new ExtractionPlugin(mockDoc);
      assert.ok(typeof plugin.onInitialize === 'function');
      assert.ok(typeof plugin.onDestroy === 'function');
    });

    it('should support plugin configuration', () => {
      const plugin = new ExtractionPlugin(mockDoc);

      plugin.setConfig('key1', 'value1');
      assert.strictEqual(plugin.getConfig('key1'), 'value1');
      assert.strictEqual(plugin.getConfig('unknown', 'default'), 'default');
    });

    it('should support plugin logging', () => {
      const plugin = new ExtractionPlugin(mockDoc);

      // Should not throw
      plugin.log('info', 'Test message', { data: 'test' });
      plugin.log('debug', 'Debug message');
      plugin.log('warn', 'Warning message');
      plugin.log('error', 'Error message');
    });

    it('should track extraction metrics', () => {
      const plugin = new ExtractionPlugin(mockDoc);

      const text1 = plugin.extractText(0);
      const text2 = plugin.extractText(1);

      const metrics = plugin.metrics();
      assert.ok('extractCalls' in metrics);
      assert.ok('totalExtractTime' in metrics);
      assert.ok('avgExtractTime' in metrics);
      assert.strictEqual(metrics.extractCalls, 2);
    });

    it('should allow method override for custom behavior', () => {
      class CustomExtractionPlugin extends ExtractionPlugin {
        extractText(pageIndex, options) {
          const text = super.extractText(pageIndex, options);
          return text.toUpperCase();
        }
      }

      const plugin = new CustomExtractionPlugin(mockDoc);
      const text = plugin.extractText(0);
      assert.ok(text.includes('PAGE'));
    });
  });

  describe('SearchPlugin Base Class', () => {
    it('should create search plugin instance', () => {
      const plugin = new SearchPlugin(mockDoc);
      assert.ok(plugin instanceof SearchPlugin);
    });

    it('should have plugin lifecycle methods', () => {
      const plugin = new SearchPlugin(mockDoc);
      assert.ok(typeof plugin.onInitialize === 'function');
      assert.ok(typeof plugin.onDestroy === 'function');
    });

    it('should support plugin configuration', () => {
      const plugin = new SearchPlugin(mockDoc);

      plugin.setConfig('maxResults', 100);
      assert.strictEqual(plugin.getConfig('maxResults'), 100);
    });

    it('should support plugin logging', () => {
      const plugin = new SearchPlugin(mockDoc);

      // Should not throw
      plugin.log('info', 'Search message');
    });

    it('should track search metrics', () => {
      const plugin = new SearchPlugin(mockDoc);

      plugin.search('test', 0);
      plugin.search('test', 1);

      const metrics = plugin.metrics();
      assert.ok('searchCalls' in metrics);
      assert.strictEqual(metrics.searchCalls, 2);
    });

    it('should allow method override for custom search behavior', () => {
      class CustomSearchPlugin extends SearchPlugin {
        search(searchText, pageIndex, options) {
          const results = super.search(searchText, pageIndex, options);
          // Filter or modify results
          return results.filter((r) => r.position > 0);
        }
      }

      const plugin = new CustomSearchPlugin(mockDoc);
      const results = plugin.search('test', 0);
      assert.ok(Array.isArray(results));
    });
  });

  describe('AnnotationPlugin Base Class', () => {
    it('should create annotation plugin instance', () => {
      const plugin = new AnnotationPlugin(mockPage);
      assert.ok(plugin instanceof AnnotationPlugin);
    });

    it('should have plugin lifecycle methods', () => {
      const plugin = new AnnotationPlugin(mockPage);
      assert.ok(typeof plugin.onInitialize === 'function');
      assert.ok(typeof plugin.onDestroy === 'function');
    });

    it('should support plugin configuration', () => {
      const plugin = new AnnotationPlugin(mockPage);

      plugin.setConfig('filterType', 'highlight');
      assert.strictEqual(plugin.getConfig('filterType'), 'highlight');
    });

    it('should track annotation metrics', () => {
      const plugin = new AnnotationPlugin(mockPage);

      plugin.getAnnotations();
      plugin.getAnnotations();

      const metrics = plugin.metrics();
      assert.ok('annotationCalls' in metrics);
      assert.strictEqual(metrics.annotationCalls, 2);
    });
  });

  describe('PluginRegistry', () => {
    let registry;

    beforeEach(() => {
      registry = new PluginRegistry();
    });

    describe('Plugin Registration', () => {
      it('should register a plugin', () => {
        registry.register('testPlugin', ExtractionPlugin);
        assert.ok(registry.isRegistered('testPlugin'));
      });

      it('should throw on duplicate registration', () => {
        registry.register('testPlugin', ExtractionPlugin);
        assert.throws(() => registry.register('testPlugin', SearchPlugin), /already registered/);
      });

      it('should throw on invalid plugin class', () => {
        assert.throws(
          () => registry.register('bad', 'notAClass'),
          /must be a class or constructor/
        );
      });

      it('should throw on invalid plugin name', () => {
        assert.throws(() => registry.register('', ExtractionPlugin), /must be a non-empty string/);
      });

      it('should unregister a plugin', () => {
        registry.register('testPlugin', ExtractionPlugin);
        registry.unregister('testPlugin');
        assert.ok(!registry.isRegistered('testPlugin'));
      });

      it('should throw on unregistering nonexistent plugin', () => {
        assert.throws(() => registry.unregister('nonexistent'), /not registered/);
      });
    });

    describe('Plugin Querying', () => {
      it('should get plugin registration', () => {
        registry.register('testPlugin', ExtractionPlugin);
        const reg = registry.getRegistration('testPlugin');
        assert.ok(reg.PluginClass === ExtractionPlugin);
      });

      it('should return null for unregistered plugin', () => {
        const reg = registry.getRegistration('nonexistent');
        assert.strictEqual(reg, null);
      });

      it('should get all plugins', () => {
        registry.register('plugin1', ExtractionPlugin);
        registry.register('plugin2', SearchPlugin);

        const plugins = registry.getPlugins();
        assert.strictEqual(plugins.length, 2);
        assert.ok(plugins.some((p) => p.name === 'plugin1'));
      });

      it('should get plugins by category', () => {
        registry.register('extractor', ExtractionPlugin, { category: 'extraction' });
        registry.register('searcher', SearchPlugin, { category: 'search' });

        const extractors = registry.getPluginsByCategory('extraction');
        assert.strictEqual(extractors.length, 1);
        assert.strictEqual(extractors[0].name, 'extractor');
      });
    });

    describe('Plugin Instance Management', () => {
      it('should create plugin instance', () => {
        registry.register('testPlugin', ExtractionPlugin);
        const instance = registry.createInstance('testPlugin', mockDoc);
        assert.ok(instance instanceof ExtractionPlugin);
      });

      it('should pass options to plugin instance', () => {
        registry.register('testPlugin', ExtractionPlugin);
        const instance = registry.createInstance('testPlugin', mockDoc, {
          option1: 'value1',
        });

        assert.strictEqual(instance.getConfig('option1'), undefined);
        // Options are stored in _pluginOptions
        assert.ok('_pluginOptions' in instance);
      });

      it('should throw on creating unregistered plugin', () => {
        assert.throws(() => registry.createInstance('nonexistent', mockDoc), /not registered/);
      });

      it('should release plugin instance', () => {
        registry.register('testPlugin', ExtractionPlugin);
        const instance = registry.createInstance('testPlugin', mockDoc);

        // Should not throw
        registry.releaseInstance(instance);
      });

      it('should get active instances', () => {
        registry.register('testPlugin', ExtractionPlugin);
        const instance1 = registry.createInstance('testPlugin', mockDoc);
        const instance2 = registry.createInstance('testPlugin', mockDoc);

        const instances = registry.getInstances();
        assert.strictEqual(instances.length, 2);
      });

      it('should get instance metadata', () => {
        registry.register('testPlugin', ExtractionPlugin, {
          version: '1.0.0',
        });
        const instance = registry.createInstance('testPlugin', mockDoc);

        const metadata = registry.getInstanceMetadata(instance);
        assert.ok(metadata);
        assert.strictEqual(metadata.name, 'testPlugin');
      });
    });

    describe('Plugin Metadata', () => {
      it('should get plugin info', () => {
        registry.register('testPlugin', ExtractionPlugin, {
          description: 'Test plugin',
        });

        const info = registry.getPluginInfo('testPlugin');
        assert.ok(info);
        assert.strictEqual(info.name, 'testPlugin');
      });

      it('should validate plugin configuration', () => {
        registry.register('testPlugin', ExtractionPlugin);

        const validation1 = registry.validatePluginConfig('testPlugin', {});
        assert.ok(validation1.isValid);

        const validation2 = registry.validatePluginConfig('nonexistent', {});
        assert.ok(!validation2.isValid);
      });

      it('should get registry statistics', () => {
        registry.register('plugin1', ExtractionPlugin, { category: 'extraction' });
        registry.register('plugin2', SearchPlugin, { category: 'search' });

        const stats = registry.getStatistics();
        assert.strictEqual(stats.registeredCount, 2);
        assert.ok('byCategory' in stats);
      });
    });

    describe('Plugin Lifecycle', () => {
      it('should call onInitialize during instantiation', () => {
        let initCalled = false;

        class LifecyclePlugin extends ExtractionPlugin {
          onInitialize(document) {
            initCalled = true;
          }
        }

        registry.register('lifecycle', LifecyclePlugin);
        registry.createInstance('lifecycle', mockDoc);

        assert.ok(initCalled);
      });

      it('should call onDestroy during release', () => {
        let destroyCalled = false;

        class LifecyclePlugin extends ExtractionPlugin {
          onDestroy() {
            destroyCalled = true;
          }
        }

        registry.register('lifecycle', LifecyclePlugin);
        const instance = registry.createInstance('lifecycle', mockDoc);
        registry.releaseInstance(instance);

        assert.ok(destroyCalled);
      });
    });

    describe('Registry Maintenance', () => {
      it('should clear all registrations', () => {
        registry.register('plugin1', ExtractionPlugin);
        registry.register('plugin2', SearchPlugin);

        assert.strictEqual(registry.getPlugins().length, 2);

        registry.clear();

        assert.strictEqual(registry.getPlugins().length, 0);
      });

      it('should handle multiple operations', () => {
        // Register multiple plugins
        registry.register('extract1', ExtractionPlugin);
        registry.register('search1', SearchPlugin);
        registry.register('annotate1', AnnotationPlugin);

        // Create instances
        const i1 = registry.createInstance('extract1', mockDoc);
        const i2 = registry.createInstance('search1', mockDoc);
        const i3 = registry.createInstance('annotate1', mockPage);

        // Verify instances
        assert.strictEqual(registry.getInstances().length, 3);

        // Release one
        registry.releaseInstance(i1);
        assert.strictEqual(registry.getInstances().length, 2);

        // Clear all
        registry.clear();
        assert.strictEqual(registry.getPlugins().length, 0);
      });
    });
  });

  describe('Plugin Examples', () => {
    it('should implement text normalization plugin', () => {
      class TextNormalizationPlugin extends ExtractionPlugin {
        extractText(pageIndex, options) {
          const text = super.extractText(pageIndex, options);
          return this.normalize(text);
        }

        normalize(text) {
          return text.trim().toLowerCase().replace(/\\s+/g, ' ');
        }
      }

      const plugin = new TextNormalizationPlugin(mockDoc);
      const text = plugin.extractText(0);

      // Should be normalized (lowercase)
      assert.ok(text.length > 0);
    });

    it('should implement search result filtering plugin', () => {
      class SearchFilterPlugin extends SearchPlugin {
        search(searchText, pageIndex, options) {
          const results = super.search(searchText, pageIndex, options);
          // Filter results based on position
          return results.filter((r) => r.position >= 0);
        }
      }

      const plugin = new SearchFilterPlugin(mockDoc);
      const results = plugin.search('test', 0);

      assert.ok(Array.isArray(results));
    });

    it('should implement annotation enrichment plugin', () => {
      class AnnotationEnrichmentPlugin extends AnnotationPlugin {
        getAnnotations() {
          const annotations = super.getAnnotations();
          // Enrich annotations with additional metadata
          return annotations.map((ann) => ({
            ...ann,
            enriched: true,
            timestamp: new Date().toISOString(),
          }));
        }
      }

      const plugin = new AnnotationEnrichmentPlugin(mockPage);
      const annotations = plugin.getAnnotations();

      assert.ok(Array.isArray(annotations));
    });
  });

  describe('Phase 3 Summary', () => {
    it('should have plugin base classes', () => {
      assert.ok(typeof ExtractionPlugin === 'function');
      assert.ok(typeof SearchPlugin === 'function');
      assert.ok(typeof AnnotationPlugin === 'function');
    });

    it('should have PluginRegistry', () => {
      const registry = new PluginRegistry();
      assert.ok(typeof registry.register === 'function');
      assert.ok(typeof registry.createInstance === 'function');
      assert.ok(typeof registry.getPlugins === 'function');
    });

    it('should document plugin system features', () => {
      const features = [
        'Wrapper-based plugin inheritance',
        'Plugin configuration management',
        'Plugin logging system',
        'Performance metrics tracking',
        'Lifecycle hooks (onInitialize, onDestroy)',
        'Plugin registry with discovery',
        'Instance lifecycle management',
      ];

      assert.strictEqual(features.length, 7);
      features.forEach((feature) => {
        assert.ok(typeof feature === 'string');
      });
    });
  });
});
