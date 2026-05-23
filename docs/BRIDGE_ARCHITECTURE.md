# Bridge Architecture — The Full Sledgehammer

> The complete blueprint for pdf_manipulator's native + web bridge layer.
> Three layers, two platforms, full symmetry. Thread pools, arena
> allocators, condvar-based streaming I/O, OPFS disk-backed reads,
> per-operation cancel, instant dispose. Zero full-file buffers.
> Zero memory leaks. Zero stuck threads. Zero UI jank.
>
> This doc is the BUILD SPEC. Every section maps to code that must exist.
> Nothing is optional. Nothing is "future work." If it's in this doc, it ships.
>
> **Companion doc:** [`API_GOLD.md`](API_GOLD.md) — the public API
> surface (every type, every method, every parameter). This doc
> (BRIDGE_ARCHITECTURE) defines HOW the bridge works. API_GOLD defines
> WHAT the consumer sees. Zero duplication between them.

---

## 1. The three layers

```
┌──────────────────────────────────────────────────────────────────┐
│  LAYER 1 — PUBLIC API (Dart)                                     │
│  What the consumer sees. PdfSource, PdfSink, Pdf, PdfEditor.     │
│  No FFI. No isolates. No threads. Pure Dart interfaces.          │
│  Folder: lib/src/api/                                            │
├──────────────────────────────────────────────────────────────────┤
│  LAYER 2 — BRIDGE (Dart + Rust + JS)                             │
│  Moves data between Dart and the engine. Manages isolates,       │
│  thread pools, native ports, shared memory, condvars, OPFS,      │
│  worker pools, cancel, dispose. The plumbing.                    │
│  Folder: lib/src/bridge/                                         │
├──────────────────────────────────────────────────────────────────┤
│  LAYER 3 — ENGINE (Rust)                                         │
│  pdf_oxide. Parses, renders, extracts, edits. Knows nothing      │
│  about Dart. Reads via Read+Seek. Writes via Write. Allocates    │
│  via arena. Pure Rust.                                           │
│  Folder: vendor/pdf_oxide/                                       │
└──────────────────────────────────────────────────────────────────┘
```

Layer 1 depends on nothing below it except one abstract interface (`PdfBridge`).
Layer 2 implements `PdfBridge` for each platform.
Layer 3 exposes C API (native) and WASM API (web).
No layer imports from a layer above it.

---

## 2. File layout (single source of truth)

`✓` = exists and compiles. `○` = planned, not yet created.

```
lib/
├── pdf_manipulator.dart                    ○ barrel (exports Layer 1 only)
│
├── src/
│   ├── api/                                ← LAYER 1: Public API
│   │   ├── pdf.dart                        ○ Pdf class (one-shot ops)
│   │   ├── pdf_editor.dart                 ○ PdfEditor (batch edit)
│   │   ├── pdf_builder.dart                ○ PdfBuilder (create from scratch)
│   │   ├── pdf_source.dart                 ✓ PdfSource interface
│   │   ├── pdf_sink.dart                   ✓ PdfSink interface
│   │   └── types/
│   │       ├── pdf_pages.dart              ✓ sealed PdfPages
│   │       ├── pdf_enums.dart              ✓ PdfEncryptionAlgorithm etc.
│   │       ├── pdf_params.dart             ✓ PdfSaveOptions etc.
│   │       ├── pdf_errors.dart             ✓ PdfCancelled
│   │       ├── pdf_config.dart             ✓ PdfConfig
│   │       ├── pdf_doc.dart                ✓
│   │       ├── pdf_image.dart              ✓
│   │       ├── pdf_page_info.dart          ✓
│   │       ├── pdf_rect.dart               ✓
│   │       ├── pdf_signature.dart          ✓
│   │       ├── search_result.dart          ✓
│   │       └── errors.dart                 ✓
│   │
│   ├── bridge/                             ← LAYER 2: The plumbing
│   │   ├── bridge.dart                     ✓ abstract PdfBridge interface
│   │   ├── bridge_factory.dart             ✓ conditional import router
│   │   ├── _factory_native.dart            ✓
│   │   ├── _factory_web.dart               ✓
│   │   │
│   │   ├── native/
│   │   │   ├── native_bridge.dart          ✓ ALL ops + result decoding + handle impls
│   │   │   ├── worker_entry.dart           ✓ worker isolate entry + dispatch
│   │   │   ├── worker_isolate.dart         ✓ listener creation helpers
│   │   │   ├── source_server.dart          ✓ main-isolate PdfSource fulfillment
│   │   │   ├── sink_server.dart            ✓ main-isolate PdfSink fulfillment
│   │   │   └── shared_buffer.dart          ✓ mirrors Rust layout
│   │   │
│   │   ├── protocol/                       ← SHARED: single source of truth
│   │   │   ├── op.dart                     ✓ EngineOp enum (wire names, both platforms)
│   │   │   ├── bridge_ops.dart             ✓ EngineRequest builders (web uses directly)
│   │   │   └── result.dart                 ✓ Result parsers (web uses directly)
│   │   │
│   │   ├── web/
│   │   │   └── web_bridge.dart             ✓ ALL ops via protocol + coordinator transport
│   │   │
│   │   └── ffi/
│   │       └── bridge_bindings.dart        ✓ @Native decls (camelCase + symbol:)
│   │
│   └── _internal.dart                      ○ internal barrel (bridge for api)
│
web_assets/
├── coordinator.js                          ✓ coordinator worker (pool + routing + I/O mode)
├── wasm_worker.js                          ✓ per-op WASM worker (engine + readFn by mode)
└── pdf_oxide.js / .wasm                    ✓ WASM engine

vendor/pdf_oxide/src/
├── bridge/                                 ← NATIVE engine bridge (FFI ← Dart)
│   ├── mod.rs                              ✓
│   ├── thread_pool.rs                      ✓ pool + shutdown
│   ├── callback_reader.rs                  ✓ condvar Read impl
│   ├── callback_writer.rs                  ✓ condvar Write impl
│   ├── arena.rs                            ✓ bumpalo per-op allocator
│   ├── shared_buffer.rs                    ✓ shared memory layout
│   ├── ffi_api.rs                          ✓ all submit ops → calls editor_ops
│   └── integration_test.rs                 ✓ condvar end-to-end
│
├── editor/
│   ├── document_editor.rs                  ✓ DocumentEditor core
│   └── editor_ops.rs                       ✓ SHARED: single source of truth for ALL edit ops.
│                                              Both ffi_api.rs AND wasm.rs call these functions.
│                                              Zero logic duplication between native and web.
│
├── document.rs                             ✓ PdfDocument + PdfReader (Boxed variant)
└── wasm.rs                                 ✓ WASM API + JsCallbackReader/Writer + all bindings
                                               Edit methods call editor_ops (same as native)

test/
├── helpers/
│   ├── pdf_fixtures.dart                   ✓ minimal PDF bytes
│   ├── test_source_sink.dart               ✓ TestSource + TestSink
│   └── asset_server.dart                   ✓ shelf server for web tests (CORS + COOP/COEP)
│
├── bridge/
│   ├── shared_tests.dart                   ✓ SHARED test functions — BOTH native + web call these.
│   │                                          registerSharedBridgeTests(getBridge) — 16 tests:
│   │                                          open, merge, structural (rotate, flatten, compress,
│   │                                          delete), extraction, search, render streaming.
│   │
│   ├── native/                             ← @TestOn('!browser')
│   │   ├── shared_native_test.dart         ✓ runs shared_tests through NativeBridge (16 tests)
│   │   ├── open_e2e_test.dart              ✓ NativeBridge.open
│   │   ├── open_test.dart                  ✓ additional open tests
│   │   ├── merge_e2e_test.dart             ✓ merge
│   │   ├── structural_e2e_test.dart        ✓ delete, rotate, flatten, compress
│   │   ├── extract_e2e_test.dart           ✓ text extraction
│   │   ├── stream_e2e_test.dart            ✓ render + extractImages streaming
│   │   ├── editor_e2e_test.dart            ✓ editor lifecycle
│   │   ├── error_e2e_test.dart             ✓ error handling
│   │   ├── dispose_e2e_test.dart           ✓ dispose lifecycle
│   │   └── timeout_e2e_test.dart           ✓ slow PdfSource timeout
│   │
│   ├── web/                                ← @TestOn('browser') — runs in Chrome
│   │   ├── shared_web_test.dart            ✓ runs shared_tests through WebBridge (16 tests)
│   │   ├── open_test.dart                  ✓ OPFS pipeline (direct coordinator, 6 tests)
│   │   ├── atomics_test.dart               ✓ Atomics mode readAt chain (chrome-coi, 6 tests)
│   │   └── web_test_helper.dart            ✓ fetchAsBlobUrl helper
│   │
│   └── protocol/                           ← pure Dart unit tests (no platform deps)
│       ├── op_test.dart                    ✓ EngineOp enum + helpers (43 ops)
│       ├── bridge_ops_test.dart            ✓ EngineRequest builders
│       ├── result_test.dart                ✓ result parsers
│       └── wire_sync_test.dart             ✓ JS↔Dart op name cross-verification (@TestOn('vm'))
│
├── api/                                    ← Layer 1 tests
│   ├── pdf_test.dart                       ○ Pdf class (one-shot ops)
│   ├── editor_test.dart                    ○ PdfEditor lifecycle
│   ├── builder_test.dart                   ○ PdfBuilder
│   └── types_test.dart                     ○ PdfPages, enums, params
│
└── fixtures/
    ├── minimal.pdf                         ○ 1-page A4 blank
    ├── multi_page.pdf                      ○ 3 pages with text
    ├── encrypted.pdf                       ○ AES-256 encrypted
    └── with_images.pdf                     ○ 2 embedded JPEG images

vendor/pdf_oxide/src/bridge/                ← Rust tests (inline, run via cargo test)
├── thread_pool.rs::tests                   ✓ 4 tests
├── shared_buffer.rs::tests                 ✓ 4 tests
├── callback_reader.rs::tests               ✓ 3 tests
├── callback_writer.rs::tests               ✓ 4 tests
├── arena.rs::tests                         ✓ 5 tests
├── ffi_api.rs::tests                       ✓ 4 tests
└── integration_test.rs::tests              ✓ 3 tests (condvar end-to-end)
                                            Total: 27 Rust tests
```

---

## 3. The PdfBridge interface — the seam between Layer 1 and Layer 2

`PdfBridge` is an abstract class that mirrors every method on `Pdf`,
`PdfEditor`, and `PdfBuilder`. Layer 1 forwards calls to it. Layer 2
implements it for each platform. The full method list with every type
and parameter lives in [`API_GOLD.md`](API_GOLD.md) — this doc does
not duplicate it.

The contract:
- Every method on `Pdf` maps 1:1 to a method on `PdfBridge`
- `PdfBridge` uses the same types (`PdfSource`, `PdfSink`, `PdfPages`,
  `PdfSaveOptions`, etc.) as the public API
- `NativeBridge` and `WebBridge` are the two implementations
- `bridge_factory.dart` uses conditional import to create the right one
- Layer 1 never knows which bridge it has

---

## 3.1 The shared protocol — `protocol/` + `editor_ops.rs` (symmetry guarantee)

Symmetry is guaranteed at TWO levels:

1. **Dart protocol** (`lib/src/bridge/protocol/`) — shared op names, arg builders, result parsers between NativeBridge and WebBridge.
2. **Rust editor_ops** (`vendor/pdf_oxide/src/editor/editor_ops.rs`) — shared edit operation logic between native FFI (`ffi_api.rs`) and WASM (`wasm.rs`). BOTH call the SAME functions for delete, extract, move, rotate, flatten, compress, watermark, stamp, etc. Zero logic duplication at the engine level.

### What's shared (Dart code, both platforms):

| File | Purpose | Who uses it |
|---|---|---|
| `op.dart` | `EngineOp` enum — every op name as a typed value with `.wire` string | Both bridges for op names. JS `wasm_worker.js` receives the same `.wire` strings. |
| `bridge_ops.dart` | `EngineRequest` builders — `mergeOp()`, `extractPagesOp()`, etc. | Web bridge calls directly. (Native can't use these because its args are binary-encoded Uint8List for FFI, not Map.) |
| `result.dart` | Result parsers — `parseOpenResult()`, `parseSearchResults()`, etc. | Web bridge calls directly. (Native parses binary Uint8List from FFI, not Map.) |
| `op.dart` helpers | `resolvePageIndices()`, `encodeRegions()`, `encodeWatermarkArgs()`, `encodeRectArgs()`, `encodeSaveArgs()` | Both bridges. |

### What CANNOT be shared (transport differs):

| Concern | Native | Web | Why different |
|---|---|---|---|
| Arg encoding | Binary `Uint8List` (little-endian ints, floats) | `Map<String, Object?>` (JSON-friendly for postMessage) | FFI needs raw bytes; JS needs objects |
| Result encoding | Binary `Uint8List` from `allo-isolate` | `Map<String, Object?>` from postMessage | Same reason |
| Read fulfillment | `SourceServer` + `SendPort` + condvar dance | `_handleReadAt` + postMessage chain | Isolate vs Worker communication model |
| Write delivery | `SinkServer` + `SendPort` + condvar dance | `_sinks[opId]` + chunk messages | Same reason |

### The symmetry it guarantees:

1. **Op names** — EVERY op sent by native uses `EngineOp.*.wire`. EVERY op sent by web uses `EngineOp.*.wire` (via protocol builders). Zero hand-written string op names on either side. Rename the enum value = compiler breaks both. `wire_sync_test.dart` verifies JS wasm_worker.js cases match.
2. **Individual op dispatch** — native's worker_entry.dart and web's wasm_worker.js both dispatch by individual op name strings (e.g. `'extract'`, `'search'`, `'render'`, `'getSignatures'`). No multiplexed `'read'` + opCode on either side. The worker_entry maps op name → FFI opCode internally.
3. **Streaming ops** — native sends `EngineOp.render.wire` / `EngineOp.extractImages.wire` as top-level ops to the worker. Web sends the same. The worker_entry maps them to stream opCodes 1/2 for the FFI.
4. **Editor sub-ops** — both native and web dispatch `editorGetMetadata`, `editorPageMediaBox`, `editorExtractPages`, `editorMergeFrom`, `editorSave`, `editorDispose` as individual ops. Worker_entry has cases for all of them.
5. **Builder sub-ops** — both dispatch `builderPageDone` as a dedicated op (not folded into `builderPageOp`).
6. **Result parsing** — web uses shared parsers from `result.dart`. Native parses binary from FFI but produces the same Dart types.
7. **JS wasm_worker.js sync** — `wire_sync_test.dart` (62 tests) reads wasm_worker.js from disk and verifies every EngineOp has a matching JS case, and no orphan JS cases exist.
8. **Rust editor_ops** — `editor_ops.rs` is the SINGLE source of edit logic. `ffi_api.rs` calls `editor_ops::delete_pages()`. `wasm.rs` calls `editor_ops::delete_pages()`. Same function, same behavior. Adding an op to `editor_ops` without wiring in both shells = the op is unavailable, not wrong.
9. **Shared tests** — `shared_tests.dart` defines test functions called by BOTH `shared_native_test.dart` (through NativeBridge) and `shared_web_test.dart` (through WebBridge). Same assertions, both platforms. Any drift = test fails on one platform.

---

## 4. Native bridge — the full flow

### 4.1 Architecture overview

```
┌─────────────────────────────────────────────────────────────┐
│ MAIN ISOLATE (Flutter UI thread)                            │
│                                                             │
│  Consumer's PdfSource + PdfSink live here.                  │
│  SourceServer listens for read requests.                    │
│  SinkServer listens for write chunks.                       │
│  UI keeps drawing. Never blocks.                            │
└────────────────────┬────────────────────────────────────────┘
                     │ SendPort messages (Dart-to-Dart, async)
                     │
┌────────────────────▼────────────────────────────────────────┐
│ WORKER ISOLATE (Dart, one per Pdf() instance)               │
│                                                             │
│  Receives WorkerMsg from main.                              │
│  Allocates shared buffers (calloc).                         │
│  Creates NativeCallable.listener callbacks (any-thread).    │
│  Calls Rust FFI to submit work to the thread pool.          │
│  Listener fires here when Rust needs bytes or sends chunks. │
│  Never blocks. Event loop always running.                   │
└────────────────────┬────────────────────────────────────────┘
                     │ shared memory + condvar + function pointers
                     │ (raw C, no Dart VM, any thread can touch)
                     │
┌────────────────────▼────────────────────────────────────────┐
│ RUST THREAD POOL (raw pthreads, NOT Dart isolate threads)   │
│                                                             │
│  Fixed size: max(2, available_parallelism() / 2).           │
│  Each thread runs PDF engine operations.                    │
│  Reads via CallbackReader (condvar + listener).             │
│  Writes via CallbackWriter (condvar + listener).            │
│  Allocates via bumpalo arena (per-operation).               │
│  Posts results via Dart_PostCObject to native port.         │
│  Can block freely. Not a UI thread. Not an isolate.         │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 How a read works (CallbackReader)

The engine runs on a pool thread. It calls `read(buf)`. The bytes are
on the main isolate (consumer's PdfSource). Two threads cooperate:

```
POOL THREAD (chef)                   WORKER ISOLATE (waiter)
    │                                     │
    │  1. Lock mutex                      │
    │  2. Write to shared buffer:         │
    │     offset=500, count=4096          │
    │  3. Call listener fn ptr            │
    │     (NativeCallable.listener —      │
    │      can be called from any thread) │
    │     ──── message to isolate ───►    │
    │  4. pthread_cond_wait               │
    │     (sleep on condvar)              │
    │     ...                             │  5. Listener fires on event loop
    │     ...                             │     Read shared buffer: offset=500, count=4096
    │     ...                             │     Send to main isolate's SourceServer
    │     ...                             │
    │     ...                             │  6. SourceServer calls PdfSource.readAt(500, 4096)
    │     ...                             │     Gets bytes back
    │     ...                             │
    │     ...                             │  7. Write bytes to shared buffer
    │     ...                             │     Set ready flag
    │     ...                             │     Lock mutex
    │     ...                             │     pthread_cond_signal
    │     ...                             │     Unlock mutex
    │                                     │
    │  8. Wake up                         │
    │  9. Read bytes from shared buffer   │
    │  10. Unlock mutex                   │
    │  11. Copy to engine's buf           │
    │  12. Return byte count to engine    │
```

The pool thread can sleep because it's NOT the isolate thread.
The isolate thread keeps running because nobody asked it to sleep.
The condvar is the handshake.

### 4.3 How a write works (CallbackWriter)

Same dance, opposite direction. Engine produces a chunk, sends it to
the consumer's PdfSink via the worker isolate:

```
POOL THREAD                          WORKER ISOLATE
    │                                     │
    │  1. Lock mutex                      │
    │  2. Write chunk to shared buffer    │
    │  3. Call write listener fn ptr      │
    │     ──── message to isolate ───►    │
    │  4. pthread_cond_wait               │
    │     ...                             │  5. Listener fires
    │     ...                             │     Read chunk from shared buffer
    │     ...                             │     Send to main isolate's SinkServer
    │     ...                             │
    │     ...                             │  6. SinkServer calls PdfSink.write(chunk)
    │     ...                             │     Gets ack
    │     ...                             │
    │     ...                             │  7. Set ack flag in shared buffer
    │     ...                             │     pthread_cond_signal
    │                                     │
    │  8. Wake up. Continue writing.      │
```

Every chunk crosses: engine → shared memory → isolate → main → PdfSink.
No full-file buffer anywhere. The engine writes as it produces.

### 4.4 Shared buffer layout

One contiguous native allocation per read-channel and per write-channel:

```
READ SHARED BUFFER (allocated by Dart via calloc):
┌──────────────────────────────────────────────────────┐
│ Offset: 0    request_offset    int64                 │
│ Offset: 8    request_count     int64                 │
│ Offset: 16   response_length   int64                 │
│ Offset: 24   flags             int32                 │
│              bit 0: ready (response available)       │
│              bit 1: error (read failed)              │
│              bit 2: cancelled (operation cancelled)  │
│ Offset: 28   _padding          4 bytes               │
│ Offset: 32   mutex             64 bytes              │
│ Offset: 96   condvar           64 bytes              │
│ Offset: 160  response_data     N bytes (max chunk)   │
│              (64KB for reads, 256KB for writes)      │
└──────────────────────────────────────────────────────┘
Total: 160 + chunk_size bytes per channel
```

Both sides (pool thread + isolate) agree on this layout.
The mutex protects concurrent access. The condvar is the sleep/wake mechanism.

### 4.5 Thread pool (Rust side)

```rust
pub struct ThreadPool {
    workers: Vec<JoinHandle<()>>,
    sender: crossbeam_channel::Sender<Task>,
    // Pool size determined at creation: max(2, available_parallelism() / 2)
}

pub struct Task {
    work: Box<dyn FnOnce(&Bump) + Send>,  // runs in a bumpalo arena
    cancel: Arc<AtomicBool>,               // cooperative cancellation
    result_port: i64,                      // Dart_PostCObject target
}

impl ThreadPool {
    pub fn new() -> Self {
        let size = std::thread::available_parallelism()
            .map(|n| n.get() / 2)
            .unwrap_or(2)
            .max(2);
        // spawn `size` worker threads, each loops on receiver.recv()
    }

    pub fn submit(&self, task: Task) { self.sender.send(task); }

    pub fn shutdown(&self) {
        // drop sender → all workers see disconnected channel → exit loop
    }
}
```

Each worker thread:
```rust
loop {
    match receiver.recv() {
        Ok(task) => {
            let arena = Bump::new();       // per-operation arena
            (task.work)(&arena);           // run the operation
            drop(arena);                   // ALL memory freed
        }
        Err(_) => break,                   // channel closed → pool shutting down
    }
}
```

### 4.6 Arena allocator (per-operation memory sandbox)

Every operation gets a fresh `bumpalo::Bump`. The engine allocates
into it. When the operation finishes (success, error, or cancel),
the arena is dropped. ALL memory freed in one shot.

```
Normal completion:
  arena created → engine runs → results extracted → arena dropped

Error:
  arena created → engine fails → arena dropped (no results to extract)

Cancel (cooperative):
  arena created → engine running → cancel flag set → engine sees flag →
  engine returns Err → arena dropped

Cancel (force kill — pthread_cancel):
  arena created → engine running → thread killed →
  arena is on the thread's stack → stack unwound → arena dropped
  (bumpalo's Drop impl frees the chunks)
```

No manual cleanup. No "free every allocation." The arena is the sandbox.
Drop the sandbox = drop everything inside it.

### 4.7 Result posting — `allo-isolate` (Rust → Dart)

The pool thread runs the engine. When the operation finishes (success
or error), the result must cross back to Dart. The mechanism:
`allo-isolate` — the battle-tested Rust crate used by `flutter_rust_bridge`
for exactly this purpose.

`allo-isolate` provides:
- `Isolate::new(port_id).post(value)` — posts any `IntoDart` value to a
  Dart `ReceivePort` via `Dart_PostCObject`. Thread-safe, callable from
  any thread including pool pthreads.
- `IntoDart` trait — auto-converts Rust types to `Dart_CObject`:
  `i64` → Dart `int`, `String` → Dart `String`, `bool` → Dart `bool`,
  `Vec<T>` → Dart `List`, `Vec<u8>` with `ZeroCopyBuffer` → Dart
  `Uint8List` (zero-copy, no memcpy).
- `store_dart_post_cobject` — stores the `Dart_PostCObject` function
  pointer once at init (called from Dart's `NativeApi.postCObject`).

**Why `allo-isolate` and not raw `Dart_CObject`:**
- Memory-safe (the crate manages `CObject` allocation and deallocation).
- Zero-copy for byte payloads (`ZeroCopyBuffer<Vec<u8>>`).
- Battle-tested — 7 years, used by `flutter_rust_bridge` in production.
- Thread-safe `Isolate::new(port).post(value)` callable from any thread.

**Result encoding:**

Each operation encodes its result as a `Vec<u8>` with a fixed binary
layout, posted via `ZeroCopyBuffer` (zero-copy transfer to Dart).
Dart decodes with `ByteData`. The layout is defined once per operation
type — Rust encoder in `ffi_api.rs`, Dart decoder in `native_bridge.dart`.

```
open result:
  [0]     u8    status (1=success, 0=error)
  [1..5]  i32   page_count
  [5]     u8    version_major
  [6]     u8    version_minor
  [7]     u8    is_encrypted (0/1)
  [8]     u8    is_tagged (0/1)
  [9..]   per page (page_count times):
            f64   width (8 bytes, little-endian)
            f64   height (8 bytes)
            i32   rotation (4 bytes)
            = 20 bytes per page
  after pages:
            u16   title_len + title_bytes (UTF-8)
            u16   author_len + author_bytes
            u16   subject_len + subject_bytes
            u16   keywords_len + keywords_bytes

error result:
  [0]     u8    status (0=error)
  [1..5]  i32   error_code
  [5..7]  u16   message_len + message_bytes (UTF-8)

merge/structural ops:
  [0]     u8    status (1=success, 0=error)
  (output went through CallbackWriter — no payload here)

extract:
  [0]     u8    status
  [1..3]  u16   text_len + text_bytes (UTF-8)

render (posted per page):
  [0]     u8    status
  [1..5]  i32   width
  [5..9]  i32   height
  [9..]   RGBA pixel bytes (width × height × 4)
```

On the Dart side:
```dart
final bytes = data as Uint8List;
final bd = ByteData.sublistView(bytes);
final status = bd.getUint8(0);
if (status == 0) throw _decodeError(bd);
final pageCount = bd.getInt32(1, Endian.little);
// ...
```

**Key Rust dependency:**
```toml
[dependencies]
allo-isolate = "0.1"
```

### 4.8 Per-item streaming (extractImages, render)

The worker sends items one at a time via `WorkerStreamItem`:

```
POOL THREAD                          WORKER ISOLATE               MAIN ISOLATE
    │                                     │                            │
    │  Engine extracts image 1            │                            │
    │  Posts WorkerStreamItem(image1)     │                            │
    │  via Dart_PostCObject ─────────►    │                            │
    │                                     │  Receives image1           │
    │  Engine extracts image 2            │  Sends to main ──────►     │
    │  Posts WorkerStreamItem(image2)     │                            │  StreamController.add(image1)
    │  ─────────────────────────────►     │                            │  Consumer processes image1
    │                                     │  Receives image2           │  image1 GC eligible
    │  Engine done                        │  Sends to main ──────►     │
    │  Posts WorkerStreamItem(done:true)  │                            │  StreamController.add(image2)
    │  ─────────────────────────────►     │                            │
    │                                     │  Closes stream             │  StreamController.close()
```

One image in memory at a time. The consumer's `await for` loop
processes one, then the next arrives. No list accumulation.

### 4.9 Dispose (instant kill)

```
Consumer calls pdf.dispose():

MAIN ISOLATE:
  bridge.dispose()

WORKER ISOLATE:
  1. Set cancelled=true on ALL active shared buffers
  2. Signal ALL condvars (wake sleeping pool threads)
  3. Send shutdown to thread pool
  4. Wait 100ms for cooperative exit

POOL THREADS:
  Each sleeping thread wakes up
  Checks cancelled flag → true
  Returns Err to engine
  Engine stops
  Arena dropped → all engine memory freed
  Thread exits loop → joins

WORKER ISOLATE:
  5. If any thread still alive after 100ms:
     pthread_cancel (force kill)
     Arena still on stack → Drop runs → memory freed
  6. Free all shared buffers (calloc.free)
  7. Kill worker isolate (Isolate.kill)
  8. Close all ReceivePorts

Result:
  Zero threads. Zero native memory. Zero handles. Instant.
```

### 4.10 Timeout on reads

If the consumer's PdfSource.readAt takes forever (slow network, hung file):

```rust
// In CallbackReader::read():
let result = pthread_cond_timedwait(condvar, mutex, 30_seconds);
if result == ETIMEDOUT {
    return Err(io::Error::new(TimedOut, "read timed out after 30s"));
}
```

The engine sees the timeout error, propagates it, the operation fails
with a clean error. No stuck thread.

---

## 5. Web bridge — first-class citizen, three I/O modes

The web bridge is NOT a degraded version of native. It's a full
implementation with the same architecture: pool of workers, coordinator
off the main thread, on-demand reads, chunked output, per-item
streaming, instant cancel, instant dispose.

The one difference between web and native: the mechanism for blocking
the engine thread while waiting for bytes from PdfSource. Native uses
pthreads + condvar. Web has three options, detected at startup,
falling through automatically:

```
JSPI available?        → Mode 1: JSPI (best — engine pauses on Promise)
SharedArrayBuffer?     → Mode 2: Atomics (condvar equivalent)
Neither?               → Mode 3: OPFS (pre-copy to disk, then local reads)
```

Detection happens ONCE at startup. No per-operation branching.

### 5.0 The three-level architecture (symmetric with native)

Native has three levels:
```
Main isolate (UI) ←→ Worker isolate (coordinator) ←→ Rust thread pool
```

Web has the same three levels:
```
Main thread (UI) ←→ Coordinator Worker (JS) ←→ WASM Worker pool
```

The **coordinator worker** is a dedicated Web Worker that sits between
the main thread and the WASM workers. It manages:
- OPFS file lifecycle (write, read, cleanup)
- I/O mode negotiation (JSPI / Atomics / OPFS)
- Worker pool management (acquire, release, cancel, dispose)
- Read fulfillment: receives "need bytes" from WASM workers, asks main
  thread for bytes, delivers them back
- Write forwarding: receives output chunks from WASM workers, forwards
  to main thread for PdfSink delivery
- Stream item routing: receives per-item data from WASM workers,
  forwards to main thread for StreamController

The main thread's job is minimal:
- Answer `readAt(offset, count)` requests (via postMessage from coordinator)
- Accept `sink.write(chunk)` deliveries (via postMessage from coordinator)
- Send operations and receive results
- UI keeps drawing. Never blocks. Never processes I/O coordination.

```
┌─────────────────────────────────────────────────────────────┐
│ MAIN THREAD (Dart, Flutter UI)                              │
│                                                             │
│  Consumer's PdfSource + PdfSink live here.                  │
│  Answers readAt requests from coordinator.                  │
│  Accepts write chunks from coordinator.                     │
│  UI keeps drawing. Never blocks. Minimal I/O involvement.   │
└────────────────────┬────────────────────────────────────────┘
                     │ postMessage (only readAt + write + results)
                     │
┌────────────────────▼────────────────────────────────────────┐
│ COORDINATOR WORKER (JS, one per Pdf() instance)             │
│                                                             │
│  Manages the WASM worker pool.                              │
│  Detects I/O mode at startup (JSPI / Atomics / OPFS).       │
│  Routes read requests: WASM worker → main → WASM worker.    │
│  Routes write chunks: WASM worker → main.                   │
│  Routes stream items: WASM worker → main.                   │
│  Manages OPFS files (mode 3 only).                          │
│  Never runs WASM. Pure JS coordination.                     │
│  Cancel = forward terminate to WASM worker.                 │
│  Dispose = terminate all WASM workers + cleanup.            │
└────────────────────┬────────────────────────────────────────┘
                     │ postMessage / SharedArrayBuffer / JSPI
                     │ (depends on detected I/O mode)
                     │
┌────────────────────▼────────────────────────────────────────┐
│ WASM WORKER POOL (JS + WASM, multiple workers)              │
│                                                             │
│  Fixed size: max(2, navigator.hardwareConcurrency / 2).     │
│  Each worker: own WASM instance.                            │
│  Runs the PDF engine. Calls readFn / writeFn.               │
│  readFn behavior depends on I/O mode (set at init).         │
│  Cancel: worker.terminate() — WASM memory freed instantly.  │
└─────────────────────────────────────────────────────────────┘
```

Why the coordinator worker matters:
- **Without it:** every readAt request and every output chunk fires a
  postMessage handler on the main thread's event loop, interleaved with
  widget builds and animations. 500 reads × 0.1ms = 50ms of jank.
- **With it:** the main thread only answers readAt calls (fast, async)
  and accepts write chunks (fast, async). All the coordination — OPFS
  management, pool management, mode detection, routing — happens on the
  coordinator's thread. Same protection as native's worker isolate.

### 5.1 I/O mode detection

At coordinator startup, detect the best available mode:

```javascript
// coordinator_worker.js — runs once at init
function detectIoMode() {
  if (typeof WebAssembly.Suspending !== 'undefined') return 'jspi';
  if (typeof SharedArrayBuffer !== 'undefined')       return 'atomics';
  return 'opfs';
}
```

The mode is sent to each WASM worker at spawn time. The WASM worker
creates its `readFn` based on the mode. All operations use the same
mode. No per-operation branching.

### 5.2 Mode 1: JSPI — engine pauses on Promise (best)

```
WASM WORKER                          COORDINATOR              MAIN THREAD
    │                                     │                        │
    │  Engine calls readFn(off, cnt)      │                        │
    │  readFn is an async JS function     │                        │
    │  → returns a Promise                │                        │
    │  Browser PAUSES WASM stack          │                        │
    │  ...                                │                        │
    │  readFn posts to coordinator:       │                        │
    │  "need bytes off=500 cnt=4096"      │                        │
    │  ────────────────────────────►      │                        │
    │                                     │  Forwards to main:     │
    │                                     │  "readAt(500, 4096)"   │
    │                                     │  ────────────────►     │
    │                                     │                        │
    │                                     │                        │  source.readAt(500, 4096)
    │                                     │                        │  Gets bytes
    │                                     │                        │
    │                                     │  ◄────────────────     │  bytes
    │                                     │  Forwards to WASM:     │
    │  ◄────────────────────────────      │  bytes                 │
    │  Promise resolves                   │                        │
    │  Browser RESUMES WASM stack         │                        │
    │  readFn returns bytes               │                        │
    │  Engine continues                   │                        │
```

Zero pre-copy. Zero blocking. Zero shared memory. Zero headers.
The browser handles WASM suspension/resumption natively.

**WASM binding required:** `#[wasm_bindgen(jspi)]` attribute on the
imported `readFn`. wasm-bindgen wraps it with `WebAssembly.Suspending`.
The export is wrapped with `WebAssembly.promising`. Rust code sees a
synchronous `readFn` call; JSPI handles the async pause/resume.

**Browser support (May 2026):** Chrome 137+ ✓, Firefox 139+ ✓ (flag),
Safari ✗ (assigned, no date).

### 5.3 Mode 2: SharedArrayBuffer + Atomics (fallback 1)

```
WASM WORKER                          COORDINATOR              MAIN THREAD
    │                                     │                        │
    │  readFn called by engine            │                        │
    │  Writes request to SAB:             │                        │
    │    offset=500, count=4096           │                        │
    │  Posts to coordinator:              │                        │
    │    "need bytes"                     │                        │
    │  ────────────────────────────►      │                        │
    │  Atomics.wait(sab, idx, 0)          │                        │
    │  BLOCKS. Worker sleeps.             │                        │
    │  ...                                │  Forwards to main      │
    │  ...                                │  ────────────────►     │
    │  ...                                │                        │  source.readAt
    │  ...                                │                        │  Gets bytes
    │  ...                                │  ◄────────────────     │  bytes
    │  ...                                │  Writes to SAB         │
    │  ...                                │  Atomics.notify        │
    │                                     │  ────notify───►        │
    │  Wakes up                           │                        │
    │  Reads bytes from SAB               │                        │
    │  Returns to engine                  │                        │
```

Same as native's condvar pattern. `Atomics.wait` = `pthread_cond_wait`.
`Atomics.notify` = `pthread_cond_signal`. `SharedArrayBuffer` = shared
memory via `calloc`.

**Requires headers on consumer's server:**
```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

**Browser support:** all browsers that support SharedArrayBuffer
(Chrome 68+, Firefox 79+, Safari 15.2+). Universal when headers set.

### 5.4 Mode 3: OPFS (fallback 2 — universal, pre-copies file)

```
MAIN THREAD                          COORDINATOR              WASM WORKER
    │                                     │                        │
    │  Streams source to coordinator:     │                        │
    │  chunk1 (256KB) ──────────────►     │                        │
    │  chunk2 (256KB) ──────────────►     │  Writes to OPFS        │
    │  chunk3 (256KB) ──────────────►     │  via SyncAccessHandle  │
    │  ...finalize ─────────────────►     │  Flush                 │
    │                                     │                        │
    │  Sends op ────────────────────►     │  Sends op ────────►    │
    │                                     │                        │
    │                                     │                        │  Opens OPFS read handle
    │                                     │                        │  readFn = syncHandle.read
    │                                     │                        │  Engine reads on demand
    │                                     │                        │  FROM DISK. Not RAM.
    │                                     │                        │
    │  ◄──────────────── result ──────────│◄──── result ───────    │
```

The full file is pre-copied to OPFS disk. The engine reads from disk
synchronously. Not symmetric with native (native never pre-copies),
but works on all browsers with zero headers.

### 5.5 Output streaming (all modes — identical)

Output streaming is the same regardless of I/O mode. The WASM engine
produces chunks. Each chunk is posted back through the coordinator
to the main thread:

```
WASM WORKER                          COORDINATOR              MAIN THREAD
    │                                     │                        │
    │  Engine writes chunk 1 (4KB)        │                        │
    │  postMessage({type:'chunk'})        │                        │
    │  ────────────────────────────►      │  Forwards to main      │
    │                                     │  ────────────────►     │  sink.write(chunk1)
    │                                     │                        │
    │  Engine writes chunk 2 (100KB)      │                        │
    │  ────────────────────────────►      │  ────────────────►     │  sink.write(chunk2)
    │                                     │                        │
    │  Engine done                        │                        │
    │  postMessage({type:'done'})         │                        │
    │  ────────────────────────────►      │  ────────────────►     │  operation complete
```

Chunks are transferred (zero-copy via `Transferable`). The consumer's
PdfSink receives chunks as produced. No full output buffer.

Output streaming requires WASM bindings for `JsCallbackWriter` and
`saveToWriter`. These are needed regardless of I/O mode — all three
modes produce output the same way.

### 5.6 Per-item streaming (all modes — identical)

Same as native's `WorkerStreamItem`. One `postMessage` per image/page:

```
WASM WORKER                          COORDINATOR              MAIN THREAD
    │                                     │                        │
    │  Engine extracts image 1            │                        │
    │  postMessage({type:'item'})         │                        │
    │  ────────────────────────────►      │  ────────────────►     │  stream.add(image1)
    │                                     │                        │  consumer processes
    │  Engine extracts image 2            │                        │  image1 GC eligible
    │  ────────────────────────────►      │  ────────────────►     │  stream.add(image2)
    │                                     │                        │
    │  postMessage({type:'done'})         │                        │
    │  ────────────────────────────►      │  ────────────────►     │  stream.close()
```

One item in memory at a time. Same guarantee as native.

### 5.7 Worker pool — session-based (same as current)

Pool management is unchanged. One session = one WASM worker, start to
finish. The coordinator manages the pool. Cancel = `worker.terminate()`.
Queuing when pool is full. Pool size = `max(2, hardwareConcurrency / 2)`.

The coordinator manages the pool — main thread never touches workers
directly. Main sends commands to coordinator, coordinator dispatches
to pool workers.

### 5.8 OPFS cleanup (mode 3 only)

The `OpfsRegistry` tracks temp files. Cleanup on operation complete,
on error, and on dispose. The coordinator owns the registry and
handles all OPFS operations. Main thread never touches OPFS.

### 5.9 Cancel and dispose

**Cancel a running operation:**
- Coordinator tells the WASM worker `worker.terminate()`.
- WASM linear memory freed instantly by browser.
- WASM instance IS the arena. `terminate()` = `drop(arena)`.
- If mode 3: OPFS temp file cleaned up.

**Dispose (kill everything):**
1. Coordinator terminates ALL WASM workers in the pool.
2. Coordinator cleans all OPFS temp files (mode 3).
3. Main thread terminates the coordinator worker.
4. Done. Zero workers. Zero WASM memory. Zero OPFS files. Instant.

### 5.10 Updated file layout

```
lib/src/bridge/web/
├── web_bridge.dart           ← WebBridge implements PdfBridge
├── worker_pool.dart          ← WASM worker pool (session-based)
├── opfs.dart                 ← OPFS helpers + cleanup registry
├── io_mode.dart              ← WebIoMode enum + detection
└── coordinator_protocol.dart ← message types main ↔ coordinator

web_assets/
├── coordinator.js            ← NEW: coordinator worker (pool + routing)
├── wasm_worker.js            ← NEW: per-operation WASM worker (engine)
├── pdf_oxide.js              ← WASM glue (generated)
└── pdf_oxide_bg.wasm         ← WASM binary (compiled)

test/bridge/web/
├── asset_server.dart         ← serves worker.js + WASM for browser tests
├── jspi_open_test.dart       ← JSPI mode: open, merge, structural, etc.
├── jspi_stream_test.dart     ← JSPI mode: render, extractImages streaming
├── jspi_editor_test.dart     ← JSPI mode: editor lifecycle
├── atomics_open_test.dart    ← Atomics mode: same ops, different I/O
├── atomics_stream_test.dart  ← Atomics mode: streaming
├── opfs_open_test.dart       ← OPFS mode: same ops, pre-copy path
├── opfs_stream_test.dart     ← OPFS mode: streaming
├── dispose_test.dart         ← cancel + dispose across modes
├── mode_detection_test.dart  ← verifies correct mode selection
└── coordinator_test.dart     ← coordinator ↔ WASM worker routing
```

The old `worker.js` (836 lines, everything in one file) is replaced
by two focused workers:
- **`coordinator.js`** — manages pool, routes messages, handles OPFS,
  detects I/O mode. Pure JS. No WASM.
- **`wasm_worker.js`** — loads WASM, runs engine ops, calls readFn
  (whose implementation depends on I/O mode set at init). One per
  concurrent operation.

### 5.11 WASM bindings required for full parity

| Binding | Purpose | I/O modes that need it |
|---|---|---|
| `WasmPdfDocument.fromReader(readFn, lengthFn)` | On-demand reads | JSPI, Atomics |
| `WasmDocumentEditor.editorFromReader(readFn, lengthFn)` | Editor without full bytes | JSPI, Atomics |
| `DocumentEditor::from_document(PdfDocument)` | Constructor from opened doc | JSPI, Atomics |
| `WasmPdfDocument.saveToWriter(writeFn)` | Streaming output | All modes |
| `WasmPdfDocument.renderPageFit(page, w, h)` | Render to pixel box | All modes |
| `WasmPdf.mergeFromReaders(readerArray)` | Merge N inputs via readers | JSPI, Atomics |
| `JsCallbackWriter` struct | `Write` impl via JS function | All modes |
| `PositionTracker<JsCallbackWriter>` | `Write + Seek` for save | All modes |

For mode 3 (OPFS): `fromReader` is still used — the readFn reads from
the OPFS SyncAccessHandle. The WASM bindings are needed for ALL modes.
The only difference is what the JS readFn does internally.

---

## 6. Symmetry table — full parity, three I/O modes

| Feature | Native | Web (JSPI) | Web (Atomics) | Web (OPFS) |
|---|---|---|---|---|
| **Three-level arch** | Main isolate → Worker isolate → Pool threads | Main thread → Coordinator Worker → WASM pool | same | same |
| **Worker pool** | Rust thread pool (`available_parallelism() / 2`) | WASM Worker pool (`hardwareConcurrency / 2`) | same | same |
| **Input reads** | CallbackReader: condvar + listener (on demand) | readFn returns Promise, JSPI suspends (on demand) | readFn blocks with Atomics.wait (on demand) | readFn calls SyncAccessHandle (pre-copied to OPFS) |
| **No pre-copy** | ✓ | ✓ | ✓ | ✗ (full file to OPFS first) |
| **Output streaming** | CallbackWriter: condvar + listener | JsCallbackWriter: postMessage chunks | same | same |
| **Per-item streaming** | WorkerStreamItem via Dart_PostCObject | postMessage per item via coordinator | same | same |
| **Coordinator off main** | Worker isolate handles all I/O coordination | Coordinator Worker handles all I/O coordination | same | same |
| **Main thread job** | SourceServer (readAt) + SinkServer (write) | Answer readAt + accept write via postMessage | same | same |
| **Cancel** | Flag + condvar signal + pthread_cancel | Worker.terminate() | same | same |
| **Dispose** | Cancel all + pool shutdown + arena drop | Terminate all workers + coordinator + OPFS cleanup | same | same |
| **Memory sandbox** | bumpalo arena per operation | WASM linear memory per worker (terminate = free all) | same | same |
| **Pool sizing** | `available_parallelism() / 2` | `hardwareConcurrency / 2` | same | same |
| **Queuing** | crossbeam channel | Dart Queue in coordinator | same | same |
| **Read timeout** | pthread_cond_timedwait(30s) | JS Promise timeout | Atomics.wait timeout | OPFS local disk — N/A |
| **Special headers** | none | none | COOP + COEP required | none |
| **Browser support** | all native platforms | Chrome + Firefox | all (with headers) | all |

**Current mode fallthrough (JSPI disabled until WASM import refactor):**
```
detectIoMode():
  SharedArrayBuffer?        → Atomics (on-demand reads, needs COOP/COEP headers)
  Neither?                  → OPFS (pre-copy to disk, then local reads)
```

**Future mode fallthrough (after JSPI ground is built):**
```
detectIoMode():
  WebAssembly.Suspending?   → JSPI (on-demand reads, zero pre-copy, no headers)
  SharedArrayBuffer?        → Atomics (on-demand reads, needs headers)
  Neither?                  → OPFS (pre-copy, then local reads)
```

**Why JSPI is disabled:** JSPI wraps WASM module imports at instantiation
time. Our readFn is currently a runtime `js_sys::Function` argument, not a
module import. JSPI can't intercept runtime function calls. Enabling JSPI
requires adding a WASM import declaration in Rust + custom instantiation
in wasm_worker.js with `WebAssembly.Suspending`. Tracked as the next build item.

Detection once at startup. No per-operation branching. The `readFn`
is created once per WASM worker based on the detected mode. Everything
else (output streaming, per-item streaming, pool, cancel, dispose)
is identical across all three modes.

**The timeline — fallbacks die naturally:**
- Today: Chrome → JSPI. Firefox → Atomics or OPFS. Safari → OPFS.
- Soon: Firefox ships JSPI unflagged → JSPI. Safari ships JSPI → JSPI.
- Eventually: JSPI is the only path. Delete Atomics + OPFS code.

**WASM bindings required (all modes):**

| Binding | Purpose |
|---|---|
| `WasmPdfDocument.fromReader(readFn, lengthFn)` | On-demand reads (all modes use readFn) |
| `WasmDocumentEditor.editorFromReader(readFn, lengthFn)` | Editor without full bytes |
| `DocumentEditor::from_document(PdfDocument)` | Constructor from already-opened doc |
| `WasmPdfDocument.saveToWriter(writeFn)` | Streaming output (all modes) |
| `WasmPdfDocument.renderPageFit(page, w, h)` | Render to pixel box |
| `WasmPdf.mergeFromReaders(readerArray)` | Merge N inputs via readers |
| `JsCallbackWriter` struct | `Write` impl via JS function |
| `PositionTracker<JsCallbackWriter>` | `Write + Seek` for save |

---

## 7. The shared buffer — exact specification

### 7.1 Read channel

```
Byte offset  Field              Type      Written by     Read by
─────────────────────────────────────────────────────────────────
0            request_offset     int64     pool thread    isolate
8            request_count      int64     pool thread    isolate
16           response_length    int64     isolate        pool thread
24           flags              uint32    both           both
               bit 0: ready                isolate →     pool thread
               bit 1: error                isolate →     pool thread
               bit 2: cancelled            isolate →     pool thread
28           _pad               4 bytes
32           mutex              64 bytes  (pthread_mutex_t)
96           condvar            64 bytes  (pthread_cond_t)
160          data               65536 bytes (64KB max read chunk)
─────────────────────────────────────────────────────────────────
Total: 65696 bytes per read channel
```

### 7.2 Write channel

Same layout but data is 256KB (engine output chunks are larger):

```
Byte offset  Field              Type      Written by     Read by
─────────────────────────────────────────────────────────────────
0            chunk_length       int64     pool thread    isolate
8            flags              uint32    both           both
               bit 0: ready                pool thread → isolate
               bit 1: ack                  isolate →     pool thread
               bit 2: cancelled            isolate →     pool thread
12           _pad               4 bytes
16           mutex              64 bytes
80           condvar            64 bytes
144          data               262144 bytes (256KB max write chunk)
─────────────────────────────────────────────────────────────────
Total: 262288 bytes per write channel
```

### 7.3 Allocation

Dart allocates both buffers via `calloc`. Passes the pointers to Rust
via FFI. Rust casts the pointer to its matching struct layout.
Both sides agree on the byte offsets (defined once in Rust, mirrored
in Dart constants).

Dart frees both buffers on operation completion or dispose.

---

## 8. Cancellation protocol

### 8.1 Cooperative (clean)

```
1. Dart sets cancelled bit (bit 2) in shared buffer flags
2. Dart signals condvar (wake the sleeping pool thread)
3. Pool thread wakes up
4. Pool thread checks: flags & CANCELLED != 0
5. Pool thread returns io::Error(Interrupted) to engine
6. Engine propagates error
7. Arena dropped → all memory freed
8. Thread returns to pool
```

### 8.2 Force kill (last resort)

```
1. Cooperative cancel attempted (above)
2. Wait 100ms
3. If thread still alive:
   pthread_cancel(thread_id)
4. Thread is killed by OS
5. Stack unwound → arena's Drop runs → memory freed
   (bumpalo chunks are freed in Drop)
6. Thread is gone
```

### 8.3 Cancellation check points

The engine doesn't check the flag on every byte read. Too expensive.
The CallbackReader checks ONCE per `read()` call — that's once per
condvar wake. Between reads, the engine parses objects. A typical
PDF parse does ~100-500 reads. Each read is a check point.

If the engine is doing CPU-heavy work between reads (rendering a
complex page), cancellation is delayed until the next read. For
rendering, this means up to ~1 second delay. Acceptable — the
alternative is checking inside hot loops, which kills performance.

---

## 9. Error handling

### 9.1 Read error

```
SourceServer catches exception from PdfSource.readAt:
  → sets error bit in shared buffer
  → sets response_length = 0
  → signals condvar

Pool thread wakes up:
  → sees error bit
  → returns io::Error to engine
  → engine returns Err(...)
  → operation fails with typed PdfError on Dart side
```

### 9.2 Write error

```
SinkServer catches exception from PdfSink.write:
  → sets error bit in shared buffer (no ack bit)
  → signals condvar

Pool thread wakes up:
  → sees error bit (no ack)
  → returns io::Error to engine
  → engine returns Err(...)
  → operation fails
```

### 9.3 Worker isolate crash

If the worker isolate crashes (should never happen, but):
  → pool threads are sleeping on condvars
  → nobody signals them
  → `pthread_cond_timedwait` times out after 30s
  → threads return timeout error
  → threads exit
  → main isolate sees the worker's exit event
  → main returns error to consumer

No stuck threads. Timeout is the safety net.

### 9.4 Pool thread panic (Rust panic)

If Rust panics on a pool thread:
  → panic unwinds the stack
  → arena's Drop runs → memory freed
  → thread exits the pool loop
  → pool detects the dead thread
  → pool spawns a replacement thread
  → Dart sees no result on the native port
  → Dart's ReceivePort times out or sees a failure signal
  → operation fails with error

---

## 10. Memory budget

### 10.1 Per Pdf() instance

| Component | Memory |
|---|---|
| Worker isolate | ~2MB heap + ~1MB stack |
| Thread pool (4 threads) | 4 × 512KB stack = 2MB |
| Shared buffers (per active op) | ~66KB read + ~262KB write = ~328KB |
| Total fixed | ~5MB |

### 10.2 Per active operation

| Component | Memory |
|---|---|
| Arena (engine working set) | 1-50MB depending on PDF complexity |
| Shared buffers | ~328KB |
| Total per op | ~1-50MB |

### 10.3 The promise

A 500MB PDF merge on a 512MB RAM device:
- Main isolate: ~1MB (PdfSource callbacks, no full buffer)
- Worker isolate: ~2MB
- Pool thread: ~512KB stack + ~10MB arena (xref + page objects)
- Shared buffers: ~328KB
- **Total: ~14MB** (not 2.5GB)

---

## 11. What this doc does NOT cover

- The public API surface — see [`API_GOLD.md`](API_GOLD.md)
- The web worker.js dispatch code (implementation detail)
- The Rust engine internals (pdf_oxide's parser, renderer, etc.)

This doc covers the BRIDGE LAYER ONLY — how data moves between
the consumer's Dart code and the engine, on both platforms, with
full cancellation, cleanup, and streaming. Test file layout is in §2.
Test strategy is in §12.

---

## 12. Build order and current status

| Phase | Scope | Tests | Status |
|---|---|---|---|
| **B1** | Rust: `bridge/thread_pool.rs` — pool + queue + shutdown | 4/4 | **DONE** |
| **B2** | Rust: `bridge/shared_buffer.rs` — memory layout + accessors | 4/4 | **DONE** |
| **B3** | Rust: `bridge/callback_reader.rs` — Read+Seek via condvar | 3/3 | **DONE** |
| **B4** | Rust: `bridge/callback_writer.rs` — Write via condvar | 4/4 | **DONE** |
| **B5** | Rust: `bridge/arena.rs` — bumpalo per-operation wrapper | 5/5 | **DONE** |
| **B6** | Rust: `bridge/ffi_api.rs` — C API + result posting via allo-isolate | 4/4 | **DONE** |
| **B-int** | Rust: `bridge/integration_test.rs` — end-to-end condvar dance | 3/3 | **DONE** |
| | **Rust total** | **27/27** | |
| **B7** | Dart: `bridge/native/shared_buffer.dart` — mirrors Rust layout | compiles | **DONE** |
| **B8** | Dart: `bridge/native/source_server.dart` + `sink_server.dart` | compiles | **DONE** |
| **B9** | Dart: `bridge/native/worker_isolate.dart` — NativeCallable.listener + dispatch | compiles | **DONE** |
| **B10** | Dart: `bridge/native/native_bridge.dart` — NativeBridge skeleton | compiles | **DONE** |
| **B11** | Dart: `bridge/web/worker_pool.dart` — Web Worker pool | compiles | **DONE** |
| **B12** | Dart: `bridge/web/opfs.dart` — OPFS streaming + cleanup registry | compiles | **DONE** |
| **B13** | Dart: `bridge/web/web_bridge.dart` — WebBridge (core ops wired) | compiles | **DONE** |
| **B14** | Dart: `bridge/bridge_factory.dart` + `_factory_*.dart` | compiles | **DONE** |
| **B15** | Dart: `api/` types — PdfPages, enums, params, PdfBridge interface | compiles | **DONE** |
| **B16** | Wire `NativeBridge.open` end-to-end | 5/5 | **DONE** |
| **B16b** | Wire `NativeBridge.merge` end-to-end (read + write pipelines) | 4/4 | **DONE** |
| **B16c** | Generic `bridge_submit_edit` — one Rust FFI for all edit ops | 4/4 e2e | **DONE** |
| **B16d** | Wire native structural ops (deletePages, rotateAll, flatten, compress) | 4/4 e2e | **DONE** |
| **B16e** | FFI bindings use `symbol:` parameter — zero `// ignore:` comments | clean | **DONE** |
| **B16f** | Wire rotatePages, applyRedactions, movePage (edit ops) | compiles | **DONE** |
| **B16g** | Rust `bridge_submit_read` + `dispatch_read_op` (read-only pipeline) | compiles | **DONE** |
| **B16h** | Wire `NativeBridge.extract` (text + markdown) end-to-end | 3/3 e2e | **DONE** |
| **B16i** | Wire embedFile, eraseRegions (edit ops with params/secondaries) | compiles | **DONE** |
| **B16j** | Wire validatePdfA, validatePdfUa (read ops with result decode) | compiles | **DONE** |
| **B16k** | Wire encrypt, decrypt (edit ops with SaveOptions + encryption config) | compiles | **DONE** |
| **B16l** | Rust `dispatch_edit_op` returns `Option<SaveOptions>` for encrypted saves | compiles | **DONE** |
| **B16m** | Rust `write_to_writer_with_options` passes SaveOptions through to save | compiles | **DONE** |
| | **Native e2e total** | **22/22** | |
| | **Edit op codes wired (Rust)** | **28** (1-3, 5-28) | |
| | **Read op codes wired (Rust)** | **7** (1, 2, 3, 4, 5, 6, 7) | |
| | **Remaining UnimplementedError (native one-shot)** | **0** | |
| **B16n** | Rust edit ops 15-18 (watermark, sign, addStamp, addImageStamp) + Dart wiring | compiles | **DONE** |
| **B16o** | Rust read ops 3-5 (search, getSignatures, verifySignatures) + Dart wiring | compiles | **DONE** |
| **B16p** | Rust `bridge_submit_images_to_pdf` + Dart wiring | compiles | **DONE** |
| **B18** | Wire remaining native ops (split, splitBySize, imagesToPdf, render, extractImages) | 31/31 e2e | **DONE** |
| | **Native bridge: zero UnimplementedError (one-shot + editor)** | all ops wired | |
| | **Remaining UnimplementedError (after B18, before B19)** | **3** — NativeBridge.createBuilder, WebBridge.openEditor, WebBridge.createBuilder | |
| **B18w** | Wire web bridge ops to parity with native (0 UnimplementedError for one-shot) | all ops wired | **DONE** |
| | Web ops wired: ALL — sign, addStamp, addImageStamp, imagesToPdf, render, extractImages, getSignatures, verifySignatures, validatePdfA, validatePdfUa + worker.js renderPage, extractImages, sign, verifySignatures cases | | |
| **B17** | WASM parity + web e2e tests — see B17a-B17l below | | NOT STARTED |
| **B19** | Wire PdfEditor + PdfBuilder through PdfBridge (persistent handles) | | **DONE** |
| | **Editor (native):** | | |
| | Rust: 7 FFI functions (`bridge_editor_open/mutate/save/dispose/page_count/get_metadata/get_page_media_box`) | compiles | **DONE** |
| | Dart FFI: 7 bindings in `bridge_bindings.dart` | compiles | **DONE** |
| | Worker dispatch: `editorOpen/Mutate/Save/Dispose/PageCount/GetMetadata` cases | compiles | **DONE** |
| | `NativeBridge.openEditor` → SourceServer → worker → Rust pool → handle map | compiles | **DONE** |
| | `_NativeEditorHandle`: ALL 28 mutations + save + metadata + pageMediaBox + extractPages + mergeFrom | compiles | **DONE** |
| | **Builder (native):** | | |
| | Rust: `BUILDER_HANDLES` global map + `BuilderState` (Option\<DocumentBuilder\> + buffered ops) | compiles | **DONE** |
| | Rust: `BuilderPageOp` enum (26 variants: font, text, heading, paragraph, image, form fields, radioGroup, fieldScripts, links, footnote, columns, etc.) | compiles | **DONE** |
| | Rust: `replay_page_ops` — replays buffered ops against real `FluentPageBuilder` on save | compiles | **DONE** |
| | Rust: `bridge_builder_create`, `bridge_builder_set_metadata` (take-apply-put for consuming API) | compiles | **DONE** |
| | Rust: `bridge_builder_add_page` (A4/Letter/Custom), `bridge_builder_page_op` (26 op codes) | compiles | **DONE** |
| | Rust: `bridge_builder_save` (replay ops → `builder.build()` → `CallbackWriter`), `bridge_builder_dispose` | compiles | **DONE** |
| | Dart FFI: 6 new `@Native` bindings for builder handle functions | compiles | **DONE** |
| | Dart: `_NativeBuilderHandle` — routes metadata/addPage/save/dispose through worker | compiles | **DONE** |
| | Dart: `_NativePageBuilderHandle` — encodes all page ops as binary params + op codes | compiles | **DONE** |
| | Worker dispatch: `builderCreate/SetMetadata/AddPage/PageOp/Save/Dispose` cases | compiles | **DONE** |
| | `NativeBridge.createBuilder` — zero stubs | compiles | **DONE** |
| | **Editor + Builder (web):** | | |
| | `WebBridge.openEditor` — acquires session, streams source to OPFS, opens persistent editor on worker | compiles | **DONE** |
| | `_WebEditorHandle` — persistent session, routes ALL mutations via `editor.*` messages | compiles | **DONE** |
| | `WebBridge.createBuilder` — acquires session, creates persistent builder on worker | compiles | **DONE** |
| | `_WebBuilderHandle` — persistent session, routes metadata/addPage/save/dispose | compiles | **DONE** |
| | `_WebPageBuilderHandle` — routes all page ops via `page.*` messages | compiles | **DONE** |
| | **Remaining UnimplementedError in ALL bridge implementations** | **0** (only worker default-case safety catch) | |
| **B20** | Wire Layer 1 `Pdf`, `PdfEditor`, `PdfBuilder` to PdfBridge via bridge_factory | 7/7 e2e | **DONE** |
| | `api/pdf.dart` — all 36 methods forwarding to `_bridge` | compiles | |
| | `api/pdf_editor.dart` — all 33 methods forwarding to `BridgeEditorHandle` | compiles | |
| | `api/pdf_builder.dart` + `PdfPageBuilder` — all methods forwarding to `BridgeBuilderHandle` | compiles | |
| | Smoke test: open, merge, extract, deletePages, render, dispose | 7/7 | |
| **B21** | Delete old code — platform/, old ffi/, old document/pdf.dart, old editor/, old builder/, old core source/sink/info | clean | **DONE** |
| | Old `lib/src/platform/` (7 files) | deleted | |
| | Old `lib/src/ffi/` (4 files) | deleted | |
| | Old `lib/src/document/pdf.dart`, `lib/src/editor/`, `lib/src/builder/` | deleted | |
| | Old `lib/src/core/pdf_sink.dart`, `pdf_source.dart`, `pdf_info.dart` | deleted | |
| | Barrel `pdf_manipulator.dart` updated — exports `api/` + shared types only | compiles | |
| | `test/helpers/pdf_fixtures.dart` updated — uses new `Pdf` instance API + `test_helpers.dart` | compiles | |
| | Old tests deleted: `test/document/`, `test/editor/`, `test/builder/`, `test/extraction/`, `test/platform/`, `test/web/` | deleted | |
| | Shared types moved to `api/types/` in B21c (errors, pdf_image, pdf_rect, pdf_signature, search_result, pdf_doc, pdf_page_info) | moved | |
| | `dart analyze lib/ test/` — zero errors, zero warnings | clean | |
| | **Total tests at B21: 55 Dart + 27 Rust = 82** | all pass | |
| **B21b** | Clean dead Rust: 11 `LOCAL PATCH` ffi.rs wrappers deleted (448 lines), `ffigen.yaml` deleted. Header cleanup + fork push pending. | 448 lines removed | **DONE** (push pending) |
| **B21c** | Move 7 shared type files from `core/`, `document/`, `page/` into `api/types/` + delete empty dirs | all imports updated | **DONE** |
| **B21d** | `web_assets/` = source (packaged), `web/` = consumer destination (setup copies there) — correct by design | N/A | **DONE** |
| **B21e** | Forward missing `PdfPageBuilder` methods — 9 methods added end-to-end (Rust enum + replay + op codes 18-26 + Dart bridge + native + web + PdfPageBuilder) | compiles | **DONE** |
| **B21f** | Align doc §2 file layout to match reality — inlined types stay inlined, no separate messages/protocol files | doc updated | **DONE** |
| **B22** | Comprehensive test suite: `pdf_all_ops_test.dart` (25), `editor_test.dart` (8), `builder_test.dart` (4), `dispose_e2e_test.dart` (4), `error_e2e_test.dart` (5), `timeout_e2e_test.dart` (3), `editor_e2e_test.dart` (7) | 56/56 (37 API + 19 edge case) | **DONE** |
| | **── B17: WEB FIRST-CLASS — three I/O modes + coordinator + full parity ──** | | |
| **B17a** | Rust WASM: `JsCallbackWriter` struct (`Write` via JS function) + `PositionTracker` | cargo check | **DONE** |
| **B17b** | Rust WASM: `WasmPdfDocument.saveToWriter(writeFn)` — streaming output via `write_full_to_writer` | cargo check | **DONE** |
| **B17c** | Rust WASM: `WasmPdfDocument.renderPageFit(page, w, h)` — pixel bounding box render | cargo check | **DONE** |
| **B17d** | Rust WASM: `WasmPdfDocument.editorFromReader(readFn, lengthFn)` — editor via `from_document`, reader re-opened for reads | cargo check | **DONE** |
| **B17e** | JSPI — no Rust-side change. `readFn` is a `js_sys::Function` called via `.call2()`. JSPI wrapping happens at JS instantiation level in `wasm_worker.js`. | N/A | **DONE** (JS-side) |
| **B17f** | Rust WASM: `WasmPdf.mergeFromReaders(readerArray)` — each input opened via `JsCallbackReader`, merged via `merge_from_document`, output via `JsCallbackWriter` | cargo check | **DONE** |
| | **── B17 JS: coordinator worker + three I/O modes ──** | | |
| **B17g** | JS: `coordinator.js` — coordinator worker. Pool management, I/O mode detection (`jspi`/`atomics`/`opfs`), read/write/stream routing between main ↔ WASM workers. OPFS lifecycle. Cancel/dispose. | written | **DONE** |
| **B17h** | JS: `wasm_worker.js` — per-operation WASM worker. Loads WASM, creates `readFn` per I/O mode (JSPI async/Atomics.wait/OPFS SyncAccessHandle), `writeFn` posts chunks. Full op dispatch (open, merge, structural, extract, render, stream, search, sign, stamps, editor, builder). | written | **DONE** |
| **B17i** | JS: JSPI mode in `wasm_worker.js` — `createReadFnJspi`: readFn is async, returns Promise, pending promises map resolved via `readAtResponse` message from coordinator. | in wasm_worker.js | **DONE** |
| **B17j** | JS: Atomics mode in `wasm_worker.js` — `createReadFnAtomics`: writes to SAB, posts to coordinator, `Atomics.wait` blocks. Coordinator writes response to SAB, `Atomics.notify`. | in wasm_worker.js + coordinator.js | **DONE** |
| **B17k** | JS: OPFS mode in `wasm_worker.js` — `createReadFnOpfs`: opens `SyncAccessHandle`, reads synchronously from disk. Coordinator handles OPFS write/finalize. | in wasm_worker.js + coordinator.js | **DONE** |
| **B17l** | JS: output streaming — `createWriteFn` in `wasm_worker.js` posts chunks via `postMessage`. Coordinator forwards `{type:'chunk'}` to main. All modes identical. | in wasm_worker.js + coordinator.js | **DONE** |
| **B17m** | JS: per-item streaming — `{type:'item'}` per image/page + `{type:'itemDone'}`. Coordinator forwards to main. All modes identical. | in wasm_worker.js + coordinator.js | **DONE** |
| **B17n** | JS: editor ops in `wasm_worker.js` — `editorOpen` (reads all bytes via reader for ensure_editor), `editorMutate`, `editorSave` (via `saveToWriter`), `editorGetMetadata`, `editorPageMediaBox`, `editorExtractPages`, `editorMergeFrom`, `editorDispose`. | in wasm_worker.js | **DONE** |
| **B17o** | JS: builder ops in `wasm_worker.js` — `builderCreate`, `builderSetMetadata`, `builderAddPage`, `builderPageOp` (26 ops), `builderPageDone`, `builderSave` (via `writeFn`), `builderDispose`. | in wasm_worker.js | **DONE** |
| | **── B17 Dart: coordinator protocol + three-mode WebBridge ──** | | |
| **B17p** | Dart: I/O mode detection — coordinator detects mode at init, reports to main. No separate `io_mode.dart` needed; detection is JS-side in coordinator.js. | in coordinator.js | **DONE** |
| **B17q** | Dart: coordinator protocol — message types between main ↔ coordinator: `submit/submitted`, `readAt/readAtResponse`, `chunk`, `item/itemDone`, `result/error`, `cancel/cancelled`, `dispose/disposed`, `opfs.write/writeAck`, `opfs.finalize/finalizeAck`. | in web_bridge.dart + coordinator.js | **DONE** |
| **B17r** | Dart: `web_bridge.dart` rewrite — coordinator worker replaces direct worker communication. Main thread sends ops to coordinator, answers readAt requests from `_opSources`, receives chunks for `_pendingChunks`, items for `_pendingStreams`. `_submitCompleters` queue handles opId assignment. | 128 tests pass, 0 analyzer issues | **DONE** |
| **B17s** | Dart: pool managed by coordinator — `_submit` sends `{type:'submit'}` to coordinator. Coordinator manages worker acquire/release/terminate. Main thread has no `WebWorkerPool` dependency for new ops. | in coordinator.js + web_bridge.dart | **DONE** |
| **B17t** | Dart: output chunk routing — coordinator forwards `{type:'chunk'}` from WASM worker to main. Main's `_onCoordinatorMessage` writes chunk to `_pendingChunks[opId]` PdfSink. | in web_bridge.dart | **DONE** |
| **B17u** | Dart: stream item routing — coordinator forwards `{type:'item'}` + `{type:'itemDone'}` from WASM worker to main. Main's handler adds to `_pendingStreams[opId]` StreamController, closes on itemDone. | in web_bridge.dart | **DONE** |
| **B17v** | Dart: editor + builder handles via coordinator — `_WebEditorHandle` and `_WebBuilderHandle` route through `_submit('editorX'/'builderX')`. Persistent handles on WASM worker side. `_WebPageBuilderHandle` routes page ops. | in web_bridge.dart | **DONE** |
| | **── B17 Tests: every mode, every op, full parity with native ──** | | |
| **B17w** | Test infra: asset server serves coordinator.js + wasm_worker.js + WASM binary for `dart test -p chrome` | | NOT STARTED |
| **B17x-jspi** | JSPI mode tests: open, merge, structural, extract, stream, editor, builder, dispose (mirror native e2e suite) | | NOT STARTED |
| **B17x-atomics** | Atomics mode tests: same ops as JSPI tests, forced Atomics mode (verify SAB + Atomics.wait path) | | NOT STARTED |
| **B17x-opfs** | OPFS mode tests: same ops, forced OPFS mode (verify pre-copy + SyncAccessHandle path) | | NOT STARTED |
| **B17y** | Mode detection test: verify correct mode selected per browser capability | | NOT STARTED |
| **B17z** | Coordinator test: verify main ↔ coordinator ↔ WASM worker message routing, cancel, dispose | | NOT STARTED |
| **B23** | Update docs: ARCHITECTURE.md, CAPABILITY_ROADMAP.md, README.md, MIGRATION.md | | NOT STARTED |
| **B24** | Rewrite example app + example integration test for new API | | NOT STARTED |
| **B25** | Final fork audit: diff our `pdf_manipulator/0.3.47-patches` against upstream `main` one more time. Verify every remaining difference is intentional and documented. Goal: our fork should be upstream + ONLY our additive patches (bridge module, editor extensions, writer extensions, wasm extensions). No stale diffs, no accidental upstream-revert, no leftover experiments. If upstream shipped features that overlap with our patches since we forked, rebase to absorb them and delete our redundant patches. Push the cleaned fork so `git diff upstream/main..HEAD` shows ONLY what we own. | | NOT STARTED |

### Planned (not blocking — engine doesn't support yet)

| Feature | Why deferred |
|---|---|
| `PdfEditor.addRedaction`, `redactionCount`, `scrubMetadata` | Needs Rust bridge wiring for redaction tracking. Add when redaction workflow is prioritized. |
| `planSplitByBookmarks`, `splitByBookmarks` | pdf_oxide has bookmark access but no split-by-bookmark API. Add when upstream ships it. |
| `convertTo` (PDF → DOCX/PPTX/XLSX) | pdf_oxide v0.3.48+. Not shipped upstream yet. |
| `convertToPdf` (DOCX/PPTX/XLSX → PDF) | Same. |
| `classifyPage`, `classifyDocument` | Not in pdf_oxide. Would need ML/heuristic engine. Future feature. |

### Rust dependencies

```toml
crossbeam-channel = "0.5"   # thread pool task queue
bumpalo = "3"                # arena allocator
allo-isolate = "0.1"         # Dart_PostCObject wrapper (result posting)
```

### Key test: Rust integration proves the condvar dance

Pool thread creates CallbackReader → writes request to shared buffer →
calls notify fn → blocks on condvar. Fulfiller thread reads request →
copies data → locks mutex → sets READY flag → signals condvar. Pool
thread wakes → reads response → returns bytes. Cancel flag → returns
`io::ErrorKind::Interrupted`. Full read: pool thread reads 16 bytes of
test data correctly. **This is the exact pattern that Dart's
`NativeCallable.listener` will use in production.**

### What's proven end-to-end

**Native read pipeline (CallbackReader → condvar → NativeCallable.listener):**
- `NativeBridge.open` — 5 e2e tests. Dart main → SourceServer → worker isolate
  → NativeCallable.listener → shared buffer → Rust pool thread → CallbackReader
  → condvar dance → pdf_oxide engine → result posted via allo-isolate → Dart
  decodes PdfDoc. Page count, version, dimensions, encryption status all verified.

**Native write pipeline (CallbackWriter → condvar → NativeCallable.listener):**
- `NativeBridge.merge` — 4 e2e tests. Two-PDF merge, three-PDF merge, output
  verified by re-opening with NativeBridge.open. Output streams via CallbackWriter
  → shared buffer → worker listener → SinkServer → consumer's PdfSink.

**Generic edit dispatch (`bridge_submit_edit`):**
- One Rust FFI function handles ALL edit operations via op code.
- Rust `dispatch_edit_op` switch dispatches: merge(1), extractPages(2),
  deletePages(3), rotateAllPages(6), flattenForms(7), compress(9).
- Dart `_handleEdit` in worker_entry.dart handles all ops generically.
- `NativeBridge._submitEdit` helper on the Dart side creates SourceServer +
  SinkServer, sends command, checks result.
- 4 structural e2e tests: deletePages (2-page→1-page), rotateAllPages (90°),
  flattenForms (valid output), compress (valid output).

**FFI bindings cleanup:**
- All Dart FFI names are camelCase with `symbol:` parameter mapping to C names.
- Zero `// ignore: non_constant_identifier_names` comments.
- Clean `dart analyze` — zero errors, zero warnings, zero infos.

**Native edit ops fully wired (18 op codes):**
- All edit ops through `_submitEdit` → worker `_handleEdit` → Rust
  `dispatch_edit_op`. Params are binary-encoded per op. Secondaries
  (cert bytes, image bytes, merge sources) passed as separate byte arrays.
- Op 15 (watermark): uses `WatermarkAnnotation` builder + `add_annotation`.
- Op 16 (sign): uses `sign_pdf_bytes` + `replace_source_bytes` (requires
  `signatures` cargo feature). Gated with `#[cfg(feature = "signatures")]`.
- Op 17 (addStamp): uses `StampAnnotation` builder + `add_annotation`.
- Op 18 (addImageStamp): uses `StampAnnotation::with_image` + `add_annotation`.

**Native read ops fully wired (7 op codes):**
- All read ops through `_submitRead` → worker `_handleRead` → Rust
  `dispatch_read_op`. Result is binary-encoded per op, decoded on Dart side.
- Op 3 (search): uses `TextSearcher::search` + filter by page. Returns
  hit count + per-hit (page, bbox.x, bbox.y, bbox.width, bbox.height, text).
- Op 4 (getSignatures): uses `enumerate_signatures`. Returns signature info.
- Op 5 (verifySignatures): returns false (verify_all not implemented in pdf_oxide).

**Shared `ByteCountSink` moved to `bridge.dart`:**
- Both NativeBridge and WebBridge use `ByteCountSink` (for `splitBySize`
  trial-extraction). Was private in native_bridge, duplicated attempt in
  web_bridge. Moved to `bridge.dart` as a public class — single definition,
  both bridges import it.

**Web bridge — full parity (zero UnimplementedError):**
- ALL ops wired: open, merge, split, splitBySize, extractPages, deletePages,
  reorderPages, movePage, rotatePages, rotateAllPages, flattenForms,
  applyRedactions, embedFile, eraseRegions, compress, extract, search,
  watermark, encrypt, encryptFull, decrypt, sign, addStamp, addImageStamp,
  imagesToPdf, render, extractImages, getSignatures, verifySignatures,
  validatePdfA, validatePdfUa.
- `worker.js` updated with renderPage, extractImages, sign, verifySignatures cases.
- render + extractImages use `_resolvePages` helper to expand `PdfPages` sealed type.
- Web e2e tests blocked on asset server (worker.js URL resolution in `dart test -p chrome`).

**Layer 1 API — comprehensive coverage (B22):**
- `pdf_all_ops_test.dart` — 25 tests covering every `Pdf` method: open (3),
  merge (1), split (1), splitBySize (1), extractPages (1), deletePages (1),
  rotateAllPages (1), flattenForms (1), compress (1), extract text+markdown (2),
  search (1), watermark (1), encrypt+decrypt (1), render single+all (2),
  extractImages (1), getSignatures (1), verifySignatures (1), validatePdfA (1),
  validatePdfUa (1), dispose double+post-dispose (2).
- `editor_test.dart` — 8 tests: open→pageCount, version, setTitle→save→verify,
  setAuthor→save→verify, delete page, rotate→verify, getPageMediaBox, double dispose.
- `builder_test.dart` — 4 tests: A4+text→save→verify, Letter+heading, custom size, double dispose.
- All tests use `TestSource`/`TestSink` from `test/bridge/test_helpers.dart`.
- All 37 tests pass on native.

**Cleanup (B21b-f):**
- 11 dead `LOCAL PATCH` ffi.rs C wrappers deleted (448 lines).
- `ffigen.yaml` deleted (bridge_bindings.dart is hand-written with `symbol:` params).
- 7 shared type files moved from `core/`, `document/`, `page/` → `api/types/`.
- 9 missing PdfPageBuilder methods added end-to-end (op codes 18-26).
- Doc §2 aligned to actual file layout.

### What's done

1. ~~**B17a-f** (Rust WASM bindings)~~ — **DONE**
2. ~~**B17g-o** (JS workers)~~ — **DONE** (coordinator.js + wasm_worker.js)
3. ~~**B17p-v** (Dart web bridge)~~ — **DONE** (WebBridge with EventStreamProvider, OPFS pre-copy, _toJSWithTransfers)
4. ~~**B17w-z** (Web tests)~~ — **DONE** (shared_tests run on both platforms, OPFS pipeline test, Atomics readAt chain test, wire sync test. 333 total tests.)
5. ~~**B22** (edge cases)~~ — **DONE**
6. ~~**Shared editor_ops.rs**~~ — **DONE** (20 ops: delete, extract, move, rotate, flatten, compress, watermark, stamp, imageStamp, metadata, etc. Both ffi_api.rs and wasm.rs call the same functions.)
7. ~~**Rendering on WASM**~~ — **DONE** (pure Rust tiny-skia, added `rendering` to `wasm` feature)
8. ~~**WASM edit method fixes**~~ — **DONE** (deletePage, movePage, extractPages use ensure_editor → editor_ops, matching native FFI pattern)

### What's next

1. **JSPI ground** — 5 steps:
   - **Step 1 — Rust `wasm.rs`**: Declare `__pdf_read_at(offset, count) -> Uint8Array` and `__pdf_source_length() -> f64` as WASM imports via `#[wasm_bindgen(module = "/pdf_read_import.js")]`. Create `WasmImportReader` implementing `Read + Seek` that calls these imports synchronously (JSPI suspends the WASM stack transparently). Create `fromReaderJspi(password)` that uses `WasmImportReader`. Keep existing `fromReader(readFn, lengthFn)` for Atomics/OPFS modes.
   - **Step 2 — JS `vendor/pdf_oxide/pdf_read_import.js`**: Stub module required by wasm-bindgen at compile time. Exports `__pdf_read_at` and `__pdf_source_length` as throwing stubs. Overridden at runtime during JSPI instantiation.
   - **Step 3 — JS `wasm_worker.js`**: In JSPI mode, monkey-patch `WebAssembly.instantiate` BEFORE calling `init()`. The patch intercepts the import object, finds the `/pdf_read_import.js` namespace, replaces `__pdf_read_at` with a `new WebAssembly.Function({parameters: ['f64','f64'], results: ['externref']}, asyncReadFn, {suspending: 'first'})`. The `asyncReadFn` returns a Promise that resolves when the coordinator delivers bytes. Wrap relevant exports with `WebAssembly.promising`. Restore original `WebAssembly.instantiate` after `init()`. Dispatch uses `fromReaderJspi()` in JSPI mode, `fromReader(readFn, lengthFn)` in Atomics/OPFS modes.
   - **Step 4 — JS `coordinator.js`**: Re-enable JSPI detection: `if (typeof WebAssembly.Suspending !== 'undefined') return 'jspi'`. ReadAt fulfillment chain is identical to Atomics mode (coordinator forwards readAt to main, main responds, coordinator resolves the Promise instead of writing to SAB).
   - **Step 5 — Test**: `jspi_test.dart` — verify `ioMode === 'jspi'`, open PDF via Promise-based readAt chain, no OPFS pre-copy, no SharedArrayBuffer headers needed.
2. **B23**: Docs — ARCHITECTURE.md, CAPABILITY_ROADMAP.md, README.md.
3. **B24**: Rewrite example app + integration test for new API.
4. **B25**: Final fork audit — diff against upstream, document all patches.

### Rust handle architecture (B19 — editors + builders)

**Editor handles:** persistent editors use a global `EDITOR_HANDLES` map (`Mutex<HashMap<u64, Arc<Mutex<DocumentEditor>>>>`).
Seven C API functions:

| Function | Purpose |
|---|---|
| `bridge_editor_open` | Read bytes via CallbackReader → `DocumentEditor::from_bytes` → store in handle map → post handle ID |
| `bridge_editor_mutate` | Look up handle → lock editor → `dispatch_edit_op(op_code, params, secondaries)` → post result |
| `bridge_editor_save` | Look up handle → lock editor → `write_to_writer_with_options(CallbackWriter, SaveOptions)` → post result |
| `bridge_editor_dispose` | Remove handle from map (editor dropped) |
| `bridge_editor_page_count` | Synchronous — returns directly (no pool thread) |
| `bridge_editor_get_metadata` | Reads title/author/subject/keywords/version → posts binary result |

The editor's `Arc<Mutex<DocumentEditor>>` is cloned from the map before releasing the map lock,
so map operations don't block editor mutations. Each mutation acquires the editor lock, applies
the change, releases. Save acquires the editor lock for the duration of the write.

On the Dart side, `_NativeEditorHandle` routes each `BridgeEditorHandle` method through
`_bridge._send('editorMutate', ...)` with the matching op code + binary-encoded params.
Save goes through `_bridge._send('editorSave', ...)` which sets up a SinkServer +
write listener (same pattern as one-shot edit ops). Metadata reads go through
`_bridge._send('editorGetMetadata', ...)` which calls `bridge_editor_get_metadata` and
decodes the binary result (version + title/author/subject/keywords). `getPageMediaBox`
is synchronous via `bridge_editor_get_page_media_box` (writes 4 f64s to caller buffer).
`extractPages` saves the editor to a temp buffer then uses one-shot `extractPages`.
`mergeFrom` reads the other source's bytes and passes as secondary to merge op (1).
All 28 edit op codes are wired with proper param encoding — zero TODO stubs on
`_NativeEditorHandle`.

**Builder handles:** persistent builders use a global `BUILDER_HANDLES` map
(`Mutex<HashMap<u64, BuilderState>>`). `BuilderState` wraps `Option<DocumentBuilder>`
(take-apply-put pattern for the consuming builder API) + `Vec<BuilderPageOp>` (buffered
page operations replayed on save).

Six C API functions:

| Function | Purpose |
|---|---|
| `bridge_builder_create` | Create `DocumentBuilder`, store in handle map, return handle ID synchronously |
| `bridge_builder_set_metadata` | Take builder out, apply `.title()`/`.author()`/etc (consuming API), put back |
| `bridge_builder_add_page` | Set page dimensions (A4=0, Letter=1, Custom=2), mark page open |
| `bridge_builder_page_op` | Generic dispatch for 17 page operation op codes (font, text, heading, image, form fields, etc.) — buffers ops |
| `bridge_builder_save` | Take builder + ops out of map, replay ops via `replay_page_ops()` against real `FluentPageBuilder`, `builder.build()` → stream bytes via `CallbackWriter` |
| `bridge_builder_dispose` | Remove handle from map (builder dropped) |

Builder page op codes: 1=font, 2=at, 3=text, 4=heading, 5=paragraph, 6=space,
7=horizontalRule, 8=image, 9=watermark, 10=textField, 11=checkbox, 12=comboBox,
13=pushButton, 14=signatureField, 15=newline, 16=newPageSameSize, 17=done,
18=radioGroup, 19=fieldKeystroke, 20=fieldFormat, 21=fieldValidate,
22=fieldCalculate, 23=linkUrl, 24=linkPage, 25=footnote, 26=columns.

The builder's consuming API (`title(self) -> Self`) is handled by `Option<DocumentBuilder>`
with take-apply-put: `state.builder.take()` → apply method → `state.builder = Some(result)`.
Same pattern used by the old FFI's `FfiDocumentBuilder`.

On the Dart side, `_NativeBuilderHandle` routes metadata/addPage/save/dispose through the
worker. `_NativePageBuilderHandle` encodes page ops as binary params (f32 coordinates,
UTF-8 strings, Uint8List secondary for image bytes) and sends via `builderPageOp` worker op.
`_WebBuilderHandle` and `_WebPageBuilderHandle` route through the same worker.js `builder.*`
and `page.*` message dispatch that already exists from the old code.

Source and test file layouts are in §2 (single source of truth).
Old code (`lib/src/platform/`, old `lib/src/ffi/`, old Layer 1 classes)
has been deleted in B21. Shared data types now live at `lib/src/api/types/`
(moved from `core/`, `document/`, `page/` in B21c).

### Test strategy

File layout for tests is in §2. The approach:

- **Rust tests** (`cargo test --lib bridge`): unit tests per module +
  integration test proving the condvar dance with real threads. 27 total.
- **Dart native tests** (`dart test test/bridge/native/`): end-to-end
  with real PDFs. Each test creates a `NativeBridge`, runs an operation,
  verifies the result. Tests cancel, dispose, timeout.
- **Dart web tests** (`dart test test/bridge/web/ -p chrome`): same ops
  but via OPFS + Web Workers. Must run in Chrome (OPFS only in browsers).
- **Dart shared tests** (`dart test test/bridge/shared/`): contract tests
  that run against BOTH bridges via the `PdfBridge` interface. Same test
  code, two implementations. Guarantees symmetry.
- **Dart API tests** (`dart test test/api/`): consumer-facing tests.
  Use `Pdf()` instance, don't know which bridge is underneath. These
  replace the existing 302 tests after the old code is deleted.

### Test counts

| Suite | Count | Status |
|---|---|---|
| Rust bridge unit + integration | 27/27 | Pass |
| Dart native bridge: open | 11/11 | Pass |
| Dart native bridge: merge | 4/4 | Pass |
| Dart native bridge: structural (delete, rotate, flatten, compress) | 4/4 | Pass |
| Dart native bridge: extract (text + markdown) | 3/3 | Pass |
| Dart native bridge: stream (render + extractImages) | 3/3 | Pass |
| Dart Layer 1 API: smoke test | 7/7 | Pass |
| Dart Layer 1 API: comprehensive ops (pdf_all_ops_test) | 25/25 | Pass |
| Dart Layer 1 API: PdfEditor lifecycle (editor_test) | 8/8 | Pass |
| Dart Layer 1 API: PdfBuilder lifecycle (builder_test) | 4/4 | Pass |
| Dart native bridge: dispose | 4/4 | Pass |
| Dart native bridge: error handling | 5/5 | Pass |
| Dart native bridge: timeout + slow source | 3/3 | Pass |
| Dart native bridge: editor lifecycle | 7/7 | Pass |
| Dart web bridge | 0/? | Blocked (asset server) |
| Dart shared contract | — | Not yet written |
| Old tests | — | Deleted (incompatible with new API) |
| **Total Dart passing** | **88** (44 bridge + 44 API) | |
| **Total Rust passing** | **27** | |
| **Grand total** | **115** | |
| **UnimplementedError remaining** | **0** in bridge impls (only worker default-case safety catch) | |

---

## 13. Critical findings during build

### Dart_ExitIsolate crashes inside NativeCallable.isolateLocal

The original design (§4.2) described using `Dart_ExitIsolate` to yield the
isolate thread while the condvar blocks. **This crashes the Dart VM.**
Tested and confirmed: calling `Dart_ExitIsolate()` inside a
`NativeCallable.isolateLocal` callback segfaults immediately.

The fix: the engine runs on a **raw pthread** (pool thread), NOT on the
isolate thread. The pool thread calls `NativeCallable.listener` (which
can be called from any thread) and blocks on the condvar. The listener
fires on the isolate's event loop (a different thread). No
`Dart_ExitIsolate` needed.

This is the same architecture that `flutter_rust_bridge` uses in
production: Rust thread pool + `Dart_PostCObject` for results.

### Memory ordering on shared buffer

The fulfiller (host thread) must lock the mutex BEFORE reading the
request fields from the shared buffer. Raw pointer reads (`read_i64`)
have no memory ordering guarantees. The mutex lock/unlock provides
the happens-before relationship that makes the pool thread's writes
visible to the fulfiller. Without the lock, the fulfiller may see
stale values.

### Generic edit dispatch — one FFI function for all edit ops

Rather than one `bridge_submit_X` per operation (which would be 30+
FFI functions), a single `bridge_submit_edit` takes an `op_code` integer
+ serialized `op_params` bytes + optional secondary byte arrays. Rust's
`dispatch_edit_op` switch handles the dispatch. This mirrors how
flutter_rust_bridge uses a single generic entry point with serialized
params.

Edit op codes: 1=merge, 2=extractPages, 3=deletePages, 5=rotatePages,
6=rotateAllPages, 7=flattenForms, 8=applyRedactions, 9=compress,
10=movePage, 11=embedFile, 12=eraseRegions, 13=encrypt, 14=decrypt,
15=watermark, 16=sign, 17=addStamp, 18=addImageStamp, 19=setTitle,
20=setAuthor, 21=setSubject, 22=setKeywords, 23=unembedStandardFonts,
24=flattenAllAnnotations, 25=setFormFieldValue, 26=cropMargins,
27=convertToPdfA, 28=resizeImage.

Read op codes: 1=extractText, 2=extractMarkdown, 3=search,
4=getSignatures, 5=verifySignatures, 6=validatePdfA, 7=validatePdfUa.

Params are little-endian binary. For example, deletePages sends
`[count: i32, page0: i32, page1: i32, ...]`. Rust reads with
`read_i32(params, offset)` helper. Dart encodes with
`ByteData.setInt32(offset, value, Endian.little)`.

On the Dart side, `NativeBridge._submitEdit(opName, source, sink, ...)`
and `worker_entry._handleEdit(args, opCode:)` are the generic handlers.
Adding a new edit op requires: one Rust switch arm + one Dart
`_dispatch` case + one `NativeBridge` method calling `_submitEdit`.

### FFI symbol: parameter — zero // ignore: comments

All FFI bindings use `@ffi.Native<...>(symbol: 'c_symbol_name')` with
camelCase Dart names. The `symbol:` parameter tells the linker to look
up the actual C name. Example:

```dart
@ffi.Native<ffi.Void Function()>(symbol: 'bridge_init')
external void bridgeInit();
```

No `// ignore: non_constant_identifier_names` anywhere. This is the
correct pattern for Dart FFI — the same approach `dart:io` uses.

### ThreadPool::Drop must join threads

The `Drop` impl originally only called `shutdown()` (drops the sender).
Workers see the disconnected channel and exit their loops — but the
`drop()` returned before they finished. The test assertion after `drop(pool)`
raced with the still-running threads, causing flaky failures.

Fix: `Drop` now calls `shutdown()` + `join()`. The `join()` blocks until
all workers have exited. This gives a happens-before guarantee between
the tasks' writes and the test's reads. Confirmed stable across 5
consecutive runs.

### split and splitBySize compose from existing ops

Rather than adding new Rust FFI functions for split, both `split` and
`splitBySize` are implemented on the Dart side by composing `open`
(get page count) + `extractPages` (extract a subset) in a loop. Each
chunk gets its own `PdfSink` from the consumer's `sinkFactory`. This
is the same pattern on both native and web — zero platform-specific code.

`splitBySize` trial-extracts each growing page set into a `_ByteCountSink`
(counts bytes without storing them) to check if the chunk exceeds
`maxBytes`. If so, it splits before the oversized page.

### imagesToPdf uses a dedicated Rust FFI

`imagesToPdf` has no PdfSource input — it's not an edit-on-existing-PDF
operation. A dedicated `bridge_submit_images_to_pdf` Rust FFI takes image
byte arrays + write channel. Internally it uses `PdfBuilder::from_image_data_multiple`
from pdf_oxide's API layer. Output streams via the same `CallbackWriter`
as all other write ops.

### Web has no Rust-side bridge test

The web bridge (OPFS + JsCallbackReader) can only be tested in a
browser (`dart test -p chrome`) because OPFS `SyncAccessHandle`,
Web Workers, and `postMessage` only exist in browser environments.
`cargo test` cannot exercise browser APIs. The Rust `JsCallbackReader`
in `wasm.rs` is compiled to WASM and runs inside the browser's
Web Worker — it's tested via Dart browser tests, not Rust unit tests.

---

## 14. The one-line summary

> **Three-level architecture on both platforms: UI thread → coordinator
> (worker isolate / coordinator Worker) → engine pool (pthreads / WASM
> Workers). Two symmetry guarantees: (1) Dart `protocol/` layer —
> `EngineOp` enum + arg builders + result parsers shared by both bridges.
> (2) Rust `editor_ops.rs` — all edit operations as shared functions
> called by both native FFI and WASM bindings. Zero logic duplication.
> Native reads via condvar+listener. Web reads via two modes:
> Atomics (on-demand, needs COOP/COEP headers) and OPFS (universal
> fallback, pre-copies to disk). JSPI (zero-header on-demand) designed
> but needs WASM import refactor — tracked. Rendering works on web
> (pure Rust tiny-skia). Mode detected once at startup. Output streams
> via condvar (native) or postMessage chunks (web). Shared tests run
> the SAME 16 test functions on both platforms — drift caught
> immediately. 333 total tests. Web is a first-class citizen.**
