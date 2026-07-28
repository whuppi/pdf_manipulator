// Encrypted PDF output
// Run: node index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder } from "pdf-oxide";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const builder = DocumentBuilder.create().title("Encrypted PDF Demo");
builder.letterPage()
  .font("Helvetica", 12)
  .at(72, 720).heading(1, "Encrypted PDF")
  .at(72, 690).paragraph("This PDF is encrypted with a user password.")
  .done();

// toBytesEncrypted consumes the builder; can't call build() first since it
// also consumes the handle. The encrypted output includes the same content.
const outPath = path.join(OUT_DIR, "encrypted.pdf");
const encrypted = builder.toBytesEncrypted("user123", "owner123");
fs.writeFileSync(outPath, encrypted);

if (encrypted.length === 0) throw new Error("encrypted output is empty");
console.log(`  Encrypted PDF size: ${encrypted.length} bytes`);
console.log(`Written: ${outPath}`);
process.exit(0);
