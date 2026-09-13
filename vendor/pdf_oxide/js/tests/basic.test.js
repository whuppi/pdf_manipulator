import assert from 'node:assert';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { before, describe, it } from 'node:test';
import { Pdf, PdfBuilder, PdfDocument } from '../index.js';

describe('PDF Oxide Node.js Bindings', () => {
  let tempDir;

  before(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'pdf-oxide-'));
  });

  describe('PdfDocument', () => {
    it('should export PdfDocument class', () => {
      assert.strictEqual(typeof PdfDocument, 'function');
    });

    it('should have static methods', () => {
      assert.strictEqual(typeof PdfDocument.open, 'function');
      assert.strictEqual(typeof PdfDocument.openWithPassword, 'function');
    });
  });

  describe('Pdf', () => {
    it('should export Pdf class', () => {
      assert.strictEqual(typeof Pdf, 'function');
    });

    it('should have factory methods', () => {
      assert.strictEqual(typeof Pdf.fromMarkdown, 'function');
      assert.strictEqual(typeof Pdf.fromHtml, 'function');
      assert.strictEqual(typeof Pdf.fromText, 'function');
      assert.strictEqual(typeof Pdf.open, 'function');
    });
  });

  describe('PdfBuilder', () => {
    it('should export PdfBuilder class', () => {
      assert.strictEqual(typeof PdfBuilder, 'function');
    });

    it('should have create method', () => {
      // Note: PdfBuilder.create() is a static method in the plan
      // This will be verified once implementation is complete
    });
  });

  describe('Error Types', () => {
    const errorClasses = [
      'PdfError',
      'PdfIoError',
      'PdfParseError',
      'PdfEncryptionError',
      'PdfUnsupportedError',
      'PdfInvalidStateError',
      'PdfDecodeError',
      'PdfEncodeError',
      'PdfFontError',
      'PdfImageError',
      'PdfCircularReferenceError',
      'PdfRecursionLimitError',
    ];

    for (const errorClass of errorClasses) {
      it(`should export ${errorClass}`, () => {
        const cls = require('../index.js')[errorClass];
        assert.ok(cls !== undefined, `${errorClass} should be exported`);
      });
    }
  });

  describe('Types', () => {
    const types = [
      'PageSize',
      'Rect',
      'Point',
      'Color',
      'ConversionOptions',
      'SearchOptions',
      'SearchResult',
    ];

    for (const type of types) {
      it(`should export ${type}`, () => {
        const t = require('../index.js')[type];
        assert.ok(t !== undefined, `${type} should be exported`);
      });
    }
  });

  describe('Version Info', () => {
    it('should provide version information', () => {
      const { getVersion, getPdfOxideVersion } = require('../index.js');
      assert.strictEqual(typeof getVersion, 'function');
      assert.strictEqual(typeof getPdfOxideVersion, 'function');
    });
  });
});
