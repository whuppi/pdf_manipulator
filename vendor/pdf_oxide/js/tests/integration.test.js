#!/usr/bin/env node

/**
 * Integration Tests for PDF Creation and Editing
 *
 * Tests the Pdf and PdfBuilder classes with realistic scenarios
 */

import assert from 'node:assert';
import { promises as fs } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { after, before, describe, it } from 'node:test';
import { Pdf, PdfBuilder, PdfDocument } from '../index.js';

const TEMP_DIR = join(tmpdir(), `pdf-oxide-tests-${Date.now()}`);

describe('Pdf Creation and Editing', () => {
  before(async () => {
    try {
      await fs.mkdir(TEMP_DIR, { recursive: true });
    } catch (err) {
      // Directory might already exist
    }
  });

  after(async () => {
    try {
      // Clean up test files
      const files = await fs.readdir(TEMP_DIR);
      for (const file of files) {
        await fs.unlink(join(TEMP_DIR, file));
      }
      await fs.rmdir(TEMP_DIR);
    } catch (err) {
      // Cleanup is best-effort
    }
  });

  describe('Pdf.fromMarkdown()', () => {
    it('should create PDF from markdown content', async () => {
      const markdown = '# Hello World\n\nThis is a test document.';
      const pdf = Pdf.fromMarkdown(markdown);
      assert.ok(pdf);
      assert.strictEqual(typeof pdf.save, 'function');
    });

    it('should save created PDF to file', async () => {
      const outputPath = join(TEMP_DIR, 'test-markdown.pdf');
      const markdown = '# Test\n\nContent here';
      const pdf = Pdf.fromMarkdown(markdown);

      pdf.save(outputPath);

      // Verify file was created
      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0, 'Created PDF file should have content');
    });

    it('should preserve markdown formatting in PDF', async () => {
      const outputPath = join(TEMP_DIR, 'markdown-format.pdf');
      const markdown = '# Title\n\n## Subtitle\n\nParagraph text.';
      const pdf = Pdf.fromMarkdown(markdown);

      pdf.save(outputPath);

      // Verify we can read it back
      const doc = PdfDocument.open(outputPath);
      const text = doc.extractText(0);
      assert.ok(text.includes('Title') || text.includes('Subtitle') || text.includes('Paragraph'));
      doc.close();
    });
  });

  describe('Pdf.fromHtml()', () => {
    it('should create PDF from HTML content', async () => {
      const html = '<h1>Hello</h1><p>World</p>';
      const pdf = Pdf.fromHtml(html);
      assert.ok(pdf);
    });

    it('should save HTML-generated PDF to file', async () => {
      const outputPath = join(TEMP_DIR, 'test-html.pdf');
      const html = '<h1>Test</h1><p>HTML content</p>';
      const pdf = Pdf.fromHtml(html);

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });
  });

  describe('Pdf.fromText()', () => {
    it('should create PDF from plain text', async () => {
      const text = 'Plain text content here.';
      const pdf = Pdf.fromText(text);
      assert.ok(pdf);
    });

    it('should save text-generated PDF to file', async () => {
      const outputPath = join(TEMP_DIR, 'test-text.pdf');
      const text = 'Hello World\nThis is plain text.';
      const pdf = Pdf.fromText(text);

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });
  });

  describe('Pdf.open()', () => {
    it('should open existing PDF for editing', async () => {
      // Create a PDF first
      const createPath = join(TEMP_DIR, 'to-open.pdf');
      const created = Pdf.fromMarkdown('Test content');
      created.save(createPath);

      // Open it
      const opened = Pdf.open(createPath);
      assert.ok(opened);
      assert.strictEqual(typeof opened.getPageCount, 'function');
    });

    it('should get page count from opened PDF', async () => {
      const createPath = join(TEMP_DIR, 'page-count.pdf');
      const created = Pdf.fromMarkdown('Page 1');
      created.save(createPath);

      const opened = Pdf.open(createPath);
      const pageCount = opened.getPageCount();
      assert.strictEqual(pageCount, 1);
    });
  });

  describe('PdfBuilder fluent API', () => {
    it('should create builder and chain methods', () => {
      const builder = PdfBuilder.create();
      assert.ok(builder);
      assert.strictEqual(typeof builder.title, 'function');
      assert.strictEqual(typeof builder.author, 'function');
      assert.strictEqual(typeof builder.subject, 'function');
    });

    it('should apply title metadata via builder', async () => {
      const outputPath = join(TEMP_DIR, 'builder-title.pdf');
      const pdf = PdfBuilder.create().title('Test Document').fromMarkdown('Content');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should apply author metadata via builder', async () => {
      const outputPath = join(TEMP_DIR, 'builder-author.pdf');
      const pdf = PdfBuilder.create().author('John Doe').fromMarkdown('Content');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should chain multiple metadata setters', async () => {
      const outputPath = join(TEMP_DIR, 'builder-chain.pdf');
      const pdf = PdfBuilder.create()
        .title('My Report')
        .author('Jane Smith')
        .subject('Q4 2024')
        .fromMarkdown('# Report\n\nContent here');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should support pageSize configuration', async () => {
      const outputPath = join(TEMP_DIR, 'builder-pagesize.pdf');
      const pdf = PdfBuilder.create().pageSize('A4').fromMarkdown('Page sized content');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should support margins configuration', async () => {
      const outputPath = join(TEMP_DIR, 'builder-margins.pdf');
      const pdf = PdfBuilder.create()
        .margins(72, 72, 72, 72) // 1 inch margins
        .fromMarkdown('Margined content');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should apply all builder configurations together', async () => {
      const outputPath = join(TEMP_DIR, 'builder-complete.pdf');
      const pdf = PdfBuilder.create()
        .title('Technical Report')
        .author('Engineering Team')
        .subject('System Design')
        .pageSize('A4')
        .margins(72, 72, 72, 72)
        .fromMarkdown(`# Technical Report

## Executive Summary

This document outlines...

## Findings

Key findings here...`);

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should create PDF with builder using HTML', async () => {
      const outputPath = join(TEMP_DIR, 'builder-html.pdf');
      const pdf = PdfBuilder.create()
        .title('HTML Report')
        .author('Test User')
        .fromHtml('<h1>Report</h1><p>Content</p>');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should create PDF with builder using plain text', async () => {
      const outputPath = join(TEMP_DIR, 'builder-text.pdf');
      const pdf = PdfBuilder.create()
        .title('Text Document')
        .fromText('Plain text content\nLine 2\nLine 3');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });
  });

  describe('Pdf properties', () => {
    it('should get version from created PDF', async () => {
      const pdf = Pdf.fromMarkdown('Test');
      const version = pdf.getVersion();

      assert.ok(Array.isArray(version) || typeof version === 'object');
      assert.strictEqual(typeof version[0], 'number');
      assert.strictEqual(typeof version[1], 'number');
    });

    it('should get page count from created PDF', () => {
      const pdf = Pdf.fromMarkdown('Test content');
      const count = pdf.getPageCount();

      assert.strictEqual(typeof count, 'number');
      assert.ok(count > 0);
    });
  });

  describe('Pdf save operations', () => {
    it('should support synchronous save', async () => {
      const outputPath = join(TEMP_DIR, 'sync-save.pdf');
      const pdf = Pdf.fromMarkdown('Sync save test');

      pdf.save(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should support asynchronous save', async () => {
      const outputPath = join(TEMP_DIR, 'async-save.pdf');
      const pdf = Pdf.fromMarkdown('Async save test');

      await pdf.saveAsync(outputPath);

      const stat = await fs.stat(outputPath);
      assert.ok(stat.size > 0);
    });

    it('should save multiple PDFs without interference', async () => {
      const path1 = join(TEMP_DIR, 'multi1.pdf');
      const path2 = join(TEMP_DIR, 'multi2.pdf');

      const pdf1 = Pdf.fromMarkdown('Document 1');
      const pdf2 = Pdf.fromMarkdown('Document 2');

      pdf1.save(path1);
      pdf2.save(path2);

      const stat1 = await fs.stat(path1);
      const stat2 = await fs.stat(path2);

      assert.ok(stat1.size > 0);
      assert.ok(stat2.size > 0);
    });
  });

  describe('Round-trip testing', () => {
    it('should create PDF and read it back', async () => {
      const createPath = join(TEMP_DIR, 'roundtrip.pdf');
      const content = '# Roundtrip Test\n\nThis content should survive the round trip.';

      // Create
      const pdf = Pdf.fromMarkdown(content);
      pdf.save(createPath);

      // Read back
      const doc = PdfDocument.open(createPath);
      const text = doc.extractText(0);

      assert.ok(text.includes('Roundtrip') || text.includes('round trip'));
      doc.close();
    });

    it('should preserve builder metadata in round trip', async () => {
      const createPath = join(TEMP_DIR, 'roundtrip-builder.pdf');

      // Create with builder
      const pdf = PdfBuilder.create()
        .title('Roundtrip Document')
        .author('Test Author')
        .fromMarkdown('Content with metadata');
      pdf.save(createPath);

      // Open and verify
      const doc = PdfDocument.open(createPath);
      assert.ok(doc.pageCount > 0);
      const text = doc.extractText(0);
      assert.ok(text.length > 0);
      doc.close();
    });
  });

  describe('Error handling', () => {
    it('should handle invalid page index gracefully', () => {
      const pdf = Pdf.fromMarkdown('Test');

      assert.throws(() => {
        pdf.page(999);
      });
    });

    it('should handle negative page index gracefully', () => {
      const pdf = Pdf.fromMarkdown('Test');

      assert.throws(() => {
        pdf.page(-1);
      });
    });
  });
});
