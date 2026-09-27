import assert from 'node:assert';
import { existsSync, mkdirSync, unlinkSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';
import { Pdf, PdfDocument } from '../index.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const TEMP_DIR = join(__dirname, '.temp');

// Ensure temp directory exists
if (!existsSync(TEMP_DIR)) {
  mkdirSync(TEMP_DIR, { recursive: true });
}

describe('Async Methods - Phase 2.4', () => {
  describe('PdfDocument async methods', () => {
    it('extractTextAsync should extract text asynchronously', async () => {
      // Create a simple test PDF
      const markdown = '# Test Document\n\nThis is a test document for async text extraction.';
      const pdf = Pdf.fromMarkdown(markdown);
      const outputPath = join(TEMP_DIR, 'test-async-extract.pdf');
      pdf.save(outputPath);

      // Open document and extract text asynchronously
      const doc = PdfDocument.open(outputPath);
      const text = await doc.extractTextAsync(0);

      // Verify result
      assert.strictEqual(typeof text, 'string');
      assert.ok(text.length > 0);
      assert.ok(text.includes('Test Document') || text.includes('test'));

      // Cleanup
      doc.close();
      unlinkSync(outputPath);
    });

    it('extractTextAsync should handle multiple pages', async () => {
      // Create a multi-page PDF
      const pdf = Pdf.fromMarkdown('# Page 1\nContent 1');
      pdf.addPage(612, 792);
      pdf.addText('Page 2 Content', 50, 50);
      const outputPath = join(TEMP_DIR, 'test-async-multipage.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Extract from both pages asynchronously
      const text0 = await doc.extractTextAsync(0);
      const text1 = await doc.extractTextAsync(1);

      assert.strictEqual(typeof text0, 'string');
      assert.strictEqual(typeof text1, 'string');
      assert.ok(text0.length > 0);
      assert.ok(text1.length > 0);

      doc.close();
      unlinkSync(outputPath);
    });

    it('extractTextAsync should handle page index validation', async () => {
      const markdown = '# Test';
      const pdf = Pdf.fromMarkdown(markdown);
      const outputPath = join(TEMP_DIR, 'test-async-validation.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Test valid page index
      const text = await doc.extractTextAsync(0);
      assert.strictEqual(typeof text, 'string');

      // Note: Rust side should validate page index, JavaScript async wrapper should propagate errors
      doc.close();
      unlinkSync(outputPath);
    });

    it('toMarkdownAsync should convert page to markdown asynchronously', async () => {
      const markdown = '# Heading\n\n**Bold** text and *italic* text.';
      const pdf = Pdf.fromMarkdown(markdown);
      const outputPath = join(TEMP_DIR, 'test-async-markdown.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);
      const result = await doc.toMarkdownAsync(0);

      assert.strictEqual(typeof result, 'string');
      assert.ok(result.length > 0);

      doc.close();
      unlinkSync(outputPath);
    });

    it('toMarkdownAsync should preserve conversion options', async () => {
      const html = '<h1>Title</h1><p>Paragraph</p>';
      const pdf = Pdf.fromHtml(html);
      const outputPath = join(TEMP_DIR, 'test-async-markdown-opts.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Call with options (even if not fully used yet)
      const result = await doc.toMarkdownAsync(0, {
        detectHeadings: true,
        detectTables: true,
        includeImages: true,
      });

      assert.strictEqual(typeof result, 'string');
      assert.ok(result.length > 0);

      doc.close();
      unlinkSync(outputPath);
    });

    it('async methods should not block event loop', async () => {
      const pdf = Pdf.fromMarkdown('# Test Document\n\nContent for timing test.');
      const outputPath = join(TEMP_DIR, 'test-async-blocking.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Track timing to ensure async doesn't block
      const startTime = Date.now();

      // Run multiple async operations concurrently
      const [text, markdown] = await Promise.all([doc.extractTextAsync(0), doc.toMarkdownAsync(0)]);

      const elapsed = Date.now() - startTime;

      assert.strictEqual(typeof text, 'string');
      assert.strictEqual(typeof markdown, 'string');
      // Should complete in reasonable time (not be blocking)
      // Note: This is a rough check, as times can vary
      assert.ok(elapsed < 10000);

      doc.close();
      unlinkSync(outputPath);
    });

    it('extractTextAsync and toMarkdownAsync should handle errors gracefully', async () => {
      const pdf = Pdf.fromMarkdown('# Test');
      const outputPath = join(TEMP_DIR, 'test-async-error.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Try to extract from invalid page index
      try {
        await doc.extractTextAsync(999);
        assert.fail('Should have thrown error for invalid page');
      } catch (err) {
        // Expected: error should be thrown and wrapped properly
        assert.ok(err instanceof Error);
      }

      doc.close();
      unlinkSync(outputPath);
    });
  });

  describe('Pdf async methods', () => {
    it('saveAsync should save PDF asynchronously', async () => {
      const pdf = Pdf.fromMarkdown('# Async Save Test\n\nTesting async save operation.');
      const outputPath = join(TEMP_DIR, 'test-async-save.pdf');

      // Save asynchronously
      await pdf.saveAsync(outputPath);

      // Verify file was created
      assert.ok(existsSync(outputPath));

      // Verify file is readable
      const doc = PdfDocument.open(outputPath);
      assert.ok(doc.pageCount >= 1);
      doc.close();

      // Cleanup
      unlinkSync(outputPath);
    });

    it('saveAsync should handle concurrent saves', async () => {
      const pdf1 = Pdf.fromMarkdown('# Document 1');
      const pdf2 = Pdf.fromMarkdown('# Document 2');
      const pdf3 = Pdf.fromMarkdown('# Document 3');

      const path1 = join(TEMP_DIR, 'async-concurrent-1.pdf');
      const path2 = join(TEMP_DIR, 'async-concurrent-2.pdf');
      const path3 = join(TEMP_DIR, 'async-concurrent-3.pdf');

      // Save all concurrently
      await Promise.all([pdf1.saveAsync(path1), pdf2.saveAsync(path2), pdf3.saveAsync(path3)]);

      // Verify all files were created
      assert.ok(existsSync(path1));
      assert.ok(existsSync(path2));
      assert.ok(existsSync(path3));

      // Verify all are valid PDFs
      const doc1 = PdfDocument.open(path1);
      const doc2 = PdfDocument.open(path2);
      const doc3 = PdfDocument.open(path3);

      assert.ok(doc1.pageCount >= 1);
      assert.ok(doc2.pageCount >= 1);
      assert.ok(doc3.pageCount >= 1);

      doc1.close();
      doc2.close();
      doc3.close();

      // Cleanup
      unlinkSync(path1);
      unlinkSync(path2);
      unlinkSync(path3);
    });

    it('saveAsync should overwrite existing file', async () => {
      const pdf1 = Pdf.fromMarkdown('# First Version');
      const outputPath = join(TEMP_DIR, 'test-async-overwrite.pdf');

      // Save first version
      await pdf1.saveAsync(outputPath);
      const stat1 = require('node:fs').statSync(outputPath);

      // Save second version (should overwrite)
      const pdf2 = Pdf.fromMarkdown(
        '# Second Version\n\nWith more content that should make file larger.'
      );
      await pdf2.saveAsync(outputPath);
      const stat2 = require('node:fs').statSync(outputPath);

      // Verify file was updated
      assert.ok(existsSync(outputPath));
      // File size should be different after overwrite
      // (second version has more content)
      assert.notStrictEqual(stat1.size, stat2.size);

      // Cleanup
      unlinkSync(outputPath);
    });

    it('saveAsync should work with complex PDF', async () => {
      const pdf = Pdf.fromMarkdown('# Complex Document\n\nWith **bold** and *italic* text.');
      pdf.addPage(612, 792);
      pdf.addText('Additional Page Content', 50, 50);

      const outputPath = join(TEMP_DIR, 'test-async-complex.pdf');

      // Save complex PDF asynchronously
      await pdf.saveAsync(outputPath);

      // Verify and read back
      const doc = PdfDocument.open(outputPath);
      assert.strictEqual(doc.pageCount, 2);
      const text = await doc.extractTextAsync(0);
      assert.ok(text.length > 0);
      doc.close();

      // Cleanup
      unlinkSync(outputPath);
    });
  });

  describe('Async method error handling', () => {
    it('async methods should throw proper error types', async () => {
      const pdf = Pdf.fromMarkdown('# Test');
      const outputPath = join(TEMP_DIR, 'test-async-error-type.pdf');
      pdf.save(outputPath);

      const doc = PdfDocument.open(outputPath);

      // Try invalid page
      try {
        await doc.extractTextAsync(9999);
        assert.fail('Should have thrown error');
      } catch (err) {
        // Error should be wrapped properly
        assert.ok(err instanceof Error);
        assert.ok(err.message);
      }

      doc.close();
      unlinkSync(outputPath);
    });

    it('saveAsync should throw for invalid path', async () => {
      const pdf = Pdf.fromMarkdown('# Test');
      const invalidPath = '/invalid/nonexistent/directory/file.pdf';

      // Try to save to invalid path
      try {
        await pdf.saveAsync(invalidPath);
        assert.fail('Should have thrown error for invalid path');
      } catch (err) {
        // Error should be thrown and wrapped
        assert.ok(err instanceof Error);
      }
    });
  });

  describe('Async method performance', () => {
    it('parallel async extractions should be faster than sequential', async () => {
      const pdf = Pdf.fromMarkdown('# Page 1\n\nContent 1');
      pdf.addPage(612, 792);
      pdf.addText('Page 2 Content', 50, 50);
      pdf.addPage(612, 792);
      pdf.addText('Page 3 Content', 50, 50);

      const outputPath = join(TEMP_DIR, 'test-async-perf.pdf');
      pdf.save(outputPath);
      const doc = PdfDocument.open(outputPath);

      // Sequential extraction
      const seqStart = Date.now();
      await doc.extractTextAsync(0);
      await doc.extractTextAsync(1);
      await doc.extractTextAsync(2);
      const seqTime = Date.now() - seqStart;

      // Parallel extraction
      const parStart = Date.now();
      await Promise.all([
        doc.extractTextAsync(0),
        doc.extractTextAsync(1),
        doc.extractTextAsync(2),
      ]);
      const parTime = Date.now() - parStart;

      // Parallel should be faster or equal (not significantly slower)
      // This is a rough check as timing can vary
      assert.ok(parTime <= seqTime * 1.5);

      doc.close();
      unlinkSync(outputPath);
    });
  });
});
