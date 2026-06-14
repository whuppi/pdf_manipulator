import assert from 'node:assert';
import { describe, it } from 'node:test';
import {
  addPdfDocumentProperties,
  addPdfPageProperties,
  addPdfProperties,
} from '../lib/properties.js';

describe('Property Getters - Phase 2.3', () => {
  describe('PdfDocument properties', () => {
    it('should add version property getter', () => {
      // Mock object with get_version method
      const MockDocument = class {
        get_version() {
          return [1, 7];
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      // Test property access
      assert.deepStrictEqual(doc.version, [1, 7]);

      // Test that property is on the prototype
      assert.ok(Object.getOwnPropertyDescriptor(MockDocument.prototype, 'version'));
    });

    it('should add pageCount property getter', () => {
      const MockDocument = class {
        get_page_count() {
          return 42;
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.strictEqual(doc.pageCount, 42);
    });

    it('should add hasStructureTree property getter', () => {
      const MockDocument = class {
        has_structure_tree() {
          return true;
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.strictEqual(doc.hasStructureTree, true);
    });

    it('should add documentInfo property getter with caching', () => {
      let callCount = 0;
      const MockDocument = class {
        get_document_info() {
          callCount++;
          return { title: 'Test' };
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      // First access
      const info1 = doc.documentInfo;
      assert.deepStrictEqual(info1, { title: 'Test' });
      assert.strictEqual(callCount, 1);

      // Second access should use cache
      const info2 = doc.documentInfo;
      assert.strictEqual(callCount, 1); // Not called again
      assert.strictEqual(info1, info2); // Same reference
    });

    it('should add metadata property getter with caching', () => {
      let callCount = 0;
      const MockDocument = class {
        get_metadata() {
          callCount++;
          return { format: 'xmp' };
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      const meta1 = doc.metadata;
      assert.deepStrictEqual(meta1, { format: 'xmp' });
      const meta2 = doc.metadata;
      assert.strictEqual(callCount, 1); // Cached
    });

    it('should add forms property getter', () => {
      const MockDocument = class {
        get_forms() {
          return { fields: [] };
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.deepStrictEqual(doc.forms, { fields: [] });
    });

    it('should add pageLabels property getter', () => {
      const MockDocument = class {
        get_page_labels() {
          return ['i', 'ii', 'iii', '1', '2'];
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.deepStrictEqual(doc.pageLabels, ['i', 'ii', 'iii', '1', '2']);
    });

    it('should add embeddedFiles property getter', () => {
      const MockDocument = class {
        get_embedded_files() {
          return [{ name: 'file.txt' }];
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.deepStrictEqual(doc.embeddedFiles, [{ name: 'file.txt' }]);
    });

    it('should handle missing methods gracefully', () => {
      const MockDocument = class {};

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      // Should return defaults when methods don't exist
      // version returns [1, 4] as default
      assert.deepStrictEqual(doc.version, [1, 4]);
      assert.strictEqual(doc.pageCount, 0);
      assert.strictEqual(doc.hasStructureTree, false);
    });

    it('should be enumerable and configurable', () => {
      const MockDocument = class {
        get_version() {
          return [1, 4];
        }
      };

      addPdfDocumentProperties(MockDocument);

      const descriptor = Object.getOwnPropertyDescriptor(MockDocument.prototype, 'version');
      assert.strictEqual(descriptor.enumerable, true);
      assert.strictEqual(descriptor.configurable, true);
      assert.ok(!descriptor.writable); // Getter only
    });
  });

  describe('Pdf properties', () => {
    it('should add version property getter', () => {
      const MockPdf = class {
        get_version() {
          return [1, 5];
        }
      };

      addPdfProperties(MockPdf);
      const pdf = new MockPdf();

      assert.deepStrictEqual(pdf.version, [1, 5]);
    });

    it('should add pageCount property getter', () => {
      const MockPdf = class {
        get_page_count() {
          return 10;
        }
      };

      addPdfProperties(MockPdf);
      const pdf = new MockPdf();

      assert.strictEqual(pdf.pageCount, 10);
    });

    it('should add documentInfo property getter with caching', () => {
      let callCount = 0;
      const MockPdf = class {
        get_document_info() {
          callCount++;
          return { title: 'PDF Title' };
        }
      };

      addPdfProperties(MockPdf);
      const pdf = new MockPdf();

      const info1 = pdf.documentInfo;
      const info2 = pdf.documentInfo;
      assert.strictEqual(callCount, 1); // Cached
    });

    it('should add metadata property getter', () => {
      const MockPdf = class {
        get_metadata() {
          return { author: 'Test Author' };
        }
      };

      addPdfProperties(MockPdf);
      const pdf = new MockPdf();

      assert.deepStrictEqual(pdf.metadata, { author: 'Test Author' });
    });

    it('should add forms property getter', () => {
      const MockPdf = class {
        get_forms() {
          return null;
        }
      };

      addPdfProperties(MockPdf);
      const pdf = new MockPdf();

      assert.strictEqual(pdf.forms, null);
    });
  });

  describe('PdfPage properties', () => {
    it('should add pageIndex property getter', () => {
      const MockPage = class {
        get_page_index() {
          return 5;
        }
      };

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      assert.strictEqual(page.pageIndex, 5);
    });

    it('should add width property getter', () => {
      const MockPage = class {
        get_width() {
          return 612.0;
        }
      };

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      assert.strictEqual(page.width, 612.0);
    });

    it('should add height property getter', () => {
      const MockPage = class {
        get_height() {
          return 792.0;
        }
      };

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      assert.strictEqual(page.height, 792.0);
    });

    it('should add bounds property getter', () => {
      const MockPage = class {
        get_width() {
          return 612.0;
        }
        get_height() {
          return 792.0;
        }
        get_bounds() {
          return { x: 0, y: 0, width: 612.0, height: 792.0 };
        }
      };

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      const bounds = page.bounds;
      assert.strictEqual(bounds.x, 0);
      assert.strictEqual(bounds.y, 0);
      assert.strictEqual(bounds.width, 612.0);
      assert.strictEqual(bounds.height, 792.0);
    });

    it('should compute orientation from dimensions', () => {
      const PortraitPage = class {
        get_width() {
          return 612.0;
        }
        get_height() {
          return 792.0;
        }
      };

      const LandscapePage = class {
        get_width() {
          return 792.0;
        }
        get_height() {
          return 612.0;
        }
      };

      addPdfPageProperties(PortraitPage);
      addPdfPageProperties(LandscapePage);

      const portrait = new PortraitPage();
      const landscape = new LandscapePage();

      assert.strictEqual(portrait.orientation, 'portrait');
      assert.strictEqual(landscape.orientation, 'landscape');
    });

    it('should compute aspect ratio', () => {
      const MockPage = class {
        get_width() {
          return 800.0;
        }
        get_height() {
          return 600.0;
        }
      };

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      // Aspect ratio should be width / height = 800 / 600 = 1.333...
      assert.strictEqual(page.aspectRatio, 800.0 / 600.0);
    });

    it('should handle missing methods gracefully', () => {
      const MockPage = class {};

      addPdfPageProperties(MockPage);
      const page = new MockPage();

      // Should return defaults
      assert.strictEqual(page.pageIndex, 0);
      assert.strictEqual(page.width, 612.0);
      assert.strictEqual(page.height, 792.0);
      assert.strictEqual(page.orientation, 'portrait'); // 612 < 792
    });

    it('should be enumerable properties', () => {
      const MockPage = class {
        get_width() {
          return 612.0;
        }
      };

      addPdfPageProperties(MockPage);

      const descriptor = Object.getOwnPropertyDescriptor(MockPage.prototype, 'width');
      assert.strictEqual(descriptor.enumerable, true);
      assert.strictEqual(descriptor.configurable, true);
    });
  });

  describe('Property getter integration', () => {
    it('should work with multiple instances independently', () => {
      const MockDocument = class {
        constructor(pages) {
          this._pages = pages;
        }

        get_page_count() {
          return this._pages;
        }
      };

      addPdfDocumentProperties(MockDocument);

      const doc1 = new MockDocument(10);
      const doc2 = new MockDocument(20);

      assert.strictEqual(doc1.pageCount, 10);
      assert.strictEqual(doc2.pageCount, 20);
    });

    it('should not interfere with other properties', () => {
      const MockDocument = class {
        constructor(title) {
          this.title = title;
        }

        get_version() {
          return [1, 4];
        }
      };

      addPdfDocumentProperties(MockDocument);

      const doc = new MockDocument('My PDF');

      // Both own and getter properties should work
      assert.strictEqual(doc.title, 'My PDF');
      assert.deepStrictEqual(doc.version, [1, 4]);
    });
  });

  describe('Edge cases', () => {
    it('should handle null/undefined gracefully', () => {
      const MockDocument = class {
        get_version() {
          return null;
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      assert.strictEqual(doc.version, null);
    });

    it('should handle error cases in getters', () => {
      const MockDocument = class {
        get_page_count() {
          throw new Error('Test error');
        }
      };

      addPdfDocumentProperties(MockDocument);
      const doc = new MockDocument();

      // Accessing the property should throw the error
      assert.throws(
        () => {
          const _ = doc.pageCount;
        },
        (err) => err.message === 'Test error'
      );
    });

    it('should work with inherited classes', () => {
      class BaseMockDocument {
        get_version() {
          return [1, 7];
        }
      }

      class DerivedDocument extends BaseMockDocument {}

      addPdfDocumentProperties(DerivedDocument);
      const doc = new DerivedDocument();

      // Should work through inheritance
      assert.deepStrictEqual(doc.version, [1, 7]);
    });
  });
});
