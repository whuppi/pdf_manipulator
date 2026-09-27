// PDF/A conversion: validate → convert → validate
// Run: node index.js

import { DocumentBuilder, PdfDocument } from "pdf-oxide";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const outDir = join(import.meta.dirname ?? ".", "output");
mkdirSync(outDir, { recursive: true });

const builder = DocumentBuilder.create().title("PDF/A Conversion Demo");
builder.letterPage()
  .font("Helvetica", 12)
  .at(72, 720).heading(1, "PDF/A-2b Conversion Demo")
  .at(72, 690).paragraph("This document will be converted to PDF/A-2b archival format.")
  .done();

const pdfBytes = builder.build();

// Step 1: validate before conversion
const doc = PdfDocument.openFromBuffer(pdfBytes);
console.log("Validating PDF/A-2b before conversion...");
try {
  const pre = doc.validatePdfA("2b");
  console.log(`  compliant: ${pre.compliant}, errors: ${pre.errors.length}`);
} catch (err) {
  console.log(`  validatePdfA skipped: ${err}`);
}

// Step 2: convert to PDF/A-2b in-place
console.log("Converting to PDF/A-2b...");
try {
  const ok = doc.convertToPdfA("2b");
  console.log(`  conversion success: ${ok}`);
} catch (err) {
  console.log(`  convertToPdfA skipped: ${err}`);
}

// Step 3: validate after conversion
console.log("Validating PDF/A-2b after conversion...");
try {
  const post = doc.validatePdfA("2b");
  console.log(`  compliant: ${post.compliant}, errors: ${post.errors.length}`);
  if (post.errors.length > 0) {
    post.errors.forEach(e => console.log(`    ! ${e}`));
  }
} catch (err) {
  console.log(`  validatePdfA skipped: ${err}`);
}

// Step 4: save result
const outBytes = doc.toBuffer();
const outPath = join(outDir, "pdfa_converted.pdf");
writeFileSync(outPath, outBytes);
console.log(`Written: ${outPath}`);

doc.close();
process.exit(0);
