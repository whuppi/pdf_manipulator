# t-572 report: whole buffers in `document_editor.rs`

Lint (`bash tool/check_whole_buffers.sh`): **59 sites before, 55 after**. `--sites` listed `write_stream_copy#1` (:3965), `deflate_stream#1` (:5144), `get_page_images#1` and `#2` (:12239, :12247) before; none after. Two new "not a site (bounded by take)" lines appear (document_editor.rs:4008 and :5190).

## Sites

1. **`write_stream_copy`** read a stream of at most 256 KiB whole with `read_whole_raw`. The streaming tail is now `write_stream_body` (:3973), which `write_stream_copy` (:3916) calls after the head checks, cuts and remap. Its in-memory case reads `raw_reader()?.take(MEMORY_STREAM).read_to_end(..)` (:4007).
   - Test or row: none can fail on behaviour, since the bytes and the output are the same. The failing check before was the lint itself, red on this site and green on it after.
   - Rows `compress` and `fullRewriteSave` for `huge`, `many`, `encrypted`, `content-fl`, `content-a85`, `content-lzw` and `forms` pass (below).
2. **`deflate_stream`** read a staged stream's stored bytes whole with no size guard, then built a second whole Vec of the compressed result.
   - Change: the save loop (:8159) sends a staged stream over `MEMORY_STREAM` of an unencrypted save to `write_stream_body`. That counts the recode, then recodes into the writer with `/Length` as an indirect object. This is the `write_stream_copy` shape, now shared.
   - `deflate_stream` (:5174) now handles only streams of at most `MEMORY_STREAM`, read through `.take(MEMORY_STREAM)` (:5189).
   - Tests (`engine_proofs/editor_deflate.rs`):
     - `a_staged_stream_over_the_memory_limit_is_deflated_without_a_copy_of_it`. Before the fix it failed with "the save held 17323757 bytes more for a staged stream of 16777216".
     - `a_staged_stream_over_the_memory_limit_that_does_not_shrink_is_written_as_it_is`. It passed before and after, and pins the Copy outcome.
3. **`get_page_images`** decoded the page's content whole and then parsed it into a Vec of operators. It now walks `page_operators` (:12306), reading one operator at a time.
   - Tests (`engine_proofs/editor_pages.rs`):
     - `the_images_of_a_page_with_a_huge_content_are_found_without_holding_the_content`. Before the fix it failed with "the read held 80781434 bytes for a content of 16777245".
     - `the_images_of_a_page_with_a_contents_array_are_found_and_a_broken_part_is_skipped`. It passed before and after, and pins the array and broken-part behaviour.

## Injected failures (each restored; `cmp` of the file against the good copy printed `restored`)

- M1: staged branch guard `> MEMORY_STREAM` changed to `> u64::MAX`. The deflate test failed with "the save held 17618328 bytes more for a staged stream of 16777216".
- M2: `page_operators(..)` followed by `.collect::<Vec<_>>().into_iter()`. The huge-content test failed with "the read held 805515732 bytes for a content of 16777245".
- Not run: a mutation for site 1 (`.take`), since no behavioural test covers it.

## Summary lines

- Rust, `cargo nextest run --no-fail-fast --lib --features ...`: `Summary [574.372s] 6756 tests run: 6754 passed (7 slow), 2 failed, 5 skipped`. The two failures are the existing tests listed under "Existing tests that pin the old behaviour" below.
- `make test-ops-native`: `All tests passed!` (+666 ~1).
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `check_patch_markers.sh`, `check_fork_tests.sh`, `check_cancel_polls.sh`: exit 0 (cancel polls printed "every loop over a document-scale collection polls for a cancel"). `check_changelog_shape.sh`: shape ok.
- `make test-memory`: the probe's own tests passed (7). The probe rows ran until my background job was stopped at its 30-minute limit, after which the probe panicked on its vanished temp folder. I did not re-run it, so the probe as a whole is **not green**. Rows seen: `fullRewriteSave many 15221628 / huge 869155 / encrypted 879797`, `compress many 15209522 / huge 869113 / content-fl 871586 / content-a85 871647 / content-lzw 871584 / forms 13043890 / encrypted 947069`, all `ok` against 16777216. Later rows `redact`, `flattenForms` and `setImageBounds` were ok too.
- `make test-fuel`: one failure, `saveFull tagged ... 968956925 FAIL: total below 1000000000`. The unmodified fork gives the same failure (968781849), so it predates this change.

## Existing tests that pin the old behaviour (not edited)

- `engine_proofs::cancel_polls::get_page_images_cancelled_in_the_parse_of_a_long_page_changes_nothing` asserts at least 22 polls (17 in a separate parse, 5 in the walk). It now sees 5, because the content is lexed lazily inside the walk's stride and there is no separate parse. A cancel still lands within 4096 operators. Keeping 22 would need a job-check accessor in `cancel.rs`, outside my share.
- `engine_proofs::whole_reads::only_the_listed_functions_read_a_stream_whole` fails because `whole_reads.txt` still lists `editor::document_editor::{deflate_stream,get_page_images,write_stream_copy}`. These no longer read whole, and the list is outside my share.
- Both tests need a one-line change by whoever owns them. This is the cost of the three fixes.

## Choices the plan left open

- **Staged stream in an encrypted save, over 256 KiB:** it keeps its data as it is, not re-deflated, since the encrypting writer has no streamed recode. This is the narrowest change that stays bounded. It is a user-visible difference, with a note in the changelog bullet.
- **Where the shared tail lives:** `write_stream_body` is extracted, so `write_stream_copy` and the staged path share one implementation. `encoding` is now read from the dictionary after cuts and remap, not before. Cuts only edit resources, so the result is identical.
- **Memprobe row:** none added, and `tool/memprobe` is unchanged. No wire op stages a huge stream without another whole read in the op itself: `reduceImages` stages under 1 MiB, `embedFile` is not written through this path, and `addImageStamp` reads the image whole. `DocumentEditor::insert_modified` is `pub(crate)`, so the probe cannot stage one directly. The bound is proved by the engine proof (`heap::growth`). The existing `compress` and `fullRewriteSave` rows cover the shared `write_stream_body` huge path.
- **`saveFull` row:** the probe names it `fullRewriteSave`.
- **Tests placed in existing proof files** (`editor_deflate.rs`, `editor_pages.rs`) so as not to touch `engine_proofs/mod.rs`, which other shares edit.
- **Changelog:** two Fixed bullets at the end of the 6.0.0-dev.0 entry.
