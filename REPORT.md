# t-542 report

## Stop: an existing test pins the old OPFS behaviour

`test/ops/core/pdf_runtime_safety_battery.dart`, "a save whose cancel fired after the engine finished resolves only after the sink took its last chunk", ends with an OPFS clause: `expect(_ok(save.bytes), isTrue)` with the comment "OPFS writes are fire-and-forget: the engine is long done when the cancel lands, so the save succeeded and must have waited".

With the spool (item 1) a cancel that lands while the output is being delivered cancels the job, as the plan says ("a cancel during that delivery makes the op ... cancel") and as t-561 does on every other mode: the other branch of the same test already accepts `[2]` for those. I did not edit the test. It fails in the OPFS runner and is the only failure there. To make it pass the OPFS clause has to go, or a cancel after the engine finished has to let delivery complete; that is a maintainer decision.

I also added `LaneMsg.spoolRead` and `LaneMsg.spoolChunk` to the two tag sets in `test/runtime/web/executor_worker_sync_test.dart`. That guard fails on any tag the worker posts that is not listed, so it had to learn the new messages; no behaviour it pinned changed.

## Items

### 1. OPFS output spool

- Cause: `lib/src/runtime/web/lease.dart` `_serveChunk` ("OPFS mode: fire-and-forget writes, no ack channel"), `web_assets/executor_worker.js` `hostWriteChunk` default branch (`postMessage`, return 0).
- Fix: the worker opens one spool file per sink before `execute` (`openSpools`, `pdf_spool_<job>_<sink>`, registered in `jobFiles`, so `opfsDrop` and reset remove it), `hostWriteChunk` writes to it through the sync access handle (`writeToSpool`), and `result` carries the sizes (`spool`). The lane holds the result: `_finishAfterSpool` / `_deliverSpool` ask `spoolRead` for at most 256 KB, await the sink's `write`, then ask for the next. A sink error fails the job (`PdfIoFailed`), a cancel or the end of the lane makes it cancelled; the ops layer aborts the sink once. The fire-and-forget branch is deleted. `docs/ARCHITECTURE.md` (Sinks, OPFS) describes it.
- Tests: the abort battery group "an op whose last write fails ..." now runs in OPFS (skip removed). In `web_lane_protocol_battery.dart`: "the main thread holds at most one chunk at a time" (1 s stall per write; counts chunk-bearing messages arrived minus writes finished), "a sink that fails fails the job, and the output is not kept on disk", "the output of a save is removed from disk once delivered", "a cancel while the output is delivered cancels the job and removes the output".
- Failure on the old code (stashed lease, worker, protocol): `Expected: a value less than or equal to <1> Actual: <2> chunks piled up on the main thread behind a slow sink`; the failing-sink test `Expected: false Actual: <true> the sink refused a chunk`; the three abort-battery tests failed before the fix. The two disk-removal tests and the cancel test cannot fail on the old code (no spool file existed, and the old path ignored the cancel only by accident); they guard the new files.
- Choice: a sink write in flight is not interrupted by a cancel, only by the end of the lane (as before); delivery stops before the next chunk.

### 2. Budget and the liveness lock

- Finding: there is nothing to fix in the pool. `ExecutorPool._retire` removes the entry from the count and then calls `terminate()`; `WebExecutor.terminate` does not wait for the lock (`_reapDeadWorker` is unawaited and only reclaims the directory). The old "slot returns on the lock" design (commit b72a04d) was replaced by the pool. What remained were two stale comments in `executor_worker.js` that still said the budget waits for the lock; I corrected them.
- Test: "a terminated worker leaves the budget at once, whatever its liveness lock does": pool with `maxExecutors: 1`, `keepBytes: 0`; the page never grants worker-lock requests; after the release `liveCount` is 0 and a second `acquire` is served.
- It passes on the current code. Injected failure: `_retire` removes the entry 60 s late; the test fails with `Expected: <0> Actual: <1> the retired worker still counts`. Restored, `git diff` clean.

### 3. Native waiter killed while waiting for a slot

- Cause: `vendor/pdf_oxide/src/host/native/executor_table.rs` `terminate` never took the executor off `Budget::waiters`; its jobs were answered only when `SlotGuard::drop` met it.
- Fix: `terminate_in` removes the waiter and `cancel_pending` answers its jobs. To hold the budget at one, `Budget` has a `limit` and `boot_in`/`terminate_in` take it; the global functions pass the global budget (no behaviour change).
- Test: `a_lane_killed_while_waiting_for_a_slot_completes_its_jobs_at_once` (budget of 1, holder keeps the slot).
- Old code: `the job of a lane killed in the queue waited for a slot to free`.

### 4. Undisposed `Pdf`

- Held past its handles: the router's lanes (each holds a pool executor), the registered fallback fonts (`_preludes`), pool membership. Nothing gave them back.
- Fix: `createBridge` attaches `HandleBackstop` to the `SharedBridge` with `router.dispose` as release; `Pdf.dispose` detaches. The debug warning says "a Pdf was never disposed; call dispose()" (the backstop takes a `remedy`).
- Choice (narrower): the owner is the bridge, not the `Pdf`. The bridge is held by the `Pdf`, every document and builder handle and every op in flight, so the backstop fires only when nothing can use the engine side; attached to the `Pdf` it would dispose the router under a live `PdfDocumentHandle` or a running `Pdf().merge(...)`. Tests reach the owner through `HandleBackstop.ownerOf(pdf)` (`@visibleForTesting`), set by the `Pdf` constructor; a public member would have been a README door.
- Tests (instance battery, all five platforms): "an undisposed Pdf is released and logged", "a disposed Pdf is not released twice".
- Old code: `Invalid argument (owner): is not registered with the backstop: Instance of 'SharedBridge'`.

### 5. Blob URL of the cross-origin bootstrap

- Cause: `host.dart` `_spawnWorker` made the URL and dropped it.
- Fix: `_spawnWorker` returns it; `_bootWorker` revokes it on `booted` and in the failure path.
- Tests: "the blob URL of a cross-origin boot is revoked once the worker has started", "... that fails is revoked" (page `createObjectURL`/`revokeObjectURL` patched; script blobs counted).
- Old code: `Expected: <0> Actual: <1> the bootstrap blob URL is never revoked` and `a failed boot leaves its blob URL`.

### 6. t-493

Test "t-493: a render of the first page of a large document, behind a slow disk and a create that does not answer once, finishes" (OPFS): every `opfsWriteAck` 150 ms late, every `removeEntry` 400 ms late, the first `createSyncAccessHandle` unanswered until the retry, a 1000-page open and a first-page render, under 40 s, directory empty afterwards. It passes on the fixed code, so it covers a slow ack, a slow reclaim and the one hung create (the 5 s retry); none of them hangs. It does not cover an ack that is lost for good: by design nothing waits on a clock there, a cancel or the end of the lane wakes the pre-copy, and I found no code path that loses an ack on its own. Injected failure: acks dropped instead of delayed, the test times out ("Test timed out after 8 seconds" with the budget cut to 8 s). Restored.

## Summary lines

- `dart analyze --fatal-infos lib hook test bin`: No issues found!
- `dart format --set-exit-if-changed lib hook tool test bin`: Formatted 277 files (0 changed)
- `dart test test/guards test/runtime test/bridge test/keep test/hook`: `01:15 +466: All tests passed!`
- `bash tool/check_patch_markers.sh`, `check_fork_tests.sh`, `check_cancel_polls.sh`, `check_changelog_shape.sh`: exit 0
- native runner: `01:19 +668 ~1: All tests passed!`
- OPFS runner: `04:24 +707 ~6 -1: Some tests failed.` (the one test above)
- JSPI: `02:58 +698 ~3: All tests passed!`
- Atomics (`-p chrome-coi`): `02:15 +695 ~4: All tests passed!`
- Asyncify: `03:24 +698 ~3: All tests passed!`
- Rust (`cargo nextest run --lib`, fork features as in PLAN.md): `Summary [ 617.088s] 6726 tests run: 6726 passed (8 slow), 5 skipped`

The runners were run as `dart test <runner> -p chrome[-coi] --concurrency=1`, not through `make`, which would have rebuilt the wasm. `web_auto_mode_test.dart`, which `make test-ops-asyncify` also runs, was not run. The Rust run is `--lib` only: the integration targets filled the disk.

## Other choices

- A cancel during spool delivery returns the cancelled status even though the engine finished; no op that has a sink creates a handle (`_handleCreators`), so no success is lost.
- Output of an op that fails in the engine is not delivered at all on OPFS (earlier chunks used to reach the sink before the failure); the ops layer aborts the sink either way.
- CHANGELOG.pre.md: four Fixed bullets (items 1, 3, 4, 5); items 2 and 6 fixed nothing user-visible.
