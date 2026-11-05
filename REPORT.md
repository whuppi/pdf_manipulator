# t-444 — builder image inputs stream from their source

A JPEG or a PNG given to `Pdf.imagesToPdf`, `PdfBuilder.addImagePage` or
`PdfPageBuilder.image` now stays in its `DataSource` until `builderSave`, and
the save reads it from there: a JPEG is copied to the sink in 64 KiB pieces; a
PNG is inflated one row at a time and deflated one row at a time, its alpha in
a second pass over the source. `tool/memprobe` shows `imagesToPdf` under 1 MiB
on 64 MiB and 128 MiB images of both kinds, flat from one size to the next.

Two patches carry the work: `t-444-fork.patch` (vendor/pdf_oxide) and
`t-444-pkg.patch` (Dart, memprobe, fuelprobe).

## 1. Where the image was held whole

The path from the Dart call to the XObject, before this change:

| Where | What it held |
|---|---|
| `src/host/ops/builder.rs` `handle_builder_add_image_page` (and the `image` / `imageArtifact` arms of `handle_builder_page_op`) | `read_all_from_reader(reader)`: the whole file in a `Vec<u8>` (grown by doubling, so up to 2x capacity). |
| `src/host/dispatch.rs` `apply_page_op` → `page_image` → `ImageData::from_bytes` (`src/writer/image_handler.rs`) | JPEG: `data.to_vec()`, a second copy of the file. PNG: `image::load_from_memory_with_format` decodes the whole raster; `to_rgba8` / `to_rgb8` / `to_luma8` convert it; the colour and alpha channels are split into two more buffers; `compress_image_data` builds a third buffer with a filter byte per row, then the deflated copy. |
| `src/writer/document_builder.rs` `image_with_options` → `ImageContent.data` | The encoded stream, moved into the page's element list. |
| `src/writer/content_stream.rs` `add_image_content` | `PendingImage { image: image.clone() }`: a clone of the whole `ImageContent`. |
| `src/writer/pdf_writer.rs` `finish_to_writer` | `image_content_to_xobject_stream` clones `data` and `soft_mask` again; `Object::stream(.., Bytes::from(data.data.clone()))` is a further copy; all of it sits in `image_objects` until the objects are written. |

Measured by `tool/memprobe` before the change (peak over baseline, one
`builderCreate` / `builderAddImagePage` / `builderSave` / `builderDispose`):

| Row | Input | Before |
|---|---|---|
| `imagesToPdf png64` | 67,135,556 byte RGBA PNG, 4096 x 4096 | 386,313,730 |
| `imagesToPdf png128` | 134,271,044 bytes, 4096 x 8192 | 772,193,794 |
| `imagesToPdf jpeg64` | 67,543,141 byte JPEG, 4096 x 3620 | 269,711,317 |
| `imagesToPdf jpeg128` | 134,946,891 bytes, 4096 x 7240 | 539,043,745 |

About 4 x the file for a JPEG (read buffer, `ImageData`, the pending clone, the
stream copy) and about 5.8 x for the PNG: linear in the image, as the slope
between each pair shows.

## 2. Changes

### Fork (`t-444-fork.patch`, vendor/pdf_oxide)

- `src/host/image_embed.rs` (new, host layer): the streamed image.
  - `take` (:142) reads the head of a source: a JPEG (`jpeg_shape` :188,
    `jpeg_frame` :208 walk marker segments up to the frame header and stop;
    the entropy-coded data is never read) or a PNG (`png_shape` :282 reads the
    chunks up to the first `IDAT` with the `png` decoder) stays in its source;
    an interlaced PNG is read whole (`read_whole` :170).
  - `StreamedImage::write` (:370) writes the objects: JPEG `copy_jpeg` (:408),
    64 KiB per read and write, `/Length` the file's length; PNG
    `write_png_stream` (:429) / `deflate_rows` (:458): `png::Reader::next_row`
    one row at a time, `narrow_row` (:503) keeps the colour or the alpha
    samples (16 bit samples rounded as `image` rounds them, `(v + 128) / 257`),
    a `None` filter byte per row, `ZlibEncoder` at `Compression::best()` (the
    level `compress_image_data` uses), `/Length` an indirect object written
    after the stream. The colour stream is one pass over the source, the
    `/SMask` another. Every row and every piece polls `cancel::poll`.
  - The XObject dictionaries come from `ImageData::build_xobject_dict` /
    `build_soft_mask_dict`, so they are today's dictionaries.
- `src/host/ops/builder.rs`: `take_image` (:186) replaces the whole read in
  `handle_builder_add_image_page` (:205) and in the `image` (:320) and
  `imageArtifact` (:542) arms; `image_pixel_dimensions` is `pub(crate)`.
- `src/host/dispatch.rs`: `PageOp::Image` / `PageOp::ImageArtifact` carry an
  `ImageBytes` (:2529, :2827); `apply_page_op` (:2888, :2979) places a streamed
  image with `image_streamed`.
- `src/writer/document_builder.rs`: `image_streamed` (:2103) and `place_image`
  (:2118), the tail of `image_with_options` shared by both.
- `src/writer/pdf_writer.rs` (inside the existing `finish_to_writer` patch):
  a pending image with a `streamed` source is numbered (:1473), skips the
  whole-image normalisation and dedup, and is written by `StreamedImage::write`
  after the other image objects (:2047).
- `src/elements/image.rs`: `ImageContent.streamed` (:60); its literal sites get
  `streamed: None` (`image.rs` :83, :241, :267; `api/pdf_builder.rs` :3157;
  `html_css/paint.rs` :473; `writer/content_stream.rs` :2004, :2075, :2112;
  `editor/document_editor.rs` :14081, :14114, :14147, :14180, :14213, :14246).
- `src/host/mod.rs:48` registers the module; `src/host/probe.rs:114`
  `build_image_page` is the entry the fuel wall runs.
- `src/host/engine_proofs/whole_reads.txt:22` lists `host::image_embed::take`
  (the interlaced fallback is a whole read; see section 6).
- `src/host/engine_proofs/builder_image_stream.rs`, `mod.rs:58`: the proofs.

### Package (`t-444-pkg.patch`)

- `lib/src/bridge/shared_bridge.dart` `_execKeepSource`: takes the handle's
  `scope`, as `_exec` does.
- `lib/src/bridge/shared_bridge_builder.dart`: `addImagePage` and
  `PdfPageBuilder.image` run through `_holdingSource`, which sends
  `keepSources: {0}`; the builder handle remembers the one resource the Router
  files its held sources under and `dispose` releases it.
- `test/bridge/shared_bridge_test.dart`: two tests of that (section 3).
- `tool/memprobe/src/main.rs`: the four rows (section 4).
- `tool/fuelprobe/guest/src/{main,gen}.rs`: the `buildImage png` case.

**Why Dart had to change.** The plan says to avoid it. The save runs in a
later op than the add, and a source's channel is freed when its op ends unless
the op says `keepSources`; without it the builder would read a freed channel at
save. The wire arguments do not change; the transport call does, the way
`editorMergeFrom` already holds its source (the Router's `holds` and
`releaseSource`). A builder with a held image keeps its lane's executor
leased until it is disposed, as an editor does.

## 3. Proofs and their failure before the change

`src/host/engine_proofs/builder_image_stream.rs`, run with
`cargo nextest run --lib --features "$F" -E 'test(builder_image_stream)'`.
Before the change (verbatim failures from that run):

| Proof | Before | After |
|---|---|---|
| `a_jpeg_embeds_in_memory_that_does_not_grow_with_its_height` | FAIL: a 9635784 byte JPEG took 36183665 bytes to embed | pass: 432,829 bytes at 128 rows and at 2048 rows |
| `a_png_embeds_in_memory_that_does_not_grow_with_its_height` | FAIL: a 11496219 byte PNG took 48662983 bytes to embed | pass: 797,368 and 797,369 bytes |
| `the_save_of_a_jpeg_polls_for_a_cancel_at_every_piece` | FAIL: 0 polls for 36 pieces | pass |
| `the_save_of_a_png_polls_for_a_cancel_at_every_row` | FAIL: 0 polls for 3000 rows in two passes | pass (cancel at poll 1500 stops within 8 polls) |
| `the_source_is_held_from_the_add_to_the_save` | FAIL: the add let go of its source | pass |
| `a_disposed_builder_releases_its_sources` | FAIL: dropped at the add | pass |
| `a_jpeg_is_embedded_byte_for_byte_from_its_source` | FAIL, after its byte-for-byte asserts held: "rgb: the add let go of its source" | pass |
| `a_png_embeds_the_pixels_and_the_alpha_of_todays_path` | FAIL, after its pixel asserts held: "gray8: the add let go of its source" | pass |
| `an_interlaced_png_embeds_as_it_does_today` | pass (pins the fallback) | pass |
| `bytes_that_are_not_an_image_are_refused` | pass (pins the refusal) | pass |

The last two pass before the change on purpose: they pin behaviour that must
not move. The JPEG proof compares the XObject description and the stream with
today's path on RGB, grey, CMYK and progressive JPEGs; the PNG proof compares
description, decoded pixels and decoded alpha with `ImageData::from_png` over
21 PNGs: grey 1/2/4/8/16, grey+alpha 8/16, RGB 8/16, RGBA 8/16, palette
1/2/4/8 with and without `tRNS`, grey and RGB with a `tRNS` colour key.

Dart (`dart test/bridge/shared_bridge_test.dart`; `dart test` itself starts the
native build hook, so the file is run directly): with the two `lib/` files
reverted, `an image source is held until its builder is disposed` fails
(`Expected: [Set:[0]] Actual: [Set:[]]`); with the change all 8 tests pass.
`dart analyze lib` is clean.

## 4. memprobe rows

`build imagesToPdf`, limit 16 MiB each (new rows; no limit was raised):

| Row | Before | After |
|---|---|---|
| `imagesToPdf png64` | 386,313,730 FAIL | 825,823 ok |
| `imagesToPdf png128` | 772,193,794 FAIL | 825,827 ok |
| `imagesToPdf jpeg64` | 269,711,317 FAIL | 439,783 ok |
| `imagesToPdf jpeg128` | 539,043,745 FAIL | 439,783 ok |

The inputs are noise, generated row by row into a file (PNG: stored deflate
blocks in 64 KiB `IDAT` chunks; JPEG: `jpeg-encoder` fed a row at a time), each
checked to be at least 64 / 128 MiB. After each step the probe reads the PDF
back: a JPEG's bytes are in it, whole and in order, behind `DCTDecode`, ended
by `endstream`; a PNG's file has its Flate image, predictor and `/SMask` and a
trailer. The read-back passed on the old engine's output too, so the rows
compare like with like. The "before" column was measured with the same rows
against the submodule's unmodified HEAD.

## 5. Fuel wall

`buildImage png` (1000 rows of 1024 RGBA pixels of noise, `probe::build_image_page`):

| | max_gap | total | polls |
|---|---|---|---|
| after | 3,806,398 | 2,076,108,210 | 2000 |
| before (same fixture through the old `PageOp::Image` path) | 1,907,901,644 | 1,907,901,644 | 0 — FAIL: gap above 1,100,000,000 |

The whole wall passes (every row under `MAX_GAP`, exit 0).

**No JPEG case.** The wall requires 10^9 units of work in a case. A JPEG build
copies 64 KiB per poll: a 48 MiB file measured max_gap 2,030,501 over 770 polls
but a total of 52,702,725, about one unit a byte, so a fixture of about 1 GiB
would be needed to reach the floor. The plan makes the cases conditional on the
wall's style; the JPEG copy is covered by
`the_save_of_a_jpeg_polls_for_a_cancel_at_every_piece` instead.

## 6. Choices the plan left open

1. **Dart change** (section 2): `keepSources` on the two builder image ops and
   the release on dispose. Required for the engine change to be sound.
2. **Adobe APP14 / CMYK inversion**: today's code does nothing with them. The
   JPEG is embedded as `DeviceCMYK` for four components, `DeviceGray` for one,
   `DeviceRGB` otherwise (also for any other count), `BitsPerComponent` 8, no
   `/Decode`. The streamed path writes the same dictionary and the file's
   bytes, so behaviour is the same, inversion included.
3. **Interlaced PNG**: today's code decodes it (through `image`), so it keeps
   working, but Adam7 needs the whole image, so `take` reads the file whole and
   embeds it by `ImageData::from_png` as before. Listed in `whole_reads.txt`.
4. **No dedup of streamed images.** `finish_to_writer` hashes an image's stream
   bytes to embed identical images once. A streamed image's bytes are not in
   memory, so two identical images in one builder are now two XObjects. The
   pixels are the same; the file is larger by one image.
5. **Where a damaged image fails.** A source that is not a PNG or a JPEG, a PNG
   whose header will not read and a JPEG with no frame header still fail at the
   add, with the same `InvalidArgument` and message for `addImagePage`. A PNG
   whose data is truncated or corrupt now fails at `builderSave` (it was decoded
   at the add): `imagesToPdf` fails either way; `PdfPageBuilder.image` now
   returns and the later `save` throws. `bytes_that_are_not_an_image_are_refused`
   accepts the add or the save.
6. **Messages and I/O errors at the add.** A page-op image that is no image
   says `image data is not a usable image` without the decoder's suffix. A read
   error or a cancel while the head is read now returns its own typed error; it
   used to read as an empty image (`unwrap_or_default`).
7. **A bug found and left alone: JPEG page size.** `image_pixel_dimensions`
   (builder.rs :133) reads a JPEG's height from the frame's width and its width
   from the component count and first component id: a 97 x 61 RGB JPEG gives
   `(768, 97)`. So `addImagePage` without a page size makes JPEG pages the wrong
   size, today and after. The streamed path takes the page size from the same
   function (run on the scanned head), so nothing moved; the XObject's own
   `/Width` and `/Height` come from the frame and are right. Fixing it changes
   every JPEG page of every caller, so it is the maintainer's call.
8. **JPEG fuel case** left out (section 5). **Docs and CHANGELOG** are not part
   of the patches (only the patch files and this report are committed):
   `docs/ARCHITECTURE.md` should gain `host/image_embed.rs` in its source tree
   and a line in "Streaming I/O".
9. **Web not exercised.** The held source rides the Router's existing `holds`
   path (`holdGroupId`) that editors use; no browser run was possible here, and
   the wasm gate is a compile check.
10. **Peak at one PNG row**: the `png` decoder's default limit (64 MiB) is left
    as it is; a row of an image wider than about 8 million pixels would exceed
    it and fail the save with a typed error.

## 7. Gate

Run on the final tree (the commands of PLAN.md, with `$F` as given there):

| Gate | Result |
|---|---|
| `cargo nextest run --manifest-path vendor/pdf_oxide/Cargo.toml --features "$F"` | 485 binaries, 10250 tests run, **10250 passed**, 65 skipped (by the repo's own config), 656 s. An earlier full run, before `whole_reads.txt` listed `host::image_embed::take`, had one failure, `engine_proofs::whole_reads::only_the_listed_functions_read_a_stream_whole` ("callers not on the list: host::image_embed::take"); it is the wall doing its job, the interlaced fallback is a real whole read, and it is listed with its reason. The disk could not hold debug info for 485 test binaries, so the runs used `CARGO_PROFILE_DEV_DEBUG=0` and a target dir of their own. |
| `cargo test --release --manifest-path tool/memprobe/Cargo.toml` | 7 passed. |
| `cargo run --release --manifest-path tool/memprobe/Cargo.toml` | 67 rows, every one `ok`, no `FAIL`, no `READ-BACK FAIL`; the four new rows are in section 4. |
| fuel wall (the `test-fuel` commands of the Makefile) | exit 0; every row under `MAX_GAP`; `buildImage png` max_gap 3,806,398, total 2,076,108,210, 2000 polls. |
| `bash tool/check_patch_markers.sh` | exit 0. (The first version put three of my edits in `pdf_writer.rs` in markers of their own inside the existing `finish_to_writer` block; blocks do not nest, so they now sit inside that block's markers.) |
| `bash tool/check_fork_tests.sh` | exit 0. |
| `bash tool/check_cancel_polls.sh` | exit 0: "every loop over a document-scale collection polls for a cancel". |
| `cargo check --manifest-path vendor/pdf_oxide/Cargo.toml --lib --target wasm32-unknown-unknown --no-default-features --features wasm,icc,legacy-crypto,rendering,signatures,pdfa,office,extract` | exit 0. Three warnings, none in a changed line: `cache.rs:88` (`len`), `xfa/parser.rs:300` (`parse_datasets`), `editor/document_editor.rs:22` (`std::fs::File`). A first run showed `private_interfaces` warnings from my `ImageBytes` being `pub(crate)` inside the public `PageOp`; the type and its module are `pub` now. |

Also run, outside the plan's gate: `dart analyze --fatal-infos lib
test/bridge/shared_bridge_test.dart` (clean), `dart format` (no change) and
`dart test/bridge/shared_bridge_test.dart` (8 passed).

**Not run:** `make check` as a whole: `test-ops-native`, `test-ops-web` (three
web modes) and `test-example` need the native build hook, a browser and the
example app, and `dart test` starts that hook. So no Dart battery of
`test/ops/` has run against this change, and the web transports have not
carried a held image source. The Dart change is covered by the unit test and
the analyzer only.
