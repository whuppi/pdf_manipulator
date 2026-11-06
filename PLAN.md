You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-542: the web and native runtime's remaining correctness edges (design d-126, step 8). Approved for 6.0.0. Each item is a bug fixed at its cause with a test that fails on the current code; the tests are deterministic (an injected delay, a patched browser API in a blob-bootstrapped worker, a fake executor), never machine load.

Read first: docs/ARCHITECTURE.md (the runtime: router, lanes, the executor pool, leases, the web modes, the cancel outcome contract, "Memory": "bounded on every path"), lib/src/runtime/ (router.dart, pool.dart, lane.dart), lib/src/runtime/web/ (host.dart, lease.dart, protocol.dart), web_assets/executor_worker.js (the OPFS output path: `hostWriteChunk` and the code near :300-330 that posts chunks with no answer; `readFromOpfs`), lib/src/runtime/native/ and vendor/pdf_oxide/src/host/native/ (executor_table.rs: the budget waiters), lib/src/ops/handle_backstop.dart (the finalizer handles already have), test/ops/core/pdf_abort_battery.dart (the group "an op whose last write fails fails and aborts its sink once", which skips OPFS today with the reason).

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE
    make build-wasm DART=dart && make build-wasm-asyncify DART=dart
    the runners: make test-ops-native / test-ops-opfs / test-ops-jspi / test-ops-atomics / test-ops-asyncify DART=dart (read the Makefile for single-file commands)
    Rust: CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts"

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code and the glossary in docs/ARCHITECTURE.md; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers; src/host/ is the package's own.

- Write each test before its fix, and see it fail on the current code.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead. The OPFS skip in pdf_abort_battery.dart is the one you remove (item 1 makes it pass).

## The items

1. **OPFS output: a sink's error reaches the op, and output is bounded.** In OPFS mode the worker has no blocking primitive, so it posts output chunks and continues; the main thread records a sink error and drops it (lib/src/runtime/web/lease.dart `_serveChunk`, comment "OPFS mode: fire-and-forget writes, no ack channel"). So `builder.save`, `editor.save` and `extractAttachment` with a throwing sink succeed in OPFS mode, and a slow sink grows a main-thread queue with no bound. The fix is the design's OPFS output spool: in OPFS mode the worker writes each sink's output to its own OPFS file through a sync access handle (bounded memory, the same directory and reclaim rules as the input pre-copy), the job's result is held until the main thread has streamed that file to the sink chunk by chunk, awaiting each write; a sink error or a cancel during that delivery makes the op fail or cancel per the cancel outcome contract, and the sink is aborted once. Delete the fire-and-forget path. Tests: the abort battery's group runs in OPFS (remove its skip); a sink that stalls one second per write holds at most one chunk in flight on the main thread (count it); the spool file is removed after delivery, after a failure and after a cancel (the directory count, as the existing OPFS tests count).
2. **The budget counts a dead worker until its lock releases** (d-126 edge case 10): find where the budget is charged and released for a web executor; a terminated worker must stop counting at termination, not when the browser releases its liveness lock. Test with a fake or a delayed lock release.
3. **A native lane killed while waiting for a budget slot** posts its jobs only when another thread exits (d-126 edge case 11, the native executor table's waiters): a waiter whose lane is ended is removed and its jobs complete cancelled at once. Test with the budget at one and a held slot.
4. **No finalizer for an undisposed `Pdf`** (edge case 13): find what a `Pdf` holds that outlives its handles (leases, preludes, registered fallback fonts, pool membership); attach a backstop in the style of handle_backstop.dart that releases it when the `Pdf` is collected undisposed, with the debug warning. Test as the handle backstop tests do.
5. **The blob URL of the cross-origin worker bootstrap is never revoked** (host.dart `_spawnWorker`, edge case 14): revoke it once the worker has started (on `booted`, and on a boot failure). Test: count `URL.createObjectURL` against `URL.revokeObjectURL` on a cross-origin boot (patch both on the test page).
6. **t-493**: web OPFS "stress doc render first page" once hung 60 s under load. After items 1-5, write a deterministic reproducer for the likely causes (a lost ack in the pre-copy, the reclaim backoff) with injected delays; if it passes, say which causes it covers; if it fails, fix it.

Not in this task: the router's scan of every pin after every job (edge case 17), a performance item for after 6.0.0.

## Gate

`dart analyze --fatal-infos lib hook test bin`; `dart format --set-exit-if-changed lib hook tool test bin`; `dart test test/guards test/runtime test/bridge test/keep test/hook`; the Rust suite if the fork changed; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; all five runners (native, OPFS, JSPI, Atomics, Asyncify); `bash tool/check_changelog_shape.sh`. One Fixed bullet per user-visible fix in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape that script checks. docs/ARCHITECTURE.md: the OPFS output section says how output is spooled and delivered. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: per item, the cause (file and line), the fix, its test, the injected failure and its message; item 6's outcome; the summary lines; every choice you made where the plan left one open.
- If the fork changed: `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-542-fork.patch` at the repository root; do not commit inside it.
- `git add -N` new package files, then `git diff HEAD -- . ':!vendor' ':!t-542-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-542-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-542`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-542`, and do not clone it.
