# pdf_manipulator — Architecture

How the package is wired. Four layers, two platforms, one binary
format end to end, streaming bounded-buffer I/O everywhere.

---

## The contract

Eight load-bearing guarantees. Every architectural decision serves one.

1. **Four layers.** Consumer API → SharedBridge → Transport → Rust.
   Each layer has one job. No layer imports from a layer above it.

2. **One binary format.** SharedBridge encodes every op as binary
   bytes. Same bytes on native and web. Same Rust parser on both
   targets. Zero format divergence.

3. **One Rust entry point.** `bridge_api.rs` compiles for native
   (`extern "C"`) AND WASM (`#[wasm_bindgen]`). Same source, same
   parser, same dispatch calls.

4. **Multi-source / multi-sink.** Every operation can take N indexed
   DataSources and M indexed DataSinks. Sources are served on demand
   via readAt callbacks. Sinks receive chunks. No full-file reads
   anywhere on the Dart side.

5. **Bounded-buffer I/O.** Every op streams its input and output through
   fixed-size buffers (64KB read, 256KB write); the transport never scales
   with file size, on either platform. Test guards throw on any over-budget
   `readAt` / `write`, proving the *transport* is O(1). (Full peak-memory
   verification of Rust-internal processing is roadmapped, not yet automated.)

6. **Upstream untouched.** `ffi.rs` and `wasm.rs` — upstream's own
   binding surfaces — are never called and never modified. Upstream
   merges stay clean.

7. **Lanes, not pools.** Every instance owns isolated lanes — a
   detached Rust thread with its own engine state (native) or a Web
   Worker with its own WASM instance (web). The Dart isolate never
   blocks. Dispose kills lanes instantly: no joins, no awaits, no
   timeouts.

8. **Patches are marked.** Every change to a vendored upstream file is
   wrapped in a boundary comment (`── pdf_manipulator patch ──` in
   pdf_oxide, `── office_kit patch ──` in office_oxide), so upstream
   merges stay clean.

---

## The four layers

Each layer has one job and never imports from a layer above it. This is the
responsibility each layer holds; the full file list is in **Source tree**, below.

**Layer 1 — Consumer API** (`lib/src/ops/`) — the public surface: `Pdf`
(lifecycle) plus the `PdfDoc` / `PdfEditor` / `PdfBuilder` handles, and the
standalone and sugar one-shots. Platform-blind; talks only to the abstract
`PdfBridge`.

↓

**Layer 2 — Bridge** (`lib/src/bridge/`) — one `SharedBridge` for both
platforms. Encodes each op to binary, sends it through the transport, decodes
the binary response back to typed Dart. Platform-blind.

↓

**Layer 3 — Runtime** (`lib/src/runtime/`) — one shared brain (`router.dart`:
pinning, placement, dispose) plus two dumb adapters (native over FFI, web over
`postMessage`). The adapters move bytes and serve `readAt` / chunk callbacks on
the main isolate — zero routing decisions, zero PDF knowledge.

↓

**Layer 4 — Rust** (`vendor/pdf_oxide/src/host/`, entirely ours) — one entry
point (`bridge_api.rs`, cfg-gated for native `extern "C"` and WASM
`#[wasm_bindgen]`) over a binary parser and one typed dispatch function
per op. Upstream's own `ffi.rs` / `wasm.rs` are never called or modified
(see **Upstream patches**).

---

## Data flow

### Simple op (one source, no output)

```
pdf.open(source)
  ↓
SharedBridge.open()
  │  encodeRequest('open', {sourceLength: N})  →  Uint8List
  ↓
Router.execute(bytes, sources: [source], keepSources: {0})
  │  pin lookup / least-loaded placement → one lane
  │
  ├── Native: lane_submit via FFI → lane thread mailbox
  └── Web:    postMessage → lane_worker.js → lane_execute via WASM
  ↓
bridge_api.rs  (SAME code, both targets)
  │  Request::parse(bytes)  →  match "open"  →  dispatch::open_document
  │  source bytes served on demand via readAt callback
  ↓
response bytes  →  decodeResponse  →  PdfDoc
```

### Multi-source op (primary PDF + secondary input)

```
editor.mergeFrom(otherPdf)
  ↓
SharedBridge._exec(editorMergeFrom, {sourceLength: N},
    sources: [otherPdf])     ← secondary source at index 0
  ↓
Router.execute(bytes,
    sources: [otherPdf])     ← lane serves readAt by sourceIndex
  ↓
bridge_api.rs
  │  source[0] = otherPdf via readAt callback (64KB chunks)
  │  editor.merge_from_reader(BoxedReader for source[0])
  ↓
response bytes
```

---

## Source tree

```
lib/
├── pdf_manipulator.dart                    ← barrel export
└── src/
    ├── ops/                                ← CONSUMER API
    │   ├── pdf.dart                        ← lifecycle, Pdf class, ensureInitialized
    │   ├── pdf_doc.dart                    ← PdfDoc: extract, search, render, ...
    │   ├── pdf_editor.dart                 ← PdfEditor: mutations only
    │   ├── pdf_builder.dart                ← PdfBuilder: create from scratch
    │   ├── pdf_standalone.dart             ← sign, convertTo, convertToPdf, extractPages
    │   └── pdf_sugar.dart                  ← merge, split, watermark, compress, ...
    │
    ├── bridge/                              ← BRIDGE
    │   ├── pdf_bridge.dart                 ← abstract PdfBridge + handle contracts
    │   ├── pdf_transport.dart              ← PdfTransport interface
    │   ├── shared_bridge.dart              ← ONE bridge, both platforms; births every PdfTask
    │   ├── shared_bridge_doc.dart          ← doc handle (part)
    │   ├── shared_bridge_editor.dart       ← editor handle (part)
    │   ├── shared_bridge_builder.dart      ← builder + page handles (part)
    │   ├── create.dart                     ← conditional import router
    │   ├── _create_native.dart             ← → SharedBridge(Router(NativeLaneHost))
    │   ├── _create_web.dart                ← → SharedBridge(Router(WebLaneHost))
    │   │
    │   └── protocol/
    │       ├── binary_codec.dart           ← binary request/response encoding
    │       ├── codec.dart                  ← typed request builders + response decoders
    │       └── op.dart                     ← EngineOp enum (the wire names)
    │
    ├── runtime/                             ← THE LANE RUNTIME
    │   ├── router.dart                     ← shared brain: pinning, placement, dispose
    │   ├── lane.dart                       ← Lane / LaneHost / LaneJob contracts
    │   ├── wire_peek.dart                  ← minimal wire reads (op, handleId)
    │   │
    │   ├── native/
    │   │   ├── native_lane.dart            ← dumb adapter: 4 verbs over FFI
    │   │   ├── native_lane_host.dart       ← spawn + one-time FFI bootstrap
    │   │   ├── channel_buffers.dart        ← condvar memory layout
    │   │   └── lane_bindings.dart          ← @Native FFI bindings
    │   │
    │   └── web/
    │       ├── web_lane.dart               ← the lane: job lifecycle, 3 I/O modes
    │       ├── web_lane_host.dart          ← worker boot handshake, budget, pristine pool
    │       └── lane_protocol.dart          ← codes injected into lane_worker.js
    │
    ├── types/                              ← shared types (all exported)
    │   ├── cancel_hook.dart                ← CancelHook: binds PdfTask.cancel to its job
    │   ├── data_source.dart                ← DataSource interface
    │   ├── data_sink.dart                  ← DataSink interface
    │   ├── errors.dart                     ← PdfError sealed hierarchy
    │   ├── pdf_config.dart                 ← PdfConfig (webIoMode, URLs)
    │   ├── pdf_enums.dart                  ← PdfIoMode, PdfEncryptionAlgorithm, ...
    │   ├── pdf_image.dart                  ← PdfImage, RenderedPage
    │   ├── pdf_page_info.dart              ← PdfPageInfo
    │   ├── pdf_pages.dart                  ← PdfPages sealed (all, single, range)
    │   ├── pdf_params.dart                 ← PdfSaveOptions, PdfEncryption, ...
    │   ├── pdf_rect.dart                   ← PdfRect
    │   ├── pdf_signature.dart              ← PdfSignatureInfo
    │   ├── pdf_task.dart                   ← PdfTask<T>: Future + cancel()
    │   └── search_result.dart              ← SearchResult
    │
    └── hook/                               ← build + setup support (not consumer API)
        ├── asset_hashes.dart               ← SHA-256 hashes for all release assets (native + web)
        └── resolver.dart                   ← ResolveRequest + 5-step waterfall (shared by hook + setup)

web_assets/                                 ← committed in git (except .wasm — gitignored, too large)
├── lane_worker.js                          ← the lane body: reader registry, 3 I/O modes,
│                                              lane_init + lane_execute (zero routing logic)
├── pdf_oxide.js                            ← wasm-bindgen glue (committed, paired with .wasm)
└── pdf_oxide_bg.wasm                       ← WASM binary (gitignored — downloaded from releases)

build.json                                  ← single source of truth: crate name, repo,
                                               cargo features, web asset map

hook/
├── build.dart                              ← Flutter hook entry (native) + resolveWeb() (web)
│                                              reads build.json, private _resolveNative
└── link.dart                               ← passthrough today; foundation for tree-shaking

bin/
└── setup.dart                              ← CLI: setup <target> (web|android|ios|macos|linux|windows)

tool/                                       ← .sh = orchestration wrappers; .dart = programs
│                                              (anything parsing structured data or using a library)
├── lib.sh                                  ← shared helpers (sourced by 2+ scripts)
├── analyze.sh                              ← format + Dart + Rust static analysis
├── check_alignment.sh                      ← 16 KB ELF alignment check for Android APK
├── check_deps.dart                         ← dependency-drift report (make check-deps; advisory)
├── compile_rust.sh                         ← Rust → native / wasm / per-target / both
├── generate_fixtures.dart                  ← test fixture generator (make fixtures; stamp-protected)
├── release.sh                              ← 7 modes: --gate, --discover, --github-notes,
│                                              --update-tag-hashes, --stamp-changelog,
│                                              --add-git-install, --add-pub-install
└── run_web_test.sh                         ← flutter drive web integration test

vendor/
├── pdf_oxide/                              ← forked yfedoseev/pdf_oxide (submodule)
└── office_oxide/                           ← forked yfedoseev/office_oxide (submodule)

vendor/pdf_oxide/src/
├── host/                                   ← OUR CODE
│   ├── mod.rs
│   ├── bridge_api.rs                       ← single entry, cfg-gated
│   ├── binary_codec.rs                     ← wire format parser/encoder
│   ├── dispatch.rs                         ← every op as a typed function
│   ├── positioned_write.rs                 ← CountingWriter + SeekWriter
│   ├── sign.rs                             ← O(1) signing with AcroForm
│   ├── image_optimizer.rs                  ← JPEG recompress
│   ├── font_optimizer.rs                   ← Standard 14 unembedding
│   ├── constants.rs                        ← buffer sizes
│   ├── lane_state.rs                       ← per-lane engine state (no locks)
│   ├── native/                             ← lanes + condvar I/O
│   │   ├── lane.rs                         ← lane thread body, job channels, cancel registry
│   │   ├── lane_table.rs                   ← global lane table, budget, FFI surface
│   │   ├── cancel.rs                       ← CancelToken (lane + job flags)
│   │   ├── callback_reader.rs, callback_writer.rs
│   │   └── shared_buffer.rs
│   └── wasm/                               ← JS callback reader/writer
│       ├── js_reader.rs                    ← Read+Seek via host_read_at
│       └── js_writer.rs                    ← Write via host_write_chunk
│
├── compliance/fonts/                       ← 12 Liberation TTF files (SIL OFL)
├── ffi.rs                                  ← UPSTREAM (untouched)
├── wasm.rs                                 ← UPSTREAM (untouched)
└── (engine: document.rs, editor/, renderer/, search/, signatures/, ...)
```

---

## The binary wire format

Same layout in both directions. Op args only — PDF bytes travel
through the transport's I/O channels, never through the codec.

```
Request:  [op_len: u8] [op: UTF-8] [field_count: u16 LE] [fields...]
Response: [status: u8]  [field_count: u16 LE] [fields...]
            status 0 → [msg_len: u32 LE] [msg: UTF-8]  (error)
            status 1 → fields                           (ok)
            status 2 → (no payload)                     (cancelled)

Field:    [key_len: u8] [key: UTF-8] [type: u8] [value]

Types:    0=null  1=i32  2=i64  3=f64  4=bool  5=string  6=bytes
          7=int_list  8=float_list  9=string_list  10=map_list
```

Op names are the EngineOp `.wire` strings (`"open"`, `"extract"`,
`"editorMutate"`, etc.). No numeric mapping to keep in sync — the
enum IS the protocol.

---

## Streaming I/O

### The rule

The transport is O(1) on every path: input and output move through
fixed-size buffers (64KB read, 256KB write), never a full-file buffer
on the Dart side. Test guards enforce the chunk limits mechanically
(see **Test architecture**). Full Rust-internal peak-memory
verification — catching any intermediate accumulation during
processing — is designed and tracked in the capability roadmap, not
yet automated.

### Transport contract

```dart
Future<...> execute(
  Uint8List request, {
  List<DataSource> sources,     // indexed 0, 1, 2, ...
  List<DataSink>   sinks,       // indexed 0, 1, 2, ...
  Set<int>         keepSources, // persist across ops (handle lifetime)
});
```

### Per-operation I/O paths

| Operation | Input | Output |
|---|---|---|
| open | reader[0] on demand | — |
| extract / search / validate / classify | reader[0] on demand | bounded text |
| render | reader[0] on demand | framed writes, one page at a time |
| extractImages | reader[0] on demand | framed writes, one image at a time |
| editorSave | pinned reader[0] | PositionedWrite streams objects |
| builderSave | — | PositionedWrite streams objects |
| sign | reader[0] 64KB chunks | writer[0] sequential |
| mergeFrom | reader[0] secondary PDF | — |
| addImageStamp / embedFile / builder.image | reader[0] secondary data | — |
| convertTo (PDF → DOCX/PPTX/XLSX) | reader[0] on demand | streaming ZIP via office_oxide |
| convertToPdf (DOCX/PPTX/XLSX → PDF) | reader[0] on demand | CountingWriter streams objects |

### PositionedWrite — Write without Seek

PDF serialization needs byte offsets for the xref table but never
seeks backward. `PositionedWrite` captures exactly that:

- **`CountingWriter<W: Write>`** — wraps any Write, tracks position
  via counter. Enables O(1) streaming to sinks that can't Seek.
- **`SeekWriter<W: Write+Seek>`** — delegates to `stream_position()`.
  For Cursor, BufWriter, etc.

### Native I/O — condvar + shared memory

Each source gets its own `CallbackReader` + condvar buffer; each sink
its own `CallbackWriter` + buffer. The lane thread reads/writes via
shared memory with condvar synchronization and a per-job
`CancelToken` checked at every wait. Dart serves the bytes on the
MAIN isolate via `NativeCallable.listener` — no helper isolate.

### Web I/O — three modes, auto-detected

| Mode | Mechanism | Requires |
|---|---|---|
| **JSPI** | `WebAssembly.Suspending` suspends WASM, JS fetches async, resumes | Chrome 137+ / Firefox 139+ |
| **Atomics** | `Atomics.wait` blocks WASM, JS fills SAB, `Atomics.notify` wakes | `SharedArrayBuffer` (COOP/COEP headers) |
| **OPFS** | Sources pre-copied to OPFS disk, WASM reads via `SyncAccessHandle` | All modern browsers |

Auto-detection priority (best first):
```
if ("Suspending" in WebAssembly) → JSPI     (streaming, no headers)
else if (SharedArrayBuffer)      → Atomics  (streaming, needs COOP/COEP)
else                             → OPFS     (pre-copy fallback)
```

Detection follows the `wasm-feature-detect` pattern (GoogleChromeLabs).
Force any mode: `Pdf(config: PdfConfig(webIoMode: PdfIoMode.jspi))`.

All three modes produce the same `Read+Seek` trait on the Rust side.
`bridge_api.rs` is identical across modes. One WASM binary serves all
three. Only OPFS pre-copies to disk.

**Web reader registry:** each lane worker keeps a reader map.
Held readers (kept sources from handle-creating ops) mount FIRST in
each exec's reader list; the job's own sources follow. Non-held
readers are dropped in `finally`; held ones survive until the
handle's dispose releases them. Exactly symmetric with native's
held-channel adoption.

### Backpressure

**Sources (input):** naturally backpressured on all platforms. The
engine calls `readAt` and blocks until the host returns bytes. The
engine never reads faster than the host can serve.

**Sinks (output):** backpressured per mode:

- **Native:** condvar shared buffer blocks the lane thread when
  full. Dart drains it before the writer can continue.
- **JSPI:** `host_write_chunk` returns a Promise. JSPI suspends the
  WASM stack. The worker forwards the chunk to Dart, Dart's sink
  consumes it, Dart acks, the worker resolves the Promise. WASM
  resumes. One chunk in flight at a time.
- **Atomics:** `host_write_chunk` posts the chunk then
  `Atomics.wait` on a dedicated write SAB. Dart consumes the chunk,
  acks via `Atomics.store + notify`. The worker wakes. One chunk in
  flight at a time.
- **OPFS:** fire-and-forget via `postMessage`. No blocking mechanism
  available (no SAB, no JSPI). In practice the WASM producer is
  slower than the main thread consumer for OPFS because OPFS source
  reads are the bottleneck, not output writes.

### Important: sources are random-access, not forward-only

`DataSource.readAt(offset, count)` is random-access. The engine
reads the xref table at the end of the file, then jumps to arbitrary
offsets to load objects. A forward-only pipe (one-shot socket,
stdin, HTTP without Range headers) cannot be a `DataSource`. Buffer
the full content first, or use a random-access backing store.

So only the backing store ever holds the whole file — the engine itself
pulls at most 64KB per `readAt`.

### OPFS mode: O(N) disk + latency

OPFS is the universal fallback (what users get without COOP/COEP
headers on browsers older than Chrome 137). It pre-copies each
source file to OPFS disk before processing — O(N) disk space, O(N)
time before the first output byte, subject to browser storage quota.

Pre-copies live under `pdf_manipulator_lanes/{sessionId}/{workerId}/`
— one random session directory per page load, one directory per
worker inside it. The cleanup rule:

> **Never race the living. Only sweep the dead.**

While a worker lives, IT deletes its own files — it created them,
holds their handles, and is their only consumer, so every deletion
is same-agent and race-free by construction. One verb per job end
(`opfsDrop`), one per held-source release (`releaseHeld`). No
retries exist on the live path because no race exists.

Death is handled by liveness, not by guesswork: every owner holds a
[Web Lock](https://developer.mozilla.org/docs/Web/API/Web_Locks_API)
for its lifetime — the page holds its session's lock, each worker
holds its own directory's lock (acquired before its first file can
exist). The browser releases a lock when its agent dies; that
release is normative spec text, the web's equivalent of the native
lane's every-job-posts-exactly-once contract (and the reason
`FinalizationRegistry`, which is best-effort by spec, is not used).
An acquirable lock IS the death certificate: retiring a worker
awaits its lock, then reclaims its whole directory; a new session's
boot sweep does the same for dead sessions. A crashed tab can never
leak disk permanently, and the only convergence loop in the runtime
runs strictly against the provably dead — where it always wins.

Use `await pdf.ensureInitialized()` at startup to detect the mode.
If `PdfIoMode.opfs` is returned, consider warning the user or
prompting for COOP/COEP headers.

---

## Instance architecture — lanes

```
Pdf()       = isolated instance: a Router + its lanes
PdfDoc      = read-only document session     (pdf.open)
PdfEditor   = mutation session               (pdf.edit)
PdfBuilder  = creation session               (pdf.build)
```

### The three nouns

- **Lane** — one isolated execution unit. Native: a detached Rust
  thread with a crossbeam mailbox and its own `LaneState` (documents,
  editors, builders — no locks, single owner). Web: a Worker running
  `lane_worker.js` with its own WASM instance. A lane runs one job at
  a time; nothing is shared between lanes.
- **Job** — one operation submitted to a lane, with a cancel ticket.
  Rust guarantees every accepted job posts exactly one result — the
  guarantee Dart's resource cleanup is built on.
- **Router** — the one shared brain (`runtime/router.dart`, both
  platforms). Pins handles to the lane that created them
  (response-driven: any success carrying a handleId creates a pin),
  places handle-less ops on the least-loaded lane, spawns lanes up
  to `maxLanes`, owns held-resource lifecycle, and implements
  instant dispose.

### The two rules

1. **Single owner.** Every piece of state has exactly one owner
   thread. Engine state is owned by its lane. Router state is owned
   by the main isolate's event loop. No cross-thread state, no
   locks above the I/O channels, no check-then-act races possible.
2. **Shared brain, dumb edges.** All decisions live in shared code
   (Router, Rust lane table). The platform adapters translate four
   verbs — submit, cancelJob, releaseHeld, kill — and decide nothing.

### Lifecycle

```dart
final pdf = Pdf();

// Eagerly init — returns detected I/O mode without running an op
final mode = await pdf.ensureInitialized();
print(mode);  // PdfIoMode.jspi, .atomics, .opfs, or .native

// Sessions are children of the instance
final doc = await pdf.open(source);
await doc.dispose();        // frees ONE doc (lane pin removed)

await pdf.dispose();        // kills every lane — instant
```

### Dispose rules

- `doc.dispose()` / `editor.dispose()` / `builder.dispose()` — frees
  one session. Others + instance stay alive.
- `pdf.dispose()` — kills every lane synchronously: each kill flips
  the lane's cancel flag, wakes its parked I/O, completes its
  pending submits as cancelled, and frees its platform resources.
  No joins, no awaits, no timeouts.
- In-flight ops on a disposed instance resolve with `PdfCancelled`
  (wire status byte 2 — typed, never string-matched).
- Double dispose is safe (no-op). Calling methods on a disposed
  handle throws `StateError`.

### Kill semantics (native)

A lane kill must guarantee no Rust thread ever touches a dead
`NativeCallable` ("Callback invoked after it has been deleted" is a
FATAL abort on Windows). The order:

```
1. lane_kill flips the lane cancel flag, then under each channel's
   pair mutex: set FLAG_CANCELLED + notify_all
     → a parked lane thread wakes INTO the cancelled check
     → the lock-order pairing makes notify-after-kill impossible
2. Every pending job still posts its (cancelled) result
     → Dart's post-driven cleanup frees each job's buffers
3. The lane thread sees the flag, drains its mailbox with cancelled
   posts, and exits — detached, never joined
```

### Kill semantics (web)

`worker.terminate()` frees the worker, its WASM heap, and every open
OPFS `SyncAccessHandle` in one stroke. The lane completes its pending
submits as cancelled on the Dart side; the host reclaims the dead
worker's OPFS directory once its liveness lock confirms the agent is
gone (see **OPFS mode**, under Streaming I/O). A killed-before-use
worker is returned to the pristine pool instead of terminated (see
budget below).

### Parallel ops + the budgets

- Ops on different handles run on different lanes — truly parallel.
- Ops on one handle are pinned to its lane — serial, by design.
- The Router spawns lanes up to `maxLanes` per instance
  (default `max(2, cores / 2)`, configurable via `PdfConfig`).
- **Native global budget:** at most 128 lane threads process-wide.
  Past the cap, lane spawns queue FIFO inside Rust — an op can
  wait, it can never fail for capacity.
- **Web global budget:** at most 64 live workers page-wide, FIFO
  waiters past the cap. Lanes killed before receiving work return
  their worker to a pristine pool, so rapid create+dispose churn
  recycles workers instead of booting thousands.
- The web worker boot uses an explicit `booted → init → ready`
  handshake: the worker announces when its message handler is
  attached, because a message posted before that is silently
  dropped (the cross-origin blob bootstrap's dynamic import is not
  part of module evaluation).

### Race freedom by construction

Beyond the two rules, four mechanical properties close the remaining
race classes:

- **One-way state machines.** Job: queued → running → done/cancelled/
  failed. Lane: spawning → live → killed. Every transition is
  set-once; a consumer that misses an update is harmless because it
  re-checks at its next boundary (I/O point, dequeue point).
- **No timing-based correctness.** No timeout anywhere is load-bearing.
  A slow device changes latency, never behavior. The only timer is a
  debug-build heartbeat that LOGS long I/O parks — it decides nothing.
- **Keys, not pointers, across FFI.** Dart holds opaque lane keys
  looked up in a locked table; a call racing a kill finds nothing and
  gets a well-defined "lane disposed" result. Keys are never reused.
- **Idempotent teardown.** dispose / cancel / kill are all safe to
  call twice, from anywhere, in any order.

### Per-op cancellation

Every engine method returns a `PdfTask<T>` — a `Future` plus
`cancel()`. The task's `CancelHook` is bound by the Router to the
submitted job; cancel-before-submit resolves without reaching a lane
(both cancel AND dispose are re-checked synchronously before submit
— without the dispose re-check, a same-tick dispose would let the op
resume on a dead Router and spawn a zombie lane that no kill ever
reaches). A live cancel flips the job's cancel
flag and wakes its parked I/O: native cancels the job's registered
channels; web answers the in-flight host read with the cancelled
code (and wakes an OPFS pre-copy parked on the caller's DataSource).
The lane survives; only that job dies — its result posts as
cancelled. Fire-and-forget tasks absorb the cancelled outcome
silently; real failures on unlistened tasks are re-raised loudly.

---

## CI/CD architecture

### Vocabulary

| Term | Meaning | Example |
|---|---|---|
| **target** | What you build for | android, ios, macos, linux, windows, web |
| **runner** | CI machine that runs the job | ubuntu-latest, macos-14, windows-latest |
| **host** | Machine doing the building (CI or dev) | `Platform.isMacOS`, `uname` |
| **capability** | One installable concern | fvm, rust, java, chrome, headless-display |
| **build** | Compile for dev iteration | `make build-native` |
| **compile** | Produce release binaries for upload | `make compile-macos` |
| **verify** | Prove release build works (output thrown away) | `make verify-android` |
| **test** | Run test suites | `make test-unit` |
| **release** | Publish a version (tag, upload, pub.dev) | `release.sh --discover` |

"Platform" is not used internally. "Cross-platform" is kept in
user-facing text (README, pubspec).

### Principles

1. **Makefile is the interface.** CI runs `make <target>`. All build
   logic lives in Makefile and scripts. CI YAML has zero build logic.

2. **Scripts handle their own deps.** Rust targets, wasm-bindgen,
   binaryen, cross-compilers — auto-installed by the script that
   needs them. `$CI` env var: auto-install on CI, error with
   instructions on dev. `rustup target add` and `cargo install` are
   always safe (user-space). System packages auto-install only on CI.

3. **Dart is never called from bash.** Bash reads `build.json` with
   pure `sed`/`grep`. No python3, no jq, no dart.

4. **Capabilities, not layers.** Each CI concern is one action under
   `capabilities/`. No runner layer, no target layer — just
   independent capabilities. Each handles all runners internally
   via `runner.os` guards. `runner.os` guards exist ONLY inside
   capability actions — nowhere else.

5. **One orchestrator.** `make-target` provisions capabilities then
   runs `make`. Zero logic — flat list of conditional capability
   calls. Workflows call `make-target`, never capabilities directly.

6. **Runners are configurable.** Every workflow defines its runner
   as a `workflow_dispatch` input with a default. No hardcoded
   `runs-on` except the 2-second `inputs` job. Infra jobs (gate,
   triage, lint) use one configurable runner. Test/compile jobs use
   per-row matrix runners.

7. **Matrix row is the manifest.** Each row declares which
   capabilities to activate and which runner to use. Adding a new
   combo = one line. Adding a new capability = one action + one
   input on `make-target` + add to rows that need it.

### Actions

```
.github/actions/
├── capabilities/          13 independent leaves
│   ├── fvm/               Flutter SDK (all runners)
│   ├── rust/              Rust toolchain + sccache (all runners)
│   ├── java/              JDK (all runners)
│   ├── chrome/            Chrome + ChromeDriver (Linux/macOS/Windows)
│   ├── headless-display/  xvfb on Linux, no-op elsewhere
│   ├── hw-accel/          KVM on Linux, no-op elsewhere
│   ├── gradle-cache/      Two-phase restore/save + daemon stop
│   ├── xcode-cache/       Xcode derived data cache
│   ├── pods-cache/        CocoaPods cache
│   ├── wasm-cache/        WASM build output cache
│   ├── wasm-build/        Compile WASM from source
│   ├── android-emulator/  x86_64 + aosp_atd (not macOS ARM — no nested virt)
│   └── ios-simulator/     macOS only
├── make-target/           Orchestrator (capabilities → make)
└── debug-ssh/             SSH tunnel for CI debugging
```

Each capability:
- Is one concern, one action, one file.
- Handles all valid runners internally (`runner.os` guards).
- Documents runner behavior in its description (what it does on
  each runner, including no-ops).
- Never calls another capability. Independent leaves.

### Workflows

| Workflow | Trigger | Runner model |
|---|---|---|
| `ci.yml` | PR to prod/dev | All jobs on one input runner (default: ubuntu) |
| `full-test.yml` | `ready-to-test` label or dispatch | Single matrix — core + portability [P] rows |
| `create-release.yml` | Changelog push or dispatch | Infra on input runner, compile on matrix |
| `pr-lint.yml` | PR to prod/dev | All jobs on one input runner |
| `triage.yml` | Issues/PRs | All jobs on env runner |
| `upgrade-check.yml` | Daily or dispatch | All jobs on input runner |
| `debug-ssh.yml` | Dispatch only | Direct input runner |

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

### Dependency ownership

| Dep | Owner | CI behavior | Dev behavior |
|---|---|---|---|
| Rust targets | `compile_rust.sh`, `hook/build.dart` | Auto-install (safe) | Auto-install (safe) |
| wasm-bindgen-cli | `compile_rust.sh` | Auto-install (safe) | Auto-install (safe) |
| binaryen | `compile_rust.sh` | GitHub releases download | Error with instructions |
| gcc-aarch64 cross | `compile_rust.sh` | Auto-install | Error with instructions |
| GTK + ninja | Makefile | Auto-install | Error with instructions |
| build.json reads | `compile_rust.sh`, `release.sh` | Pure bash `sed`/`grep` | Same |

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
   and the VM-only source-parity guards (wire sync, worker-verb sync,
   dumb-edges) — they read repo SOURCE to assert sync, not fixtures.
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
│   ├── slow_source.dart            Parks the engine on a read (cancel tests)
│   ├── test_source_sink.dart       TestSource + TestSink (O(1) guards)
│   ├── timeouts.dart               t() — per-test timeout helper
│   └── streaming_guard_test.dart   Tests the guards themselves
│
├── ops/
│   ├── core/                       7 shared batteries (*_battery.dart —
│   │                               register-only, no main; runners call them)
│   ├── stress/                     6 stress batteries (1000-page)
│   ├── platform/                   guarantees only ONE platform can break
│   │   ├── native/                 finalizer battery + dispose-exit payload
│   │   └── web/                    config inheritance, clean boot failure,
│   │                               dead-session reclamation, zero OPFS residue
│   └── runners/                    4 target runners (the ONLY entry points)
│       ├── native_runner_test.dart
│       ├── web_opfs_runner_test.dart
│       ├── web_jspi_runner_test.dart
│       └── web_atomics_runner_test.dart
│
├── bridge/                         Unit tests (no engine)
│   ├── bridge_contract_test.dart
│   ├── shared_bridge_test.dart
│   └── protocol/
│       ├── binary_codec_test.dart
│       ├── codec_test.dart
│       └── wire_sync_test.dart     ← parity guard (see below)
│
├── runtime/                        Lane runtime unit tests
│   ├── router_test.dart            ← shared brain vs fake lanes:
│   │                                  placement, pinning, dispose,
│   │                                  cancel binding (incl. the
│   │                                  cancel-before-submit AND
│   │                                  dispose-before-submit races)
│   ├── wire_peek_test.dart         ← peeks + response builders
│   ├── dumb_edges_test.dart        ← dumb-edge guard on all 4 adapter files
│   ├── native/channel_buffers_test.dart  ← Dart↔Rust layout parity
│   └── web/lane_worker_sync_test.dart    ← Dart↔worker protocol parity
│
├── io/                            MemorySource/MemorySink (pure, any
│                                   platform) + FileSource/FileSink
│                                   (native dart:io — VM-only tests)
│
└── types/                          errors, page_info, pdf_rect +
                                    pdf_task (zone physics) + cancel_hook
```

### The parity guards

Four guards stop a cross-boundary contract from drifting silently. The
first three parse the OTHER side's source from disk — zero hardcoded
allowlists, so neither language can fall out of sync; the fourth keeps
our own platform adapters honest:

- **`bridge/protocol/wire_sync_test.dart`** — every `EngineOp` has a
  Rust match arm; every Rust case maps back; sub-dispatches stay
  disjoint and never leak into top-level.
- **`runtime/web/lane_worker_sync_test.dart`** — the worker handles
  every Dart→worker tag, posts only known worker→Dart tags, uses
  only injected protocol codes, calls the lane WASM surface, and
  contains no routing logic — shared brain, dumb edges.
- **`runtime/native/channel_buffers_test.dart`** — Dart's channel
  layout constants match Rust's `shared_buffer.rs` byte-for-byte.
- **`runtime/dumb_edges_test.dart`** — the four platform adapter
  files make no routing decisions (no op literals, no peeks).

Adding a Dart op without a Rust handler → test fails. Adding routing
logic to any dumb edge → test fails.

---

## Upstream patches

### pdf_oxide — patched upstream files

Every modification carries a `── pdf_manipulator patch ──` boundary
pair. The marker grep in `UPDATING.md` is the authoritative inventory;
this table summarizes what each patched file carries and why.

| File | What the patches carry |
|---|---|
| `Cargo.toml` | `native-bridge` deps + `office_oxide` as path dependency |
| `lib.rs` | `pub mod host;` |
| `document.rs` | External reader variant + `from_external_reader()` (O(1)-memory open), info/encryption/permissions accessors, `collect_refs_of()` (zero-clone GC), streaming `to_docx/pptx/xlsx_writer_flow()`, scan-all `/Subtype` Form detection |
| `editor/document_editor.rs` | State accessors, `merge_from_reader()` + shared merge core, zero-clone GC BFS + `stage_trimmed_pages_for_gc()`, `write_full_to_writer(PositionedWrite)` (function-wide streaming offsets), per-save page-ref cache, scoped destructive erase (`erase_regions_destructive`), appearance generation for AP-less annotation types, §12.5.5 appearance placement, `add_page_annotation()`, `all_media_boxes()` |
| `encryption/mod.rs` + `encryption/algorithms.rs` | Raw file key exposed to the writer (`file_key`, `build_with_key`) so streams encrypt with the key the dict advertises; PDF 2.0 Algorithm 10 (`/Perms` for R6) |
| `compliance/converter.rs` | Expose `convert_with_editor`, bundled 12 Liberation fonts (WASM has no system fonts) |
| `writer/pdf_writer.rs` | `finish_to_writer(PositionedWrite)` — streaming save |
| `writer/document_builder.rs` | `build_to_writer(PositionedWrite)` + `assemble_writer()` |
| `writer/stamp.rs` + `writer/appearance_stream.rs` + `writer/annotation_builder.rs` | Rubber-stamp normal appearance (label + inline font + text escaping) and the appearance-generation dispatch |
| `converters/office/mod.rs` | Finalize-callback core refactor for streaming, `convert_X_reader_to_writer()`, `ir_to_pdf_writer()`, fallback-font loader visibility |
| `text/word_boundary.rs` | Geometric-gap threshold contract documentation |

### office_oxide — streaming OPC writer

Branch `office_kit/0.1.2-patches`. Marked with `── office_kit patch ──`.

| File | Patch |
|---|---|
| `core/opc.rs` | `OpcWriter<W: Write>` with `ZipWriter::new_stream` (streaming ZIP, no Seek) |
| `core/editable.rs` | `write_to` uses streaming ZIP |
| 10 other files | `Write + Seek` → `Write` bounds throughout |

Everything in `src/host/` is entirely ours — no patch markers needed.

---

## The one-line summary

> **Four layers, two rules. One Router (the shared brain), dumb lane
> adapters per platform. One binary format. Three web I/O modes
> auto-detected (JSPI > Atomics > OPFS), one WASM binary. Instant
> dispose — kills, never joins. Budgets queue, never fail. O(1)
> memory on every path — enforced by test guards. Same tests, same
> pass count, all 4 platforms.**
