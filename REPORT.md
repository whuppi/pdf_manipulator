# t-573 report

All four sites of my share are fixed. Fork changes are in `t-573-fork.patch`, package changes in `t-573-pkg.patch`.

## Sites

`bash tool/check_whole_buffers.sh --sites`, before: **59 sites**; after: **55 sites**. My four are gone and nothing else moved. The one site left in my files' neighbourhood is `host/ops/fonts.rs`, which is not my share.

### 1. `embedFile` (`host/ops/editor.rs`, was :570)
- Read whole: the caller's source, through `read_all_from_reader`, into `EmbeddedFile.data`.
- Change: `PdfReader`/`Source` can sit over a caller's reader, so the plan's stop condition did not apply. `Source::over` (`src/source.rs`) builds a `Source` from the reader. `host/staged.rs::body_over` makes an `Extent` over it. `EmbeddedFile::staged` (`writer/embedded_files.rs`, patch-marked) carries that body. The save writes it through `write_indirect` (`editor/document_editor.rs`, patch-marked, embedded-files block). `dispatch::edit_embed_file` takes `FileData::{Whole, Source}`; `ops/editor.rs` passes the reader.
- `/Params /Size` is the extent length. `/CheckSum` (MD5) is summed in 64 KiB pieces with cancel polls when the file is staged (`staged::md5_of`), so the saved file spec equals the old one.
- Dart: `_mutateWithSource` now passes `keepSources: {0}` (`lib/src/bridge/shared_bridge_editor.dart`), so the source channel lives until the editor is disposed, the same as `appendPages`. No public API changed.
- Test: memprobe row `embedFile file64` (64 MiB of noise). Before: peak 134,325,471 B, FAIL (limit 16,777,216). After: 876,841 B, ok. Proofs in `engine_proofs/editor_streamed_inputs.rs`: `an_embedded_file_is_copied_from_its_source_at_the_save` (bytes, size, checksum equal to the whole-bytes path; source held after the op, dropped at dispose) and `a_cancelled_embed_changes_nothing`.

### 2. `addImageStamp` (`host/ops/editor.rs`, was :629)
- Read whole: the image, through `read_all_from_reader`, then `ImageData::from_bytes`.
- Change: the op reads it through `image_embed::take`, as the builder does. `dispatch::edit_add_image_stamp` takes `impl Into<ImageBytes>`, so existing callers that pass a `Vec<u8>` are untouched. `StreamedImage::stage` (`host/image_embed.rs`) returns the image and `/SMask` streams as extents:
  - JPEG: an extent over a `Shared` reader of the image source, copied in pieces at the save.
  - PNG: an extent over `Deflated`, a `Read + Seek` that decodes a row, narrows it, and deflates it as it is read. This is the same bytes the builder writes. The deflated length is measured once with a counting pass at the op, since an extent states `/Length`, and the save repeats the pass. A mismatch fails the save with "the image changed under the save".
  - Interlaced PNG: `take` reads it whole, the allowed table case.
- `png_decoder` was generalised to any `Read + Seek` so both paths share it. The builder code is otherwise untouched.
- Test: memprobe rows `addImageStamp png64` and `jpeg64`. Before: 386,331,565 B and 201,811,915 B, FAIL. After: 1,688,129 B and 883,067 B, ok. Proofs: `a_png_stamp_saves_the_pixels_of_the_whole_bytes_path` (every colour type and depth), `a_jpeg_stamp_saves_its_own_bytes`, `an_interlaced_png_stamps_as_it_does_today`, `a_stamp_of_bytes_that_are_no_image_changes_nothing`, `the_save_of_a_png_stamp_polls_for_a_cancel`.

### 3. `attachment_bytes` (`host/attachments.rs`, was :57)
- Read whole: the decoded attachment, through `decode_stream_data`.
- Change: replaced by `copy_attachment` (`host/attachments.rs:55`). It runs `decoders::stream::reader_in` over the stream and copies 64 KiB pieces to the sink, polling for a cancel. `dispatch::doc_extract_attachment` calls it. `host::attachments::attachment_bytes` is removed from `engine_proofs/whole_reads.txt`, as that proof demands once a caller stops reading whole.
- The Dart API already writes to a `DataSink` (`extractAttachment(name, DataSink)`), so there is no breaking change. There is no Breaking bullet, migration entry or README change. There are three Fixed bullets instead (see Choices).
- Test: memprobe row `attach attach64` (64 MiB Flate attachment). Before: peak 67,334,445 B, FAIL. After: 299,302 B, ok. Proofs in `engine_proofs/attachment_stream.rs`. Before the fix, `an_attachment_reaches_the_sink_in_pieces_of_at_most_64_kib` failed with "plain: a write of 262144 bytes" and `an_attachment_copy_polls_for_a_cancel` failed with "called `Result::unwrap_err()` on an `Ok` value: ()".

### 4. convert no-sink fallback (`host/ops/convert.rs`, was :155)
- Finding: Dart `convertFromOffice` is the only caller of `EngineOp.convertToPdf` and always passes `sinks: [output]` (`lib/src/bridge/shared_bridge.dart`). The fallback was unreachable from Dart.
- Change: the fallback is deleted. A request without a sink now answers `convertToPdf requires a sink`, as `convertXfaToAcroForm` does. The now-unused imports are gone.
- Test: `convert_to_pdf_without_a_sink_asks_for_one`. Before, it failed with `Invalid operation: DOCX parse: ZIP error: invalid Zip archive: Could not find EOCD`, which is the fallback running.
- Pinned by an existing test: `dispatch::convert_from_format_to_bytes` is still called by `office_trim_probe_tests` (a `not(feature = "office")` test). I did not edit that test, so I kept the helper. It has no caller outside that trim test.

## Injected failures
Each was broken on purpose, run, and restored. The tests pass again in the final nextest run.
| Injection | Result |
|---|---|
| `attachments.rs` `PIECE` 64 → 256 KiB | `an_attachment_reaches_the_sink_in_pieces_of_at_most_64_kib` fails: "plain: a write of 262144 bytes" |
| `attachments.rs` cancel poll removed | `an_attachment_copy_polls_for_a_cancel` fails |
| `staged::md5_of` hashes nothing | `an_embedded_file_is_copied_from_its_source_at_the_save` fails on the size/checksum comparison |
| `Deflated::advance` cancel poll removed | `the_save_of_a_png_stamp_polls_for_a_cancel` fails: "18 polls for 200 rows and a mask" |
| `stage` states a length one byte too long | `a_png_stamp_saves_the_pixels_of_the_whole_bytes_path` and `the_save_of_a_png_stamp_polls_for_a_cancel` fail: "the editor saves: IO error: PNG: the image changed under the save" |
| convert error text changed | `convert_to_pdf_without_a_sink_asks_for_one` fails |
| `Deflated` level `best` → `fast` | passes. That is not a real injection: the measure pass and the read pass share the setting, so it was dropped |

## Summary lines
- `cargo nextest run --lib --features …`: `Summary [573.552s] 6763 tests run: 6763 passed (7 slow), 5 skipped`. A plain `cargo test` of the whole suite fails `filter_readers::a_64_mib_stream_through_ascii85_and_flate_is_never_held` and `page_operators::a_page_of_96_mib_of_flate_content_iterates_in_under_a_mebibyte_of_growth`. They measure process growth and need nextest's process per test; both pass alone.
- memprobe rows (release): see sites 1-3. Final run `EXIT=0`.
- `check_whole_buffers.sh`: 59 → 55 sites.
- `make test-memory`: the probe's own tests `7 passed; 0 failed`; the probe `EXIT=0` with `attach attach64 peak_over_baseline=299302 limit=16777216 ok`, `embedFile file64 peak_over_baseline=876841 limit=16777216 ok`, `addImageStamp png64 peak_over_baseline=1688129 limit=16777216 ok`, `addImageStamp jpeg64 peak_over_baseline=883067 limit=16777216 ok`.
- `make test-ops-native DART=dart`: `+666 ~1: All tests passed!`.
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`.
- `check_patch_markers.sh`, `check_fork_tests.sh`, `check_migration.sh` ("every breaking change of the newest release has its migration"), `check_migration_snippets.sh` ("every migration snippet compiles against its version"), `check_cancel_polls.sh` ("every loop over a document-scale collection polls for a cancel"), `check_changelog_shape.sh`: all pass.
- `bash tool/check_whole_buffers.sh`: `55 site(s) read a whole buffer that is not a table` (59 before). It stays red until the other shares land, as the plan says.

## Not green, and why
- `make test-fuel DART=dart` fails on `saveFull tagged ... 967748182 ... FAIL: total below 1000000000`. The original engine code fails the same row (`saveFull tagged 61737718 967748182 16890 FAIL`), so the failure predates my change. My rows (`addStamp pages` among them) run without a gap failure.
- `make test-ops-opfs` and `make test-ops-jspi` each fail the same two tests: `web wasm parts (opfs|jspi) a corrupt part fails the worker's own compile…` and `a short part fails the worker's own compile…`, with `TimeoutException after 0:00:04`. Every other test passes (693 in opfs, 690 in jspi), including the embed, stamp and attachment batteries. I could not run these two against the original engine code (a background baseline run was killed by a container restart), so I have not shown they predate my change. They time out compiling a deliberately corrupt wasm part, which my change does not touch. The Atomics and Asyncify runners were not run, since the plan names only opfs and jspi.

## Choices where the plan left one open
- Memprobe limits: "at 16 MiB" is the ceiling `16 * MIB`. The inputs are 64 MiB so a row can fail; the plan's "a 16 MiB attachment" would sit at the ceiling and discriminate nothing.
- Memprobe row names: `attach attach64`, `embedFile file64`, `addImageStamp png64|jpeg64`. The 128 MiB image rows get no stamp row (narrower).
- The memprobe rows were written and run against the original engine code (all four FAIL) before the fix. The `embedFile` and `addImageStamp` byte-equality proofs (`editor_streamed_inputs.rs`) were written after the fix, so they did not fail first; their ability to fail is shown by the injections above. The attachment and convert proofs were written first and failed first.
- PNG stamp deflation costs two passes (one to learn the length, one at the save), against the builder's one, because an extent states `/Length` up front. Narrower than adding a deferred-length stream body to `object.rs`.
- `extractAttachment` and `convertToPdf` need no Dart change. Only `_mutateWithSource` changed, to keep the source for the editor's life; each embed or stamp holds one channel until dispose.
- A stamp of bytes `take` calls unreadable now says "invalid image: the bytes are not a readable PNG or JPEG image" (`Error::InvalidPdf`, as before) instead of the decoder's message; the error type is the same.
- No Breaking bullet, migration entry or README change (item 3's condition was not met). Three Fixed bullets in `CHANGELOG.pre.md` cover items 1-3; item 4 has no user-visible change and no bullet.
- `engine_proofs/whole_reads.txt` lost its `host::attachments::attachment_bytes` line, which that proof requires once the function stops reading whole. The new proof files are registered in `engine_proofs/mod.rs`.
