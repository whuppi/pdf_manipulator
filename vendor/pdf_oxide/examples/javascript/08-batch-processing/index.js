// Process multiple PDFs concurrently using Promise.all.
// Run: node index.js file1.pdf file2.pdf ...

import { PdfDocument } from "pdf-oxide";

const paths = process.argv.slice(2);
if (paths.length === 0) {
  console.error("Usage: node index.js <file1.pdf> <file2.pdf> ...");
  process.exit(1);
}

function processPdf(filePath) {
  const doc = PdfDocument.open(filePath);
  const pages = doc.pageCount();
  let totalWords = 0;
  let totalTables = 0;
  for (let p = 0; p < pages; p++) {
    totalWords += (doc.extractWords(p) || []).length;
    totalTables += (doc.extractTables(p) || []).length;
  }
  doc.close();
  return { path: filePath, pages, words: totalWords, tables: totalTables };
}

async function main() {
  console.log(`Processing ${paths.length} PDFs concurrently...`);
  const start = Date.now();

  const results = await Promise.all(
    paths.map((p) =>
      Promise.resolve()
        .then(() => processPdf(p))
        .catch((err) => ({ path: p, error: err.message }))
    )
  );

  for (const r of results) {
    if (r.error) {
      console.log(`[${r.path}]\tERROR: ${r.error}`);
    } else {
      console.log(
        `[${r.path}]\tpages=${r.pages}\twords=${r.words}\ttables=${r.tables}`
      );
    }
  }

  const elapsed = ((Date.now() - start) / 1000).toFixed(2);
  console.log(`\nDone: ${paths.length} files processed in ${elapsed}s`);
}

main().then(() => process.exit(0));
