You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-582, for pdf_manipulator 6.0.0. Other workers change other files at the same time; stay inside the files named below, and when the fix needs another file, stop and say which and why in REPORT.md.

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
A JPEG on a lane is read whole as its file when the DCT arm finds no reduction to make. In vendor/pdf_oxide/src/extractors/images.rs, `extract_image_from_xobject_at` (near :1282-1292) calls `decode_dct_covering` (near :2770) when a `target` is given; its `None` branches (4 components near :2775, no reduction near :2794, an unreadable header, another pixel format) fall to `ImageData::Jpeg(decode_stream_data)` near :1291. `target` means "the size it will be painted at" (src/host/images/stream.rs near :582). Lanes reach the hole from `CmykRows::open` (src/host/images/execute.rs near :360-373, reduceImages of a CMYK JPEG) and from `ImageRows`/`ImageCursor::open` (stream.rs near :650-692, render and reduce).

The rule: with a target, a DCT image never becomes `ImageData::Jpeg`. Without a target the library keeps returning the file (extract_images, ffi.rs, the CLI): `ImageData::Jpeg(Vec<u8>)`, `to_dynamic_image_covering`, `save_as_jpeg`, `to_base64_data_uri`, src/ffi.rs and src/rendering/separation_renderer.rs stay as they are.

Files: src/extractors/images.rs, src/redaction/transform.rs (read only, unless its inline arm must pass a target), src/host/images/execute.rs, src/host/images/stream.rs (only if the Raw path needs it), src/host/engine_proofs/extract_jpeg_window.rs, tool/memprobe/, docs/ARCHITECTURE.md (one sentence), CHANGELOG.pre.md.

1. The DCT arm with a target decodes from the stream (open the stream's reader as `decode_dct_covering` does; never `decode_stream_data`) at the covering reduction, or at 1:1 when none covers, into `ImageData::Raw { pixels, format }`. A 4-component JPEG decodes to Raw CMYK with the Adobe inversion undone (the logic of `cmyk_jpeg_samples_are_inverted` and the code near :2295-2336). That check scans bytes today: from a stream it walks the marker segments up to SOS, holding one segment at a time (64 KiB at most), then reopens the reader for the decoder. An unreadable header or a pixel format the arm does not name is `Error::Image`, never the whole-file fallback.
1b. The library's file stays at the library's entry, so the whole-buffer lint can prove the host never reaches it: `extract_image_from_xobject` (images.rs near :1046, the no-target wrapper) returns the file for a DCT image through a new function of its own (name it for what it returns, e.g. `jpeg_file`), and `extract_image_from_xobject_at` with `target: None` decodes a DCT image at 1:1 like the target case. Grep that no file under src/host outside engine_proofs/ and `#[cfg(test)]` modules calls `extract_image_from_xobject`. `redaction/transform.rs` near :1322 (inline images) keeps calling `_at`: say in REPORT.md what it now gets for an inline JPEG and that its tests pass. In extract_jpeg_window.rs the no-target assertion (near :81) moves to `extract_image_from_xobject`, since the contract moved there; that is the second and last existing-test change.
2. `CmykRows::open` (execute.rs) passes the target and reads Raw CMYK straight into `CmykRows::Raster`; its own `decode_cmyk_jpeg_to_raw_cmyk` call goes. stream.rs near :692 should need no change: `to_dynamic_image_covering` on Raw falls to `to_dynamic_image`, whose Raw-CMYK branch applies the ICC transform (images.rs near :635-638); confirm it and say so.
3. Before you write a memprobe limit, read the decoder crate's source in ~/.cargo/registry: say in REPORT.md whether the JPEG decoder you use reads its input whole internally. If it does, the stream decode must feed it a reader that does not buffer the file, or name the gap in REPORT.md and stop.
4. Existing test change, ruled in d-133 (the old behaviour was the hole): in src/host/engine_proofs/extract_jpeg_window.rs, `assert_eq!(held, target.0 >= 192)` (near :78) becomes `assert!(!held)`. The no-target assertion moves as item 1b says. No other existing test is edited; if one moves (a golden that pins exact bytes of a JPEG render pins the decoder, and a shift of 1 per channel is not a bug), stop and name it in REPORT.md.
5. New proofs, each failing first, in extract_jpeg_window.rs: `a_jpeg_painted_at_its_size_is_not_held_as_a_file` (a noise JPEG 2048x1536 at quality 100, its file larger than its raster; the thread grows by less than the raster plus 4 MiB) and `a_cmyk_jpeg_on_a_lane_is_not_held_as_a_file` (through `CmykRows::open` and through `ImageRows`/`ImageCursor::open`). A proof per path that the pixels equal today's decode on a small image (RGB at 1:1, RGB reduced, CMYK, CMYK with the Adobe inversion).
6. Memprobe rows that fail first: a huge JPEG painted at 1:1 (render) and a huge CMYK JPEG (render and reduceImages). Limit: the raster plus 16 MiB, written in the table with the comment "a decoder that returns rows" naming the target, the way the hugeJbig2 row names its.
7. docs/ARCHITECTURE.md "The promise", one sentence: "A codec's input is read whole only when its decoder cannot read a stream (JPEG 2000, JBIG2). A JPEG on a lane is decoded from its stream at the reduction that covers its placement, or at its size; the file is the image only when the library returns it as bytes."
8. CHANGELOG.pre.md 6.0.0-dev.0, one Fixed bullet: a JPEG rendered, reduced or routed as CMYK is decoded from its stream, and the file is no longer held beside the raster.
Run `bash tool/check_whole_buffers.sh --sites` before and after and paste both counts. Then run the integration tests too (`cargo nextest run` without `--lib`, same features, debug info off), since the fax change before you altered what extracted images look like.

## Gate

The Rust suite (as above); `make test-memory DART=dart` (every row); the integration tests (the last line of The work); `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`; `bash tool/check_cancel_polls.sh`; `make test-ops-native DART=dart`; `dart analyze --fatal-infos lib hook test bin`. One Fixed bullet per user-visible change in CHANGELOG.pre.md's 6.0.0-dev.0 entry, in the shape tool/check_changelog_shape.sh checks. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the cause (file and line), the change (file:line), each test or row and its failure before; each injected failure and its message; the summary lines; every choice you made where the plan left one open.
- In each submodule you changed (vendor/pdf_oxide, vendor/office_oxide): `git -C <it> add -N .` then `git -C <it> diff HEAD > t-582-<pdf_oxide|office_oxide>.patch` at the repository root. Do not commit inside a submodule.
- If package files changed: `git add -N` them, then `git diff HEAD -- . ':!vendor' ':!t-582-*.patch' ':!REPORT.md' ':!PLAN.md' > t-582-pkg.patch`. Undo your working changes in every tree so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-582`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-582`, and do not clone it.
