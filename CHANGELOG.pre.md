# Prerelease Changelog

<!--
  PRERELEASE VERSIONS ONLY. Stable releases go in CHANGELOG.md.

  How to add a prerelease:
  1. Add a new ## heading at the top with the prerelease version (e.g. 1.1.0-dev.0)
  2. Write a summary of what changed SINCE THE PREVIOUS ENTRY IN THIS FILE
     - First prerelease after a stable: changes since the last stable version
     - Subsequent prereleases: changes since the previous prerelease
  3. Run: dart run tool/commits.dart v<PREVIOUS_TAG>
     (e.g. dart run tool/commits.dart v1.1.0-dev.0)
     Copy the <details> block it prints and paste it at the end of your entry
  4. Commit and push to dev
  5. CI reads the version from the top ## heading, tags, and publishes as prerelease

  Rules:
  - Version in ## heading is the source of truth for the prerelease version
  - Each entry covers changes since the PREVIOUS entry in THIS file (not CHANGELOG.md)
  - pub.dev shows this file as CHANGELOG.md for the prerelease version
    (CI copies this file to CHANGELOG.md in the published tarball)
  - When the stable release ships, write the full summary in CHANGELOG.md
    covering everything since the last stable — this file is not consulted
  - Entries here are permanent history — don't delete old entries
-->

## 1.0.0-dev.0

Complete v1 rewrite. New Rust engine, new Dart API, every platform.

### Features

- Four-layer architecture: Consumer API → Transport → Rust Host → Engine
- Shared `dispatch.rs` — all operations (read + edit) go through one brain, both platforms
- Symmetric file naming: `bridge↔bridge`, `coordinator↔coordinator`, `wire↔wire`, `ffi_api↔wasm_api`, `ffi_encode↔wasm_encode`
- `DataSource` / `DataSink` I/O — no file paths, no `dart:io`, works on every platform
- Sealed types: `PdfPages`, `PdfError`, `PdfEncryption`, `PdfWatermarkPosition`
- `PdfWatermarkPosition.center()` / `.corner()` / `.tiled()` / `.exact()` — engine resolves per page
- `PdfWatermarkLayer.foreground` / `.background` — annotation vs content-stream watermarks
- `PdfEditor` — open once, mutate many times, save once (full rewrite or incremental)
- `PdfBuilder` — create PDFs from scratch with text, images, forms, links
- Streaming: `render()` and `extractImages()` yield one item at a time
- Off-main-thread on every platform: Rust thread pool (native), Web Worker pool (web)
- Condvar streaming I/O (native) + Atomics/OPFS (web)
- Arena allocator per operation (bumpalo)
- 50+ operations: merge, split, extract, search, render, sign, encrypt, validate, convert, classify, redact, compress, watermark, stamp, embed, flatten

### Bug Fixes

- Fixed 5 silently-dropped API parameters across the bridge layer
- Fixed native comboBox builder sending zero parameters to Rust
- Fixed web watermark loop bug (was calling WASM per-page instead of single dispatch)
- Fixed FFI bypass deadlock (pageCount, isModified, getPageMediaBox routed through worker)

### Tests

- 129 behavioral tests (zero liveness tests — every test verifies output content)
- API coverage check across all 112 public methods
- Mobile integration tests upgraded from liveness to behavioral

<details><summary>Commits since 945a51f (28)</summary>

- 4f8d16a feat!: v1 architecture — symmetric dispatch, sealed types, behavioral tests, release system
- ff59008 feat: streaming I/O for ALL ops + incremental writer + sign via editor
- 846a67f refactor: shared split algorithms + O(n²) → O(k log n) splitBySize
- f49835d feat: 200-page stress tests with splitBySize edge cases
- 51aaf3b feat: stress tests + fix multi-page builder bug
- b46ed40 feat: example covers all 37 Pdf methods + editor redaction
- 479fbab fix: build hook PATH stripping + example app update
- 2149445 fix: update example app + integration test for current API
- 3886973 feat: tests for all new features + bookmarked PDF fixture
- fbc9cd1 feat: Rust bridge handlers for bookmarks, classify, redaction
- d07a9ee feat: wire upstream features — bookmarks, classify, convert, redaction
- 80ebb12 docs: update UPDATING.md — branch rename step, v0.3.53 provenance
- 890c793 build: sync upstream v0.3.53 + rebuild WASM
- dcbbece feat: streaming I/O redesign — restructure, web parity, sealed sign API
- d97803e feat: wire Layer 1 + delete old code (B20-B21)
- 2d2b2dd feat: new bridge architecture — thread pool, condvar streaming, arena allocator
- 3c1876e chore: simplify merge protection — native GitHub only, no rulesets
- 645a2c3 feat: review policy CI check replaces rulesets
- 3f799dc docs: fix stale workflow refs — pr-lint→pr-checks, Android→ubuntu
- 0fd8b09 docs: re-stamp AGENTS.md — pr-lint.yml → pr-checks.yml, main → prod
- ef5e821 docs: fix last two main → prod references (slopfairy review)
- 7406e75 perf: sccache + mold + disable incremental for faster CI builds
- d37ef5c ci: merge 3 PR workflows into one pr-checks.yml
- 14f917f docs: fix remaining main → prod references (slopfairy review)
- 17974a0 chore: update all workflow branches + docs from main → prod
- 76e2917 chore: rename main → prod everywhere (branch, workflows, docs, ruleset)
- 8dcc13f ci: commit-msg hook (merge blocker + printf), promotion-check, hooksPath docs (#23)
- 074868f ci: fix release-please config and PR formatting (#21)

</details>
