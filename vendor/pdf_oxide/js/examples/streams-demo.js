/**
 * Stream API Demo - PDF Oxide Node.js
 *
 * Comprehensive examples demonstrating Stream API usage including:
 * - SearchStream for result streaming
 * - ExtractionStream for extraction with progress
 * - MetadataStream for page metadata
 * - Pipe composition and backpressure handling
 * - Error handling and integration patterns
 */

import { pipeline } from 'node:stream/promises';
import { Transform, Writable } from 'node:stream';
import { createWriteStream } from 'node:fs';

import {
  SearchStream,
  ExtractionStream,
  MetadataStream,
  createSearchStream,
  createExtractionStream,
  createMetadataStream,
} from '../index.js';

// Mock setup for demonstration
class MockSearchManager {
  search(term, pageIndex, options) {
    return [
      { text: 'error found', pageIndex, position: 0, boundingBox: null },
      { text: 'error again', pageIndex, position: 50, boundingBox: null },
    ];
  }

  searchAll(term, options) {
    const results = [];
    for (let i = 0; i < 5; i++) {
      results.push({
        text: `${term} on page ${i}`,
        pageIndex: i,
        position: i * 10,
      });
    }
    return results;
  }
}

class MockExtractionManager {
  extractText(pageIndex) {
    return `Text from page ${pageIndex}. Lorem ipsum dolor sit amet.`;
  }

  extractMarkdown(pageIndex) {
    return `# Page ${pageIndex}\n\nMarkdown content for page ${pageIndex}.`;
  }

  extractHtml(pageIndex) {
    return `<html><body><p>Page ${pageIndex}</p></body></html>`;
  }
}

class MockRenderingManager {
  getPageDimensions(pageIndex) {
    return {
      width: 612,
      height: 792,
      rotation: pageIndex % 2 === 0 ? 0 : 90,
    };
  }

  getEmbeddedFonts(pageIndex) {
    return ['Arial', 'Times', 'Helvetica'];
  }

  getEmbeddedImages(pageIndex) {
    return pageIndex === 0 ? ['img1.jpg', 'img2.png'] : ['img3.jpg'];
  }
}

// =============================================================================
// Example 1: Basic Search Stream
// =============================================================================

async function example1BasicSearch() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 1: Basic Search Stream');
  console.log('='.repeat(70) + '\n');

  const manager = new MockSearchManager();

  // Create search stream
  const stream = new SearchStream(manager, 'error');

  let count = 0;
  console.log('Searching for "error" across entire document:\n');

  return new Promise((resolve) => {
    stream.on('data', (result) => {
      count++;
      console.log(
        `  ${count}. "${result.text}" on page ${result.pageIndex + 1} at position ${result.position}`
      );
    });

    stream.on('end', () => {
      console.log(`\n✓ Search complete: Found ${count} results`);
      resolve();
    });

    stream.on('error', (error) => {
      console.error('Stream error:', error);
      resolve();
    });
  });
}

// =============================================================================
// Example 2: Search Stream with Pipeline
// =============================================================================

async function example2PipelineSearch() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 2: Search Stream with Pipeline');
  console.log('='.repeat(70) + '\n');

  const manager = new MockSearchManager();

  // Create transform to format results
  const formatter = new Transform({
    objectMode: true,
    transform(result, encoding, callback) {
      const line = `Page ${result.pageIndex + 1}: "${result.text}"\n`;
      callback(null, line);
    },
  });

  // Create writable to collect output
  const output = [];
  const collector = new Writable({
    transform(chunk, encoding, callback) {
      output.push(chunk.toString());
      callback();
    },
  });

  console.log('Piping search results through formatter:\n');

  await pipeline(
    new SearchStream(manager, 'error'),
    formatter,
    collector
  );

  console.log('Formatted output:');
  output.forEach((line) => process.stdout.write(`  ${line}`));
  console.log('\n✓ Pipeline complete');
}

// =============================================================================
// Example 3: Extraction Stream with Progress
// =============================================================================

async function example3ExtractionProgress() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 3: Extraction Stream with Progress');
  console.log('='.repeat(70) + '\n');

  const manager = new MockExtractionManager();

  // Create extraction stream
  const stream = new ExtractionStream(manager, 0, 5, 'text');

  console.log('Extracting pages 0-4 with progress:\n');

  return new Promise((resolve) => {
    stream.on('data', (progress) => {
      const percent = Math.round(progress.progress * 100);
      const bar = '█'.repeat(Math.floor(percent / 5)) + '░'.repeat(20 - Math.floor(percent / 5));

      console.log(
        `  [${bar}] ${percent}% | Page ${progress.pageIndex + 1}/${progress.totalPages} | ` +
          `${progress.extractedText.length} chars`
      );
    });

    stream.on('end', () => {
      console.log('\n✓ Extraction complete');
      resolve();
    });

    stream.on('error', (error) => {
      console.error('Stream error:', error);
      resolve();
    });
  });
}

// =============================================================================
// Example 4: Multiple Extraction Formats
// =============================================================================

async function example4MultipleFormats() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 4: Multiple Extraction Formats');
  console.log('='.repeat(70) + '\n');

  const manager = new MockExtractionManager();

  for (const format of ['text', 'markdown', 'html']) {
    console.log(`Extracting as ${format}:\n`);

    const stream = new ExtractionStream(manager, 0, 2, format);

    const samples = [];
    await new Promise((resolve) => {
      stream.on('data', (progress) => {
        samples.push(progress.extractedText);
      });

      stream.on('end', resolve);
    });

    samples.forEach((text, i) => {
      const preview = text.substring(0, 50).replace(/\n/g, ' ');
      console.log(`  Page ${i}: ${preview}...`);
    });
    console.log();
  }

  console.log('✓ All formats extracted');
}

// =============================================================================
// Example 5: Metadata Stream Analysis
// =============================================================================

async function example5MetadataAnalysis() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 5: Metadata Stream Analysis');
  console.log('='.repeat(70) + '\n');

  const manager = new MockRenderingManager();

  // Create metadata stream
  const stream = new MetadataStream(manager, 0, 5);

  console.log('Analyzing page metadata:\n');

  const stats = {
    totalImages: 0,
    totalFonts: 0,
    totalWidth: 0,
    totalHeight: 0,
    rotations: {},
  };

  return new Promise((resolve) => {
    stream.on('data', (metadata) => {
      stats.totalImages += metadata.imageCount;
      stats.totalFonts += metadata.fontCount;
      stats.totalWidth += metadata.width;
      stats.totalHeight += metadata.height;
      stats.rotations[metadata.rotation] = (stats.rotations[metadata.rotation] || 0) + 1;

      console.log(`  Page ${metadata.pageIndex + 1}:`);
      console.log(`    Size: ${metadata.width} × ${metadata.height}`);
      console.log(`    Fonts: ${metadata.fontCount}, Images: ${metadata.imageCount}`);
      console.log(`    Rotation: ${metadata.rotation}°\n`);
    });

    stream.on('end', () => {
      const pageCount = 5;
      console.log('Document Statistics:');
      console.log(`  Total images: ${stats.totalImages}`);
      console.log(`  Total fonts: ${stats.totalFonts}`);
      console.log(`  Average size: ${stats.totalWidth / pageCount} × ${stats.totalHeight / pageCount}`);
      console.log(`  Rotations: ${JSON.stringify(stats.rotations)}`);
      console.log('\n✓ Analysis complete');
      resolve();
    });

    stream.on('error', (error) => {
      console.error('Stream error:', error);
      resolve();
    });
  });
}

// =============================================================================
// Example 6: Collecting All Results
// =============================================================================

async function example6CollectingResults() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 6: Collecting Stream Results');
  console.log('='.repeat(70) + '\n');

  const searchManager = new MockSearchManager();
  const extractionManager = new MockExtractionManager();

  console.log('Collecting all search results:\n');

  // Collect search results
  const searchResults = [];
  await new Promise((resolve) => {
    const stream = new SearchStream(searchManager, 'error');
    stream.on('data', (result) => searchResults.push(result));
    stream.on('end', resolve);
  });

  console.log(`  Collected ${searchResults.length} search results`);

  // Collect extraction results
  console.log('\nCollecting all extracted text:\n');

  const textResults = [];
  await new Promise((resolve) => {
    const stream = new ExtractionStream(extractionManager, 0, 3, 'text');
    stream.on('data', (progress) => textResults.push(progress.extractedText));
    stream.on('end', resolve);
  });

  console.log(`  Collected ${textResults.length} pages`);
  console.log(`  Total characters: ${textResults.join('').length}`);
  console.log('\n✓ Collection complete');
}

// =============================================================================
// Example 7: Filtering Stream Data
// =============================================================================

async function example7FilteringData() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 7: Filtering Stream Data');
  console.log('='.repeat(70) + '\n');

  const manager = new MockSearchManager();

  console.log('Filtering search results to page 2 and beyond:\n');

  // Create filter transform
  const filterPages = new Transform({
    objectMode: true,
    transform(result, encoding, callback) {
      if (result.pageIndex >= 2) {
        callback(null, result);
      } else {
        callback();
      }
    },
  });

  const filtered = [];

  await pipeline(
    new SearchStream(manager, 'error'),
    filterPages,
    new Writable({
      objectMode: true,
      write(result, encoding, callback) {
        filtered.push(result);
        console.log(`  Filtered: "${result.text}" on page ${result.pageIndex + 1}`);
        callback();
      },
    })
  );

  console.log(`\n✓ Filtered ${filtered.length} results`);
}

// =============================================================================
// Example 8: Factory Functions
// =============================================================================

async function example8FactoryFunctions() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 8: Factory Functions');
  console.log('='.repeat(70) + '\n');

  const searchManager = new MockSearchManager();
  const extractionManager = new MockExtractionManager();
  const renderingManager = new MockRenderingManager();

  console.log('Using factory functions:\n');

  // Create streams using factory functions
  const searchStream = createSearchStream(searchManager, 'error');
  const extractionStream = createExtractionStream(extractionManager, 0, 3, 'markdown');
  const metadataStream = createMetadataStream(renderingManager, 0, 3);

  console.log('  ✓ SearchStream created via createSearchStream()');
  console.log('  ✓ ExtractionStream created via createExtractionStream()');
  console.log('  ✓ MetadataStream created via createMetadataStream()');

  let count = 0;

  await new Promise((resolve) => {
    searchStream.on('data', () => count++);
    searchStream.on('end', resolve);
  });

  console.log(`\n  Search results: ${count}`);
  console.log('✓ Factory functions work correctly');
}

// =============================================================================
// Example 9: Error Handling
// =============================================================================

async function example9ErrorHandling() {
  console.log('\n' + '='.repeat(70));
  console.log('EXAMPLE 9: Error Handling');
  console.log('='.repeat(70) + '\n');

  const manager = new MockSearchManager();

  console.log('Demonstrating error handling patterns:\n');

  // Pattern 1: Event-based error handling
  console.log('  1. Event-based error handling:');

  try {
    await new Promise((resolve) => {
      const stream = new SearchStream(manager, 'test');

      stream.on('data', (result) => {
        console.log(`     Processing: ${result.text}`);
      });

      stream.on('error', (error) => {
        console.log(`     Error caught: ${error.message}`);
      });

      stream.on('end', () => {
        console.log('     Stream ended successfully');
        resolve();
      });
    });
  } catch (error) {
    console.log(`     Unexpected error: ${error.message}`);
  }

  // Pattern 2: Pipeline with try/catch
  console.log('\n  2. Pipeline error handling:');

  try {
    await pipeline(
      new SearchStream(manager, 'test'),
      new Transform({
        objectMode: true,
        transform(result, encoding, callback) {
          callback(null, result);
        },
      })
    );
    console.log('     Pipeline succeeded');
  } catch (error) {
    console.log(`     Pipeline error caught: ${error.message}`);
  }

  console.log('\n✓ Error handling patterns demonstrated');
}

// =============================================================================
// Main Demo Runner
// =============================================================================

async function runAllExamples() {
  console.log('\n' + '='.repeat(70));
  console.log('PDF OXIDE NODE.JS - STREAM API DEMO');
  console.log('='.repeat(70));

  try {
    await example1BasicSearch();
    await example2PipelineSearch();
    await example3ExtractionProgress();
    await example4MultipleFormats();
    await example5MetadataAnalysis();
    await example6CollectingResults();
    await example7FilteringData();
    await example8FactoryFunctions();
    await example9ErrorHandling();

    console.log('\n' + '='.repeat(70));
    console.log('✓ ALL EXAMPLES COMPLETED');
    console.log('='.repeat(70) + '\n');
  } catch (error) {
    console.error('Error running examples:', error);
    process.exit(1);
  }
}

// Run examples
runAllExamples().catch(console.error);
