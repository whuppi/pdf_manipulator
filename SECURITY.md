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

## Operational notes (known, accepted)

These are conscious trade-offs, documented so they aren't mistaken for oversights:

- **Prebuilt binaries depend on the main repo's Releases.** Source is baked into each release tag (it survives even if the engine submodule repos are deleted), but the prebuilt download URLs point at this repo's Releases. Deleting the main repo 404s them, so fresh installs fall back to compile-from-source (which needs Rust).

- **pub.dev secret scanning is disabled for `vendor/**`.** The vendored engine source is excluded from publish-time secret scanning, so a secret accidentally committed into the engine would not be flagged. Self-inflicted only — we control the engine source.

- **CI runs Chrome with `--no-sandbox`.** Standard for CI runners (no SUID sandbox available); only the trusted example app is ever loaded, and the runner is ephemeral.

- **`git:`-ref consumers fetch the engine at build time.** Depending on this package by git ref initializes the engine submodules from `whuppi/*`. The supply-chain trust is the same as any git dependency.

- **Some pinned binary hashes are self-computed, not upstream-published.** FVM (leoafarias/fvm) and Chrome for Testing publish no digests, so their `tool/versions.env` sha256 pins come from the assets we first downloaded, not from an upstream source of truth. The pins still catch a later swap (a repointed tag, a CDN substitution), and `upgrade.sh verify-pinned` re-hashes them on the daily radar while `check-availability` HEADs them on every PR, so a swap or prune surfaces before a build fails on it. But the first download still defines trust: had a release already been compromised when `upgrade.sh` first fetched it, the bad hash would simply be recorded. Inherent to the first-to-download problem; not closable without upstream digest publication.

## Response

Valid reports are fixed and shipped as patch versions.
