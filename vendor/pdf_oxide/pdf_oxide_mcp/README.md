# pdf-oxide-mcp — PDF Extraction MCP Server for AI Assistants

An [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that gives AI assistants like Claude, Cursor, and GitHub Copilot the ability to extract text, markdown, and HTML from PDF files. Powered by [pdf_oxide](https://crates.io/crates/pdf_oxide), the fastest Rust PDF library. All processing runs locally — no files leave your machine.

[![Crates.io](https://img.shields.io/crates/v/pdf_oxide_mcp.svg)](https://crates.io/crates/pdf_oxide_mcp)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses)

## Install

```bash
brew install yfedoseev/tap/pdf-oxide    # Homebrew (macOS/Linux) — includes both CLI and MCP
cargo install pdf_oxide_mcp             # Cargo
```

## Configure Your AI Assistant

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "pdf-oxide": {
      "command": "pdf-oxide-mcp"
    }
  }
}
```

### Claude Code

Add to your project's `.mcp.json` or global settings:

```json
{
  "mcpServers": {
    "pdf-oxide": {
      "command": "pdf-oxide-mcp"
    }
  }
}
```

### Cursor

Add to Cursor's MCP configuration:

```json
{
  "mcpServers": {
    "pdf-oxide": {
      "command": "pdf-oxide-mcp"
    }
  }
}
```

### npx (no install required)

```json
{
  "mcpServers": {
    "pdf-oxide": {
      "command": "crgx",
      "args": ["pdf_oxide_mcp@latest"]
    }
  }
}
```

## Tools

The server exposes an `extract` tool with the following parameters:

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | yes | Path to the PDF file |
| `output_path` | string | yes | Path to write extracted content |
| `format` | string | no | `text` (default), `markdown`, or `html` |
| `pages` | string | no | Page range, e.g. `"1-3,7,10-12"` |
| `password` | string | no | Password for encrypted PDFs |
| `images` | boolean | no | Extract images to files alongside output |
| `embed_images` | boolean | no | Embed images as base64 data URIs (default: true) |

## How It Works

`pdf-oxide-mcp` implements the Model Context Protocol over stdin/stdout using JSON-RPC. When an AI assistant needs to read a PDF, it calls the `extract` tool with the file path and desired format. The server processes the PDF locally using the pdf_oxide library and returns the extracted content.

- **Text** — plain text extraction preserving reading order
- **Markdown** — structured output with headings, lists, and column-aware layout
- **HTML** — formatted HTML output
- **Images** — optional image extraction as separate files or embedded base64

## Use Cases

- **RAG pipelines** — Convert PDFs to markdown for retrieval-augmented generation with LangChain, LlamaIndex, or any framework
- **Document Q&A** — Ask Claude questions about PDF content directly
- **Data extraction** — Pull text and tables from invoices, reports, and forms
- **Academic research** — Parse papers and extract content for analysis
- **Code documentation** — Let AI assistants read PDF specs and documentation

## Performance

Built on pdf_oxide, which processes PDFs at 0.8ms mean per document with a 100% pass rate on 3,830 test PDFs. The MCP server adds minimal overhead — PDF processing is the same high-performance Rust core used by the library and CLI.

## Protocol

Implements [MCP protocol version 2024-11-05](https://modelcontextprotocol.io/) with:
- `initialize` — server capability negotiation
- `tools/list` — tool discovery
- `tools/call` — tool execution
- `ping` — health check

## Documentation

- **[Full Documentation](https://pdf.oxide.fyi)** — Getting started and guides
- **[MCP Setup Guide](https://pdf.oxide.fyi/docs/getting-started/mcp)** — Detailed configuration for each AI assistant
- **[GitHub](https://github.com/yfedoseev/pdf_oxide)** — Source code and issue tracker
- **[Model Context Protocol](https://modelcontextprotocol.io/)** — MCP specification

## Related Crates

- [`pdf_oxide`](https://crates.io/crates/pdf_oxide) — Rust PDF library (core)
- [`pdf_oxide_cli`](https://crates.io/crates/pdf_oxide_cli) — CLI tool with 22 PDF commands

## License

MIT OR Apache-2.0
