// Create PDFs using DocumentBuilder.
// Run: node index.js

import { DocumentBuilder } from "pdf-oxide";
import { mkdirSync } from "node:fs";

mkdirSync("output", { recursive: true });
console.log("Creating PDFs...");

const builder = DocumentBuilder.create().title("Project Report");
builder
  .a4Page()
  .font("Helvetica", 12)
  .at(72, 750)
  .heading(1, "Project Report")
  .at(72, 720)
  .paragraph("Generated from Markdown using pdf_oxide.")
  .done();
builder.save("output/from_markdown.pdf");
console.log("Saved: output/from_markdown.pdf");

const builder2 = DocumentBuilder.create().title("Invoice");
builder2
  .a4Page()
  .font("Helvetica", 12)
  .at(72, 750)
  .heading(1, "Invoice #1234")
  .at(72, 720)
  .paragraph("Generated from HTML using pdf_oxide.")
  .done();
builder2.save("output/from_html.pdf");
console.log("Saved: output/from_html.pdf");

const builder3 = DocumentBuilder.create();
builder3
  .a4Page()
  .font("Helvetica", 12)
  .at(72, 750)
  .paragraph(
    "Hello, World!\n\nThis PDF was created from plain text using pdf_oxide."
  )
  .done();
builder3.save("output/from_text.pdf");
console.log("Saved: output/from_text.pdf");

console.log("Done. 3 PDFs created in output/");
process.exit(0);
