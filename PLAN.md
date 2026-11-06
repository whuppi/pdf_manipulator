You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-578, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

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
An encrypted full save never recompresses a stream over 256 KiB. `write_stream_copy` (vendor/pdf_oxide/src/editor/document_editor.rs near :3925) returns false for an encrypted output, and the staged path (near :8159) handles big staged streams only for unencrypted output, so `compress` and every full save of an encrypted document copy every large stream as it was. `write_pass_stream` (near :4214) already shows deflate into `h.stream_cipher(..).encrypting(writer)` with an indirect /Length written after.

File: vendor/pdf_oxide/src/editor/document_editor.rs (and engine proofs, tool/memprobe for rows).

1. `write_stream_body` (near :3973) serves encrypted output: the count pass works on the plain data as today; the streamed write deflates into the encrypting writer for that object, /Length indirect, written after. The in-memory case (at most MEMORY_STREAM) encrypts its re-encoded bytes the way the serializer encrypts any stream. `write_stream_copy` and the staged path stop excluding encrypted output.
2. Tests: a compress of an encrypted fixture with a large compressible stream (built in the proof) comes out smaller than today, opens with its password, and its stream decrypts and decodes to the same bytes (AES-256 and RC4 if the fork writes both). Fails first today (the size).
3. Memprobe: `compress encrypted` and `fullRewriteSave encrypted` stay under their limits.

## Gate

The Rust suite (as above); `make test-memory DART=dart` (every row); `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-578-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-578-*.patch' ':!REPORT.md' ':!PLAN.md' > t-578-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-578`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-578`, and do not clone it.
