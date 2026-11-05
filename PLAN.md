You work alone in this repository. Everything you need is in this file and the code. Do exactly this plan; where it does not settle a choice, take the narrower one and say so in REPORT.md.

Task t-444: builder image inputs stream from their source. pdf_manipulator 6.0.0 promises engine memory under a fixed per-op bound (16 MiB over baseline, tool/memprobe) for any input. Building a PDF from images (`Pdf.imagesToPdf`, `PdfBuilder.addImagePage`, `PdfPageBuilder.image`) takes each image as a DataSource; today the engine reads an image whole into memory to embed it (find where: the builder's image path in vendor/pdf_oxide, from the host op the Dart builder calls). Make a JPEG and a PNG of any size embed in bounded memory: a JPEG is embedded as DCTDecode by copying its bytes from the source to the output in chunks (no decode; read its dimensions and components from the SOF marker by scanning only the header); a PNG is decoded row by row from the source (its IDAT chunks inflated as a stream) and written as a Flate stream row by row, its alpha channel written as an SMask in a second pass over the source, never the whole image in memory. Engine fork plus the host op and tool/memprobe; Dart only if the op's arguments must change (avoid it).

## Setup

    git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide vendor/tiny_skia
    cargo install cargo-nextest --locked; rustup target add wasm32-wasip1 wasm32-unknown-unknown
    F=icc,legacy-crypto,rendering,signatures,native-bridge,pdfa,office,extract,test-support,public-api,cjk-form-fonts
    cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "$F"
    CARGO_TARGET_DIR=target/memprobe cargo test --release --manifest-path tool/memprobe/Cargo.toml
    CARGO_TARGET_DIR=target/memprobe cargo run --release --manifest-path tool/memprobe/Cargo.toml
    the fuel wall: the Makefile's test-fuel commands

## The repository's rules

Upstream files change only between `// ── pdf_manipulator patch: <why> ──` and `// ── end pdf_manipulator patch ──`; src/host/ is ours. Proofs in src/host/engine_proofs/, `///` directly above `#[test]`. Every loop over a document-scale collection (here: the image's rows, chunks) polls the cancel check (src/cancel.rs `poll`, `Stride::job`); `bash tool/check_cancel_polls.sh`. Never raise a memprobe limit. The output must decode to the same pixels as today's embedding (a PNG's rows, its alpha; a JPEG's bytes byte for byte). Show each proof fails before its change. Names follow the repo's glossary: read docs/ and the code's existing names (EngineState, executor, lease) and match them.

## The work

1. Find today's image path from the host op to the PDF image XObject; write in REPORT.md where the image is held whole and how big.
2. JPEG: header scan for width, height, components, bits; the stream copied in chunks; Adobe APP14 and CMYK inversion handled as today's code does (read it); progressive JPEGs embed as today (DCTDecode supports them).
3. PNG: streaming inflate of the concatenated IDAT data, unfiltering row by row (keep only the previous row), writing the colour rows to a Flate stream; palette and transparency (tRNS) and 16-bit handled as today's code does; alpha written in a second pass as an SMask. Interlaced (Adam7) PNG: if today's code supports it, keep it working; if it needs the whole image, say so in REPORT.md and leave it as is.
4. Proofs: a JPEG and a PNG (with alpha) built in code, embedded, read back: same bytes (JPEG) and same pixels (PNG, alpha included) as today's path; peak memory per image bounded (the counting allocator, src/host/heap.rs) and independent of the image's height.
5. memprobe rows: `build imagesToPdf` on a 64 MiB PNG and a 64 MiB JPEG (generated in the probe's style), each under its limit; one more row at twice the size of each, so the slope shows.
6. Fuel cases for the two builds if the wall's style allows (a long image is a document-scale loop); each passes MAX_GAP.

## Gate

nextest (full, with $F), the memprobe (every row), the fuel wall, check_patch_markers, check_fork_tests, check_cancel_polls, and the wasm lib check (`cargo check --manifest-path vendor/pdf_oxide/Cargo.toml --lib --target wasm32-unknown-unknown --no-default-features --features wasm,icc,legacy-crypto,rendering,signatures,pdfa,office,extract`).

## Done

- REPORT.md at the repository root: where the image was held whole, each change (file:line), each proof and its failure before the change, the memprobe rows before and after, each gate line, every choice the plan left open.
- `git -C vendor/pdf_oxide add -N .`, `git -C vendor/pdf_oxide diff HEAD > t-444-fork.patch`; `git add -N` new files, `git diff HEAD -- . ':!vendor' ':!t-444-fork.patch' ':!REPORT.md' ':!PLAN.md' > t-444-pkg.patch`. Undo your working changes so only the patches carry them.
- Commit only the patch files and REPORT.md.
- You have no git remote, so do not try to push. End your last reply with the commit hash and the words "not pushed". The assistant then asks you to push; when it does, follow its message.
