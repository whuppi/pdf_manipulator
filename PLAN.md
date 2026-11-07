You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-579, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

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
`convertToPdfA` today ends with `commit_in_place` (vendor/pdf_oxide/src/editor/document_editor.rs near :1548-1554), which saves the whole document into a Vec and reopens it, then runs a final validation (src/compliance/converter.rs near :326-331) whose result the host discards (src/host/dispatch.rs near :1619). That is a copy of the whole file on every conversion. The memprobe row `convertToPdfA xmp64` (tool/memprobe/src/main.rs, read its comment) sits at 129 MiB because of it and the staged XMP packet.

Files: src/compliance/converter.rs, src/editor/document_editor.rs (commit_in_place and its callers only), src/host/dispatch.rs, tool/memprobe/src/main.rs, docs/ARCHITECTURE.md (one sentence).

1. On the host path, the conversion stages its edits like every other mutation (docs/ARCHITECTURE.md near :198): no `commit_in_place`, no whole-file save to a Vec, no final validation whose result is thrown away. Keep any public engine API that other callers use (grep the bindings and tests for commit_in_place and the converter's entry points); if a non-host caller needs the old behaviour, keep it for that caller and say so.
2. The PDF/A output must stay valid: the existing PDF/A tests and the Dart convertToPdfA batteries pass, and a converted file saved and opened again validates (use the engine's validator in a proof, the way the old final validation did, but outside the op).
3. Re-measure `convertToPdfA xmp64`. Its limit becomes the ceiling plus the packet, written as `16 * MIB + <packet bytes>` with the comment: "convertToPdfA holds the one metadata packet it patches, once, until the save writes it." Paste before and after.
4. docs/ARCHITECTURE.md "The promise": add that sentence.

## Gate

The Rust suite (as above); `make test-memory DART=dart` (every row); the PDF/A tests in nextest; `make test-ops-native DART=dart`; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-579-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-579-*.patch' ':!REPORT.md' ':!PLAN.md' > t-579-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-579`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-579`, and do not clone it.
