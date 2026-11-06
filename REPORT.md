# t-571 report

Share: `vendor/pdf_oxide/src/document.rs`, `vendor/pdf_oxide/src/pipeline/reading_order/xycut.rs`,
`tool/fuelprobe/guest/src/main.rs`. Items 1, 3 (partly) and 4 are done. Item 2 is **not done**,
and the `extractAll form` fuel row is **still red** (see "Open"). The patches are
`t-571-fork.patch` (vendor/pdf_oxide) and `t-571-pkg.patch` (package files).

## Open — read these first

1. **Item 2 stopped, as the plan says to.** `extract_embedded_files` has a caller:
   `vendor/pdf_oxide/tests/test_embedded_files.rs:159`
   (`saved.extract_embedded_files().expect("embedded files").len()`), inside a
   `pdf_manipulator patch` block that reads the attachments back after a save. The plan says to
   stop and report when a caller exists, and not to edit an existing test that pins the old
   behaviour. The function is left in place, and its site (`document.rs::extract_embedded_files#1`)
   is still in the lint's list. The function is also named in
   `src/host/engine_proofs/whole_reads.txt`, which would have to change with it. To delete it, the
   maintainer decides what happens to that test (for example, read the attachments back through
   `host::attachments`).
2. **`extractAll form` still fails the fuel wall: max_gap 1,579,413,849 against `MAX_GAP`
   1,100,000,000.** The XY-cut and `postprocess_spans` polls are in and cut the gap from
   4,076,532,208 to 1,579,413,849, but the longest remaining gap is one `sort_by` over every glyph of
   the page in `TextExtractor::extract_into_self` → `sort_glyphs` (`src/extractors/text.rs`, called from
   `stamp_char_x_offsets`). The host's stack sampler (`FUELPROBE_SAMPLE=120`) put 58 of 65 samples there.
   One stdlib sort cannot take a poll, and `text.rs` is not in my share. Fixing it needs either a
   smaller `FORM_TEXT_LINES` (the plan says 20,000) or a sort the engine can poll in; that is the
   maintainer's call.
3. `make test-fuel` therefore exits non-zero on that one row. Every other row passes.

## Per site

### 1. `extract_text_from_ap_stream` (document.rs:11235) — done
- Read whole: the widget/annotation's `/AP /N` stream, through `decode_stream_data()`, then
  `may_contain_text(&bytes)` and `extract_text_spans(&bytes)`.
- Change (document.rs, from about :11250): the stream is opened with `self.form_operators(n_ref)` in
  `Mode::Text`. The quick "does it show text" check walks those operators for a `BT` or `Do`
  (same test as `page_may_contain_text`), and the spans come from
  `TextExtractor::extract_spans_from(source)` over the same operator source. No whole buffer is built.
- Also: `engine_proofs/whole_reads.txt` loses the line `document::extract_text_from_ap_stream`,
  as that file's header says the list "only shrinks as the callers move to readers".
- Row/test before the fix: the wall `host::engine_proofs::whole_reads::only_the_listed_functions_read_a_stream_whole`.
  With the line removed from the list and the old code, it failed:
  `callers not on the list: ["document::extract_text_from_ap_stream"]; listed callers that no longer read whole: []`.
  After the fix it passes.
- Behaviour check: a throwaway proof (a `/Stamp` annotation, no `/Contents`, `/AP /N` form
  with `(STAMPED) Tj`) extracted `"STAMPED"` through the new path. I removed it afterwards because Dart
  batteries see the same text, so `tool/check_fork_tests.sh` would have no honest "no Dart battery can
  observe this" for it.
- No memprobe row: `tool/memprobe` is outside my share, so the wall is the only red-first test.
- Lint: `tool/check_whole_buffers.sh` 59 sites → 58; the diff of the two `--sites` lists is exactly
  `src/document.rs::extract_text_from_ap_stream#1`.

### 2. `extract_embedded_files` — not done (see Open 1)

### 3. Cancel polls in the reading-order XY-cut and the span post-pass — partly done
- Fixture first: `extractAll form` now builds `gen::text_form(size::FORM_TEXT_LINES)` with
  `FORM_TEXT_LINES = 20_000` (main.rs:114, :182). I deleted `gen::graphics_text_form`, which then had no
  caller. **Choice:** the old fixture kept 300,000 graphics triples as well; with 20,000 lines that
  fixture burns more than the host's 20e9 `MAX_TOTAL` (the run is cut off with "fixture too big"). Without
  the triples the total is about 10.3e9. `WHOLE_FORM_TRIPLES` stays for the render rows.
- Failure before: with 20,000 lines and the old XY-cut,
  `extractAll form  max_gap 4985926179  total 20000000000  polls 7424  FAIL: cut off at 20000000000, fixture too big`.
  With the triples removed and the polls disabled (below):
  `max_gap 4076532208  total 10329508282  polls 323  FAIL: gap above 1100000000`.
- Change, xycut.rs:
  - `partition_indexed_depth` polls once per region (:515). **Choice:** it cannot `?`.
    `partition_region` has 13 existing test callers and `order_by_xy_cut` has one engine proof, all
    infallible, and I may not edit them; so on a cancelled job the region ends in the flat sort
    that the depth cap already uses. The cancel itself is reported by the `poll()?` that follows in
    `postprocess_spans`.
  - `find_horizontal_split_indexed`: the `full_width_left_rows` filter scanned both sides
    for each left run (quadratic: about 3.6e9 fuel in one call at 20,000 lines). It is now a loop that
    polls per run (`poll().ok()?`; the function returns `Option`, so a cancel is "no cut"). The
    arithmetic is unchanged.
- Change, document.rs `postprocess_spans`: `crate::cancel::poll()?` before each of the passes
  (off-page, drop caps, vertical-writing/ordering, the rotation firewall, running headers, space
  normalising, char-width splits, super/subscripts, combining marks).
- Injected failure: I disabled both XY-cut polls (`if false {` and the loop poll deleted). Result:
  `extractAll form  max_gap 4076532208  total 10329508282  polls 323  FAIL: gap above 1100000000`
  (polls 323 against 13,667 with them). Restored from a copy; `git diff` shows no `if false`.
  I did not inject separately on the `postprocess_spans` polls: they sit between passes whose gaps
  are far below the sort's, so this row cannot tell their presence.
- Result now: `extractAll  form  1579413849  10275460504  13667  FAIL: gap above 1100000000` (Open 2).

### 4. `TAGGED_SAVE` — done
- main.rs:95: 5,000 → 8,000.
- Before (5,000, this toolchain): `saveFull tagged 61741467 969659970 16890  FAIL: total below 1000000000`.
- After (8,000): `saveFull tagged 68674179 1560580772 27016`, above the floor.

## Choices where the plan left one open
- Item 2 stopped rather than deleting (the plan's own rule).
- Fixture of item 3: lines instead of triples + lines (above).
- XY-cut cancel: flat-sort fallback instead of `Result` plumbing (above), to leave the existing tests
  untouched.
- `whole_reads.txt` edited (one line removed): the wall's own header says it only shrinks.
- The plan's "59 sites" matches the lint's own count; the 86 vs 87 in raw `--sites` lines also
  counts the "not a site" notes.
- The unused scaffolding in item 1: the old marker block in `extract_text_from_ap_stream` had its end
  marker inside a `match`; I closed it where the block ends.
- Changelog: two Fixed bullets in `CHANGELOG.pre.md`'s 6.0.0-dev.0 entry (the appearance stream read in
  pieces; a cancel reaching the reading-order partition and the span passes).

## Summary lines
- Rust suite (`cargo nextest run --lib`, the plan's features): `Summary [ 591.818s] 6752 tests run: 6752 passed (7 slow), 5 skipped`
- `bash tool/check_whole_buffers.sh`: before `59 site(s) read a whole buffer that is not a table`;
  after `58 site(s) read a whole buffer that is not a table`.
- `bash tool/check_patch_markers.sh`: exit 0, no output. `bash tool/check_fork_tests.sh`: exit 0, no output.
  `bash tool/check_cancel_polls.sh`: `every loop over a document-scale collection polls for a cancel`.
  `bash tool/check_changelog_shape.sh`: exit 0, no output.
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `make test-fuel DART=dart`: every row passes except
  `extractAll form  1579413849  10275460504  13667  FAIL: gap above 1100000000`;
  `saveFull tagged  68674179  1560580772  27016`.
- `make test-ops-native DART=dart`: `01:13 +666 ~1: All tests passed!`
- `make test-memory`: not run; it is not in the plan's gate and no memprobe row covers appearance streams.
