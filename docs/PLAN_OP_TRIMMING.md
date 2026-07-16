# Plan — Ship only the ops the app calls (#167)

> Working doc for [#167](https://github.com/whuppi/pdf_manipulator/issues/167).
> Not canonical; promote pieces into CAPABILITY_ROADMAP.md as they land.
> Status: Stage 0 (measurement) COMPLETE 2026-07-16. Architecture proven by
> experiment on wasm; native root inventory taken. Numbers marked (measured)
> are measured; everything else is estimate.

## The architecture (proven on wasm)

We do not build a tree-shaker. The linker already is one (LTO + wasm-gc;
`lto = true`, `codegen-units = 1` confirmed in the release profile). We build
the two things Dart lacks:

```
[ DETECTOR ]            [ ROOT OWNERSHIP ]           [ SHAKER ]
which ops does    →     every GC/linker root    →    LTO strips all code+data
the app call?           of the binary is one          unreachable from live
(we build this)         WE gate (we patch this)       roots (exists, proven)
```

Precedent: Flutter's icon-font tree-shaking (const_finder) — a targeted
static analysis for one API surface, shipped without waiting for the SDK.

## The literal guarantee — what "everything shaken" means

The promise to a rebuild-mode consumer, stated exactly:

> After a trim rebuild, the binary contains the transitive closure of the
> roots you kept, and nothing else. The linker guarantees this by
> reachability — it is not a best-effort cleanup pass.

For that promise to be TRUE, every root class must be inventoried and
gated. The full root inventory (found by experiment — missing any one of
these silently re-pins everything behind it, which is exactly what
rootcut1 measured):

| Root class | Where | Count | Status |
|---|---|---|---|
| Our lane bridge (wasm) | `host/bridge_api.rs` `lane_init/execute/destroy` | 3 | ours, per-op cfg goes here |
| Upstream wasm JS API | `src/wasm.rs` `#[wasm_bindgen]` | **289** | dead to us — amputate (Stage 1) |
| Our lane bridge (native) | `host/native/lane_table.rs` `lane_*`/`channel_*` | ~8 | ours, per-op cfg goes here |
| Upstream C API | `src/ffi.rs` `#[no_mangle]` | **~420** | dead to us — amputate (Stage 1b, native twin) |
| `include_bytes!` data | fonts, ICC profiles | — | strips when its referencing code strips (proven: CJK font fell with its feature) |
| Rust std / alloc / panic / bindgen glue | — | — | the floor; not removable |

Honest granularity limits (the guarantee is real, but know its units):

- **Per-op, not per-line.** Keeping `extract` keeps the whole parser the
  extractor needs. Shared internals used by ANY kept op stay — that is
  correctness, not leakage.
- **Debug builds are always full-size** (link hooks do not run). Correct
  semantics: dev experience untouched, release trims.
- **The floor exists**: core parser/writer + std + glue, ~4-5 MB raw
  (estimate) for a forms-only profile before Stage 4.

### The shake verifier — "not leave anything" is proven, not hoped

A guard, not a vibe. New CI + local target (`make shake-audit`):

1. Build a minimal profile (e.g. forms-only).
2. Assert **absent symbols**: `twiggy`/name-section grep (wasm) and `nm`
   (native) must find ZERO office_oxide / signature / render symbols in a
   build that excluded them. A hit = a new root leaked in = FAIL.
3. Assert **size ceilings** per profile (raw + gz budgets, updated on
   engine bumps like the README size-audit discipline).
4. Runtime probe: calling an excluded op must return the typed
   "not in this build" error; a kept op must succeed.

This is what makes the guarantee durable across upstream rebases: a tag
move that adds a new export surface or a new unconditional root fails the
audit immediately instead of silently re-fattening every user's app.

## Stage 0 results — the measured truth (2026-07-16)

Baseline wasm: 25,797,870 raw / 11,284,482 gz. Split: **12.5 MB data
segments + 10.9 MB code** (+ ~2.4 MB tables/misc).

| Experiment | Cut | raw | gz | Δ raw vs baseline |
|---|---|---|---|---|
| baseline | — | 25.80 MB | 11.28 MB | — |
| A | `rendering` feature | 18.88 MB | 7.91 MB | −6.92 MB (measured) |
| B | + `cjk-form-fonts` feature | 14.40 MB | 5.46 MB | −11.40 MB (measured) |
| rootcut1 | convert arms only | 25.61 MB | 11.23 MB | −0.19 MB ← exports pinned everything |
| rootcut2 | wasm export surface only | 22.58 MB | 9.93 MB | −3.21 MB (measured) |
| rootcut3 | exports + convert arms | 21.02 MB | 9.35 MB | −4.55 MB; arm cut alone worth 1.49 MB once exports gone — 8× rootcut1 |

Readings: (1) the two-API discovery — the binary ships upstream's own
JS SDK, 289 exports Dart can never call (`lane_worker.js` verified to
import only `lane_init`/`lane_execute`/`memory`); (2) root-cutting works
once we own ALL roots; (3) half the binary is DATA and the old roadmap's
"size is code, not data" claim is disproven; (4) feature-axis alone
already gives a stamp+forms app **5.46 MB wire instead of 11.28 (−52%)**
with zero fork edits.

Side-find RESOLVED (false alarm): the two `DejaVuSans.ttf` embeds
(`html_css/paint.rs`, `writer/font_shaping.rs`) both sit inside
`mod tests {` blocks — compiled only for `cargo test`, never shipped in
release binaries. No action needed; Stage 1c is closed.

## The detector — both candidates validated

**Detector A — analyzer call-finder (prototype WORKS).** ~80 lines on
`package:analyzer`; resolved-AST walk recording every invocation whose
element's library is `package:pdf_manipulator/...`. Against `example/lib`:
156 resolved references, full op-bearing surface, **13/13 ground truth,
zero misses**. True element resolution — a consumer's own `render` method
cannot false-positive. Runs anywhere Dart runs → web gets it TODAY via the
setup command. Must also map **sugar ops** (`pdf_sugar.dart` one-shots
compose editor ops) and the builder DSL to their underlying EngineOps —
the map is a maintained table in the package, checked by wire_sync_test
so a new op cannot ship unmapped.

**Detector B — @RecordUse (rails exist NOW).** `record_use 1.0.0` +
`hooks 1.0.0` (`LinkInput.recordedUsagesFile`) are live in the current
SDK. Records calls to statically-resolved fns WITH const args,
reachability-aware. Shim pattern: `@RecordUse() void useOp(String op)`
called with a const op name inside each public API method — Dart's own
AOT tree-shaker becomes the oracle. Native+release only; end-to-end AOT
spike still UNVERIFIED (timebox it).

**Detection failure policy: fail CLOSED.** Any unresolved file, any
analysis error, any dynamic pattern the detector cannot prove → full
binary + a warning. A trim must never break an app. Explicit
`user_defines` manifest always overrides detection (escape hatch both
directions: force-trim or force-full).

## The fonts workstream — decouple the fallback WITHOUT losing the feature

Current state (found in fork): `src/fonts/form_fallback.rs` embeds
`DroidSansFallbackFull.ttf` (CJK, ~4 MB — the measured `data[539]`) and
`NotoEmoji-Regular.ttf`, behind `cjk-form-fonts` / `cjk-render-fallback`.
Consumed by the form-fill appearance path in `document_editor.rs` (the
#155/#156 fix: CJK/emoji values drawing nothing). The engine needs the
font BYTES AT FILL TIME; nothing requires them to be COMPILED IN —
embedding was the expedient wiring, not a requirement of the feature.

Options researched:

| Option | Mechanism | Verdict |
|---|---|---|
| A. Embedded (today) | `include_bytes!` | works offline, +4 MB on everyone with the feature; the thing we are removing |
| B. **Injected bytes (RECOMMENDED core)** | engine gains a fallback-font registry: a lane op / config field accepts TTF bytes at runtime; `form_fallback.rs` reads the registry first, embedded bytes demoted to an optional compat feature | keeps the feature fully, moves the 4 MB out of the binary, works native + web, offline-safe (bytes come from the app's own assets) |
| C. Companion asset package | tiny `pdf_manipulator_cjk_fallback` package shipping the TTF as a Flutter asset; sugar API feeds it to B | opt-in by dependency choice — the cleanest consumer story on top of B |
| D. Lazy fetch (icu_kit model) | setup/web places the font next to the wasm; worker fetches on first CJK fill; native build hook downloads to cache | good web ergonomics on top of B; adds network-at-op-time only for users who never declared the asset |
| E. System fonts (native) | load OS-installed CJK font (Android ships Noto CJK) | nice opportunistic FIRST TRY on native before the registry; never the only path (Linux containers, licensing of redistribution not implicated when reading at runtime) |
| F. Subsetting | pre-subset the shipped font | dead end alone: fill values are unknown ahead of time, so the full font must exist SOMEWHERE at fill time — subsetting only applies to what gets embedded INTO the produced PDF (the engine's existing concern), not to what we ship |

Decision: **B is the mechanism; C is the packaging; D is web sugar; E is a
native nicety.** End state: default build ships ZERO embedded fallback
fonts; CJK/emoji form-fill keeps working for anyone who adds the companion
package (or hands us bytes); the typed error for "value needs a fallback
font and none is registered" tells the user exactly what to add.

## The stages

### Stage 1 — the everyone-wins amputations (no detector needed)

- **1a wasm**: gate upstream's `pub mod wasm` (289 exports) behind a
  default-off feature — 2-line markered patch, worker-compat verified.
  −3.21 MB raw / −1.36 MB gz (measured) for every web user.
- **1b native twin**: same amputation for upstream's `src/ffi.rs` C API
  (~420 exports), keeping only `lane_*`/`channel_*` + the log hooks our
  FFI bindings declare. Delta unmeasured — run the same experiment before
  shipping.
- **1c**: CLOSED — DejaVu embeds are test-gated (`mod tests`), never shipped.
- Test strategy: `make test-rust` runs with `public-api` ENABLED (Makefile-side,
  ours) so upstream's ffi/wasm test suites keep compiling and covering the
  amputated surfaces; only shipped builds (build.json feature lists,
  unchanged) drop them.
- Gate: full `make check` + web battery (all 3 modes) + native op battery.

### Stage 2 — fonts decoupled (B + C above)

Registry op in the engine (host/ + one markered touch at the
`form_fallback.rs` seam), Dart API (`PdfConfig.fallbackFonts` or explicit
`registerFallbackFont`), companion asset package, typed
missing-fallback-font error. Default build drops `cjk-form-fonts`.
−4 MB raw against TODAY'S baseline for everyone (measured as part of
build B's delta).

### Stage 3 — coarse trim via `user_defines` (manual rebuild mode)

Consumer opt-out in pubspec → cargo `--features`; web gets the same knob
on setup. Non-default sets fall through the resolution waterfall to local
compile (Rust toolchain required; loud, actionable error). Cache keyed by
feature-set hash. Fork patches: decouple `wasm` feature from
`signatures`+`barcodes`; office_oxide behind a feature. Excluded op =
typed error. **Ships together with the shake verifier** so the literal
guarantee is enforced from the first trim release, and wire_sync_test
becomes feature-aware.

### Stage 4 — per-op roots + detector (automatic rebuild mode)

Feature-per-op-group cfg on OUR dispatch arms (wasm match + native lane
table together — one op gated in both or neither). Detector output →
feature set → recompile: native release via link hook (B or A), web via
setup (A). Manifest override. Fail-closed policy above.

### Stage 5 — data lazy-loading (option D wiring, if demand)

## Order of work

1. Stage 1a/1b/1c amputations (+ measure 1b) — post results to #167
2. Fonts Stage 2 (kills the single biggest data chunk, keeps DC's feature)
3. Detector B AOT spike (timeboxed) → pick composition (A-everywhere is
   the current lean)
4. Stage 3 user_defines + shake verifier
5. Stage 4 automatic detection
6. Stage 5 if demand

## Scratch artifacts

`/tmp/treeshake_rnd/` — experiment wasms, build logs, results.txt,
detector prototype. All repo files restored; vendor submodules clean.
