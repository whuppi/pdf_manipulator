# t-571 report

Share: `vendor/pdf_oxide/src/document.rs`, `vendor/pdf_oxide/src/pipeline/reading_order/xycut.rs`,
`tool/fuelprobe/guest/src/main.rs`, and, by the second ruling, `src/extractors/text.rs` for the glyph
sort only. All four items are done and every gate line is green. The patches are `t-571-fork.patch`
(vendor/pdf_oxide, against the submodule commit the repository records) and `t-571-pkg.patch`
(package files), both against the plan commit 555b79d. This replaces the earlier report.

## Per item

### 1. `extract_text_from_ap_stream` (document.rs) — done
- Read whole: the annotation's `/AP /N` stream, decoded in one buffer, then `may_contain_text` and
  `extract_text_spans` over it.
- Change: the stream is opened with `form_operators(n_ref)` in `Mode::Text`. The "does it show text"
  check walks those operators for a `BT` or `Do` (the test `page_may_contain_text` uses), and the spans
  come from `TextExtractor::extract_text_spans_from(source)`.
- Test and failure before: the wall `host::engine_proofs::whole_reads::only_the_listed_functions_read_a_stream_whole`.
  With the caller's line removed from `whole_reads.txt` and the old code it failed with
  `callers not on the list: ["document::extract_text_from_ap_stream"]; listed callers that no longer read whole: []`.
- A scratch proof (a `/Stamp` annotation with `(STAMPED) Tj` in its `/AP /N`) extracted `"STAMPED"`
  through the new path; I removed it because Dart sees the same text.
- No memprobe row: `tool/memprobe` is outside the share.

### 2. `extract_embedded_files` — done (ruling 1)
- Deleted `PdfDocument::extract_embedded_files` and, as it had no other caller, its helper
  `collect_embedded_filespecs` (document.rs, both removed; about 100 lines).
- The one caller, `tests/test_embedded_files.rs`, inside its patch markers, reads the attachments back
  with `pdf_oxide::host::attachments::list_attachments(&saved)` and the same assertion (`len() == 3`).
  `list_attachments` never decodes a stream, so it is the bounded path; `attachment_bytes` decodes whole and
  is not used. This is the one existing test edited, as authorised.
- `whole_reads.txt` loses `document::extract_embedded_files`.
- Test and failure before: the same wall. With the line removed and the function still there:
  `callers not on the list: ["document::extract_embedded_files"]; listed callers that no longer read whole: []`.
- Lint: the site `src/document.rs::extract_embedded_files#1` is gone from `--sites`.

### 3. Cancel polls in the reading-order partition and the span passes — done
- Fixture first: `extractAll form` is `gen::text_form(size::FORM_TEXT_LINES)`, `FORM_TEXT_LINES = 20_000`
  (main.rs:114, :182). `gen::graphics_text_form` had no caller left and is deleted. **Choice:** the old fixture
  also had 300,000 graphics triples; with 20,000 lines it passes the host's 20e9 `MAX_TOTAL` and is cut off
  ("fixture too big"), so the row has the lines only. `WHOLE_FORM_TRIPLES` stays for the render rows.
- Failure before (old XY-cut, 20,000 lines, triples kept):
  `extractAll form  4985926179  20000000000  7424  FAIL: cut off at 20000000000, fixture too big`.
- xycut.rs:
  - `partition_indexed_depth` polls once per region. **Choice:** it cannot `?`: `partition_region` has 13
    existing test callers and `order_by_xy_cut` one engine proof, all infallible, so a cancelled job ends the
    region in the flat sort the depth cap already uses; the `poll()?` that follows in `postprocess_spans`
    reports it.
  - `find_horizontal_split_indexed`: the `full_width_left_rows` scan (every left run scanned both sides:
    about 3.6e9 fuel in one call) is a loop that polls per run (`poll().ok()?`; the function returns `Option`).
- document.rs `postprocess_spans`: `crate::cancel::poll()?` before each pass over the page's spans, and one
  after `stamp_char_x_offsets` (see "A failure the suite found").
- document.rs `assemble_text_from_spans`: `crate::cancel::poll()?` before each of its whole-text passes
  (metadata filter, Kangxi, Arabic forms, whitespace cleanup, mojibake repair, ligatures, CJK digit spaces,
  prime decimal spaces). With the sort polled, this was the next longest gap (1,214,761,275).
- text.rs (ruling 2), `sort_glyphs`: the comparator counts comparisons and calls `crate::cancel::poll()`
  every 4,096; once it reports a cancel a flag makes every later comparison return `Equal` at once.
  The comparator and the stable `sort_by` are unchanged, so without a cancel the order is today's.
  `extract_into_self` calls `crate::cancel::poll()?` right after the sort to return the cancel error.
  `sort_glyphs` is `pub(crate)` so the proof can call it; `sort_by_reading_order` keeps its signature, as three
  existing tests call it.
  **Risk:** a comparator that returns `Equal` after having ordered other pairs is not a total order, and the
  standard sort may panic on that. It did not in the proof (20,000 glyphs, cancel at the second poll) or the suite,
  but the standard library does not promise it.
- Proofs (`src/host/engine_proofs/sort_cancel.rs`, new): (a) a run with no cancel gives the order of an
  independent stable sort by line then x, and polls more than twice; (b) under `tripping_at(2)` the sort polls
  exactly twice, then stops, and `poll()` reports `operation cancelled`.
- Failures shown:
  - Proofs on the old sort: `assertion left == right failed ... left: 0, right: 2` (the sort polled 0 times) and
    the `polls() > 2` assertion of (a).
  - Comparator poll disabled (`false && poll().is_err()`), everything else in place:
    `extractAll form  1545129127  10428539724  13675  FAIL: gap above 1100000000`; the two proofs fail the same
    way (`the sort polled 0 times`). Restored; `grep "false && crate"` finds nothing.
  - XY-cut polls disabled (earlier, with the 20,000-line fixture): `max_gap 4076532208  polls 323  FAIL`, against
    13,667 polls with them.
  - I did not inject separately on the `postprocess_spans` and `assemble_text_from_spans` polls: each sits between
    passes and the row sees them only as the sum.
- After: `extractAll  form  898924743  10481121160  15266`, under `MAX_GAP` 1,100,000,000.

### 4. `TAGGED_SAVE` — done
- main.rs:95: 5,000 → 8,000. Before (5,000 on this toolchain):
  `saveFull tagged 61741467 969659970 16890  FAIL: total below 1000000000`. After:
  `saveFull tagged 68427131 1553183612 27016`.

## A failure the suite found
After the post-sort poll went in, `host::engine_proofs::cancel_polls::page_list_edits_cancelled_at_any_poll_change_nothing`
failed: `a cancel at poll 72 of 72 did not stop the op`. `edit_add_stamp` builds each page with `get_page`, whose
spans reach `stamp_char_x_offsets`; that function returns `()` and swallows the error of its aligned extraction,
so the new poll was the op's last and its cancel was lost. Fixed in `postprocess_spans`: a `poll()?` right after
`stamp_char_x_offsets` (the thread-local cancel is sticky). No existing test was changed for it. The 20
`cancel_polls` proofs pass.

## Choices where the plan left one open
- XY-cut cancel as a flat-sort fallback instead of `Result` plumbing (tests stay untouched).
- Fixture of item 3: lines only, no triples (total cap).
- `collect_embedded_filespecs` deleted with `extract_embedded_files` (it would be dead code).
- `whole_reads.txt`: two lines removed (the wall's header says the list only shrinks).
- The old marker block in `extract_text_from_ap_stream` had its end marker inside a `match`; I closed it where the
  block ends.
- Changelog: two Fixed bullets in `CHANGELOG.pre.md`'s 6.0.0-dev.0 entry (the appearance stream read in pieces; a
  cancel reaching the partition, the glyph sort and the span and text passes of a huge page).
- Not done: `make test-memory` (not in the gate; no memprobe row covers appearance streams).
- A note on method: my first measurements of this round ran on the base tree because I had undone the earlier
  working changes after making the patches; I reapplied `t-571-fork.patch` and `t-571-pkg.patch` before every number
  quoted above.

## Summary lines
- Rust suite (`cargo nextest run --no-fail-fast --lib`, the plan's features):
  `Summary [ 593.981s] 6754 tests run: 6754 passed (7 slow), 5 skipped`
- `bash tool/check_whole_buffers.sh`: before `59 site(s) read a whole buffer that is not a table`; after
  `57 site(s) read a whole buffer that is not a table`. The `--sites` diff is exactly
  `src/document.rs::extract_text_from_ap_stream#1` and `src/document.rs::extract_embedded_files#1`.
- `bash tool/check_patch_markers.sh`, `check_fork_tests.sh`, `check_cancel_polls.sh`, `check_changelog_shape.sh`: all exit 0.
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `make test-fuel DART=dart`: exit 0, no FAIL, 39 rows. Rows of this task:
  `extractAll form  898924743  10481121160  15266`; `saveFull tagged  68427131  1553183612  27016`.
- `make test-ops-native DART=dart`: `01:17 +666 ~1: All tests passed!`
