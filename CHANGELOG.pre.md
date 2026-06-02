# Prerelease Changelog

<!--
  PRERELEASE VERSIONS ONLY. Stable releases go in CHANGELOG.md.

  How to add a prerelease:
  1. Add a new ## heading at the top with the prerelease version (e.g. 1.1.0-dev.0)
  2. Write a summary of what changed SINCE THE PREVIOUS ENTRY IN THIS FILE
     - First prerelease after a stable: changes since the last stable version
     - Subsequent prereleases: changes since the previous prerelease
  3. Generate the commit list for the <details> block:
       git log v<PREVIOUS_TAG>..HEAD --oneline --no-decorate
     Wrap the output in a <details> block and paste at the end of your entry
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

Complete ground-up rewrite. New engine, new API, every platform. See the [migration guide](docs/MIGRATION.md) for upgrading from the old Android-only version.

### What changed

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the previous Android-only backend
- **Platforms:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** Instance-based `Pdf()` with `dispose()`. Batch editing via `pdf.edit(source)`. Create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`. Same code on every platform. Engine reads only what it needs, never the full file
- **Errors:** Typed `PdfError` sealed class — no more `PlatformException`
- **Performance:** Every operation runs off the main thread. Zero UI jank. No full-file buffers
- **SDK:** Requires Dart >=3.10.0

### Capabilities

- Open and inspect (page count, version, dimensions, metadata, encryption, permissions)
- Merge, split, split by size, split by bookmarks
- Extract pages, delete pages, reorder, move page
- Rotate (per-page and all pages)
- Compress with image optimization
- Watermark (styled, positioned — sealed PdfWatermarkPosition with center/corner/tiled/exact, foreground/background layer)
- Encrypt (4 algorithms, 8 permission flags) and decrypt
- Digital signatures (inspect, verify, sign via PKCS12/PEM)
- Extract text, Markdown, HTML
- Search text with bounding rectangles
- Render pages to RGBA images (Stream)
- Extract embedded images (Stream)
- PDF/A and PDF/UA validation
- Page and document classification
- Convert to/from DOCX, PPTX, XLSX
- PdfEditor — open once, mutate many, save once (full rewrite or incremental)
- PdfBuilder — create PDFs from scratch (text, headings, images, form fields, links, columns, footnotes)
- Form fields: text, checkbox, combo box, push button, signature
- Stamp annotations (14 built-in types + image stamps)
- Font unembedding, image resize, crop margins
- Embed files, erase regions, flatten forms/annotations
- Redaction (add, count, apply, scrub metadata)
- PDF/A conversion
- Resource pruning on GC save
- Images to PDF
- 21 sugar methods on PdfOperations extension

<details><summary>Commits since a99fbdb (62)</summary>

- 95a196b build: update pdf_oxide submodule — watermark position/layer + comboBox fix
- e814e6d Merge remote-tracking branch 'origin/dev' into chore/rename-main-to-prod
- e547a68 fix(ci): handle large PR diffs in triage workflow
- e154b4c docs: add 1.0.0-dev.0 prerelease changelog entry
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
- 7456369 chore: rename main → prod (workflows, docs, ruleset) (#26)
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
- 945a51f ci: add release-please + tag-triggered release pipeline (#19)
- e5e698e ci: enforce conventional commits via hook + PR title check (#17)
- 67edc60 add pre-commit hook: reject commits with gitignored files
- da16d8a untrack example/pubspec.lock (gitignored, should never have been tracked)
- 9a3dc68 fix: .pubignore for vendor/ + compile WASM before web tests
- 02e92a5 fix(ci): checkout in workflows before local composite actions
- 55d9e58 ci: 10 composite actions, zero duplication, test-first flow
- e6bf54b release: manual re-trigger + overwrite existing releases
- 9328919 ci: split WASM into its own compile job
- dfcd65f fix: WASM target + Rust warnings in release CI
- 2dbb00e hook: rewrite using native_toolchain_rust patterns
- 28daa58 hook: use StaticLinking for iOS + install both device and sim targets
- c8f4bfe hook: set NDK linker for Android cargo cross-compilation
- 4e72a4a ci: add x86_64-linux-android rust target for Android emulator tests
- c9c26ab ci: fix android emulator-runner cd not persisting between commands
- eef11c8 ci: commit .metadata, remove destructive flutter create
- f144ba5 ci: fix Android + iOS integration test failures
- 6b9e7d7 updating.md: add submodule commit discipline (3-step sequence)
- bf50a39 submodule: update to include all Rust patches
- 8fcec5f ci: fix pub get failure — skip example deps in Dart-only jobs
- a62a36b roadmap: add upstream v0.3.48 office conversion features
- 621f091 fix upstream URL + expand S1 bump recipe
- 8e67064 provenance: link to fork repo + branch, document fork convention
- b93b9ec v1.0.0 API redesign + web feature parity + doc cleanup
- e7e0338 editor: add missing addStamp + addWatermarkPositioned wrappers
- 4ba4264 fix migration guide + expand contributing
- 0374c59 readme: feature×platform matrix, docs table, community files
- c0e712c bump to 1.0.0, fix stale README references
- 581e844 v1.0.0 — complete ground-up rewrite

</details>
