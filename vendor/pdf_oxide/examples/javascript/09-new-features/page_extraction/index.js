// Page extraction and chunking
//
// Demonstrates DocumentEditor.extractPagesToBytes() to split a multi-page PDF
// into per-chunk Buffers — all in memory, no temp files or S3.
// Addresses issue #384: replacing pypdf-style reader.pages slicing.
//
// Run: node index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder, DocumentEditor } from "pdf-oxide";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const CHUNK_SIZE = 2;

// Helper: split an array into chunks of size n.
function* batched(arr, n) {
  for (let i = 0; i < arr.length; i += n) {
    yield arr.slice(i, i + n);
  }
}

// Build a 5-page source document by merging single-page PDFs.
function buildPage(label) {
  const b = DocumentBuilder.create().title(label);
  b.letterPage().font("Helvetica", 12).at(72, 720).heading(1, label).done();
  return b.build();
}

const editor = DocumentEditor.openFromBytes(buildPage("Page 1"));
for (let i = 2; i <= 5; i++) {
  editor.mergeFromBytes(buildPage(`Page ${i}`));
}
const sourceBytes = editor.saveToBytes();
editor.close();

const total = (() => {
  const e = DocumentEditor.openFromBytes(sourceBytes);
  const n = e.pageCount();
  e.close();
  return n;
})();
console.log(`Source document: ${total} pages`);

// Split into chunks of CHUNK_SIZE — all in memory.
const pageIndices = Array.from({ length: total }, (_, i) => i);
const chunks = [];
let chunkNum = 0;
for (const chunkIndices of batched(pageIndices, CHUNK_SIZE)) {
  const src = DocumentEditor.openFromBytes(sourceBytes);
  const chunkBytes = src.extractPagesToBytes(chunkIndices);
  src.close();

  const check = DocumentEditor.openFromBytes(chunkBytes);
  const chunkPageCount = check.pageCount();
  check.close();

  console.log(
    `  Chunk ${chunkNum}: pages [${chunkIndices}] → ${chunkPageCount} pages, ${chunkBytes.length} bytes`
  );
  if (chunkPageCount !== chunkIndices.length) {
    throw new Error(`Expected ${chunkIndices.length} pages, got ${chunkPageCount}`);
  }
  chunks.push(chunkBytes);
  chunkNum++;
}
console.log(`Produced ${chunks.length} chunk(s)`);

// Write first chunk to disk as a demo output.
const outPath = path.join(OUT_DIR, "chunk_0.pdf");
fs.writeFileSync(outPath, chunks[0]);
console.log(`Written: ${outPath}`);
process.exit(0);
