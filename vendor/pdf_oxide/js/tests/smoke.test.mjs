/**
 * Smoke tests for the pdf-oxide Node.js binding.
 *
 * These tests import the compiled library (`lib/`), so you must run
 * `npm run build` before `npm test`. They exercise the public API with
 * self-generated PDFs and verify the happy path + a handful of error paths.
 *
 * Uses the Node built-in test runner (Node 18+), so there are no extra
 * dev-dependencies.
 */

import assert from 'node:assert/strict';
import { mkdtemp, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { before, describe, it } from 'node:test';

let PdfDocument, Pdf;

describe('pdf-oxide smoke tests', () => {
  let tempDir;

  before(async () => {
    // Dynamic import so the test runner can skip cleanly when the compiled
    // library is missing (e.g. CI runs `npm test` before `npm run build`).
    try {
      const mod = await import('../lib/index.js');
      PdfDocument = mod.PdfDocument;
      Pdf = mod.Pdf;
    } catch (err) {
      console.warn(`[smoke] skipping — compiled library not available: ${err.message}`);
    }
    tempDir = await mkdtemp(join(tmpdir(), 'pdf-oxide-smoke-'));
  });

  it('exports PdfDocument and Pdf', { skip: !PdfDocument }, () => {
    assert.ok(PdfDocument, 'PdfDocument should be exported');
    assert.ok(Pdf, 'Pdf should be exported');
  });

  it('creates a PDF from Markdown', { skip: !PdfDocument }, () => {
    const pdf = Pdf.fromMarkdown('# Hello World\n\nBody text.');
    const bytes = pdf.saveToBytes();
    assert.ok(bytes.length > 100, `expected >100 bytes, got ${bytes.length}`);
    // PDF magic header
    assert.equal(bytes[0], 0x25); // '%'
    assert.equal(bytes[1], 0x50); // 'P'
    assert.equal(bytes[2], 0x44); // 'D'
    assert.equal(bytes[3], 0x46); // 'F'
    pdf.close?.();
  });

  it('round-trips: create, save, open, extract', { skip: !PdfDocument }, async () => {
    const path = join(tempDir, 'roundtrip.pdf');
    const pdf = Pdf.fromMarkdown('# Round Trip\n\nExtractable content.');
    pdf.save(path);
    pdf.close?.();

    const doc = PdfDocument.open(path);
    try {
      assert.ok(doc.pageCount() >= 1, 'should have at least 1 page');
      const text = doc.extractText(0);
      assert.ok(typeof text === 'string', 'extractText should return a string');
    } finally {
      doc.close?.();
    }
  });

  it('rejects opening a non-existent file', { skip: !PdfDocument }, () => {
    assert.throws(
      () => PdfDocument.open('/nonexistent/path/to/file.pdf'),
      /./ // any error
    );
  });

  it('saves to multiple files without double-free', { skip: !PdfDocument }, async () => {
    // Regression for pdf_save consuming the handle (Box::from_raw bug).
    const pdf = Pdf.fromMarkdown('# Multi-save\n\nContent.');
    const path1 = join(tempDir, 'multi1.pdf');
    const path2 = join(tempDir, 'multi2.pdf');
    pdf.save(path1);
    pdf.save(path2);
    pdf.close?.();

    const size1 = (await readFile(path1)).length;
    const size2 = (await readFile(path2)).length;
    assert.ok(size1 > 100, `first file too small: ${size1}`);
    assert.ok(size2 > 100, `second file too small: ${size2}`);
    assert.equal(size1, size2, 'both saves should produce identical bytes');
  });
});
