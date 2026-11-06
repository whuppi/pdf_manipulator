You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-541: the Asyncify web mode, approved by the user. Today on a browser with neither JSPI nor SharedArrayBuffer the engine runs in OPFS mode: every input file is first copied whole into the browser's private storage, then read from there. Asyncify (binaryen's `--asyncify`) lets the wasm pause at a host import and resume later, so the worker can await the main thread's read of the app's DataSource and the sink's write directly, like JSPI does, with no pre-copy. Read docs/ARCHITECTURE.md first (the runtime, the web modes, the cancel flag per mode, the executor worker), then lib/src/runtime/web/ (host.dart: `_detectMode` near :396; lease.dart; protocol.dart), web_assets/executor_worker.js (how imports are wrapped for JSPI with `WebAssembly.Suspending`/`promising`, `hostReadAt`, `hostWriteChunk`, `hostCancelled`, the OPFS path), lib/src/hook/ (pdf_config.dart parses the app's `hooks: user_defines: pdf_manipulator:` options: keep, build, detector, scan-dirs; engine_compiler.dart and tool/compile.dart build the wasm; build.json maps the web assets), tool/measure_wasm.dart, tool/versions.env (binaryen pin), the Makefile's build-wasm and test-ops-* targets, tool/ci/release_hooks.sh (the released web assets and their hashes).

## The approved design (decision d-126, web modes and the asyncify option)

- Modes: JSPI, Atomics, Asyncify (new), OPFS. Auto-detect: JSPI -> Atomics -> Asyncify (when shipped) -> OPFS.
- `asyncify: true|false` in the app's `hooks: user_defines: pdf_manipulator:` block (parsed by lib/src/hook/pdf_config.dart beside keep, build, detector, scan-dirs). Web only. Default false. True: the hook also downloads (full engine) or builds (trimmed keep) the Asyncify wasm. A browser fetches only the wasm its mode uses. The name is the established term (Emscripten -sASYNCIFY, binaryen --asyncify).
- Asyncify instruments only the functions that can be on the stack when the engine pauses (a precise list, kept true by a wall), so its wasm fits the 25 MiB per-file cap of Cloudflare Pages and Workers static assets. Never by switching to the size build.
- Release wall: every wasm we ship (normal and asyncify, full) is under 25 MiB. The normal wasm is about 19.6 MB today.
- `webIoMode` stays public as an override; forcing a mode the browser cannot run or the app did not ship is a typed error at startup, never a silent fallback.
- Later: measure Asyncify against OPFS on large files to decide which is the automatic fallback.

Measured earlier (a prototype): all-functions asyncify of the wasm is 26,677,030 bytes (+36%; +25% gzip) and 10-35% slower on CPU-heavy ops; `asyncify-ignore-indirect` (+0.5%) breaks the engine (`unreachable`). So the list of functions that may be on the stack at a pause must be computed, not guessed: binaryen's `asyncify-onlylist` (or `asyncify-removelist`) from a call-graph analysis of which functions can reach the suspending imports (`host_read_at`, `host_write_chunk`, and whatever else the worker awaits), including indirect calls through the tables the engine uses (trait objects: the readers and writers are `dyn Read + Seek` / `dyn Write`). If a sound list cannot be computed, report the exact obstacle before building anything else.

Cancel in this mode: the job's cancel flag is read by `hostCancelled` like the other modes; with Asyncify the worker can also unwind at the poll and observe a cancel message, but the per-job blob URL the worker already polls in a mode without SharedArrayBuffer is the simpler path: keep it unless Asyncify gives a reason to change.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown; install binaryen at the version tool/versions.env pins (or say which you used)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE; make build-wasm DART=dart
    the web runners: test/ops/runners/web_{opfs,jspi,atomics}_runner_test.dart with -p chrome (atomics: -p chrome-coi)

## The repository's rules

AGENTS.md: match the existing code and the glossary in docs/ARCHITECTURE.md; a comment says only what the code cannot; no TODO; never silence a check. Upstream files of the engine fork change only between patch markers. A new runner for the mode follows the existing three. Every new wall is shown to fail once (inject, run, paste, restore).

## The work, in order

1. The instrumentation list: a tool (tool/ beside measure_wasm.dart, or a binaryen pass driven by a script) that computes the functions that can be on the stack at a suspending import, from the built wasm; its output feeds `wasm-opt --asyncify --pass-arg=asyncify-onlylist@...`. A wall: the list is recomputed from the wasm on every build and the build refuses if a function reachable from a suspending import is missing (or the list is stale).
2. The Asyncify wasm: built by tool/compile.dart (and the hook, for the trimmed keep) as a second web asset beside the normal one, at the release optimisation level; its size reported. The release wall: a test that every shipped wasm (normal, asyncify) is under 25 MiB, in test/guards or the release hook's check.
3. The mode: `PdfIoMode.asyncify`; detection order JSPI -> Atomics -> Asyncify (only when the app shipped the Asyncify wasm) -> OPFS; the worker drives an Asyncify instance (start, unwind at a suspending import, await the read or write message, rewind) using the same protocol messages as JSPI for reads and writes; forcing a mode the browser cannot run or the app did not ship throws the typed startup error.
4. The hook option: `asyncify: true|false` in pdf_config.dart with validation in its style; the hook then fetches (full engine, from the release assets: add the asset to the release hook's list and build.json) or builds (trimmed keep) the Asyncify wasm; default false ships nothing new.
5. Tests: a web runner for Asyncify mode (plain chrome, which lacks SharedArrayBuffer; force the mode) running the same batteries as OPFS, all passing; the cancel-latency battery row for this mode; a test that a page without the Asyncify asset never fetches it, and that forcing asyncify without it is the typed error.
6. The measurement the design asks for: on a large generated file (hundreds of MB if the machine allows, else the largest that fits), time open + extract and merge in Asyncify mode against OPFS mode (OPFS includes its pre-copy). Report the numbers; do not change the automatic order based on them, the user decides.
7. Docs: ARCHITECTURE.md's web modes section and the README's web setup and options table: the mode, the option, the sizes, the trade-off in one plain paragraph. One Added bullet in CHANGELOG.pre.md's 6.0.0-dev.0 entry in the shape its header comment defines (tool/check_changelog_shape.sh).

## Gate

`dart analyze --fatal-infos lib test`; `dart test test/guards test/runtime test/bridge test/keep test/hook` (whatever hook tests exist); the native runner; all four web runners (OPFS, JSPI, Atomics, Asyncify); the size wall; `bash tool/check_changelog_shape.sh`. If the engine fork changed: `cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts"`, `bash tool/check_patch_markers.sh`, `bash tool/check_fork_tests.sh`.

## Done

- REPORT.md at the repository root: the instrumentation list method and its size, the Asyncify wasm size against the normal one (raw and gzip), each change (file:line), each wall and its injected failure, the measurement table, each gate line, every choice the plan left open, and anything you could not build and why.
- If the engine fork changed: `git -C vendor/pdf_oxide add -N .`, `git -C vendor/pdf_oxide diff HEAD > t-541-fork.patch`. Then `git add -N` new files and `git diff HEAD -- . ':!vendor' ':!t-541-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-541-pkg.patch`. Generated wasm files are not in the patch (say how to rebuild them). Undo your working changes so only the patches carry them.
- Commit only the patch files and REPORT.md.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then asks you to push; when it does, follow its message.
