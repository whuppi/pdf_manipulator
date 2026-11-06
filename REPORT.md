# t-541 — the Asyncify web mode

All seven steps are built and the Asyncify mode runs the whole suite. **One thing
the design assumed does not hold**: the full-engine Asyncify wasm is
**31,968,901 bytes, over the 25 MiB (26,214,400) per-file cap**. The release
wall refuses it, so a release of a full-engine Asyncify asset fails at compile
until that is decided (see "The size, and what the wall does").

## The instrumentation list

Method (`lib/src/hook/asyncify_list.dart`, run on the built `pdf_oxide_bg.wasm`;
`tool/asyncify_list.dart` prints its counts):

1. Pausing imports: `host_read_at` and `host_write_chunk` (the two the JSPI
   worker wraps in `WebAssembly.Suspending`; `host_cancelled` is polled, never
   suspends, and is not in `asyncify-imports`).
2. Their callers, transitively, over direct `call`s.
3. A `call_indirect` of type T reaches every function of type T named by an
   element segment or `ref.func` (this is where `dyn Read + Seek` and
   `dyn Write` live). The parser refuses a module with `table.get/set`,
   `table.init/copy/grow/fill`, `call_ref` or an imported table (none here),
   which is what makes the resolution sound.
4. The result is given to binaryen as `asyncify-onlylist` (a file), with
   `asyncify-imports` set to the two import names; the optimizer (`-O2`) runs
   after it.

Result on the wasm built here (Rust 1.97, `make build-wasm`; 8,737 functions of
which 12 are imports, 2,354 address-taken):

| Set | Functions |
|---|---|
| callers by direct calls only | 20 |
| callers with `call_indirect` resolved by signature (**the list**) | 7,442 (85.2% of the module); 7,440 defined functions, a 36,140-byte file |

Where it grows, from the tool: `(i32,i32,i32,i32)->()` (`dyn Read::read` / `Write::write`)
+1,059; `(i32,i32,i32)->()` +273; `(i32)->()` (drop glue) +681; `(i32,i32)->()`
**+4,642** (from 78 call-site functions; three tiny functions of two to 484 direct
callers do most of it); `(i32,i32,i32)->(i32)` +724. Treating those three as
non-reaching would give 3,217 functions, but nothing in the wasm proves it, so
the list stays at the sound 7,442. Two cheaper refinements did not help (7,436
with vtable-slot resolution, because 894 of 1,916 indirect sites are not vtable
loads; restricting to what `execute` reaches changes nothing, 8,714 of 8,737 are
reachable from it).

**A bug I found and fixed on the way.** binaryen names an unnamed *defined*
function by its index among the defined functions, imports not counted. The
first list used module-wide indices, shifted by the 12 imports: the build
succeeded, about 190 tests had trapped with `unreachable` when I stopped that run, and the output was nearly the
same size (32,449,093 bytes). `asyncify_list_test.dart` now pins the numbering
("fails on a list numbered with the imports counted"). The sizes below are from
the corrected list.

## Sizes

| | raw | gzip (Dart `gzip`) |
|---|---|---|
| normal, full engine | 20,870,567 | 8,106,180 |
| Asyncify, full engine | 31,968,901 (+53.2%) | 12,436,134 (+53.4%) |
| cap | 26,214,400 | |
| normal, core only (`keep: []`) | 7,647,201 | 2,650,907 |
| Asyncify, core only | 12,871,458 (+68.3%) | 4,700,874 |

The core-only pair was built through the hook's trimmed path
(`compileWasmEngine(features: ..., asyncify: true)`), so a trimmed `keep` with
`asyncify: true` fits the cap. The full engine does not. Rust 1.97 builds the
normal wasm 1.2 MB larger than the plan's ~19.6 MB.

## The size, and what the wall does

`test/guards/wasm_size_test.dart` fails on any `web_assets/*.wasm` at or over
25 MiB, and `tool/compile.dart` (release, `COMPILE_OUTPUT_DIR` set) builds the
Asyncify wasm and throws the same way, so the release pipeline stops before it
uploads a wasm Cloudflare static assets cannot serve. With the full Asyncify
wasm built, `dart test test/guards` is red on that one test (output below).
Options (not decided here): ship the Asyncify asset only for trimmed `keep`
sets; remove the bridge functions at the engine (a larger change, listed
above); or raise the wall for that one asset. I did not weaken the wall.

## Changes (file:line)

- `lib/src/hook/asyncify_list.dart` (new): `WasmCallGraph` (parser, `ancestors`,
  `stackFunctions`) :103; `asyncifyList` :37; `asyncifyImports` :26;
  `verifyAsyncifyList` (the wall) :45.
- `vendor/pdf_oxide/bindgen_runner/src/main.rs` `fn asyncify` :44 and the
  `--asyncify` mode in `main`: binaryen Asyncify on its own, then `-O2`.
  Fork-only crate (`t-541-fork.patch`).
- `lib/src/hook/engine_compiler.dart`: `compileWasmEngine(asyncify:)` :333 and
  `buildAsyncifyWasm` :420 (list written next to the output, checked against the
  wasm, then binaryen).
- `tool/compile.dart`: `wasm-asyncify` mode, release builds and mirrors the
  Asyncify wasm and runs `_checkWasmSizes` :314; `tool/asyncify_list.dart` (new).
- `lib/src/hook/build_constants.dart`: `asyncifyWebAsset`, `maxWebAssetBytes`.
  `build.json`: the asset in `web` and `wasmBuildOutputs`.
  `.github/workflows/release.yml`: the asset in the verified list.
- `lib/src/hook/pdf_config.dart`: key `asyncify` (`knownKeys`, `_parseAsyncify`
  :148, YAML boolean only, native accepts and ignores it).
  `hook/build.dart`: `resolveWeb(asyncify:)` :274 skips the asset when false;
  `_compileWasm` copies a fresher local asset, builds from the normal wasm, or
  compiles with `asyncify: true` (trimmed keep). `bin/setup.dart`: help + message.
- `lib/src/types/pdf_enums.dart:27` `PdfIoMode.asyncify`; `pdf_config.dart` doc.
- `lib/src/runtime/web/host.dart`: `_detectMode` :413 (JSPI → Atomics →
  Asyncify when shipped → OPFS), `_requireRunnable` :433 (a forced mode the
  browser cannot run, or Asyncify without its file, throws `PdfUnsupported` at
  creation), `_asyncifyShipped` (one synchronous `HEAD` per location, only on a
  page that could run the mode or forced it), `wasmFileOf` :296 (a browser
  fetches only the wasm of its mode), per-file module cache.
- `lib/src/runtime/web/lease.dart`: `_suspends` :190 (JSPI and Asyncify share the
  read/write answer messages; the flags were renamed `_awaitingRead/Write`).
  `protocol.dart:201` `kAsyncifyStackBytes`, injected as `asyncifyStackBytes`.
- `web_assets/executor_worker.js`: `resumingAsyncify` :216, `suspendAsyncify`
  :222, `runAsyncify` :449, `wasmFile` :399, the saved-stack pages grown from
  linear memory (:389) with an overflow check after each unwind.
- `Makefile`: `build-wasm-asyncify`, `ASYNCIFY_OUT`, `test-ops-asyncify`
  (with the auto-mode file), in `test-ops-web`. `AGENTS.md`, `CONTRIBUTING.md`:
  "4 web modes".
- Tests: `test/hook/asyncify_list_test.dart`, `test/hook/pdf_config_test.dart`
  (asyncify group), `test/guards/wasm_size_test.dart`,
  `test/ops/runners/web_asyncify_runner_test.dart` (same batteries as OPFS/JSPI,
  incl. the cancel batteries and `registerWebCancelChannelTests`, the
  cancel-latency row, since plain chrome has no SAB),
  `test/ops/runners/web_auto_mode_test.dart`,
  `test/ops/platform/web/web_asyncify_assets_battery.dart` (registered in all
  four runners), `test/harness/audited_asset_server.dart` + `audited_server.dart`,
  `test/ops/runners/web_io_measure_test.dart`; two batteries accept Asyncify
  where they accepted JSPI.
- Docs: `docs/ARCHITECTURE.md` (web modes, the Asyncify wasm, cancel table,
  source tree), `README.md` (mode table, explanation, "Asyncify, the second
  WASM" with the option and the trade-off), `CHANGELOG.pre.md` (one Added bullet
  in 6.0.0-dev.0).

## Walls and their injected failures

1. **The list wall** (`verifyAsyncifyList`, run by every build between writing
   the list and binaryen reading it). Injected: `if (false && missing.isNotEmpty)`:
   ```
   00:00 +5 -1: the wall fails when a function reachable from an import is missing [E]
     Expected: throws <Instance of 'StateError'> with `message`: contains '2'
   00:00 +5 -2: the wall fails on a list numbered with the imports counted [E]
   00:00 +5 -3: the wall fails on a list from another build (stale) [E]
   ```
   Restored; 8 pass.
2. **The size wall**, red for real on the corrected build:
   ```
   Expected: empty
     Actual: {'web_assets/pdf_oxide_asyncify_bg.wasm': '31968901 bytes'}
   limit is 26214400 bytes; built: {...asyncify_bg.wasm: 31968901, ...pdf_oxide_bg.wasm: 20870567}
   ```
   Green with only the normal wasm present.
3. **build.json ↔ release workflow parity** (same file). Injected: the asset
   line removed from `release.yml`: `Expected: contains '            wasm-pdf_oxide_asyncify_bg.wasm\n'`. Restored.
4. **A page without the Asyncify wasm never asks for it.** Injected: probe
   always: `Expected: empty / Actual: ['HEAD web_assets/pdf_oxide_asyncify_bg.wasm']`
   (jspi runner). Restored.
5. **Forcing Asyncify without the file is a typed error.** Injected: the check
   made `true`: `Expected: throws ... PdfUnsupported ... Actual: <Closure ...>`
   (asyncify runner). Restored.

## The measurement (step 6)

Generated text PDF (48 KiB of content per page), `Pdf` forced to each mode,
open + `extractText` of the first 50 pages, then `merge` of two copies, 4-core
machine, headless Chrome 153; each mode twice, alternating
(`test/ops/runners/web_io_measure_test.dart`, `--dart2js-args=-DMEASURE_MB=n`).
OPFS includes its pre-copy.

| File | Mode | open + extract | source reads | merge (2 files) |
|---|---|---|---|---|
| 105 MB | Asyncify | 31.2 s, 28.2 s | 7,054 | 5.8 s, 4.9 s |
| 105 MB | OPFS | 14.5 s, 13.4 s | 1,607 (the pre-copy) | 2.6 s, 2.7 s |
| 316 MB | Asyncify | 93.2 s, 94.9 s | | 18.7 s, 16.4 s |
| 316 MB | OPFS | 41.2 s, 41.6 s | | 10.4 s, 9.0 s |

Asyncify is about 2.2× slower than OPFS here: about 4 ms per source read
(a round trip plus unwinding and rewinding the stack), against 64 KiB local
reads after a sequential copy. The automatic order is unchanged
(JSPI → Atomics → Asyncify → OPFS); this is the user's decision. Not measured:
a file read in few large pieces, or peak memory.

## Gate

| Line | Result |
|---|---|
| `dart analyze --fatal-infos lib test` | No issues; `dart format` clean; `cargo fmt --check` and `cargo clippy -D warnings` clean on `bindgen_runner` |
| `dart test test/guards test/runtime test/bridge test/keep test/hook` | 443 passed with only the normal wasm built; **1 failed (the size wall) with the full Asyncify wasm built** |
| native runner | 663 passed, 1 skipped |
| OPFS runner (`-p chrome`) | 684 passed, 6 skipped |
| JSPI runner | 676 passed, 1 skipped |
| Atomics runner (`-p chrome-coi`) | 673 passed, 2 skipped |
| Asyncify runner | 676 passed, 1 skipped at `--timeout=3x`; at the default 1x, 5 and 12 tests exceeded their 1-second budget in two runs (all `TimeoutException`, none a failure), so `make test-ops-asyncify` uses 5x locally (CI's 30x unchanged); at 3x one RSA test (`pkcs12`) timed out once, in the 128 KiB stack experiment run |
| `web_auto_mode_test.dart` (JSPI, Asyncify before OPFS, OPFS) | 3 passed |
| `bash tool/check_changelog_shape.sh`, `check_migration*.sh`, `check_fork_tests.sh`, `check_patch_markers.sh`, `check_cancel_polls.sh` | exit 0 |
| `cargo nextest` on `vendor/pdf_oxide` | **not run**: the fork changed only in `bindgen_runner` (a tool crate, not the engine); the engine sources are untouched |
| `make check`, `test-example-*` | not run (the example app has no Asyncify row) |

A bug the gate caught in my own change: keying the compiled-module cache by
URL broke "a failed boot does not poison the lane" in every mode; it is keyed by
file name again (assets live in one place per page).

The stack for the unwound frames is 4 MiB of fresh pages per worker; with the
check set to 128 KiB the whole Asyncify runner tripped nothing (32× margin).

## Choices the plan left open

- Detection of "shipped": a synchronous `HEAD` on the main thread, only for a
  page with neither JSPI nor SAB or that forced Asyncify (the Dart `mode` is
  decided at construction and tests read it synchronously; an async probe would
  have changed the booter, router and pool). Auto-detect issues that `HEAD`
  once per location; a page that never needs it never asks.
- `asyncify:` is accepted on native and does nothing there (one pubspec serves
  both); a non-boolean is refused.
- The release builds the Asyncify wasm on every `compile wasm`; locally
  `make build-wasm` stays the normal wasm only.
- Cancel keeps the blob-URL poll (`hostCancelled` unchanged); Asyncify gives no
  reason to change it.
- binaryen is the `wasm-opt` 0.116 crate pinned by `Cargo.lock`, Rust 1.97 not
  the pinned 1.92.

## Not built / caveats

- The Asyncify asset has no prebuilt hash yet (`asset_hashes.dart` is stamped by
  a release), and the full-engine asset cannot be released while it is over the
  cap.
- The trimmed Asyncify wasm was built and passed the in-build wall; it was not
  run in a browser (the suite runs against the full engine).
- No Asyncify row in the example app's integration tests.
- Generated wasm is not in the patches; rebuild with `make build-wasm` then
  `make build-wasm-asyncify`.
