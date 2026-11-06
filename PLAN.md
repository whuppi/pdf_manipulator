You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-572: part of the whole-buffer work of 6.0.0. The engine (vendor/pdf_oxide) promises bounded memory (docs/ARCHITECTURE.md "Memory"): no op holds a whole stream, file or page, except tables by nature. A lint, `tool/check_whole_buffers.sh` (patterns in tool/whole_buffer_patterns.txt, allowed sites in tool/whole_buffer_allow.txt), flags every whole read; run it with `--sites` to list them. It is red today on 59 sites. An outside review ruled each one; your share is below. Other workers fix the other shares at the same time on other files; do not touch files outside your share, and do not edit the lint or its allow list (another task makes it green when every share has landed).

Read first: docs/ARCHITECTURE.md "Memory", tool/check_whole_buffers.sh (its header: what a site is, the `.take(` window rule), the files of your share, and how a bounded read is done elsewhere in the engine (`stream_reader()`, `raw_reader()`, `page_operators`, `form_operators`, `Operators`, `image_embed::take`, the memprobe rows in tool/memprobe/).

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    rustup target add wasm32-unknown-unknown wasm32-wasip1; cargo install cargo-nextest --locked (if missing)
    dart pub get --no-example && dart tool/generate_fixtures.dart
    Rust: CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --lib --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts" (debug info off and --lib: the full build fills the disk)
    make test-memory DART=dart (memprobe rows; read the Makefile for one row) ; make test-fuel DART=dart ; make test-ops-native DART=dart

The first build takes minutes. Run commands in the foreground and read the summary line.

## The repository's rules

From AGENTS.md: match the existing code and the glossary in docs/ARCHITECTURE.md; a comment says only what the code cannot; no TODO; never silence a check. vendor/pdf_oxide is a fork: upstream files change only between patch markers (tool/check_patch_markers.sh); src/host/ is the package's own; fork tests go where tool/check_fork_tests.sh expects.

- Write each test (or memprobe row) before its fix, and see it fail on the current code.
- Show each new test can fail: break the fix on purpose, run, see the failure, restore, and confirm with `git diff` that nothing injected is left.
- Never edit an existing test to make it pass. When an existing test pins the old behaviour, stop and write that in REPORT.md instead.

## Your share
File: vendor/pdf_oxide/src/editor/document_editor.rs (and tool/memprobe for rows).

1. `write_stream_copy` (near :3965) reads a stream whole when `data_len <= MEMORY_STREAM` (256 KiB): write it as `raw_reader()?.take(MEMORY_STREAM)` into the Vec, so the bound is in the code (the lint's `.take(` window rule then sees it).
2. `deflate_stream` (near :5144) reads a staged stream whole with no size guard at all: give it the `write_stream_copy` shape: count first, then a streamed recode (Flate in, Flate out, in pieces).
3. `get_page_images` (near :12239 and :12247) concatenates a page's content whole: walk it through `page_operators` as the other page walks do.
Memprobe: show `compress` and `saveFull` rows bounded with a huge staged stream (add a row if none covers `deflate_stream`; say which).

## Gate

The Rust suite (as above); `make test-memory DART=dart` (the `compress` and `saveFull` rows, and any row you add); `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`; `bash tool/check_whole_buffers.sh --sites` before and after (your sites leave the list; paste both counts). One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: per site, what it read whole, the change (file:line), its test or row and the failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-572-fork.patch` at the repository root. Do not commit inside vendor/pdf_oxide.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-572-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-572-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-572`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-572`, and do not clone it.
