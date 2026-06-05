/**
 * PDF Oxide Node.js: Result Accessors Example
 *
 * Demonstrates how to use Result Accessors to extract extended properties
 * from search results, fonts, images, and annotations.
 *
 * Result Accessors provide detailed metadata that enriches the basic results
 * from other PDF operations, enabling advanced features like:
 * - Context extraction around search results
 * - Font metric analysis
 * - Image profile inspection
 * - Annotation metadata tracking
 */

import { PdfDocument, ResultAccessorsManager } from 'pdf_oxide';

async function main() {
  console.log('PDF Oxide - Result Accessors Examples\n');
  console.log('=====================================================\n');

  try {
    // Open PDF document
    const document = await PdfDocument.open('sample.pdf');

    // Initialize Result Accessors Manager
    const accessors = new ResultAccessorsManager(document);

    // Example 1: Search Result Enrichment
    await searchResultEnrichment(accessors);

    // Example 2: Font Metrics Extraction
    await fontMetricsExtraction(accessors);

    // Example 3: Image Metadata Inspection
    await imageMetadataInspection(accessors);

    // Example 4: Annotation Relationships
    await annotationRelationships(accessors);

    // Example 5: Advanced Search Filtering
    await advancedSearchFiltering(accessors);

    // Example 6: Batch Property Extraction
    await batchPropertyExtraction(accessors);

    // Example 7: Data Class Patterns
    await dataClassPatterns(accessors);

    document.close();
  } catch (error) {
    console.error('Error:', (error as Error).message);
    process.exit(1);
  }
}

/**
 * Example: Extract enriched properties from search results
 */
async function searchResultEnrichment(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Search Result Enrichment ===\n');

  console.log('Getting all search result properties at once:');
  console.log('  const results = await searchManager.search("important", 0);');
  console.log('  const props = await accessors.getSearchResultAllProperties(results, 0);');
  console.log('  console.log("Context:", props.context);');
  console.log('  console.log("Confidence:", props.confidence);');
  console.log('  console.log("Color (RGB):", props.color);\n');

  console.log('Individual property access:');
  console.log('  const context = await accessors.getSearchResultContext(results, 0, 50);');
  console.log('  const confidence = await accessors.getSearchResultConfidence(results, 0);');
  console.log('  const color = await accessors.getSearchResultColor(results, 0);\n');
}

/**
 * Example: Extract detailed font metrics
 */
async function fontMetricsExtraction(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Font Metrics Extraction ===\n');

  console.log('Getting all font properties at once:');
  console.log('  const fonts = await extractionMgr.getPageFonts(0);');
  console.log('  const fontProps = await accessors.getFontAllProperties(fonts, 0);');
  console.log('  console.log("Font:", fontProps.baseFontName);');
  console.log('  console.log("Ascender:", fontProps.ascender);');
  console.log('  console.log("Descender:", fontProps.descender);');
  console.log('  console.log("Vertical:", fontProps.isVertical);\n');

  console.log('Individual font property access:');
  console.log('  const ascender = await accessors.getFontAscender(fonts, 0);');
  console.log('  const descender = await accessors.getFontDescender(fonts, 0);');
  console.log('  const widths = await accessors.getFontWidths(fonts, 0);\n');
}

/**
 * Example: Inspect detailed image metadata
 */
async function imageMetadataInspection(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Image Metadata Inspection ===\n');

  console.log('Getting all image properties at once:');
  console.log('  const images = await extractionMgr.getPageImages(0);');
  console.log('  const imgProps = await accessors.getImageAllProperties(images, 0);');
  console.log('  console.log("Alpha Channel:", imgProps.hasAlphaChannel);');
  console.log('  console.log("ICC Profile Size:", imgProps.iccProfile.length);');
  console.log('  console.log("Filter Chain:", imgProps.filterChain);\n');

  console.log('Individual image property access:');
  console.log('  const hasAlpha = await accessors.hasImageAlphaChannel(images, 0);');
  console.log('  const iccProfile = await accessors.getImageIccProfile(images, 0);');
  console.log('  const decoded = await accessors.getImageDecodedData(images, 0);\n');
}

/**
 * Example: Track annotation relationships and metadata
 */
async function annotationRelationships(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Annotation Relationships ===\n');

  console.log('Getting all annotation properties:');
  console.log('  const annotations = await annotationMgr.getPageAnnotations(0);');
  console.log('  for (let i = 0; i < annotations.length; i++) {');
  console.log('    const props = await accessors.getAnnotationAllProperties(annotations, i);');
  console.log('    console.log("Subject:", props.subject);');
  console.log('    console.log("Icon:", props.iconName);');
  console.log('    console.log("Modified:", new Date(props.modifiedDate));');
  console.log('    if (props.replyToIndex >= 0) {');
  console.log('      console.log("Reply To:", props.replyToIndex);');
  console.log('    }');
  console.log('  }\n');

  console.log('Individual annotation property access:');
  console.log('  const subject = await accessors.getAnnotationSubject(annotations, 0);');
  console.log('  const modDate = await accessors.getAnnotationModifiedDate(annotations, 0);\n');
}

/**
 * Example: Filter and analyze search results by properties
 */
async function advancedSearchFiltering(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Advanced Search Filtering ===\n');

  console.log('Example: Find high-confidence OCR results');
  console.log('  for (let page = 0; page < doc.getPageCount(); page++) {');
  console.log('    const results = await searchMgr.search("data", page);');
  console.log('    for (let i = 0; i < results.length; i++) {');
  console.log('      const confidence = await accessors.getSearchResultConfidence(results, i);');
  console.log('      if (confidence >= 0.95) {');
  console.log('        const context = await accessors.getSearchResultContext(results, i, 50);');
  console.log('        console.log("High confidence:", context);');
  console.log('      }');
  console.log('    }');
  console.log('  }\n');

  console.log('Example: Extract and categorize results by color');
  console.log('  const byColor = new Map<string, any[]>();');
  console.log('  for (let i = 0; i < results.length; i++) {');
  console.log('    const color = await accessors.getSearchResultColor(results, i);');
  console.log('    const key = color.join(",");');
  console.log('    if (!byColor.has(key)) byColor.set(key, []);');
  console.log('    byColor.get(key)!.push(results[i]);');
  console.log('  }\n');
}

/**
 * Example: Batch property extraction
 */
async function batchPropertyExtraction(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Batch Property Extraction ===\n');

  console.log('The ResultAccessorsManager provides batch methods');
  console.log('for efficient bulk property extraction:\n');

  console.log('  // Get all search result properties at once');
  console.log('  const props = await accessors.getSearchResultAllProperties(results, 0);\n');

  console.log('  // Get all font properties at once');
  console.log('  const fontProps = await accessors.getFontAllProperties(fonts, 0);\n');

  console.log('  // Get all image properties at once');
  console.log('  const imgProps = await accessors.getImageAllProperties(images, 0);\n');

  console.log('  // Get all annotation properties at once');
  console.log('  const annoProps = await accessors.getAnnotationAllProperties(anno, 0);\n');
}

/**
 * Example: Data class usage patterns
 */
async function dataClassPatterns(accessors: ResultAccessorsManager): Promise<void> {
  console.log('=== Data Class Patterns ===\n');

  console.log('SearchResultProperties interface:');
  console.log('  {');
  console.log('    context: string;');
  console.log('    lineNumber: number;');
  console.log('    paragraphNumber: number;');
  console.log('    confidence: number;');
  console.log('    isHighlighted: boolean;');
  console.log('    fontInfo: string; // JSON');
  console.log('    color: [number, number, number]; // RGB');
  console.log('    rotation: number;');
  console.log('    objectId: number;');
  console.log('    streamIndex: number;');
  console.log('  }\n');

  console.log('FontProperties interface:');
  console.log('  {');
  console.log('    baseFontName: string;');
  console.log('    descriptor: string; // JSON');
  console.log('    descendantFont: string;');
  console.log('    toUnicodeCmap: string;');
  console.log('    isVertical: boolean;');
  console.log('    widths: Float32Array;');
  console.log('    ascender: number;');
  console.log('    descender: number;');
  console.log('  }\n');

  console.log('ImageProperties interface:');
  console.log('  {');
  console.log('    hasAlphaChannel: boolean;');
  console.log('    iccProfile: Uint8Array;');
  console.log('    filterChain: string; // JSON');
  console.log('    decodedData: Uint8Array;');
  console.log('  }\n');

  console.log('AnnotationProperties interface:');
  console.log('  {');
  console.log('    modifiedDate: number; // timestamp');
  console.log('    subject: string;');
  console.log('    replyToIndex: number;');
  console.log('    pageNumber: number;');
  console.log('    iconName: string;');
  console.log('  }\n');

  // Cache statistics example
  const stats = accessors.getCacheStats();
  console.log('Cache Statistics:');
  console.log(`  Size: ${stats.cacheSize} / ${stats.maxCacheSize}`);
  console.log(`  Entries: ${stats.entries.join(', ')}\n`);
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
