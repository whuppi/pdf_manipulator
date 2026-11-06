You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-560: every wasm the package ships becomes a manifest-listed set of parts. Approved by the user after an outside ruling. Why: static hosts cap each uploaded file (Cloudflare Pages and Workers static assets: 25 MiB, 26,214,400 bytes, per file), not the size of a wasm module a browser compiles. The Asyncify wasm (t-541, already in this branch) is 28.2 MB, and the normal wasm is 20.9 MB and growing. A module is bytes, not a file: the build cuts it into parts, and the browser joins the parts into one stream while it compiles.

Read first: docs/ARCHITECTURE.md (the web runtime, the four web modes, the executor worker), lib/src/runtime/web/host.dart (`WebExecutorBooter`: `_detectMode` near :412, `_requireRunnable`, `_asyncifyShipped` near :467, `_wasmModule` near :488), lib/src/runtime/web/lease.dart, lib/src/runtime/router.dart (`ioMode`, `ensureInitialized` near :155), lib/src/ops/pdf.dart:141-160, web_assets/executor_worker.js (boot, `wasmFile` near :399, the instantiateStreaming override near :357-376), lib/src/hook/ (build_constants.dart, engine_compiler.dart, resolver.dart, asset_hashes.dart), hook/build.dart (`_compileWasm`), tool/compile.dart (`_doWasm`, `_doWasmAsyncify`, `_checkWasmSizes`), build.json, .github/workflows/release.yml, .github/actions/capabilities/wasm-build and wasm-cache, tool/ci/release_hooks.sh, test/guards/wasm_size_test.dart, test/harness/audited_asset_server.dart, the web runners in test/ops/runners/.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE
    make build-wasm DART=dart && make build-wasm-asyncify DART=dart
    one test file: dart test <path> ; one test: dart test <path> -N '<name>'
    the web runners: test/ops/runners/web_{opfs,jspi,atomics,asyncify}_runner_test.dart (see the Makefile's test-ops-* lines for the flags and platforms)

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code and the glossary in docs/ARCHITECTURE.md; a comment says only what the code cannot; no TODO; never silence a check; the engine fork (vendor/pdf_oxide) is not touched by this task.

- Write each test before its fix, and see it fail on the current code.
- Show each new test and wall can fail: break the code on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour (a file name, the auto order), change it only as this plan requires and list each such change in REPORT.md with the reason.

## The design (settled; do not reopen)

1. **Parts.** One constant, `wasmPartBytes = 16 * 1024 * 1024` (16 MiB), in lib/src/hook/build_constants.dart beside `maxWebAssetBytes`. A module `pdf_oxide_bg` (normal) or `pdf_oxide_asyncify_bg` (Asyncify) is cut into consecutive parts of exactly `wasmPartBytes`, the last one shorter, named `<module>.<n>.wasm` from 0: `pdf_oxide_bg.0.wasm`, `pdf_oxide_bg.1.wasm`. The `.wasm` extension is required: hosts pick the Content-Type, and with it compression, from the extension. Every module is cut, even one that would fit in one file, so every web mode runs the join path; today both modules come out as two parts each.
2. **The manifest.** `pdf_oxide_manifest.json` beside `executor_worker.js`, written by the same code that cuts the parts:
   `{"pdf_oxide_bg": {"bytes": N, "sha256": "<hex of the whole>", "parts": [{"file": "pdf_oxide_bg.0.wasm", "bytes": N0, "sha256": "<hex>"}, ...]}, "pdf_oxide_asyncify_bg": {...}}`.
   The Asyncify entry is present only when the app shipped the Asyncify wasm. The manifest is the only way the runtime learns which modules exist: no HEAD probe, no fetch-until-404. (A single-page-app host answers any unknown path with 200 and index.html, which is how today's `_asyncifyShipped` HEAD probe picks Asyncify on a page that never shipped it.)
3. **One function cuts and writes.** A Dart function in lib/src/hook/ (a new file `wasm_parts.dart`), given a built wasm file, a module name and an output directory, writes the parts and merges the module's entry into the manifest. Every producer calls it: `make build-wasm` / `make build-wasm-asyncify` (through tool/compile.dart), the release (`tool/compile.dart` with `COMPILE_OUTPUT_DIR`), and the hook's from-source build (`hook/build.dart` `_compileWasm`). The whole wasm is kept only where a build step needs it as input (the Asyncify build reads the normal wasm; keep it outside `web_assets/` or as a gitignored build file, your choice, say which); `web_assets/` holds the parts, the manifest, `pdf_oxide.js`, `executor_worker.js`.
4. **Release and hook.** build.json's web asset list, release.yml's verified list, tool/ci/release_hooks.sh and asset_hashes.dart name the parts and the manifest instead of the two whole files. The hook (lib/src/hook/resolver.dart and hook/build.dart) downloads or copies the parts and the manifest for a prebuilt (default) build, and cuts them for a from-source build. With `asyncify: false` the manifest has no Asyncify entry and no Asyncify part is shipped.
5. **One loader.** One JS function in web_assets/executor_worker.js, and its Dart twin in host.dart if the host keeps compiling on the main thread, takes a manifest entry and returns `WebAssembly.compileStreaming(new Response(stream, {headers: {'content-type': 'application/wasm'}}))`, where `stream` is a ReadableStream that fetches the parts in order (start every fetch at once; enqueue in order) and pipes each through a TransformStream that counts its bytes and checks its SHA-256 against the manifest before the stream ends (a mismatch or a short part errors the stream, so the compile rejects with a message naming the part). The host's compile (host.dart `_wasmModule`) and the worker's own fallback compile (when the host's compile failed: executor_worker.js near :357-376 and :399) both use it; nothing fetches a whole `.wasm` file any more. If both sides need the code, keep it in the worker script and have the host call the same logic through the worker protocol or duplicate it once with a test that pins both against the same manifest; say which in REPORT.md.
6. **The mode is decided asynchronously.** `Pdf.ioMode` is already null until initialized and `ensureInitialized()` already returns a Future (lib/src/runtime/router.dart:155-158). Move mode detection out of `WebExecutorBooter`'s constructor into its first async initialization: fetch the manifest once per page (cached per base URL), then decide. A missing or unparsable manifest (including an HTML answer) is a `PdfUnsupported` at initialization naming `pdf_oxide_manifest.json` and the web setup command. Keep `forceMode` and its typed errors (`_requireRunnable`): forcing Asyncify with no Asyncify entry in the manifest is the typed error.
7. **The auto order becomes JSPI, Atomics, OPFS, Asyncify.** The measurement in t-541 found Asyncify about 2.2 times slower than OPFS (OPFS's pre-copy included). OPFS is runnable when `navigator.storage.getDirectory()` resolves (await it; some private-browsing modes reject it), not when `navigator.storage` merely exists. Asyncify is picked only when OPFS is not runnable and the manifest lists the Asyncify module. With neither, initialization fails with `PdfUnsupported` naming both.
8. **The walls** (test/guards/wasm_size_test.dart, and the release check in tool/compile.dart `_checkWasmSizes`):
   - every file in `web_assets/` and in the release output is under 26,214,400 bytes;
   - for each manifest module, the parts concatenated are byte for byte the module (bytes and sha256 match the manifest, each part's hash matches its file);
   - a download budget per module, in gzip bytes of the whole module (Dart `gzip`, level 6 default): `pdf_oxide_bg` at most 8.5 MiB (8,912,896), `pdf_oxide_asyncify_bg` at most 10.5 MiB (11,010,048), as named constants in build_constants.dart with one comment line: raising one is a decision, not a fix. Today they measure 8,108,428 and 10,159,769.
   Show each wall failing once (a part over the cap with a temporary small cap, a corrupted part byte, a budget set below today's size), paste the failures, restore.

## Tests

- test/hook/: the cutter (a 40 MiB-and-one-byte input gives parts 16, 16, 8 MiB plus one byte with the right names and hashes; an input of exactly 16 MiB gives one part; the manifest merge keeps the other module's entry).
- Web (a battery registered in all four runners, beside test/ops/platform/web/web_asyncify_assets_battery.dart, using test/harness/audited_asset_server.dart): the page fetches the manifest once and the parts of exactly one module, never a whole `.wasm`; a corrupted part fails initialization with an error naming the part; a server that answers every unknown path with 200 and an HTML page (single-page-app mode) and a manifest without the Asyncify entry never selects Asyncify and never requests an Asyncify part; a missing manifest is the typed error.
- test/ops/runners/web_auto_mode_test.dart: update to the new order: with JSPI, JSPI; with plain chrome (no JSPI, no SAB), OPFS; with `navigator.storage.getDirectory` made to reject on the test page and the Asyncify entry present, Asyncify; with it rejecting and no Asyncify entry, the typed error.
- The four web runners and the native runner pass in full.

## Docs

- docs/ARCHITECTURE.md: the web assets (manifest, parts, the one loader, the order), the walls. README.md: the web setup's asset list and the self-hosting note (serve `.wasm` as `application/wasm` with compression on; a service worker that precaches assets reads the manifest), the auto order. CHANGELOG.pre.md: one Changed bullet in the 6.0.0-dev.0 entry in the shape its header comment defines (tool/check_changelog_shape.sh). The migration guide that tool/check_migration.sh checks: an entry for apps that list the old `pdf_oxide_bg.wasm` file (precache lists, hosting rules), in its existing shape.
- grep the repository (outside vendor/ and example/build) for `pdf_oxide_bg.wasm`, `pdf_oxide_asyncify_bg.wasm`, `asyncifyWasmFile`, `asyncifyWebAsset`: every reference either names the parts/manifest or is gone; list the files you changed for this in REPORT.md. The example app's web folder (example/web/pdf_manipulator/) is refreshed by the setup the way it is today.

## Gate

`dart analyze --fatal-infos lib hook tool test bin`; `dart format --set-exit-if-changed lib hook tool test bin`; `dart test test/guards test/runtime test/bridge test/keep test/hook`; the native runner; all four web runners; `web_auto_mode_test.dart`; `bash tool/check_changelog_shape.sh`; `bash tool/check_migration.sh`; `bash tool/check_migration_snippets.sh`. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: what you changed (file:line), each test, each wall and its injected failure with the failing message, the sizes (each module's parts, raw and gzip), the summary lines, and every choice the plan left open.
- `git add -N` new files, then `git diff HEAD -- . ':!vendor' ':!REPORT.md' ':!PLAN.md' > t-560-pkg.patch`. Generated wasm parts and the manifest are not in the patch (say how to rebuild them). Undo your working changes so only the patch carries them.
- Commit only `t-560-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-560`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-560`, and do not clone it.
