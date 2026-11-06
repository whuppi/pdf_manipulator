# t-577 — a failing source is `PdfIoFailed`, and one failed read is not absorbed

## Cause

Native behaves like web: a source whose first read throws opened as `PdfCorrupted`, and one failed read during open or mid-op was absorbed and the op succeeded. Before the fix the new battery failed natively exactly so (first read: not `PdfIoFailed`; read 2 of 4 during open: the document opened; read 5 mid-op: text of page 900 came back whole).

Three places, all in `vendor/pdf_oxide`:

1. **Header** — `src/document.rs` `parse_header` (about line 26843): any read error other than EOF became `Error::InvalidHeader("Failed to read file: …")`, which `kind_of` maps to `Corrupted`. This is the "host_read_at failed" inside a `PdfCorrupted` message.
2. **The retry** — `src/document.rs` `open_from_reader_cancellable` (about line 1253): when the regular xref parse returned *any* error, including an `Io` one, the engine logged "attempting reconstruction" and scanned the whole file again. The second pass read the source afresh, so a source that failed once succeeded. No `BufReader` or `Interrupted` retry is involved: both host readers answer `ErrorKind::Other`, on purpose.
3. **Swallowing sites** — many callers drop an `Err` instead of passing it up. Examples: `validate_object_at_offset` (`reader.read(..).unwrap_or(0)`, line 26705, whose `false` also starts a rebuild), `load_object`'s `Err(_)` arms (about line 2869, "treating as Null per §7.3.10"), and the `.ok()` and `Err(_) => continue` sites throughout the extraction paths. A host read error that reaches one of them turns into a missing object, a missing font or an empty page, not an error.

No retry is kept. The contract (`DataSource.readAt`) does not promise a transient failure, so a read error ends the op.

## Change

All in `vendor/pdf_oxide` (`t-577-pdf_oxide.patch`):

- `src/document.rs:26843` — `parse_header` returns `Error::Io(e)` for a read error (between patch markers).
- `src/document.rs:1253` — a regular-xref error that is `Error::Io` is returned at once, without a rebuild (between patch markers).
- `src/host/bridge_api.rs:46-65` — a per-op, thread-local latch: `fail_source` records the first host read failure and returns the error, `source_failure` returns it again, `take_source_failure` empties it.
- `src/host/bridge_api.rs:192, 222` — `handle_request` empties the latch when an op starts. When the op ends with the latch set, the response becomes an `Io` error (a cancelled response stays cancelled), and the documents, editors and builders the op created are dropped, so a failed open leaves no handle in the lane.
- `src/host/native/callback_reader.rs:86, 158` and `src/host/wasm/js_reader.rs:74, 101` — a host read failure goes through `fail_source`, and every later read of the op answers the same error without asking the host again. A cancel is not a failure and is not latched.
- `src/host/engine_proofs/shared_state.txt` — the new thread-local is listed, as `shared_state.rs` requires.

A thread-local is sound here: neither build enables the `parallel` feature, the engine starts no threads, and one executor runs one op at a time. The swallowing sites of cause 3 are left as they are: the boundary check in `handle_request` overrules them, which is the narrower change.

The Dart decoder (`lib/src/bridge/protocol/binary_codec.dart:102`) already maps the `Io` kind to `PdfIoFailed`; no Dart library file changed.

Package files (`t-577-pkg.patch`): `test/ops/core/pdf_source_failure_battery.dart` (new), the five runners in `test/ops/runners/`, and `CHANGELOG.pre.md` (two Fixed bullets in 6.0.0-dev.0; `tool/check_changelog_shape.sh` passes).

## Tests, and how each failed before the fix

Dart battery `test/ops/core/pdf_source_failure_battery.dart`, registered in `native_runner_test.dart` and the four web runners:

| test | failure before the fix (native) |
|---|---|
| first read fails → `open` is `PdfIoFailed` | threw `PdfCorrupted` (header mapping) |
| a source that fails once after the first read fails `open` too | read 2 of 4: `open` succeeded |
| a read that fails once mid-op fails that op | read 5: the op returned the page text |
| a corrupt file read by a sound source is still `PdfCorrupted` | passes before and after (guards against over-reach) |

Rust proofs, `src/host/engine_proofs/source_failure.rs`:

- `a_failed_read_of_the_header_is_an_io_error_not_a_bad_header` — failed before, `Error::InvalidHeader`.
- `a_failed_read_of_the_xref_ends_the_open_without_a_rebuild` (the test of the retry decision) — failed before: "the open fails" panicked because the open succeeded; it also counts reads after the failed one and requires zero.
- `bridge::a_trip_that_fails_once_ends_the_open_with_an_io_error_and_keeps_no_document` — fails each trip of an open in turn through the real `CallbackReader` and `handle_request`; requires an `Io` response and no document kept. I did not capture its pre-fix output on its own: the first red run overlapped my first edits to `bridge_api.rs`. Its failure is shown by the injection below.

### Injected failures (all restored; `git diff` showed nothing injected left)

- Rust: `callback_reader.rs` back to a plain `io::Error::other` and the `Io` early return in `document.rs` removed →
  `bridge::a_trip_…`: `assertion left == right failed: trip 2 of 3 failed and the open succeeded`;
  `a_failed_read_of_the_xref_ends_the_open_without_a_rebuild`: `the open fails` (panic at source_failure.rs:73).
- Rust: the `handle_request` override disabled: all three Rust proofs still pass, because the document-level changes and the sticky reader already cover open. The override matters mid-op, shown next.
- Dart native: the `handle_request` override disabled →
  `a source that fails once after the first read fails open too`: `Expected: throws PdfIoFailed … emitted PdfDocumentHandle` (read 4 of 4);
  `a source whose read fails once mid-op …`: `threw PdfEngineFailed: Invalid PDF: Page index 900 not found by scanning`.

**Not done:** I did not break the `js_reader.rs` edit and rebuild the wasm (about 25 minutes per build) to see the web battery fail. The web battery passes with it (jspi, asyncify, atomics), and the web run before the fix was not taken. The native reader has the same code shape.

## Summary lines

- Rust (`cargo nextest run --no-fail-fast … --lib`, the plan's features): `Summary [ 372.623s] 6759 tests run: 6759 passed (5 slow), 5 skipped`. An earlier fail-fast run stopped at one failure, `shared_state::every_shared_static_is_listed_with_a_class`, because the ledger lacked the new thread-local; it was a data-file entry, fixed, and the full run above followed.
- `make test-ops-native`: `01:20 +670 ~1: All tests passed!`
- `make test-ops-jspi`: `02:04 +702 ~2: All tests passed!`
- `make test-ops-atomics`: first run `01:40 +698 ~3 -1: Some tests failed.` — one failure, `stress builder build and open a 15,-page image PDF inline`, a timing budget (`15.32x`, limit 15) on a test that reads no source; rerun alone: `01:41 +699 ~3: All tests passed!`.
- `make test-ops-asyncify`: `02:17 +702 ~2: All tests passed!`; auto mode `00:07 +5 ~1` and `00:02 +1 ~5`, both `All tests passed!`.
- `make test-ops-opfs`: **`02:15 +703 ~12 -2: Some tests failed.` — see Blocked.**
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `tool/check_patch_markers.sh`, `tool/check_fork_tests.sh`, `tool/check_cancel_polls.sh`: exit 0. `tool/check_changelog_shape.sh`: exit 0.

## Blocked: needs `lib/src/runtime/web/lease.dart` (not in my files)

On web, a source up to the 16 MiB transfer budget is read whole in Dart before the job starts (`_backSources`/`_readWhole`, lease.dart about line 897-940), and in OPFS mode every source is copied the same way (`_preCopySources`, about line 950). A `readAt` that throws there never reaches the Rust reader. `_runJob`'s catch-all (lease.dart about line 541) answers `buildErrorResponse(e.toString())`, kind engine, so the caller gets `PdfEngineFailed: Bad state: …`, not `PdfIoFailed`.

Effect: in OPFS mode two battery tests fail (`a source whose first read fails makes open fail with PdfIoFailed`, `a source that fails once after the first read fails open too`). They are left failing and not skipped. In the other three modes they pass only because the runners set `WebExecutorBooter.transferBudget = 0` for this battery, as the cancel batteries do for the same reason, so the read reaches the worker. Without that, those modes show the same failure with a source under 16 MiB.

The fix is a few lines in `lease.dart`: in `_readWhole` and in the `_preCopySources` read, catch a non-`PdfError` thrown by `source.readAt` and throw `PdfIoFailed` (the existing `on PdfIoFailed` arm of `_runJob` then answers kind `io`). Also in lease.dart, `_serveRead` (about line 800) turns a throw into an `error` string, which reaches Rust as a failed read and is handled by this change.

The mid-op test is skipped for OPFS (`skipMidOp`), as the cancel battery does, because OPFS copies each source before the op and no read lands in the op.

## Choices where the plan left one open

- Where the failure lives: a per-op thread-local latch in `bridge_api.rs`, not a field on `EngineState`/`Source`, because those files are not on the list.
- Overrule at the boundary instead of fixing each swallowing site, which is the narrower change; the document-level fixes cover only the two sites that make open lie.
- A cancelled response stays cancelled even if a read had failed before it.
- A failed op drops the handles it created (a successful open can end in a latched failure).
- The Rust proof file and a ledger line sit in `src/host/engine_proofs/` and `shared_state.txt`, where `check_fork_tests.sh` and `shared_state.rs` expect them.
- I did not edit `lease.dart`, `engine_state.rs` or `executor.rs`, per the plan.
- `office_oxide` is unchanged, so there is no `t-577-office_oxide.patch`.
