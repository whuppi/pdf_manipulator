You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-561: an op's last output bytes are written by an explicit flush, never by a writer's Drop. Approved by the user.

The bug. Both host writers keep a fixed buffer and write it out when it fills, on `flush()`, and in `Drop`: vendor/pdf_oxide/src/host/wasm/js_writer.rs:90-94 (`JsCallbackWriter`, calls the `host_write_chunk` import) and vendor/pdf_oxide/src/host/native/callback_writer.rs:141-145 (`CallbackWriter`). Drop ignores the result (`let _ = self.flush_pending()`). The sinks are owned by `handle_request` (vendor/pdf_oxide/src/host/bridge_api.rs:80-119) and dropped after the response is built, so when an op's final write fails or is cancelled, the response still says ok and the output is silently short. Nothing in the ops calls `flush()` on the sinks (grep `flush` under src/host/ops: none).

The second reason (web Asyncify mode, t-541): the drop glue of `JsCallbackWriter` calls the pausing import, and drop glue has the wasm type `(i32)->()`, shared with every drop in the module, so the Asyncify instrumentation list (lib/src/hook/asyncify_list.dart) must treat every dynamic drop as able to pause.

Read first: docs/ARCHITECTURE.md (the engine's I/O, cancel, the cancel outcome contract: an op that ends cancelled or failed has its sink aborted, so it holds no partial output), the two writer files, bridge_api.rs `handle_request` and `BoxedWriter` (:43-70), src/host/binary_codec.rs `ResponseWriter` (status byte: 1 ok, 2 cancelled, 0 error; `cancelled()`, `error_kind`), how ops map an io error from a sink to a cancelled or failed response (grep for `HOST_IO_CANCELLED`, "operation cancelled", and the cancel helpers in src/cancel.rs), and lib/src/hook/asyncify_list.dart plus tool/asyncify_list.dart.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    install google-chrome or chromium, set CHROME_EXECUTABLE; make build-wasm DART=dart
    Rust: cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" [-E 'test(<name>)']
    Dart: dart test <path> [-N '<name>']; the runners: test/ops/runners/ (see the Makefile's test-ops-* lines)

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); files under src/host/ are the package's own. A Rust test of the fork goes where tool/check_fork_tests.sh expects.

- Write each test before its fix, and see it fail on the current code.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## The work

1. In `handle_request`, after the op returns: if the response's status is ok, flush every sink in order. A flush error turns the response into the one the op would have produced for the same error from a write: cancelled (`ResponseWriter::cancelled()`) for the host's cancel error, otherwise the failed-write response the ops use (find it; do not invent a new kind). If the status is not ok, do not flush.
2. Both writers' `Drop` stop writing: they drop their buffered bytes. Add a `finished` (or equivalent) state the flush sets, so a writer dropped with bytes still buffered after an ok response is a bug the debug build catches (`debug_assert!`), while a writer dropped after a failed or cancelled op discards silently, as the cancel outcome contract wants. Keep `BoxedWriter` as it is unless the change needs it; say why if it does.
3. Tests:
   - Rust, native: an op whose final buffered bytes fail to write (a sink whose write fails, or a cancel landing at the final flush) ends with a failed or cancelled response, never ok; on the current code it returns ok. Use the test-support hooks the fork already has for sinks and cancel (grep `test_support`, `CallbackWriter` tests in callback_writer.rs) or the Dart native runner if that is the only way to drive it.
   - Dart, native runner and the web runners: a sink whose final write throws makes the op fail (or cancel) instead of succeeding; place it in the battery that already holds sink-failure cases (grep `throwing sink`, `FailingSink`, `abort` under test/ops).
   - Every existing test still passes: the Rust suite, the native runner, the four web runners (OPFS, JSPI, Atomics, Asyncify).
4. The measurement: before and after, run `make build-wasm DART=dart` then `dart --packages=.dart_tool/package_config.json tool/asyncify_list.dart web_assets/pdf_oxide_bg.wasm` and paste both outputs (counts and the per-signature lines). Then `make build-wasm-asyncify DART=dart` and report the Asyncify wasm size before and after. Do not change the list tool.
5. One Fixed bullet in CHANGELOG.pre.md's 6.0.0-dev.0 entry in the shape its header comment defines (tool/check_changelog_shape.sh): an op whose last write fails or is cancelled now reports it.

## Gate

`dart analyze --fatal-infos lib hook tool test bin`; `dart test test/guards test/runtime test/bridge test/keep test/hook` (the wasm size wall in test/guards may already fail on the Asyncify wasm's size; report its numbers, it is another task's); the Rust suite with the features above; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; the native runner; the four web runners; `bash tool/check_changelog_shape.sh`. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), what you changed, each test, each injected failure and its failing message, the list counts and Asyncify sizes before and after, the summary lines, and every choice you made where the plan left one open.
- In vendor/pdf_oxide: `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-561-fork.patch` (at the repository root). Do not commit inside it.
- `git add -N` new package files, then `git diff HEAD -- . ':!vendor' ':!t-561-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-561-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only `t-561-fork.patch`, `t-561-pkg.patch` and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-561`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-561`, and do not clone it.
