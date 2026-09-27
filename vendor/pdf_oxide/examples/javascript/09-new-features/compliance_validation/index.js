// PDF/A, PDF/X, PDF/UA compliance validation
// Run: node index.js

import { DocumentBuilder, PdfDocument } from "pdf-oxide";

const builder = DocumentBuilder.create().title("Compliance Validation Demo");
builder.letterPage()
  .font("Helvetica", 12)
  .at(72, 720).heading(1, "Compliance Validation")
  .at(72, 690).paragraph("Testing PDF/A, PDF/X, and PDF/UA compliance validators.")
  .done();

const pdfBytes = builder.build();
const doc = PdfDocument.openFromBuffer(pdfBytes);

console.log("Validating PDF/A-2b compliance...");
try {
  const result = doc.validatePdfA("2b");
  console.log(`  compliant: ${result.compliant}`);
  console.log(`  errors:   ${JSON.stringify(result.errors)}`);
  console.log(`  warnings: ${JSON.stringify(result.warnings)}`);
} catch (err) {
  console.log(`  validatePdfA skipped or errored: ${err}`);
}

console.log("Validating PDF/X-4 compliance...");
try {
  const result = doc.validatePdfX("4");
  console.log(`  compliant: ${result.compliant}`);
  console.log(`  errors:   ${JSON.stringify(result.errors)}`);
  console.log(`  warnings: ${JSON.stringify(result.warnings)}`);
} catch (err) {
  console.log(`  validatePdfX skipped or errored: ${err}`);
}

console.log("Validating PDF/UA-1 compliance...");
try {
  const result = doc.validatePdfUA("ua1");
  console.log(`  accessible: ${result.accessible}`);
  console.log(`  errors:   ${JSON.stringify(result.errors)}`);
  console.log(`  warnings: ${JSON.stringify(result.warnings)}`);
} catch (err) {
  console.log(`  validatePdfUA skipped or errored: ${err}`);
}

process.exit(0);
