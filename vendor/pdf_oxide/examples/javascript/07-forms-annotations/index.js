// Extract form fields and annotations from a PDF.
// Run: node index.js form.pdf

import { PdfDocument } from "pdf-oxide";

const path = process.argv[2];
if (!path) {
  console.error("Usage: node index.js <form.pdf>");
  process.exit(1);
}

const doc = PdfDocument.open(path);
console.log(`Opened: ${path}`);

const fields = doc.getFormFields() || [];
if (fields.length > 0) {
  console.log("\n--- Form Fields ---");
  for (const f of fields) {
    console.log(
      `  Name: ${JSON.stringify(f.name).padEnd(20)} ` +
        `Type: ${(f.type || "").padEnd(12)} ` +
        `Value: ${JSON.stringify(f.value).padEnd(16)} ` +
        `Required: ${f.required}`
    );
  }
}

const pages = doc.pageCount();
for (let page = 0; page < pages; page++) {
  const annotations = doc.getPageAnnotations(page) || [];
  if (annotations.length > 0) {
    console.log(`\n--- Annotations (page ${page + 1}) ---`);
    for (const a of annotations) {
      console.log(
        `  Type: ${(a.type || "").padEnd(14)} Page: ${page + 1}   ` +
          `Contents: "${a.content || a.contents || ""}"`
      );
    }
  }
}

doc.close();
process.exit(0);
