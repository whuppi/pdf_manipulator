You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-575: make the whole-buffer lint green and turn it on. Approved for 6.0.0 (decision d-132, quoted below).

The engine (vendor/pdf_oxide) promises bounded memory (docs/ARCHITECTURE.md "Memory"). `tool/check_whole_buffers.sh` (patterns in tool/whole_buffer_patterns.txt, allowed sites in tool/whole_buffer_allow.txt; `--sites` lists every site) flags each engine call that reads a whole stream, file or page. Four other tasks have already fixed every lane-reachable site that is not a table. What is left is the classification below. Run `bash tool/check_whole_buffers.sh --sites` first and list what it still flags; any flagged site not covered by the rules below is a finding: report it with file:line and stop on it, never allow it under a wrong class.

## The rules (d-132)

1. Three new table kinds the allow list accepts under `# table:`: "PostScript calculator function" (a Type 4 function's program, evaluated per sample, O(its own length): mesh_shading.rs, page_renderer.rs, resolution/color.rs, resolution/pipeline.rs), "JBIG2 stream and globals" (hayro-jbig2 takes `&[u8]`; extractors/images.rs), "JPX codestream" (OpenJPEG takes `&[u8]`; extractors/images.rs). Add the kinds to the script's list of accepted kinds and the allow lines for those sites, each with its why.
2. No `bounded:` class. A bound is a `.take(N)` in the code, which the script already treats as a window.
3. One new class, `# unreached: <why>`, which the script proves rather than trusts; a line whose proof fails fails the build. A site passes as unreached by one of three proofs, checked by the script:
   - it is inside a `#[cfg(feature = "x")]` item or file (or a `cfg(not(target_arch = "wasm32"))`-style gate is not enough) where `x` is in neither feature string of build.json (the `features` entries the package builds the engine with): the bindings (`ffi` behind `public-api`? check: if `public-api` is in build.json's features, ffi.rs is NOT unreached by this proof and needs the third proof), python, ocr, bin;
   - the enclosing function's signature takes a `Path` or `&Path`/`PathBuf` (a lane holds no path, only a `DataSource`);
   - the enclosing function's name appears in no file under vendor/pdf_oxide/src/host and in no file under lib/ (the wasm exports, api/, the separation renderer's entry points), checked transitively one level: a caller that is itself reachable from src/host fails the proof.
   Sites ruled unreached: the separation renderer (rendering/separation_renderer.rs: its plates move after 6.0.0), the bindings and CLI (ffi.rs, python.rs, bin/), the builder API (api/pdf_builder.rs), OCR (ocr/), the office path entries (converters/office/ mod.rs path functions, docx.rs, pptx.rs), extractors/auto.rs, `open(path)` (document.rs), `merge_from(path)` (document_editor.rs), `from_file` (writer/font_manager.rs or image_handler.rs), `split_by_bookmarks_to_dir` (split_bookmarks.rs), `Pdf::to_bytes` / wasm.rs exports. Each gets its line with the proof the script ran.
3b. For the third proof, a caller inside a `#[cfg(test)]` module or under src/host/engine_proofs/ is not a caller. Task t-582 runs beside you and moves the DCT arm's whole read (`decode_stream_data` in `extract_image_from_xobject_at`, src/extractors/images.rs near :1291) into a function only `extract_image_from_xobject` calls: leave that one site out of the allow list, name it in REPORT.md, and the lint may stay red on exactly that site; the assistant adds its line when both land. The fax site and `decode_slice` already changed (t-580): its seven callers are new sites; classify them by the rules or stop on them.
4. Delete `whole_reads.rs` and `whole_reads.txt` (find them; two walls for one rule) and their references; docs/ARCHITECTURE.md "Memory" (near :566-584) describes the one lint: what a site is, the kinds, the `take` window, the `unreached:` proofs.
5. Decision d-133 adds: the remaining whole reads of a returned JPEG (`save_as_jpeg`, `to_base64_data_uri`, ffi.rs, separation_renderer.rs) and the `Decoder::decode` callers of `decode_slice` are `unreached:` by the third proof. docs/ARCHITECTURE.md near :580 drops the clause naming `engine_proofs/whole_reads.txt`, and near :690 the office row says what is true: the package stays in its source and office_oxide holds the whole document model. "The promise" and README.md (near :333) gain: "Converting an Office file to PDF is the one operation outside the ceiling: office_oxide parses the whole document into its model, so the conversion costs that model, in proportion to the file. Nothing is skipped or refused at any size." Add memprobe rows `convertToPdf docx64` and `pptx64` (a stored-ZIP package holding one 64 MiB JPEG; the docx also 20,000 paragraphs) at the measured peak, rounded up to the next MiB, with that sentence as their comment, in the shape the `hugeJbig2` row uses.
6. Wire the lint into `make test-guards` after `check_cancel_polls.sh`, and add it to the Makefile's guard comment list in the same style as the others.

## Setup

    curl -fsSLo /tmp/dart.zip https://storage.googleapis.com/dart-archive/channels/stable/release/3.13.4/sdk/dartsdk-linux-x64-release.zip
    unzip -q /tmp/dart.zip -d /opt && export PATH=/opt/dart-sdk/bin:$PATH
    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    dart pub get --no-example && dart tool/generate_fixtures.dart
    bash tool/check_whole_buffers.sh --sites ; make test-guards DART=dart

## The repository's rules

From AGENTS.md: match the existing code; a comment says only what the code cannot; no TODO; never silence a check.

- Show each new rule failing once: an `unreached:` line on a reachable site (pick one fixed site and point a line at a host function) fails with its proof's message; a `table:` line with an unknown kind fails; a stale line fails. Paste each, restore, confirm with `git diff`.

## Gate

`bash tool/check_whole_buffers.sh` exit 0 with its summary line, or red on exactly the t-582 site of rule 3b; `make test-guards DART=dart`; `bash tool/check_patch_markers.sh`; `bash tool/check_fork_tests.sh`. If the Rust tree changed (deleting whole_reads.rs), the Rust suite: `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --lib --features "icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts"`. Paste every summary line into REPORT.md.

## Done

- Write `REPORT.md` at the repository root: the sites `--sites` listed before you started, each classification with its proof, each injected failure and its message, any finding you stopped on, the summary lines, and every choice you made where the plan left one open.
- If the fork changed: `git -C vendor/pdf_oxide add -N .` then `git -C vendor/pdf_oxide diff HEAD > t-575-fork.patch` at the repository root; do not commit inside it.
- `git add -N` new package files, then `git diff HEAD -- . ':!vendor' ':!t-575-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-575-pkg.patch`. Undo your working changes in both trees so only the patches carry them.
- Commit only the patch files and `REPORT.md`, in the repository itself.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then attaches the repository and asks you to push to the branch `cloud/t-575`; when it does, add the attached repository as `origin` if your checkout has none, push with `git push origin HEAD:cloud/t-575`, and do not clone it.
