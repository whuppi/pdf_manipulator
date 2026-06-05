// Extract text from every page of a PDF and print it.
// Run: node index.js document.pdf

import { PdfDocument } from "pdf-oxide";

const path = process.argv[2];
if (!path) {
  console.error("Usage: node index.js <file.pdf>");
  process.exit(1);
}

const doc = PdfDocument.open(path);
const pages = doc.pageCount();
console.log(`Opened: ${path}`);
console.log(`Pages: ${pages}\n`);

for (let i = 0; i < pages; i++) {
  const text = doc.extractText(i);
  console.log(`--- Page ${i + 1} ---`);
  console.log(`${text}\n`);
}

doc.close();
process.exit(0);
