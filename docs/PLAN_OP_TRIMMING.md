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

### Dual-detector decision (2026-07-16) — rationale

The detector selector and per-detector behavior are canonical in
`ARCHITECTURE.md` §Trim. The decision recorded here: BOTH lanes ship —
analyzer as the stable default, RecordUse tagged EXPERIMENTAL and riding
along — so the RecordUse integration stays built, in sync, and testable as
the SDK matures, instead of being re-invented the day it stabilizes. Our
instance-method API is correct Dart design and does NOT get reworked for
RecordUse's current statics-only limit (the shim absorbs it).

### R2 — detector productionization — SHIPPED
`lib/src/trim/detector.dart` — resolved-AST call-finder over the app's
lib/. Fail closed: any unresolvable file → full binary + warning.
Proven end-to-end on example/ (keep=[pdfa, render, signatures], every
expected member matched incl. sugar).

### The `trim` API — design rationale (2026-07-16)

The grammar itself is canonical in `ARCHITECTURE.md` §Trim (spec) and the
README (consumer form) — not restated here. What lives here is WHY it has
that shape (the gold rules):

1. **One key, one artifact.** Auto and manual both produce the same internal
   thing — a KeepSet of capabilities — feeding one build pipeline. Manual IS
   the detector override; there is no second mechanism.
2. **Users speak capabilities, never cargo features.** The public vocabulary
   names what the app does, stable across engine bumps; the
   capability→feature map is internal. Engine internals
   (native-bridge/icc/...) are not expressible — not droppable, not a
   foot-gun.
3. **Keep-list, not drop-list.** Trim's contract is "ship only what I say";
   an allowlist states it exactly and mirrors the detector's output shape.
   Forgetting a capability fails safe: the typed "not enabled in this build"
   error names exactly what to add to `keep:`.
4. **Core is always included.** The keep-list only names optional heavy
   modules.
5. **Config errors are loud.** A typo'd capability or malformed value fails
   the build with the grammar — never a silent full binary (that would be a
   lie about what was requested).
6. **One source of truth for both platforms.** The pubspec entry drives the
   native hook and web setup identically.

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
  `record-use` (EXPERIMENTAL) is a FULL drive path following the official
  link-hook pattern (the font-subsetting shape): the build hook ships the
  full binary to the link phase; `hook/link.dart` reads
  `input.recordedUses`, computes the keep-set via `recordedCapabilities`,
  compiles the trimmed engine through the SHARED compiler, and emits it
  in place of the full one. Recordings absent (SDK experiment off) → the
  full binary ships, loudly (fail closed). Debug builds skip linking →
  full binary by design (fast iteration, release gets the trim). The lane
  becomes live the day the SDK starts recording — zero changes needed
  here. `compare` trims with the analyzer and reports the recorded set
  for diffing. The shim: `TrimRecord.op('<capability>')` const calls in
  every capability-bearing public op (`record_use_shim.dart`) — deleted
  when dart-lang/native#2902 lands instance-method support. Wasm cannot
  be record-use-driven until web reaches hooks (in progress upstream);
  the analyzer covers web regardless.
- **One compile path, two callers**: the hook orchestration lives in
  `lib/src/hook/` — `build_constants.dart` (build.json), `engine_compiler.dart`
  (CodeConfig→triple/key/linkmode mappers, NDK env, the cargo invocation),
  `trim_plan.dart` (user-defines → TrimPlan; Recordings → keep-set).
  `hook/build.dart` and `hook/link.dart` are thin callers. Change compile
  behavior in the shared module, never in a hook.
  Tests: `test/trim/trim_plan_test.dart` proves the plan matrix (defer /
  manual / fail-closed / loud grammar) and the recordings extraction
  against an in-memory `Recordings` fixture — the drive path is testable
  today without the SDK experiment.

### R4 — shake verifier — SHIPPED
`make shake-audit` (`tool/shake_audit.sh`): full vs core-only release
builds, nm symbol autopsy (banned public-api C exports absent, lane
bridge exports present), size assertions (core ≥2 MB smaller than full,
13 MB ceiling), and the typed not-enabled runtime probes (Rust unit
tests in `host/dispatch.rs`, cfg-gated to fire only on trimmed builds).
Measured ledger (core-only = every capability feature off):
full 21,109,840 → core 11,804,832 (capabilities) → 9,287,920 (+office
gate) → 8,398,880 (+extract CID tables + search) → 6,314,544 (+extract
root gates) — trim deletes 14.8 MB, 70% of the full native library. `SHAKE_AUDIT_WASM=1` adds the wasm size check. Never edit the
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

## The op-unit scaffold (built 2026-07-17)

Every bridge op is now a UNIT — `vendor/pdf_oxide/src/host/ops/` — instead
of an arm in one match: a registry entry + a handler with one shared
calling convention (`OpCtx`) + an exported linker anchor
(`pdf_op_<name>_anchor`, inert today). Family files group along
capability lines (pdfa / signatures / render / convert vs core
doc / editor / builder / fonts). `registry.rs` is the ONE swappable
backend: an explicit table today, linker-driven collection later. The op
still TRAVELS as data through the single door — required (worker/thread
crossing, replayable preludes, 3-export ABI); the unit layer carries
reachability, the door carries bytes.

Why this shape (researched against how the futures are actually landing):

- **Dart static linking** ([dart-lang/sdk#49418], open exploration,
  unassigned): the sketched design is Dart AOT emitting relocatable
  objects with per-SYMBOL relocations, native static libs, ONE native
  link, `asset` tags disambiguating symbols. Per-op anchor symbols are
  exactly the referents that world needs (the landing steps are tracked
  in `CAPABILITY_ROADMAP.md` §When the futures arrive).
- **Wasm component model** (1.0 in the cloud ecosystem; dart2wasm has
  only proposal issue dart-lang/sdk#56366): composition is typed
  interface functions — units map one-to-one. Browser-side is the
  furthest future; the analyzer detector covers web regardless.
- **linkme / link-section crates don't support wasm32** — hence the
  explicit table as today's backend, not linker magic that would fork
  per-platform.
- **Today's payoff**: per-op cargo features (the "not fat" trim beyond
  capabilities) are now one cfg per registry row + unit, instead of
  surgery on a 175-line match. Byte autopsy (`cargo bloat`) decides
  which ops earn a feature.

Fleet note: this is the standard-setter shape for future packages —
data through one door, reachability through per-unit symbols, registry
backend swappable, futures made cheap rather than pre-built.

## The core byte autopsy (2026-07-17) — where 9.29 MB lives, and the verdicts

Method that survived scrutiny: macOS linker map (`-Wl,-map`) on the exact
release build — cargo-bloat's numbers were untrustworthy (its own build,
strip disabled) and nm address-deltas fabricated a "925 KB KANGXI table"
(real size: 19 KB source). Trust only per-symbol sizes from the map.

Core (icc,legacy-crypto,native-bridge; 9,290,592 stripped — the op-unit
layer costs +2.6 KB total):

| Bucket | ~Size | Notes |
|---|---|---|
| extraction brain (code) | 1.7 MB | spatial_table_detector 295K · layout::text_block 232K · reading_order 190K · extractors::text 129K · pipeline::converters 118K · document.rs extraction share |
| CJK CID→Unicode tables (const) | 940 KB | `fonts/cid_mappings/` (adobe gb1/cns1/korea1/japan1) — predefined CMaps for CJK text extraction |
| regex stack | 450 KB | pulled ~entirely by extraction (markdown/citations/whitespace/search) |
| always-core (parser, writers, forms+appearances, fonts machinery, crypto, codecs) | ~3.4 MB | the six promises; PDF is a monster spec (MuPDF ~8 MB, pdfium ~10 MB) |
| unwind/exception metadata | ~1.0 MB | scales with code |
| Rust runtime | ~0.7 MB | fixed |

### Verdicts

- **`extract` capability — APPROVED (2026-07-17), executing as dominoes.**
  ~2.6-2.9 MB, 30% of core. Survey verdicts: `layout/` is a chasm (20
  consumers incl. the writer core — shared geometry/span types must stay;
  only the algorithms gate); `structure/` splits (tagged-tree machinery
  serves pdfa/writing; only spatial_table_detector + table_extractor are
  extract-only); writer needs just two enums from extractors::text
  (ArtifactType, PaginationSubtype — carve out); planSplitByBookmarks is
  outline-only → STAYS CORE (do not map it to the capability).

  The domino queue (each lands green + committed before the next):
  1. DONE — cid_mappings CJK tables (~0.9 MB): lookups return None when
     off; callers' fallback chains degrade gracefully. office/public-api/
     python pull extract.
  2a. DONE — search: module + Pdf::search*/highlight_matches API +
     dispatch typed error + trim probe. rust_bench bin requires extract.
  2b+3. DONE — superseded by ROOT GATES. The planned module-gating
     cascade through document.rs proved unnecessary: because the shipped
     artifact is a cdylib, only exported symbols are roots — gating just
     THREE dispatch fns (extract_text, classify_page, classify_document)
     let LTO delete the entire extraction web (text_block algorithms,
     reading_order, pipeline, spatial tables, extract-format converters,
     extractors::text): 8,398,880 → 6,314,544 (−2.08 MB) with ZERO
     markered patches in document.rs. Module gates are only needed where
     core references PIN data (the CID tables via font_dict — domino 1)
     or where an optional dep must not compile (office_oxide). This is
     the "cut roots, let LTO shake" thesis — module surgery is the
     exception, not the method. (form_xobject_finder is office-only —
     minor drift, fold into any future converters touch-up.)
  4. DONE — public vocabulary: extract capability (members PdfDoc.extract/
     search/classifyPage/classifyDocument), build.json defaults +=
     extract, core promise redefined (parse/write/edit/forms/builder),
     grammar + README + ARCHITECTURE updated, shake-audit ceiling 8 MB,
     four trim probes.
- **panic=abort — CLOSED.** Native lane isolation IS `catch_unwind`
  (host/native/lane.rs: one bad PDF → typed "operation panicked" error,
  engine survives). abort would turn any engine panic into a whole-app
  crash — violates the broken-app-is-impossible law. Wasm already ships
  panic=abort via `release-small` (the JS worker boundary provides the
  isolation there). Nothing left to win.
- **writer monomorphization dedup — CLOSED.** Measured, not eyeballed:
  the honest dedupable waste is the write_full/finish/assemble pairs
  (Boxed vs Seek writer), ~150-200 KB (2% of core) — the rest of the
  441 KB "duplication" is distinct generic instantiations, not waste.
  Price: dyn dispatch in the hottest safety-critical write loop +
  invasive upstream surgery. Bad trade at 2%.
- **opt-level=z** — possible ~15-20% of text at CPU cost on every op;
  wrong default for a PDF engine. Could become a trim option only if
  users ask.

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
