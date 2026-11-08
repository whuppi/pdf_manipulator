#!/usr/bin/env node

/**
 * Example: Creating PDFs
 *
 * Demonstrates:
 * - Creating PDF from Markdown
 * - Creating PDF from HTML
 * - Creating PDF from plain text
 * - Using PdfBuilder for advanced configuration
 * - Saving PDFs asynchronously
 */

const { Pdf, PdfBuilder, PageSize } = require('../index.js');
const path = require('path');

async function main() {
  try {
    // Example 1: Simple PDF from Markdown
    console.log('=== Creating Simple PDF from Markdown ===');
    let doc = Pdf.fromMarkdown(`
# Hello World

This is a simple PDF created from Markdown.

## Features

- **Text extraction** - Extract text with automatic reading order
- **Format conversion** - Convert to Markdown, HTML, or plain text
- **PDF creation** - Create from Markdown, HTML, or text
- **Editing** - Edit existing PDFs with DOM-like navigation
    `);
    doc.save('simple.pdf');
    console.log('Saved: simple.pdf\n');

    // Example 2: PDF from HTML
    console.log('=== Creating PDF from HTML ===');
    doc = Pdf.fromHtml(`
<html>
  <head><title>HTML PDF</title></head>
  <body>
    <h1>PDF from HTML</h1>
    <p>This PDF was created from HTML content.</p>
    <ul>
      <li>Supports semantic HTML</li>
      <li>Preserves structure</li>
      <li>Auto-detects headings</li>
    </ul>
  </body>
</html>
    `);
    doc.save('from-html.pdf');
    console.log('Saved: from-html.pdf\n');

    // Example 3: PDF from plain text
    console.log('=== Creating PDF from Text ===');
    doc = Pdf.fromText(`
PDF Oxide - Complete PDF Toolkit

This is a simple text document converted to PDF.

Key Features:
- Text Extraction
- PDF Creation
- PDF Editing
- Format Conversion
- Full Text Search
- Annotation Support
- Form Processing
    `);
    doc.save('from-text.pdf');
    console.log('Saved: from-text.pdf\n');

    // Example 4: Advanced PDF with PdfBuilder
    console.log('=== Creating Advanced PDF with PdfBuilder ===');
    doc = PdfBuilder.create()
      .title('Quarterly Report Q4 2024')
      .author('John Doe')
      .subject('2024 Financial Results')
      .pageSize(PageSize.A4)
      .margins(72, 72, 72, 72) // 1 inch margins
      .fromMarkdown(`
# Quarterly Report - Q4 2024

## Executive Summary

This quarter exceeded expectations with strong growth across all segments.

### Key Metrics

- Revenue: \$10.5M (+15% YoY)
- Profit Margin: 28% (+2%)
- Customer Growth: 450 new accounts
- Churn Rate: 2.1% (-0.5%)

## Strategic Initiatives

1. **Market Expansion** - Entered 3 new regions
2. **Product Enhancement** - Launched v2.0 with AI features
3. **Team Growth** - Hired 25 new engineers

## Outlook

For Q1 2025, we expect continued growth driven by:
- New product launches
- Expansion into APAC
- Strategic partnerships

---

*Report generated with pdf_oxide Node.js bindings*
      `);

    // Save asynchronously
    await doc.saveAsync('report.pdf');
    console.log('Saved: report.pdf\n');

    console.log('✓ All PDFs created successfully!');
  } catch (error) {
    console.error('Error:', error.message);
    process.exit(1);
  }
}

main();
