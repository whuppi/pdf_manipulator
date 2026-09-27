# Prerelease Changelog

<!--
═══════════════════════════════════════════════════════════════════════
CHANGELOG STANDARD — read before editing. Applies to both changelogs.
═══════════════════════════════════════════════════════════════════════
Two INDEPENDENT lane changelogs — do NOT mirror one from the other:
  • CHANGELOG.pre.md — the prerelease lane (`## X.Y.Z-dev.N`). Add an
    entry per prerelease you cut on dev.
  • CHANGELOG.md — the stable lane (`## X.Y.Z`). Add an entry per stable
    release, CONSOLIDATING the prerelease entries that ship under it.
They share prose but track their OWN version sequences. There is no
`cp + sed` regen: that mirror falsely assumed every prerelease becomes a
same-numbered stable, so it manufactured stable headings for versions
that never shipped — which `tool/ci/release.sh --check-versions` now flags.
Hand-edit each lane's file directly.

VERSION NUMBERS ARE SEMVER — the bullets decide the number
  Write the entry FIRST, then read it back; the bullets dictate the bump:
  • Any **Breaking:** bullet → MAJOR. No exceptions — not "it's tiny",
    not "nobody uses that knob". Caret consumers (^X.Y.Z) auto-upgrade
    through minors and patches, so a breaking minor/patch ambushes them.
  • Added / Changed capability → MINOR.
  • Only Fixed bullets → PATCH.
  If the heading you were about to write disagrees with its own bullets,
  the heading is wrong — renumber before merging. Precedent: 2.2.0
  shipped a **Breaking:** bullet as a minor and had to be retracted;
  never again.

ADDING A VERSION
  Add a heading at the TOP (newest first) of the right lane's file and
  write the summary. Exactly ONE new (untagged) version may sit at the
  top of each file — every heading BELOW it must already have its git tag
  (or a verified `release: no-tag` HTML-comment directive). `--check-versions`
  enforces this at PR + release time: a second un-released version is
  rejected, since it would collapse into the one release the merge cuts.
  Versions, commit lists, tags, publishing — the release tooling owns all
  of it; you only write the human summary.

ENTRY SHAPE
  ## X.Y.Z-dev.0
  <one-line prose lead — only to frame a big release or signal "no
   behavior change"; omit when the bullets speak for themselves>
  - **Breaking:** <what changed> → <migration step, INLINE>   ← always first
  - <upgrade action, e.g. "re-run setup --force web">         ← any required action next
  - Added/Changed <capability or improvement>                ← then improvements
  - Fixed <symptom> — <the fix, one clause> ([#N](issue-url) reported by [@user](abs-url))  ← fixes last

  Order IS the grouping — Breaking → action → added/changed → fixed. No
  `###` subsections: bullet order carries the categories. Only Breaking
  is bold-tagged; everything else is verb-led. Fixes start with "Fixed".

BULLET SHAPE — one change, one sentence
  A reader asks two things: does this touch me, and what do I change.
  Answer those and stop.
  • `<Verb> <what the reader sees changed> — <one clause: the fix or
    the why> (<issue credit>)`. A Fixed bullet names the symptom first,
    as the user met it (the error text, the wrong result), then the fix.
  • One sentence. A **Breaking** bullet may add a second: the migration.
    No trailing period.
  • Under 40 words is the target; 60 is the wall, 80 for a Breaking
    bullet whose migration is inline. `make test-guards` counts.
  • The cause, the mechanism, the spec citation and the proof belong in
    the PR, which the "Commits since" list under every entry already
    reaches. A bullet that explains why the bug existed is a PR body in
    the wrong file.
  • Short is not vague: "Fixed image optimization" is short and useless;
    the symptom and the change must both survive the cut.

  EXCEPTION — the genesis entry (a ground-up rewrite, no prior version)
  uses facet tags instead of deltas: **Engine:** / **API:** / **I/O:** /
  etc., describing the new package's dimensions. See the 1.0.0 entry.

CONTENT RULES (never change)
  • Migrate from the entry ALONE — breaking changes inline, old → new.
    (pub.dev freezes each version's CHANGELOG as a snapshot, so an entry
    can't rely on anything that later moves.)
  • NEVER link a living doc (README, docs/*) from an entry — it rots when
    the doc moves on. The migration guide is reached from the README.
  • Links point only at IMMUTABLE targets. Credit the issue and its
    reporter when a reported issue drove the change:
    ([#N](https://github.com/whuppi/pdf_manipulator/issues/N) reported by
    [@user](https://github.com/user)). No PR links in bullets: the release
    tooling appends a "Commits since" list with every PR number under
    each entry, so the trail exists without lengthening the line. A PR in
    another repository (a contributor's engine fix) may be linked as the
    credit, since no commit list here reaches it.
  • No capability inventories — "what's shipped" lives in README +
    docs/CAPABILITY_ROADMAP.md; the changelog says only what CHANGED.
  • Engine/submodule bump → web re-fetch action (NEVER miss this). When a
    release bumps a vendored engine submodule (vendor/pdf_oxide,
    vendor/office_oxide), the web WASM is rebuilt and consumers must
    re-fetch it, so ALWAYS add the action bullet:
      - Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
    Native self-updates via the build hook; only web needs the manual step.
    When cutting a release, diff the submodule pointer against the previous
    tag (`git ls-tree <prev-tag> vendor/pdf_oxide`) so an engine bump never
    ships without the bullet.
═══════════════════════════════════════════════════════════════════════
-->

<!-- Add new versions below, newest first. -->

## 5.0.1-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a `keep:` list without `extract`, such as `keep: []`, failing to compile the engine with `cannot find search in crate` — the search module now compiles in every keep-set ([#256](https://github.com/whuppi/pdf_manipulator/issues/256) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed a PDF encrypted with Standard Security R2–R4 (RC4) failing to open on web with `compiled without 'legacy-crypto'` — the web engine now carries the same legacy encryption and ICC colour support as native ([#257](https://github.com/whuppi/pdf_manipulator/issues/257) reported by [@crurui](https://github.com/crurui))

## 5.0.0-dev.0

- **Breaking:** `PdfEditor.optimizeImages(quality:, minSize:)` is removed → use `reduceImages(PdfImagePolicy(jpegQuality: quality, minPixels: minSize, convertCmykToRgb: true))`; the count it returned is `report.changed`
- **Breaking:** `compress(imageQuality:)` → `compress(images:)`, a `PdfImagePolicy` defaulting to `PdfImagePolicy.ebook` (150 ppi, JPEG quality 75), so a plain `compress` now downsamples images drawn above 225 ppi; pass `PdfImagePolicy(jpegQuality: q)` for the old behaviour
- **Breaking:** the editor's argument-less reads are getters (a property reads as a noun, never with a `get` prefix): `getTitle()` → `title`, `getAuthor()` → `author`, `getSubject()` → `subject`, `getKeywords()` → `keywords`, `getProducer()` → `producer`, `getCreationDate()` → `creationDate`, and on `PdfDoc` `getSignatures()` → `signatures`; the one read with an argument is a noun method, `getPageMediaBox(page)` → `pageMediaBox(page)`
- **Breaking:** `flattenAllAnnotations()` is `flattenAnnotations({int? page})`, and `flattenForms()` gains the same optional `page`; without it both flatten every page as before
- **Breaking:** `applyRedactions()` now returns a `PdfRedactionReport` (regions, glyphs removed, images modified or removed, paths pruned) instead of `void`; awaiting it still applies the redactions
- **Breaking:** `mergeFrom(other)` gains `pages:` (a 0-based list; `null` merges every page) and `embedFile(name, data)` gains `description:`, `mimeType:` and `relationship:`; positional callers are unchanged
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added `PdfEditor.pageImages(page)` — every image XObject drawn on a page with its resource name, bounds and transform; the name is what `resizeImage` takes
- Added `PdfEditor.repositionImage(page, name, x:, y:)` and `setImageBounds(page, name, bounds)` — move, or move and resize, an image by the name `pageImages` lists
- Added `PdfEditor.reduceImages(PdfImagePolicy)` — downsamples images to the resolution they are drawn at and re-encodes them by kind (JPEG, predicted Flate, CCITT G4), soft masks included, presets `screen`/`ebook`/`print`/`lossless`, one `PdfImageReport` row per image with the reason when it is kept; images reached only through annotation appearances, patterns or inline `BI … EI` stay as stored
- Added `compress(images: PdfImagePolicy.screen)` — the one-shot takes the same policy
- Added `PdfImagePolicy.minSavings` (default 10%: a lossy re-encode must earn its bytes or the image is kept), `chromaSubsampling` (`auto` = 4:4:4 from quality 90, `full`, `half`; `print` pins `full`) and `recompressJpeg` (re-encode stored JPEGs at `jpegQuality` even when not downsampled; off by default)
- Added `PdfEditor.pageCropBox(page)` (`null` when the page has no CropBox), `setPageMediaBox`, `setPageCropBox` and `setPageRotation(page, degrees:)` (absolute, where `rotatePage` is relative)
- Added `PdfEditor.clearEraseRegions(page)` to drop the regions queued by `eraseRegions` before save
- Added `PdfEditor.sanitize(PdfSanitizeOptions)` — strip metadata, JavaScript actions and embedded files in one pass; `scrubMetadata()` is its metadata-only form
- Added `PdfDoc.formFields` and `formField(name)` — every AcroForm field with its type, typed value, tooltip, bounds, max length, alignment and read-only/required flags
- Added the form field property setters on `PdfEditor`: `removeFormField`, `setFormFieldReadOnly`, `setFormFieldRequired`, `setFormFieldTooltip`, `setFormFieldBounds`, `setFormFieldMaxLength`, `setFormFieldAlignment`, `setFormFieldBackgroundColor`, `setFormFieldBorderColor`, `setFormFieldBorderWidth`, `setFormFieldAppearance` and `setFormFieldFlags` — each read back after save by `formFields`
- Added `PdfDoc.exportFormData(sink, format:)` — the filled values as FDF or XFDF
- Added `PdfDoc.xfa` (`PdfXfaInfo` with field count, page count and field types, or `null` without an XFA packet) and the one-shot `convertXfaToAcroForm(source, output)` that rewrites an XFA form as a plain AcroForm
- Added `PdfDoc.attachments` (name, size, description, MIME type, read from the name tree without decoding the files) and `extractAttachment(name, sink)`
- Fixed images optimized in an edit session being saved unchanged — the full-rewrite writer copied page-referenced XObjects from the source and skipped the staged replacement
- Fixed `pageImages` listing Form XObjects — only `/Subtype /Image` resources are listed, so every name is a valid `resizeImage` target

## 4.2.1-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a DOCX table converting to PDF several times wider than declared, columns off the page — the converter now honours the `w:tblGrid` widths, scaled to the printable width ([#243](https://github.com/whuppi/pdf_manipulator/issues/243) reported by [@kampmapa1-design](https://github.com/kampmapa1-design))
- Fixed a web instance failing with `WASM init failed: Out of memory` during rapid create-and-dispose churn — a worker slot is freed when the browser confirms the worker is gone, not at `terminate()`
- Fixed `pageMediaBox` returning the box's far corner as its width and height whenever the MediaBox origin is not (0, 0)
- Fixed form field property changes being lost on save unless the form was also flattened — the engine wrote only the value; every property and a removed field now reach the saved file
- Fixed `eraseRegions` and `addRedaction` acting on the wrong rectangle — the width and height were passed where the far corner was expected, so every region past the origin was off by its own size
- Fixed `addStamp` losing the stamp on save when its rectangle sits high on the page — the same size-for-corner mix-up placed the label above the page edge; the builder's `image` was drawn at the wrong size for the same reason
- Fixed `mergeFrom` dropping the merged document's form fields — the pages arrived but no `/Fields` entry pointed at them, so they could not be listed or filled; the AcroForm is carried across now
- Fixed an embedded file's MIME type being written as `text#232Fplain` — the solidus was escaped twice
- Vendored engines synced upstream: pdf_oxide v0.3.73 → v0.3.78 and office_oxide v0.1.3 → v0.1.11 (extraction, rendering, table-structure and legacy `.doc` fixes)

## 4.2.0-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added `setCheckboxFieldValue(fieldName, checked)` — checks a checkbox without knowing its on-state name (`/Yes`, `/On`, `/1` …); `setFormFieldValue` still takes the name directly
- Fixed a checked checkbox flattening as an empty box and losing its value on reopen — button values are read, resolved and written as appearance-state names, and flattening draws the state that was set ([#215](https://github.com/whuppi/pdf_manipulator/issues/215) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed radio groups being impossible to select — the state is set on the kid widget that offers it (siblings cleared), and the Radio/Pushbutton flag bits are read per ISO 32000-1

## 4.1.0-dev.0

- Added `scan-dirs` — extra directories for `keep: auto` to scan beyond `lib/` and `bin/` (`scan-dirs: [tools, packages/shared/lib]`, relative to your app); a directory that is not there keeps the full binary rather than scanning less
- Verify your app after upgrading: `keep: auto` now actually trims the native engine; if something is missing, state it with `keep: [render, …]`
- Fixed `setup` ignoring your config when run from a subfolder of your project — both lanes now locate your app from its own `.dart_tool/`, not from the working directory
- Fixed `keep: auto` keeping every capability on native — the build hook scanned its own package instead of your app; it now derives your app from the hook's output path (web was never affected)
- Fixed the native keep decision never re-evaluating — your source is now a hook dependency, so adding a `sign()` call recompiles the engine with signatures

## 4.0.0-dev.0

- **Breaking:** the engine is configured with three flat keys under `hooks: user_defines: pdf_manipulator:` — `keep` (`auto`, `[render, …]`, or omit for everything), `detector` (only with `keep: auto`) and `build`. Migrate: `trim: auto` → `keep: auto`, `trim: {keep: […]}` → `keep: […]`, `trim-detector: X` → `detector: X`, drop `setup --trim` and re-run `flutter pub run pdf_manipulator:setup`; an invalid config fails the build with a clear message
- Added `build:` in the same block — `speed` (default, prebuilt), `size` (smaller, opt-level z) or `debug` (symbols for native crash traces); `size` and `debug` compile from source and need [Rust](https://rustup.rs)
- Changed the prebuilt-binary download to retry and resume over HTTP Range, so one network blip on a large asset no longer forces a from-source compile
- Fixed a cryptic `feature edition2024 is required` cargo error on older Rust toolchains — the build checks the required Rust version first and names the `rustup` command to run ([#183](https://github.com/whuppi/pdf_manipulator/issues/183) reported by [@mrhazelh](https://github.com/mrhazelh))

## 3.0.0-dev.1

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Changed the web build to compile wasm-bindgen and wasm-opt inside the engine's cargo workspace — `wasm-bindgen-cli`, `binaryen` and `jq` are no longer needed, [Rust](https://rustup.rs) is the only requirement ([#177](https://github.com/whuppi/pdf_manipulator/issues/177) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed `trim: auto` counting member names inside comments as usage — the scan ignores comments and prints where each kept member matched, e.g. `render (lib/preview.dart:12)` ([#175](https://github.com/whuppi/pdf_manipulator/issues/175) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed source builds failing with "rustup: command not found" on Rust installs not managed by rustup — the build asks rustc which targets are installed and uses rustup only as the fallback ([#176](https://github.com/whuppi/pdf_manipulator/issues/176) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))

## 3.0.0-dev.0

- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (4.4 MB on every install) → register one: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji); without one the value is still saved, only the baked-in look falls back to the field's font
- **Breaking** (only if you took the retracted 2.2.0): `trim-detector: analyzer` is now `scan` → change it or delete the line, it is the default
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses: `trim: auto` under `hooks: user_defines: pdf_manipulator:`, or `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`; needs [Rust](https://rustup.rs), compiled once and cached ([#167](https://github.com/whuppi/pdf_manipulator/issues/167))
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`
- Changed the default binary — dead engine surfaces, barcode support and the embedded fonts are gone: native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB; no action needed
- Fixed the package forcing an old `analyzer` version onto your app — the dependency is gone; `trim: auto` uses a dependency-free scan that can only keep slightly more than you use, never less ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the package archive missing the engine's `Cargo.lock`, which broke `trim` and every compile-from-source path ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the web compile error telling you to install `wasm-pack` — it now names the tool it actually misses, with the install command

## 2.2.0-dev.0

> Retracted on pub.dev: the published archive was missing a build file (the engine's `Cargo.lock`), breaking `trim` and compile-from-source installs. Superseded by 3.0.0, which consolidates everything here.


- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (4.4 MB on every install) → register one: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji); without one the value is still saved, only the baked-in look falls back to the field's font
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses: `trim: auto` under `hooks: user_defines: pdf_manipulator:`, or `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`; needs [Rust](https://rustup.rs), compiled once and cached ([#167](https://github.com/whuppi/pdf_manipulator/issues/167))
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`
- Changed the default binary — dead engine surfaces, barcode support and the embedded fonts are gone: native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB; no action needed

## 2.1.4-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `flattenForms` dropping a field's value when fill and flatten happen in separate editor sessions — the appearance is regenerated from the persisted `/V` ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed `addImageStamp` erasing a page's form widgets when `/Annots` is an indirect reference ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed a reopened filled form rendering blank where its value should appear — the renderer regenerates a widget's appearance from `/V` under `/NeedAppearances`, matching the flattener

## 2.1.3-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `setFormFieldValue` reporting field-not-found on forms whose field names are raw UTF-8 (LibreOffice-class producers) — one spec-tolerant text-string decoder covers UTF-16, UTF-8 and PDFDocEncoding ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed `flattenForms` baking mojibake for values outside ASCII (`ß` → `ÃŸ`) — values decode per ISO 32000-1 §7.9.2.2 and appearance text is written in WinAnsi ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed fill → flatten silently dropping the value on widgets without an appearance stream when the field name is non-ASCII — the flattener and the form extractor decode names the same way ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed flattening CJK and emoji values drawing nothing — the bundled fallback font ships in native and web builds ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))
- Fixed document metadata (`getTitle` and friends) mangling non-ASCII on read, and writes now encode per spec ([#155](https://github.com/whuppi/pdf_manipulator/issues/155))

## 2.1.2-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a Flutter Web WASM (`dart2wasm`) compile failure — a switch over `Object?` that dart2js accepted was non-exhaustive under dart2wasm ([#145](https://github.com/whuppi/pdf_manipulator/issues/145) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed pub.dev not advertising Flutter Web support — the web runtime resolves to a stub default that `pana` can analyze

## 2.1.1-dev.0

- Fixed the README banner not rendering on pub.dev — the `<picture>` element is flattened to a plain image in the published package

## 2.1.0-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added document producer and creation-date metadata — `PdfEditor.setProducer()` / `getProducer()` and `setCreationDate()` / `getCreationDate()` (raw PDF date strings, e.g. `D:20240101120000Z`), plus `PdfDoc.producer`, `PdfDoc.creator`, and `PdfDoc.creationDate` read on open

## 2.0.2-dev.0

- Fixed `addImageStamp` rendering a transparent-background PNG as a solid black box — the alpha channel ships as a grayscale `/SMask` ([#103](https://github.com/whuppi/pdf_manipulator/issues/103) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack))
- Fixed the `RenderedPage.data` doc — `render()` returns PNG-encoded bytes (decode to read pixels), not raw RGBA

## 2.0.1-dev.0

Docs-only — no code or API changes.

- README and every guide (architecture, capabilities, migration, updating, contributing) rewritten, restructured, and verified against the source

## 2.0.0-dev.0

The concurrency rewrite. Every operation now runs fully isolated on its
own *lane* — a dedicated Rust thread (native) or Web Worker (web).

- **Breaking:** `webCoordinatorUrl` + `webWorkerUrl` → one `webLaneWorkerUrl`
- **Breaking:** web now ships `lane_worker.js` (was `coordinator.js` + `worker.js`)
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Every method returns `PdfTask<T>` — a `Future` plus `cancel()`; cancelling kills just that job
- `pdf.dispose()` is instant — no joins, no timeouts, no leaks; in-flight ops resolve with `PdfCancelled`
- Operations on different handles now run truly parallel on native (1.x serialized them)
- Lane budgets never error — past the cap, work queues instead of failing
- Added `PdfConfig.maxLanes` — concurrent lanes per instance (default: half the cores, min 2)
- All three web modes (JSPI, Atomics, OPFS) behave identically; a bad worker/WASM URL now fails instantly with a typed error instead of hanging
- Fixed flattening — translated appearances no longer land off-page, unfilled fields render their default value, stamps render their label
- Fixed missing WASM binaries in the 1.0.6 release
- Fixed Android 16 KB page-size alignment for Google Play API 35+ ([PR whuppi/pdf_oxide#1](https://github.com/whuppi/pdf_oxide/pull/1), [@Binary-Parse](https://github.com/Binary-Parse))

## 1.0.6-dev.0

- Fixed release build routing for consumer builds (by [@Binary-Parse](https://github.com/Binary-Parse))
- Fixed Windows NDK linker `.cmd` extension for Android cross-compilation (by [@Binary-Parse](https://github.com/Binary-Parse))
- Added CI verify tests — release builds now verified on all 6 targets (Android, iOS, macOS, Linux, Windows, Web)

## 1.0.5-dev.0

- Fixed README version not stamped on pub.dev
- Updated tracking links for web build hook support

## 1.0.4-dev.0

- Web setup now verifies all assets against release hashes — detects stale files automatically
- `setup` supports `--force` to re-download everything, `--native` to pre-fetch the native binary

## 1.0.3-dev.0

- Fixed changelog on pub.dev

## 1.0.2-dev.0

- Fixed changelog on pub.dev missing commit history between versions

## 1.0.1-dev.0

- Added public API doc comments across all exported classes and methods
- Minimum Android API corrected from 35 to 21 (Android 5.0)
- Setup command is now `flutter pub run pdf_manipulator:setup` (avoids triggering native build hooks with `dart run`)

## 1.0.0-dev.0

Complete ground-up rewrite — new Rust engine, new instance API, cross-platform (previously Android only). The package docs carry the migration guide and the full capability list.

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the Android-only backend
- **Targets:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** instance-based `Pdf()` with `dispose()`; batch editing via `pdf.edit(source)`, create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`, same code on every target
- **Errors:** typed `PdfError` sealed class — no more `PlatformException`
- **Performance:** every operation off the main thread, no full-file buffers
- **SDK:** requires Dart >=3.10.0
