// Extract words with bounding boxes and tables from a PDF page.
// Run: node index.js document.pdf

import { PdfDocument } from "pdf-oxide";

const path = process.argv[2];
if (!path) {
  console.error("Usage: node index.js <file.pdf>");
  process.exit(1);
}

const doc = PdfDocument.open(path);
console.log(`Opened: ${path}`);

const page = 0;

const words = doc.extractWords(page);
console.log(`\n--- Words (page ${page + 1}) ---`);
words.slice(0, 20).forEach((w) => {
  console.log(
    `"${w.text}"`.padEnd(20) +
      ` x=${w.x.toFixed(1).padEnd(7)} y=${w.y.toFixed(1).padEnd(7)}` +
      ` w=${w.width.toFixed(1).padEnd(7)} h=${w.height.toFixed(1).padEnd(7)}` +
      ` font=${w.fontName}  size=${w.fontSize.toFixed(1)}`
  );
});
if (words.length > 20) {
  console.log(`... (${words.length - 20} more words)`);
}

const tables = doc.extractTables(page);
console.log(`\n--- Tables (page ${page + 1}) ---`);
if (tables.length === 0) {
  console.log("(no tables found)");
}
tables.forEach((t, i) => {
  console.log(`Table ${i + 1}: ${t.rows} rows x ${t.cols} cols`);
  t.cells.slice(0, 5).forEach((row, r) => {
    const cells = row
      .slice(0, 6)
      .map((v, c) => `[${r},${c}] "${v}"`)
      .join("  ");
    console.log(`  ${cells}`);
  });
});

doc.close();
process.exit(0);
