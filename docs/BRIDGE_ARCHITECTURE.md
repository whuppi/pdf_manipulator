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
│   │       ├── pdf_doc.dart                ○ (currently at lib/src/document/)
│   │       ├── pdf_image.dart              ○ (currently at lib/src/core/)
│   │       ├── pdf_page_info.dart          ○ (currently at lib/src/page/)
│   │       ├── pdf_rect.dart               ○ (currently at lib/src/core/)
│   │       ├── pdf_signature.dart          ○ (currently at lib/src/core/)
│   │       ├── search_result.dart          ○ (currently at lib/src/core/)
│   │       └── errors.dart                 ○ (currently at lib/src/core/)
│   │
│   ├── bridge/                             ← LAYER 2: The plumbing
│   │   ├── bridge.dart                     ✓ abstract PdfBridge interface
│   │   ├── bridge_factory.dart             ✓ conditional import router
│   │   ├── _factory_native.dart            ✓
│   │   ├── _factory_web.dart               ✓
│   │   │
│   │   ├── native/
│   │   │   ├── native_bridge.dart          ✓ ALL one-shot ops + openEditor + createBuilder + ALL handle methods wired
│   │   │   ├── worker_isolate.dart         ✓ listener creation helpers
│   │   │   ├── source_server.dart          ✓ main-isolate PdfSource fulfillment
│   │   │   ├── sink_server.dart            ✓ main-isolate PdfSink fulfillment
│   │   │   ├── shared_buffer.dart          ✓ mirrors Rust layout
│   │   │   ├── result_decoder.dart         ○ binary result → PdfDoc / PdfError (currently inline in native_bridge.dart)
│   │   │   ├── messages.dart               ○ WorkerMsg, WorkerResult, Op enum
│   │   │   └── stream_protocol.dart        ○ WorkerStreamItem for per-item yields
│   │   │
│   │   ├── web/
│   │   │   ├── web_bridge.dart             ✓ ALL ops + openEditor + createBuilder wired — zero UnimplementedError
│   │   │   ├── worker_pool.dart            ✓ pool + queue + cancel
│   │   │   ├── opfs.dart                   ✓ OPFS streaming + cleanup registry
│   │   │   └── worker_protocol.dart        ○ message types for postMessage
│   │   │
│   │   └── ffi/
│   │       └── bridge_bindings.dart        ✓ @Native decls (camelCase + symbol:)
│   │
│   └── _internal.dart                      ○ internal barrel (bridge for api)
│
web_assets/
├── worker.js                               ✓ web worker (OPFS + WASM dispatch)
└── pdf_oxide.js / .wasm                    ✓ WASM engine

vendor/pdf_oxide/src/
├── bridge/                                 ← engine-side bridge
│   ├── mod.rs                              ✓
│   ├── thread_pool.rs                      ✓ 4 tests
│   ├── callback_reader.rs                  ✓ 3 tests
│   ├── callback_writer.rs                  ✓ 4 tests
│   ├── arena.rs                            ✓ 5 tests
│   ├── shared_buffer.rs                    ✓ 4 tests
│   ├── ffi_api.rs                          ✓ 4 tests + all submit ops + editor handle ops + builder handle ops (6 FFI fns) + 28 edit op codes + 17 builder page op codes
│   └── integration_test.rs                 ✓ 3 tests (condvar end-to-end)
├── document.rs                             ✓ PdfDocument + PdfReader (Boxed variant)
├── editor/document_editor.rs               ✓ DocumentEditor
├── ffi.rs                                  ✓ existing C API
└── wasm.rs                                 ✓ WASM API + JsCallbackReader

test/
├── helpers/
│   ├── memory_io.dart                      ✓ TestPdfSource + TestPdfSink
│   ├── pdf_fixtures.dart                   ✓ minimal PDF bytes, multi-page builder
│   ├── slow_source.dart                    ○ PdfSource that delays (timeout tests)
│   └── test_server.dart                    ○ hybrid server for web tests
│
├── bridge/                                 ← LAYER 2 tests
│   ├── test_helpers.dart                   ✓ TestSource + TestSink (new api types)
│   ├── native/
│   │   ├── open_e2e_test.dart              ✓ NativeBridge.open (5 tests)
│   │   ├── open_test.dart                  ✓ additional open tests
│   │   ├── merge_e2e_test.dart             ✓ NativeBridge.merge (4 tests)
│   │   ├── structural_e2e_test.dart        ✓ delete, rotate, flatten, compress (4 tests)
│   │   ├── extract_e2e_test.dart           ✓ text + markdown extraction (3 tests)
│   │   ├── stream_e2e_test.dart            ✓ per-item streaming (render + extractImages, 3 tests)
│   │   ├── cancel_e2e_test.dart            ○ cancel running op mid-flight
│   │   ├── dispose_e2e_test.dart           ○ instant dispose kills everything
│   │   ├── timeout_e2e_test.dart           ○ slow PdfSource triggers timeout
│   │   └── editor_e2e_test.dart            ○ PdfEditor lifecycle (open, mutate, save, dispose)
│   │
│   ├── web/                                ← @TestOn('browser') — runs in Chrome
│   │   ├── open_e2e_test.dart              ○ WebBridge.open via OPFS (blocked: asset server)
│   │   ├── merge_e2e_test.dart             ○ WebBridge.merge
│   │   ├── stream_e2e_test.dart            ○ per-item streaming via postMessage
│   │   ├── cancel_e2e_test.dart            ○ cancel via Worker.terminate
│   │   ├── dispose_e2e_test.dart           ○ terminate all + OPFS cleanup
│   │   └── opfs_cleanup_test.dart          ○ registry cleans on failure
│   │
│   └── shared/                             ← platform-agnostic contract tests
│       ├── bridge_contract_test.dart       ○ every PdfBridge method (both bridges)
│       └── source_sink_test.dart           ○ PdfSource/PdfSink edge cases
│
├── api/                                    ← LAYER 1 tests (consumer-facing)
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

## 5. Web bridge — the full flow

### 5.1 Architecture overview

```
┌─────────────────────────────────────────────────────────────┐
│ MAIN THREAD (Dart, Flutter UI)                              │
│                                                             │
│  Consumer's PdfSource + PdfSink live here.                  │
│  Streams PdfSource data to OPFS via worker.                 │
│  Receives output chunks from worker.                        │
│  UI keeps drawing. Never blocks.                            │
└────────────────────┬────────────────────────────────────────┘
                     │ postMessage (structured clone + transfer)
                     │
┌────────────────────▼────────────────────────────────────────┐
│ WEB WORKER POOL (JS, multiple workers)                      │
│                                                             │
│  Fixed size: max(2, navigator.hardwareConcurrency / 2).     │
│  Each worker: own WASM instance, own OPFS file handles.     │
│  Idle workers sit in the pool waiting for ops.              │
│  Operations queue when pool is full.                        │
│                                                             │
│  PER WORKER:                                                │
│  ┌────────────────────────────────────────────────────┐     │
│  │ WASM INSTANCE (pdf_oxide compiled to WASM)         │     │
│  │                                                    │     │
│  │ JsCallbackReader:                                  │     │
│  │   readFn(offset, count) → syncHandle.read(buf,     │     │
│  │     {at: offset}) — synchronous, from OPFS disk    │     │
│  │                                                    │     │
│  │ Output: write to OPFS file, stream chunks back     │     │
│  │   to main via postMessage                          │     │
│  │                                                    │     │
│  │ Cancel: Worker.terminate() — instant kill.          │     │
│  │   WASM linear memory freed by browser.             │     │
│  │   WASM instance IS the arena. Terminate = drop.    │     │
│  └────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 How a read works (JsCallbackReader via OPFS)

```
MAIN THREAD                              WEB WORKER
    │                                        │
    │  1. Stream PdfSource to OPFS:          │
    │     for each 256KB chunk:              │
    │       source.readAt(offset, 256KB)     │
    │       postMessage('opfs.write',        │
    │         {filename, chunk, offset})      │
    │       ──────────────────────────►       │
    │       await response                   │  worker writes to OPFS
    │                                        │  via SyncAccessHandle.write
    │     postMessage('opfs.finalize')       │
    │     ──────────────────────────►         │  flush + close write handle
    │                                        │
    │  2. Send operation:                    │
    │     postMessage({op, opfsFile})        │
    │     ──────────────────────────►         │
    │                                        │  3. Open OPFS read handle:
    │                                        │     syncHandle = createSyncAccessHandle()
    │                                        │
    │                                        │  4. Create JS read functions:
    │                                        │     readFn = (offset, count) => {
    │                                        │       buf = new Uint8Array(count)
    │                                        │       syncHandle.read(buf, {at: offset})
    │                                        │       return buf
    │                                        │     }
    │                                        │     ← SYNCHRONOUS. From disk. Not RAM.
    │                                        │
    │                                        │  5. WasmPdfDocument.fromReader(readFn, lengthFn)
    │                                        │     Engine reads xref (few KB)
    │                                        │     Engine reads page objects (few KB each)
    │                                        │     NEVER reads the full file
    │                                        │
    │                                        │  6. Operation completes
    │                                        │     Close read handle
    │                                        │     Delete OPFS temp file
    │  7. Receive result                     │
    │     ◄──────────────────────────         │  postMessage(result)
```

The full file lives in OPFS (browser disk storage). Only the ranges
the engine needs enter WASM memory. A 500MB PDF might read 3KB total.

### 5.3 How output streaming works (web)

```
WEB WORKER                               MAIN THREAD
    │                                        │
    │  Engine saves — produces chunks        │
    │                                        │
    │  Chunk 1 (header + objects):           │
    │  postMessage({type:'chunk',            │
    │    data: chunk1.buffer}, [transfer])   │
    │  ──────────────────────────────►        │  sink.write(chunk1)
    │                                        │
    │  Chunk 2 (more objects):               │
    │  postMessage({type:'chunk',            │
    │    data: chunk2.buffer}, [transfer])   │
    │  ──────────────────────────────►        │  sink.write(chunk2)
    │                                        │
    │  Chunk 3 (xref + trailer):             │
    │  postMessage({type:'chunk',            │
    │    data: chunk3.buffer}, [transfer])   │
    │  ──────────────────────────────►        │  sink.write(chunk3)
    │                                        │
    │  postMessage({type:'done'})            │
    │  ──────────────────────────────►        │  operation complete
```

Each chunk is transferred (zero-copy via `Transferable`).
The consumer's PdfSink receives chunks as they're produced.
No full output buffer in the worker.

### 5.4 Worker pool — session-based (pin operation to worker)

**The design principle:** one operation = one worker, start to finish.
No operation hops between workers. This mirrors native (one task = one
pool thread, start to finish) and eliminates the OPFS lock conflict
(SyncAccessHandle takes an exclusive lock per file — if write and read
go to different workers, the read worker can't open the handle).

The pool manages worker SESSIONS, not individual messages. A session
is a dedicated worker for one operation's entire lifecycle.

```dart
class WebWorkerPool {
  final int size;  // max(2, hardwareConcurrency ~/ 2)
  final _idle = <WebWorkerSession>[];
  final _queue = Queue<Completer<WebWorkerSession>>();
  final _busy = <int, WebWorkerSession>{};  // opId → session (for cancel)
  final _opfs = OpfsRegistry();
  int _totalCreated = 0;

  /// Acquire a dedicated worker for one operation.
  /// If all workers are busy, the caller waits in the queue.
  Future<WebWorkerSession> acquire() async {
    if (_idle.isNotEmpty) return _idle.removeLast();
    if (_totalCreated < size) return _createSession();
    final c = Completer<WebWorkerSession>();
    _queue.add(c);
    return c.future;
  }

  /// Release the worker back to the pool after the operation finishes.
  void release(WebWorkerSession session) {
    if (_queue.isNotEmpty) {
      _queue.removeFirst().complete(session);
    } else {
      _idle.add(session);
    }
  }

  WebWorkerSession _createSession() {
    _totalCreated++;
    final worker = web.Worker(workerUrl, WorkerOptions(type: 'module'));
    return WebWorkerSession(worker);
  }

  void cancel(int opId) {
    final session = _busy.remove(opId);
    if (session != null) {
      session.terminate();
      _totalCreated--;  // dead worker, don't count it
    }
  }

  void dispose() {
    for (final s in _busy.values) s.terminate();
    for (final s in _idle) s.terminate();
    _busy.clear();
    _idle.clear();
    _totalCreated = 0;
    _opfs.releaseAll();
  }
}

/// A dedicated worker for one operation. All messages go to the same
/// worker — guaranteed sequential, same OPFS context, same WASM instance.
class WebWorkerSession {
  final web.Worker _worker;
  WebWorkerSession(this._worker);

  Future<Map<Object?, Object?>> send(String op, Map<String, Object?> args);
  void terminate() => _worker.terminate();
}
```

**Usage in WebBridge:**

```dart
Future<PdfDoc> open(PdfSource source, {String? password}) async {
  final session = await _pool.acquire();
  try {
    // All on the same worker — OPFS handle stays within one context:
    final filename = _pool._opfs.register();
    await _streamSourceToOpfs(session, source, filename);
    final result = await session.send('open', {'opfsFile': filename, 'password': password});
    return _decodeDoc(result);
  } finally {
    _pool.release(session);
  }
}
```

**Why this fixes the OPFS lock problem:**

```
SAME WORKER for the entire operation:
  1. session.send('opfs.write', chunk1)  → Worker A writes
  2. session.send('opfs.write', chunk2)  → Worker A writes
  3. session.send('opfs.finalize')       → Worker A flushes
  4. session.send('open', {opfsFile})    → Worker A opens (same file, same handle context)

  Worker A never releases the lock to another worker.
  In fact — the worker can keep ONE SyncAccessHandle open for the entire
  operation: write chunks → flush → seek to 0 → read ranges. No close+reopen.
```

**The single-handle optimization:**

```javascript
// worker.js — one SyncAccessHandle for the entire operation lifecycle
const handle = await fileHandle.createSyncAccessHandle();

// Phase 1: receive chunks from main, write to OPFS
let writeOffset = 0;
for (const chunk of incomingChunks) {
  handle.write(chunk, { at: writeOffset });
  writeOffset += chunk.length;
}
handle.flush();

// Phase 2: engine reads from same handle (no close+reopen)
const readFn = (offset, count) => {
  const buf = new Uint8Array(count);
  handle.read(buf, { at: offset });
  return buf;
};
const doc = WasmPdfDocument.fromReader(readFn, () => handle.getSize());

// Phase 3: operate, produce result
// ...

// Phase 4: cleanup (operation done)
handle.close();
await opfsRoot.removeEntry(filename);
```

One handle. One worker. Zero lock conflicts. Zero race conditions.
No timing gaps between close and reopen. No "readwrite-unsafe" hacks.

**Symmetry with native:**

| Native | Web |
|---|---|
| Pool thread picks up task from channel | `pool.acquire()` returns a session |
| Thread runs entire operation | Session sends all messages to one worker |
| Thread's CallbackReader reads via condvar | Worker's SyncAccessHandle reads from disk |
| Thread's CallbackWriter writes via condvar | Worker streams chunks back via postMessage |
| Thread returns to pool loop | `pool.release(session)` |
| Cancel: set flag + signal condvar | `session.terminate()` |

Same architecture. Same guarantees. Different runtime.

### 5.5 OPFS cleanup registry

```dart
class OpfsRegistry {
  final _files = <String>{};
  int _counter = 0;

  String register() {
    final name = '_pdf_${_counter++}.tmp';
    _files.add(name);
    return name;
  }

  Future<void> release(String name) async {
    _files.remove(name);
    // Tell worker to delete the file (fire-and-forget)
    // If worker is dead, the file stays — cleaned up on dispose
  }

  Future<void> releaseAll() async {
    // Delete ALL registered files from OPFS
    // Used on dispose()
    for (final name in _files) {
      // opfs.removeEntry(name) — ignore errors
    }
    _files.clear();
  }
}
```

### 5.6 Dispose on web (instant kill)

```
Consumer calls pdf.dispose():

  1. Worker pool terminates ALL workers (Worker.terminate())
     Each worker's WASM linear memory is freed by the browser.
     The WASM instance IS the arena — terminate = drop everything.

  2. OPFS registry cleans ALL temp files.

  3. Done. Zero workers. Zero WASM memory. Zero OPFS files.
```

`Worker.terminate()` is Chrome's tab-kill button. The browser owns
the worker's memory. The browser reclaims it. No cleanup code runs
inside the worker. No leaks possible.

---

## 6. Symmetry table

| Feature | Native | Web |
|---|---|---|
| **Worker pool** | Rust thread pool (`available_parallelism() / 2`) | Web Worker pool (`hardwareConcurrency / 2`) |
| **Pool thread type** | raw pthread (not Dart isolate) | Web Worker (own WASM instance) |
| **Input streaming** | CallbackReader: condvar + NativeCallable.listener | JsCallbackReader: OPFS SyncAccessHandle |
| **Output streaming** | CallbackWriter: condvar + NativeCallable.listener | OPFS write + postMessage chunks |
| **Per-item streaming** | WorkerStreamItem via Dart_PostCObject | postMessage per item |
| **Cancel running op** | Cancellation flag + condvar signal | Worker.terminate() |
| **Dispose (kill all)** | Cancel all + pool shutdown + pthread_cancel + arena drop | Terminate all workers + OPFS cleanup |
| **Memory sandbox** | bumpalo arena per operation | WASM linear memory per worker |
| **Force-kill cleanup** | Arena drop (bumpalo) — all memory freed | Browser frees WASM memory on terminate |
| **No full-file buffer** | CallbackReader reads ranges via condvar | JsCallbackReader reads ranges from OPFS |
| **No output buffer** | CallbackWriter streams chunks via condvar | postMessage streams chunks |
| **Pool sizing** | `available_parallelism() / 2` | `hardwareConcurrency / 2` |
| **Queuing when full** | crossbeam bounded channel | Dart Queue in WebWorkerPool |
| **Read timeout** | `pthread_cond_timedwait(30s)` | OPFS reads are local disk — no timeout needed |
| **Temp file cleanup** | N/A (no temp files) | OpfsRegistry tracks + cleans on error/dispose |

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
| **B17** | Wire `WebBridge.open` end-to-end test (Dart → OPFS → WASM → result) | 0/3 (asset server needed) | **BLOCKED** |
| **B19** | Wire PdfEditor + PdfBuilder through PdfBridge (persistent handles) | | **DONE** |
| | **Editor (native):** | | |
| | Rust: 7 FFI functions (`bridge_editor_open/mutate/save/dispose/page_count/get_metadata/get_page_media_box`) | compiles | **DONE** |
| | Dart FFI: 7 bindings in `bridge_bindings.dart` | compiles | **DONE** |
| | Worker dispatch: `editorOpen/Mutate/Save/Dispose/PageCount/GetMetadata` cases | compiles | **DONE** |
| | `NativeBridge.openEditor` → SourceServer → worker → Rust pool → handle map | compiles | **DONE** |
| | `_NativeEditorHandle`: ALL 28 mutations + save + metadata + pageMediaBox + extractPages + mergeFrom | compiles | **DONE** |
| | **Builder (native):** | | |
| | Rust: `BUILDER_HANDLES` global map + `BuilderState` (Option\<DocumentBuilder\> + buffered ops) | compiles | **DONE** |
| | Rust: `BuilderPageOp` enum (17 variants: font, text, heading, paragraph, image, form fields, etc.) | compiles | **DONE** |
| | Rust: `replay_page_ops` — replays buffered ops against real `FluentPageBuilder` on save | compiles | **DONE** |
| | Rust: `bridge_builder_create`, `bridge_builder_set_metadata` (take-apply-put for consuming API) | compiles | **DONE** |
| | Rust: `bridge_builder_add_page` (A4/Letter/Custom), `bridge_builder_page_op` (17 op codes) | compiles | **DONE** |
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
| **B20** | Wire Layer 1 `Pdf` class to PdfBridge via bridge_factory | | NOT STARTED |
| **B21** | Delete old code (`lib/src/platform/`, old `lib/src/ffi/`, etc.) | | NOT STARTED |
| **B22** | Full test suite (all 327+ tests pass on new bridge) | | NOT STARTED |
| **B23** | Docs (ARCHITECTURE.md, CAPABILITY_ROADMAP.md, README.md) | | NOT STARTED |

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

### What's next

1. **B20**: Wire Layer 1 `Pdf` class to PdfBridge via bridge_factory.
2. **B21**: Delete old code in one cut.
3. **B17**: Fix web test infrastructure (asset server for worker.js + WASM).
4. **B22-B23**: Full test suite + docs.

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
13=pushButton, 14=signatureField, 15=newline, 16=newPageSameSize, 17=done.

The builder's consuming API (`title(self) -> Self`) is handled by `Option<DocumentBuilder>`
with take-apply-put: `state.builder.take()` → apply method → `state.builder = Some(result)`.
Same pattern used by the old FFI's `FfiDocumentBuilder`.

On the Dart side, `_NativeBuilderHandle` routes metadata/addPage/save/dispose through the
worker. `_NativePageBuilderHandle` encodes page ops as binary params (f32 coordinates,
UTF-8 strings, Uint8List secondary for image bytes) and sends via `builderPageOp` worker op.
`_WebBuilderHandle` and `_WebPageBuilderHandle` route through the same worker.js `builder.*`
and `page.*` message dispatch that already exists from the old code.

Source and test file layouts are in §2 (single source of truth).
Old code at `lib/src/platform/`, `lib/src/ffi/`, `lib/src/core/`,
`lib/src/document/`, `lib/src/editor/`, `lib/src/builder/` is untouched
(302/302 old tests pass). Will be deleted in B21 after new bridge is wired.

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
| Dart native bridge: open | 5/5 | Pass |
| Dart native bridge: open (additional) | 6/6 | Pass |
| Dart native bridge: merge | 4/4 | Pass |
| Dart native bridge: structural (delete, rotate, flatten, compress) | 4/4 | Pass |
| Dart native bridge: extract (text + markdown) | 3/3 | Pass |
| Dart native bridge: stream (render + extractImages) | 3/3 | Pass |
| Dart old tests (untouched, old code path) | 273/273 | Pass |
| Dart web bridge | 0/? | Blocked (asset server) |
| Dart shared contract | — | Not yet written |
| Dart API tests | — | Not yet written |
| **Total Dart passing** | **298** (25 new bridge + 273 old) | |
| **Total Rust passing** | **27** | |
| **Grand total** | **325** | |
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

> **Thread pool of raw pthreads (native) or Web Workers (web). Engine reads
> via condvar+listener (native) or OPFS SyncAccessHandle (web). Engine
> writes via condvar+listener (native) or postMessage chunks (web). Arena
> allocator per operation (native) or WASM linear memory per worker (web).
> Cancel via flag+signal (native) or Worker.terminate() (web). Dispose
> kills everything instantly. Zero full-file buffers. Zero leaks. Zero
> stuck threads. Zero UI jank.**
