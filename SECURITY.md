# Security Policy

## Reporting a vulnerability

Report privately via [GitHub Security Advisories](https://github.com/whuppi/pdf_manipulator/security/advisories/new). Do not open a public issue.

## What's in scope

- **Pre-built binary supply chain** — consumers get native binaries and web assets from GitHub Releases via the build hook and setup command. SHA-256 hashes in `lib/src/hook/asset_hashes.dart` verify integrity for all platforms. A compromised release asset or tampered hash file is a valid security report.

- **Data leakage via the transport layer** — if the Dart↔engine bridge exposes data from one operation to another (e.g. a previous PDF's content leaking into a subsequent operation's output), that's a security issue.

- **Crafted PDF input causing code execution or sandbox escape** — Rust's memory safety mitigates this, but if a crafted PDF achieves anything beyond a crash, report it here.

## What's NOT in scope

- **Crashes, hangs, or panics from malformed PDFs** — these are bugs, not security vulnerabilities. Report them as [regular issues](https://github.com/whuppi/pdf_manipulator/issues).

- **Web Worker sandbox** — on web, the engine runs inside a WASM Web Worker. The browser's sandbox prevents access to the main thread's DOM or cookies.

- **Network fetching** — the engine never makes network requests. All I/O goes through the caller's `DataSource`/`DataSink`. URL-based attacks in PDF content are inert.

- **Upstream Rust engine bugs** not triggered through this package — report to [yfedoseev/pdf_oxide](https://github.com/yfedoseev/pdf_oxide).

## Response

Valid reports are fixed and shipped as patch versions.
