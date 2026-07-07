# Stream API for PDF Oxide Node.js

## Overview

Phase 2.4 adds comprehensive Stream API support to PDF Oxide Node.js, enabling idiomatic Node.js streaming patterns for PDF operations. These readable streams handle backpressure automatically and integrate seamlessly with Node.js ecosystem tools.

## Key Features

✅ **Readable Streams** - Standard Node.js stream interface
✅ **Backpressure Handling** - Automatic flow control
✅ **Object Mode** - Streams emit objects, not buffers
✅ **Memory Efficient** - Streams results one at a time
✅ **Pipe-Compatible** - Works with `pipe()` and other stream transformers
✅ **Promise-Compatible** - Can be used with async/await
✅ **Error Handling** - Proper error propagation through streams

## The Three Streams

### 1. SearchStream

A readable stream that emits search results one at a time.

#### Basic Usage

```javascript
import { SearchStream, SearchManager } from 'pdf_oxide';

const doc = PdfDocument.open('document.pdf');
const searchManager = new SearchManager(doc);

// Create a stream to search for 'invoice'
const stream = new SearchStream(searchManager, 'invoice');

stream.on('data', (result) => {
  console.log(`Found on page ${result.pageIndex + 1}: ${result.text}`);
});

stream.on('end', () => {
  console.log('Search complete');
});
```

#### SearchStream Options

```javascript
// Page-specific search
new SearchStream(manager, 'keyword', {
  pageIndex: 0,           // Search only page 0
  caseSensitive: false,   // Case-insensitive
  wholeWords: false,      // Include partial matches
  maxResults: 100         // Limit results
});

// Document-wide search (default)
new SearchStream(manager, 'keyword');  // No pageIndex = search all pages
```

#### SearchResult Object

Each data event emits a SearchResult object:

```javascript
{
  text: string,                    // Matched text
  pageIndex: number,               // Zero-based page number
  position: number,                // Position within page
  boundingBox: {                   // Optional
    x: number,
    y: number,
    width: number,
    height: number
  }
}
```

#### Using with Pipes

```javascript
import { createReadStream, createWriteStream } from 'node:fs';
import { pipeline } from 'node:stream/promises';
import { Transform } from 'node:stream';

// Create a transform to format results
const formatter = new Transform({
  objectMode: true,
  transform(result, encoding, callback) {
    const line = `Page ${result.pageIndex + 1}: ${result.text}\n`;
    callback(null, line);
  }
});

// Pipe search results to file
await pipeline(
  new SearchStream(manager, 'invoice'),
  formatter,
  createWriteStream('search_results.txt')
);
```

#### Using with Promises

```javascript
import { pipeline } from 'node:stream/promises';
import { Writable } from 'node:stream';

// Collect all results
const results = [];
await pipeline(
  new SearchStream(manager, 'error'),
  new Writable({
    objectMode: true,
    write(result, encoding, callback) {
      results.push(result);
      callback();
    }
  })
);

console.log(`Found ${results.length} errors`);
```

---

### 2. ExtractionStream

A readable stream that emits extraction progress with extracted text for each page.

#### Basic Usage

```javascript
import { ExtractionStream, ExtractionManager } from 'pdf_oxide';

const doc = PdfDocument.open('document.pdf');
const extractionManager = new ExtractionManager(doc);

// Extract pages 0-10 as text
const stream = new ExtractionStream(extractionManager, 0, 10, 'text');

stream.on('data', (progress) => {
  const percent = Math.round(progress.progress * 100);
  console.log(`[${percent}%] Page ${progress.pageIndex + 1}: ${progress.extractedText.length} chars`);
});

stream.on('end', () => {
  console.log('Extraction complete');
});
```

#### Extraction Types

```javascript
// Plain text extraction (default)
new ExtractionStream(manager, 0, 10, 'text');

// Markdown extraction with structure
new ExtractionStream(manager, 0, 10, 'markdown');

// HTML extraction with formatting
new ExtractionStream(manager, 0, 10, 'html');
```

#### ExtractionProgress Object

Each data event emits an ExtractionProgress object:

```javascript
{
  pageIndex: number,           // Current page (0-indexed)
  totalPages: number,          // Total pages in range
  extractedText: string,       // Extracted content
  extractionType: string,      // 'text', 'markdown', or 'html'
  progress: number             // 0.0 to 1.0 completion
}
```

#### Visual Progress Indicator

```javascript
import blessed from 'blessed';

const screen = blessed.screen();
const progressBar = blessed.progressbar({
  parent: screen,
  top: 0,
  left: 0,
  width: '100%',
  height: 3
});

const stream = new ExtractionStream(extractionManager, 0, 100, 'markdown');

stream.on('data', (progress) => {
  progressBar.setProgress(progress.progress * 100);
});
```

#### Collecting All Extracted Text

```javascript
import { Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const textCollector = [];

await pipeline(
  new ExtractionStream(manager, 0, 50, 'text'),
  new Transform({
    objectMode: true,
    transform(progress, encoding, callback) {
      textCollector.push(progress.extractedText);
      callback();
    }
  })
);

const allText = textCollector.join('\n---\n');
console.log(`Extracted ${allText.length} total characters`);
```

---

### 3. MetadataStream

A readable stream that emits page metadata (dimensions, fonts, images, rotation).

#### Basic Usage

```javascript
import { MetadataStream, RenderingManager } from 'pdf_oxide';

const doc = PdfDocument.open('document.pdf');
const renderingManager = new RenderingManager(doc);

// Get metadata for pages 0-10
const stream = new MetadataStream(renderingManager, 0, 10);

stream.on('data', (metadata) => {
  console.log(`Page ${metadata.pageIndex + 1}:`);
  console.log(`  Size: ${metadata.width} × ${metadata.height} points`);
  console.log(`  Fonts: ${metadata.fontCount}`);
  console.log(`  Images: ${metadata.imageCount}`);
});

stream.on('end', () => {
  console.log('Metadata retrieval complete');
});
```

#### PageMetadata Object

Each data event emits a PageMetadata object:

```javascript
{
  pageIndex: number,      // Zero-based page number
  width: number,          // Page width in points
  height: number,         // Page height in points
  fontCount: number,      // Embedded font count
  imageCount: number,     // Embedded image count
  rotation: number        // 0, 90, 180, or 270 degrees
}
```

#### Document Analysis

```javascript
import { Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const stats = {
  totalImages: 0,
  totalFonts: 0,
  averageWidth: 0,
  averageHeight: 0,
  pageCount: 0
};

await pipeline(
  new MetadataStream(manager, 0, doc.pageCount),
  new Transform({
    objectMode: true,
    transform(metadata, encoding, callback) {
      stats.totalImages += metadata.imageCount;
      stats.totalFonts += metadata.fontCount;
      stats.averageWidth += metadata.width;
      stats.averageHeight += metadata.height;
      stats.pageCount++;
      callback();
    }
  })
);

stats.averageWidth /= stats.pageCount;
stats.averageHeight /= stats.pageCount;

console.log('Document Statistics:');
console.log(`  Pages: ${stats.pageCount}`);
console.log(`  Total images: ${stats.totalImages}`);
console.log(`  Total fonts: ${stats.totalFonts}`);
console.log(`  Average size: ${stats.averageWidth} × ${stats.averageHeight}`);
```

---

## Factory Functions

Convenient factory functions for creating streams:

```javascript
import {
  createSearchStream,
  createExtractionStream,
  createMetadataStream
} from 'pdf_oxide';

// These are equivalent to using constructors
const searchStream = createSearchStream(manager, 'keyword');
const extractionStream = createExtractionStream(manager, 0, 10, 'text');
const metadataStream = createMetadataStream(manager, 0, 10);
```

---

## Stream Event Handling

All streams support standard Node.js stream events:

```javascript
const stream = new SearchStream(manager, 'text');

// Data events
stream.on('data', (result) => {
  console.log('New result:', result);
});

// End event
stream.on('end', () => {
  console.log('Stream ended');
});

// Error events
stream.on('error', (error) => {
  console.error('Stream error:', error);
});

// Pause/resume
stream.on('pause', () => {
  console.log('Stream paused');
});

stream.on('resume', () => {
  console.log('Stream resumed');
});

// Close event
stream.on('close', () => {
  console.log('Stream closed');
});

// Drain event (backpressure)
stream.on('drain', () => {
  console.log('Internal buffer drained');
});
```

---

## Backpressure Handling

Streams automatically handle backpressure - the internal mechanism that prevents memory overflow when consuming streams faster than they're produced.

```javascript
import { createWriteStream } from 'node:fs';

const searchStream = new SearchStream(manager, 'data');
const fileStream = createWriteStream('output.jsonl');

// Pipe automatically handles backpressure
searchStream.pipe(fileStream);
```

Manual backpressure handling:

```javascript
const stream = new SearchStream(manager, 'keyword');
let keepReading = true;

stream.on('data', (result) => {
  const shouldContinue = process.stdout.write(JSON.stringify(result) + '\n');

  if (!shouldContinue) {
    console.log('Pausing due to backpressure...');
    stream.pause();
  }
});

process.stdout.on('drain', () => {
  console.log('Resuming after drain...');
  stream.resume();
});
```

---

## Advanced Patterns

### Combining Multiple Streams

```javascript
import { PassThrough } from 'node:stream';

// Create combined analysis
const combined = new PassThrough({ objectMode: true });

const searchResults = new SearchStream(searchManager, 'error');
const pageMetadata = new MetadataStream(renderingManager, 0, doc.pageCount);

// Merge both streams
searchResults.on('data', (result) => {
  combined.write({ type: 'search', data: result });
});

pageMetadata.on('data', (metadata) => {
  combined.write({ type: 'metadata', data: metadata });
});

searchResults.on('end', () => {
  pageMetadata.on('end', () => {
    combined.end();
  });
});

combined.on('data', (item) => {
  console.log(`${item.type}:`, item.data);
});
```

### Filtering Stream Results

```javascript
import { Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';

const filterPages = new Transform({
  objectMode: true,
  transform(result, encoding, callback) {
    // Only emit results from pages 5 and beyond
    if (result.pageIndex >= 5) {
      callback(null, result);
    } else {
      callback();
    }
  }
});

await pipeline(
  new SearchStream(manager, 'keyword'),
  filterPages
);
```

### Rate Limiting

```javascript
import { Transform } from 'node:stream';

const rateLimiter = new Transform({
  objectMode: true,
  highWaterMark: 10,

  transform(result, encoding, callback) {
    setTimeout(() => {
      callback(null, result);
    }, 100); // 100ms delay per result
  }
});

const stream = new SearchStream(manager, 'keyword');

stream
  .pipe(rateLimiter)
  .on('data', (result) => {
    console.log('Processing:', result.text);
  });
```

---

## Error Handling

Proper error handling with streams:

```javascript
const stream = new SearchStream(manager, 'test');

stream.on('error', (error) => {
  console.error('Search stream error:', error.message);
  // Handle error - stream is destroyed
});

// Or with pipeline
import { pipeline } from 'node:stream/promises';

try {
  await pipeline(
    new ExtractionStream(manager, 0, 100),
    processStream,
    outputStream
  );
} catch (error) {
  console.error('Pipeline failed:', error);
}
```

---

## Performance Considerations

### Memory Efficiency

Streams process one item at a time, perfect for large result sets:

```javascript
// ❌ Memory intensive - loads all 100k results into memory
const allResults = manager.searchAll('keyword');

// ✅ Memory efficient - streams one result at a time
const stream = new SearchStream(manager, 'keyword');
stream.on('data', (result) => {
  // Process one result
});
```

### Controlling Flow

```javascript
const stream = new SearchStream(manager, 'keyword');
let processed = 0;

stream.on('data', (result) => {
  processed++;

  // Stop after 1000 results
  if (processed >= 1000) {
    stream.destroy();
  }
});
```

---

## Integration with Popular Libraries

### Using with database insertion

```javascript
import { pipeline } from 'node:stream/promises';
import { Transform } from 'node:stream';
import sqlite3 from 'sqlite3';

const db = new sqlite3.Database(':memory:');

await pipeline(
  new SearchStream(manager, 'invoice'),
  new Transform({
    objectMode: true,
    async transform(result, encoding, callback) {
      db.run(
        'INSERT INTO results (text, page, position) VALUES (?, ?, ?)',
        [result.text, result.pageIndex, result.position],
        callback
      );
    }
  })
);
```

### Using with Express response

```javascript
import express from 'express';
import { createSearchStream } from 'pdf_oxide';

const app = express();

app.get('/search/:term', (req, res) => {
  const stream = createSearchStream(manager, req.params.term);

  res.setHeader('Content-Type', 'application/x-ndjson');
  stream.pipe(res);

  stream.on('error', (error) => {
    res.status(500).json({ error: error.message });
  });
});
```

---

## Backward Compatibility

All streams are **purely additive** - existing APIs remain unchanged:

```javascript
// Original synchronous API still works
const results = manager.searchAll('keyword');

// New streaming API is optional
const stream = createSearchStream(manager, 'keyword');
stream.on('data', (result) => {
  console.log(result);
});
```

---

## References

- [Node.js Stream Documentation](https://nodejs.org/api/stream.html)
- [Stream Handbook](https://github.com/substack/stream-handbook)
- [Stream Backpressure Guide](https://nodejs.org/en/docs/guides/backpressuring-in-streams/)
