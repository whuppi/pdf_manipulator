# PDF Oxide Node.js/TypeScript Complete Guide

## Table of Contents

1. [Overview](#overview)
2. [Installation](#installation)
3. [Quick Start](#quick-start)
4. [Core API](#core-api)
5. [Builders (Fluent Configuration)](#builders-fluent-configuration)
6. [Managers (Domain-specific Operations)](#managers-domain-specific-operations)
7. [Async Methods (Promises)](#async-methods-promises)
8. [Error Handling](#error-handling)
9. [TypeScript Support](#typescript-support)
10. [Examples](#examples)

---

## Overview

PDF Oxide provides a complete, idiomatic API for working with PDFs in Node.js and TypeScript. Unlike thin FFI wrappers, this library provides:

- **Full TypeScript Definitions**: Complete type safety with IntelliSense
- **Fluent Builders**: Chainable configuration APIs for complex operations
- **Domain Managers**: Specialized facades for document analysis, extraction, search, and security
- **Promise-based Async**: Modern async/await support for I/O operations
- **Proper Error Handling**: JavaScript Error subclasses instead of objects
- **Property API**: Modern property syntax (not Java-style getters)

### DX Score Comparison

| Feature | Node.js | Java | C# |
|---------|:-------:|:----:|:--:|
| TypeScript Definitions | ✅ | ✅ | ✅ |
| Fluent Builders | ✅ | ✅ | ✅ |
| Manager Pattern | ✅ | ✅ | ✅ |
| Async/Promises | ✅ | ✅ | ✅ |
| Error Handling | ✅ | ✅ | ✅ |
| DX Score | **9/10** | **10/10** | **9/10** |

---

## Installation

```bash
npm install pdf_oxide
```

### TypeScript Setup

No additional setup needed. TypeScript definitions are automatically included:

```typescript
import { PdfDocument, PdfBuilder } from 'pdf_oxide';

// Full type safety with IntelliSense
const doc = PdfDocument.open('document.pdf');
const pageCount: number = doc.pageCount;
```

---

## Quick Start

### Reading a PDF

```javascript
import { PdfDocument, MetadataManager, ExtractionManager } from 'pdf_oxide';

// Open document
const doc = PdfDocument.open('document.pdf');

// Access metadata
const meta = new MetadataManager(doc);
console.log(`Title: ${meta.getTitle()}`);
console.log(`Author: ${meta.getAuthor()}`);

// Extract text
const extraction = new ExtractionManager(doc);
const text = extraction.extractAllText();
console.log(text);

// Search content
const search = new SearchManager(doc);
const results = search.searchAll('keyword');
console.log(`Found ${results.length} matches`);

// Close when done
doc.close();
```

### Creating a PDF

```javascript
import { PdfBuilder, ConversionOptionsBuilder } from 'pdf_oxide';
import * as fs from 'fs';

// Create with fluent builder
const markdown = `
# My Document

This is a **PDF** document created with pdf_oxide.

## Section 2

- Point 1
- Point 2
- Point 3
`;

const pdf = PdfBuilder.create()
  .title('My PDF Document')
  .author('John Doe')
  .subject('Example PDF')
  .keywords(['pdf', 'example', 'nodejs'])
  .fromMarkdown(markdown);

pdf.save('output.pdf');
```

---

## Core API

### PdfDocument (Read-only)

```typescript
class PdfDocument {
  // Static methods
  static open(path: string): PdfDocument;
  static openWithPassword(path: string, password: string): PdfDocument;

  // Properties
  readonly version: [number, number];
  readonly pageCount: number;
  readonly hasStructureTree: boolean;

  // Methods
  extractText(pageIndex: number): string;
  extractTextAsync(pageIndex: number): Promise<string>;

  toMarkdown(pageIndex: number, options?: ConversionOptions): string;
  toMarkdownAsync(pageIndex: number, options?: ConversionOptions): Promise<string>;

  search(searchText: string, pageIndex: number, options?: SearchOptions): SearchResult[];

  getPage(pageIndex: number): PdfPage;

  close(): void;
}
```

### Pdf (Create & Modify)

```typescript
class Pdf {
  // Static methods
  static open(path: string): Pdf;
  static fromMarkdown(markdown: string): Pdf;
  static fromHtml(html: string): Pdf;
  static fromText(text: string): Pdf;

  // Properties
  readonly pageCount: number;
  documentInfo: DocumentInfo;

  // Methods
  save(path: string): void;
  saveAsync(path: string): Promise<void>;

  addPage(content: string): void;
  removePage(pageIndex: number): void;

  close(): void;
}
```

### PdfPage

```typescript
class PdfPage {
  // Properties
  readonly pageIndex: number;
  readonly pageNumber: number;
  readonly width: number;
  readonly height: number;
  readonly orientation: 'portrait' | 'landscape';
  readonly aspectRatio: number;

  // Methods
  extractText(): string;
  getAnnotations(): Annotation[];
}
```

---

## Builders (Fluent Configuration)

Builders provide **fluent, chainable APIs** for complex configuration with validation.

### PdfBuilder

Create PDFs with metadata and formatting:

```javascript
import { PdfBuilder } from 'pdf_oxide';

const pdf = PdfBuilder.create()
  .title('Annual Report 2024')
  .author('Example Author')
  .subject('Financial Summary')
  .keywords(['finance', 'annual', 'report'])
  .addKeyword('2024')
  .pageSize('A4')
  .margins(20, 20, 20, 20) // top, right, bottom, left
  .fromMarkdown(markdownContent);

pdf.save('annual_report.pdf');

// Async creation
const pdfAsync = await PdfBuilder.create()
  .title('Report')
  .fromMarkdownAsync(markdownContent);
```

### ConversionOptionsBuilder

Configure PDF format conversion:

```javascript
import { ConversionOptionsBuilder } from 'pdf_oxide';

// Use presets
const defaultOptions = ConversionOptionsBuilder.default().build();
const textOnly = ConversionOptionsBuilder.textOnly().build();
const highQuality = ConversionOptionsBuilder.highQuality().build();
const fast = ConversionOptionsBuilder.fast().build();

// Custom configuration
const customOptions = ConversionOptionsBuilder.create()
  .preserveFormatting(true)
  .detectHeadings(true)
  .detectTables(true)
  .detectLists(true)
  .includeImages(true)
  .imageFormat('webp')
  .imageQuality(90)
  .normalizeWhitespace(false)
  .build();

// Use with extraction
const text = doc.extractText(0);
const markdown = doc.toMarkdown(0, customOptions);
```

### MetadataBuilder

Configure document metadata:

```javascript
import { MetadataBuilder } from 'pdf_oxide';

const metadata = MetadataBuilder.create()
  .title('Project Proposal')
  .author('Project Manager')
  .subject('Q4 Planning')
  .keywords(['project', 'planning', 'q4'])
  .creator('PDF Oxide')
  .creationDate(new Date('2024-01-01'))
  .modificationDate(new Date())
  .withCurrentDate()
  .customProperty('Department', 'Engineering')
  .customProperty('Classification', 'Confidential')
  .customProperties({
    ProjectID: 'PRJ-2024-001',
    Version: '1.0',
  })
  .build();

// Use with PDF creation
const pdf = new Pdf();
// pdf.setMetadata(metadata);
```

### AnnotationBuilder

Create PDF annotations (comments, highlights, etc.):

```javascript
import { AnnotationBuilder } from 'pdf_oxide';

// Highlight annotation
const highlight = AnnotationBuilder.create()
  .asHighlight()
  .content('Important note')
  .author('Reviewer')
  .colorName('yellow')
  .opacity(0.8)
  .bounds({ x: 50, y: 100, width: 200, height: 20 })
  .creationDate(new Date())
  .build();

// Comment annotation
const comment = AnnotationBuilder.create()
  .asText()
  .content('Please revise this section')
  .author('Editor')
  .color([1, 0, 0]) // Red in RGB
  .build();

// Underline annotation
const underline = AnnotationBuilder.create()
  .asUnderline()
  .author('Reviewer')
  .colorName('green')
  .bounds({ x: 0, y: 200, width: 300, height: 15 })
  .build();

// Type-specific builders
const strikeout = AnnotationBuilder.create().asStrikeout().build();
const squiggly = AnnotationBuilder.create().asSquiggly().build();
```

### SearchOptionsBuilder

Configure text search:

```javascript
import { SearchOptionsBuilder } from 'pdf_oxide';

// Use presets
const defaultOpts = SearchOptionsBuilder.default().build();
const strictOpts = SearchOptionsBuilder.strict().build();
const regexOpts = SearchOptionsBuilder.regex().build();

// Custom search options
const customOpts = SearchOptionsBuilder.create()
  .caseSensitive(true)
  .wholeWords(true)
  .useRegex(false)
  .ignoreAccents(false)
  .maxResults(100)
  .searchAnnotations(true)
  .build();

// Use with search
const results = doc.search('pattern', 0, customOpts);
```

---

## Managers (Domain-specific Operations)

Managers provide **specialized facades** for domain-specific document operations.

### OutlineManager

Navigate PDF bookmarks/outline:

```javascript
import { OutlineManager } from 'pdf_oxide';

const outline = new OutlineManager(doc);

// Check and get
if (outline.hasOutlines()) {
  const count = outline.getOutlineCount();
  const items = outline.getOutlines();

  // Find outlines
  const item = outline.findByTitle('Introduction');
  const matches = outline.findAllByTitle('Chapter');

  // Check pages
  const pageOutlines = outline.getOutlinesForPage(0);
  if (outline.pageHasOutlines(5)) {
    console.log('Page 6 has outline items');
  }
}
```

### MetadataManager

Access document metadata:

```javascript
import { MetadataManager } from 'pdf_oxide';

const metadata = new MetadataManager(doc);

// Get individual fields
const title = metadata.getTitle();
const author = metadata.getAuthor();
const subject = metadata.getSubject();
const keywords = metadata.getKeywords();
const created = metadata.getCreationDate();
const modified = metadata.getModificationDate();

// Get all metadata at once
const allMeta = metadata.getAllMetadata();

// Query metadata
if (metadata.hasMetadata()) {
  const summary = metadata.getMetadataSummary();
  console.log(summary);

  if (metadata.hasKeyword('confidential')) {
    console.log('Document is marked confidential');
  }
}

// Validation
const validation = metadata.validate();
if (!validation.isComplete) {
  console.log('Missing fields:', validation.issues);
}

// Compare documents
const doc2 = PdfDocument.open('other.pdf');
const comparison = metadata.compareWith(doc2);
console.log('Matching fields:', comparison.matching);
console.log('Differing fields:', comparison.differing);
```

### ExtractionManager

Extract content from PDFs:

```javascript
import { ExtractionManager } from 'pdf_oxide';

const extraction = new ExtractionManager(doc);

// Extract text
const pageText = extraction.extractText(0);
const allText = extraction.extractAllText();
const rangeText = extraction.extractTextRange(0, 10);

// Extract as Markdown
const pageMarkdown = extraction.extractMarkdown(0);
const allMarkdown = extraction.extractAllMarkdown();
const rangeMarkdown = extraction.extractMarkdownRange(5, 15);

// Get statistics
const pageWords = extraction.getPageWordCount(0);
const totalWords = extraction.getTotalWordCount();
const pageChars = extraction.getPageCharacterCount(0);
const totalChars = extraction.getTotalCharacterCount();
const pageLines = extraction.getPageLineCount(0);

const stats = extraction.getContentStatistics();
console.log(`${stats.pageCount} pages`);
console.log(`${stats.wordCount} total words`);
console.log(`${stats.averageWordsPerPage} words per page`);

// Search within extracted content
const matches = extraction.searchContent('keyword', 50); // 50 chars context
matches.forEach(match => {
  console.log(`Page ${match.pageNumber}: ...${match.snippet}...`);
});
```

### SearchManager

Perform text search across document:

```javascript
import { SearchManager } from 'pdf_oxide';

const search = new SearchManager(doc);

// Search single page
const pageResults = search.search('keyword', 0);

// Search all pages
const allResults = search.searchAll('keyword');

// Count occurrences
const pageCount = search.countOccurrences('keyword', 0);
const totalCount = search.countAllOccurrences('keyword');

// Check containment
if (search.contains('error', 0)) {
  console.log('Page has "error"');
}

if (search.containsAnywhere('important')) {
  console.log('Document contains "important"');
}

// Get pages containing text
const pages = search.getPagesContaining('TODO');
console.log(`"TODO" found on pages: ${pages.map(p => p + 1).join(', ')}`);

// Search statistics
const stats = search.getSearchStatistics('error');
console.log(`${stats.totalOccurrences} occurrences`);
console.log(`On ${stats.pagesContaining} pages`);
console.log(`First match: page ${stats.firstMatchPage + 1}`);
console.log(`Last match: page ${stats.lastMatchPage + 1}`);

// Regex search
const regexResults = search.searchRegex(/error\d+/i);

// Find first/last
const first = search.findFirst('warning');
const last = search.findLast('warning');

if (first) {
  console.log(`First "warning" on page ${first.pageNumber}`);
}

// Highlight matches (for UI)
const highlights = search.highlightMatches('important');
```

### SecurityManager

Check document security and permissions:

```javascript
import { SecurityManager } from 'pdf_oxide';

const security = new SecurityManager(doc);

// Check encryption
if (security.isEncrypted()) {
  console.log(`Algorithm: ${security.getEncryptionAlgorithm()}`);
  if (security.requiresPassword()) {
    console.log('Password required to open');
  }
}

// Check permissions
console.log(`Can print: ${security.canPrint()}`);
console.log(`Can copy: ${security.canCopy()}`);
console.log(`Can modify: ${security.canModify()}`);
console.log(`Can annotate: ${security.canAnnotate()}`);
console.log(`Can fill forms: ${security.canFillForms()}`);

// Get permissions summary
const perms = security.getPermissionsSummary();
if (perms.isViewOnly) {
  console.log('Document is VIEW-ONLY');
}

// Security level
const level = security.getSecurityLevel();
console.log(`Security: ${level.level} (${level.description})`);

// Accessibility validation
const access = security.validateAccessibility();
if (!access.canExtractText) {
  console.warn('Text extraction is restricted');
}

// Generate report
console.log(security.generateSecurityReport());
```

### AnnotationManager

Work with page annotations:

```javascript
import { AnnotationManager } from 'pdf_oxide';

const page = doc.getPage(0);
const annotations = new AnnotationManager(page);

// Get annotations
const allAnnotations = annotations.getAnnotations();
const highlights = annotations.getHighlights();
const comments = annotations.getComments();
const underlines = annotations.getUnderlines();
const strikeouts = annotations.getStrikeouts();

// Filter annotations
const byType = annotations.getAnnotationsByType('highlight');
const byAuthor = annotations.getAnnotationsByAuthor('John');
const authors = annotations.getAnnotationAuthors();

// Search annotations
const withContent = annotations.getAnnotationsWithContent('review');
const recent = annotations.getRecentAnnotations(7); // Last 7 days

// Annotation statistics
const count = annotations.getAnnotationCount();
const stats = annotations.getAnnotationStatistics();
console.log(`Total: ${stats.total}`);
console.log(`By type:`, stats.byType);
console.log(`By author:`, stats.byAuthor);
console.log(`Average opacity: ${stats.averageOpacity}`);

// Generate summary
console.log(annotations.generateAnnotationSummary());

// Validate annotation
const validation = annotations.validateAnnotation(ann);
if (!validation.isValid) {
  console.log('Issues:', validation.issues);
}
```

---

## Async Methods (Promises)

Modern async/await support for I/O operations:

```javascript
import { PdfDocument, ExtractionManager } from 'pdf_oxide';

// Async extraction
async function extractDocument(path) {
  const doc = PdfDocument.open(path);

  try {
    // Extract text asynchronously
    const text = await doc.extractTextAsync(0);
    console.log(`Extracted: ${text.substring(0, 100)}...`);

    // Convert to markdown asynchronously
    const markdown = await doc.toMarkdownAsync(0);
    console.log(`Markdown length: ${markdown.length}`);

  } finally {
    doc.close();
  }
}

// Async PDF saving
async function createAndSave(path) {
  const pdf = PdfBuilder.create()
    .title('Async PDF')
    .fromMarkdown('# Hello World');

  // Save asynchronously
  await pdf.saveAsync(path);
  console.log(`Saved to ${path}`);

  pdf.close();
}

// Parallel operations
async function parallelOperations(path) {
  const doc = PdfDocument.open(path);

  try {
    const [text0, text1, markdown0] = await Promise.all([
      doc.extractTextAsync(0),
      doc.extractTextAsync(1),
      doc.toMarkdownAsync(0),
    ]);

    console.log(`Page 0 text: ${text0.length} chars`);
    console.log(`Page 1 text: ${text1.length} chars`);
    console.log(`Page 0 markdown: ${markdown0.length} chars`);

  } finally {
    doc.close();
  }
}

// Use in async context
await extractDocument('input.pdf');
await createAndSave('output.pdf');
await parallelOperations('input.pdf');
```

---

## Error Handling

PDF Oxide uses proper JavaScript Error subclasses:

```javascript
import {
  PdfError,
  PdfIoError,
  PdfParseError,
  PdfEncryptionError,
  PdfUnsupportedError,
  PdfInvalidStateError,
  PdfDecodeError,
  PdfEncodeError,
  PdfFontError,
  PdfImageError,
  PdfCircularReferenceError,
  PdfRecursionLimitError,
  PdfOcrError,
  PdfMlError,
  PdfBarcodeError,
} from 'pdf_oxide';

try {
  const doc = PdfDocument.open('nonexistent.pdf');
} catch (error) {
  if (error instanceof PdfIoError) {
    console.error('File not found or unreadable');
  } else if (error instanceof PdfParseError) {
    console.error('Invalid PDF format');
  } else if (error instanceof PdfEncryptionError) {
    console.error('PDF is encrypted or password incorrect');
  } else if (error instanceof PdfError) {
    console.error('PDF processing error:', error.message);
  } else {
    throw error; // Unknown error
  }
}

try {
  const builder = PdfBuilder.create();
  builder.pageSize('InvalidSize'); // Will throw
} catch (error) {
  if (error instanceof Error) {
    console.error('Builder error:', error.message);
  }
}
```

---

## TypeScript Support

Full TypeScript definitions for complete type safety:

```typescript
import {
  PdfDocument,
  PdfBuilder,
  ConversionOptionsBuilder,
  SearchOptionsBuilder,
  MetadataManager,
  ExtractionManager,
  SearchManager,
  SecurityManager,
  OutlineManager,
  AnnotationManager,
  SearchResult,
  ConversionOptions,
  SearchOptions,
  DocumentInfo,
  Annotation,
} from 'pdf_oxide';

// Strongly typed
function analyzeDocument(path: string): void {
  const doc: PdfDocument = PdfDocument.open(path);
  const pageCount: number = doc.pageCount;

  const extraction: ExtractionManager = new ExtractionManager(doc);
  const wordCount: number = extraction.getTotalWordCount();

  const search: SearchManager = new SearchManager(doc);
  const results: SearchResult[] = search.searchAll('keyword');

  const options: ConversionOptions = ConversionOptionsBuilder
    .highQuality()
    .build();

  const markdown: string = doc.toMarkdown(0, options);

  doc.close();
}

// TypeScript with async
async function createPdfAsync(content: string): Promise<void> {
  const pdf = PdfBuilder.create()
    .title('TypeScript PDF')
    .fromMarkdown(content);

  await pdf.saveAsync('output.pdf');
}
```

---

## Examples

### Complete Example: Document Analysis

```javascript
import {
  PdfDocument,
  MetadataManager,
  ExtractionManager,
  SearchManager,
  SecurityManager,
} from 'pdf_oxide';
import * as fs from 'fs';

function analyzeDocument(filePath) {
  const doc = PdfDocument.open(filePath);

  try {
    const metadata = new MetadataManager(doc);
    const extraction = new ExtractionManager(doc);
    const search = new SearchManager(doc);
    const security = new SecurityManager(doc);

    // Get basic info
    console.log('=== Document Analysis ===\n');
    console.log(`File: ${filePath}`);
    console.log(`Pages: ${doc.pageCount}`);
    console.log(`Version: ${doc.version.join('.')}\n`);

    // Metadata
    console.log('--- Metadata ---');
    console.log(metadata.getMetadataSummary());
    console.log();

    // Content
    console.log('--- Content Statistics ---');
    const stats = extraction.getContentStatistics();
    console.log(`Total words: ${stats.wordCount}`);
    console.log(`Total characters: ${stats.characterCount}`);
    console.log(`Average page length: ${stats.averageWordsPerPage} words`);
    console.log();

    // Search
    console.log('--- Keyword Search ---');
    const searchStats = search.getSearchStatistics('the');
    console.log(`"the" appears ${searchStats.totalOccurrences} times`);
    console.log(`On ${searchStats.pagesContaining} pages`);
    console.log();

    // Security
    console.log('--- Security ---');
    if (security.isEncrypted()) {
      console.log(`Encrypted: ${security.getEncryptionAlgorithm()}`);
    } else {
      console.log('Not encrypted');
    }
    console.log(`Can print: ${security.canPrint()}`);
    console.log(`Can copy: ${security.canCopy()}`);
    console.log(`Can modify: ${security.canModify()}\n`);

  } finally {
    doc.close();
  }
}

// Run analysis
analyzeDocument('document.pdf');
```

### Example: Content Extraction Pipeline

```javascript
import {
  PdfDocument,
  ExtractionManager,
  ConversionOptionsBuilder,
  SearchManager,
} from 'pdf_oxide';
import * as fs from 'fs';

async function extractAndProcess(pdfPath, outputDir) {
  const doc = PdfDocument.open(pdfPath);

  try {
    const extraction = new ExtractionManager(doc);
    const search = new SearchManager(doc);

    // Extract as multiple formats
    const plainText = extraction.extractAllText();
    const markdown = extraction.extractAllMarkdown();
    const highQualityMd = extraction.extractAllMarkdown(
      ConversionOptionsBuilder.highQuality().build()
    );

    // Extract per page
    for (let i = 0; i < doc.pageCount; i++) {
      const pageText = extraction.extractText(i);
      const pageFile = `${outputDir}/page_${i + 1}.txt`;
      fs.writeFileSync(pageFile, pageText);
    }

    // Find important pages
    const importantPages = search.getPagesContaining('important');
    console.log(`Important pages: ${importantPages.map(p => p + 1).join(', ')}`);

    // Save extracted content
    fs.writeFileSync(`${outputDir}/full.txt`, plainText);
    fs.writeFileSync(`${outputDir}/full.md`, markdown);
    fs.writeFileSync(`${outputDir}/full_hq.md`, highQualityMd);

    console.log('Extraction complete');

  } finally {
    doc.close();
  }
}

await extractAndProcess('large.pdf', './output');
```

---

## API Summary

### Quick Reference

| Category | Class | Purpose |
|----------|-------|---------|
| **Reading** | PdfDocument | Open and read PDF files |
| **Creating** | Pdf | Create new PDF documents |
| **Configuration** | PdfBuilder | Document creation with metadata |
| | ConversionOptionsBuilder | Format conversion options |
| | MetadataBuilder | Document metadata |
| | AnnotationBuilder | PDF annotations |
| | SearchOptionsBuilder | Text search options |
| **Analysis** | MetadataManager | Document metadata access |
| | ExtractionManager | Content extraction |
| | SearchManager | Text search |
| | SecurityManager | Encryption/permissions |
| | OutlineManager | Bookmarks navigation |
| | AnnotationManager | Page annotations |
| **Pages** | PdfPage | Individual page access |
| **Error** | PdfError* | Error hierarchy |

---

## Feature Comparison

### vs. Thin FFI Wrappers

```javascript
// Thin FFI (unsafe, verbose)
const text = doc.getPageText(0); // Java-style
try {
  const results = doc.search({ text: 'key', pageIndex: 0 }); // Config object
} catch (e) {
  if (e.kind === 'ParseError') { } // String comparison
}

// PDF Oxide (idiomatic, safe)
const text = doc.extractText(0); // Modern property
try {
  const results = doc.search('key', 0); // Direct parameters
} catch (e) {
  if (e instanceof PdfParseError) { } // Type checking
}
```

### vs. Java Bindings

- **PDF Oxide (Node.js)**: JavaScript-first, async/await, property syntax
- **Java**: Class-heavy, method-heavy, not async-first

### vs. C# Bindings

- **PDF Oxide (Node.js)**: Fluent builders, manager pattern, full TypeScript
- **C#**: Similar patterns, but C#-specific (IEnumerable, properties, etc.)

---

## Best Practices

1. **Always close documents**: Use try/finally or close() method
2. **Use managers for domain operations**: MetadataManager for metadata, etc.
3. **Use builders for configuration**: Prefer fluent builders over object literals
4. **Use TypeScript**: Get IntelliSense and type checking
5. **Use async methods for I/O**: Better performance in concurrent scenarios
6. **Use proper error handling**: instanceof checks with Error subclasses

---

## Contributing

To report bugs or request features, visit the [GitHub repository](https://github.com/yfedoseev/pdf_oxide).

---

**PDF Oxide Version**: 0.3.2+
**Last Updated**: 2024-01-17
**Maintainer**: yfedoseev
