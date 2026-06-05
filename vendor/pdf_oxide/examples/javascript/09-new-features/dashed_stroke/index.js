// Dashed stroke lines and rectangles
// Run: node index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder } from "pdf-oxide";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const builder = DocumentBuilder.create().title("Dashed Stroke Demo");
const page = builder.letterPage()
  .font("Helvetica", 12)
  .at(72, 720).heading(1, "Dashed Stroke Demo")
  .at(72, 680).text("Rectangles and lines drawn with configurable dash patterns.");

// Dashed rectangle — [5 on, 3 off] pattern, blue border
page.strokeRectDashed(72, 580, 300, 80, [5, 3], 0, { width: 2, color: [0, 0.2, 0.8] });

// Dashed line — [8 on, 4 off] pattern, red
page.strokeLineDashed(72, 550, 372, 550, [8, 4], 0, { width: 1.5, color: [0.8, 0, 0] });

// Fine dotted rectangle — [2 on, 2 off] with phase offset, green
page.strokeRectDashed(72, 460, 200, 60, [2, 2], 1, { width: 1, color: [0, 0.6, 0] });

page.done();
const outPath = path.join(OUT_DIR, "dashed_stroke.pdf");
builder.save(outPath);
console.log(`Written: ${outPath}`);
process.exit(0);
