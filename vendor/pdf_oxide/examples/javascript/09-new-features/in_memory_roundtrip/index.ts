// In-memory round-trip (TypeScript)

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder, PdfDocument } from "pdf-oxide";

const __dirname: string = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR: string = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const builder = DocumentBuilder.create().title("In-Memory Round-Trip (TS)");
builder.letterPage()
  .font("Helvetica", 12)
  .at(72, 720).heading(1, "In-Memory Round-Trip")
  .at(72, 690).paragraph("Built in memory, never written to disk mid-way.")
  .done();

const pdfBytes: Buffer = builder.build();
const doc = PdfDocument.openFromBuffer(pdfBytes);
let text = "";
for (let i = 0; i < doc.pageCount(); i++) text += doc.extractText(i);
console.log(`  Extracted ${text.length} chars`);
if (!text.includes("In-Memory")) throw new Error("round-trip text missing");

const outPath: string = path.join(OUT_DIR, "in_memory_roundtrip_ts.pdf");
fs.writeFileSync(outPath, pdfBytes);
console.log(`Written: ${outPath}`);
