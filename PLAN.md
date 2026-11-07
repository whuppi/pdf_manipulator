You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-580, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

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
A JPEG the engine returns as-is (no target, no reduction, a CMYK or YCCK JPEG) is read whole into `ImageData::Jpeg(Vec<u8>)` (vendor/pdf_oxide/src/extractors/images.rs near :1283-1291). Lanes reach it from `reduceImages` of a CMYK JPEG (src/host/images/execute.rs near :361) and from src/host/images/stream.rs near :650 when `decode_dct_covering` declines. A CCITT fax image is read whole at images.rs near :1298, reached when `fax_params` returns None (stream.rs near :633) and from inline images (src/redaction/transform.rs near :1322). extractImages' own pass-through already streams (src/host/images/extract_stream.rs near :216-233): copy its shape.

Files: src/extractors/images.rs, src/host/images/ (execute.rs, stream.rs), src/host/font_budget.rs, src/redaction/transform.rs (only the fax arm), tool/whole_buffer_patterns.txt and tool/whole_buffer_allow.txt (only the lines named below), tool/memprobe/.

1. `ImageData::Jpeg` carries the `StreamBody` (or a type that holds it) instead of the bytes. `to_dynamic_image_covering` (images.rs near :484-530) and `decode_cmyk_jpeg_to_raw_cmyk` (execute.rs near :365) open the stream's reader (`reader_in`, as extract_stream.rs does) and decode from it. `font_budget.rs` near :133 counts the body's length. The remaining whole reads, `save_as_jpeg` (near :311), `to_base64_data_uri` (near :353), ffi.rs near :2556 and separation_renderer.rs near :3006, are not on a lane path: list each in REPORT.md with how you checked (no reference from src/host or lib/).
2. Fax: `fax_params` returning None becomes a typed error in `ImageCursor::open` (an image the engine cannot read, not a whole read). The arm at images.rs near :1298 reads from the line decoder (`Fax::open`, stream.rs near :645), as the JBIG2 arm near :1236 reads from `decode_jbig2_bits`.
3. Add `\bdecode_slice\(` to tool/whole_buffer_patterns.txt (the lint then treats decode_slice's body as a helper and its callers as sites). Do not edit tests/.
4. Memprobe rows that fail first: `reduceImages cmykJpeg64` (a page with one 64 MiB CMYK JPEG), and an extract or render row over a 64 MiB fax image. Each under the 16 MiB ceiling after the fix. Output pixels unchanged: a proof per path compares with today's decode on a small image.
Run `bash tool/check_whole_buffers.sh --sites` before and after and paste both counts.

## Gate

The Rust suite (as above); `make test-memory DART=dart` (every row); `bash tool/check_whole_buffers.sh --sites`; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-580-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-580-*.patch' ':!REPORT.md' ':!PLAN.md' > t-580-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-580`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-580`, and do not clone it.
