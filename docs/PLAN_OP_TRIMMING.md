# Plan — Ship only the ops the app calls (#167)

> Working doc for [#167](https://github.com/whuppi/pdf_manipulator/issues/167).
> Not canonical; promote pieces into CAPABILITY_ROADMAP.md as they land.
> All work LOCAL (no pushes) on `feat/op-trimming` + the fork patch branch.

## The product shape (decided 2026-07-16, after design review)

```
default        →  prebuilt binary from GitHub Releases — already trimmed of
                  everything no consumer can ever call (Stages 1-3 below)
trim: true     →  the DETECTOR reads the app's source, finds which ops it
                  can reach, and rebuilds the engine with only those
```

One public knob. No per-feature booleans in the public API (they exist only
as internal plumbing feeding the same build path; a documented
`trim: {without: [...]}` advanced form ships later ONLY if real users hit
over-keeping and ask).

**One detector: the analyzer call-finder, on every platform.** The
`@RecordUse` alternative is dead — native-only, release-only, experimental,
and redundant once one mechanism covers all platforms. `package:analyzer`
is tooling-only (never imported by `lib/`), so it adds download time, zero
app bytes.

**Why this is true tree shaking.** Same contract as Dart's own shaker:
anything not PROVABLY unused is kept. The detector resolves the app's real
call graph against this package's API; any file it cannot resolve → full
binary (fail closed). The only possible failure direction is a
bigger-than-optimal binary — a broken app is not an outcome the design
permits. The typed "not enabled in this build" engine error is
defense-in-depth against our own bugs, not an expected path. We build the
bridge because Dart's shaker cannot see through FFI (dart-lang/sdk#52970):
we run conservative reachability at the source level, and Rust's
LTO/wasm-gc does the actual deletion.

## Measured ledger (defaults everyone gets, no trim needed)

| Milestone | wasm raw | wire (gz) | native dylib |
|---|---|---|---|
| original baseline | 25.80 MB | 11.28 MB | 28.66 MB |
| Stage 1 — dead APIs amputated | 22.58 | 9.93 | 25.60 |
| Stage 2 — fonts → runtime registry | 18.14 | 7.49 | 21.11 |
| Stage 3 — barcodes off, pdfa feature'd | **17.21** | **7.17 (−36%)** | 21.11 (unchanged, pdfa stays on) |

## Shipped so far (all local commits)

- **Stage 1**: `public-api` feature gates upstream's dead-to-us surfaces
  (289 wasm exports, ~420 C exports). Stable `lane_alloc`/`lane_dealloc`
  ABI fixed a pre-existing JSPI fragility (worker called renumbered
  `__wbindgen_export_N` internals). Native −3.06 MB, wasm −3.21 MB raw.
- **Stage 2**: fallback fonts out of the binary, feature intact.
  `Pdf.registerFallbackFont` → router prelude (replays to every current +
  future lane) → engine registry → form baking. Proven end-to-end native +
  web incl. graceful no-font degradation. −4.4 MB raw.
- **Stage 3 (in working tree, gates green except one env-flaky web run)**:
  - `wasm` feature decoupled from `signatures`+`barcodes` (coupling moved
    to `public-api`, so upstream's JS API still builds when enabled).
    Barcodes ship OFF (zero Dart ops use them): −0.9 MB raw.
  - `pdfa` feature gates the compliance module (Liberation family + sRGB
    live there). ON by default — behavior identical; droppable by trim.
    Typed "PDF/A support not enabled in this build" from the three
    dispatch fns when off (mirrors upstream's rendering idiom).
  - Build plumbing (INTERNAL — the path trim mode drives): user_defines →
    effective feature set in `hook/build.dart`; custom sets skip the
    prebuilt download + pinned hash (version-0.0.0 pathway) and compile
    locally, cargo's feature-aware fingerprint as the cache;
    `PDF_FEATURES_NATIVE/WASM` env overrides in `compile_rust.sh`;
    `resolveWeb(wasmFeaturesOverride:)` + `_compileWasm` env passthrough.
  - `test-rust` runs with `public-api,cjk-form-fonts,pdfa` so upstream
    suites keep covering everything the defaults exclude.

## Remaining work

*(all R-items below are SHIPPED as of 2026-07-16 — statuses inline)*

### R1 — capability→feature map — SHIPPED
`lib/src/trim/capabilities.dart`: `PdfCapability` (render / signatures /
pdfa / office) with `apiMembers` (the detector's member table) and
`featuresFor` (keep-set → cargo features). Grammar + mapping pinned by
`test/trim/capabilities_test.dart` (13 tests).

### Dual-detector decision (2026-07-16)

Both detectors ship behind one selector; public `trim:` grammar unchanged:
`trim-detector: analyzer` (default, stable, all platforms) |
`record-use` (EXPERIMENTAL, native+release only, loud failure elsewhere;
implemented via an internal static `useOp('<op>')` shim inside each public
op method — zero public API change, deleted when dart-lang/native#2902
ships instance-method support) | `compare` (trims with analyzer, runs
both, prints the keep-set diff — the living testbed for RecordUse
maturity). Our instance-method API is correct Dart design and does NOT
get reworked for RecordUse's current statics-only limit.

### R2 — detector productionization — SHIPPED
`lib/src/trim/detector.dart` — resolved-AST call-finder over the app's
lib/. Fail closed: any unresolvable file → full binary + warning.
Proven end-to-end on example/ (keep=[pdfa, render, signatures], every
expected member matched incl. sugar).

### The `trim` API (designed 2026-07-16 — auto AND manual, one key)

```yaml
hooks:
  user_defines:
    pdf_manipulator:
      trim: auto                       # detector reads the app, keeps what it calls
      # or full manual override — EXACTLY these capabilities (plus core):
      trim:
        keep: [render, signatures]
      # absent → full default binary
```

Semantics:

| Value | Meaning |
|---|---|
| absent / `false` | full default binary (prebuilt download) |
| `auto` (canonical; `true` accepted as alias) | detector-computed keep-set |
| `{keep: [<capability>...]}` | user-supplied keep-set — the full override |
| anything else / unknown capability name | **BUILD ERROR** printing the valid grammar — config mistakes never silently produce a fallback |

Design invariants (the gold rules):

1. **One key, one artifact.** Auto and manual both produce the same internal
   thing — a KeepSet of capabilities — feeding one build pipeline. Manual IS
   the detector override; there is no second mechanism.
2. **Users speak capabilities, never cargo features.** The public vocabulary
   names what the app does (`render`, `signatures`, `pdfa`, later `office`),
   stable across engine bumps; the capability→feature map (R1) is internal.
   Internals like `native-bridge`/`icc` are not expressible — not droppable,
   not a foot-gun.
3. **Keep-list, not drop-list.** Trim's contract is "ship only what I say";
   an allowlist states it exactly and mirrors the detector's output shape.
   Forgetting a capability fails safe: the typed "not enabled in this build"
   error names exactly what to add to `keep:`.
4. **Core is always included** (parse/write/edit/forms/extract/builder — the
   engine's muscle). The keep-list only names the optional heavy modules.
5. **Config errors are loud.** A typo'd capability or malformed value fails
   the build with the grammar — never a silent full binary (that would be a
   lie about what was requested).
6. **One source of truth for both platforms.** The pubspec entry drives the
   native hook (`input.userDefines['trim']`) and web `setup` (which parses
   the app's pubspec from cwd) identically.

### R3 — trim wiring — SHIPPED
- **Web**: `setup --trim` runs the detector over the app cwd → wasm
  feature set → `resolveWeb(wasmFeaturesOverride:)` → trimmed wasm
  compiled locally (E2E verified: 3 files installed).
- **Native**: `hook/build.dart` parses the `trim:` user-define via
  `TrimConfig.parse` (loud `TrimConfigError` fails the build). Manual
  `keep:` works unconditionally; `auto` uses the option-(a) route with a
  cwd heuristic (`Directory.current` has a pubspec) because hooks 2.0.2's
  `BuildInput` exposes NO app root — heuristic miss → FULL binary +
  warning (fail closed). Custom feature sets skip the prebuilt download
  and compile locally (version-0.0.0 pathway, cargo fingerprint = cache).
- **Detector selector**: `trim-detector: analyzer | record-use | compare`.
  `record-use` is EXPERIMENTAL and cannot drive the build (its data
  appears in the link phase, after the native compile) — selecting it
  fails loudly with that explanation. `compare` trims with the analyzer
  and the link hook (`hook/link.dart`, via `package:record_use` +
  `input.recordedUses`) prints the RecordUse-observed capability set for
  diffing. The shim: `TrimRecord.op('<capability>')` const calls in every
  capability-bearing public op (`record_use_shim.dart`) — deleted when
  dart-lang/native#2902 lands instance-method support.

### R4 — shake verifier — SHIPPED
`make shake-audit` (`tool/shake_audit.sh`): full vs core-only release
builds, nm symbol autopsy (banned public-api C exports absent, lane
bridge exports present), size assertions (core ≥2 MB smaller than full,
13 MB ceiling), and the typed not-enabled runtime probes (Rust unit
tests in `host/dispatch.rs`, cfg-gated to fire only on trimmed builds).
Measured (office feature in place): full 21,109,840 → core-only
9,287,920 — trim deletes 11.8 MB, 56% of the full native library
(pre-office-split core was 11,804,832; gating office shaved another
2.5 MB). `SHAKE_AUDIT_WASM=1` adds the wasm size check. Never edit the
script while an audit is running — bash re-reads shifted bytes.

### R5 — office/converters gating — SHIPPED
The entanglement turned out clean: the 9 cross-tree consumers only use
`ConversionOptions` + enums (lightweight, stays); the heavy office code
is `converters/office/`, the three `*_layout.rs`, `pdf_to_ir`, and one
contiguous `to_docx/pptx/xlsx` region in document.rs (now its own
`#[cfg(feature = "office")] impl` block). `office` cargo feature gates
`office_oxide` (made optional); `public-api` and `python` pull it so
upstream surfaces keep compiling. Dispatch convert fns answer the typed
not-enabled error when off. Dart: `office` capability mapped from
`convertTo`/`convertToPdf` (their `PdfDocumentFormat` is docx/pptx/xlsx
only — purely office ops, no argument-awareness needed). Defaults KEEP
office (behavior unchanged); trim drops it.

### R6 — consumer docs — README SHIPPED; release items open
README: "CJK & emoji in form fields" (registerFallbackFont) + "Ship only
what you use (trim)" sections. STILL OPEN at release time: changelog cut
(Engine updated bullet REQUIRED — submodule pointer changed),
CAPABILITY_ROADMAP promotion, companion CJK asset package decision.

## Verification status

Stage 3: test-rust PASS · analyze PASS · test-ops-native PASS · the two
web flakes (atomics stress OOM, jspi 2s init timeout) both passed solo
reruns — environmental. R1–R5: trim tests 13/13 · shake-audit PASS
(post-office: full 21.1 MB → core 9.29 MB, both runtime probes green) ·
test-rust PASS (both crates) · test-ops-native 234/234 · cargo check
green on
native core / native full+public-api / wasm lib with and without office
(the 2 unused-import wasm warnings in document_editor.rs are
pre-existing, present with office on and off) · make analyze PASS.

## Scratch

`/tmp/treeshake_rnd/` — all experiment artifacts, build logs, results.
Detector prototype: `/private/tmp/treeshake_rnd/detector/`.
