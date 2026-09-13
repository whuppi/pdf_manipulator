#!/usr/bin/env node

/**
 * Example: Reading and Text Extraction
 *
 * Demonstrates:
 * - Opening a PDF file
 * - Getting PDF metadata (version, page count)
 * - Extracting text from pages
 * - Converting pages to Markdown
 * - Converting pages to HTML
 */

const { PdfDocument } = require('../index.js');
const fs = require('fs').promises;
const path = require('path');

async function main() {
  const pdfPath = process.argv[2];

  if (!pdfPath) {
    console.error('Usage: node read-extract.js <pdf-file>');
    process.exit(1);
  }

  try {
    console.log(`Opening PDF: ${pdfPath}\n`);

    // Using explicit resource management (using statement)
    // For now, we'll use try/finally for cleanup
    let doc;
    try {
      doc = PdfDocument.open(pdfPath);

      // Get PDF metadata
      const { major, minor } = doc.getVersion();
      const pageCount = doc.getPageCount();
      const hasStructure = doc.hasStructureTree();

      console.log('=== PDF Metadata ===');
      console.log(`Version: ${major}.${minor}`);
      console.log(`Pages: ${pageCount}`);
      console.log(`Tagged PDF: ${hasStructure ? 'Yes' : 'No'}`);
      console.log();

      // Extract text from first page
      console.log('=== Extracting Text from Page 1 ===');
      const text = doc.extractText(0);
      console.log(text.substring(0, 500) + (text.length > 500 ? '...' : ''));
      console.log();

      // Convert first page to Markdown
      console.log('=== Converting Page 1 to Markdown ===');
      const markdown = doc.toMarkdown(0, {
        detectHeadings: true,
        preserveLayout: false,
        includeImages: true,
        embedImages: true,
      });
      console.log(markdown.substring(0, 500) + (markdown.length > 500 ? '...' : ''));
      console.log();

      // Convert first page to HTML
      console.log('=== Converting Page 1 to HTML ===');
      const html = doc.toHtml(0, {
        detectHeadings: true,
        preserveLayout: false,
      });
      console.log(html.substring(0, 500) + (html.length > 500 ? '...' : ''));
      console.log();

      // Save all pages to Markdown file
      const outputPath = path.join(path.dirname(pdfPath), path.basename(pdfPath, '.pdf') + '.md');
      const allMarkdown = doc.toMarkdownAll();
      await fs.writeFile(outputPath, allMarkdown);
      console.log(`Saved all pages to Markdown: ${outputPath}`);
    } finally {
      if (doc) {
        doc.close();
      }
    }
  } catch (error) {
    console.error('Error:', error.message);
    process.exit(1);
  }
}

main();
