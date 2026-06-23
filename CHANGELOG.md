# Changelog

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
═══════════════════════════════════════════════════════════════════════
-->

<!-- Add new versions below, newest first. -->

## 2.1.1

- Fixed the README banner not rendering on pub.dev — the `<picture>` element is flattened to a plain image in the published package ([PR #111](https://github.com/whuppi/pdf_manipulator/pull/111))

## 2.1.0

- Added document producer and creation-date metadata — `PdfEditor.setProducer()` / `getProducer()` and `setCreationDate()` / `getCreationDate()` (raw PDF date strings, e.g. `D:20240101120000Z`), plus `PdfDoc.producer`, `PdfDoc.creator`, and `PdfDoc.creationDate` read on open
- Fixed `addImageStamp` rendering a transparent-background PNG as a solid black box — the alpha channel now ships as a grayscale `/SMask` and the PNG predictor params are preserved, so transparent areas reveal the page instead of painting black ([#103](https://github.com/whuppi/pdf_manipulator/issues/103) reported by [@DarkWingMcQuack](https://github.com/DarkWingMcQuack), [PR #104](https://github.com/whuppi/pdf_manipulator/pull/104))
- Fixed the `RenderedPage.data` doc — `render()` returns PNG-encoded bytes (decode to read pixels), not raw RGBA ([PR #104](https://github.com/whuppi/pdf_manipulator/pull/104))

## 2.0.1

Docs-only — no code or API changes.

- README and every guide (architecture, capabilities, migration, updating, contributing) rewritten, restructured, and verified against the source

## 2.0.0

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

## 1.0.6

- Fixed release build routing for consumer builds ([PR #80](https://github.com/whuppi/pdf_manipulator/pull/80), [@Binary-Parse](https://github.com/Binary-Parse))
- Fixed Windows NDK linker `.cmd` extension for Android cross-compilation ([PR #81](https://github.com/whuppi/pdf_manipulator/pull/81), [@Binary-Parse](https://github.com/Binary-Parse))
- Added CI verify tests — release builds now verified on all 6 targets (Android, iOS, macOS, Linux, Windows, Web)

## 1.0.5

- Fixed README version not stamped on pub.dev
- Updated tracking links for web build hook support

## 1.0.4

- Web setup now verifies all assets against release hashes — detects stale files automatically
- `setup` supports `--force` to re-download everything, `--native` to pre-fetch the native binary

## 1.0.3

- Fixed changelog on pub.dev

## 1.0.2

- Fixed changelog on pub.dev missing commit history between versions

## 1.0.1

- Added public API doc comments across all exported classes and methods
- Minimum Android API corrected from 35 to 21 (Android 5.0)
- Setup command is now `flutter pub run pdf_manipulator:setup` (avoids triggering native build hooks with `dart run`)

## 1.0.0

Complete ground-up rewrite — new Rust engine, new instance API, cross-platform (previously Android only). The package docs carry the migration guide and the full capability list.

- **Engine:** pdf_oxide (Rust, MIT/Apache-2.0) replaces the Android-only backend
- **Targets:** iOS, Android, macOS, Windows, Linux, Web — previously Android only
- **API:** instance-based `Pdf()` with `dispose()`; batch editing via `pdf.edit(source)`, create from scratch via `pdf.build()`
- **I/O:** `DataSource` in, `DataSink` out — no file paths, no `dart:io`, same code on every target
- **Errors:** typed `PdfError` sealed class — no more `PlatformException`
- **Performance:** every operation off the main thread, no full-file buffers
- **SDK:** requires Dart >=3.10.0

## 0.5.9
<!-- release: no-tag -->

- The last release of Android-only version before the cross-platform rewrite.
