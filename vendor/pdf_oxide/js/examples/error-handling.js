#!/usr/bin/env node

/**
 * Example: Error Handling
 *
 * Demonstrates:
 * - Handling different error types
 * - Error codes and messages
 * - Graceful error recovery
 * - Encrypted PDF handling
 */

const {
  PdfDocument,
  PdfIoError,
  PdfParseError,
  PdfEncryptionError,
  PdfUnsupportedError,
} = require('../index.js');

function handleFileNotFound() {
  console.log('=== Example 1: File Not Found ===\n');

  try {
    const doc = PdfDocument.open('/nonexistent/file.pdf');
  } catch (err) {
    if (err instanceof PdfIoError) {
      console.log('Error Type: I/O Error');
      console.log('Code:', err.code);
      console.log('Message:', err.message);
      console.log('Recovery: Check the file path and ensure file exists\n');
    } else {
      throw err;
    }
  }
}

function handleInvalidPdf() {
  console.log('=== Example 2: Invalid PDF Structure ===\n');

  try {
    // Create a file with invalid PDF content
    const fs = require('fs');
    const invalidPath = '/tmp/invalid.pdf';
    fs.writeFileSync(invalidPath, 'This is not a valid PDF file');

    const doc = PdfDocument.open(invalidPath);
  } catch (err) {
    if (err instanceof PdfParseError) {
      console.log('Error Type: Parse Error');
      console.log('Code:', err.code);
      console.log('Message:', err.message);
      if (err.offset !== undefined) {
        console.log('Offset:', err.offset);
      }
      console.log('Recovery: File may be corrupted. Try with a different PDF\n');
    } else if (err instanceof PdfIoError) {
      console.log('Error Type: I/O Error');
      console.log('Code:', err.code);
      console.log('Message:', err.message);
      console.log('Recovery: Check file permissions\n');
    } else {
      throw err;
    }
  }
}

function handleEncryptedPdf() {
  console.log('=== Example 3: Encrypted PDF ===\n');

  try {
    // This example demonstrates how to handle encrypted PDFs
    // You would need an actual encrypted PDF for this
    const doc = PdfDocument.open('/path/to/encrypted.pdf');
  } catch (err) {
    if (err instanceof PdfEncryptionError) {
      console.log('Error Type: Encryption Error');
      console.log('Code:', err.code);
      console.log('Message:', err.message);
      console.log('Recovery: Try opening with password using openWithPassword()\n');
    } else if (err instanceof PdfIoError) {
      console.log('File not found - this is a demonstration example');
      console.log('For real encrypted PDFs, use:');
      console.log('  const doc = PdfDocument.openWithPassword(path, password);\n');
    } else {
      throw err;
    }
  }
}

function handleUnsupportedFeature() {
  console.log('=== Example 4: Unsupported Feature ===\n');

  console.log('Error Type: Unsupported Feature');
  console.log('Code: UNSUPPORTED');
  console.log('Message: Unsupported feature: rendering');
  console.log('Recovery: Feature may require building with optional dependencies');
  console.log('  cargo build --release --features rendering\n');
}

function bestPractices() {
  console.log('=== Best Practices for Error Handling ===\n');

  const example = `
// 1. Check error type specifically
try {
  const doc = PdfDocument.open('file.pdf');
  const text = doc.extractText(0);
} catch (err) {
  if (err instanceof PdfIoError) {
    // Handle file I/O errors
    console.error('Cannot open file:', err.message);
  } else if (err instanceof PdfParseError) {
    // Handle parse errors
    console.error('Invalid PDF format:', err.message);
  } else if (err instanceof PdfEncryptionError) {
    // Handle encryption errors
    console.error('PDF is encrypted');
  } else {
    // Handle unknown errors
    throw err;
  }
}

// 2. Use cleanup code (finally or try-with-resources when available)
let doc;
try {
  doc = PdfDocument.open('file.pdf');
  const text = doc.extractText(0);
  console.log(text);
} catch (err) {
  console.error('Error:', err.message);
} finally {
  if (doc) {
    doc.close(); // Always cleanup
  }
}

// 3. Handle async errors
try {
  const doc = PdfDocument.open('file.pdf');
  const text = await doc.extractTextAsync(0);
  console.log(text);
} catch (err) {
  console.error('Async error:', err.message);
}

// 4. Validate input before calling API
function safeExtractText(path, pageIndex) {
  if (!path || typeof path !== 'string') {
    throw new Error('Invalid path');
  }
  if (!Number.isInteger(pageIndex) || pageIndex < 0) {
    throw new Error('Invalid page index');
  }

  try {
    const doc = PdfDocument.open(path);
    return doc.extractText(pageIndex);
  } catch (err) {
    // Handle known errors
    if (err instanceof PdfParseError) {
      throw new Error('PDF is invalid or corrupted');
    }
    throw err;
  }
}
  `;

  console.log(example);
}

// Run examples
async function main() {
  console.log('PDF Oxide Error Handling Examples\n');
  console.log('='.repeat(50));
  console.log();

  handleFileNotFound();
  handleInvalidPdf();
  handleEncryptedPdf();
  handleUnsupportedFeature();
  bestPractices();

  console.log('\nFor more information, see the error types documentation:');
  console.log('- PdfIoError: File I/O errors');
  console.log('- PdfParseError: PDF format errors');
  console.log('- PdfEncryptionError: Encryption-related errors');
  console.log('- PdfUnsupportedError: Unsupported features');
  console.log('- And many more specialized error types...');
}

main();
