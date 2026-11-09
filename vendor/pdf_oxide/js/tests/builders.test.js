import assert from 'node:assert';
import { describe, it } from 'node:test';
import {
  AnnotationBuilder,
  ConversionOptionsBuilder,
  MetadataBuilder,
  PdfBuilder,
  SearchOptionsBuilder,
} from '../lib/builders/index.js';

describe('Builders - Phase 2.5', () => {
  describe('PdfBuilder', () => {
    it('should create builder instance', () => {
      const builder = PdfBuilder.create();
      assert.ok(builder instanceof PdfBuilder);
    });

    it('should support fluent chaining', () => {
      const builder = PdfBuilder.create()
        .title('Test')
        .author('Author')
        .subject('Subject')
        .pageSize('A4');

      assert.ok(builder instanceof PdfBuilder);
    });

    it('should validate title', () => {
      const builder = PdfBuilder.create();
      assert.throws(() => builder.title(''), /non-empty/);
      assert.throws(() => builder.title(123), /string/);
    });

    it('should validate author', () => {
      const builder = PdfBuilder.create();
      assert.throws(() => builder.author(''), /non-empty/);
      assert.throws(() => builder.author(null), /string/);
    });

    it('should validate keywords array', () => {
      const builder = PdfBuilder.create();
      assert.throws(() => builder.keywords('not-array'), /array/);
      assert.throws(() => builder.keywords([1, 2, 3]), /strings/);
    });

    it('should add single keyword', () => {
      const builder = PdfBuilder.create().addKeyword('tag1').addKeyword('tag2');

      assert.ok(builder instanceof PdfBuilder);
    });

    it('should validate page size', () => {
      const builder = PdfBuilder.create();
      assert.throws(() => builder.pageSize('InvalidSize'), /Invalid page size/);
    });

    it('should accept valid page sizes', () => {
      const sizes = ['Letter', 'Legal', 'A4', 'A3', 'A5', 'B4', 'B5'];
      sizes.forEach((size) => {
        const builder = PdfBuilder.create().pageSize(size);
        assert.ok(builder instanceof PdfBuilder);
      });
    });

    it('should validate margins', () => {
      const builder = PdfBuilder.create();
      assert.throws(() => builder.margins(-1, 0, 0, 0), /non-negative/);
      assert.throws(() => builder.margins('10', 0, 0, 0), /numbers/);
    });

    it('should create PDF from markdown', () => {
      const builder = PdfBuilder.create().title('Test Document').author('Test Author');

      // Note: This would require native module to test fully
      // For now, we test the builder configuration
      assert.ok(builder instanceof PdfBuilder);
    });
  });

  describe('ConversionOptionsBuilder', () => {
    it('should create builder instance', () => {
      const builder = ConversionOptionsBuilder.create();
      assert.ok(builder instanceof ConversionOptionsBuilder);
    });

    it('should provide default preset', () => {
      const options = ConversionOptionsBuilder.default();
      assert.ok(options);
      assert.strictEqual(typeof options.preserveFormatting, 'boolean');
      assert.strictEqual(typeof options.imageQuality, 'number');
    });

    it('should provide text-only preset', () => {
      const options = ConversionOptionsBuilder.textOnly();
      assert.strictEqual(options.preserveFormatting, false);
      assert.strictEqual(options.detectTables, false);
      assert.strictEqual(options.includeImages, false);
    });

    it('should provide high-quality preset', () => {
      const options = ConversionOptionsBuilder.highQuality();
      assert.strictEqual(options.preserveFormatting, true);
      assert.strictEqual(options.detectTables, true);
      assert.strictEqual(options.imageQuality, 95);
    });

    it('should provide fast preset', () => {
      const options = ConversionOptionsBuilder.fast();
      assert.strictEqual(options.normalizeWhitespace, true);
      assert.strictEqual(options.includeImages, false);
    });

    it('should validate preserveFormatting', () => {
      const builder = ConversionOptionsBuilder.create();
      assert.throws(() => builder.preserveFormatting('yes'), /boolean/);
    });

    it('should validate image quality', () => {
      const builder = ConversionOptionsBuilder.create();
      assert.throws(() => builder.imageQuality(150), /0 and 100/);
      assert.throws(() => builder.imageQuality(-10), /0 and 100/);
    });

    it('should validate image format', () => {
      const builder = ConversionOptionsBuilder.create();
      assert.throws(() => builder.imageFormat('bmp'), /Invalid image format/);
    });

    it('should accept valid image formats', () => {
      const formats = ['png', 'jpg', 'jpeg', 'webp'];
      formats.forEach((fmt) => {
        const options = ConversionOptionsBuilder.create().imageFormat(fmt).build();
        assert.strictEqual(options.imageFormat, fmt.toLowerCase());
      });
    });

    it('should build options object', () => {
      const options = ConversionOptionsBuilder.create()
        .preserveFormatting(true)
        .detectHeadings(true)
        .detectTables(true)
        .includeImages(true)
        .imageQuality(90)
        .build();

      assert.ok(options);
      assert.strictEqual(options.preserveFormatting, true);
      assert.strictEqual(options.detectHeadings, true);
      assert.strictEqual(options.imageQuality, 90);
    });
  });

  describe('MetadataBuilder', () => {
    it('should create builder instance', () => {
      const builder = MetadataBuilder.create();
      assert.ok(builder instanceof MetadataBuilder);
    });

    it('should support fluent chaining', () => {
      const builder = MetadataBuilder.create()
        .title('Test')
        .author('Author')
        .subject('Subject')
        .keywords(['tag1', 'tag2']);

      assert.ok(builder instanceof MetadataBuilder);
    });

    it('should build metadata object', () => {
      const metadata = MetadataBuilder.create()
        .title('Test Document')
        .author('Test Author')
        .subject('Test Subject')
        .keywords(['test', 'document'])
        .build();

      assert.ok(metadata);
      assert.strictEqual(metadata.title, 'Test Document');
      assert.strictEqual(metadata.author, 'Test Author');
      assert.strictEqual(metadata.subject, 'Test Subject');
      assert.deepStrictEqual(metadata.keywords, ['test', 'document']);
    });

    it('should validate dates', () => {
      const builder = MetadataBuilder.create();
      assert.throws(() => builder.creationDate('2024-01-01'), /Date object/);
      assert.throws(() => builder.modificationDate('not-a-date'), /Date object/);
    });

    it('should accept Date objects', () => {
      const now = new Date();
      const metadata = MetadataBuilder.create().creationDate(now).modificationDate(now).build();

      assert.ok(metadata.creationDate instanceof Date);
      assert.ok(metadata.modificationDate instanceof Date);
    });

    it('should set custom properties', () => {
      const metadata = MetadataBuilder.create()
        .customProperty('Department', 'Engineering')
        .customProperty('Classification', 'Confidential')
        .build();

      assert.strictEqual(metadata.customProperties['Department'], 'Engineering');
      assert.strictEqual(metadata.customProperties['Classification'], 'Confidential');
    });

    it('should set multiple custom properties at once', () => {
      const metadata = MetadataBuilder.create()
        .customProperties({
          Dept: 'IT',
          Project: 'Alpha',
        })
        .build();

      assert.strictEqual(metadata.customProperties['Dept'], 'IT');
      assert.strictEqual(metadata.customProperties['Project'], 'Alpha');
    });

    it('should validate custom properties', () => {
      const builder = MetadataBuilder.create();
      assert.throws(() => builder.customProperty('', 'value'), /non-empty/);
      assert.throws(() => builder.customProperty('key', 123), /string/);
    });

    it('should set current date', () => {
      const before = new Date();
      const metadata = MetadataBuilder.create().withCurrentDate().build();
      const after = new Date();

      assert.ok(metadata.modificationDate >= before);
      assert.ok(metadata.modificationDate <= after);
    });
  });

  describe('AnnotationBuilder', () => {
    it('should create builder instance', () => {
      const builder = AnnotationBuilder.create();
      assert.ok(builder instanceof AnnotationBuilder);
    });

    it('should set annotation type', () => {
      const annotation = AnnotationBuilder.create()
        .type('highlight')
        .bounds({ x: 10, y: 20, width: 100, height: 30 })
        .build();

      assert.strictEqual(annotation.type, 'highlight');
    });

    it('should validate annotation type', () => {
      const builder = AnnotationBuilder.create();
      assert.throws(() => builder.type('invalid-type'), /Invalid annotation type/);
    });

    it('should support type-specific builders', () => {
      const types = [
        { builder: 'asText', type: 'text' },
        { builder: 'asHighlight', type: 'highlight' },
        { builder: 'asUnderline', type: 'underline' },
        { builder: 'asStrikeout', type: 'strikeout' },
        { builder: 'asSquiggly', type: 'squiggly' },
      ];

      types.forEach(({ builder: methodName, type }) => {
        let builder = AnnotationBuilder.create()[methodName]();
        // Text annotations don't require bounds, others do
        if (type !== 'text') {
          builder = builder.bounds({ x: 0, y: 0, width: 100, height: 20 });
        }
        const annotation = builder.build();
        assert.strictEqual(annotation.type, type);
      });
    });

    it('should set color with RGB values', () => {
      const annotation = AnnotationBuilder.create()
        .asHighlight()
        .color([1, 0.5, 0])
        .bounds({ x: 0, y: 0, width: 50, height: 10 })
        .build();

      assert.deepStrictEqual(annotation.color, [1, 0.5, 0]);
    });

    it('should set color by name', () => {
      const annotation = AnnotationBuilder.create()
        .asHighlight()
        .colorName('yellow')
        .bounds({ x: 0, y: 0, width: 50, height: 10 })
        .build();

      assert.deepStrictEqual(annotation.color, [1, 1, 0]);
    });

    it('should validate color names', () => {
      const builder = AnnotationBuilder.create();
      assert.throws(() => builder.colorName('invalidColor'), /Unknown color/);
    });

    it('should validate RGB color values', () => {
      const builder = AnnotationBuilder.create();
      assert.throws(() => builder.color([1.5, 0, 0]), /0 and 1/);
      assert.throws(() => builder.color([-0.1, 0, 0]), /0 and 1/);
      assert.throws(() => builder.color([1, 0]), /3 RGB values/);
    });

    it('should set opacity', () => {
      const annotation = AnnotationBuilder.create().opacity(0.7).build();

      assert.strictEqual(annotation.opacity, 0.7);
    });

    it('should validate opacity', () => {
      const builder = AnnotationBuilder.create();
      assert.throws(() => builder.opacity(1.5), /0 and 1/);
      assert.throws(() => builder.opacity(-0.1), /0 and 1/);
    });

    it('should set bounds', () => {
      const bounds = { x: 100, y: 200, width: 150, height: 30 };
      const annotation = AnnotationBuilder.create().asHighlight().bounds(bounds).build();

      assert.deepStrictEqual(annotation.bounds, bounds);
    });

    it('should validate bounds', () => {
      const builder = AnnotationBuilder.create();
      assert.throws(() => builder.bounds({ x: -1, y: 0, width: 100, height: 100 }), /numeric/);
    });

    it('should set author and subject', () => {
      const annotation = AnnotationBuilder.create().author('Reviewer').subject('Comment').build();

      assert.strictEqual(annotation.author, 'Reviewer');
      assert.strictEqual(annotation.subject, 'Comment');
    });

    it('should build annotation', () => {
      const annotation = AnnotationBuilder.create()
        .asHighlight()
        .content('Important!')
        .author('Jane')
        .colorName('yellow')
        .opacity(0.8)
        .bounds({ x: 50, y: 100, width: 200, height: 20 })
        .build();

      assert.strictEqual(annotation.type, 'highlight');
      assert.strictEqual(annotation.content, 'Important!');
      assert.strictEqual(annotation.author, 'Jane');
      assert.strictEqual(annotation.opacity, 0.8);
    });
  });

  describe('SearchOptionsBuilder', () => {
    it('should create builder instance', () => {
      const builder = SearchOptionsBuilder.create();
      assert.ok(builder instanceof SearchOptionsBuilder);
    });

    it('should provide presets', () => {
      const presets = [
        { name: 'default', expectedSensitive: false },
        { name: 'strict', expectedSensitive: true },
        { name: 'regex', expectedSensitive: false },
      ];

      presets.forEach(({ name, expectedSensitive }) => {
        const options = SearchOptionsBuilder[name]();
        assert.strictEqual(options.caseSensitive, expectedSensitive);
      });
    });

    it('should build search options', () => {
      const options = SearchOptionsBuilder.create()
        .caseSensitive(true)
        .wholeWords(true)
        .useRegex(false)
        .maxResults(100)
        .build();

      assert.strictEqual(options.caseSensitive, true);
      assert.strictEqual(options.wholeWords, true);
      assert.strictEqual(options.useRegex, false);
      assert.strictEqual(options.maxResults, 100);
    });

    it('should validate boolean options', () => {
      const builder = SearchOptionsBuilder.create();
      assert.throws(() => builder.caseSensitive('yes'), /boolean/);
      assert.throws(() => builder.wholeWords(1), /boolean/);
      assert.throws(() => builder.useRegex(null), /boolean/);
      assert.throws(() => builder.ignoreAccents('maybe'), /boolean/);
      assert.throws(() => builder.searchAnnotations(0), /boolean/);
    });

    it('should validate maxResults', () => {
      const builder = SearchOptionsBuilder.create();
      assert.throws(() => builder.maxResults(0), /positive/);
      assert.throws(() => builder.maxResults(-10), /positive/);
      assert.throws(() => builder.maxResults('10'), /positive/);
    });

    it('should floor maxResults', () => {
      const options = SearchOptionsBuilder.create().maxResults(123.7).build();

      assert.strictEqual(options.maxResults, 123);
    });
  });

  describe('Builder integration', () => {
    it('should work with all builders', () => {
      const builders = [
        PdfBuilder.create(),
        ConversionOptionsBuilder.create(),
        MetadataBuilder.create(),
        AnnotationBuilder.create(),
        SearchOptionsBuilder.create(),
      ];

      builders.forEach((builder) => {
        assert.ok(builder instanceof Object);
      });
    });

    it('should create complete configuration', () => {
      const metadata = MetadataBuilder.create().title('Test').author('Author').build();

      const options = ConversionOptionsBuilder.create()
        .preserveFormatting(true)
        .detectTables(true)
        .build();

      const searchOpts = SearchOptionsBuilder.create()
        .caseSensitive(false)
        .wholeWords(true)
        .build();

      assert.ok(metadata);
      assert.ok(options);
      assert.ok(searchOpts);
    });
  });
});
