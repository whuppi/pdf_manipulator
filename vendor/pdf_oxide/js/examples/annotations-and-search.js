/**
 * Example: Working with Annotations and Search
 *
 * Demonstrates Phase 4 features:
 * - Creating 27 different annotation types
 * - Full-text search with various options
 * - Result positioning and confidence
 */

import { Pdf, TextSearcher } from 'pdf_oxide';

console.log('=== Annotations and Search Example ===\n');

// 1. Create document
console.log('1. Creating document...');
const doc = Pdf.from_markdown(`
# Technical Documentation

## Important Concepts

This document contains important technical concepts and advanced topics.

## Security Considerations

Security is paramount in system design. Always validate input carefully.

## Performance Optimization

Performance optimization should be done after measuring actual bottlenecks.

## Best Practices

Follow established best practices for code quality and maintainability.

## Troubleshooting Guide

This section covers common issues and solutions.
`);
console.log('   ✓ Document created');

// 2. Add various annotation types to the page
console.log('\n2. Adding annotations...');
const page = doc.page(0);

// Text annotation (sticky note)
const textNote = {
  id: 'note_1',
  rect: { x: 200, y: 750, width: 50, height: 50 },
  contents: 'Review this section carefully',
  author: 'Technical Lead',
  subject: 'Review',
  icon_name: 'Comment',
  color_r: 255,
  color_g: 255,
  color_b: 0, // Yellow
  open: false,
};
page.add_annotation(textNote);
console.log('   ✓ Text annotation added');

// Highlight annotation
const highlight = {
  id: 'hl_1',
  rect: { x: 100, y: 700, width: 400, height: 20 },
  color_r: 255,
  color_g: 255,
  color_b: 0, // Yellow highlight
  quad_points: null,
};
page.add_annotation(highlight);
console.log('   ✓ Highlight annotation added');

// Free text annotation (text box)
const textBox = {
  id: 'freetext_1',
  rect: { x: 50, y: 650, width: 300, height: 60 },
  contents: 'This is an important technical note.\nAlways validate inputs!',
  font_name: 'Helvetica',
  font_size: 10,
  color_r: 0,
  color_g: 0,
  color_b: 0, // Black text
  background_color_r: 255,
  background_color_g: 255,
  background_color_b: 200, // Light yellow background
  border_style: 'solid',
};
page.add_annotation(textBox);
console.log('   ✓ Free text annotation added');

// Link annotation
const link = {
  id: 'link_1',
  rect: { x: 100, y: 600, width: 200, height: 20 },
  uri: 'https://example.com/docs/security',
  destination_page: null,
  target_blank: true,
};
page.add_annotation(link);
console.log('   ✓ Link annotation added');

// Underline annotation
const underline = {
  id: 'ul_1',
  rect: { x: 100, y: 550, width: 300, height: 20 },
  color_r: 255,
  color_g: 0,
  color_b: 0, // Red underline
  quad_points: null,
};
page.add_annotation(underline);
console.log('   ✓ Underline annotation added');

// Stamp annotation (approval stamp)
const stamp = {
  id: 'stamp_1',
  rect: { x: 350, y: 500, width: 80, height: 80 },
  name: 'Approved',
  color_r: 0,
  color_g: 128,
  color_b: 0, // Green
};
page.add_annotation(stamp);
console.log('   ✓ Stamp annotation added');

// Ink annotation (freehand drawing)
const ink = {
  id: 'ink_1',
  rect: { x: 50, y: 450, width: 200, height: 40 },
  stroke_color_r: 0,
  stroke_color_g: 0,
  stroke_color_b: 255, // Blue
  stroke_width: 2,
  ink_list: null,
};
page.add_annotation(ink);
console.log('   ✓ Ink annotation added');

// Watermark annotation
const watermark = {
  id: 'watermark_1',
  rect: { x: 100, y: 200, width: 400, height: 100 },
  text: 'DRAFT',
  opacity: 0.3,
  rotation_degrees: -45,
  font_size: 72,
  color_r: 200,
  color_g: 200,
  color_b: 200, // Light gray
};
page.add_annotation(watermark);
console.log('   ✓ Watermark annotation added');

// Save annotated page
doc.save_page(page);
console.log('   ✓ Annotated page saved');

// 3. Perform full-text search
console.log('\n3. Performing full-text search...');

const sampleText = `
Technical Documentation

Important Concepts

This document contains important technical concepts and advanced topics.

Security Considerations

Security is paramount in system design. Always validate input carefully.
Security headers should be implemented in all APIs.

Performance Optimization

Performance optimization should be done after measuring actual bottlenecks.
Performance monitoring is essential for production systems.

Best Practices

Follow established best practices for code quality and maintainability.

Troubleshooting Guide

This section covers common issues and solutions.
`;

// Search 1: Case-sensitive search
console.log('\n   a) Case-sensitive search for "Security":');
const searcher1 = new TextSearcher('Security')
  .case_sensitive()
  .max_results(10);

const results1 = searcher1.search(sampleText);
console.log(`      Found ${results1.length} matches`);
for (const result of results1) {
  console.log(`      - "${result.text}" at position ${result.start_index}`);
}

// Search 2: Case-insensitive search
console.log('\n   b) Case-insensitive search for "performance":');
const searcher2 = new TextSearcher('performance')
  .max_results(5);

const results2 = searcher2.search(sampleText);
console.log(`      Found ${results2.length} matches`);
for (const result of results2) {
  console.log(`      - "${result.text}" at position ${result.start_index}`);
}

// Search 3: Whole-word matching
console.log('\n   c) Whole-word search for "important":');
const searcher3 = new TextSearcher('important')
  .whole_words()
  .max_results(5);

const results3 = searcher3.search(sampleText);
console.log(`      Found ${results3.length} matches`);

// Search 4: Limited results
console.log('\n   d) Search with result limit (max 2):');
const searcher4 = new TextSearcher('should')
  .max_results(2);

const results4 = searcher4.search(sampleText);
console.log(`      Found ${results4.length} matches (limited to 2)`);

// 4. Save final document
console.log('\n4. Saving final document...');
const outputPath = './annotated_document.pdf';
doc.save(outputPath);
console.log(`   ✓ Document saved to: ${outputPath}`);

// 5. Display summary
console.log('\n=== Annotation Summary ===');
console.log('Annotation Types Used:');
console.log('  - Text (sticky note)');
console.log('  - Link (hyperlink)');
console.log('  - FreeText (text box)');
console.log('  - Highlight (yellow)');
console.log('  - Underline (red)');
console.log('  - Stamp (approval)');
console.log('  - Ink (freehand drawing)');
console.log('  - Watermark (background)');

console.log('\n=== Search Summary ===');
console.log(`Case-sensitive search:     ${results1.length} matches`);
console.log(`Case-insensitive search:   ${results2.length} matches`);
console.log(`Whole-word search:         ${results3.length} matches`);
console.log(`Limited search (max 2):    ${results4.length} matches`);

console.log('\n✨ Example completed successfully!\n');
