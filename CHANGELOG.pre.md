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
  - Fixed <bug> ([#N](issue-url) reported by [@user](abs-url), [PR #N](abs-url))  ← fixes last

  Order IS the grouping — Breaking → action → added/changed → fixed. No
  `###` subsections: bullet order carries the categories. Only Breaking
  is bold-tagged; everything else is verb-led. Fixes start with "Fixed".

  EXCEPTION — the genesis entry (a ground-up rewrite, no prior version)
  uses facet tags instead of deltas: **Engine:** / **API:** / **I/O:** /
  etc., describing the new package's dimensions. See the 1.0.0 entry.

CONTENT RULES (never change)
  • Migrate from the entry ALONE — breaking changes inline, old → new.
    (pub.dev freezes each version's CHANGELOG as a snapshot, so an entry
    can't rely on anything that later moves.)
  • NEVER link a living doc (README, docs/*) from an entry — it rots when
    the doc moves on. The migration guide is reached from the README.
  • Links point only at IMMUTABLE targets — a PR, commit, or issue:
    ([#N](https://github.com/whuppi/pdf_manipulator/issues/N) reported by
    [@user](https://github.com/user), [PR #N](https://github.com/whuppi/pdf_manipulator/pull/N)).
    Credit the issue + reporter when a reported issue drove the fix; the PR
    (or commit) link alone otherwise.
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

## 3.0.1-dev.0

- Changed the prebuilt-binary download to retry transient failures and resume interrupted transfers over HTTP Range, so a flaky network on a large asset (e.g. the ~180 MB iOS static library) no longer forces a slow from-source compile on a single blip ([PR #185](https://github.com/whuppi/pdf_manipulator/pull/185))
- Fixed a cryptic `failed to load manifest ... feature edition2024 is required` cargo error when the engine compiles from source (iOS device builds, git dependencies, or any download-miss) on an older Rust toolchain — the build now checks the required Rust version first and stops with a clear message naming the exact version and the `rustup` command to install it ([#183](https://github.com/whuppi/pdf_manipulator/issues/183) reported by [@mrhazelh](https://github.com/mrhazelh), [PR #185](https://github.com/whuppi/pdf_manipulator/pull/185))

## 3.0.0-dev.1

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Changed the web build to compile its own wasm-bindgen + wasm-opt inside the engine's cargo workspace, version-locked by the engine's `Cargo.lock` — `wasm-bindgen-cli`, `binaryen`, and `jq` are no longer needed, [Rust](https://rustup.rs) is the only requirement on every platform, and the exact-version rejection is gone with the tools ([#177](https://github.com/whuppi/pdf_manipulator/issues/177) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #178](https://github.com/whuppi/pdf_manipulator/pull/178))
- Fixed `trim: auto` counting member names inside comments as usage — a doc line saying "render them" kept the render capability. The scan now ignores comments, and prints where each kept member was matched, e.g. `render (lib/preview.dart:12)` ([#175](https://github.com/whuppi/pdf_manipulator/issues/175) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #178](https://github.com/whuppi/pdf_manipulator/pull/178))
- Fixed source builds failing with "rustup: command not found" on Rust installs not managed by rustup (Homebrew, distro packages) — the build now asks rustc itself whether a target is installed and uses rustup only as the fallback ([#176](https://github.com/whuppi/pdf_manipulator/issues/176) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #178](https://github.com/whuppi/pdf_manipulator/pull/178))

## 3.0.0-dev.0

- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (it added 4.4 MB to every install). Register one once: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji). Without one the value is still saved correctly — only the baked-in look falls back to the field's own font.
- **Breaking** (only if you took the retracted 2.2.0): the `trim-detector` value `analyzer` is now `scan` → change it or delete the line — it is the default ([PR #172](https://github.com/whuppi/pdf_manipulator/pull/172))
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses ([#167](https://github.com/whuppi/pdf_manipulator/issues/167)). Put `trim: auto` under `hooks: user_defines: pdf_manipulator:` in your app pubspec, or choose yourself with `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`. Needs [Rust](https://rustup.rs) — the engine compiles once on your machine and is cached. Wrong or missing pieces fail with a clear message, never a broken app.
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`.
- Changed the default binary: dead engine surfaces, unused barcode support, and the embedded fonts are gone — native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB. No action needed.
- Fixed the package forcing an old `analyzer` version onto your app, which blocked current freezed / json_serializable and friends. The `analyzer` dependency is gone: `trim: auto` uses a dependency-free source scan. It can only keep slightly more than you use, never less — state `trim: {keep: [...]}` yourself for the exact minimum ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #172](https://github.com/whuppi/pdf_manipulator/pull/172))
- Fixed the package archive missing a build file (the engine's `Cargo.lock`), which broke `trim` and every compile-from-source path with "No such file or directory" ([#171](https://github.com/whuppi/pdf_manipulator/issues/171) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #172](https://github.com/whuppi/pdf_manipulator/pull/172))
- Fixed the web compile error message telling you to install `wasm-pack` — it is not used. The build now points at the tool it actually misses, with the exact install command ([PR #172](https://github.com/whuppi/pdf_manipulator/pull/172))

## 2.2.0-dev.0

> Retracted on pub.dev: the published archive was missing a build file (the engine's `Cargo.lock`), breaking `trim` and compile-from-source installs. Superseded by 3.0.0, which consolidates everything here.


- **Breaking:** flattening CJK or emoji form values no longer uses a bundled font (it added 4.4 MB to every install). Register one once: `await pdf.registerFallbackFont(PdfFallbackFontKind.cjk, fontBytes)` (`.emoji` for emoji). Without one the value is still saved correctly — only the baked-in look falls back to the field's own font.
- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added trim — keep only the features your app uses ([#167](https://github.com/whuppi/pdf_manipulator/issues/167)). Put `trim: auto` under `hooks: user_defines: pdf_manipulator:` in your app pubspec, or choose yourself with `trim: {keep: [render, signatures]}`; on web also run `flutter pub run pdf_manipulator:setup --trim`. Needs [Rust](https://rustup.rs) — the engine compiles once on your machine and is cached. Wrong or missing pieces fail with a clear message, never a broken app.
- Added `Pdf.registerFallbackFont(PdfFallbackFontKind, Uint8List)`.
- Changed the default binary: dead engine surfaces, unused barcode support, and the embedded fonts are gone — native 28.7 MB → 21.1 MB, gzipped web download 11.3 MB → 7.2 MB. No action needed.

## 2.1.4-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `flattenForms` dropping a field's value when the fill and the flatten happen in separate editor sessions — a reopened editor carries no in-session modified-field map, so the flattener now regenerates the appearance from the persisted `/V` (saved alongside `/NeedAppearances`) instead of baking the stale placeholder ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #162](https://github.com/whuppi/pdf_manipulator/pull/162))
- Fixed `addImageStamp` erasing a page's existing form widgets when the page references its annotations through an indirect `/Annots` reference rather than a direct array ([#161](https://github.com/whuppi/pdf_manipulator/issues/161) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #162](https://github.com/whuppi/pdf_manipulator/pull/162))
- Fixed a reopened filled form rendering blank where its value should appear — the renderer now regenerates a widget's appearance from `/V` when the AcroForm sets `/NeedAppearances`, matching the flattener ([PR #162](https://github.com/whuppi/pdf_manipulator/pull/162))

## 2.1.3-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed `setFormFieldValue` reporting field-not-found on forms whose field names are stored as raw UTF-8 (LibreOffice-class producers) — one spec-tolerant text-string decoder now handles UTF-16BE/LE, UTF-8 with and without BOM, and PDFDocEncoding across every read ([#155](https://github.com/whuppi/pdf_manipulator/issues/155), [PR #156](https://github.com/whuppi/pdf_manipulator/pull/156))
- Fixed `flattenForms` baking mojibake for values outside ASCII (`ß` → `ÃŸ` or `�`) — values decode per ISO 32000-1 §7.9.2.2 and appearance text is written one WinAnsi byte per character instead of UTF-8 ([#155](https://github.com/whuppi/pdf_manipulator/issues/155), [PR #156](https://github.com/whuppi/pdf_manipulator/pull/156))
- Fixed fill → flatten silently dropping the value on widgets without an appearance stream when the field name is non-ASCII — the flattener and the form extractor now agree on how names decode ([#155](https://github.com/whuppi/pdf_manipulator/issues/155), [PR #156](https://github.com/whuppi/pdf_manipulator/pull/156))
- Fixed flattening CJK and emoji values drawing nothing — the bundled fallback font now ships in both native and web builds and is embedded when the field's own font cannot render the text ([#155](https://github.com/whuppi/pdf_manipulator/issues/155), [PR #156](https://github.com/whuppi/pdf_manipulator/pull/156))
- Fixed document metadata (`getTitle` and friends) mangling non-ASCII on read, and metadata writes now encode per spec so other readers see the right text ([#155](https://github.com/whuppi/pdf_manipulator/issues/155), [PR #156](https://github.com/whuppi/pdf_manipulator/pull/156))

## 2.1.2-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Fixed a Flutter Web WASM (`dart2wasm`) compile failure — the `_post` switch over `Object?` was non-exhaustive under dart2wasm (dart2js treats `JSAny` as a catch-all, dart2wasm doesn't), now converted with `jsify()` ([#145](https://github.com/whuppi/pdf_manipulator/issues/145) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #146](https://github.com/whuppi/pdf_manipulator/pull/146))
- Fixed pub.dev not advertising Flutter Web support — the web runtime now resolves to a stub default that `pana` can analyze, so the package shows web (dart2js) support ([PR #133](https://github.com/whuppi/pdf_manipulator/pull/133))

## 2.1.1-dev.0

- Fixed the README banner not rendering on pub.dev — the `<picture>` element is flattened to a plain image in the published package ([PR #111](https://github.com/whuppi/pdf_manipulator/pull/111))

## 2.1.0-dev.0

- Engine updated — web: re-run `flutter pub run pdf_manipulator:setup --force web` (native updates itself)
- Added document producer and creation-date metadata — `PdfEditor.setProducer()` / `getProducer()` and `setCreationDate()` / `getCreationDate()` (raw PDF date strings, e.g. `D:20240101120000Z`), plus `PdfDoc.producer`, `PdfDoc.creator`, and `PdfDoc.creationDate` read on open

## 2.0.2-dev.0

- Fixed `addImageStamp` rendering a transparent-background PNG as a solid black box — the alpha channel now ships as a grayscale `/SMask` and the PNG predictor params are preserved, so transparent areas reveal the page instead of painting black ([#103](https://github.com/whuppi/pdf_manipulator/issues/103) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #104](https://github.com/whuppi/pdf_manipulator/pull/104))
- Fixed the `RenderedPage.data` doc — `render()` returns PNG-encoded bytes (decode to read pixels), not raw RGBA ([PR #104](https://github.com/whuppi/pdf_manipulator/pull/104))

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

- Fixed release build routing for consumer builds ([PR #80](https://github.com/whuppi/pdf_manipulator/pull/80), [@Binary-Parse](https://github.com/Binary-Parse))
- Fixed Windows NDK linker `.cmd` extension for Android cross-compilation ([PR #81](https://github.com/whuppi/pdf_manipulator/pull/81), [@Binary-Parse](https://github.com/Binary-Parse))
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
