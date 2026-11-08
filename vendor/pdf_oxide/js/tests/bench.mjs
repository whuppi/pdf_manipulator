// Cross-language benchmark — JavaScript / TypeScript binding.
//
// Emits NDJSON matching the Rust baseline (bench/rust_bench.rs) so results
// can be aggregated and compared.
//
// Run with:
//   LD_LIBRARY_PATH=../target/release node tests/bench.mjs ../bench_fixtures/tiny.pdf ...

import { Buffer } from 'node:buffer';
import { statSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename } from 'node:path';
import { hrtime } from 'node:process';

const require = createRequire(import.meta.url);
const native = require('../build/Release/pdf_oxide.node');

const ITERATIONS = 5;

function ns(fn) {
  const start = hrtime.bigint();
  fn();
  return Number(hrtime.bigint() - start);
}

function benchFixture(path) {
  const size = statSync(path).size;

  // Warm-up — exercises every code path so JIT / lazy init is amortized.
  {
    const doc = native.openDocument(path);
    native.extractText(doc, 0);
    const searchHandle = native.searchAll(doc, 'the', false);
    native.searchResultFree(searchHandle);
    native.closeDocument(doc);
  }

  // Open (average).
  let openTotal = 0;
  for (let i = 0; i < ITERATIONS; i++) {
    const t = ns(() => {
      const doc = native.openDocument(path);
      native.closeDocument(doc);
    });
    openTotal += t;
  }
  const openNs = Math.round(openTotal / ITERATIONS);

  // Extract page 0 (average) + all pages + search on a single reused doc.
  const doc = native.openDocument(path);
  try {
    const pageCount = native.getPageCount(doc);

    let p0Total = 0;
    let textLen = 0;
    for (let i = 0; i < ITERATIONS; i++) {
      const t = ns(() => {
        const text = native.extractText(doc, 0);
        // Report UTF-8 byte count (not .length which counts UTF-16 code
        // units) so the number is directly comparable to Go/Rust.
        textLen = Buffer.byteLength(text, 'utf8');
      });
      p0Total += t;
    }
    const extractPage0Ns = Math.round(p0Total / ITERATIONS);

    const extractAllNs = ns(() => {
      for (let i = 0; i < pageCount; i++) {
        native.extractText(doc, i);
      }
    });

    const searchNs = ns(() => {
      const h = native.searchAll(doc, 'the', false);
      native.searchResultFree(h);
    });

    return {
      language: 'js',
      fixture: basename(path),
      sizeBytes: size,
      openNs,
      extractPage0Ns,
      extractAllNs,
      searchNs,
      pageCount,
      textLen,
    };
  } finally {
    native.closeDocument(doc);
  }
}

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error('usage: node bench.mjs <fixture.pdf>...');
  process.exit(1);
}

for (const path of args) {
  try {
    const result = benchFixture(path);
    console.log(JSON.stringify(result));
  } catch (err) {
    console.error(`js_bench failed for ${path}: ${err.message}`);
    process.exit(2);
  }
}
