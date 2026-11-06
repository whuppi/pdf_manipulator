You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-576, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

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
Compression at level 9 is slow on repetitive page content. The fuel probe (tool/fuelprobe) measured a save that re-deflates one page of 150,000 identical `q 1 0 0 1 0 0 cm Q` triples (about 3 MB): 3.4e9 wasm instructions without deflate, 1.96e10 with it, about 5,000 instructions per byte. The engine deflates through flate2 with the zlib-rs backend (vendor/pdf_oxide/Cargo.toml near :161) at `host::deflate::zlib_level` (src/host/deflate.rs near :121: level 9 becomes 8 there; read why). Level 9 was chosen in an earlier change to match pdf_document's sizes on the issue #268 test files (see CHANGELOG.pre.md, the compress bullets).

Files: vendor/pdf_oxide/src/host/deflate.rs, vendor/pdf_oxide/Cargo.toml (flate2 features or version only), tool/fuelprobe/, tool/bench/ (read only, for sizes).

1. Measure, native and in the fuel probe's wasm: instructions (fuel) or ns per input byte for levels 6, 8 and 9 on three inputs: the repetitive page above, ordinary page text, and image data already compressed. Table it.
2. Find the cause in zlib-rs (or in our use: buffer sizes, flush calls, a write per operator into the encoder). The likely suspects are the lazy-match chain length at level 8/9 on input with long repeats, and many small `write` calls. Name it with evidence (a profile, or a change that removes it).
3. Fix at the cause. If it is our use (small writes, flushes), fix our use. If it is zlib-rs's level 9 path, choose the level by measurement: the lowest level whose output on the #268 corpus (tool/bench; read its README for how to run it, or measure the sizes of a level change on the fixtures it names) stays within 0.5% of today's sizes, and say the numbers. Never drop below today's sizes by more than 0.5% without saying so.
4. A fuel row that holds the cost: the repetitive page save at a size that fails today's cost bound with the slow path and passes with the fix.

## Gate

The Rust suite (as above); `make test-fuel DART=dart` (every row); the size table for the #268 files before and after; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-576-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-576-*.patch' ':!REPORT.md' ':!PLAN.md' > t-576-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-576`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-576`, and do not clone it.
