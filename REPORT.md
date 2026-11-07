# t-579 — `convertToPdfA` stages its edits

Status: the change is done and its own tests pass. Two checks of the gate fail because they pin the old behaviour; both are in files outside my share, so I stopped there (see **Blocked**).

## Cause

- `vendor/pdf_oxide/src/compliance/converter.rs`, `convert_with_editor` (old lines ~326-331): after staging, it called `editor.commit_in_place()` (`src/editor/document_editor.rs:1548`, saves the whole document to a `Vec` and reparses it), then ran a second full validation.
- `vendor/pdf_oxide/src/host/dispatch.rs:1619` (`edit_convert_to_pdf_a`) discarded the result of that validation (`.map(|_| ())`).

## Change

- `converter.rs:274-` `convert()` (the non-host entry, used by `convert_to_pdf_a` and `tests/test_pdfa_roundtrip.rs`): stages through `convert_with_editor`, then, unless the document was already compliant, calls `commit_in_place` and `validate_converted`. Old behaviour is kept for this caller.
- `converter.rs` `convert_with_editor`: stages fixes only. No commit, no final validation. Its result carries the validation from before the fixes.
- `converter.rs` new `validate_converted`: the old final validation plus the #451 downgrade, moved out of the staging path unchanged.
- `dispatch.rs:1617` comment updated; the call is unchanged (`editor.undoable(... convert_with_editor ...)`).
- `tool/memprobe/src/main.rs`: `("convertToPdfA", "xmp64") => 16 * MIB + XMP_PACKET` with the plan's comment; `XMP_PACKET` (64 MiB) is now one constant shared with `generate_huge_xmp`.
- `docs/ARCHITECTURE.md` "The promise": the sentence, at the end of the section.
- `CHANGELOG.pre.md`: one Fixed bullet in 6.0.0-dev.0.
- `commit_in_place` itself is unchanged; its only caller is now `convert()`.

## Tests, each written before the fix

In `vendor/pdf_oxide/src/host/engine_proofs/mem_pdfa_xmp.rs`:

1. `the_conversion_stages_its_edits_without_saving_the_document` — before the fix: `FAILED: the catalog is staged, not committed`. It also bounds the thread's growth at 8 MiB over a 24 MiB file.
2. `a_converted_file_saved_and_opened_again_validates` — converts, saves, reopens, validates with `validate_pdf_a` (A2b). It passed before the fix (a guard for the output, not a red test).

Memprobe row: written first, but it did **not** fail on the old code (see Measurements).

## Injected failures (each restored; `git diff` shows nothing injected)

- Put `editor.commit_in_place()` back after `convert_with_editor` in `dispatch.rs`: test 1 failed with `the catalog is staged, not committed`.
- Made `try_fix_error` return at once: test 2 failed with `[ComplianceError { code: MissingXmpMetadata, message: "Document is missing XMP metadata stream" ...}]`.

## Measurements — `convertToPdfA xmp64`

Before (unfixed binary, new limit): `convertToPdfA xmp64 peak_over_baseline=68169837 limit=83886080 ok wall_ms=4598`
After: `convertToPdfA xmp64 peak_over_baseline=67969926 limit=83886080 ok wall_ms=3068`

The plan says the row sat at 129 MiB. On this checkout it measured 65.0 MiB before the change, so the row did not move (-0.2 MB) and the new limit would have passed on the old code. I did not find why the 129 MiB figure does not reproduce. The staging proof (test 1) is what shows the copy gone.

## Summary lines

- `cargo nextest run --no-fail-fast --lib --features ...`: `6789 tests run: 6788 passed (16 slow), 1 failed, 5 skipped` — the one failure is `cancel_polls::convert_to_pdf_a_with_fonts_cancelled_at_any_poll_changes_nothing`.
- `cargo test --test test_pdfa_roundtrip`: `20 passed; 0 failed`.
- `make test-memory`: `test result: ok. 7 passed`; every probe row ok; exit 0.
- `make test-ops-native`: `+672 ~1: All tests passed!`
- `make test-fuel`: `convertToPdfA pages  7835342  474847120  3513  FAIL: total below 1000000000`; every other row passes; exit 2.
- `check_patch_markers.sh`, `check_fork_tests.sh`, `check_cancel_polls.sh`, `check_changelog_shape.sh`: exit 0.
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`

## Blocked (outside my share; existing tests not edited)

1. `vendor/pdf_oxide/src/host/engine_proofs/cancel_polls.rs:318` `convert_to_pdf_a_with_fonts_cancelled_at_any_poll_changes_nothing` asserts at least 38 cancel polls: `a full run polls at least 38 times, once for each page of each loop: 22`. The 16 missing polls were the final validation the plan removes. The minimum should become 22; that file is not in my list and the test pins the old behaviour.
2. `tool/fuelprobe/guest/src/main.rs:261` row `convertToPdfA pages` needs at least 10^9 units of work and now does 474,847,120 (largest gap between polls 3513, unchanged in kind). Its fixture needs more pages or more text to make up the work the final validation did.

## Choices where the plan left one open

- The proofs went into the existing `engine_proofs/mem_pdfa_xmp.rs` instead of a new module, so `engine_proofs/mod.rs` (touched by every worker) stays unchanged. That file is not in my list; fork tests have to live in `engine_proofs/`.
- `convert_with_editor` keeps its name and visibility; the result's `success` is true only when the document was already compliant.
- `commit_in_place` stays `pub(crate)` for `convert()`; no comment added to it (it is upstream code outside patch markers).
- The ARCHITECTURE sentence is placed at the end of "The promise".
- Nothing changed in `vendor/office_oxide`, so no patch for it.
