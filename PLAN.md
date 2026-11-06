You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-577, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

Read first: docs/ARCHITECTURE.md (the engine, I/O, cancel, "Memory"), AGENTS.md, and the files of your share.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown wasm32-wasip1; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    Rust: CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --lib --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" (debug info off and --lib: the full build fills the disk)
    make test-memory DART=dart ; make test-fuel DART=dart ; make test-ops-native DART=dart (read the Makefile for single rows and runners)

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); src/host/ is the package's own; fork tests go where tool/check_fork_tests.sh expects.

- Write each test (or memprobe or fuel row) before its fix, and see it fail on the current code.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## The work
When a source fails while a document opens, the error is `PdfCorrupted` (the engine's message contains "host_read_at failed"), not `PdfIoFailed`; a caller cannot tell a broken file from a broken source. And the engine retries a failed read, so one I/O error from the host is absorbed: a test had to make every read fail to see an error at all. Found with a web `BlobSource` whose read throws; check whether native behaves the same.

Files: vendor/pdf_oxide/src/host/wasm/js_reader.rs, vendor/pdf_oxide/src/host/native/ (the callback reader), vendor/pdf_oxide/src/host/bridge_api.rs (the error mapping), vendor/pdf_oxide/src/host/binary_codec.rs (`kind_of`), the engine's open path (document.rs: wherever a read error during open, xref repair or recovery becomes a parse error), lib/src/bridge/ (decoding of error kinds), test/ops/core/ (batteries).

1. Find the retry (file:line): who retries a failed read, and why (a repair scan? a `BufReader` that reads again? an `Interrupted` kind?). Keep a retry only where a host can fail transiently and the contract says so; otherwise a read error ends the op.
2. Find where a read error at open becomes `PdfCorrupted`: likely a parse or repair path that turns any error into "corrupt". An I/O error from the source must surface as the I/O kind (`ErrorKind::Io` -> `PdfIoFailed`) at every point of every op; a parse error stays `PdfCorrupted`.
3. Tests, in a Dart battery registered in the native runner and the four web runners: a source whose first read fails makes `open` fail with `PdfIoFailed`; a source whose read fails once mid-op makes that op fail with `PdfIoFailed` (not succeed); a truly corrupt file still gives `PdfCorrupted`. A Rust test for the retry decision.

## Gate

The Rust suite (as above); the four web runners (`make test-ops-opfs test-ops-jspi test-ops-atomics test-ops-asyncify DART=dart`, after `make build-wasm build-wasm-asyncify DART=dart`); `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-577-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-577-*.patch' ':!REPORT.md' ':!PLAN.md' > t-577-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-577`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-577`, and do not clone it.
