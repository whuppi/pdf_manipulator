// StreamingTable with rowspan (TypeScript)
// Compile: npx tsc -p ../../tsconfig.json
// Run: node ../../dist/09-new-features/streaming_table/index.js

import path from "node:path";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import { DocumentBuilder, Align, StreamingTableConfig, SpanCell } from "pdf-oxide";

const __dirname: string = path.dirname(fileURLToPath(import.meta.url));
const OUT_DIR: string = path.join(__dirname, "output");
fs.mkdirSync(OUT_DIR, { recursive: true });

const config: StreamingTableConfig = {
  columns: [
    { header: "Category", width: 120 },
    { header: "Item",     width: 160 },
    { header: "Notes",    width: 150, align: Align.Right },
  ],
  repeatHeader: true,
  maxRowspan: 2,
};

const builder = DocumentBuilder.create().title("StreamingTable Demo (TS)");
const page = builder.letterPage().font("Helvetica", 10).at(72, 700).heading(1, "Product Catalogue").at(72, 660);
const tbl = page.streamingTable(config);

const rows: SpanCell[][] = [
  [{ text: "Fruits",     rowspan: 2 }, { text: "Apple",  rowspan: 1 }, { text: "crisp",  rowspan: 1 }],
  [{ text: "",           rowspan: 1 }, { text: "Banana", rowspan: 1 }, { text: "sweet",  rowspan: 1 }],
  [{ text: "Vegetables", rowspan: 1 }, { text: "Carrot", rowspan: 1 }, { text: "earthy", rowspan: 1 }],
];
for (const row of rows) tbl.pushRowSpan(row);

const outPath: string = path.join(OUT_DIR, "streaming_table_rowspan_ts.pdf");
(await tbl.finish()).done();
builder.save(outPath);
console.log(`Written: ${outPath}`);
