// Open a PDF, modify metadata, delete a page, and save.
// Run: node index.js input.pdf output.pdf

import { DocumentEditor } from "pdf-oxide";

const input = process.argv[2];
const output = process.argv[3];
if (!input || !output) {
  console.error("Usage: node index.js <input.pdf> <output.pdf>");
  process.exit(1);
}

const editor = DocumentEditor.open(input);
console.log(`Opened: ${input}`);

editor.setTitle("Edited Document");
console.log('Set title: "Edited Document"');

editor.setAuthor("pdf_oxide");
console.log('Set author: "pdf_oxide"');

editor.deletePage(1); // 0-indexed, deletes page 2
console.log("Deleted page 2");

editor.save(output);
editor.close();
console.log(`Saved: ${output}`);
