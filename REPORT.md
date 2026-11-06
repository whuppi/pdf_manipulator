# t-570 — convertToPdf (docx / pptx / xlsx) input memory

**Outcome: stopped after item 1, as PLAN.md's last line of "The work" directs.** Item 2 is a rewrite of the
document model, the IR and the PDF layout consumer, which is more than "the reader, the zip and the XML
parse". No code, test, memprobe row, fuel row or doc was changed. There are no `t-570-*.patch` files
because no tree changed; the only commit is this file.

## What is and is not true of the plan's premise

The reader path **already** keeps the input in its source. `Document::from_reader`
(`vendor/office_oxide/src/lib.rs:305`) → `from_reader_inner` (`:312`) → `DocxDocument::from_reader`
(`src/docx/mod.rs:159`) / `PptxDocument::from_reader` (`src/pptx/mod.rs:83`) / `XlsxDocument::from_reader`
→ `OpcReader::new` (`src/core/opc.rs:239`) → `ZipArchive::new(reader)` (`opc.rs:245`) over the caller's
`Read + Seek`. There is no `read_to_end` of the package and no `Cursor` over a `Vec` on that path. The host
passes its `BoxedReader` straight through (`vendor/pdf_oxide/src/host/ops/convert.rs:136-137` →
`host/dispatch.rs:1819` → `converters/office/mod.rs:179`). The only package-sized copies are the
fallbacks that have no reader: `convert.rs:144` (`bytes.to_vec()` when only `source_bytes` exists),
`convert.rs:155-166` (no sink: `read_all_from_reader`) and `dispatch.rs:1841` (`convert_from_format_to_bytes`).
The probe and the Dart bridge take the reader-and-sink path.

So `docs/ARCHITECTURE.md:690` ("whole input buffered (office_oxide reads the ZIP from an in-memory cursor)")
is wrong for the engine's path, and the real cost is elsewhere (below). I did not edit it: `docs/` is not in
this task's file list, and with item 2 not done there is no "true after the change" text to write. The
accurate line today would be "the package stays in its source; every part the document uses, the parsed
model and the IR are held whole (a 64 MiB image is held 2–3 times)".

## Measured, current code

A throwaway probe row (reverted; `git status` clean) wrote a stored-ZIP docx and pptx into the memprobe
temp dir and ran the wire op `convertToPdf` (`format`, `sourceLength`, one source file, one sink file)
through `step()` with the 16 MiB ceiling. Each package holds one 64 MiB noise JPEG (`generate_image_jpeg`,
`JPEG_ROWS`); the docx also has 20,000 paragraphs of 200 hex characters (4.3 MB of `document.xml`).

```
memprobe: scratch docx: 72205524 bytes
convertToPdf docx peak_over_baseline=566822459 limit=16777216 FAIL wall_ms=619
memprobe: scratch out 70412540 bytes
memprobe: scratch pptx: 67546187 bytes
convertToPdf pptx peak_over_baseline=404794541 limit=16777216 FAIL wall_ms=453
memprobe: scratch out 67545596 bytes
```

That is 540.6 MiB for a 68.9 MiB docx (7.8×) and 386.0 MiB for a 64.4 MiB pptx (6.0×); both conversions
succeed and write a PDF of about the input's size. The per-site figures below are from reading the code
(copies of the 64 MiB image part), not separate measurements; only the two totals above are measured.

## Item 1 — where the input is held whole (64 MiB image part, 20k-paragraph body)

| # | site | what is held | bytes for the 64 MiB input |
|---|------|--------------|----------------------------|
| 1 | `core/opc.rs:239-245` `OpcReader::new` | nothing of the package: `ZipArchive` over the caller's reader | 0 (central directory only) |
| 2 | `core/opc.rs:436-448` `central_directory_entry_count` | the last 22 + 64 KiB of the file | 64 KiB, freed |
| 3 | `core/opc.rs:475-535` `read_zip_entry` (`:518-519` `Vec::with_capacity(min(size,1 MiB))` + `take(MAX_PART_SIZE+1).read_to_end`) | every part read is a whole `Vec`; the Vec doubles from 1 MiB, so a 64 MiB part reaches a 64 MiB buffer after copying through 32 MiB → 64 MiB; cap `MAX_PART_SIZE` = 512 MiB (`:473`) | ≥ 64 MiB (up to ~96 MiB transient across a realloc) |
| 4 | `core/opc.rs:322-335` `read_part` → `core/xml.rs:620` `ensure_utf8` | a second copy of an XML part only when it is not UTF-8 | 0 for UTF-8 parts |
| 5 | `docx/mod.rs:222` `doc_data = read_part(main)`; `:226` `parse_document` | whole `document.xml`, then the whole `Body` tree of block elements | XML size + tree (~several × the text) |
| 6 | `docx/mod.rs:368-384` images map (`HashMap<rId, (Vec<u8>, ext)>`) | **every image part, whole, for the life of the `DocxDocument`** | 64 MiB |
| 7 | `convert_docx.rs:574` and `:697` `doc.images.get(..).cloned()` into `ir::Image.data` (`ir.rs:1126`, `Option<Vec<u8>>`) | a second whole copy of the image per use in the IR | 64 MiB |
| 8 | `docx/mod.rs:247,274,308,330` styles / numbering / header-footer / notes / fonts | each whole, then parsed to trees; embedded fonts kept whole in `embedded_fonts` (`:325-345`) and cloned by the converter (`pdf_oxide converters/office/mod.rs:~176`) | per part |
| 9 | `pptx/mod.rs:130-212` Phase 1 `bundles: Vec<SlideBundle>` | for **every** slide, at once: slide XML, notes, comments and **all of the slide's media bytes** (`:186` `read_part(target)` into `media`), before any slide is parsed | 64 MiB per image, all slides resident together |
| 10 | `pptx/slide.rs:329` / `convert_pptx.rs:244` `pic.data.clone()` | media bytes copied into the `Picture` shape, then again into the IR | +64 MiB, +64 MiB |
| 11 | `pptx/mod.rs:250` | `ppt/fonts/*` whole | per font |
| 12 | `xlsx/mod.rs:147,310,361-476,762` | shared-string table, styles, workbook, each worksheet's XML whole and its cell grid; drawing images read whole by `read_zip_entry` at `:762` | sheet XML + grid; 64 MiB per image |
| 13 | `lib.rs:403` `Document::to_ir` | the whole `DocumentIR` (all elements, all image bytes) | model + IR |
| 14 | `pdf_oxide converters/office/mod.rs:179-191,306,762` | `has_positional_layout(&ir)` scans the whole IR; `ir_to_pdf_writer(&ir, …)` / `render_positional_ir_writer` take `&DocumentIR` whole; images are drawn from `&img.data` at `:962,1929,2008,2716` (decode or pass-through, whole) | IR + the image once more in the page builder |

Peak = parts (3) + model (5, 6, 9) + IR clone (7, 10, 13) + layout/writer (14); a 64 MiB image appears
about three times in the docx and pptx paths, which accounts for most of the measured 540 / 386 MiB. The
pptx is lower only because its slide has no paragraph body.

### What by nature may stay whole, and what may not

- A single slide's XML or a single sheet's cell grid may stay whole (the layout needs the shape tree / grid).
- The package, a part's *bytes* and an image's bytes need not: rows 3, 6, 7, 9, 10 and 12 (media) hold
  bytes the output only copies or passes through (the output PDF was about the input's size).
- The paragraph body (5, 13, 14) is whole because the pipeline is parse → model → IR → layout, each stage
  over the whole document.

## Why item 2 is larger than the reader, the zip and the XML parse

The reader and zip are already right. What is left is not a change inside them:

1. **Images are owned bytes in a public model.** `DocxDocument.images` (`docx/mod.rs`), pptx
   `Picture.data`, xlsx `pictures`, and `ir::Image.data: Option<Vec<u8>>` (`ir.rs:1126`, `Serialize`/
   `Deserialize`) are public types. `Document` is also consumed by `ir_render.rs` (base64 embed),
   `ffi.rs`, `python.rs`, `wasm.rs`, `office_oxide_cli` and `office_oxide_mcp`. Making image bytes lazy means a
   new variant of that public type and a change in each consumer.
2. **A lazy image needs the archive to outlive the parse.** The `ZipArchive` would move into the `Document`
   (behind a lock, since pptx/xlsx parse slides/sheets in parallel, `core/parallel`), and the document's
   `Send + 'static` reader bound would then hold for the life of the IR, not the open.
3. **The PDF side holds the bytes again.** `converters/office/mod.rs` draws from a whole `&Vec<u8>`
   (`:962,1929,2008,2716`); a JPEG passes through, a PNG/other is decoded whole. Streaming them is a change
   to the image embed path, not to the converter's entry.
4. **The paragraph body is whole across three stages** (`Body` → `DocumentIR` → `ir_to_pdf_writer(&ir)`),
   and `has_positional_layout(&ir)` is a pre-scan over the whole IR. Replacing "build a tree, walk it once"
   with events means the converter (`convert_docx.rs`, 1,361 lines; `convert_pptx.rs`; `convert_xlsx.rs`) and
   the layout (`converters/office/mod.rs`, 2,972 lines) change to consume a stream of elements. That is a
   rewrite of the IR contract and the layout, not of the XML parse.

## Plan for the follow-up (not started)

1. **Image bytes by reference** (the larger win, two of three copies): `ir::Image` and the docx/pptx/xlsx
   image fields hold `{ part, len, format }` and a handle to the package; bytes are read on demand into a
   64 KiB window and written to the PDF stream. Serde/ffi/python/wasm/`ir_render` get a `read_image(&Image,
   &mut impl Write)` instead of a field. Pptx Phase 1 stops materialising media. `pdf_oxide` embeds a JPEG
   by copying the window (as `imagesToPdf` already does) and a PNG row by row (`imagesToPdf png64/128` rows
   are the model).
2. **Package kept in the Document**: `OpcReader` moved into `Arc<Mutex<…>>` inside `Document`, parts
   re-read per use; `read_zip_entry` gets a streaming sibling (`Read` over the entry, same
   `MAX_PART_SIZE` take and CRC-tolerance) for parts that are parsed once.
3. **XML**: `parse_document`, styles, numbering, notes and sheet parsing already use `quick_xml` events
   (`docx/mod.rs:89`, `make_content_reader`); change them to take `impl BufRead` over the entry stream
   instead of `&[u8]`. Parts the layout needs whole: one slide, one sheet's cells, styles/numbering/theme.
4. **Body streaming** (the part the plan's size rule excludes): `to_ir` and `ir_to_pdf_writer` over an
   iterator of elements, with `has_positional_layout` decided from a first cheap pass. Without this, the
   20k-paragraph docx stays at (document.xml + tree + IR + layout) and its row's limit is the ceiling
   plus that, named in `limit()`.
5. Then the memprobe rows (`convertToPdf docx`, `convertToPdf pptx`; the generator I used is described
   above: stored ZIP, 64 MiB JPEG from `generate_image_jpeg`, 20k paragraphs), `docs/ARCHITECTURE.md:690`,
   and one `Fixed` bullet in `CHANGELOG.pre.md` for 6.0.0-dev.0.

I would split 1+2 (images and package; no layout change; expected to take the 540 / 386 MiB to roughly
the body size plus a window) from 3+4 (layout), as separate tasks.

## Choices where the plan left one open

- The plan's "stop after item 1" was taken at the narrower reading: no rows were committed, because a row
  that fails on the current code and has no fix would turn `make test-memory` red. The two numbers above
  come from a probe I reverted, so they can be reproduced but are not in the tree.
- `docs/ARCHITECTURE.md:690` is left as is (not in this task's files; nothing is true "after the change").
- No attribution trailer in the commit: `AGENTS.md` ("No AI co-author lines in commits") overrides the
  session default.

## Gate

Not run. No source, test, doc or changelog file changed, so the Rust suite, `make test-memory`,
`tool/check_patch_markers.sh`, `tool/check_fork_tests.sh`, `tool/check_cancel_polls.sh`,
`make test-ops-native` and `dart analyze` have nothing to check and would report the base commit's state,
not this task's. Setup done: `git submodule update --init --depth 1 vendor/pdf_oxide vendor/office_oxide
vendor/tiny_skia`; the memprobe release build (`cargo build --release --manifest-path tool/memprobe/Cargo.toml`)
finished. The Dart SDK, `dart pub get` and the Rust nextest run were not set up, as no gate step was run.
