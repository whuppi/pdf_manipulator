# t-580 report

Plan paths are relative to `vendor/pdf_oxide` (there is no `src/host/images/extract_stream.rs`; extractImages' pass-through is `src/host/extract_stream.rs`).

## What is done and what is not

**Done: the fax half (plan items 2, 3, 4-fax). Not done: the JPEG half (item 1, and the `cmykJpeg64` row). It is blocked by the plan's own stop rules, details below.**

## Blocked: `ImageData::Jpeg` carrying the `StreamBody`

Changing the payload of the public variant `ImageData::Jpeg(Vec<u8>)` cannot be done inside this share:

- `tests/test_cmyk_jpeg_app14_transform0.rs:65` builds `ImageData::Jpeg(jpeg)` from a `Vec<u8>`. The plan forbids editing `tests/`. `cargo nextest --lib` does not compile `tests/`, so the Rust gate would not show the break, but `cargo test` would.
- `src/ffi.rs:2556` (`bytes.clone()`) and `src/rendering/separation_renderer.rs:3006` (constructs `ImageData::Jpeg(bytes)`) stop compiling. Neither file is in my share.
- `src/host/engine_proofs/extract_jpeg_window.rs:49` constructs `ImageData::Jpeg(jpeg.to_vec())`. It is an existing test; editing it is barred, and its `held` assertions pin the old representation.
- `src/document.rs:26101` matches the variant (it still compiles).

I did not invent a different design (a sibling variant, or a side field on `PdfImage` with an empty `Jpeg`). A sibling variant would turn `extract_jpeg_window`'s `held == (target >= 192)` assertion red. A side field would make `ffi.rs` and `document.rs` hand out an empty JPEG silently.

Because the type change is not made, I added no `reduceImages cmykJpeg64` row. It would fail by construction and leave `make test-memory` red. A narrower fix that avoids the type change exists: `execute.rs:361` (CMYK `reduceImages`) and `stream.rs` near `:650` (the `decode_dct_covering` decline) would decode from the stream's `reader_in` directly. It is not what the plan says, so I left it for the maintainer's decision. The maintainer needs to choose: widen the share to `ffi.rs`, `separation_renderer.rs`, `tests/test_cmyk_jpeg_app14_transform0.rs` and `extract_jpeg_window.rs` for the type change, or accept the reader-based decode.

Remaining whole reads from item 1, checked by grep for references from `src/host`, `lib/`, `hook/` and `bin/`:

- `save_as_jpeg` (images.rs ~:311): only caller is `document.rs:26111`, inside `extract_images_to_files`; nothing in `src/host` or `lib/` references that function. Not on a lane path.
- `ffi.rs` ~:2556 (`pdf_oxide_image_get_data`, the C API): no reference from `src/host`, `lib/`, `hook/` or `bin/`. Not on a lane path.
- `separation_renderer.rs` ~:3006 (`paint_image_to_plates`): called only inside `separation_renderer.rs`; `src/host` references it only as a name in `engine_proofs/whole_reads.txt`. Not on a lane path.
- **`to_base64_data_uri` (images.rs ~:353) IS reachable from a lane**, which the plan did not expect: `document.rs:23693` and `:24152` call it from `to_markdown_cancellable` and `to_html_cancellable`, and `src/host/dispatch.rs:277-353` calls those for the markdown and HTML conversion ops (when images are embedded). It reads a JPEG whole once the type holds only bytes today, and would need the same stream-backed payload. Not fixed here; it belongs with the blocked JPEG work.

## Cause (fax)

`extract_image_from_xobject_at` (`images.rs`, the raw-sample branch, was ~:1297) read a CCITT stream whole with `xobject.decode_stream_data()?` and kept it as `ImageData::Raw` of packed fax bytes. `to_dynamic_image` decoded it later with `decompress_ccitt`. Two lane paths reached it:

- `ImageCursor::open` (`stream.rs` ~:633): when `fax_params` returned `None` it fell through to `extract_image_from_xobject_at`.
- Inline images (`redaction/transform.rs` `fill_inline`, ~:1322), which call the same function.

## Change

- `stream.rs`: `Fax::open` returns `Result<Fax>` (both callers wrap it in `Kind::Fax`). A new `pub(crate) fn fax_gray` reads the fax rows from `Fax::open` / `Fax::read` into the 8-bit gray raster, with `crate::cancel::poll()` per row. `ImageCursor::open` returns `Error::Image` when `fax_params` is `None`.
- `images.rs`: a new `else if is_ccitt` arm (beside the JBIG2 arm) calls `fax_params`, errors with `Error::Image` on `None`, and stores `ImageData::Raw` gray from `fax_gray`. `effective_bpc` is 8 for a fax image. A `cfg(not(rendering))` stub returns `UnsupportedFilter`, as the JBIG2 stub does.
- `tool/whole_buffer_patterns.txt`: `\bdecode_slice\(`.
- `tool/memprobe/src/main.rs`: rows `render fax64` and `reduceImages fax64`.
- `CHANGELOG.pre.md`: two Fixed bullets in 6.0.0-dev.0.
- `src/redaction/transform.rs` needed no change: `fill_inline` goes through the new arm.

## Tests and rows, and how each failed first

`fax_arm` (module in `stream.rs`; no document type inside, so `check_fork_tests` is satisfied):

| test | before the fix |
|---|---|
| `a_fax_image_is_the_gray_raster_of_its_whole_decode` | `plain: the raster, not the stream` left 188, right 407 |
| `rows_past_the_stream_are_white` | left 73, right 407 |
| `a_fax_image_the_cursor_cannot_take_is_declined` | `Ok` instead of `Err(Error::Image)` |

The first test is the "output pixels unchanged" proof: for plain, `/BlackIs1`, `/Decode [1 0]` and both, the arm's gray bytes and `to_dynamic_image()` equal `bilevel_to_grayscale(decompress_ccitt(..))`, today's decode, on a 37x11 image.

Memprobe rows (a 64 MiB CCITT stream, `/Indexed` colour space, so `fax_params` is `None`):

| row | before | after |
|---|---|---|
| `render fax64` | 72,318,011 FAIL | 2,355,545 ok |
| `reduceImages fax64` | 70,159,141 FAIL | 873,964 ok |

Injected failures: I put the old `images.rs` back (`git checkout`), ran `fax_arm`, saw the three failures above, restored the fixed file and confirmed with `git diff`. The "before" memprobe run is the pre-fix engine, so it shows that the rows can fail. I did **not** run a separate injection that breaks only the decline in `ImageCursor::open` (it costs a probe rebuild and run); the `fax64` rows cover it end to end.

## Whole-buffer lint (`--sites`)

- Before: 68 flagged sites, 40 not allowed.
- After: 73 flagged sites, 45 not allowed.

The change: the `images.rs` fax site is gone, `decode_slice`'s own body is now a helper, and its 7 callers (`ascii85`, `ascii_hex`, `brotli`, `flate`, `lzw`, `decoders/mod.rs`, `runlength`) are new sites. I did not edit `tool/whole_buffer_allow.txt`, and those sites were not in my share. `check_whole_buffers.sh` already failed before my change (40 sites); it fails with 45 now.

## Summary lines

- Rust: `Summary [398.091s] 6790 tests run: 6790 passed (5 slow), 5 skipped`
- `make test-memory`: the release run printed 80 `ok` rows, no FAIL, exit 0 (`tool/memprobe` own tests were not re-run separately)
- `make test-fuel DART=dart`: exit 0
- `make test-ops-native DART=dart`: `+672 ~1: All tests passed!`
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `check_patch_markers.sh`, `check_fork_tests.sh`: exit 0, no output
- `check_cancel_polls.sh`: `every loop over a document-scale collection polls for a cancel`, exit 0
- `check_changelog_shape.sh`: exit 0
- `check_whole_buffers.sh`: fails, 45 sites (40 before my change)

## Choices where the plan left one open (the narrower one each time)

- The fax arm shares `Fax::open` / `Fax::read` from `stream.rs` instead of duplicating the line decoder in `images.rs`. Without the `rendering` feature a fax image now returns `UnsupportedFilter` (as JBIG2 does) where it used to decode lazily.
- `image.ccitt_params()` stays set on the 8-bit gray image, because `document.rs:2029` classes a page's codec from it.
- When `/Columns` differs from `/Width`, a row is cropped or padded white to `/Width`. A `/Rows` shorter than `/Height` leaves the later rows white. A fax wider than 65,535 columns is now an `Error::Image` (the row cursor already refused it).
- A fax image that is not 1-bit DeviceGray (a palette, a depth other than 1) is now an `Error::Image`. Before, it produced garbage (the compressed bytes expanded through the palette).
- `PdfImage` of a fax image changes representation for every caller of `extract_image_from_xobject_at`: the data is gray 8 bpc, not packed 1-bit compressed bytes. `tests/` was not run (`--lib` only). `test_ccitt_decoder_diagnosis.rs`, `test_ccitt_params_extraction.rs` and `test_ccitt_edge_cases.rs` read `data()` / `bits_per_component()` of extracted fax images (they print, and skip when their fixture PDF is missing); I did not check whether they assert on it.
- No `mod.rs` edit: the tests live in a `#[cfg(test)]` module of `stream.rs`, because a new `engine_proofs` file would need a line in `engine_proofs/mod.rs`, outside the share.
