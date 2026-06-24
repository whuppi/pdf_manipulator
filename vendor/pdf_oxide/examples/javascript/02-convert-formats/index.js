// Convert PDF pages to Markdown, HTML, and plain text files.
// Run: node index.js document.pdf

import { PdfDocument } from "pdf-oxide";
import { mkdirSync, writeFileSync } from "node:fs";

const path = process.argv[2];
if (!path) {
  console.error("Usage: node index.js <file.pdf>");
  process.exit(1);
}

const doc = PdfDocument.open(path);
mkdirSync("output", { recursive: true });

const pages = doc.pageCount();
console.log(`Converting ${pages} pages from ${path}...`);

for (let i = 0; i < pages; i++) {
  const n = i + 1;
  const formats = [
    ["md", doc.toMarkdown(i)],
    ["html", doc.toHtml(i)],
    ["txt", doc.extractText(i)],
  ];
  for (const [ext, content] of formats) {
    const filename = `output/page_${n}.${ext}`;
    writeFileSync(filename, content);
    console.log(`Saved: ${filename}`);
  }
}

doc.close();
console.log("Done. Files written to output/");
process.exit(0);
