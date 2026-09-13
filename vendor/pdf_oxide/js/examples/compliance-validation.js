// Compliance validation (PDF/A, PDF/X, PDF/UA) — v0.3.40
//
// Demonstrates validatePdfA, validatePdfX, and validatePdfUA on any PDF.
// Run: node compliance-validation.js [path/to/document.pdf]
//
// When no file is supplied the example runs in demo mode and prints the
// method signatures to confirm the API is wired up correctly.

import { PdfDocument } from '../index.js';

function printResult(label, result) {
  console.log(`\n${label}`);
  for (const [key, val] of Object.entries(result)) {
    if (Array.isArray(val)) {
      console.log(`  ${key}: [${val.length} item(s)]${val.length ? ' — ' + val.slice(0, 2).join(', ') + (val.length > 2 ? '...' : '') : ''}`);
    } else if (val && typeof val === 'object') {
      console.log(`  ${key}:`, val);
    } else {
      console.log(`  ${key}:`, val);
    }
  }
}

const filePath = process.argv[2];

if (!filePath) {
  console.log('Usage: node compliance-validation.js <path/to/document.pdf>');
  console.log('\nDemo mode — confirming API availability...');
  console.log('PdfDocument.prototype.validatePdfA:', typeof PdfDocument.prototype.validatePdfA);
  console.log('PdfDocument.prototype.validatePdfX:', typeof PdfDocument.prototype.validatePdfX);
  console.log('PdfDocument.prototype.validatePdfUA:', typeof PdfDocument.prototype.validatePdfUA);
  process.exit(0);
}

const doc = PdfDocument.open(filePath);

console.log(`Validating "${filePath}" (${doc.getPageCount()} page(s))\n`);

// PDF/A validation
for (const level of ['1b', '2b', '3b']) {
  const result = doc.validatePdfA(level);
  printResult(`PDF/A-${level}`, result);
}

// PDF/X validation
for (const level of ['1a_2001', '4']) {
  const result = doc.validatePdfX(level);
  printResult(`PDF/X-${level}`, result);
}

// PDF/UA validation
const uaResult = doc.validatePdfUA('ua1');
printResult('PDF/UA-1', uaResult);
