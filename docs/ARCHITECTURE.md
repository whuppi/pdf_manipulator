# pdf_manipulator — Architecture

How the package is wired. Four layers, two platforms, one binary format end
to end, bounded-buffer I/O everywhere. Sections go from the public API down to
the engine. Each section says a thing once and names the file or constant that
holds it. A number that can change is not copied here: the constant's name is.

---

## The contract

Eight guarantees. Every decision in this document serves one of them.

1. **Four layers.** Consumer API → bridge → runtime → host layer. Each layer
   has one job. No layer imports from a layer above it.

2. **One binary format.** The bridge encodes every op as bytes. The bytes are
   the same on native and web, and the same Rust parser reads them on both.

3. **One Rust entry point.** `bridge_api.rs` compiles for native
   (`extern "C"`) and for wasm (`#[wasm_bindgen]`). Same source, same parser,
   same dispatch.

4. **Sources and sinks.** An op takes any number of indexed `DataSource`s and
   `DataSink`s. The engine reads a source on demand with `readAt`. A sink
   receives chunks. The Dart side never reads a whole file.

5. **Bounded transport.** Input and output move through fixed buffers
   (`READ_BUF_CAPACITY` and `WRITE_BUF_CAPACITY` in `host/constants.rs`), on
   both platforms. The test guards throw on an over-budget `readAt` or
   `write`. The engine's own memory is a separate claim, measured by
   `make test-memory` (see **Memory**).

6. **Upstream bindings uncalled.** `ffi.rs` and `wasm.rs`, upstream's own
   binding surfaces, are never called by the host layer. They are patched only
   where the engine's own types force it (a type they use changed shape),
   between markers, so every feature still builds and upstream merges stay
   clean.

7. **Executors are pooled and leased.** An executor is a Rust thread with its
   own engine state (native) or a Web Worker with its own wasm instance (web).
   One pool owns every executor of a platform configuration. A `Pdf`'s lanes
   lease executors from it and give them back. The Dart isolate never blocks.
   Dispose ends lanes and returns executors; it joins nothing, awaits nothing
   and sets no timeout. The pool terminates an executor only when its engine
   died, a reset failed, its heap passed the keep line, or more executors wait
   idle than the pool keeps.

8. **Patches are marked.** Every change to a vendored upstream file sits
   between boundary comments (`── pdf_manipulator patch ──` in pdf_oxide and
   tiny-skia, `── office_kit patch ──` in office_oxide).

---

## Glossary

One name per concept. A name in this table means the same thing in the Dart
runtime, the Rust host layer, the web executor script and every document.

| Name | Meaning |
|---|---|
| **instance** | One `Pdf()`: a router and its lanes. |
| **executor** | The platform resource: one Rust thread with its engine state (native), or one Web Worker with its wasm instance (web). The pool owns it. Its states are booting, idle, leased, draining, dead. |
| **engine state** | What one executor holds between resets: open documents, editors, builders, the handle counter and the engine caches. Rust `EngineState`. |
| **engine caches** | The parsed fonts, the CMaps and the registered fallback fonts of one executor. Rust `EngineCaches`, owned by the engine state. |
| **pool** | The one owner of the executors of a platform configuration, page- or process-wide: budget, idle set, order, prewarm, keep line. `ExecutorPool`. |
| **booter** | The per-platform object that boots executors and reports the I/O mode, the default concurrency and the pool key. `ExecutorBooter`. |
| **boot** | Start an executor until it is ready for a lease. On web the handshake is booted → init → ready. |
| **prewarm** | Boot one executor ahead of the first job, once per pool. |
| **lane** | One instance's queue of work: its pins, its holds, and a lease on one executor from its first job until it is idle with nothing pinned or held. `Lane`, in `pool.dart`. |
| **lease** | One lane's use of one executor, named by a number from the executor's own counter. The number is never reused. Every message carries it. The executor drops a message that carries an older one. |
| **lease adapter** | The platform-blind driver of one lease. It has five verbs (`submit`, `cancelJob`, `releaseHold`, `reset`, `drain`) and decides nothing. Dart `Lease`: `NativeLease`, `WebLease`. |
| **router** | One per instance. It translates handle ids, pins handles to lanes, places other jobs on the least loaded lane, owns resources, replays preludes and ends lanes on dispose. `Router`. |
| **op** | The engine operation named on the wire: an `EngineOp.wire` string. |
| **request** | The encoded bytes of one op and its arguments. |
| **job** | One request submitted to one lane, with its cancel flag. Created → queued → posted → running → done, cancelled or failed. Every accepted job posts exactly one result. |
| **post** | The one result a job sends back: native to its Dart port, web as the `result` message. Cleanup of the job's resources waits for it. |
| **task** | The public `PdfTask` the caller holds. It maps to one job, or to several for a combinator. |
| **cancel flag** | A set-once flag that ends a job: the job's own flag (set by `PdfTask.cancel`) and, on native, the executor's flag (set by terminate). On web the job's flag is a word in a `SharedArrayBuffer`, or a blob URL the main thread revokes. |
| **cancel poll** | The engine reading the cancel flag once per page, image or rule batch. Rust `CancelCheck`. |
| **hook** | The Dart `CancelHook` that binds `PdfTask.cancel` to the actions it runs. |
| **reset** | Replace an executor's engine state with a new one between leases. |
| **drain** | End a lane's use of its executor: waiting jobs complete cancelled at once, the running job is cancelled, and the lane is done when that job has posted. |
| **release** | A lane gives its executor back and stays usable. Its next job leases again. |
| **end** | A lane is finished for good, because the instance was disposed. |
| **retire** | The pool's decision to remove an executor from its count and terminate it. |
| **terminate** | The platform act that stops an executor: `worker.terminate()` on web, `executor_terminate` on native. |
| **dead** | An executor whose engine trapped or panicked. It is never leased again. |
| **keep line** | The heap peak at which a returned executor is retired instead of reset. `KEEP_BYTES`. |
| **heap** | An executor's peak live bytes since its last reset. `Executor.heapBytes`, wire field `peak`. |
| **prelude** | A job the router replays on every new lease before the user's jobs. A registered fallback font is one. |
| **pin** | A handle bound to the lane whose executor holds its engine handle. |
| **engine handle** | The id of a document, editor or builder in one executor's engine state. Ids collide across executors. |
| **global handle id** | The router's id for a pinned handle. It never collides. |
| **handle** | The public owned object (`PdfDocument`, `PdfEditor`, `PdfBuilder`). |
| **kept source** | A job's source that the engine moves into a handle, so its I/O outlives the job. |
| **hold** | The lane's platform object for a kept source, named by an opaque token: a native channel kept registered, or a web reader filed in the handle's group. |
| **resource** | The router's id for the holds of one handle. They are released together with the handle. |
| **channel** | The native pair of shared buffer and condition variable that serves one source or one sink to the thread. Native only. |
| **source**, **sink** | The public `DataSource` and `DataSink`. In an op, `source 0` is the first one. |
| **reader**, **writer** | The Rust `Read + Seek` and `Write` adapters over a source or a sink (`CallbackReader`, `JsCallbackReader`, `BoxedWriter`). Rust only. |
| **registry** | Rust's table of executor keys to controllers (`TABLE` in `executor_table.rs`). |
| **thread budget** | Rust's cap on started executor threads, `MAX_EXECUTORS_GLOBAL`, with a first-in-first-out queue of pending starts. |
| **slot** | One started-thread allowance in the thread budget. |
| **host layer** | `vendor/pdf_oxide/src/host/`: everything of ours inside the engine. |
| **mode** | The I/O mode, `PdfIoMode`: native, atomics, jspi, opfs or asyncify. |

---

## The four layers

Each layer has one job and never imports from a layer above it. The full file
list is in **Source tree**.

**Layer 1 — Consumer API** (`lib/src/ops/`). `Pdf` holds the lifecycle and
every door: the handle doors (`open`, `edit`, `build`), the scope doors (`read`,
`rewrite`, `buildTo`) and the one-call doors (`merge`, `split`, `compress`,
`sign`, ...). `PdfDocument`, `PdfEditor` and `PdfBuilder` are the handles.
Platform-blind; it talks only to the abstract `PdfBridge`.

↓

**Layer 2 — Bridge** (`lib/src/bridge/`). One `SharedBridge` for both
platforms. It encodes each op to bytes, sends it through the `PdfTransport`,
decodes the response into typed Dart, makes every `PdfTask` with its
`CancelHook`, and aborts the sinks of an op that ends cancelled or failed.
Platform-blind.

↓

**Layer 3 — Runtime** (`lib/src/runtime/`). The `Router` pins handles and
places jobs. Each of its lanes leases an executor from the `ExecutorPool`.
A lease adapter (native over FFI, web over `postMessage`) drives each lease, and
an `ExecutorBooter` boots the executors. The adapters move bytes and serve
`readAt` and chunk callbacks on the main isolate. They make no routing
decision and hold no PDF knowledge.

↓

**Layer 4 — Host layer** (`vendor/pdf_oxide/src/host/`, entirely ours). One
entry point (`bridge_api.rs`, cfg-gated for native `extern "C"` and wasm
`#[wasm_bindgen]`) over a binary parser and one op unit per op. Upstream's own
`ffi.rs` and `wasm.rs` are never called and are patched only where the engine's
types force it (see **Upstream patches**).

---

## Data flow

### Simple op (one source, no output)

```
pdf.open(PdfInput(source))
  ↓
SharedBridge.open()
  │  encodeRequest('open', {sourceLength: N})  →  request bytes
  ↓
Router.execute(request, sources: [source], keepSources: {0})
  │  pin lookup, or placement on the least loaded lane
  │  the lane leases an executor when it holds none
  │
  ├── Native: executor_submit via FFI → executor mailbox
  └── Web:    postMessage → executor_worker.js → execute via wasm
  ↓
bridge_api.rs  (the same code on both targets)
  │  Request::parse(bytes) → registry::find("open") → the op unit's handler
  │  source bytes served on demand through the reader (readAt)
  ↓
response bytes → decodeResponse → the router swaps the engine handle id for a
global handle id → PdfDocument
```

### Multi-source op (primary PDF plus a secondary input)

```
editor.appendPages(other)
  ↓
SharedBridge: _exec(editorMergeFrom, {sourceLength: N},
    sources: [other],        ← the secondary source, index 0 of this job
    keepSources: {0})        ← kept for the editor's lifetime
  ↓
Router.execute(...)
  │  the lane serves readAt by source index
  │  on success the hold joins the editor's one resource
  ↓
bridge_api.rs
  │  source 0 = other, read through the reader in read-buffer pieces
  │  the editor walks the other document's page graphs and records ids only;
  │  the reader stays inside the editor and the save streams each object from it
  ↓
response bytes
```

The editor's own source and every appended source share one resource id and one
release (`editorDispose`, then `releaseSource`). An append that fails registers
nothing: the router releases the job's holds at once.

### Editor mutations — staged, except the destructive ones

`DocumentEditor` never changes the source `PdfDocument`. It is a copy-on-write
overlay: each mutation files its *intent* into a staging field, and
`save_to_bytes()` writes the whole set in one pass.

```
editor.rotatePages()  → modified_page_props[page]     (params only)
editor.removePages()  → page_order                    (index list)
editor.addWatermark() → overlay_additions[page]       (elements)
editor.setMetadata()  → modified_info
                              ↓
                        save_to_bytes()   ← every byte is produced here
```

Each Dart call is a real round trip to the engine (`_mutate` →
`handle_editor_mutate` → `dispatch::edit_*`), so a bad argument fails at the
call, not at save. Only the *writing* is deferred.

**Destructive ops are the exception: they do their work at call time.**
`applyRedactions` and `redact` both run the shared `destroy_regions_on_page`
core at once (prune glyphs, prune images, scrub font subsets) and stage the
*result*, not the request:

```
editor.applyRedactions()
  │  RegionSet = /Redact annots + regions queued by markRedaction
  │  destroy_regions_on_page()          ← the removal happens NOW
  ├─→ redacted_content[page]      = the rewritten content bytes
  └─→ redacted_orphan_ids        += object ids of the ORIGINAL /Contents
                              ↓
                        save_to_bytes()
                          ├ redacted_content becomes the page's single new
                          │ /Contents; the cosmetic overlay path is skipped
                          └ redacted_orphan_ids are dropped BEFORE the
                            reachability check — pre-redaction bytes can
                            never be emitted, even if GC would keep them
```

The asymmetry is deliberate. Ordinary mutations are additive or positional, so
they compress to a few parameters the writer can compose. Removal cannot: the
content stream has to be parsed and rewritten, and there is no parameter form
of "these glyphs are gone". Running it at call time also makes the returned
report (regions, glyphs removed, bytes removed) and `redactionCount()` true
before the file is written.

Redaction is therefore a two-phase lifecycle: `markRedaction` marks (staged,
like every other op) and `applyRedactions` destroys (at call time). `redact` is
the same destructive core in one call over its own regions. A page destroyed
before is destroyed again over its earlier regions, so the later record keeps
what the earlier one removed.

---

## The runtime

A `Pdf` has one `Router` (`router.dart`). Its lanes (`Lane`, in `pool.dart`) lease executors
from the `ExecutorPool`, and a lease adapter (`NativeLease`, `WebLease`) drives each lease.
The router and the pool run on the main isolate's event loop: one owner, no lock. An engine
state has one owner too, its executor. Only the executor key, the mailbox, the cancel flags
and the channel registry cross threads.

### The pool

One pool serves every instance of a platform configuration (`ExecutorPool.of`, keyed by the
booter's `poolKey`: one key on native, the `executor_worker.js` URL and I/O mode on web). It lives as long
as the process or the page.

```
booting → idle → leased → draining → idle
                                   ↘ dead
```

- **Budget.** At most `kMaxExecutors` executors are live (booting, idle, leased or
  draining), a fixed number, not derived from the device. A terminated executor leaves the
  count at once, so a waiting request is served the moment one is freed.
- **Order.** A request takes the idle executor with the smallest heap, else boots one under
  the budget, else waits. Waiters are served in arrival order. A booting or draining
  executor counts as on its way to idle, so a `Pdf` disposed and replaced in a loop reuses
  one executor. It counts so for `kCoverFor` only; then it is recorded as a slow transition
  and hides no waiter.
- **Prewarm.** The first use boots one executor ahead of the first job. A failed prewarm
  is silent; the first request boots again.
- **Release and retire.** The pool drains the lane, reads the executor's heap, then resets
  it (a lane that never ran a job skips the reset). It terminates the executor instead when
  the engine died, the reset failed, the heap reached the keep line (`kKeepBytes`, the
  mirror of `KEEP_BYTES`; see **Memory**), or more than `kMaxIdleExecutors` are idle.

### The lane

The router makes a lane when none exists, or when all are busy and fewer than
`PdfConfig.concurrency` exist (else the booter's `defaultConcurrency`, `max(2, cores ÷ 2)`).
A job with no handle goes to the least loaded lane.

- **Pins and holds.** A job on an existing handle runs on the lane that owns it: ops on one
  handle run one after another, ops on different handles in parallel. The router maps the
  global handle id to the engine handle id, because every executor counts engine handles
  from 1. A lane keeps its executor while it has a pin or a hold.
- **Lease on first job.** A lane has no executor until its first job. The job waits in the
  lane while the pool serves the request; then the preludes run, then the waiting jobs, in
  order. A failed boot fails the jobs waiting for it.
- **Release when idle.** After every job the router asks whether the lane has no job, no pin
  and no hold. If so, `Lane.release` gives the executor back. The next job leases again, and
  waits for the returned executor for `kCoverFor` at most.
- **End on dispose.** `Router.dispose` ends every lane (`Lane.end`): waiting jobs complete
  cancelled at once, the running job is cancelled, and the executor goes back to the pool
  when that job has posted. The router clears its maps and returns.

### The lease

A lease is one lane's use of one executor. Its number comes from the executor's own counter
(`begin_lease` on native, `WebExecutor.beginLane` on web), starts at 1 and is never reused.
A native executor refuses a lease while a job is outstanding or a held channel is
registered. Any executor refuses one after its engine died.

- **Every message carries the number.** Native: `executor_submit`, `executor_job_cancel`,
  `executor_reset` and `executor_hold_release`. Web: every message after the boot handshake
  has a `lease` field.
- **Stale drop.** Native `admit_job` answers a job of an older lease with
  `STALE_LEASE_MESSAGE` and touches nothing else; a cancel under an older lease changes
  nothing. On web the adapter ignores a message of another lease. The executor serves its
  current lease or, while idle, a newer one; a newer one a busy executor cannot admit is
  answered with an `error` that has `protocol: true`.
- **Adapter verbs.** `submit`, `cancelJob`, `releaseHold`, `reset`, `drain`, and nothing
  that decides: `adapter_decides_nothing_test.dart` fails when an adapter file names an op,
  a handle or a pin.

### The job

```
created → queued → posted → running → done | cancelled | failed
```

The router creates the job. It queues in the lane while no executor is leased, then in the
adapter (the native mailbox, or a queue in Dart on web, because a Worker cannot look into
its own message queue). An executor runs one job at a time.

**Every accepted job posts exactly one result:** a success, an engine error, the cancelled
status, the stale-lease error or the dead-engine error. Native `SlotGuard` posts for the
jobs queued on an executor terminated before its thread started.

**Cleanup runs after the post.** Dart frees a job's buffers and closes its callbacks only
when the post arrives, and the native executor removes a job's channels from its wake
registry before it posts. So no thread calls a closed callback (a fatal abort on Windows).

### Engine state and reset

`EngineState` holds an executor's documents, editors, builders, handle counter and
`EngineCaches`: parsed fonts, CMaps and registered fallback fonts. `EngineState::new`
installs the caches in a thread-local pointer for its life, so no upstream signature
changed. With none installed (the bindings, upstream tests) `caches::with` uses one process
default. On wasm32 a `thread_local!` is a plain static, so the same source is per executor
on both platforms.

A **reset** drops the engine state and makes a new one. Documents, editors, builders, caches
and registered fonts go; the thread or Web Worker and the wasm instance stay. `install` also
empties the thread-locals the engine keeps (warning sink, resolving stack, office context,
rasteriser fill count) and starts the heap peak over. A reset is refused while a job is
outstanding (`executorBusyMessage`).

Registered fonts are per executor, so the router keeps each registration as a **prelude**
and replays it at every lease. `engine_proofs/shared_state.txt` lists every
static and thread-local of the engine with its class (`executor`, `thread`, `machine`,
`registry`, `upstream-binding`); `shared_state.rs` fails on one that is not listed. The
machine's own caches (system fonts, the renderer's font and face caches, the PDF/A font
database) stay process-wide and are counted to the process (`heap::process_owned`).

### Thread budget and registry (native)

Dart holds executor keys, never pointers. `TABLE` in `executor_table.rs` maps a key to its
controller; keys come from a counter and are never reused, so a call that races a terminate
finds nothing and gets a defined answer. The thread budget (`MAX_EXECUTORS_GLOBAL`) caps started
executor threads in the process. At the cap, `executor_boot` still returns a key, and the
thread starts later, first in first out, when an exiting thread hands over its slot
(`SlotGuard`, also on unwind). A boot queues; it never fails. A caller meets the pool's
`kMaxExecutors` first; the thread budget is reached only while terminated threads linger.

**Terminate** is instant, idempotent and never joins: set the executor's cancel flag, wake
every parked channel under its lock, close the mailbox, remove the key. After it returns, no
callback fires for that executor.

### Dead executors and panics

Native: `handle_request` runs under `catch_unwind`. A panic sets `dead` before the job's
post, drops the engine state and closes the mailbox; later jobs post `ENGINE_DEAD_MESSAGE`.
Web: a wasm trap (`WebAssembly.RuntimeError`) is posted with `trap`, and the lease adapter
marks the lane dead. The result carries `executorDied`. The router takes the lane out of
placement, fails the handles pinned to it with `engineDeadMessage` (disposing one succeeds),
and gives the executor up once its jobs have answered. The pool finds the engine dead and
terminates it. The next job boots another.

### Web executors

An executor is a Worker running `executor_worker.js`. The booter compiles the wasm module
once per page and hands it to each executor.

- **Boot.** `booted → init → ready`. The executor announces `booted` when its message handler
  is attached, because a message posted earlier is dropped. No deadline applies: every
  failure arrives as a typed error. Before `ready` the executor takes a Web Lock, in every
  mode; the browser releases it when the executor dies.
- **Terminate.** `worker.terminate()` is a request. The adapter completes its pending jobs
  as cancelled at once, and the pool's count does not wait for the browser to reap the
  executor. In OPFS mode the booter reclaims its directory once it can acquire the lock.
- **Reset.** A web executor also drops its readers and job files and empties its own OPFS
  directory. Its wasm memory does not shrink (`mem`, a diagnostic); it reports `peak`.
- **An unanswered `createSyncAccessHandle`.** The call that creates the handle of a fresh
  pre-copy file can stay unanswered; Chrome has been seen to do it under full CPU load.
  Nothing waits for that call but the write that made it. A reset, or the end of the job,
  sets the file aside. The call's own continuation closes the handle, removes the file and
  takes it off the abandoned list if the call ever settles, and the directory cleanup skips
  a listed file. A live pre-copy write gives the call `kCreateHandleWait`, then sets its
  file aside and creates the handle once on a fresh file. If that call is not answered
  either, the write fails and the job fails with `PdfIoFailed`. The executor reports abandoned
  files on `resetDone` and a retried create on the write's acknowledgement. Both are counted
  (`WebExecutorBooter.abandonedFiles`, `WebExecutorBooter.retriedCreates`), and the suites
  assert they stay zero.

---

## Cancellation

### The outcome contract

`PdfTask.cancel` documents this contract.
- If the engine had not finished when the cancel landed, the task resolves with
  `PdfCancelled` and the operation did not happen. A mutating op leaves its handle exactly
  as before. A sink the op wrote to is aborted (`DataSink.abort`: called once, no write
  after it, and the owner still closes it), so it holds no partial output.
- If the engine finished first, the task resolves with its result, as if cancel had not been
  called. One exception: an op that opens a handle (`open`, `editorOpen`, `builderCreate`)
  is disposed by the library and resolves with `PdfCancelled`, because a cancelled caller
  never received the handle.
- A combinator (`read`, `rewrite`, `buildTo`, `readOnce`, `editOnce` and the one-call doors)
  resolves with `PdfCancelled` whenever cancel was called before it finished. Its handle is
  the library's and is disposed. Its output sink is aborted.
- `isCancelled` reports the request, not the outcome. A task nobody listens to absorbs
  `PdfCancelled` silently; any other failure on it is raised into the zone.

A mutating op is all or nothing. Only `reduceImages` and `convertToPdfA` poll for a cancel
inside a mutation. `reduceImages` stages its replacements after its last image;
`convertToPdfA` runs under `editor.undoable`, which restores the editor on any error.
`engine_proofs/cancel_at_every_poll.rs` cancels both at every poll and asserts the editor is
as before or the op succeeded.

### From task to job

`PdfTask.cancel` calls its `CancelHook`. The router registers the job's cancel with the hook
when it submits the job (`lane.cancelJob`). A cancel before the submit resolves the op as
cancelled without reaching a lane; the router checks cancel and dispose again, synchronously,
just before the submit, so a same-tick dispose cannot start a lane nothing ends.

- A job waiting in the lane or the adapter queue completes cancelled at once and never
  reaches the engine. A running job has its cancel flag set and its parked I/O woken. The
  executor survives, and the job's result posts as cancelled.
- The engine never rewrites a success: an op that creates a handle and finishes in the same
  window as its cancel has put a document in the executor, and only the router can name it.
  The router disposes that handle on its own lane (`_disposeOrphan`) and the caller gets
  `PdfCancelled`. A failure after a cancel reads as the cancelled status. On web a success
  waits for its sink writes before it ends.

### The cancel flag, per mode

| Mode | Flag | How the engine reads it |
|---|---|---|
| native | The job's `AtomicBool`, filed in `cancel_flags` before the job is queued, and the executor's (terminate). `CancelFlags` holds both. | `CancelCheck::from_flags` loads both. |
| web with `SharedArrayBuffer` | A word of a per-job `SharedArrayBuffer` (`cancelSabBytes`). The main thread stores 1. | The wasm import `host_cancelled` loads it. |
| web without (OPFS, and JSPI or Asyncify on such a page) | A per-job blob URL (`cancelUrl`). The main thread revokes it. | `host_cancelled` polls it with a synchronous XHR, at most once per `CANCEL_POLL_MS`. A poll that times out (`CANCEL_POLL_TIMEOUT_MS`) is not a cancel; a failed poll is. Asyncify keeps this path: it reads the flag where it already does, and unwinding at the poll would add nothing. |

Parked I/O is woken separately. Native: a job cancel takes each channel's pair mutex, sets
`FLAG_CANCELLED` and notifies. Web: a pending read or write is answered with the cancelled
status, and an OPFS pre-copy waiting on the caller's `DataSource` is woken (`cancelWake`).
The cancelled error is `ErrorKind::Other`, because `std` retries `Interrupted`.

### The cancel poll

Readers and writers check the flag at every I/O call. A loop that works between I/O calls
calls `CancelCheck::check` (or `each`, or a `Stride`, which checks every `STRIDE` iterations
of an inner loop) once per page, image or rule batch. Code with no check in scope calls
`cancel::poll`, which reads the check that `handle_request` entered for the job. A cancel
therefore stops a running op within one unit of work, even when the loop makes no I/O call.

### The combinator's one hook

A `CancelHook` holds any number of actions. `register` returns the function that unregisters
one, `cancel` runs each once, and an action registered after the cancel runs at once. A
combinator opens its handle through `_own`, which gives the handle the combinator's hook as
its scope; each op on that handle registers its cancel with the scope while it runs.
Cancelling the combinator cancels every op in flight, makes a later op fail at once without
reaching the engine, skips the save and disposes the handle. The dispose op carries no
scope, so it always runs. A handle the caller opened has no scope.

### The walls

- `make test-fuel` (`tool/fuelprobe`) runs every op in wasmtime with fuel metering. It fails
  any gap of work between two polls over `MAX_GAP`, and any case under `MIN_TOTAL`.
- `tool/check_cancel_polls.sh` (in `make test-guards`) fails a loop over a document-scale
  collection that does not poll; `tool/cancel_poll_allow.txt` lists the exceptions.
- `engine_proofs/cancel_polls.rs` and `cancel_at_every_poll.rs` cancel at every poll and
  assert the document or editor is as before. `pdf_cancel_battery`,
  `pdf_combinator_cancel_battery` and `pdf_abort_battery` assert the contract per platform.

---

## Memory

### The promise

Memory is bounded for any number of pages. Every operation stays under a fixed ceiling
over the memory the process already holds, whatever the size of the file (the `limit`
rows of `tool/memprobe/src/main.rs`). One input does not fit that ceiling: the text of a
page. Reading order is a sort over all the text pieces of a page, and the first word can
be the last piece in the file, so no text leaves until every piece of the page is read. A
page's text table is whole by nature, like the xref. So one page's text costs a declared
amount per text piece of the one page being read, on top of the ceiling.

The amounts are in `host/piece_budget.rs`: `EXTRACT_BYTES_PER_PIECE` and
`EXTRACT_BYTES_PER_FONT` for a whole extraction (per text run and per font the page
names), and the stage budgets `STAMP_BYTES_PER_PIECE`, `DEDUPE_BYTES_PER_PIECE`,
`ASSEMBLY_BYTES_PER_PIECE` and `LOAD_FONTS_BYTES_PER_FONT`. Engine proofs
(`engine_proofs/mem_page_text.rs`) assert each, and the `extractAll fonts` and `tagged`
rows of `make test-memory`, at one and two times the page, hold the engine to them: their
limit is the ceiling plus those amounts, so the slope is tested, not one point. An external
sort that spills sorted runs to storage would keep one page under the plain ceiling; it is
not built, and the trigger to reconsider it is a user with single pages too large for the
per-piece cost and real storage to spill to.

### The counting allocator and the heap

One `#[global_allocator]` (`Counting` in `host/heap.rs`) forwards every allocation to the
platform allocator (the module's own on wasm32) and counts it twice: in a per-thread pair
of live and peak bytes, and in a process-wide pair.

- An executor is one thread, so the per-thread figure is the executor's **heap**, blind to
  every other executor. It is the peak of live bytes since the last reset. `install`
  starts it over; `Executor.heapBytes` reads it. Native stores it in the controller after
  each job and each reset (`executor_heap_peak`). A web executor exports `heap_peak` and
  posts it as `peak` on `result`, `error` and `resetDone`. Native and web mean the same
  thing by it.
- Memory that crosses a thread would skew the figure, so the boundary moves it by hand:
  `heap::adopt` for the request and channel lists that Dart allocates and the executor
  frees, `heap::release` for the response the executor allocates and Dart frees.
  `engine_proofs/heap_counts.rs` and `executor_boundary.rs` assert it.
- The machine's font caches (`MACHINE_FONT_BYTES`, `MACHINE_FACE_BYTES`) serve every
  executor. Their inserts and evictions run inside `heap::process_owned`, which counts
  them to the process. An evicted value that another executor still holds can lower that
  executor's figure by its size. That can only keep an executor that should be kept,
  never retire one wrongly.
- The web `mem` field is the wasm linear memory. It never falls and no decision reads it.

### The keep line

`KEEP_BYTES` (`host/heap.rs`) is the heap peak at which a returned executor is retired
instead of reset. The next job boots a replacement. The line is one resident document
(its object cache `DEFAULT_OBJECT_CACHE_MAX_BYTES`, its content cache
`CONTENT_CACHE_BYTES`, its fonts `DOCUMENT_FONT_BYTES`) plus one operation, which peaks at
most the ceiling above its own baseline. The rows `hugeJpeg` (render, reduceImages) and
`hugeJbig2` (render, extractImages) in `limit()` are exceptions with their own ceilings.
Their executors are retired by design. Dart's `kKeepBytes` mirrors the constant, and
`keep_bytes_test.dart` reads `heap.rs` and compares them.

### The wall

`make test-memory` runs the probe's own tests, then the probe, in a release build. The
probe builds its inputs once (`many` pages, `huge` streams, a damaged file, an encrypted
file, forms and others), runs each wire step on each of them, and prints one line per
step: `<op> <input> peak_over_baseline=… limit=… ok|FAIL`. It reads the process-wide pair
and exits 1 on any FAIL.

### What the engine holds

The transport is bounded on every path (see **Streaming I/O**). The engine's own memory
is a separate claim, and these paragraphs state what each op holds.

Open, `extractPages`, the full and the incremental save, sign and compress hold the
ceiling on both probe inputs, and so does the full save of a large encrypted stream. The
engine reads through an object cache bounded by `DEFAULT_OBJECT_CACHE_MAX_BYTES`, copies
unchanged streams in fixed pieces (an encrypted stream is decrypted in the same pieces),
and reads page facts without keeping the page dictionaries. A stream of a document is a
window onto the document's bytes that reads itself; only the whole reads listed in
`engine_proofs/whole_reads.txt` hold a stream's data in one buffer. Merge keeps the merged
document open with its own small object cache, records only the ids of the objects each
page reaches, and writes every merged object one at a time from that document.
`tool/check_whole_buffers.sh` fails the build on any engine call that reads a whole stream,
file or page into one buffer unless it is a line of `tool/whole_buffer_allow.txt`, which
admits only the kinds that are tables by nature (a font program, a sampled function, an ICC
profile, the CMap table, a DER certificate or the DSS, the xref stream, one object-stream
member, a progressive JPEG raster, a JPX recompressed at full size).

`extractImages` writes each image to a sink the caller's factory returns, on the same
chunked wire as `render` (one shared framer in the engine, one shared delivery in Dart).
The wire has a header (`kind`, `page`, `index`, `width`, `height`, `bitsPerComponent`,
`encoding`, `color`), data frames of at most `DATA_FRAME_BYTES`, an end frame, and then
the terminator. A JPEG or JPEG 2000 stream goes through byte for byte, decrypted and
never decoded. Every other image is a PNG written row by row from the image cursor, with
its `/SMask` or `/Mask` stream as the alpha channel. The library closes the sink of the
image in flight on an error or a cancel. Images already delivered remain.

`reduceImages` reads an image the same way. The rows come from the image cursor (a
raw-sample stream a strip at a time, a DCT image at the reduction that covers the target,
fax and JBIG2 bitmaps at one bit a pixel, a CMYK image's samples as stored). They go one
at a time through the row resampler and into a row encoder: Flate with predictors, CCITT
G4 by line, or a baseline JPEG whose encoder takes MCU rows. The JPEG uses the standard
Huffman tables, because optimised ones need every coefficient before the first byte. The
engine holds a strip or the codec's raster, the resampler's rows, and the encoded output
of the one image being rewritten, which is the unit the editor stages. A soft mask goes
through the same pipe at its parent's new size. A stored JPEG that is not downsampled is
kept (its quality is read from its first megabyte), unless the policy recompresses JPEGs.
The two exceptions are the `hugeJpeg` row's: a progressive JPEG is one raster, and so is a
CMYK JPEG.

`render` writes each page to a sink the caller's factory returns. The engine rasters the
page in bands (`BAND_BUDGET_BYTES` of raster at a time), passes each band's rows to a PNG
row encoder (or, for `rgba`, writes the rows as they are), and cuts the bytes into frames:
a header (`kind`, `page`, `width`, `height`, `format`), data frames of at most
`DATA_FRAME_BYTES`, and an end frame, then one zero-length terminator after the last page.
Dart opens the page's sink at the header, awaits each data write before the engine goes on
(back-pressure), and closes the sink at the end frame. Neither side holds a page: the
engine holds a band, three PNG rows and one data frame; Dart holds one chunk in transit.
An error or a cancel closes the sink of the page in flight. Pages already delivered remain.

The engine reads an image on a page as the bands reach it and never decodes it whole. It
keeps one open decode per image for the page. A raw-sample image is read through its
filters a strip of rows at a time, and a pure scale-down is resampled row by row to its
device size. Any other transform (rotated, sheared, flipped or enlarged) is drawn from the
rectangle of source samples the band reaches, under the whole-page transform, so the pixels
are those of one whole raster. A DCT image decodes at the largest reduction that still
covers its footprint, and a JPEG 2000 image at the resolution level that covers it. Fax and
JBIG2 images stay at one bit a pixel, and a stencil is kept the same way, so only the
samples a band reaches become pixels. These limits hold:

- A progressive JPEG and a JPEG 2000 image at full size are one raster (the `hugeJpeg` row
  states the peak).
- An image whose band footprint in the source is large (a rotation by an eighth turn spans
  the diagonal) holds the rectangle of that footprint.
- An image drawn flipped or quarter-turned is read again from its first row for each
  band, because the rows its band needs run against the order the stream gives them.
- The open decodes of a page share one byte budget. One that the budget evicted is read again
  from its first row when it is next drawn.

Every other buffer a page needs is a window onto the page too. A transparency group
(isolated or not, with knockout), a soft mask, a blend mode that needs a copy of the
backdrop, a Type 3 glyph, a text clip and a pattern cell each get a pixmap of the band's
rows, at their place in the page, and every composite is per pixel, so a band holds the
bytes the whole page holds. The depth of that nesting cuts the band's height, so memory
follows the nesting and not the page. A group's content is clipped to its `/BBox` (a
knockout group too), because the band culling takes the box for the form's extent. The
engine decides once for the page whether a soft mask's group paints nothing (a mask that
does is dropped, as the other renderers drop it), by drawing the group over the page's own
bands, because a band the group does not reach cannot tell. The content of a form, a
pattern cell, a glyph procedure and a soft-mask group is read as operators; a knockout
group holds its operators, since it replays them before each element. A tiling pattern
whose cell raster fits the texture budget is drawn once to a texture. One over the budget
is drawn tile by tile into the band, where the tiles sit on the lattice and not on the
pixel grid (the one documented difference), and a turned lattice shows its content instead
of the flat average colour a texture gives it.

---

## Streaming I/O

### The transport contract

Input and output move through fixed buffers, never a full-file buffer on the Dart side.
The test guards enforce the chunk limits (see **Test architecture**).

```dart
Future<({Uint8List bytes, Map<int, int> resourceIds})> execute(
  Uint8List request, {
  List<DataSource> sources,     // indexed 0, 1, 2, ...
  List<DataSink>   sinks,       // indexed 0, 1, 2, ...
  Set<int>         keepSources, // kept: the engine moves them into a handle
  CancelHook?      cancel,
});
```

### Per-operation I/O paths

| Operation | Input | Output |
|---|---|---|
| open | source 0 on demand | — |
| extract / search / validate / classify | source 0 on demand | bounded text |
| render | source 0 on demand | per page: header, data frames of at most `DATA_FRAME_BYTES`, end |
| extractImages | source 0 on demand | per image: header, data frames, end |
| editorSave | pinned source 0 | PositionedWrite streams objects |
| builderSave | — | PositionedWrite streams objects |
| sign | source 0 in read-buffer chunks | sink 0, sequential |
| appendPages | source 0, the other PDF, kept for the handle's lifetime and read again by the save | — |
| addImageStamp / addAttachment / builder `image` | source 0, the secondary data | — |
| convertToOffice (PDF → DOCX/PPTX/XLSX) | source 0 on demand | streaming ZIP via office_oxide |
| convertFromOffice (DOCX/PPTX/XLSX → PDF) | whole input buffered (office_oxide reads the ZIP from an in-memory cursor) | CountingWriter streams objects |

### PositionedWrite — Write without Seek

PDF serialization needs byte offsets for the xref table but never seeks backward.
`PositionedWrite` captures exactly that:

- **`CountingWriter<W: Write>`** wraps any Write and tracks the position with a counter.
  It streams in constant memory to sinks that cannot seek.
- **`SeekWriter<W: Write+Seek>`** delegates to `stream_position()`. For Cursor, BufWriter
  and the like.

### Native I/O — condvar and shared memory

Each source gets its own `CallbackReader` and channel. Each sink gets its own
`CallbackWriter` and channel. The executor thread reads and writes through shared memory
with condvar synchronization, and checks `CancelFlags` at every wait. Dart serves the bytes
on the main isolate through `NativeCallable.listener`. No helper isolate exists.

### Web I/O — four modes, auto-detected

| Mode | Mechanism | Requires |
|---|---|---|
| **JSPI** | `WebAssembly.Suspending` suspends wasm, JS fetches async, wasm resumes | Chrome 137+ / Firefox 139+ |
| **Atomics** | `Atomics.wait` blocks wasm, JS fills a SAB, `Atomics.notify` wakes it | `SharedArrayBuffer` (COOP/COEP headers) |
| **Asyncify** | The instrumented wasm unwinds at the read or write, the worker awaits the answer and rewinds | The app shipped the Asyncify module (an entry in `pdf_oxide_manifest.json`, `asyncify: true`) |
| **OPFS** | Sources are pre-copied to OPFS disk, wasm reads through `SyncAccessHandle` | All modern browsers |

Detection (`WebExecutorBooter.resolveMode`, asynchronous: it first fetches the manifest, once per page and location), best first:

```
if (SharedArrayBuffer)           → Atomics  (streaming, needs COOP/COEP)
else if ("Suspending" in WebAssembly) → JSPI (streaming, no headers)
else if (getDirectory() resolves)→ OPFS     (pre-copy; Asyncify measured ~2.2x slower)
else if (the manifest lists the  → Asyncify (streaming, no headers, larger wasm)
         Asyncify module)
else                             → PdfUnsupported naming both
```

### Web assets — the manifest and the parts

Static hosts cap each uploaded file (Cloudflare Pages and Workers: 25 MiB), not the size of
a module a browser compiles, so every wasm the package ships is cut into parts: consecutive
pieces of exactly `wasmPartBytes` (16 MiB, `build_constants.dart`), the last one shorter,
named `<module>.<n>.wasm` (the extension makes hosts pick `application/wasm` and compression).
The modules are `pdf_oxide_bg` and, with `asyncify: true`, `pdf_oxide_asyncify_bg`. Every module
is cut, even one that would fit in a file, so every mode runs the same join path.

`pdf_oxide_manifest.json` sits beside `executor_worker.js` and lists each module: its bytes,
the SHA-256 of the whole, and its parts (file, bytes, SHA-256). It is the only way the runtime
learns which modules exist: there is no probe, so a host that answers any path with an HTML
page cannot pass for one that shipped Asyncify. One function, `writeWasmParts`
(`lib/src/hook/wasm_parts.dart`), cuts and merges a module's entry into the manifest; the make
targets, the release and the hook's from-source build all call it. The whole modules exist only
as build intermediates in a temp folder (the Asyncify build joins the normal module's parts
back for its input).

One loader joins the parts: `compileParts` in `executor_worker.js` and its main-thread twin
`_compileParts` in `lib/src/runtime/web/wasm_loader.dart` start every part's fetch at once,
enqueue them in order into one `ReadableStream`, and hand `new Response(stream)` with
`content-type: application/wasm` to `WebAssembly.compileStreaming`. Each part is counted and its
SHA-256 checked against the manifest before the stream goes on; a short or corrupt part errors
the stream and the compile rejects with a message naming the part. The host compiles once per
page and posts the module to its workers; when that compile fails, each worker compiles from
the manifest entry the host sends in `init`. The web battery pins both loaders against the same
manifest.

The walls (`test/guards/wasm_size_test.dart`, and the release in `tool/compile.dart`): every
file in `web_assets/` and the release output is under 26,214,400 bytes; each module's parts
concatenated are byte for byte the module the manifest lists; and each module's gzip size,
which is what a visitor downloads, is within `normalWasmGzipBudget` /
`asyncifyWasmGzipBudget` (8.5 and 10.5 MiB) — raising one is a decision, not a fix.

Force a mode with `Pdf(config: PdfConfig(webIoMode: PdfIoMode.jspi))`. A forced mode the
browser cannot run, or Asyncify when the manifest has no entry for it, throws `PdfUnsupported`
when the instance initializes; there is never a silent fallback. All four modes give the Rust
side the same `Read+Seek` trait and `bridge_api.rs` is the same. JSPI, Atomics and OPFS run
the one wasm binary; Asyncify runs its own twin, and a browser fetches only the wasm its mode
runs. Only OPFS pre-copies to disk. Call `await pdf.ensureInitialized()` at startup to read
the mode. If it is `PdfIoMode.opfs`, consider telling the user, shipping the Asyncify wasm, or
serving the COOP/COEP headers.

**Whether the app shipped the Asyncify wasm** is one `HEAD` request for the file beside
`executor_worker.js`, sent once per location, and only by a page that could run the mode
(neither JSPI nor `SharedArrayBuffer`) or that forced it. The file itself is not fetched.

#### The Asyncify wasm

`wasm-opt --asyncify` turns the engine into a module that can stop in the middle of a call
and continue later. `lib/src/hook/asyncify_list.dart` computes which functions to
instrument; the rest are left as they are, and the pass is run by `bindgen_runner
--asyncify` (the workspace's own binaryen) followed by the same `-O2` as the normal wasm.

- **The list.** The functions that can be on the stack while the module waits in
  `host_read_at` or `host_write_chunk`: those imports' callers, transitively, where a
  `call_indirect` reaches every address-taken function of its signature (the trait objects
  `dyn Read + Seek` and `dyn Write` are such calls). The parser refuses a module whose
  tables can change at run time. The list names each function by its index among the defined
  functions, which is how binaryen names an unnamed function.
- **The wall.** The build recomputes the list from the wasm it is about to instrument, writes it
  and checks the written file against the wasm before binaryen reads it
  (`verifyAsyncifyList`): a list that is stale or misses a function the wasm can have on the
  stack stops the build. `test/hook/asyncify_list_test.dart` pins it.
- **The driver** (`runAsyncify`, `executor_worker.js`). The two imports are plain functions.
  Called while the module runs, an import starts the unwind and returns; the wasm unwinds to
  `execute`, which returns with the stack saved. The worker posts the same `readAt` or `chunk`
  message JSPI posts, awaits the `readAtResult` or `chunkAck`, starts the rewind and calls
  `execute` again with the same arguments; the wasm replays down to the import, which
  returns the answer. The saved stack lives in pages the worker adds to linear memory
  (`kAsyncifyStackBytes`), outside the engine's heap; the pass writes it without a bound, so
  the worker checks the fill after every unwind and fails the job when it overflowed.
- **Size.** The list is most of the module, so the twin is about half again as large as the
  normal wasm (31.97 MB against 20.87 MB raw, 12.4 MB against 8.1 MB gzip, for the full
  engine), which is more than the 25 MiB per file that Cloudflare Pages and Workers static
  assets serve. `test/guards/wasm_size_test.dart` and the release compile refuse a shipped
  wasm that reaches that size. The obstacle is in the call graph: a signature such as
  `(i32,i32)->()` is shared by thousands of functions, and the graph cannot tell which of
  them an indirect call reaches.
- **Cost.** A read or write costs a round trip through the main thread like JSPI's, plus the
  unwind and rewind of the stack in between, and the instrumented functions run slower. On
  a 300 MB text PDF on a 4-core machine, open + extract of 50 pages took 93 s in Asyncify against
  41 s in OPFS (pre-copy included), and a merge of two such files 16–19 s against 9–10 s: the
  source is read in many small pieces, and each costs a round trip. The automatic order puts
  Asyncify before OPFS all the same, until that is decided.

**Web hold registry.** Each web executor keeps a reader map. A kept reader belongs to a group: the
id of the job that created the handle. A later job on that handle that keeps a source (an
append) files its reader in the same group, after the earlier ones. A handle's held readers
(the editor's source at index 0, then each appended source in order) mount first in each
job's reader list, and the job's own sources follow. Readers that are not held are dropped
when the job ends. Held ones survive until the handle's `releaseHold` drops the group. Dart
serves held reads in the same order (`WebLease._serveReadAt`). Native does the same with its
held channels.

### Backpressure

**Sources (input)** are backpressured on every platform. The engine calls `readAt` and
blocks until the Dart side returns bytes, so it never reads faster than the host can serve.

**Sinks (output)** are backpressured per mode:

- **Native:** the condvar buffer blocks the executor thread when it is full. Dart drains it
  before the writer can continue.
- **JSPI:** `host_write_chunk` returns a Promise and JSPI suspends the wasm stack. The executor
  forwards the chunk to Dart, Dart's sink consumes it, Dart acknowledges, the executor resolves
  the Promise and wasm resumes. One chunk is in flight at a time.
- **Asyncify:** the same as JSPI: `host_write_chunk` unwinds the wasm, the worker posts the
  chunk and awaits the acknowledgement, and the rewound import returns. One chunk is in
  flight at a time.
- **Atomics:** `host_write_chunk` posts the chunk, then `Atomics.wait`s on a dedicated write
  SAB. Dart consumes the chunk and acknowledges with `Atomics.store` and `notify`. One chunk
  is in flight at a time.
- **OPFS:** `postMessage`, with no wait. No blocking primitive is available (no SAB, no
  JSPI). The wasm producer is slower than the main-thread consumer, because OPFS source
  reads are the bottleneck and not the output writes.

### Sources are random-access, not forward-only

`DataSource.readAt(offset, count)` is random-access. The engine reads the xref table at the
end of the file, then jumps to arbitrary offsets to load objects. A forward-only pipe (a
one-shot socket, stdin, HTTP without Range headers) cannot be a `DataSource`. Buffer the
content first, or use a random-access backing store. Only the backing store ever holds the
whole file; the engine pulls at most one read buffer per `readAt`.

### OPFS mode: O(N) disk and latency

OPFS is the universal fallback: what users get without COOP/COEP headers on browsers older
than Chrome 137. It pre-copies each source file to OPFS disk before the job runs. That costs
disk space and time before the first output byte in proportion to the size of the source,
and it is subject to the browser's storage quota.

Pre-copies live under `WebExecutorBooter.opfsRootDir`, in one random directory per page load
and one directory per executor inside it. One rule governs cleanup: only a live executor touches
its own files, and anyone else sweeps only the provably dead.

While an executor lives, it deletes its own files. It created them, holds their handles and is
their only consumer, so each deletion is by the same agent and cannot race. One message
ends a job's files (`opfsDrop`), and one releases a hold (`releaseHold`). No retry exists on
the live path, because no race exists there. The one call that can leave a live executor
waiting is described under **Web executors**.

Death is decided by liveness, not by guesswork. Every owner holds a
[Web Lock](https://developer.mozilla.org/docs/Web/API/Web_Locks_API) for its lifetime: the
page holds its session's lock, and each executor holds the lock of its own directory, taken
before its first file can exist. The browser releases a lock when its agent dies, and the
specification says so. This is the web's equivalent of the native executor's
every-job-posts-exactly-once contract, and it is why `FinalizationRegistry`, which is best
effort by specification, is not used. An acquirable lock is the proof of death. Terminating an
executor awaits its lock and then reclaims its directory (`_deleteDeadWorkerDir`, which retries
while the browser reaps the dead executor's handles; a directory it gives up on is listed in
`abandonedWorkerDirs` and left to the next session's sweep). A new session reclaims dead
sessions' directories at boot, under the same lock rule in `legacyOpfsRootDir` too, the root
that 5.x apps used; that root goes once no live session is left in it. A crashed tab cannot
leak disk for good. The only retry loop in the runtime runs against the provably dead, where
the obstacle ends on its own.

---

## The binary wire format

The layout is the same in both directions. The codec carries op arguments only. PDF bytes
travel through the transport's I/O channels, never through the codec.

```
Request:  [op_len: u8] [op: UTF-8] [field_count: u16 LE] [fields...]
Response: [status: u8]  [field_count: u16 LE] [fields...]
            status 0 → [kind: u8] [field_count: u16 LE] [fields...]
                       (error; field `message` always present,
                        pageRange kind also carries `page`, `pageCount`)
            status 1 → fields                           (ok)
            status 2 → (no payload)                     (cancelled)

Field:    [key_len: u8] [key: UTF-8] [type: u8] [value]

Types:    0=null  1=i32  2=i64  3=f64  4=bool  5=string  6=bytes
          7=int_list  8=float_list  9=string_list  10=map_list
```

Error kinds (byte 1 of a status-0 response) and the Dart type each throws:

| Kind | Name | Dart type |
|---|---|---|
| 0 | engine | `PdfEngineFailed` |
| 1 | invalidArgument | `PdfEngineFailed` (the engine refused an argument the caller could not have checked) |
| 2 | io | `PdfIoFailed` |
| 3 | corrupted | `PdfCorrupted` |
| 4 | unsupported | `PdfUnsupported` |
| 5 | passwordRequired | `PdfPasswordRequired` |
| 6 | wrongPassword | `PdfWrongPassword` |
| 7 | crypto | `PdfCryptoFailed` |
| 8 | pageRange | `PdfPageOutOfRange` (fields `page`, `pageCount`) |
| 9 | incrementalRefused | `PdfIncrementalRefused` |

Status 2 throws `PdfCancelled`. `PdfNotFound` never crosses the wire: the ops layer throws it
when a name the caller gave (an attachment, an image) is not in the document. A programmer's
mistake is an `ArgumentError` or a `StateError`, thrown before any I/O and outside `PdfError`.

Op names are the `EngineOp.wire` strings (`"open"`, `"extract"`, `"editorMutate"` and so on).
No numeric mapping needs to stay in sync, because the enum is the protocol.

### The envelope around a request

The codec bytes travel in an envelope that the executor reads first. The envelope carries the
**lease** and brings back the **heap peak**:

- Native: the lease is an argument of `executor_submit`, `executor_job_cancel`,
  `executor_reset` and `executor_hold_release`. The peak is read with `executor_heap_peak`
  after the post.
- Web: every message after the boot handshake has a `lease` field. `result`, `error` and
  `resetDone` carry `peak` and `mem`. `resetDone` also carries `abandoned`, and
  `opfsWriteAck` carries `retried` when the handle was created twice. An `error` with
  `protocol: true` means the executor refused the message and it never started; `trap` marks a
  wasm trap.

The message tags and field names are in `lib/src/runtime/web/protocol.dart`.
`executor_worker_sync_test.dart` pins them against `executor_worker.js`.

---

## Source tree

```
lib/
├── pdf_manipulator.dart                    ← barrel export (no dart:io)
├── io.dart                                 ← dart:io barrel: FileSource, FileSink, PdfFile
└── src/
    ├── ops/                                ← LAYER 1: CONSUMER API
    │   ├── pdf.dart                        ← Pdf: lifecycle, the handle, scope and one-call doors
    │   ├── pdf_doc.dart                    ← PdfDocument and its handle
    │   ├── pdf_reads.dart                  ← every read, declared once (PdfDocument and readOnce)
    │   ├── pdf_editor.dart                 ← PdfEditor and its handle
    │   ├── pdf_edits.dart                  ← every edit, declared once (PdfEditor and editOnce)
    │   ├── pdf_builder.dart                ← PdfBuilder: create from scratch
    │   ├── pdf_jobs.dart                   ← the doors that open several handles
    │   └── handle_backstop.dart            ← the backstop for a handle nobody disposed
    │
    ├── bridge/                             ← LAYER 2: BRIDGE (platform-blind)
    │   ├── pdf_bridge.dart                 ← abstract PdfBridge + handle contracts
    │   ├── pdf_transport.dart              ← PdfTransport interface
    │   ├── shared_bridge.dart              ← ONE bridge, both platforms; makes every PdfTask
    │   ├── shared_bridge_doc.dart          ← document handle (part)
    │   ├── shared_bridge_editor.dart       ← editor handle (part)
    │   ├── shared_bridge_builder.dart      ← builder and page handles (part)
    │   ├── create_bridge.dart              ← SharedBridge(Router(pool: ExecutorPool.of(booter)))
    │   └── protocol/
    │       ├── binary_codec.dart           ← binary request/response encoding, WireErrorKind
    │       ├── codec.dart                  ← typed request builders and response decoders
    │       ├── pdf_date.dart               ← PDF date string ⇄ DateTime (UTC)
    │       └── op.dart                     ← EngineOp enum (the wire names)
    │
    ├── runtime/                            ← LAYER 3: RUNTIME
    │   ├── router.dart                     ← Router: pinning, placement, resources, preludes, dispose
    │   ├── pool.dart                       ← ExecutorPool (states, budget, order, keep line) and Lane
    │   ├── lane.dart                       ← the contracts: Lease (the adapter), Executor,
    │   │                                      ExecutorBooter, Job, JobResult
    │   ├── host.dart                       ← booter selector: stub default,
    │   │                                      dart.library.io → native, js_interop → web
    │   ├── host_stub.dart                  ← neutral default target (no platform library,
    │   │                                      so pub.dev keeps web supported)
    │   ├── wire_peek.dart                  ← minimal wire reads (op, handle id)
    │   ├── native/
    │   │   ├── lease.dart                  ← NativeExecutor, NativeLease: the verbs over FFI
    │   │   ├── host.dart                   ← NativeExecutorBooter (part of lease.dart)
    │   │   ├── channel_buffers.dart        ← condvar memory layout
    │   │   └── bindings.dart               ← @Native FFI bindings
    │   └── web/
    │       ├── lease.dart                  ← WebLease: job lifecycle, the 4 I/O modes
    │       ├── host.dart                   ← WebExecutorBooter, WebExecutor: boot handshake,
    │       │                                  OPFS session and directory reclaim (part of lease.dart)
    │       └── protocol.dart               ← message tags and codes injected into executor_worker.js
    │
    ├── io/                                 ← MemorySource/MemorySink; FileSource/FileSink/PdfFile
    ├── types/                              ← public and shared types (CancelHook, DataSource,
    │                                          DataSink, PdfTask, PdfError, PdfConfig, ...)
    ├── keep/                               ← trim: capabilities and the detectors (tooling only)
    └── hook/                               ← build and setup support (not consumer API)
        ├── asset_hashes.dart               ← SHA-256 hashes for all release assets
        ├── resolver.dart                   ← ResolveRequest + 5-step waterfall
        ├── wasm_parts.dart                 ← the cut: writeWasmParts, the manifest, the walls
        ├── asyncify_list.dart              ← the wasm call graph, the Asyncify instrumentation
        │                                      list and its wall
        └── (build_constants, engine_compiler, engine_build, keep_plan, pdf_config,
             user_defines, app_root)

web_assets/                                 ← committed in git (except .wasm, which is too large)
├── executor_worker.js                      ← the executor body: reader registry, 4 I/O modes,
│                                              reset, execute (no routing logic)
├── pdf_oxide.js                            ← wasm-bindgen glue (committed, paired with .wasm)
├── pdf_oxide_bg.<n>.wasm                   ← the wasm binary in 16 MiB parts (gitignored; built or
│                                              downloaded from releases)
├── pdf_oxide_asyncify_bg.<n>.wasm          ← its Asyncify twin's parts, for apps with `asyncify: true`
│                                              (gitignored)
└── pdf_oxide_manifest.json                 ← the modules, parts and hashes (gitignored)

build.json                                  ← single source of truth: crate name, repo,
                                               cargo features, web asset map

hook/
├── build.dart                              ← Flutter hook entry (native) + resolveWeb() (web)
└── link.dart                               ← forwards assets; drives the RecordUse trim

bin/
└── setup.dart                              ← CLI: setup <target> (web|android|ios|macos|linux|windows)

tool/                                       ← .sh = orchestration wrappers; .dart = programs
│                                              (anything that parses structured data or uses a library)
├── build_lib.sh                            ← shared build helpers (sourced by 2+ scripts)
├── versions.env                            ← single source of truth for pinned versions
├── analyze.sh                              ← format + Dart + Rust static analysis
├── check_cancel_polls.sh                   ← the loop poll lint (in make test-guards)
├── compile.dart                            ← Rust → native / wasm / per-target / both
├── asyncify_list.dart                      ← the Asyncify list of a built wasm, with its counts
├── generate_fixtures.dart                  ← test fixture generator (make fixtures; stamp-protected)
├── memprobe/                               ← the memory wall (make test-memory)
├── fuelprobe/                              ← the cancel fuel wall (make test-fuel)
└── ci/                                     ← invoked directly by workflows, not by make

vendor/
├── pdf_oxide/                              ← forked yfedoseev/pdf_oxide (submodule)
├── office_oxide/                           ← forked yfedoseev/office_oxide (submodule)
└── tiny_skia/                              ← forked linebender/tiny-skia (submodule; the page rasteriser)

vendor/pdf_oxide/src/
├── host/                                   ← LAYER 4: OUR CODE
│   ├── mod.rs
│   ├── bridge_api.rs                       ← the door: parse the request, dispatch through the registry
│   ├── ops/                                ← one op unit per bridge op, each with its handler body
│   │   ├── mod.rs                          ← OpCtx, OpEntry, the op_unit! macro
│   │   ├── registry.rs                     ← the table of units; the one swappable backend
│   │   ├── support.rs                      ← helpers shared by several units
│   │   └── builder.rs, convert.rs, doc.rs, editor.rs, fonts.rs, pdfa.rs, render.rs, signatures.rs
│   ├── binary_codec.rs                     ← wire format parser and encoder
│   ├── dispatch.rs                         ← every op as a typed function
│   ├── engine_state.rs                     ← EngineState: the state of one executor (no locks)
│   ├── caches.rs                           ← EngineCaches: fonts, CMaps and fallback fonts per executor
│   ├── heap.rs                             ← the counting allocator, heap, KEEP_BYTES
│   ├── font_budget.rs                      ← byte budgets of the font caches
│   ├── constants.rs                        ← buffer sizes, host I/O codes
│   ├── incremental.rs                      ← append-only save: source bytes, then one update section
│   ├── page_prune.rs                       ← what a full rewrite drops with dropped pages
│   ├── positioned_write.rs                 ← CountingWriter + SeekWriter
│   ├── sign.rs                             ← signing by one appended update section
│   ├── chunked.rs, render_stream.rs, extract_stream.rs  ← the chunked wire of render and extractImages
│   ├── images/                             ← image reducer behind reduceImages and compress
│   │   └── inventory, classify, policy, execute, encode, resample, report, stream, dct, redact_copy
│   ├── content/                            ← content-stream reading as operators
│   ├── font_subset/                        ← font subsets (TrueType, CFF)
│   ├── font_optimizer.rs                   ← Standard 14 unembedding
│   ├── probe.rs                            ← the out-of-crate door that tool/memprobe runs requests through
│   ├── engine_proofs/                      ← tests that open documents; shared_state.txt, whole_reads.txt
│   ├── native/                             ← executors and condvar I/O
│   │   ├── executor.rs                     ← the executor body: thread loop, jobs, leases, reset
│   │   ├── executor_table.rs               ← registry, thread budget, FFI surface
│   │   ├── cancel.rs                       ← CancelFlags (executor flag + job flag)
│   │   ├── callback_reader.rs, callback_writer.rs
│   │   └── shared_buffer.rs
│   └── wasm/                               ← JS callback reader/writer
│       ├── js_reader.rs                    ← Read+Seek via host_read_at
│       └── js_writer.rs                    ← Write via host_write_chunk
│
├── cancel.rs                               ← CancelCheck, Stride, cancel::enter and poll (fork-only)
├── compliance/fonts/                       ← 12 Liberation TTF files (SIL OFL)
├── ffi.rs                                  ← UPSTREAM (never called; patched only where the engine's types force it)
├── wasm.rs                                 ← UPSTREAM (never called; patched only where the engine's types force it)
└── (engine: document.rs, editor/, renderer/, search/, signatures/, ...)
```

---

## CI/CD architecture

### Vocabulary

| Term | Meaning | Example |
|---|---|---|
| **target** | What you build for | android, ios, macos, linux, windows, web |
| **runner** | CI machine that runs the job (each workflow sets its own) | e.g. `ubuntu-24.04` |
| **host** | Machine doing the building (CI or dev) | `Platform.isMacOS`, `uname` |
| **capability** | One installable concern | fvm, rust, java, chrome, headless-display |
| **build** | Compile for dev iteration | `make build-native` |
| **compile** | Produce release binaries for upload | `make compile-macos` |
| **verify** | Prove release build works (output thrown away) | `make verify-android` |
| **test** | Run test suites | `make test-unit` |
| **release** | Publish a version (tag, upload, pub.dev) | `release.sh --discover` |

"Platform" is not used internally. "Cross-platform" is kept in
user-facing text (README, pubspec).

*host* and *runner* are CI words. In the runtime, *host layer* means
`vendor/pdf_oxide/src/host/` (see **Glossary**).

### Principles

1. **Makefile is the interface.** CI runs `make <target>`. All build
   logic lives in Makefile and scripts. CI YAML has zero build logic.

2. **One compile driver, shared by consumer and CI.** `tool/compile.dart`
   loops the release target matrix; the consumer build hook
   (`hook/build.dart`) compiles the single target Flutter asks for. Both
   call the SAME core — `compileEngineForTarget` / `compileWasmEngine` in
   `lib/src/hook/engine_compiler.dart` — so the two can't drift. Deps are
   auto-handled there: Rust targets install via `ensureRustTarget`
   (rustc-probe first, rustup fallback); the aarch64 Linux cross-linker
   and the Android NDK auto-install / resolve on CI, error with
   instructions on dev. wasm-bindgen + wasm-opt need no install at all —
   they run as library calls inside the `bindgen_runner` crate,
   version-locked by the engine's own Cargo.lock. The required Rust
   version is `RUST_VERSION` in `tool/versions.env` — the one pin CI
   installs AND the build hook's `ensureCargoVersion` gate enforces, so a
   too-old cargo fails with a clear message instead of cargo's cryptic
   edition error (issue #183).

3. **jq is the JSON tool for the remaining bash scripts.** Analysis /
   gate scripts that need a `build.json` value read it through
   `build_lib.sh`'s jq-backed accessor (`json_get`), never hand-rolled
   `sed`/`grep`; `ensure_jq` guarantees jq is present. Compilation is the
   exception that moved to Dart: `tool/compile.dart` (and the build hook)
   read `build.json` via `BuildConstants`, and the Makefile invokes the
   Dart driver for every compile target. So the one former parallel-bash
   cargo wrapper is now the same code as the consumer path.

4. **The capability model and the single `make-target` orchestrator are
   the shared model.** See whuppi/ci/docs/ARCHITECTURE.md "make-target —
   the orchestration contract".

5. **Runners are configurable.** Every workflow resolves its runner in
   one identical `inputs` job — `runs-on: ${{ needs.inputs.outputs.runner }}`
   on every real job, the only place a literal `runs-on` appears. A
   workflow that can be hand-run (a real `workflow_dispatch`) exposes a
   `runner` choice that overrides `DEFAULT_RUNNER`; an event-only bot
   resolves the hardcoded default through the same job. Test/compile jobs
   add per-row matrix runners.

6. **Matrix row is the manifest.** Each row declares which
   capabilities to activate and which runner to use. Adding a new
   combo = one line. Adding a new capability = one action + one
   input on `make-target` + add to rows that need it.

7. **Every job is named.** A job's YAML key (its ID) is the code
   handle — terse, lowercase, used only by `needs:` and
   `${{ needs.<id>.* }}`, and hyphen-free where referenced in a `needs`
   expression (a hyphen reads as minus there, so `rust`, not
   `rust-tests`). The `name:` is the human label the GitHub UI shows
   and is REQUIRED on every job — never let one fall back to its raw
   ID. Matrix jobs set `name: <prefix>: ${{ matrix.name }}`.

### Actions

pdf carries three local native capabilities under
`.github/actions/capabilities/`:

- **`rust/`** — pinned Rust toolchain, one shared cargo target dir, and
  the cargo cache (rust-cache). Sets the `pdf_oxide` submodule rev the
  wasm and xcode caches key on.
- **`wasm-cache/`** — caches WASM build output.
- **`wasm-build/`** — compiles WASM on a cache miss.

pdf's local `make-target` wrapper provisions these before delegating the
generic run to whuppi/ci. The capability model and the "what is a
capability" test live in whuppi/ci/docs/ARCHITECTURE.md.

### Caching — what is cached, why, and the cold guard

The engine's final artifact is a cdylib built with fat LTO. No compiler
wrapper cache (sccache, ccache) can cache a crate that invokes the system
linker, so a wrapper at a 100 % hit rate still rebuilt the expensive step
every job. The only thing that reuses it is cargo's fingerprint inside
`target/` — so `target/` is what CI caches.

| Piece | How |
|---|---|
| One target dir | Every build path in a job — the consumer hook (`dart test`, `flutter build` inside Gradle/Xcode), `tool/compile.dart`, `cargo test` — lands in `vendor/pdf_oxide/target`. The hook learns this from `~/.pdf_manipulator/cargo-target-dir`, a file, because `hooks_runner` strips env vars to an allowlist (`HOME` survives). Consumers never have the file; their hook keeps building inside `.dart_tool`. |
| The cache | `Swatinem/rust-cache` archives `~/.cargo` + both vendored `target/` dirs per key. Key = rustc host + version, every `Cargo.lock`/`Cargo.toml` hash, the `CARGO`/`RUST` env, the **build family** (`native` / `wasm` / `android` / `ios` / `check` / `test` — each matrix row declares its `cache:`; rows that build the same triples and features share one archive, rows that build something else never do) and both vendored submodule revs. One warm set is ~12 archives, ~6 GB; keyed per row it was 33 archives, ~14 GB, mostly identical bytes. `cache-workspace-crates: true` is required — the default drops the workspace's own crates, and the vendored engine IS the workspace crate. |
| Path-crate freshness | cargo fingerprints a path crate by source mtimes, and a checkout stamps every file "now", so a restored cache would still rebuild the engine (the first warm run proved it: deps hit, `pdf_oxide` recompiled). After restore, CI pins every file under `vendor/` to a fixed past instant. Safe only because the key names the exact vendor revs: same key ⇒ identical sources ⇒ the artifact is the right one; any change ⇒ new key ⇒ cold. The revs in the key and the mtime pin are one mechanism — never keep one without the other. |
| Unhashed engine outputs | Both engines are `cdylib + rlib + staticlib`, and cargo emits every output of such a lib unit without a hash (`libpdf_oxide.so`, `.rlib`, `.a`). rust-cache's save-time cleanup keeps a `deps/` entry only when its name is `<crate>-<hash>`, so it deletes exactly those files and the fat-LTO engine rebuilt on every "warm" run (probe: `FailedToReadMetadata(deps/libpdf_oxide.so)` with everything else cached). The rust action therefore keeps a second, small archive of just those outputs, restored over rust-cache's and saved from `actions/cache`'s own post step so it runs before rust-cache's cleanup. It also carries the crates' `.fingerprint/` and `build/` entries — cargo keeps one fingerprint per unit hash, so a row whose units differ from the deps archive's saver would otherwise restore the artifacts without their fingerprints and rebuild anyway. Same key ingredients, same save gate. The compile rows key this archive by their own family (`engine:` in the matrix) while sharing the hook rows' deps archive — rust-cache writes an archive once per key from whichever row finishes first, so their 4-ABI outputs need a key nobody races for, and a second deps archive per compile family would cost ~3.5 GB of the 10 GB quota for bytes already cached. |
| Staleness bound | rust-cache prunes artifacts older than seven days at save time, so a warm cache can never drift more than a week from a from-scratch build. |
| The cold guard | `release.yml` compiles with the cache off, always: a release binary's provenance is the tagged source, not "whatever was cached". `full-test.yml` runs the whole matrix cold every Monday, and both workflows take `cold: true` on manual dispatch. This is the routine proof that a fresh clone builds every target from vendor source — the promise the pub.dev tarball makes. |
| The test family, the compile rows and the slow hook rows on ghcr | The Rust tests job, every `compile:` row, `verify: Android`, `int: Android`, `verify: macOS` and `pkg: Web` (whose `dart test -p chrome` builds the host engine through the hook first) keep their whole vendored `target` trees (every integration-test and bin executable, which rust-cache always dropped) plus the cargo registry on the GitHub Container Registry, keyed on the vendor revs with a `-latest` fallback so an engine-bump PR rebuilds only what changed; rust-cache is skipped for those families. Each compile row is its own family (`compile-android`, `compile-ios`, …): on the Actions cache the compile rows shared the hook rows' key, rust-cache saves once per key from the first finisher, and the near-empty hook rows always won the race, so the compile rows' dependency trees were never saved and every warm run rebuilt them (Android: 22 min for four ABIs). Gigabytes per revision — fine on GHCR (public, uncapped, pruned at 14 days), impossible on the Actions cache. |
| Rust test speed | `make test-rust` runs the engines' suites through cargo-nextest — every test its own process across all cores; `cargo test` ran pdf_oxide's ~330 test binaries one after another (9,078 tests: 5 min → 1.8 min) — plus `cargo test --doc` for the doctests nextest skips. Same tests, same pass/fail; nextest is required, never a fallback, so "the suite passed" means one thing on every machine. It is a pinned, sha256-verified download (`NEXTEST_VERSION` in `tool/versions.env`, radar-tracked by `upgrade.sh`). The build half (~2.5 min for ~330 test crates) is rustc-bound on the runner's cores: mold was measured at no gain and dropped; caching the binaries would cost gigabytes per family. |
| Disk headroom | `make-target`'s `free-disk` is a guard, not a fixed step: it reads live `df` and runs the ~2 min reclaim only when the image has less than `min-free-gb` free (the ubuntu-24.04 image went from ~14 GB to ~80 GB free; the reclaim was pure overhead on every Rust tests run). A post-build `df` line is the evidence for the threshold. |
| Where the big archives live | The Flutter SDK (~0.8 GB per OS), Gradle homes (~1.3 GB per OS) and AVD snapshots (~1 GB) are stored on the GitHub Container Registry through whuppi/ci's `oci-cache` (`cache-backend: ghcr` on every make-target call): GitHub Packages is free and uncapped for this public repo and Actions pulls are free, while the Actions cache is 10 GB per repo and evicts by last read — a fully warm set is ~19 GB, so on the Actions cache alone every matrix run found rows cold again. The Rust deps + engine archives (~8 GB) stay on the Actions cache, which is what rust-cache's key and cleanup machinery need. `oci-cache-prune.yml` is the ghcr retention (14 days; fallback tags kept). |
| Scope | GitHub lets a run read caches from its own ref, the PR base, and the default branch only — caches written from a PR ref are invisible to every other PR and only spend the 10 GB quota. So `dev` is the one warm source: `ci.yml` and `full-test.yml` run on push to `dev` (paths = the cache-key inputs) with every save on; PR runs pass `rust-cache-save`/`cache-save: false` and only read — except a PR whose Rust inputs changed, which writes its own cargo cache so its later pushes aren't cold. `cache-cleanup.yml` deletes a PR's caches when it closes. |

The bar this protects: a green row must mean the row did its own work.
A cache hit skips a rebuild of identical inputs; it never substitutes
another row's artifact or a lighter profile.

### Workflows

`ci.yml`, `full-test.yml`, `release.yml` are local workflows whose jobs
call pdf's local `make-target` wrapper (which delegates to whuppi/ci); the
rest are thin callers to whuppi/ci reusable workflows.

| Workflow | Trigger | Kind |
|---|---|---|
| `ci.yml` | PR to prod/dev | Local — jobs call `make-target` (→ whuppi/ci) |
| `full-test.yml` | `ready-to-test` label or dispatch | Local — matrix jobs call `make-target` |
| `release.yml` | Changelog push or dispatch | Local — compile matrix + `release-tool` |
| `debug-ssh.yml` | Dispatch only | Local — uses shared `debug-ssh` action |
| `pr-checks.yml` | PR to prod/dev | Thin caller → whuppi/ci reusable |
| `triage.yml` | Issues / fork PRs | Thin caller → whuppi/ci reusable (privileged) |
| `retry.yml` | CI / Full Test completed | Thin caller → whuppi/ci reusable (privileged) |
| `auto-close.yml` | Schedule / issues / comments | Thin caller → whuppi/ci reusable |
| `labels.yml` | Label config push / dispatch | Thin caller → whuppi/ci reusable |
| `upgrade-check.yml` | Daily / dispatch | Thin caller → whuppi/ci reusable |

### Workflow security

`triage` and `retry` are privileged (fork-triggerable write). The hardening
(dangerous-triggers, the ten conditions, the single zizmor receipt, the owner
guard `github.repository_owner == 'whuppi'`) lives once in the shared reusables.
See whuppi/ci/docs/ARCHITECTURE.md "The repo guard".

### Test matrix (full-test.yml)

Single matrix with two tiers in one sorted list:

- **Core rows** — one runner per target, proves the code works.
- **Portability rows `[P]`** — extra runners, proves any dev
  machine can build with this package. Controlled by
  `DEFAULT_PORTABILITY` env var. Skipped when toggle is off.

| Category | Core | Portability [P] |
|---|---|---|
| Package | macOS, Linux, Windows, Web | Web on macos + win |
| Integration | macOS, Linux, Windows, Android, iOS, Web | Android on macos-intel + win, Web on macos + win |
| Verify | Android, iOS, macOS, Linux, Web | Android on macos + win, Web on macos + win |

Every row family states, in a comment beside it in `full-test.yml`, the
claim it proves and what a build from a different host would silently stop
proving. That comment is the contract: CI gets faster by caching what a row
builds, never by removing the row or handing it another row's output.

### Build inputs — `build.json` vs `versions.env`

Two files hold the project's pinned inputs, split by *what the value is*, not by
who reads it:

- **`build.json`** — facts about the **subject** being built: the vendored
  `pdf_oxide` crate (`crate`, `repo`, its `baseTag`), the cargo `features` per
  target, the wasm outputs. Hand-set; changes only when the vendored submodule
  moves. Read by `hook/build.dart` + `tool/compile.dart` (`jsonDecode` /
  `BuildConstants`) and by `release.sh` / `analyze.sh` via the `json_get`
  helper in `tool/build_lib.sh`.
- **`tool/versions.env`** — the single source of truth for every version this
  package pins: `RUST_VERSION` (the engine's Rust toolchain — CI installs it,
  the build hook's gate enforces it) and `PANA_VERSION` (the platform gate).
  Shell `KEY="value"`, sourced by the scripts + parsed by the Dart hook. Most
  entries are auto-bumped by `tool/ci/upgrade.sh` (`PANA_VERSION`); `RUST_VERSION`
  is hand-bumped only — it is the engine's MSRV and rises when the vendored
  crates actually need a newer toolchain, never automatically. The shared tool
  pins (`fvm`, Chrome, the gate binaries) live in whuppi/ci's own `versions.env`,
  not here.

The rule: a fact about *what* is built (the crate, its base version, its
features) belongs in `build.json`; a pinned version of an *external tool* that
does the building belongs in `versions.env`. So `baseTag` — the upstream
`pdf_oxide` version — sits beside `crate`/`repo` in `build.json`, not among the
tool pins.

### Dependency ownership

| Dep | Owner | CI behavior | Dev behavior |
|---|---|---|---|
| Rust targets | `tool/compile.dart`, `hook/build.dart` | Auto-install (safe) | Auto-install (safe) |
| wasm-bindgen + wasm-opt | `bindgen_runner` (vendor workspace crate) | Built by cargo, Cargo.lock-pinned | Same |
| gcc-aarch64 cross | `tool/compile.dart` | Auto-install | Error with instructions |
| GTK + ninja | Makefile | Auto-install | Error with instructions |
| build.json reads | `tool/compile.dart` (Dart), `release.sh` (jq) | build.json | Same |


---

## Test architecture

### The test invariants

1. **Foreign diet.** Read/edit/sugar ops are tested against PDFs from
   INDEPENDENT producers (dart-pdf via the fixture generator, the
   handwritten micro fixtures, the committed qpdf-encrypted fixture) —
   never against this package's own builder output. Self-feeding lets
   mirrored reader/writer bugs cancel into green tests.
2. **Creation is a subject, not infrastructure.** The builder battery
   builds inline; the builder never manufactures fixtures.
3. **Declared truths.** Every generated fixture carries typed
   ground-truth constants; tests assert against declared intent,
   never re-derived engine output.
4. **Existence is never proof.** `make fixtures` stamps generated/
   with a hash of its inputs; mismatch regenerates everything.
5. **No `dart:io` in tests** (mechanically guarded by `make
   test-guards`). Fixtures are imported Dart source — VM and browser
   consume identical bytes. Exceptions: the hybrid asset server, the
   native process-death tests, the native `FileSource`/`FileSink` tests
   (those classes wrap `dart:io`, so their tests must too — VM-only),
   and the VM-only source-parity guards (wire sync, worker protocol sync,
   adapter-decides-nothing, keep line) — they read repo SOURCE to assert
   sync, not fixtures.
6. **Semantic assertions.** Content claims go through extract/search/
   render — never byte-grepping (guarded; survivors: `%PDF-`/ZIP
   magic, encrypted-leak negatives, and the builder battery, whose
   byte emission IS its subject).
7. **One charter per battery.** Each battery's header names exactly
   what it alone proves; no two tests prove the same claim.
8. **Per-test timeouts + O(1) I/O guards always on.** `TestSource`
   throws on `readAt > 64KB`; `TestSink` on `write > 256KB`. Exceeds
   a budget → fix the algorithm, never the timeout.
9. **Tests must be able to fail.** Every assertion answers: "if the
   feature silently did nothing, does this go red?" If not, it is
   not a test.

Every stress test makes two separate checks. The test timeout
(`t(60)`) is a hang guard only. Speed is a ratio budget:
`stressTest` in `harness/perf.dart` times a fixed calibration (open and
extract of a tiny PDF 20 times through the same engine, after one
untimed warm-up, the fastest of three samples) before and after the
operation, divides the operation's time by the slower of the two, and
fails above the test's `ratio`. Machine load slows both sides, so the
check does not fail on a busy machine and still fails on a real
slowdown. Each `ratio` is 3x the largest ratio observed over four runs
(one in reverse test order), recorded in the comment above the test.

### Structure

```
test/
├── fixtures/                       DATA — three independent producers
│   ├── catalog.dart                spec + truths of every generated fixture
│   ├── generated/                  GITIGNORED — emitted by `make fixtures`
│   │   ├── .stamp                  input hash (existence is never proof)
│   │   ├── f_*.dart                base64 bytes + truth constants
│   │   └── f_photo_png.dart        the photo-like raster (seeded noise),
│   │                               built by the generator, fed into the
│   │                               image PDF fixtures
│   ├── third_party/
│   │   └── tp_encrypted.dart       committed qpdf AES-256 fixture
│   └── handwritten.dart            hand-authored micro fixtures + certs
│
├── harness/                        MACHINERY — not data
│   ├── asset_server.dart           HTTP server for web test assets
│   ├── perf.dart                   stressTest() — hang guard + ratio budget
│   ├── slow_source.dart            Parks the engine on a read (cancel tests)
│   ├── fake_executors.dart         Fake executors, leases and booter (pool and router tests)
│   ├── test_source_sink.dart       TestSource + TestSink (O(1) guards)
│   ├── timeouts.dart               t() — per-test timeout helper
│   └── streaming_guard_test.dart   Tests the guards themselves
│
├── ops/
│   ├── core/                       shared batteries (*_battery.dart —
│   │                               register-only, no main; runners call them)
│   ├── stress/                     stress batteries (1000-page)
│   ├── platform/                   guarantees only ONE platform can break
│   │   ├── native/                 finalizer, lease protocol and stale-answer
│   │   │                           batteries, dispose-exit payload
│   │   └── web/                    lease protocol, cancel channel, config
│   │                               inheritance, clean boot failure, dead-session
│   │                               reclamation, zero OPFS residue
│   └── runners/                    4 target runners (the ONLY entry points)
│       ├── native_runner_test.dart
│       ├── web_opfs_runner_test.dart
│       ├── web_jspi_runner_test.dart
│       ├── web_atomics_runner_test.dart
│       ├── web_asyncify_runner_test.dart
│       └── web_io_measure_test.dart        ← timing of Asyncify against OPFS (skipped unless sized)
│
├── bridge/                         Unit tests (no engine)
│   ├── bridge_contract_test.dart
│   ├── shared_bridge_test.dart
│   └── protocol/
│       ├── binary_codec_test.dart
│       ├── codec_test.dart
│       └── wire_sync_test.dart     ← parity guard (see below)
│
├── runtime/                        Runtime unit tests (no engine)
│   ├── router_test.dart            ← router vs fake executors: placement,
│   │                                  pinning, dispose, cancel binding (incl.
│   │                                  the cancel-before-submit AND
│   │                                  dispose-before-submit races)
│   ├── pool_test.dart              ← pool and lane vs fake executors: lease
│   │                                  exclusivity, stale lease, drain then
│   │                                  reset, dead never reused, smallest heap
│   │                                  first, prewarm once, waiters in order,
│   │                                  budget frees at terminate
│   ├── keep_bytes_test.dart        ← kKeepBytes equals KEEP_BYTES in heap.rs
│   ├── wire_peek_test.dart         ← peeks + response builders
│   ├── adapter_decides_nothing_test.dart ← the four adapter files
│   ├── native/channel_buffers_test.dart  ← Dart↔Rust layout parity
│   └── web/executor_worker_sync_test.dart ← Dart↔worker protocol parity
│
├── io/                            MemorySource/MemorySink (pure, any
│                                   platform) + FileSource/FileSink
│                                   (native dart:io — VM-only tests)
│
├── hook/, keep/                    build-hook and trim unit tests
│
└── types/                          errors, page_info, pdf_rect +
                                    pdf_task (zone rules) + cancel_hook
```

### The parity guards

These guards stop a cross-boundary contract from drifting silently. The first
three and the fifth parse the OTHER side's source from disk, with no hardcoded
allowlists, so neither language can drift. The fourth keeps the
platform adapters honest. The last two prove the pool and the engine's shared
state.

- **`bridge/protocol/wire_sync_test.dart`** — every `EngineOp` has a Rust match
  arm; every Rust case maps back; sub-dispatches stay disjoint and never leak
  into top-level.
- **`runtime/web/executor_worker_sync_test.dart`** — `executor_worker.js` handles
  every Dart→worker tag, posts only known worker→Dart tags, uses only injected
  protocol codes, calls the wasm surface, and contains no routing logic.
- **`runtime/native/channel_buffers_test.dart`** — Dart's channel layout
  constants match Rust's `shared_buffer.rs` byte for byte.
- **`runtime/adapter_decides_nothing_test.dart`** — the four lease adapter files
  make no routing decisions (no op literals, no peeks, no pins).
- **`runtime/keep_bytes_test.dart`** — `kKeepBytes` equals `KEEP_BYTES` in
  `heap.rs`, the one home of the keep line.
- **`runtime/pool_test.dart`** and **`runtime/router_test.dart`** — the pool, the
  lane and the router against fake executors (`harness/fake_executors.dart`).
- **`engine_proofs/shared_state.rs`** — every static and thread-local of the
  engine is listed in `shared_state.txt` with its class, and the `executor` class
  holds only the pointer to the installed caches and the allocator's counters.

Adding a Dart op without a Rust handler → test fails. Adding a decision to a
lease adapter → test fails. Adding state that outlives an executor without a
class → test fails.

---

## Upstream patches

### pdf_oxide — patched upstream files

Every modification carries a `── pdf_manipulator patch ──` boundary
pair. The marker grep in `UPDATING.md` is the authoritative inventory;
this table summarizes what each patched file carries and why.

| File | What the patches carry |
|---|---|
| `Cargo.toml` | `native-bridge` deps + `office_oxide` and `tiny-skia` as path dependencies |
| `lib.rs` | The shared widget-appearance regeneration helpers, the `pdfa` gate, the two `public-api` gates, `pub mod host;` |
| `document.rs` | External reader variant + `from_external_reader()` (reads the source through a 64KB window; parsed objects live in a cache bounded at 8 MiB), info/encryption/permissions accessors, `collect_refs_of()` (zero-clone GC), streaming `to_docx/pptx/xlsx_writer_flow()`, scan-all `/Subtype` Form detection, raw source access for the incremental save (`source_len`, `read_source_at`, `header_offset`, `xref_repaired`, `object_generation`), the loader that reads an object through a growing window (a stream is its dictionary and an extent, `/Length` verified against `endstream` or repaired by a windowed scan, no size limit), `stream_head()` over `load_object()`, `reopen()` and `source_bytes_whole()` in place of the `source_bytes` field |
| `object.rs` + `objstm.rs` | `StreamBody::{Inline, Extent}`: an `Extent` is a window onto its document's bytes (`Source`) that decrypts itself when read (`raw_reader`, `read_whole_raw`, `decrypted_len`); `decode_stream_data()` is the one named whole read of decoded bytes; `parse_object_stream` and `decode_object_stream` take no decryption closure |
| `encryption/stream.rs` + `encryption/handler.rs` + `encryption/write_handler.rs` + `encryption/rc4.rs` + `crypto/` | `Decrypt` and `Encrypt` adapters (AES-CBC holding back the last block, RC4 as a keystream through the provider's new `rc4_stream`, which a FIPS provider refuses as it refuses `rc4`), `cipher_for` / `stream_cipher` (an object's key, no data) |
| `writer/object_serializer.rs` | `write_indirect`, `write_indirect_encrypted` and `write_stream_open`: an extent body is written in 64 KiB pieces, `/Length` counted from the decrypted length |
| `source.rs` | Fork-only: `Source` (the reader and the encryption handler behind one lock each), `SourceRead`, `SourceCursor`, `Window`, `Counter` |

| `editor/document_editor.rs` | State accessors, `merge_from_reader()` + shared merge core, zero-clone GC BFS that follows the save's page-prune plan (`host/page_prune.rs`: dropped pages, inner `/Pages` nodes and the dropped pages' own annotations are never written, kept pages are re-parented to the root with their inherited attributes, and destinations, outline items, links, `/OpenAction` and form fields that pointed at a dropped page are removed), `write_full_to_writer(PositionedWrite)` (function-wide streaming offsets), per-save page-ref cache, scoped destructive erase (`erase_regions_destructive`), appearance generation for AP-less annotation types, §12.5.5 appearance placement, `add_page_annotation()`, `all_media_boxes()`, image modifications applied in page space (`rewrite_content_stream_with_image_mods` composes with the enclosing CTM, matching `get_page_images`), `staged_or_source_object()` so the resource walk writes a staged XObject/ExtGState instead of the source copy (the sweep skips ids already written), form field properties reach the saved file (`flush_form_fields_to_modified_objects` writes every dirty wrapper's `/TU`, `/Rect`, `/MaxLen`, `/Q`, `/Ff`, `/MK`, `/BS`, `/DA`, not only `/V`) and `remove_deleted_form_fields_from_acroform` drops removed fields from `/Fields` and `/Annots`, `incremental_refusal()` / `incremental_value_only()` / `incremental_objects()` (what an append can carry, and the objects it appends), and the full-rewrite sweep never copies a source object stream or xref stream |
| `extractors/images.rs` | `PdfImageHandle::object_ref()` and `ctm()` — the identity and composed placement the host image reducer groups and measures by |
| `encryption/mod.rs` + `encryption/algorithms.rs` | Raw file key exposed to the writer (`file_key`, `build_with_key`) so streams encrypt with the key the dict advertises; PDF 2.0 Algorithm 10 (`/Perms` for R6) |
| `compliance/converter.rs` | Expose `convert_with_editor`, bundled 12 Liberation fonts (WASM has no system fonts) |
| `writer/pdf_writer.rs` | `finish_to_writer(PositionedWrite)` — streaming save |
| `writer/document_builder.rs` | `build_to_writer(PositionedWrite)` + `assemble_writer()`; per-page state on the page record, a `PageBuilder` that applies page ops at once through `&mut self` (`FluentPageBuilder` wraps it), and `begin_page` / `end_page` / `page_mut` / `page_has_form_field` / `page_has_text` for the host |
| `writer/stamp.rs` + `writer/appearance_stream.rs` + `writer/annotation_builder.rs` | Rubber-stamp normal appearance (label + inline font + text escaping) and the appearance-generation dispatch |
| `converters/office/mod.rs` | Finalize-callback core refactor for streaming, `convert_X_reader_to_writer()`, `ir_to_pdf_writer()`, fallback-font loader visibility |
| `text/word_boundary.rs` | Geometric-gap threshold contract documentation |

### office_oxide — streaming OPC writer

Branch `office_kit/0.1.11-patches`. Marked with `── office_kit patch ──`.

| File | Patch |
|---|---|
| `core/opc.rs` | `OpcWriter<W: Write>` with `ZipWriter::new_stream` (streaming ZIP, no Seek) |
| `core/editable.rs` | `write_to` uses streaming ZIP |
| 10 other files | `Write + Seek` → `Write` bounds throughout |

Everything in `src/host/` is entirely ours — no patch markers needed.

### tiny-skia — a pixmap that is a window

Branch `pdf_manipulator/0.12.0-patches`. Marked with `── pdf_manipulator patch ──`;
every file of it is upstream code, so every change is between markers.

The page renderer paints a page in bands (2 MiB of raster at a time), and a
band must hold the bytes the whole page holds. A band is a `Pixmap` with an
`origin`; drawing is in the page's coordinates and the origin leaves after the
float mapping, in whole pixels.

| File | Patch |
|---|---|
| `pixmap.rs`, `mask.rs` | `origin`, `new_at`, `new_like`; sub-regions keep the plane's coordinates |
| `edge.rs`, `edge_builder.rs`, `scan/path.rs`, `scan/path_aa.rs` | Edges are built from the plane's floats and shifted in fixed point; a vertical clip advances the edge (`Edge::advance_to`) instead of chopping it; "contained" asks about columns |
| `scan/hairline.rs`, `scan/hairline_aa.rs`, `scan/mod.rs` | The same for hairlines and rects; rows above the window are dropped, not moved to its edge |
| `pipeline/{highp,lowp,blitter}.rs` | Shaders seed from the plane's coordinates (gradients, patterns, image sampling) |
| `painter.rs` | Tiles are windows (no float translation); `draw_pixmap` sits at the source's own origin |
| `tests/windows.rs` | The proof: windows equal the same rows of a whole canvas |

The limits of the exactness are in `docs/UPDATING.md`.

---

---

## Trim — ship only the ops the app calls

The engine cannot be tree-shaken by Dart (the op name crosses FFI as
runtime data, so every dispatch arm looks reachable). Trim recreates the
contract at the source level: a detector proves which capabilities the
app can reach, the keep-set maps to cargo features that delete the
unreachable dispatch arms and modules, and Rust's LTO erases everything
behind them. Conservative by construction — anything not provably unused
is kept; the failure direction is a bigger binary, never a broken app.

### Vocabulary

Users speak capabilities, never cargo features. Core
(parse/write/edit/forms/builder) is always included.

| Capability | Gates |
|---|---|
| `render` | page rasterization, image re-compression |
| `signatures` | signing + verification |
| `pdfa` | PDF/A validation + conversion (bundles the Liberation faces the spec requires) |
| `office` | PDF ↔ DOCX/PPTX/XLSX (gates the office_oxide crate; requires `extract`) |
| `extract` | text extraction + search + page/document classification (includes the CJK CID→Unicode tables) |

Grammar (`hooks: user_defines: pdf_manipulator:` in the app pubspec) — three
flat keys: `keep` picks WHICH capabilities compile (`auto` detects · `all`/
absent keeps everything · `[render, …]` is an exact set); `detector` picks
HOW `auto` detects (`scan` default · `record-use`/`compare` experimental) and
is valid ONLY with `keep: auto`; `build` picks what the engine is optimized
for (`speed` default+prebuilt · `size` opt-level z · `debug` symbols kept).
Invalid configs are unrepresentable by design (`lib/src/hook/pdf_config.dart`,
the ONE parser): unknown keys, bad values, and the one cross-axis coupling
(`detector` without `keep: auto`) all fail the build LOUDLY — never a silent
change to what ships. All keys are read the SAME way for web and native: the
native hook gets them from `BuildInput.userDefines`; the web `setup` script
reads the same pubspec block itself (`lib/src/hook/user_defines.dart`), so the
command
never carries flags and there is one place to configure both.

The design rules behind that grammar:

1. One key, one artifact — auto and manual both produce a keep-set
   feeding one build pipeline; manual IS the detector override.
2. Users speak capabilities, never cargo features — names stay stable
   across engine bumps; internals (native-bridge, icc, …) are not
   expressible, so not droppable.
3. Keep-list, not drop-list — "ship only what I say" stated exactly;
   forgetting something fails safe via the typed error.
4. Core is always included; dependencies expand automatically
   (`keep: [office]` also keeps `extract`).
5. Config errors are loud — never a silent full binary.
6. One pubspec entry drives native and web identically.

### Detectors

| Detector | Status | How |
|---|---|---|
| `scan` (default) | stable | dependency-free text scan over the app source (`lib/src/keep/detector.dart`): files importing the package (directly or through a re-export barrel) are searched for capability member names; errs only toward over-keeping. Any unreadable file → full binary + warning (fail closed) |
| `record-use` | EXPERIMENTAL, internal | the SDK's `@RecordUse` recordings, read in the link hook after AOT; dormant until the SDK experiment activates |
| `compare` | internal | the scan trims; the link hook reports the recorded set for diffing |

The experimental detector is deliberately not documented for consumers yet —
both detectors ship so the RecordUse integration stays built, in sync,
and testable as the SDK matures, instead of being re-invented the day it
stabilizes. The public API does not get reworked for RecordUse's current
statics-only limit; the shim absorbs it.

### Build-mode behavior

| | debug build | release build |
|---|---|---|
| no trim | prebuilt download | prebuilt download |
| scan trim (auto or `keep:`) | compiles trimmed (Rust needed, once — cargo caches) | compiles trimmed |
| record-use trim | prebuilt full binary (debug skips linking) | link hook compiles trimmed |

The scan detector trims debug builds too, on purpose: a too-narrow
`keep:` surfaces its typed "not enabled in this build" error at the
developer's desk, not in release testing. Debug/release parity beats a
one-time compile.

Rust is required exactly when the resolution waterfall skips the GitHub
Release download: any custom (trimmed) feature set, version 0.0.0
consumers (git branch / path), or download failure — then the vendored
source in the pub tarball compiles locally.

### The machinery

- `lib/src/keep/` — capabilities + grammar (`capabilities.dart`), the
  text-scan detector (`detector.dart`), the RecordUse shim
  (`record_use_shim.dart`). Tooling-only: never exported by the barrel,
  zero bytes in consumer apps.
- `lib/src/hook/` — ONE compile path, two callers: `build_constants.dart`
  (build.json), `engine_compiler.dart` (CodeConfig → target mappings, NDK
  env, the cargo invocation), `pdf_config.dart` (the one config parser —
  keep/detector/build/scan-dirs/asyncify, mismatch-proof), `keep_plan.dart` (config → plan,
  recordings → keep-set), `engine_build.dart` (the `build:` axis),
  `asyncify_list.dart` (which functions the Asyncify wasm instruments).
  `hook/build.dart` and `hook/link.dart` are thin callers.
- The cutting doctrine: cut ROOTS and let LTO shake. Because the shipped
  artifact is a cdylib, only exported symbols are roots — gating a few
  dispatch entry fns lets LTO delete whole subsystems. Module-level cfg
  gates are the exception, needed only where core references pin data
  (the CID tables via font parsing) or where an optional dependency must
  not compile at all (office_oxide).
- Engine-side, each bridge op is a self-contained UNIT
  (`vendor/pdf_oxide/src/host/ops/`): a registry entry + handler on one
  shared calling convention + an exported `pdf_op_<name>_anchor` symbol
  (inert today; the referent a future Dart static-linking toolchain
  garbage-collects against). Each unit carries its own handler body in
  its unit file; `bridge_api.rs` is the door only (parse the request,
  dispatch through the registry). The op still travels as data through
  the single bridge door — the unit layer carries reachability, the door
  carries bytes. `registry.rs` is the one swappable backend (explicit
  table today, linker-driven collection when the platform arrives).
  Each droppable capability is a cargo feature; excluded handlers answer
  a typed "not enabled in this build" error (defense-in-depth — the
  detector should make it unreachable).
- `make shake-audit` keeps the guarantee durable across upstream rebases:
  full vs core-only builds, symbol autopsy, size ceiling, and runtime
  probes of the typed errors. It is also the measurement tool — the
  size-measuring recipe (audit modes, the README verifier, and why the
  linker map is the only trustworthy instrument) lives in
  [`UPDATING.md`](UPDATING.md).
- `make check-keep-sets` compiles every keep-set a user can write, both
  targets, with `cfg(test)` on, so the trim probes are checked too. The
  list is generated from `capabilities.dart`, one home for the requires
  graph. CI's `keepsets` job runs it on every PR that touches the
  engine or the keep vocabulary. It catches code that compiles at the
  full feature set and fails at a subset, such as upstream code that
  uses a module one of our feature gates removes.

## The one-line summary

> **Four layers. One router per instance; lanes lease executors from one pool;
> adapters decide nothing. One binary format. Native, and four web I/O modes
> auto-detected (Atomics > JSPI > OPFS > Asyncify), one wasm per mode. Dispose ends lanes and returns
> executors; an executor is terminated only when it is dead, over the keep line
> or surplus. Budgets queue, never fail. Bounded memory on every path, measured,
> with one declared cost per text piece of the page being read.**
