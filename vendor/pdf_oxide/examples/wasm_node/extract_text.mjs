#!/usr/bin/env node
//
// WASM API demo: extract text from a PDF using pdf_oxide WASM bindings.
//
// Usage:
//   node extract_text.mjs                     # roundtrip demo (create + extract)
//   node extract_text.mjs <path-to-pdf>       # extract from existing PDF
//

import { readFileSync } from "fs";
import { resolve } from "path";
import { WasmPdfDocument, WasmPdf } from "./pdf_oxide.js";

console.log(`\n=== PDF Oxide WASM — Text Extraction Demo ===\n`);

// ---------- Part 1: Roundtrip (create PDF from text, then extract) ----------
console.log("--- Roundtrip: Create PDF from text, then extract ---\n");

const pdf = WasmPdf.fromText(
  "Hello from PDF Oxide WASM!\nThis PDF was created entirely in WebAssembly.",
  "WASM Demo",
  "pdf_oxide"
);
console.log(`Created PDF: ${pdf.size} bytes`);

const roundtripDoc = new WasmPdfDocument(pdf.toBytes());
const rtVersion = roundtripDoc.version();
console.log(`PDF version: ${rtVersion[0]}.${rtVersion[1]}`);
console.log(`Pages: ${roundtripDoc.pageCount()}`);
console.log(`Extracted text: "${roundtripDoc.extractText(0).trim()}"`);
console.log(`Markdown:\n${roundtripDoc.toMarkdown(0)}`);
roundtripDoc.free();

// ---------- Part 2: Extract from file (if provided) ----------
const pdfPath = process.argv[2];
if (pdfPath) {
  const absPath = resolve(pdfPath);
  console.log(`\n--- Extracting from file: ${absPath} ---\n`);

  const bytes = readFileSync(absPath);
  const data = new Uint8Array(bytes);
  const doc = new WasmPdfDocument(data);

  const pageCount = doc.pageCount();
  const version = doc.version();
  console.log(`PDF version: ${version[0]}.${version[1]}`);
  console.log(`Pages: ${pageCount}`);
  console.log(`Has structure tree: ${doc.hasStructureTree()}`);

  console.log(`\n--- Extracted Text ---\n`);
  for (let i = 0; i < pageCount; i++) {
    const text = doc.extractText(i);
    console.log(`[Page ${i + 1}]`);
    console.log(text);
    console.log();
  }

  if (pageCount > 0) {
    console.log(`--- Markdown (page 1) ---\n`);
    console.log(doc.toMarkdown(0));
    console.log();

    console.log(`--- HTML (page 1) ---\n`);
    console.log(doc.toHtml(0));
  }

  doc.free();
}

console.log(`\n=== Done ===`);
