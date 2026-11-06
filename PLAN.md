You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-562: measure the web read path, so the automatic mode order and the read window rest on numbers. Approved by the user. This task measures; it changes no behaviour. The only lasting change is the measurement test itself.

Read first: docs/ARCHITECTURE.md (the four web modes: JSPI, Atomics, Asyncify, OPFS; the executor worker), test/ops/runners/web_io_measure_test.dart (the existing measurement: Asyncify against OPFS, an in-memory `TestSource`), test/harness/test_source_sink.dart, lib/src/io/ (the web sources; find the one backed by a browser `Blob`/`File`, or the closest), web_assets/executor_worker.js (`hostReadAt`, the OPFS pre-copy), vendor/pdf_oxide/src/host/wasm/js_reader.rs (the read window: one `READ_BUF_CAPACITY` chunk per miss, 64 KiB, from vendor/pdf_oxide/src/host/constants.rs), the Makefile's test-ops-jspi, test-ops-atomics, test-ops-opfs, test-ops-asyncify lines (their platforms and flags: JSPI needs the browser flags that runner uses; Atomics needs `chrome-coi`).

Known from t-541 (4-core machine, headless Chrome 153, in-memory source, 50 pages extracted, merge of two copies): Asyncify 28-31 s against OPFS 13-15 s on 105 MB (OPFS's pre-copy included); 93-95 s against 41 s on 316 MB; Asyncify does 7,054 source reads on 105 MB, about 4 ms each.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE
    make build-wasm DART=dart && make build-wasm-asyncify DART=dart
    the measurement: dart test test/ops/runners/web_io_measure_test.dart -p <platform> --dart2js-args=-DMEASURE_MB=<n> (see its header)

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. The engine fork (vendor/pdf_oxide) and lib/ are not changed by this task; any window change is a local experiment, reverted before you commit.

## The work

1. Extend web_io_measure_test.dart:
   - the modes come from a define (`-DMEASURE_MODES=jspi,opfs,...`, default all four), each run twice, alternating, as today; a mode the platform cannot run is skipped with a reason, never failed;
   - two source kinds, from a define (`-DMEASURE_SOURCE=memory|blob`): today's in-memory `TestSource`, and a source backed by a browser `Blob` built from the same bytes, read through the package's own Blob/File source if one exists (else the narrowest `DataSource` over `Blob.slice().arrayBuffer()`, say which);
   - each `MEASURE` line also prints the source kind and the read count for every mode.
2. Measure on the 105 MB and the 316 MB files, both source kinds:
   - JSPI, OPFS and Asyncify on the platform each needs (JSPI with the JSPI runner's flags; OPFS and Asyncify on plain chrome, which has neither JSPI nor SharedArrayBuffer, forced with `webIoMode`); Atomics on `chrome-coi`.
   - Record `nproc`, the Chrome version, and `uptime` before each run.
3. The window experiment (local, reverted): set `READ_BUF_CAPACITY` in vendor/pdf_oxide/src/host/constants.rs to 256 KiB, then 1 MiB; for each, `make build-wasm DART=dart && make build-wasm-asyncify DART=dart`, then measure Asyncify and JSPI on the 105 MB file, Blob source. Before you change it, find every other user of `READ_BUF_CAPACITY` (native included) and write in REPORT.md what a larger value would cost per open source in memory (the d-130 memory promise in docs/ARCHITECTURE.md "Memory"). Restore the constant and rebuild; `git -C vendor/pdf_oxide diff` must be empty at the end.
4. A table in REPORT.md: rows = mode x source kind x file size (and window, for step 3); columns = open+extract ms (both runs), merge ms (both runs), reads. Then two short paragraphs of what the numbers say, with no recommendation beyond them: (a) the order of JSPI, Atomics, OPFS, Asyncify by these numbers; (b) whether a larger window pays in Asyncify and JSPI and at what memory cost.

## Gate

`dart analyze --fatal-infos lib hook tool test bin`; `dart format --set-exit-if-changed test`; the measurement test with no `MEASURE_MB` skips cleanly on plain chrome. Paste the summary lines into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the table, the two paragraphs, the machine facts, every choice the plan left open, and the summary lines.
- `git diff HEAD -- test ':!REPORT.md' ':!PLAN.md' > t-562-pkg.patch`. Undo your working changes so only the patch carries them.
- Commit only `t-562-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-562`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-562`, and do not clone it.
