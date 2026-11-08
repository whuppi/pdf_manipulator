# PDF Oxide for WASM — The Fastest PDF Toolkit for Browsers, Deno, Bun & Edge

The fastest WebAssembly PDF library for text extraction, image extraction, and markdown conversion. Powered by a pure-Rust core compiled to WebAssembly. Runs in Node.js, browsers, Deno, Bun, and serverless edge runtimes — no native binaries, no `node-gyp`, no `postinstall`. 0.8ms mean per document, 5× faster than PyMuPDF, 15× faster than pypdf. 100% pass rate on 3,830 real-world PDFs. MIT / Apache-2.0 licensed.

[![npm](https://img.shields.io/npm/v/pdf-oxide-wasm)](https://www.npmjs.com/package/pdf-oxide-wasm)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses)

> **Part of the [PDF Oxide](https://github.com/yfedoseev/pdf_oxide) toolkit.** Same Rust core, same speed, same 100% pass rate as the [Rust](https://docs.rs/pdf_oxide), [Python](https://github.com/yfedoseev/pdf_oxide/blob/main/python/README.md), [Go](https://github.com/yfedoseev/pdf_oxide/blob/main/go/README.md), [JavaScript / TypeScript (Node.js native)](https://github.com/yfedoseev/pdf_oxide/blob/main/js/README.md), and [C# / .NET](https://github.com/yfedoseev/pdf_oxide/blob/main/csharp/README.md) bindings.
>
> Need a faster Node.js binding with native code? Use [pdf-oxide](https://www.npmjs.com/package/pdf-oxide) instead — same API, native N-API addon.

## Quick Start

```bash
npm install pdf-oxide-wasm
```

```javascript
const { WasmPdfDocument } = require("pdf-oxide-wasm");
const fs = require("fs");

const bytes = new Uint8Array(fs.readFileSync("paper.pdf"));
const doc = new WasmPdfDocument(bytes);

console.log(doc.extractText(0));
console.log(doc.toMarkdown(0));

doc.free();
```

## Why pdf-oxide-wasm?

| Feature | pdf-oxide-wasm | pdf-parse | pdf-lib | pdfjs-dist |
|---|---|---|---|---|
| Text extraction | Yes | Yes | No | Yes |
| Markdown / HTML output | Yes | No | No | No |
| PDF creation | Yes | No | Yes | No |
| Form field read/write | Yes | No | Partial | No |
| Full-text search (regex) | Yes | No | No | No |
| Image extraction | Yes | No | No | No |
| Merge, encrypt, edit | Yes | No | Yes | No |
| Serverless / edge runtimes | Yes | No | No | No |
| Zero native dependencies | Yes | Yes | Yes | No |
| WebAssembly-based | Yes | No | No | No |
| TypeScript types included | Yes | No | Yes | Yes |
| License | MIT / Apache-2.0 | MIT | MIT | Apache-2.0 |

- **Fast** — 0.8ms mean per document, 5× faster than PyMuPDF, 15× faster than pypdf
- **Reliable** — 100% pass rate on 3,830 test PDFs, zero panics, zero timeouts
- **Universal** — Runs in Node.js, browsers, Deno, Bun, and Cloudflare Workers without modification
- **Zero install friction** — No native binaries, no `node-gyp`, no `postinstall` scripts
- **Pure Rust core** — Memory-safe, panic-free, compiled straight to WebAssembly
- **Full TypeScript support** — Type definitions ship in the package

## Performance

Benchmarked on 3,830 PDFs from three independent public test suites (veraPDF, Mozilla pdf.js, DARPA SafeDocs). Text extraction libraries only. Single-thread, 60s timeout, no warm-up.

| Library | Mean | p99 | Pass Rate | License |
|---------|------|-----|-----------|---------|
| **PDF Oxide** | **0.8ms** | **9ms** | **100%** | **MIT / Apache-2.0** |
| PyMuPDF | 4.6ms | 28ms | 99.3% | AGPL-3.0 |
| pypdfium2 | 4.1ms | 42ms | 99.2% | Apache-2.0 |
| pdftext | 7.3ms | 82ms | 99.0% | GPL-3.0 |
| pdfminer | 16.8ms | 124ms | 98.8% | MIT |
| pypdf | 12.1ms | 97ms | 98.4% | BSD-3 |

99.5% text parity vs PyMuPDF and pypdfium2 across the full corpus. The WASM compilation preserves near-native performance — no garbage collection overhead, no child process spawning, no temp files.

## Installation

```bash
npm install pdf-oxide-wasm
```

Works without modification in:

- **Node.js** 18+ (CommonJS and ESM)
- **Browsers** — Chrome, Firefox, Safari, Edge
- **Cloudflare Workers** — runs in V8 isolates with WASM support
- **Deno** — native WASM support
- **Bun** — native WASM support

No native binaries, no system dependencies, no build step.

## API Tour

### Open and extract text

```javascript
const { WasmPdfDocument } = require("pdf-oxide-wasm");
const fs = require("fs");

const bytes = new Uint8Array(fs.readFileSync("document.pdf"));
const doc = new WasmPdfDocument(bytes);

console.log(`Pages: ${doc.pageCount()}`);
console.log(doc.extractText(0));        // plain text
console.log(doc.toMarkdown(0));         // markdown
console.log(doc.toHtml(0));             // HTML

doc.free();
```

ESM / TypeScript:

```typescript
import { WasmPdfDocument } from "pdf-oxide-wasm";
import { readFile } from "fs/promises";

const bytes = new Uint8Array(await readFile("document.pdf"));
const doc = new WasmPdfDocument(bytes);

const text = doc.extractAllText();
const markdown = doc.toMarkdownAll();

doc.free();
```

### Search

```javascript
const results = doc.search("quarterly revenue", true); // case-insensitive
// Returns: [{ page, text, bbox, start_index, end_index, span_boxes }]
```

### Form fields

```javascript
const fields = doc.getFormFields();
// [{ name, field_type, value, tooltip, bounds, is_readonly, is_required }]

doc.setFormFieldValue("name", "Jane Doe");
doc.setFormFieldValue("agree_terms", true);

const filledPdf = doc.saveToBytes();
```

### Create a PDF from Markdown

```javascript
import { WasmPdf } from "pdf-oxide-wasm";

const pdf = WasmPdf.fromMarkdown("# Invoice\n\nTotal: $42.00", "Invoice", "Acme Corp");
const bytes = pdf.toBytes();
```

### Encrypt a PDF (AES-256)

```javascript
const encrypted = doc.saveEncryptedToBytes(
  "user-password",
  "owner-password",
  true,  // allow print
  false, // deny copy
);
```

### Render and extract images

```javascript
const images = doc.extractImages(0);
const pngBytes = doc.extractImageBytes(0);
```

### Edit metadata, pages, and content

```javascript
doc.setTitle("Quarterly Report");
doc.setAuthor("Example Author");
doc.setPageRotation(0, 90);
doc.cropMargins(36, 36, 36, 36);
doc.eraseRegion(0, 50, 50, 200, 100);
doc.flattenAllAnnotations();

const editedBytes = doc.saveToBytes();
```

## Other languages

PDF Oxide ships the same Rust core through six bindings:

- **Rust** — `cargo add pdf_oxide` — see [docs.rs/pdf_oxide](https://docs.rs/pdf_oxide)
- **Python** — `pip install pdf_oxide` — see [python/README.md](https://github.com/yfedoseev/pdf_oxide/blob/main/python/README.md)
- **Go** — `go get github.com/yfedoseev/pdf_oxide/go` — see [go/README.md](https://github.com/yfedoseev/pdf_oxide/blob/main/go/README.md)
- **JavaScript / TypeScript (Node.js native)** — `npm install pdf-oxide` — see [js/README.md](https://github.com/yfedoseev/pdf_oxide/blob/main/js/README.md)
- **C# / .NET** — `dotnet add package PdfOxide` — see [csharp/README.md](https://github.com/yfedoseev/pdf_oxide/blob/main/csharp/README.md)

A bug fix in the Rust core lands in every binding on the next release.

## Documentation

- **[Full Documentation](https://pdf.oxide.fyi)** — Complete documentation site
- **[WASM Getting Started](https://github.com/yfedoseev/pdf_oxide/blob/main/docs/getting-started-wasm.md)** — Step-by-step WASM guide
- **[Main Repository](https://github.com/yfedoseev/pdf_oxide)** — Rust core, CLI, MCP server, all bindings
- **[Performance Benchmarks](https://pdf.oxide.fyi/docs/performance)** — Full benchmark methodology and results
- **[GitHub Issues](https://github.com/yfedoseev/pdf_oxide/issues)** — Bug reports and feature requests

## Use Cases

- **Browser PDF tooling** — Extract, search, and convert PDFs entirely client-side, no server upload
- **Edge / serverless workers** — Process PDFs in Cloudflare Workers, Vercel Edge, Deno Deploy
- **RAG / LLM pipelines** — Convert PDFs to clean Markdown for retrieval-augmented generation
- **PDF generation** — Create invoices, reports, certificates programmatically without a backend
- **Universal Node.js packages** — Same code runs in Node.js, the browser, and edge runtimes

## Why I built this

I needed PyMuPDF's speed without its AGPL license, and I needed it in more than one language. Nothing existed that ticked all three boxes — fast, MIT, multi-language — so I wrote it. The Rust core is what does the real work; the bindings for Python, Go, JS/TS, C#, and WASM are thin shells around the same code, so a bug fix in one lands in all of them. It now passes 100% of the veraPDF + Mozilla pdf.js + DARPA SafeDocs test corpora (3,830 PDFs) on every platform I've tested.

If it's useful to you, a star on GitHub genuinely helps. If something's broken or missing, [open an issue](https://github.com/yfedoseev/pdf_oxide/issues) — I read all of them.

— Yury

## License

Dual-licensed under [MIT](https://github.com/yfedoseev/pdf_oxide/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/yfedoseev/pdf_oxide/blob/main/LICENSE-APACHE) at your option. Unlike AGPL-licensed alternatives, pdf_oxide can be used freely in any project — commercial or open-source — with no copyleft restrictions.

## Citation

```bibtex
@software{pdf_oxide,
  title = {PDF Oxide: Fast PDF Toolkit for Rust, Python, Go, JavaScript, and C#},
  author = {Yury Fedoseev},
  year = {2025},
  url = {https://github.com/yfedoseev/pdf_oxide}
}
```

---

**WASM** + **Rust core** | MIT / Apache-2.0 | 100% pass rate on 3,830 PDFs | 0.8ms mean | 5× faster than the industry leaders
