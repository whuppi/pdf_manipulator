# t-563: a backing per web reader

Status: implemented, tested, and the gate is green on every runner. This is the second revision: it applies your two decisions (the existing tests forced onto the path they test, and OPFS mode pre-copying a `BlobSource`).

## What changed

- `lib/src/io/blob_source.dart` (new): `BlobSource`, web only. `readAt` on the main thread uses `Blob.slice().arrayBuffer()`.
- `lib/web.dart` (new): barrel for `BlobSource`. `io.dart` imports `dart:io` and the main barrel cannot hold a web-only type, so a third barrel mirrors `io.dart`. Added to `test/guards/barrel_exports_test.dart` as `_web`.
- `lib/src/runtime/web/protocol.dart`: `kTransferBytes` (16 MiB), `LaneMsgFields.heldBytes` (`held`).
- `lib/src/runtime/web/host.dart`: `WebExecutor.heldBytes` (diagnostic), `WebExecutorBooter.transferBudget` (static, tests lower it to 0 to force the fallback).
- `lib/src/runtime/web/lease.dart`: `_backSources` and `_readWhole` (the backing choice and the whole read, raced against `cancelWake`; a cancel finishes the job cancelled before exec is posted; a short read fails the job with `PdfIoFailed`); exec message carries `blobs` and `buffers` (buffers transferred); `_heldCopies` counts the budget per held group; `_heldSources` is `List<DataSource?>` with a null for a backed reader so indices stay aligned; `_preCopySources` skips copied sources and only names the file of a Blob (the worker fills it); an `error` carrying `io` becomes the I/O error; `held` read in `deliver`.
- `web_assets/executor_worker.js`: `copyBlobToOpfs` (OPFS mode: the worker copies a Blob to its pre-copy file with `FileReaderSync`, before the readers are made; a Blob that cannot be read throws with `hostIo`, posted as `io` on the `error`); reader has `blob` (not in OPFS mode) and `bytes`; `hostReadAt` order is bytes, blob, OPFS handle, then the mode; `readFromBytes`, `readFromBlob` (FileReaderSync; a throw or short read returns `hostIoError`); `heldBytes()` posted as `held` on `result`, `error` and `resetDone`.
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

## Existing tests forced onto the path they test (your decision 1)

Only a setup line was added; no assertion changed.
- `web lane protocol (opfs)` (`test/ops/platform/web/web_lane_protocol_battery.dart`): the group's `setUp` sets `WebExecutorBooter.transferBudget = 0` when `mode == opfs`, and a `tearDown` restores `kTransferBytes`. That is the 7 failing tests plus the rest of the OPFS group, which already passed and now runs on the pre-copy as before. The JSPI, Atomics and Asyncify registrations of the group are untouched.
- The cancel groups in the JSPI, Atomics and Asyncify runners (`web_jspi_runner_test.dart`, `web_atomics_runner_test.dart`, `web_asyncify_runner_test.dart`): the two calls `registerCancelTests(() => pdf)` and `registerCombinatorCancelTests(() => pdf)` are each wrapped in `group('sources read through the mode', ...)` with the same `setUp`/`tearDown`. The batteries are shared with the native runner, which has no `WebExecutorBooter`, so the line sits in the web runners. The wrapper adds the group name to the ids of every test in those two batteries (they were already passing; the 4 that failed are among them). OPFS keeps its existing `skipExtract`.

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
| opfs | BlobSource, read slice by slice (first revision) | 20509 / 19416 | 11812 / 11453 | 0 |
| opfs | BlobSource, pre-copied by the worker from the Blob (this revision) | 14492 / 13723 | 4985 / 4172 | 0 |

The streaming modes gain 7 to 10% on open+extract and 20 to 30% on merge. In OPFS mode a `BlobSource` is pre-copied (your decision 2), not read slice by slice: reading the Blob slice by slice in the worker was about 30% slower on open+extract and 2x slower on merge than the pre-copy of today's source, because every engine read became a Blob slice read instead of a read from a local disk copy. The pre-copy is now made by the worker from the Blob, with no main-thread read and no chunk sent from the page, and it measures 5 to 10% faster than today on open+extract and 12 to 27% faster on merge (the last two rows of the OPFS block).

Main-thread copy of a MemorySource into the transfer buffer (median of 20): 1 MiB 0.60 ms, 4 MiB 2.40 ms, 16 MiB 9.10 ms (about 0.6 ms per MiB). This times the same 64 KB loop `_readWhole` runs (the harness replicates it).

## Summary lines

All of these were run on the final tree.
- `dart format --set-exit-if-changed lib hook tool test bin`: `Formatted 281 files (0 changed)`
- `dart analyze --fatal-infos lib hook test bin`: `No issues found!`
- `dart test test/guards test/runtime test/bridge test/keep test/hook`: `01:09 +468: All tests passed!`
- native runner: `01:12 +666 ~1: All tests passed!`
- web OPFS (`chrome`): `02:37 +702 ~9: All tests passed!`
- web JSPI (`chrome`): `02:16 +696 ~2: All tests passed!`
- web Atomics (`chrome-coi`): `01:55 +693 ~3: All tests passed!`
- web Asyncify (`chrome`): `02:42 +696 ~2: All tests passed!`
- `web_auto_mode_test.dart` on `chrome`: `00:08 +5 ~1: All tests passed!`; on `chrome-coi`: `00:02 +1 ~5: All tests passed!`
- new battery alone: OPFS `00:03 +8: All tests passed!` (engine init, six tests, and the OPFS pre-copy test); JSPI, Atomics, Asyncify `+7 ~1` (the OPFS test skipped)
- `check_changelog_shape.sh`: exit 0. `check_migration.sh`: `every breaking change of the newest release has its migration`. `check_migration_snippets.sh`: `every migration snippet compiles against its version`.
- The runners were run with `dart test ... -p chrome` (`chrome-coi` for Atomics), not through `make`.

## Choices where the plan left one open

- OPFS pre-copy of a Blob (decision 2): the worker does it, in `exec`, before the readers exist, so a failure leaves no reader behind and the files are dropped by `opfsDrop` as any pre-copy. It checks the cancel channel between 64 KB slices. A Blob that cannot be read fails the job with the I/O error (the worker marks the `error` with `io`). A `BlobSource` is pre-copied in OPFS mode whatever its size, and does not use the transfer budget.
- The Blob-failure test in OPFS mode fails the Blob read during the pre-copy of the open, because the Blob is not read again afterwards; in the other modes it fails a later save, as before.
- New test `OPFS mode pre-copies a BlobSource, in the worker and from the Blob` (OPFS only, skipped elsewhere). Injected failure: the lane does not name the pre-copy file and the worker keeps a Blob backing in OPFS mode. Result: `Expected: '1' Actual: '0'` (the `opfsFiles` shape), in this test and in the BlobSource test.

- The whole read is done in 64 KB `readAt` calls into one fresh buffer, not one `readAt(0, length)`: `DataSource` promises 64 KB per call and `TestSource` enforces it. A short chunk fails the job.
- A source joins whenever it fits in what is left (a later small one can join after a larger one was skipped), not "stop at the first that does not fit".
- The budget the lane uses is `min(WebExecutorBooter.transferBudget, kTransferBytes)` minus its held copies, counted by the lane, not the worker's reported `held` (which would lag a `releaseHold`).
- A zero-length source gets no backing.
- `WebExecutorBooter.transferBudget` is a mutable static for tests (like the existing statics there).
- The new barrel is `web.dart`, not a conditional export.
- The Blob-failure test throws on every read from `armBlob` until `disarmBlob`, not once: the engine retries a failed read, so a single throw is absorbed. A failure on the first read of an open reports `PdfCorrupted` (the engine's message contains `host_read_at failed`), so the test fails a later save of a held document, which reports `PdfIoFailed`. The engine fork is untouched.
- No migration guide entry and no Breaking bullet: the `DataSource` lifetime sentence documents what the editor already did, and nothing the library accepted before is refused.
- A `File` that fails to read becomes the host's I/O error via `hostIoError`; a short slice read does the same.
