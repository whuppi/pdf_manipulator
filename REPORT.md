# t-560 — every wasm the package ships is a manifest-listed set of parts

Patch: `t-560-pkg.patch` (everything except `vendor/`, `REPORT.md`, `PLAN.md`). Generated wasm parts and
`web_assets/pdf_oxide_manifest.json` are not in it; rebuild them with `make build-wasm DART=dart && make build-wasm-asyncify DART=dart`
(first build takes minutes; `web_assets/*.wasm` and the manifest are gitignored).

## What changed (file:line)

**The cut and the constants**
- `lib/src/hook/build_constants.dart` — `wasmPartBytes = 16 MiB`, `normalWasmGzipBudget` 8,912,896, `asyncifyWasmGzipBudget` 11,010,048 (one comment line each); `asyncifyWebAsset` is gone.
- `lib/src/hook/wasm_parts.dart` (new) — `writeWasmParts` (:167, cuts + merges the manifest entry, replaces the module's stale parts), `WasmManifest`, `joinWasmParts`, `verifyWasmParts`, `removeWasmModule`, `wasmWallProblems` (:265), `buildJsonPartProblems` (:302).
- `lib/src/hook/engine_compiler.dart:329,369` — `buildWasmParts` / `buildAsyncifyParts`: the one producer path (compile into a temp folder, cut into the output folder, delete the wholes).
- `tool/compile.dart:264` (`_doWasm`, `_doWasmAsyncify`, `_reportWasm`, `_checkWasm`:321) — make targets and the release go through those; the release check runs the wall and requires the manifest and build.json's list to agree both ways.
- `hook/build.dart:251` `resolveWeb` — default build: per-file waterfall over build.json's list (Asyncify parts skipped without `asyncify: true`); `_resolveManifestWithoutAsyncify`:327; custom build: `_installCustomWeb`:369 (one compile, every output copied, stale parts pruned); `_defaultWebOutputs`:550.
- `build.json`, `.github/workflows/release.yml`, `.github/actions/capabilities/wasm-{build,cache}/action.yml`, `Makefile`, `.gitignore`, `tool/verify_readme_sizes.dart`, `tool/shake_audit.sh`, `tool/asyncify_list.dart` (doc), `lib/src/hook/resolver.dart` (doc) name parts / manifest.
- `tool/ci/release_hooks.sh` is unchanged: it reads build.json generically, and the parts and manifest are `wasmBuildOutputs`, so they take the Release API digests like the wasm did. `asset_hashes.dart` is generated at release (empty on this branch).

**The runtime**
- `web_assets/executor_worker.js:401,447` — `compileParts` / `verifyPart` (the one JS loader); the `instantiateStreaming` override and `wasmFile()` are gone; `init` carries `wasmEntry`.
- `lib/src/runtime/web/wasm_loader.dart` (new) — manifest parsing and `_compileParts`:86 / `_pumpPart`:131, the main-thread twin.
- `lib/src/runtime/web/host.dart:292,302` — `resolveMode` / `_decideMode` (asynchronous; manifest fetched once per page and location, `_manifestOf`:511; `_opfsRuns`:464 awaits `getDirectory()`); order JSPI, Atomics, OPFS, Asyncify; `_requireRunnable` async and keeps `forceMode`'s typed errors; the pool key is `url#<forced mode or auto>`; the module cache is keyed by location + module.
- `lib/src/runtime/lane.dart` (`ExecutorBooter.resolveMode` replaces the sync `mode`), `router.dart:153` (`ensureInitialized` resolves the mode through the pool; the constructor loses `mode:`), `pool.dart`, `native/host.dart`, `bridge/create_bridge.dart`.
- `lib/src/types/pdf_config.dart`, `pdf_enums.dart` — docs say the new order and that the typed error comes at initialization.
- `host.dart` also marks `booted.future` observed (see "Found on the way").

**Tests** — `test/hook/wasm_parts_test.dart` (cutter: 40 MiB+1 → 16/16/8 MiB+1 with names and hashes; exactly 16 MiB → one part; one byte over → two; merge keeps the other module; recut replaces stale parts; join; remove; a changed byte is named); `test/guards/wasm_size_test.dart` (the walls, each shown failing on a synthetic folder, then the built `web_assets/`); `test/ops/platform/web/web_wasm_parts_battery.dart` (new, registered in all four runners): manifest fetched once + parts of exactly one module once each + never a whole `.wasm`; corrupt part and short part each fail initialization naming the part, on the host's compile and on the worker's own (a raw worker booted with no module); the worker compiles from the entry; missing manifest and an HTML answer for it are `PdfUnsupported` naming `pdf_oxide_manifest.json` and `pdf_manipulator:setup`; SPA host + manifest without Asyncify never asks for an Asyncify part (typed error when Asyncify is forced); `web_auto_mode_test.dart` (rewritten for the new order, see below). Harness: `audited_asset_server.dart` takes a map (`hide`, `spa`, `manifestWithout`, `corrupt`, `truncate`), `audited_server.dart` passes it.

**Docs** — `docs/ARCHITECTURE.md` (new "Web assets — the manifest and the parts" section, order, file tree), `README.md` (setup asset list, order, self-hosting note: serve `.wasm` as `application/wasm` with compression, a precaching service worker reads the manifest), `CHANGELOG.pre.md` (one Changed bullet in 6.0.0-dev.0, and the Asyncify bullet no longer names the whole file / 32 MB), `docs/MIGRATION.md` (a Removed bullet for apps listing `pdf_oxide_bg.wasm`).

**Grep sweep** for `pdf_oxide_bg.wasm`, `pdf_oxide_asyncify_bg.wasm`, `asyncifyWasmFile`, `asyncifyWebAsset` (outside vendor/, example/build): files changed for it: `README.md`, `docs/ARCHITECTURE.md`, `CHANGELOG.pre.md`, `docs/MIGRATION.md`, `Makefile`, `tool/compile.dart`, `tool/verify_readme_sizes.dart`, `tool/shake_audit.sh`, `tool/asyncify_list.dart`, `lib/src/hook/resolver.dart`, `lib/src/runtime/web/host.dart`, `test/guards/wasm_size_test.dart`, both asset batteries and the auto test, `.github/…`. What remains is intended: `lib/src/hook/engine_compiler.dart` and `tool/measure_wasm.dart` name the whole modules that are build intermediates in a temp folder; `test/guards/wasm_size_test.dart` and the new battery name them to assert they are never shipped or fetched; the generated, committed `web_assets/pdf_oxide.js:312` has bindgen's default `new URL('pdf_oxide_bg.wasm', …)`, which is never reached because both loaders hand `init` a compiled module.

## Existing tests changed, and why
- `test/guards/wasm_size_test.dart` — pinned the two whole file names and the `.wasm` size scan; now the walls and the part names (plan item 8).
- `test/ops/platform/web/web_asyncify_assets_battery.dart` — hid and counted the whole `pdf_oxide_asyncify_bg.wasm`, and expected the typed error "at creation" after a HEAD probe; now parts, a manifest without the entry, the error at `ensureInitialized`, and no probe (plan items 2, 6).
- `test/ops/runners/web_auto_mode_test.dart` — the old order and the HEAD probe; rewritten to the order JSPI, Atomics, OPFS, Asyncify and the cases in the plan, plus the SPA case.
- `ExecutorBooter.mode` became `resolveMode()` (plan item 6), so the call sites follow: `test/harness/fake_executors.dart`, `test/ops/core/pdf_executor_battery.dart`, `test/ops/core/pdf_runtime_safety_battery.dart` (wrapper hosts forward it), and `test/runtime/router_test.dart` (the `Router(mode:)` argument and an unused import removed). `test/ops/runners/web_*_runner_test.dart` only register the new battery.

## Each new test seen failing first
The tests were written before the runtime code. The runtime was then written, and the failure shown by putting the old `host.dart` / `executor_worker.js` back temporarily (with a one-line `resolveMode` shim and the old whole wasm in place), running, and restoring (`git diff` and a grep confirm nothing is left):
- OPFS runner, `-N 'web wasm parts'`: 9 failures, e.g. `Expected: ['GET web_assets/pdf_oxide_manifest.json'] Actual: []`; a corrupt part: `Expected: throws an error naming pdf_oxide_bg.0.wasm Actual: … emitted <Instance of 'PdfDocumentHandle'>`; the raw worker: `Expected: 'error' Actual: 'ready'`.
- `web_auto_mode_test.dart`: 3 of 5 failed: `Expected: PdfIoMode.asyncify Actual: PdfIoMode.opfs`; `Expected: throws PdfUnsupported … Actual: <Instance of '_Future<PdfIoMode>'>`.
- The cutter test needs `wasm_parts.dart`, which did not exist when it was written (it could not compile).

## The walls, each failing once (then restored; `build_constants.dart` diff holds only the intended constants)
- Cap lowered to 10 MiB: `'pdf_oxide_asyncify_bg.0.wasm: 16777216 bytes reaches the 10485760-byte limit of a file'`, and the same for `pdf_oxide_bg.0.wasm`.
- One byte of `pdf_oxide_bg.1.wasm` changed: `['pdf_oxide_bg.1.wasm: its sha256 differs from the manifest']`.
- `normalWasmGzipBudget` set to 8,000,000: `'pdf_oxide_bg: 8108654 gzip bytes exceed the budget of 8000000; raising a budget is a decision, not a fix'`.
- The synthetic-folder tests in `wasm_size_test.dart` keep each of these failures (and a missing part) permanently tested.

## Sizes (this build)
| module | raw | parts | gzip (whole) | budget |
|---|---|---|---|---|
| `pdf_oxide_bg` | 20,877,699 | 16,777,216 + 4,100,483 | 8,108,654 | 8,912,896 |
| `pdf_oxide_asyncify_bg` | 28,229,263 | 16,777,216 + 11,452,047 | 10,159,632 | 11,010,048 |

(The plan measured 8,108,428 / 10,159,769; the small difference is this machine's build.) No file in `web_assets/` reaches 26,214,400 bytes.

## Summary lines
- `dart analyze --fatal-infos lib hook tool test bin`: `12 issues found.` — all 12 are in `tool/bench/` (its own package; `package:toml`, `pdf_document`, `pdf_manipulator_bench` are not resolvable from the root); none in any file this task touched. `dart analyze` over `lib hook test bin tool/*.dart tool/ci`: `No issues found!`
- `dart format --set-exit-if-changed lib hook tool test bin`: `Formatted 277 files (0 changed)`
- `dart test test/guards test/runtime test/bridge test/keep test/hook`: `01:11 +466: All tests passed!`
- native runner: `01:16 +663 ~1: All tests passed!`
- `web_opfs_runner_test.dart`: `03:45 +693 ~6: All tests passed!`
- `web_jspi_runner_test.dart`: `02:32 +685 ~1: All tests passed!`
- `web_atomics_runner_test.dart` (`-p chrome-coi`): `01:53 +682 ~2: All tests passed!`
- `web_asyncify_runner_test.dart` (`--timeout=5x`): `02:59 +685 ~1: All tests passed!`
- `web_auto_mode_test.dart`: `00:10 +5: All tests passed!`
- `check_changelog_shape.sh`: exit 0; `check_migration.sh`: `every breaking change of the newest release has its migration`; `check_migration_snippets.sh`: `every migration snippet compiles against its version`
- Manual: `resolveWeb` on a dev version into a temp folder: `asyncify: false` wrote 5 files and a manifest with only `pdf_oxide_bg`; `true` added the Asyncify parts and entry; `false` again removed them; `verifyWasmParts` empty each time. The download path (release assets, hashes) cannot run on this branch (no release has the parts).

## Choices the plan left open
- **Whole wasm**: kept nowhere. The builds make wholes in a temp folder and delete them; the Asyncify build joins the normal module's parts back for its input. `web_assets/` holds only parts, manifest and the two JS files.
- **The loader twin**: duplicated once, not routed through the worker: `compileParts` (JS) and `_compileParts` (Dart) keep the host's compile-once-per-page. The battery pins both against the same manifest: the host path (through `Pdf`) and the worker path (a raw worker given only the manifest entry) both fail with the part's name on a corrupt and on a short part.
- **Manifest to the worker**: the host sends the module's entry in `init` (`wasmEntry`), so the manifest is fetched once per page and the worker never fetches it.
- **No `crypto.subtle`** (a page that is not a secure context): the byte count is still enforced and the SHA-256 is skipped; stated in the README. A hash implementation of our own was the wider choice.
- **`asyncify: false` and the release manifest**: the release manifest lists both modules, so the hook rewrites it without the Asyncify entry. That file no longer matches the release's pinned hash, so a rewritten manifest stands on its parts (it is current when it has no Asyncify entry and every part it lists verifies, the parts having passed their pinned hashes); otherwise it is resolved again into a temp file and filtered.
- **Old whole files**: setup does not delete a stale `pdf_oxide_bg.wasm` from an app's web folder; it only prunes unlisted part files. The migration bullet tells apps to delete them (an app keeping the 28 MB Asyncify whole would still break a 25 MiB host).
- **build.json's part list**: build.json lists two parts per module, as both modules are today; the wall and the release check fail when the manifest has a part build.json does not list (a module past 32 MiB).
- **CI cache**: the wasm cache saves `pdf_oxide*_bg.*.wasm` and the manifest, not `pdf_oxide.js` (tracked in git, as before).
- **Pool key** is `url#<forced mode|auto>` since the mode is no longer known at construction; instances of one page and location share a pool as before.
- **A failed mode resolution or manifest fetch is not cached**: the next call asks again, so a fixed deploy recovers without a reload.

## Found on the way
- `WebExecutorBooter._bootWorker` left `booted.future` unobserved while the module compiled. The old whole-file compile was cached by file name across locations, so the flaky-server test never waited; keyed by location, it does, and a worker failing meanwhile surfaced as an unhandled error in `web platform a failed boot does not poison the lane`. `booted.future.ignore()`, like `ready`, fixes it.
- Two guards forced the error types: failures in the loader are `PdfIoFailed` (a part) and `PdfUnsupported` (a manifest), not `StateError`/`FormatException`; `WasmManifest.write` became `storeIn` (the sink-write guard).
