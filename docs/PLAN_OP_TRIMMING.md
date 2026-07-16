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

### R1 — op→feature map (mechanical, next)
One table in the package: 32 EngineOps (+ sugar/builder composition) →
cargo features each requires. Guarded by wire_sync-style parity test so a
new op cannot ship unmapped.

### R2 — detector productionization
Promote the validated prototype (resolved-AST call-finder; 13/13 ground
truth on example/) into package tooling. Input: app source root. Output:
reachable op set → feature set via R1. Fail closed: any unresolvable file
→ full set + a printed warning.

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

### R3 — trim wiring
- **Web**: `setup --trim` runs the detector over the app cwd → wasm
  feature set → `resolveWeb(wasmFeaturesOverride:)`. All pieces exist.
- **Native**: `trim: true` in `hooks: user_defines:` — OPEN QUESTION: where
  the detector runs. Options: (a) inside the build hook (hook deps may
  carry analyzer; needs app-root discovery from the hooks API — verify
  what BuildInput exposes), (b) `setup --trim` also writes a manifest the
  hook reads via the user-defines path mechanism. Decide by reading the
  hooks API, prefer (a) if the app root is reachable.

### R4 — shake verifier (`make shake-audit`)
Trim-profile build + assert absent symbols (nm / wasm names) + size
ceilings + the typed excluded-op runtime probe. This is what makes the
guarantee durable across upstream rebases.

### R5 — office/converters gating (the big web)
office_oxide + src/converters bleed into the extraction pipeline
(`pipeline/*` references — the two "converters" trees need untangling).
Measured value: ~1.5 MB raw (rootcut3). Do after R1-R4; needs its own
entanglement pass.

### R6 — consumer docs + changelog + CAPABILITY_ROADMAP promotion
At release time: README (registerFallbackFont + trim), changelog cut
(Engine updated bullet per the standard — submodule bumped), companion
CJK asset package decision.

## Verification status (Stage 3 tree)

test-rust PASS · analyze PASS · test-ops-native PASS · web: one jspi
`engine init` 2s-timeout in the chain run (solo rerun pending — earlier
identical pattern was environment load; if solo passes, chalk to env like
the atomics stress OOM, else diff the new feature set's glue).

## Scratch

`/tmp/treeshake_rnd/` — all experiment artifacts, build logs, results.
Detector prototype: `/private/tmp/treeshake_rnd/detector/`.
