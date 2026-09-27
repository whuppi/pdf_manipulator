# PDF Oxide API Guide

Complete API reference for all Phase 1-4 features.

## Table of Contents
1. [Read Interface (PdfDocument)](#read-interface)
2. [Create/Edit Interface (Pdf, PdfBuilder, PdfPage)](#createedit-interface)
3. [Annotations (27 types)](#annotations)
4. [Search](#search)
5. [Forms (AcroForm, XFA)](#forms)
6. [Metadata (XMP, PageLabels, EmbeddedFiles)](#metadata)
7. [Error Handling](#error-handling)

---

## Read Interface

### PdfDocument

Read-only access to PDF documents with automatic reading order detection.

#### Static Methods

```typescript
// Open from file
static open(path: string): PdfDocument

// Open with password
static open_with_password(path: string, password: string): PdfDocument

// Open from bytes
static open_from_bytes(data: Buffer): PdfDocument
```

#### Properties

```typescript
// Get PDF version
get_version(): [major: number, minor: number]

// Get page count
get_page_count(): number

// Check for structure tree (Tagged PDF)
has_structure_tree(): boolean
```

#### Text Extraction

```typescript
// Extract text from single page
extract_text(page_index: number): string

// Async text extraction
extract_text_async(page_index: number): Promise<string>
```

#### Format Conversion

```typescript
// Convert page to Markdown
to_markdown(page_index: number, options?: ConversionOptions): string

// Async conversion
to_markdown_async(page_index: number, options?: ConversionOptions): Promise<string>

// Convert all pages
to_markdown_all(options?: ConversionOptions): string

// Convert to HTML
to_html(page_index: number, options?: ConversionOptions): string

// Convert all pages to HTML
to_html_all(options?: ConversionOptions): string
```

#### Metadata & Features (Phase 4)

```typescript
// Get document information
get_document_info(): DocumentInfo

// Get XMP metadata
get_metadata(): XMPMetadata

// Get forms
get_forms(): AcroForm | null

// Get page labels
get_page_labels(): PageLabel[]

// Get embedded files
get_embedded_files(): EmbeddedFile[]
```

#### Resource Management

```typescript
close(): void
```

---

## Create/Edit Interface

### Pdf

Unified interface for creating and editing PDFs.

#### Creation Methods

```typescript
// Create from Markdown
static from_markdown(markdown: string): Pdf

// Create from HTML
static from_html(html: string): Pdf

// Create from text
static from_text(text: string): Pdf

// Open existing PDF
static open(path: string): Pdf
```

#### Properties

```typescript
get_version(): [major: number, minor: number]
get_page_count(): number
```

#### Document Operations

```typescript
// Get page for DOM access
page(index: number): PdfPage

// Save modified page
save_page(page: PdfPage): void

// Save to file
save(path: string): void

// Async save
save_async(path: string): Promise<void>
```

#### Metadata Management (Phase 4)

```typescript
// Set individual metadata fields
set_metadata_title(title: string): void
set_metadata_author(author: string): void
set_metadata_subject(subject: string): void

// Get current metadata
get_metadata(): XMPMetadata

// Set all metadata at once
set_metadata(metadata: XMPMetadata): void
```

#### Forms Management (Phase 4)

```typescript
// Get forms from document
get_forms(): AcroForm | null

// Set forms
set_forms(form: AcroForm): void
```

#### Page Labels (Phase 4)

```typescript
// Get all page labels
get_page_labels(): PageLabel[]

// Set label for specific page
set_page_label(page_index: number, label: PageLabel): void
```

#### Embedded Files (Phase 4)

```typescript
// Get all embedded files
get_embedded_files(): EmbeddedFile[]

// Add embedded file
add_embedded_file(file: EmbeddedFile): void

// Extract embedded file
extract_embedded_file(file_id: string): string | null
```

#### Resource Management

```typescript
close(): void
```

### PdfBuilder

Fluent API for advanced PDF configuration.

```typescript
// Create builder
static create(): PdfBuilder

// Configuration methods (chainable)
title(title: string): PdfBuilder
author(author: string): PdfBuilder
subject(subject: string): PdfBuilder
page_size(size: PageSize): PdfBuilder
margins(top: number, right: number, bottom: number, left: number): PdfBuilder

// Terminal methods
from_markdown(markdown: string): Pdf
from_html(html: string): Pdf
from_text(text: string): Pdf
```

#### Usage Example

```javascript
const doc = PdfBuilder.create()
  .title('My Report')
  .author('John Doe')
  .subject('Q4 Results')
  .page_size('A4')
  .margins(72, 72, 72, 72)
  .from_markdown('# Content');

doc.save('report.pdf');
```

### PdfPage

DOM-like page access and manipulation.

#### Properties

```typescript
get_page_index(): number
get_width(): number
get_height(): number
```

#### Element Access

```typescript
// Get all child elements
children(): string[] // Returns element IDs

// Find text elements
find_text_containing(query: string): string[]

// Search with options
find_text(query: string, options?: SearchOptions): SearchResult[]
```

#### Element Manipulation

```typescript
// Set text content
set_text(element_id: string, new_text: string): void

// Add new element
add_element(element: any): string // Returns element ID

// Remove element
remove_element(element_id: string): void
```

#### Annotations (Phase 4)

```typescript
// Get all annotations
annotations(): Annotation[]

// Add annotation
add_annotation(annotation: AnnotationContent): string // Returns annotation ID
```

---

## Annotations

### AnnotationType Enum (Phase 4)

All 27 PDF annotation types:

```typescript
type AnnotationType =
  | 'Text'                  // Sticky notes
  | 'Link'                  // Hyperlinks
  | 'FreeText'              // Text boxes
  | 'Line'                  // Lines
  | 'Square'                // Rectangles
  | 'Circle'                // Circles/ellipses
  | 'Polygon'               // Closed polygons
  | 'PolyLine'              // Open polylines
  | 'Highlight'             // Text highlights
  | 'Underline'             // Text underline
  | 'Squiggly'              // Wavy underline
  | 'StrikeOut'             // Strikethrough
  | 'Stamp'                 // Rubber stamps
  | 'Caret'                 // Text insertion
  | 'Ink'                   // Freehand drawing
  | 'Popup'                 // Popup window
  | 'FileAttachment'        // File attachment
  | 'Sound'                 // Audio playback
  | 'Movie'                 // Video (legacy)
  | 'Screen'                // Multimedia
  | 'Widget'                // Form field
  | 'PrinterMark'           // Printer mark
  | 'TrapNet'               // Trap network
  | 'Watermark'             // Watermark
  | 'Redact'                // Redaction
  | 'ThreeD'                // 3D model
  | 'RichMedia';            // Rich media
```

### Concrete Annotation Types

#### TextAnnotation
```typescript
interface TextAnnotation {
  id: string;
  rect: Rect;
  contents?: string;
  author?: string;
  subject?: string;
  icon_name?: string;         // Comment, Key, Note, Help, etc.
  color_r: number;            // 0-255
  color_g: number;            // 0-255
  color_b: number;            // 0-255
  open?: boolean;             // Initially open
}
```

#### LinkAnnotation
```typescript
interface LinkAnnotation {
  id: string;
  rect: Rect;
  uri?: string;               // External link
  destination_page?: number;  // Internal page
  target_blank?: boolean;     // Open in new window
}
```

#### HighlightAnnotation
```typescript
interface HighlightAnnotation {
  id: string;
  rect: Rect;
  color_r: number;
  color_g: number;
  color_b: number;
  quad_points?: string;       // JSON array of text areas
}
```

#### StampAnnotation
```typescript
interface StampAnnotation {
  id: string;
  rect: Rect;
  name: string;               // Approved, Draft, Confidential, Final, etc.
  color_r: number;
  color_g: number;
  color_b: number;
}
```

#### FreeTextAnnotation
```typescript
interface FreeTextAnnotation {
  id: string;
  rect: Rect;
  contents: string;
  font_name?: string;         // Helvetica, Times, Courier
  font_size: number;
  color_r: number;
  color_g: number;
  color_b: number;
  background_color_r?: number;
  background_color_g?: number;
  background_color_b?: number;
  border_style?: string;      // solid, dashed, beveled, etc.
}
```

#### InkAnnotation
```typescript
interface InkAnnotation {
  id: string;
  rect: Rect;
  stroke_color_r: number;
  stroke_color_g: number;
  stroke_color_b: number;
  stroke_width: number;
  ink_list?: string;          // JSON array of paths
}
```

#### FileAttachmentAnnotation
```typescript
interface FileAttachmentAnnotation {
  id: string;
  rect: Rect;
  filename: string;
  description?: string;
  icon_name?: string;         // Graph, Paperclip, PushPin, Tag
}
```

#### SignatureField
```typescript
interface SignatureField {
  id: string;
  rect: Rect;
  field_name: string;
  is_signed: boolean;
  signature_type?: string;    // Approval, Certification
  signer_name?: string;
  signature_date?: string;    // ISO 8601
  reason?: string;
  location?: string;
  contact_info?: string;
}
```

And 19 more types...

---

## Search

### TextSearcher (Phase 4)

Fluent API for full-text search.

#### Creation

```typescript
// Create searcher with pattern
const searcher = new TextSearcher(pattern: string);
```

#### Configuration (chainable)

```typescript
searcher
  .case_sensitive()    // Enable case sensitivity
  .whole_words()       // Enable whole-word matching
  .use_regex()         // Enable regex support
  .max_results(100)    // Limit results
```

#### Query Methods

```typescript
searcher.get_pattern(): string
searcher.is_case_sensitive(): boolean
searcher.is_whole_words(): boolean
searcher.is_regex(): boolean
```

#### Search

```typescript
searcher.search(text: string, options?: SearchOptions): SearchResult[]
```

### SearchResult

```typescript
interface SearchResult {
  text: string;            // Matched text
  page_index: number;      // 0-based page number
  bbox: Rect;              // Bounding box
  start_index: number;     // Start position in text
  end_index: number;       // End position in text
  confidence?: number;     // 0.0-1.0
}
```

#### Usage Example

```javascript
const searcher = new TextSearcher('important')
  .case_sensitive()
  .max_results(50);

const results = searcher.search(pageText);
for (const result of results) {
  console.log(`Match: "${result.text}" at ${result.start_index}`);
}
```

---

## Forms

### AcroForm (Phase 4)

Traditional PDF form handling (Section 12.7).

#### Creation

```typescript
const form = AcroForm.new(name?: string);
```

#### Field Management

```typescript
// Add field
form.add_field(field: FormField): void

// Get field by name
form.get_field(field_name: string): FormField | null

// Get all field names
form.get_field_names(): string[]

// Set field value
form.set_field_value(field_name: string, value: string): boolean

// Get field count
form.field_count(): number

// Check for signature fields
form.has_signature_fields(): boolean

// Get required fields
form.get_required_fields(): string[]
```

### FormFieldType (Phase 4)

```typescript
type FormFieldType =
  | 'Text'          // Single/multi-line text
  | 'Paragraph'     // Multi-line text
  | 'Checkbox'      // Boolean checkbox
  | 'Radio'         // Radio button
  | 'List'          // Dropdown list
  | 'Combo'         // Editable dropdown
  | 'Button'        // Push button
  | 'Signature';    // Digital signature
```

### FormField (Phase 4)

```typescript
interface FormField {
  id: string;
  field_name: string;
  field_type: FormFieldType;
  label?: string;
  field_value?: string;
  default_value?: string;
  rect: Rect;
  page_index: number;
  read_only: boolean;
  required: boolean;
  hidden: boolean;
  export_value?: string;
}
```

### Specialized Field Types

#### TextFormField
```typescript
interface TextFormField {
  id: string;
  field_name: string;
  field_value?: string;
  rect: Rect;
  font_name?: string;        // Helvetica, Times, Courier
  font_size: number;
  max_length?: number;
  multiline: boolean;
  color_r: number;
  color_g: number;
  color_b: number;
  text_alignment?: string;   // left, center, right
}
```

#### CheckboxField
```typescript
interface CheckboxField {
  id: string;
  field_name: string;
  rect: Rect;
  is_checked: boolean;
  checked_value?: string;
  style?: string;            // square, circle, diamond
}
```

#### RadioButtonField
```typescript
interface RadioButtonField {
  id: string;
  field_name: string;
  rect: Rect;
  options: string[];
  selected_option?: string;
  export_values?: string[];
}
```

#### ListField
```typescript
interface ListField {
  id: string;
  field_name: string;
  rect: Rect;
  options: string[];
  display_values?: string[];
  selected_options: string[];
  multi_select: boolean;
  is_combo: boolean;         // Editable dropdown
}
```

#### ButtonField
```typescript
interface ButtonField {
  id: string;
  field_name: string;
  rect: Rect;
  label: string;
  action?: string;           // Submit, Reset, JavaScript, URI
  action_target?: string;    // URL or script
  appearance?: string;       // normal, pressed, rollover
}
```

### XFAForm (Phase 4)

XML Forms Architecture for advanced forms.

```typescript
class XFAForm {
  static new(template_xml: string): XFAForm;

  set_data(data_xml: string): void;
  get_template(): string;
  get_data(): string | null;
}
```

---

## Metadata

### XMPMetadata (Phase 4)

Extensible Metadata Platform with 18 standard fields (Section 14.3).

#### Creation

```typescript
const metadata = XMPMetadata.new();
```

#### Setters

```typescript
metadata.set_title(title: string): void;
metadata.set_author(author: string): void;
metadata.set_subject(subject: string): void;
metadata.set_keywords(keywords: string): void;
metadata.set_creator(creator: string): void;
metadata.set_copyright(copyright: string): void;
metadata.set_language(language: string): void;
```

#### Getters

```typescript
metadata.get_title(): string | null;
metadata.get_author(): string | null;
metadata.get_subject(): string | null;
metadata.get_keywords(): string | null;
metadata.get_creator(): string | null;
metadata.get_copyright(): string | null;
metadata.get_language(): string | null;
```

#### Additional Fields

```typescript
interface XMPMetadata {
  title?: string;
  author?: string;
  subject?: string;
  keywords?: string;
  creator?: string;
  created?: string;             // ISO 8601
  modified?: string;            // ISO 8601
  copyright?: string;
  producer?: string;
  language?: string;
  description?: string;
  rights?: string;
  contributors?: string[];
  format?: string;
  identifier?: string;
  source?: string;
  relation?: string;
  coverage?: string;
  raw_xml?: string;             // Custom metadata
}
```

#### Utility Methods

```typescript
// Convert to key-value pairs
metadata.to_map(): Array<[string, string]>;

// Check if empty
metadata.is_empty(): boolean;
```

### PageLabel (Phase 4)

Intelligent page numbering (Section 12.4.2).

#### Creation

```typescript
const label = PageLabel.new(page_index: number);
```

#### Configuration

```typescript
label.set_style(style: string): void;    // decimal, roman, letters, uppercase
label.set_prefix(prefix: string): void;  // "Chapter-", "Appendix-"
label.set_start_value(value: number): void;
```

#### Label Generation

```typescript
label.get_label_text(): string;          // "Chapter-5", "Introduction-iii"
```

#### Supported Styles

```typescript
'decimal'           // 1, 2, 3
'roman'             // i, ii, iii
'uppercase_roman'   // I, II, III
'letters'           // a, b, c, aa, ab
'uppercase'         // A, B, C, AA, AB
```

#### Usage Example

```javascript
// Front matter with Roman numerals
const intro = PageLabel.new(0);
intro.set_prefix('Intro-');
intro.set_style('roman');
console.log(intro.get_label_text());  // "Intro-i"

// Main content with chapters
const chapter = PageLabel.new(5);
chapter.set_prefix('Chapter-');
chapter.set_style('decimal');
console.log(chapter.get_label_text());  // "Chapter-1"

// Appendix with letters
const appendix = PageLabel.new(20);
appendix.set_prefix('Appendix-');
appendix.set_style('uppercase');
console.log(appendix.get_label_text());  // "Appendix-A"
```

### EmbeddedFile (Phase 4)

Manage attached files.

#### Creation

```typescript
const file = EmbeddedFile.new(
  id: string,
  filename: string,
  mime_type: string,
  size: number
);
```

#### File Metadata

```typescript
file.set_description(description: string): void;
file.get_description(): string | null;

file.set_creation_date(date: string): void;  // ISO 8601
file.get_creation_date(): string | null;

file.set_modification_date(date: string): void;
file.get_modification_date(): string | null;

file.set_access_date(date: string): void;
file.get_access_date(): string | null;
```

#### File Data

```typescript
file.set_data(data: string): void;         // Base64 encoded
file.get_data(): string | null;
file.has_data(): boolean;
```

#### Properties

```typescript
interface EmbeddedFile {
  id: string;
  filename: string;
  description?: string;
  mime_type: string;
  size: number;
  creation_date?: string;
  modification_date?: string;
  access_date?: string;
  data?: string;
}
```

### DocumentInfo (Phase 4)

Basic document information.

#### Creation

```typescript
const info = DocumentInfo.new(version: string);
```

#### Methods

```typescript
info.set_title(title: string): void;
info.to_summary(): string;  // JSON summary
```

#### Properties

```typescript
interface DocumentInfo {
  version: string;
  title?: string;
  author?: string;
  subject?: string;
  keywords?: string;
  creator?: string;
  producer?: string;
  created?: string;
  modified?: string;
  is_encrypted: boolean;
  encryption_algorithm?: string;
}
```

---

## Error Handling

### Error Hierarchy

```typescript
// Base error
class PdfError extends Error {
  code: string;
}

// I/O errors
class PdfIoError extends PdfError
  // InvalidHeader, Io

// Parse errors
class PdfParseError extends PdfError
  // ParseError, InvalidXref, ObjectNotFound, etc.

// Encryption
class PdfEncryptionError extends PdfError
  // Encryption-related

// Unsupported features
class PdfUnsupportedError extends PdfError
  // UnsupportedVersion, Unsupported

// And more...
class PdfInvalidStateError extends PdfError
class PdfDecodeError extends PdfError
class PdfEncodeError extends PdfError
class PdfFontError extends PdfError
class PdfImageError extends PdfError
class PdfCircularReferenceError extends PdfError
class PdfRecursionLimitError extends PdfError
```

### Usage Example

```javascript
import {
  PdfDocument,
  PdfIoError,
  PdfParseError,
  PdfEncryptionError,
} from 'pdf_oxide';

try {
  const doc = PdfDocument.open('document.pdf');
} catch (err) {
  if (err instanceof PdfEncryptionError) {
    console.error('PDF is password-protected');
  } else if (err instanceof PdfIoError) {
    console.error('File not found or cannot be read');
  } else if (err instanceof PdfParseError) {
    console.error('Invalid PDF format');
  } else {
    throw err;
  }
}
```

---

## Type Definitions

### Common Types

```typescript
interface Rect {
  x: number;       // Left position
  y: number;       // Top position
  width: number;   // Width
  height: number;  // Height
}

interface Point {
  x: number;
  y: number;
}

interface Color {
  r: number;       // 0-255
  g: number;       // 0-255
  b: number;       // 0-255
  a?: number;      // 0-255 (alpha)
}

interface ConversionOptions {
  detectHeadings?: boolean;
  preserveLayout?: boolean;
  includeImages?: boolean;
  imageOutputDir?: string;
  embedImages?: boolean;
}

interface SearchOptions {
  caseSensitive?: boolean;
  wholeWords?: boolean;
  regex?: boolean;
}

type PageSize = 'A0' | 'A1' | 'A2' | 'A3' | 'A4' | 'A5' |
                'Letter' | 'Legal' | 'Tabloid' | 'Ledger';
```

---

## Summary

This API guide covers all **52 types and classes** across Phases 1-4:
- ✅ 2 read/create classes
- ✅ 2 builder classes
- ✅ 1 page class
- ✅ 27 annotation types
- ✅ 1 search class
- ✅ 8 form field types + 2 form classes
- ✅ 4 metadata types
- ✅ 21 error types

For more details, see:
- [README.md](./README.md) - Quick start and examples
- [PHASE4_COMPLETE.md](./PHASE4_COMPLETE.md) - Phase 4 implementation details
- Examples in [examples/](./examples/) directory
