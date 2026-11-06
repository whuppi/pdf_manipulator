You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-564: on web, a source read past 4 GiB returns the wrong bytes. Approved by the user.

The bug. vendor/pdf_oxide/src/host/wasm/js_reader.rs:29 declares the import `fn host_read_at(source_index: u32, offset: u32, count: u32, buf_ptr: u32) -> i32`, and the reader passes `self.position as u32` (near :80). A position past 4 GiB wraps, so the host reads from the wrapped offset and the engine gets other bytes with no error. Source lengths already cross as f64 (vendor/pdf_oxide/src/host/bridge_api.rs:193-223); the read offset does not. On the JS side every path carries the offset as a number (web_assets/executor_worker.js `hostReadAt` near :188, `readViaAtomics` posts it, `readViaJspi`, the Asyncify path, `readFromOpfs` uses `SyncAccessHandle.read({at})`), so only the import's type truncates. Check that claim on every path, including the Dart main-thread answer (lib/src/runtime/web/lease.dart, the `readAt` handler) and any typed array that might hold an offset.

Read first: docs/ARCHITECTURE.md (the web runtime and modes), js_reader.rs, bridge_api.rs `execute`, executor_worker.js `hostReadAt` and the per-mode read functions, lib/src/runtime/web/lease.dart (read answers), test/harness/test_source_sink.dart, the web runners in test/ops/runners/.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE
    make build-wasm DART=dart && make build-wasm-asyncify DART=dart (rebuild both after any Rust change)
    Rust: cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts"
    Dart: dart test <path> [-N '<name>']; the runners: see the Makefile's test-ops-* lines for flags and platforms

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); src/host/ is the package's own.

- Write each test before its fix, and see it fail on the current code.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## The work

1. A test first, in a battery registered in the web runners: a `DataSource` of length 4 GiB + 64 MiB that holds no such buffer: it synthesizes its bytes on `readAt` from a small PDF placed so that its objects and xref sit past 4 GiB (bytes 0..n: the `%PDF-1.7` header; then filler that a PDF parser skips between the header and the first object, such as whitespace or a comment line repeated; then the objects, the xref with their true offsets past 4 GiB, the trailer). Open it and extract the text of its one page; the expected text proves the bytes came from past 4 GiB. On the current code it fails (wrong bytes or a parse error). Run it in JSPI, Atomics and Asyncify; in OPFS mode it would pre-copy 4 GiB to disk, so skip it there with that reason, and instead prove the OPFS path reads past 4 GiB with a smaller check of `readFromOpfs`'s offset handling if one can be written without the copy (else say so).
2. The fix: the offset crosses as f64, like the source lengths (wasm-bindgen maps f64 to a JS number exactly up to 2^53). Change the import and the call in js_reader.rs, and any JS or Dart step that narrows the offset. Keep `count` and `buf_ptr` as u32 (a window and a wasm address).
3. Look for the same narrowing on the write side (vendor/pdf_oxide/src/host/wasm/js_writer.rs, `host_write_chunk`, the OPFS output path in executor_worker.js) and any other web import that carries a file offset or size; fix each the same way with a test, or show in REPORT.md why it cannot overflow.
4. Every existing test still passes: the Rust suite, the native runner, the four web runners.
5. One Fixed bullet in CHANGELOG.pre.md's 6.0.0-dev.0 entry in the shape its header comment defines (tool/check_changelog_shape.sh).

## Gate

`dart analyze --fatal-infos lib hook tool test bin`; `dart format --set-exit-if-changed lib hook tool test bin`; `dart test test/guards test/runtime test/bridge test/keep test/hook`; the Rust suite; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; the native runner; the four web runners; `bash tool/check_changelog_shape.sh`. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), every narrowing you found and fixed, each test, each injected failure and its failing message, the summary lines, and every choice you made where the plan left one open.
- `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-564-fork.patch` at the repository root. Do not commit inside vendor/pdf_oxide.
- `git add -N` new package files, then `git diff HEAD -- . ':!vendor' ':!t-564-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-564-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only `t-564-fork.patch`, `t-564-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-564`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-564`, and do not clone it.
