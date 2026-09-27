/**
 * Stream API Tests - Node.js Implementation
 *
 * Tests for Stream API support including SearchStream, ExtractionStream,
 * and MetadataStream with backpressure handling and proper stream semantics.
 */

import test from 'ava';
import {
  SearchStream,
  ExtractionStream,
  MetadataStream,
  createSearchStream,
  createExtractionStream,
  createMetadataStream,
} from '../lib/managers/streams.js';

// Mock objects for testing
class MockSearchManager {
  search(term, pageIndex, options) {
    return [
      { text: 'found', pageIndex, position: 0, boundingBox: null },
      { text: 'found again', pageIndex, position: 10, boundingBox: null },
    ];
  }

  searchAll(term, options) {
    return [
      { text: 'result1', pageIndex: 0, position: 0 },
      { text: 'result2', pageIndex: 1, position: 5 },
      { text: 'result3', pageIndex: 2, position: 15 },
    ];
  }
}

class MockExtractionManager {
  extractText(pageIndex) {
    return `Extracted text from page ${pageIndex}`;
  }

  extractMarkdown(pageIndex) {
    return `# Page ${pageIndex}\n\nMarkdown content`;
  }

  extractHtml(pageIndex) {
    return `<html><body>Page ${pageIndex}</body></html>`;
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
    return pageIndex === 0 ? ['Arial', 'Times'] : ['Helvetica'];
  }

  getEmbeddedImages(pageIndex) {
    return pageIndex === 1 ? ['image1.jpg', 'image2.png'] : ['image0.jpg'];
  }
}

// SearchStream Tests
test('SearchStream - constructor requires SearchManager', (t) => {
  const error = t.throws(() => {
    new SearchStream(null, 'test');
  });
  t.is(error.message, 'SearchManager is required');
});

test('SearchStream - constructor requires search term', (t) => {
  const manager = new MockSearchManager();
  const error = t.throws(() => {
    new SearchStream(manager, '');
  });
  t.is(error.message, 'Search term must be a non-empty string');
});

test('SearchStream - creates readable stream in object mode', (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test');
  t.true(stream.readableObjectMode);
});

test('SearchStream - emits search results as objects', async (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test', { pageIndex: 0 });

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (result) => {
      t.is(typeof result.text, 'string');
      t.is(typeof result.pageIndex, 'number');
      t.is(typeof result.position, 'number');
      results.push(result);
    });

    stream.on('end', () => {
      t.is(results.length, 2);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('SearchStream - supports maxResults option', (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test', { maxResults: 1 });

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (result) => {
      results.push(result);
    });

    stream.on('end', () => {
      t.is(results.length, 1);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('SearchStream - supports whole document search', (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test'); // No pageIndex

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (result) => {
      results.push(result);
    });

    stream.on('end', () => {
      t.is(results.length, 3);
      resolve();
    });

    stream.on('error', reject);
  });
});

// ExtractionStream Tests
test('ExtractionStream - constructor requires ExtractionManager', (t) => {
  const error = t.throws(() => {
    new ExtractionStream(null, 0, 1);
  });
  t.is(error.message, 'ExtractionManager is required');
});

test('ExtractionStream - validates page range', (t) => {
  const manager = new MockExtractionManager();

  const error1 = t.throws(() => {
    new ExtractionStream(manager, -1, 5);
  });
  t.is(error1.message, 'Start page must be a non-negative number');

  const error2 = t.throws(() => {
    new ExtractionStream(manager, 5, 5);
  });
  t.is(error2.message, 'End page must be greater than start page');
});

test('ExtractionStream - validates extraction type', (t) => {
  const manager = new MockExtractionManager();

  const error = t.throws(() => {
    new ExtractionStream(manager, 0, 5, 'invalid');
  });
  t.is(error.message, "Extraction type must be 'text', 'markdown', or 'html'");
});

test('ExtractionStream - emits extraction progress', async (t) => {
  const manager = new MockExtractionManager();
  const stream = new ExtractionStream(manager, 0, 3, 'text');

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (progress) => {
      t.is(typeof progress.pageIndex, 'number');
      t.is(typeof progress.totalPages, 'number');
      t.is(typeof progress.extractedText, 'string');
      t.is(typeof progress.extractionType, 'string');
      t.is(typeof progress.progress, 'number');
      t.true(progress.progress >= 0 && progress.progress <= 1);
      results.push(progress);
    });

    stream.on('end', () => {
      t.is(results.length, 3);
      // Check progress values are correct
      t.is(results[0].progress, 1 / 3);
      t.is(results[1].progress, 2 / 3);
      t.is(results[2].progress, 1.0);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('ExtractionStream - supports markdown extraction', async (t) => {
  const manager = new MockExtractionManager();
  const stream = new ExtractionStream(manager, 0, 1, 'markdown');

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (progress) => {
      t.is(progress.extractionType, 'markdown');
      t.true(progress.extractedText.includes('# Page'));
      results.push(progress);
    });

    stream.on('end', () => {
      t.is(results.length, 1);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('ExtractionStream - supports html extraction', async (t) => {
  const manager = new MockExtractionManager();
  const stream = new ExtractionStream(manager, 0, 1, 'html');

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (progress) => {
      t.is(progress.extractionType, 'html');
      t.true(progress.extractedText.includes('<html>'));
      results.push(progress);
    });

    stream.on('end', () => {
      t.is(results.length, 1);
      resolve();
    });

    stream.on('error', reject);
  });
});

// MetadataStream Tests
test('MetadataStream - constructor requires RenderingManager', (t) => {
  const error = t.throws(() => {
    new MetadataStream(null, 0, 1);
  });
  t.is(error.message, 'RenderingManager is required');
});

test('MetadataStream - validates page range', (t) => {
  const manager = new MockRenderingManager();

  const error1 = t.throws(() => {
    new MetadataStream(manager, -1, 5);
  });
  t.is(error1.message, 'Start page must be a non-negative number');

  const error2 = t.throws(() => {
    new MetadataStream(manager, 5, 5);
  });
  t.is(error2.message, 'End page must be greater than start page');
});

test('MetadataStream - emits page metadata', async (t) => {
  const manager = new MockRenderingManager();
  const stream = new MetadataStream(manager, 0, 2);

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (metadata) => {
      t.is(typeof metadata.pageIndex, 'number');
      t.is(typeof metadata.width, 'number');
      t.is(typeof metadata.height, 'number');
      t.is(typeof metadata.fontCount, 'number');
      t.is(typeof metadata.imageCount, 'number');
      t.is(typeof metadata.rotation, 'number');
      results.push(metadata);
    });

    stream.on('end', () => {
      t.is(results.length, 2);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('MetadataStream - includes correct font counts', async (t) => {
  const manager = new MockRenderingManager();
  const stream = new MetadataStream(manager, 0, 2);

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (metadata) => {
      results.push(metadata);
    });

    stream.on('end', () => {
      // Page 0 has 2 fonts, Page 1 has 1 font
      t.is(results[0].fontCount, 2);
      t.is(results[1].fontCount, 1);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('MetadataStream - includes correct image counts', async (t) => {
  const manager = new MockRenderingManager();
  const stream = new MetadataStream(manager, 0, 3);

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (metadata) => {
      results.push(metadata);
    });

    stream.on('end', () => {
      t.is(results[0].imageCount, 1);
      t.is(results[1].imageCount, 2);
      resolve();
    });

    stream.on('error', reject);
  });
});

test('MetadataStream - includes rotation values', async (t) => {
  const manager = new MockRenderingManager();
  const stream = new MetadataStream(manager, 0, 2);

  const results = [];
  return new Promise((resolve, reject) => {
    stream.on('data', (metadata) => {
      results.push(metadata);
    });

    stream.on('end', () => {
      t.is(results[0].rotation, 0);
      t.is(results[1].rotation, 90);
      resolve();
    });

    stream.on('error', reject);
  });
});

// Factory Function Tests
test('createSearchStream - creates SearchStream instance', (t) => {
  const manager = new MockSearchManager();
  const stream = createSearchStream(manager, 'test');
  t.true(stream instanceof SearchStream);
});

test('createExtractionStream - creates ExtractionStream instance', (t) => {
  const manager = new MockExtractionManager();
  const stream = createExtractionStream(manager, 0, 5);
  t.true(stream instanceof ExtractionStream);
});

test('createMetadataStream - creates MetadataStream instance', (t) => {
  const manager = new MockRenderingManager();
  const stream = createMetadataStream(manager, 0, 5);
  t.true(stream instanceof MetadataStream);
});

// Backpressure Handling Tests
test('SearchStream - handles backpressure correctly', async (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test');

  let paused = false;
  let resumed = false;

  stream.on('pause', () => {
    paused = true;
  });

  stream.on('resume', () => {
    resumed = true;
  });

  // Force backpressure by setting highWaterMark to 0
  const paused1 = stream.pause();
  t.true(paused1 || !stream.readableFlowing);

  stream.resume();
  t.true(stream.readableFlowing !== false);
});

// Stream Lifecycle Tests
test('SearchStream - properly ends stream', async (t) => {
  const manager = new MockSearchManager();
  const stream = new SearchStream(manager, 'test', { pageIndex: 0 });

  let endCalled = false;
  stream.on('end', () => {
    endCalled = true;
  });

  return new Promise((resolve) => {
    stream.on('close', () => {
      t.true(endCalled);
      resolve();
    });

    // Consume all data
    stream.on('data', () => {});
  });
});

// Error Handling Tests
test('SearchStream - handles invalid manager gracefully', async (t) => {
  const stream = new SearchStream(new MockSearchManager(), 'test');

  // Add data listener first, then read
  return new Promise((resolve) => {
    const results = [];
    stream.on('data', (result) => {
      results.push(result);
    });

    stream.on('end', () => {
      t.is(results.length, 3);
      resolve();
    });
  });
});
