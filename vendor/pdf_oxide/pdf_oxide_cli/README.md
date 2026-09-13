# pdf-oxide — The Fastest PDF CLI Toolkit

A command-line tool for PDF text extraction, markdown conversion, search, merge, split, image extraction, and more. Built on [pdf_oxide](https://crates.io/crates/pdf_oxide), the fastest Rust PDF library (0.8ms mean, 100% pass rate on 3,830 PDFs). MIT licensed.

[![Crates.io](https://img.shields.io/crates/v/pdf_oxide_cli.svg)](https://crates.io/crates/pdf_oxide_cli)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses)

## Install

```bash
brew install yfedoseev/tap/pdf-oxide    # Homebrew (macOS/Linux)
cargo install pdf_oxide_cli             # Cargo
cargo binstall pdf_oxide_cli            # Pre-built binary via cargo-binstall
```

## Quick Start

```bash
pdf-oxide text report.pdf                      # Extract text
pdf-oxide markdown report.pdf -o report.md     # Convert to Markdown
pdf-oxide html report.pdf -o report.html       # Convert to HTML
pdf-oxide search report.pdf "neural.?network"  # Regex search
pdf-oxide images report.pdf -o ./images/       # Extract images
pdf-oxide merge a.pdf b.pdf -o combined.pdf    # Merge PDFs
pdf-oxide split report.pdf -o ./pages/         # Split into pages
```

## All Commands

| Command | Description |
|---------|-------------|
| `text` | Extract text from PDF pages |
| `markdown` | Convert PDF to Markdown with headings, lists, and layout |
| `html` | Convert PDF to HTML |
| `search` | Search PDF content with regex patterns |
| `images` | Extract images to files (PNG, JPEG, etc.) |
| `info` | Show PDF metadata, page count, and version |
| `metadata` | Read and write PDF metadata fields |
| `merge` | Combine multiple PDFs into one |
| `split` | Split PDF into individual pages |
| `compress` | Reduce PDF file size |
| `encrypt` | Password-protect a PDF |
| `decrypt` | Remove password from a PDF |
| `rotate` | Rotate pages by 90, 180, or 270 degrees |
| `crop` | Set page crop box dimensions |
| `delete` | Remove specific pages |
| `reorder` | Rearrange page order |
| `watermark` | Add text watermark to pages |
| `flatten` | Flatten form fields and annotations |
| `forms` | Read and fill PDF form fields |
| `bookmarks` | Extract document bookmarks/outline |
| `create` | Create new PDF documents programmatically |

## Features

- **22 commands** for complete PDF processing from the terminal
- **Fast** — powered by pdf_oxide, 5× faster than the industry leaders
- **PDF to Markdown** — headings, bullet lists, column-aware reading order
- **Regex search** — full regex pattern matching across pages
- **Image extraction** — extracts images from content streams, form XObjects, and inline images
- **Form filling** — read and write PDF form fields from the command line
- **Page range support** — use `--pages 1-5,10` on any command
- **JSON output** — add `--json` for machine-readable results
- **Interactive REPL** — run `pdf-oxide` with no arguments for interactive mode
- **Encrypted PDFs** — supply `--password` to open protected files
- **Cross-platform** — Linux, macOS, and Windows

## Usage Examples

### Extract text from specific pages

```bash
pdf-oxide text paper.pdf --pages 1-5
pdf-oxide text paper.pdf --pages 1,3,7-10
```

### Convert to Markdown for LLM/RAG pipelines

```bash
pdf-oxide markdown paper.pdf -o paper.md
pdf-oxide markdown paper.pdf --pages 1 --detect-headings
```

### Search across a PDF

```bash
pdf-oxide search contract.pdf "termination|cancellation"
pdf-oxide search paper.pdf "equation \d+" --json
```

### Merge and split

```bash
pdf-oxide merge chapter1.pdf chapter2.pdf chapter3.pdf -o book.pdf
pdf-oxide split book.pdf -o ./chapters/
```

### Work with forms

```bash
pdf-oxide forms w2.pdf                              # List fields
pdf-oxide forms w2.pdf --fill "employee_name=Jane"   # Fill fields
```

### Extract images

```bash
pdf-oxide images paper.pdf -o ./figures/ --pages 1-10
```

## Performance

pdf_oxide processes PDFs at 0.8ms mean per document — 5× faster than PyMuPDF, 15× faster than pypdf. Text extraction, markdown conversion, and all operations share the same high-performance Rust core.

## Documentation

- **[Full Documentation](https://pdf.oxide.fyi)** — Getting started, CLI guide, and API reference
- **[CLI Guide](https://pdf.oxide.fyi/docs/getting-started/cli)** — Detailed command reference
- **[GitHub](https://github.com/yfedoseev/pdf_oxide)** — Source code and issue tracker

## Related Crates

- [`pdf_oxide`](https://crates.io/crates/pdf_oxide) — Rust PDF library (core)
- [`pdf_oxide_mcp`](https://crates.io/crates/pdf_oxide_mcp) — MCP server for AI assistants

## License

MIT OR Apache-2.0
