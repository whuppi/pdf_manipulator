// StreamingTable with rowspan and batchSize
// Run: node index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder, Align } from "pdf-oxide";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const builder = DocumentBuilder.create().title("StreamingTable Demo");
const page = builder.letterPage().font("Helvetica", 10).at(72, 700).heading(1, "Product Catalogue").at(72, 660);

const tbl = page.streamingTable({
  columns: [
    { header: "Category", width: 120 },
    { header: "Item",     width: 160 },
    { header: "Notes",    width: 150, align: Align.Right },
  ],
  repeatHeader: true,
  maxRowspan: 2,
  batchSize: 2,
});

tbl.pushRowSpan([{ text: "Fruits",     rowspan: 2 }, { text: "Apple",  rowspan: 1 }, { text: "crisp",  rowspan: 1 }]);
tbl.pushRowSpan([{ text: "",           rowspan: 1 }, { text: "Banana", rowspan: 1 }, { text: "sweet",  rowspan: 1 }]);
tbl.pushRowSpan([{ text: "Vegetables", rowspan: 1 }, { text: "Carrot", rowspan: 1 }, { text: "earthy", rowspan: 1 }]);

console.log(`  batchCount=${tbl.batchCount}, pendingRowCount=${tbl.pendingRowCount}`);

const outPath = path.join(OUT_DIR, "streaming_table_rowspan.pdf");
(await tbl.finish()).done();
builder.save(outPath);
console.log(`Written: ${outPath}`);
process.exit(0);
