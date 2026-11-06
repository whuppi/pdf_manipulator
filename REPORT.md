# t-563: a backing per web reader

Status: implemented and tested, **but the gate is not green**: 7 OPFS tests and 4 cancel tests per streaming mode (12 in all, listed below) pin the old behaviour. Per the plan I did not edit them. They need your decision.

## What changed

- `lib/src/io/blob_source.dart` (new): `BlobSource`, web only. `readAt` on the main thread uses `Blob.slice().arrayBuffer()`.
- `lib/web.dart` (new): barrel for `BlobSource`. `io.dart` imports `dart:io` and the main barrel cannot hold a web-only type, so a third barrel mirrors `io.dart`. Added to `test/guards/barrel_exports_test.dart` as `_web`.
- `lib/src/runtime/web/protocol.dart`: `kTransferBytes` (16 MiB), `LaneMsgFields.heldBytes` (`held`).
- `lib/src/runtime/web/host.dart`: `WebExecutor.heldBytes` (diagnostic), `WebExecutorBooter.transferBudget` (static, tests lower it to 0 to force the fallback).
- `lib/src/runtime/web/lease.dart`: `_backSources` and `_readWhole` (the backing choice and the whole read, raced against `cancelWake`; a cancel finishes the job cancelled before exec is posted; a short read fails the job with `PdfIoFailed`); exec message carries `blobs` and `buffers` (buffers transferred); `_heldCopies` counts the budget per held group; `_heldSources` is `List<DataSource?>` with a null for a backed reader so indices stay aligned; `_preCopySources` skips backed sources; `held` read in `deliver`.
- `web_assets/executor_worker.js`: reader has `blob` and `bytes`; `hostReadAt` order is bytes, blob, OPFS handle, then the mode; `readFromBytes`, `readFromBlob` (FileReaderSync; a throw or short read returns `hostIoError`); `heldBytes()` posted as `held` on `result`, `error` and `resetDone`.
- `lib/src/types/data_source.dart`: lifetime sentence is now "while a handle opened from it is live", plus the snapshot sentence. `lib/io.dart` and `file_source.dart` point web users at `BlobSource`.
- Docs: README (web setup, sources table, three backings paragraph; the hand-written `BlobSource` example is replaced by the shipped one), `docs/ARCHITECTURE.md` ("Web reader backings", the Memory sentence, OPFS row, tree), `CHANGELOG.pre.md` (one Added, one Changed).
- Tests: `test/ops/platform/web/web_backing_battery.dart` (registered in all four web runners), `test/runtime/transfer_budget_test.dart`, `test/ops/runners/web_io_measure_test.dart` (source `blobsource`, and `MEASURE_COPY`).

## Tests and their injected failures

Each was first run red on the unfixed code (5 of 6 failed; the cancel test passed only in OPFS mode, where the pre-copy already parked before exec), then green. Mutations below were run in JSPI and restored (`diff` against saved copies).

| Test (per mode) | Injected failure | Message |
|---|---|---|
| BlobSource: no main-thread read, no pre-copy | `if (source is BlobSource && false)` | `Expected: '1' Actual: '0'` (blobs shape) |
| MemorySource under budget: one whole read, no pre-copy, bytes not detached | transfer a view of the caller's bytes | `Expected: <37> Actual: <null>` (caller's `bytes[0]` after the op) |
| `held` = transferred sizes, 0 after release and reset | worker `heldBytes()` ignores `bytes` | `Expected: <205220> Actual: <0>` and `Expected: <72098>` |
| merge past the budget, same output as budget 0 | `_heldCopies` never added | `Expected: '11111111111111100000' Actual: '11111111111111111111'` |
| cancel during the whole read, exec not posted | cancel wake removed from the race | `Expected: <0> Actual: <2>` (exec/opfsWrite posted) |
| BlobSource read failure -> `PdfIoFailed` | catch returns 0 (EOF) | `Expected: <Instance of 'PdfIoFailed'> Actual: PdfCorrupted: End of file reached unexpectedly` |
| `kTransferBytes <= kKeepBytes ~/ 2` | 20 MiB | `Expected: a value less than or equal to <16777216> Actual: <20971520>` |

The barrel guard was run with `BlobSource` removed from `_web` and fails.

## Existing tests that pin the old behaviour (not edited)

OPFS, `web lane protocol (opfs)`: they use `minimalPdf`, which is now copied instead of pre-copied, so the pre-copy they inspect or stall no longer happens.
- a reset gives the worker a fresh engine state (expects an OPFS file while a source is held)
- a reset is judged by its own directory, not by the files of workers that are gone
- a reset stuck behind a pre-copy write names the call it waits for (timeout)
- a drain settles a job whose pre-copy write is never answered (timeout)
- a create that does not answer is retried once on a fresh file, and the job succeeds
- a create that never answers fails the job with an I/O error within twice the bound
- a late pre-copy ack of one job does not complete the write of another

JSPI, Atomics, Asyncify (the same four each): they use a source read as the marker that the op started, which now happens before the job is posted. OPFS already skips them for that reason.
- `cancel mid-loop extract of all pages cancelled mid-op ends PdfCancelled`
- `combinator cancel edits: cancel stops the one-shot once the edit has started`
- `combinator cancel read: cancel stops an op the engine is running`
- `combinator cancel reads: cancel stops the one op the engine is running`

Suggested fix for you: give the OPFS pre-copy tests a source past the budget (or set `WebExecutorBooter.transferBudget = 0`), and skip the four cancel tests for streaming modes as OPFS does, or mark the start by a sink write. I did not do either.

## Measurements (105 MB, 110,569,021 bytes; ms; two runs per cell)

Open+extract (50 pages) and merge of two inputs. "Today" is the Blob read on the main thread with `Blob.slice` (`blob`); "BlobSource" is the new source. Reads are main-thread `readAt` calls.

| Mode | Source | open+extract | merge | main-thread reads |
|---|---|---|---|---|
| jspi | today | 22243 / 22092 | 15688 / 15814 | 7405 / 12333 |
| jspi | BlobSource | 20205 / 19925 | 11807 / 11731 | 0 |
| atomics | today | 19302 / 19060 | 12418 / 12227 | 7405 / 12333 |
| atomics | BlobSource | 18234 / 18102 | 10216 / 10120 | 0 |
| asyncify | today | 32205 / 31769 | 17891 / 16753 | 7405 / 12333 |
| asyncify | BlobSource | 29480 / 29630 | 12662 / 12479 | 0 |
| opfs | today (pre-copy included) | 15348 / 15289 | 5694 / 5657 | 1688 / 3376 |
| opfs | BlobSource | 20509 / 19416 | 11812 / 11453 | 0 |

The streaming modes gain 7 to 10% on open+extract and 20 to 30% on merge. **OPFS mode is slower with BlobSource** (about 30% on open+extract, about 2x on merge) than the pre-copy, because it reads the Blob slice by slice instead of from a local disk copy; it saves the disk and the pre-copy wait but not the total time on this machine. Decide whether OPFS mode should still pre-copy a BlobSource; the plan says Blob in every mode, so I followed it.

Main-thread copy of a MemorySource into the transfer buffer (median of 20): 1 MiB 0.60 ms, 4 MiB 2.40 ms, 16 MiB 9.10 ms (about 0.6 ms per MiB). This times the same 64 KB loop `_readWhole` runs (the harness replicates it).

## Summary lines

- `dart format --set-exit-if-changed lib hook tool test bin`: `Formatted 281 files (0 changed)`
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `dart test test/guards test/runtime test/bridge test/keep test/hook`: `01:07 +468: All tests passed!`
- native runner: `01:12 +666 ~1: All tests passed!`
- web OPFS: `03:12 +694 ~9 -7: Some tests failed.`
- web JSPI: `02:34 +692 ~1 -4: Some tests failed.`
- web Atomics: `02:07 +689 ~2 -4: Some tests failed.`
- web Asyncify: `02:49 +692 ~1 -4: Some tests failed.`
- new battery alone, each mode: `00:03 +7: All tests passed!` (engine init plus six tests)
- `check_changelog_shape.sh`: exit 0. `check_migration.sh`: `every breaking change of the newest release has its migration`. `check_migration_snippets.sh`: `every migration snippet compiles against its version`.
- I ran the web runners without the `make` wrapper (`dart test ... -p chrome`, `chrome-coi` for Atomics), and did not run `web_auto_mode_test.dart`.

## Choices where the plan left one open

- The whole read is done in 64 KB `readAt` calls into one fresh buffer, not one `readAt(0, length)`: `DataSource` promises 64 KB per call and `TestSource` enforces it. A short chunk fails the job.
- A source joins whenever it fits in what is left (a later small one can join after a larger one was skipped), not "stop at the first that does not fit".
- The budget the lane uses is `min(WebExecutorBooter.transferBudget, kTransferBytes)` minus its held copies, counted by the lane, not the worker's reported `held` (which would lag a `releaseHold`).
- A zero-length source gets no backing.
- `WebExecutorBooter.transferBudget` is a mutable static for tests (like the existing statics there).
- The new barrel is `web.dart`, not a conditional export.
- The Blob-failure test throws on every read from `armBlob` until `disarmBlob`, not once: the engine retries a failed read, so a single throw is absorbed. A failure on the first read of an open reports `PdfCorrupted` (the engine's message contains `host_read_at failed`), so the test fails a later save of a held document, which reports `PdfIoFailed`. The engine fork is untouched.
- No migration guide entry and no Breaking bullet: the `DataSource` lifetime sentence documents what the editor already did, and nothing the library accepted before is refused.
- A `File` that fails to read becomes the host's I/O error via `hostIoError`; a short slice read does the same.
