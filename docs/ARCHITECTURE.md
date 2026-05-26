# pdf_manipulator — Architecture

How the package is wired. Four layers, two platforms, one shared dispatch, full streaming I/O.

---

## 1. The contract

- **Four layers.** Consumer API (Dart) → Transport (Dart+JS) → Host (Rust, per-platform thin shell) → Engine (Rust, pure). Each layer has one job. No layer imports from a layer above it.
- **One dispatch, two encoders.** `host/dispatch.rs` holds ALL operation logic — read, stream, and edit. Native encodes results as binary bytes (`ffi_encode.rs`). Web encodes as JS objects (`wasm_encode.rs`). Fix dispatch once, both platforms fixed.
- **`DataSource` in, `DataSink` out.** The consumer implements two interfaces. The engine reads targeted ranges via callback (never the full file). The engine writes chunks as it produces them. No full-file buffers. No `dart:io`.
- **Thread pool, not isolate thread.** The Rust engine runs on raw pthreads in a fixed-size pool (native) or Web Workers in a pool (web). The Dart isolate never blocks. The UI never janks.
- **Arena allocator per operation.** Every operation gets a `bumpalo::Bump`. Drop the arena = free ALL engine memory. No leaks on cancel.
- **Sealed types, not nulls.** `PdfPages.all()`, `PdfPages.single(0)`, `PdfPages.range(5, 10)` — compiler-enforced exhaustive handling.
- **One `save()` with `PdfSaveOptions`.** Compression, garbage collection, save mode (full rewrite or incremental), encryption — all in one options class.
- **Stream\<T\> for multi-item ops.** `render()`, `extractImages()` yield items one at a time. Consumer processes one, GC collects it, next arrives. No list accumulation.
- **Vendor via submodule.** pdf_oxide is a git submodule at `vendor/pdf_oxide`, pointing at a fork with additive patches.

---

## 2. The four layers

```
┌──────────────────────────────────────────────────────────────────┐
│                       CONSUMER API (Dart)                        │
│                                                                  │
│  pdf.dart            — standalone ops (open, sign, extract)      │
│  pdf_editor.dart     — open once, mutate many, save once         │
│  pdf_builder.dart    — create PDFs from scratch                  │
│  pdf_operations.dart — sugar (extractPages, reorderPages)        │
│                                                                  │
│  Calls PdfBridge (abstract). Platform-blind.                     │
└───────────────────────────────┬──────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│                       TRANSPORT (Dart + JS)                      │
│                                                                  │
│  Shared:  protocol/codec.dart    — encode requests + decode logic │
│           protocol/op.dart       — EngineOp enum                 │
│                                                                  │
│  Native:  native/bridge.dart       — NativeBridge                │
│           native/coordinator.dart  — Dart isolate (FFI dispatch) │
│           native/wire.dart         — binary → typed decoder      │
│                                                                  │
│  Web:     web/bridge.dart          — WebBridge                   │
│           web/wire.dart            — Map → typed decoder         │
│           coordinator.js           — JS Worker (routes ops)      │
│           worker.js                — JS Worker (runs WASM)       │
└───────────────────────────────┬──────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│                       RUST HOST LAYER                            │
│                                                                  │
│  Shared:  host/dispatch.rs         — ONE BRAIN, both platforms   │
│                                                                  │
│  Native:  host/native/ffi_api.rs   — C extern thin shell         │
│           host/native/ffi_encode.rs — result → binary            │
│                                                                  │
│  Web:     host/web/wasm_api.rs     — #[wasm_bindgen] thin shell  │
│           host/web/wasm_encode.rs  — result → JsValue            │
│                                                                  │
│  Native infra (no web equivalent):                               │
│           arena.rs, callback_reader.rs, callback_writer.rs,      │
│           shared_buffer.rs, thread_pool.rs                       │
│                                                                  │
│  Web types (no native equivalent):                               │
│           src/wasm.rs — WasmPdfDocument, WasmPdf, etc.           │
└───────────────────────────────┬──────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│                       PDF ENGINE (Rust, pure)                    │
│                                                                  │
│  document.rs, editor/, renderer/, search/, compliance/,          │
│  signatures/, converters/, extractors/                           │
│                                                                  │
│  Knows nothing about FFI, WASM, dispatch, bridges, or Dart.      │
│  Reads via Read+Seek. Writes via Write. Pure PDF work.           │
└──────────────────────────────────────────────────────────────────┘
```

---

## 3. Symmetry map

Every role has a named file on each platform. Where a role doesn't apply to a platform, the reason is noted.

| Role | Native | Web |
|---|---|---|
| **Bridge** (Dart ↔ Coordinator) | `native/bridge.dart` | `web/bridge.dart` |
| **Coordinator** (receives ops, dispatches to engine) | `native/coordinator.dart` (Dart isolate — dispatches FFI to Rust thread pool) | `coordinator.js` (JS Worker — routes to worker.js) |
| **Worker** (runs engine code) | Rust pthread (spawned by `host/native/thread_pool.rs`) | `worker.js` (JS Worker — loads WASM, executes ops) |
| **Wire decoder** (raw → typed results) | `native/wire.dart` (binary → Map → typed) | `web/wire.dart` (Map → typed) |
| **Rust API** (entry points from worker) | `host/native/ffi_api.rs` (C extern) | `host/web/wasm_api.rs` (#[wasm_bindgen]) |
| **Rust encoder** (dispatch result → wire format) | `host/native/ffi_encode.rs` (→ binary) | `host/web/wasm_encode.rs` (→ JsValue) |
| **Shared dispatch** | `host/dispatch.rs` | `host/dispatch.rs` |
| **Shared protocol** | `protocol/codec.dart` + `protocol/op.dart` | same files |
| **Platform I/O transport** | Rust: arena, condvar, shared_buffer, callback_reader/writer, thread_pool | JS: OPFS, SharedArrayBuffer, postMessage (in coordinator.js + worker.js) |
| **Platform type wrappers** | N/A — C FFI uses raw pointers directly | `src/wasm.rs` — WasmPdfDocument wraps PdfDocument for #[wasm_bindgen] |
| **Consumer API** | `ops/pdf.dart`, `pdf_editor.dart`, `pdf_builder.dart`, `pdf_operations.dart` | same files |

Both bridges call `wireDecodeXxx(raw)` → get typed Dart objects back. Neither bridge calls codec decode functions directly — the wire layer absorbs that internally. Web bridge imports codec for request encoding only (`openOp()`, `extractOp()`, etc.); native bridge doesn't import codec at all.

- **Native wire.dart**: binary bytes → Map (binary parsing) → codec → typed
- **Web wire.dart**: Map (already from JS) → codec → typed

Same public API on both sides. Same return types. Different internal work.

`codec.dart` has two roles: **request encoding** (`openOp()`, `extractOp()`, etc. — used by both bridges directly) and **response decoding** (`decodeOpenResult()`, etc. — used by wire.dart internally, never by bridges directly).

---

## 4. Source tree

```
lib/
├── pdf_manipulator.dart                    ← barrel
└── src/
    ├── ops/                                ← CONSUMER API
    │   ├── pdf.dart                        ← standalone ops + edit() + build()
    │   ├── pdf_editor.dart                 ← editor session
    │   ├── pdf_builder.dart                ← PDF creation from scratch
    │   └── pdf_operations.dart             ← sugar (extractPages, reorderPages, etc.)
    │
    ├── transport/                           ← TRANSPORT
    │   ├── pdf_bridge.dart                 ← abstract PdfBridge contract
    │   ├── create.dart                     ← conditional import router
    │   ├── _create_native.dart             ← returns NativeBridge
    │   ├── _create_web.dart                ← returns WebBridge
    │   │
    │   ├── protocol/                       ← shared by both bridges
    │   │   ├── codec.dart                  ← encode args, decode results
    │   │   └── op.dart                     ← EngineOp enum (31 values)
    │   │
    │   ├── native/                         ← native platform
    │   │   ├── bridge.dart                 ← NativeBridge
    │   │   ├── coordinator.dart            ← Dart isolate (FFI dispatch to Rust pool)
    │   │   ├── wire.dart                   ← binary → typed decoder (calls codec)
    │   │   ├── bindings.dart               ← dart:ffi function signatures
    │   │   ├── shared_buffer.dart          ← condvar shared memory helpers
    │   │   ├── source_server.dart          ← serves source bytes to Rust
    │   │   └── sink_server.dart            ← receives output bytes from Rust
    │   │
    │   └── web/                            ← web platform
    │       ├── bridge.dart                 ← WebBridge
    │       └── wire.dart                   ← Map → typed decoder (calls codec)
    │
    └── types/                              ← shared types
        ├── pdf_doc.dart, pdf_page.dart
        ├── pdf_params.dart                 ← PdfSaveOptions, PdfSaveMode, etc.
        ├── pdf_image.dart, pdf_error.dart
        └── ...

bin/
└── setup.dart                              ← `dart run pdf_manipulator:setup` — web asset installer

hook/
├── build.dart                              ← native build hook (cargo build or download pre-built)
└── asset_hashes.dart                       ← SHA256 hashes of pre-built binaries (generated)

tool/
├── build_wasm.sh                           ← compile Rust → WASM (`make wasm`)
├── compile_natives.sh                      ← compile for all 13 native targets
└── write_asset_hashes.dart                 ← generate asset_hashes.dart from a GitHub Release

dart_test.yaml                              ← test config: VM default, chrome-coi for Atomics tests

web_assets/                                 ← JS workers (web platform)
├── coordinator.js                          ← manages WASM worker pool
├── worker.js                               ← calls WASM methods
├── pdf_oxide.js                            ← wasm-bindgen generated
└── pdf_oxide_bg.wasm                       ← compiled WASM binary

vendor/pdf_oxide/src/
├── host/                                   ← HOST LAYER
│   ├── dispatch.rs                         ← shared dispatch (both platforms)
│   ├── constants.rs                        ← buffer sizes
│   │
│   ├── native/                             ← native host
│   │   ├── ffi_api.rs                      ← C extern entry points
│   │   ├── ffi_encode.rs                   ← dispatch result → binary
│   │   ├── arena.rs                        ← per-op bumpalo arena
│   │   ├── callback_reader.rs              ← Read+Seek via condvar
│   │   ├── callback_writer.rs              ← Write via condvar
│   │   ├── shared_buffer.rs                ← shared memory layout
│   │   └── thread_pool.rs                  ← fixed-size pool + cancel
│   │
│   └── web/                                ← web host
│       ├── wasm_api.rs                     ← #[wasm_bindgen] entry points
│       └── wasm_encode.rs                  ← dispatch result → JsValue
│
├── wasm.rs                                 ← WASM type wrappers (author code)
│                                             WasmPdfDocument, WasmPdf,
│                                             WasmCertificate, WasmDocumentBuilder
│
└── (engine: document.rs, editor/, renderer/, search/, ...)
```

---

## 5. Data flow — native `pdf.open(source)`

```
Pdf.open(source)
  │
  ▼
NativeBridge.open()
  Wraps source as SourceServer
  Sends [id, 'open', {sourcePort, sourceLength, password}] via SendPort
  │
  ▼
coordinator.dart (Dart isolate)
  Receives op on ReceivePort
  Allocates SharedReadBuffer + NativeCallable listener
  Calls bridgeSubmitOpen() via dart:ffi → submits to Rust thread pool
  │
  ▼
ffi_api.rs (Rust thread pool worker)
  Creates CallbackReader (condvar-based streaming I/O)
  Opens PdfDocument from reader
  Calls dispatch::open_document()
  │
  ▼
dispatch.rs (shared brain)
  Reads page count, version, encryption, metadata
  Returns typed OpenResult
  │
  ▼
ffi_api.rs
  Calls ffi_encode::encode_open() → binary Vec<u8>
  Posts via allo-isolate to Dart native port
  │
  ▼
coordinator.dart
  Receives Uint8List on result port
  Sends [id, resultBytes] via SendPort to main isolate
  │
  ▼
NativeBridge
  wireDecodeOpen(bytes) → PdfDoc (wire handles binary→Map→codec internally)
  Completes the Future<PdfDoc>
```

---

## 6. Data flow — web `pdf.open(source)`

```
Pdf.open(source)
  │
  ▼
WebBridge.open()
  If OPFS mode: pre-copies source to OPFS (256KB chunked postMessage)
  If Atomics mode: no pre-copy (reads go through SharedArrayBuffer)
  Posts {type:'submit', op:'open', args} to coordinator
  │
  ▼
coordinator.js (JS Worker)
  Assigns opId
  Acquires a worker from pool
  Forwards {type:'exec', opId, op:'open', args} to worker
  │
  ▼
worker.js (JS Worker, has WASM instance)
  Creates readerCtx (OPFS: SyncAccessHandle / Atomics: SAB + Atomics.wait)
  Calls WasmPdfDocument.fromReader(readFn, lengthFn, password)
  Calls doc.dispatchOpen() — synchronous WASM call
  │
  ▼
wasm_api.rs (#[wasm_bindgen])
  Locks inner PdfDocument mutex via with_doc()
  Calls wasm_encode::open_to_js()
  │
  ▼
wasm_encode.rs
  Calls dispatch::open_document()
  │
  ▼
dispatch.rs (same shared brain as native)
  Returns typed OpenResult
  │
  ▼
wasm_encode.rs
  Formats as JS object {pageCount, version, isEncrypted, ...}
  Returns JsValue
  │
  ▼
worker.js
  Posts {type:'result', result} to coordinator
  │
  ▼
coordinator.js
  Forwards {type:'result', opId, result} to Dart
  │
  ▼
WebBridge
  wireDecodeOpen(jsMap) → PdfDoc (wire calls codec internally)
  Completes the Future<PdfDoc>
```

---

## 7. The dispatch layer — one brain, two encoders

`host/dispatch.rs` contains every operation as a typed function. Read/stream ops return typed result structs. Edit ops take typed args and mutate the editor.

| Function | Returns |
|---|---|
| `open_document(doc)` | `OpenResult` (pages, version, encryption, metadata) |
| `extract_text(doc, page, format)` | `ExtractTextResult` (text) |
| `search_text(doc, query, page)` | `SearchResult` (hits with flat x,y,w,h) |
| `get_signatures(doc)` | `SignaturesResult` |
| `verify_signatures(doc)` | `bool` |
| `validate_pdf_a(doc, level)` | `ValidationResult` (compliant, errors, warnings) |
| `validate_pdf_ua(doc, level)` | `bool` |
| `classify_page(doc, page)` | `ClassificationResult` |
| `classify_document(doc)` | `ClassificationResult` |
| `plan_split_by_bookmarks(doc)` | `Vec<BookmarkSplit>` |
| `render_page(doc, page, w, h)` | `RenderedPage` (width, height, RGBA data) |
| `extract_images(doc, page)` | `Vec<ExtractedImage>` |

All page routing (all pages vs single page), result flattening (nested bbox → flat x,y,w,h), and engine method calls live here. Both platforms call the same function, get the same result, encode it differently:

- **Native:** `ffi_encode.rs` → binary `Vec<u8>` → Dart `native/wire.dart` → typed
- **Web:** `wasm_encode.rs` → `JsValue` object → Dart `web/wire.dart` → typed

Both `wire.dart` files call `codec.dart` internally. Bridges never import codec for decoding.

Step-by-step checklists for adding new read and edit operations are in [`UPDATING.md`](UPDATING.md) (§S3 and §S4). The wire_sync_test catches parity drift — if you add a dispatch function but miss the coordinator or worker.js case, the test fails.

---

## 8. Editor lifecycle

```
pdf.edit(source)
  → bridge.openEditor(source)
  → Rust: open PdfDocument, create DocumentEditor, return handleId
  → Dart wraps as PdfEditor(handleId)

editor.selectPages([0, 2, 5])
  → bridge.editorMutate(handleId, opCode=2, params)
  → Rust: editor.select_pages()

editor.addWatermark(...)
  → bridge.editorMutate(handleId, opCode=12, params)
  → Rust: editor.add_watermark()

editor.save(sink, options)
  → bridge.editorSave(handleId, sink, options)
  → Rust: editor writes via condvar writer (native) or JS callback (web)
  → chunks stream to DataSink

editor.dispose()
  → bridge.editorDispose(handleId)
  → Rust: drop editor + reader state
```

The editor holds its source reader alive for the session's lifetime. Mutations are lazy (modify the object tree in memory). Save materializes all mutations to output.

`PdfSaveMode.fullRewrite` (default): GC traversal + rewrite every object. Correct for heavy mutations. ~664ms on a 1000-page PDF.

`PdfSaveMode.incremental`: copy original bytes + append modified objects. ~43ms. Best when few objects changed.

---

## 9. Streaming I/O — how bytes move

No full-file buffers on either platform. The engine reads targeted ranges on demand and writes output in chunks as it produces them.

### DataSource and DataSink — the two consumer interfaces

The consumer implements two interfaces (`lib/src/types/data_source.dart`, `data_sink.dart`). The package calls them — the consumer doesn't pull from the package.

```dart
abstract interface class DataSource {
  int get length;                                     // total bytes, known upfront
  FutureOr<Uint8List> readAt(int offset, int count);  // random-access read
}

abstract interface class DataSink {
  FutureOr<void> write(Uint8List chunk);              // sequential append
}
```

**Why `readAt(offset, count)` not `Read + Seek`:** maps directly to `pread()` (file), `Blob.slice()` (web), HTTP `Range` (network), `Uint8List.sublistView` (memory). Stateless per call — no shared cursor, concurrent-safe. The Rust side wraps it into a `Read + Seek` impl (`CallbackReader`) that tracks position internally.

**Why `write(chunk)` not `Stream<List<int>>`:** push model matches how the engine works (it drives I/O). No `StreamController`, no backpressure management. Consumer wants a stream? Bridge it in their `DataSink` implementation.

**Why `FutureOr`:** memory-backed sources are synchronous. Wrapping `Uint8List.sublistView` in a `Future` adds microtask overhead for the common case. `FutureOr` lets memory be zero-overhead sync, file/network return `Future`.

### Native — condvar + shared memory

Three threads cooperate. The engine (Rust pthread) never touches the consumer's data directly — everything goes through shared memory with condvar synchronization.

**Reading (engine needs bytes from DataSource):**

```
Rust pthread (engine)              Dart coordinator isolate
    │                                     │
    │  1. Lock mutex                      │
    │  2. Write request to shared buf:    │
    │     offset=500, count=4096          │
    │  3. Call NativeCallable listener    │
    │     ──── fires on isolate loop ──►  │
    │  4. pthread_cond_timedwait (sleep)  │
    │     ...                             │  5. Listener reads request from buf
    │     ...                             │  6. Asks main isolate's SourceServer
    │     ...                             │  7. SourceServer calls source.readAt()
    │     ...                             │  8. Writes bytes to shared buf
    │     ...                             │  9. Sets READY flag, signals condvar
    │                                     │
    │  10. Wakes, reads bytes from buf    │
    │  11. Returns to engine              │
```

**Writing (engine produces output for DataSink):**

Same dance, reversed. Engine writes a chunk (up to 256KB) to shared memory, signals the isolate, blocks until ACK. The isolate forwards to main thread's SinkServer which calls `sink.write(chunk)`.

**Shared buffer layout:**

```
READ BUFFER (Dart calloc, 160 + 64KB):
  [0..8]    request_offset    i64
  [8..16]   request_count     i64
  [16..24]  response_length   i64
  [24..28]  flags             u32  (READY=1, ERROR=2, CANCELLED=4, ACK=8)
  [28..32]  padding
  [32..96]  pthread_mutex
  [96..160] pthread_condvar
  [160..]   data              64KB max

WRITE BUFFER (Dart calloc, 144 + 256KB):
  [0..8]    chunk_length      i64
  [8..12]   flags             u32  (same flag bits)
  [12..16]  padding
  [16..80]  pthread_mutex
  [80..144] pthread_condvar
  [144..]   data              256KB max
```

Both sides agree on this layout. The mutex protects concurrent access. The condvar is the sleep/wake handshake. 30-second timeout on both reads and writes prevents stuck threads.

**Thread pool:**

```
Pool size = max(2, available_parallelism / 2)
Bounded channel (depth 64) — submit blocks when full (backpressure)
Each worker thread loops: recv task → check cancel → create arena → run → drop arena
Shutdown: drop sender → workers see disconnected channel → exit loop → join
```

**Arena allocator (per-operation memory sandbox):**

Every operation gets a fresh `bumpalo::Bump` arena. All engine memory for that operation is allocated from the arena. When the operation ends — success, error, or cancel — the arena is dropped. ALL memory freed in one shot. No per-object cleanup.

```
Success:   arena created → engine runs → result extracted → arena dropped
Error:     arena created → engine fails → arena dropped
Cancel:    arena created → engine running → cancel flag set → engine returns Err → arena dropped
Timeout:   arena created → condvar times out (30s) → engine returns Err → arena dropped
```

**Result posting (Rust → Dart):**

Results cross back via `allo-isolate` — `Isolate::new(port).post(ZeroCopyBuffer(bytes))`. Thread-safe, callable from any pthread. `ZeroCopyBuffer<Vec<u8>>` transfers ownership to Dart as `Uint8List` with zero memcpy. The Dart coordinator receives the result on a `ReceivePort` and forwards to the bridge via `SendPort`.

**Per-item streaming (render, extractImages):**

The engine posts items one at a time via `allo-isolate`. Each item is a separate `Dart_PostCObject`. The coordinator forwards each to the bridge, which yields via `StreamController.add()`. One image/page in memory at a time — consumer processes one, GC collects it, next arrives.

```
Rust pthread                     Coordinator isolate              Main isolate
    │                                 │                               │
    │  Render page 1                  │                               │
    │  Post item bytes ──────────►    │  Forward ──────────────►      │
    │                                 │                               │ stream.add(page1)
    │  Render page 2                  │                               │ page1 GC eligible
    │  Post item bytes ──────────►    │  Forward ──────────────►      │
    │                                 │                               │ stream.add(page2)
    │  Post done marker ─────────►   │  Forward ──────────────►      │
    │                                 │                               │ stream.close()
```

### Web — two I/O modes

Detected at startup based on browser capabilities:

**Mode 1: Atomics (SharedArrayBuffer available)**

The coordinator creates a `SharedArrayBuffer` (8-byte header + 64KB data). Passed to the WASM worker at exec time. When the engine needs bytes:

```
WASM worker (worker.js)            Coordinator (coordinator.js)        Main (Dart)
    │                                     │                                │
    │  1. Atomics.store(status, 0)        │                                │
    │  2. postMessage({readAt,            │                                │
    │     offset, count, sab})            │                                │
    │  3. Atomics.wait(status, 0)  ──►    │                                │
    │     (blocks thread)                 │  4. postMessage({readAt})  ──► │
    │     ...                             │                                │ 5. source.readAt()
    │     ...                             │  ◄── readAtResponse(bytes) ──  │
    │     ...                             │  6. Write bytes to SAB         │
    │     ...                             │  7. Atomics.store(status, 1)   │
    │     ...                             │  8. Atomics.notify(status)     │
    │                                     │                                │
    │  9. Wakes, reads from SAB           │                                │
    │  10. Returns to WASM engine         │                                │
```

On-demand reads. No pre-copy. Lowest memory — only the SAB header + one chunk in flight.

**Requires COOP/COEP headers** on the consumer's server for `SharedArrayBuffer` to be available:
```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Without these headers, the browser blocks `SharedArrayBuffer` and the coordinator falls through to OPFS mode automatically.

**Mode 2: OPFS (no SharedArrayBuffer)**

Fallback when `SharedArrayBuffer` is unavailable (non-crossOriginIsolated contexts). Source bytes are pre-copied to OPFS disk before the operation starts:

```
Main (Dart)                        Coordinator                    WASM worker
    │                                     │                           │
    │  1. Read source in 256KB chunks     │                           │
    │  2. postMessage({opfs.write,        │                           │
    │     chunk, offset})            ──►  │                           │
    │                                     │  3. OPFS write(chunk)     │
    │  ◄── opfs.writeAck ──              │                           │
    │  ... repeat until EOF ...           │                           │
    │  postMessage({opfs.finalize}) ──►   │                           │
    │  ◄── opfs.finalizeAck ──           │                           │
    │                                     │                           │
    │  4. postMessage({submit, opfsFile}) │                           │
    │                                ──►  │  5. exec ──►              │
    │                                     │                           │ 6. SyncAccessHandle.read()
    │                                     │                           │    (reads from OPFS disk)
```

Pre-copy cost is paid once per operation. The WASM worker reads via `SyncAccessHandle.read()` — synchronous, disk-backed, no further Dart round-trips. OPFS temp files are cleaned up on worker release.

**Why the coordinator JS Worker exists:**

Without it, every `readAt` request and every output chunk fires a `postMessage` handler on the main thread's event loop, interleaved with widget builds and animations. 500 reads × 0.1ms = 50ms of jank. With the coordinator, the main thread only answers `readAt` calls (fast, async) and accepts write chunks (fast, async). All pool management, OPFS lifecycle, and routing happens on the coordinator's thread. Same protection as native's coordinator isolate.

**Output writing (both web modes):**

The WASM worker calls `writeFn(chunk)` which posts `{type:'chunk', data}` to the coordinator, which forwards to Dart's bridge. Dart calls `sink.write(bytes)`. Transfer-based — ArrayBuffer ownership is moved, not copied.

**Per-item streaming (both web modes):**

Same as native — one `postMessage` per image/page. Worker posts `{type:'item', data}` to coordinator, coordinator forwards to Dart, Dart yields via `StreamController.add()`. Worker posts `{type:'itemDone'}` at the end → stream closes. One item in memory at a time.

### Memory model

| Operation | Native peak | Web peak (Atomics) | Web peak (OPFS) |
|---|---|---|---|
| Open 500MB PDF | ~14MB (shared buf + engine working set) | ~64KB SAB + engine | source size on disk + engine |
| Render 1 page | ~14MB | ~64KB + 1 page RGBA | same |
| Extract 50 images | 1 image at a time | 1 image at a time | 1 image at a time |

No full-file buffers. Engine reads targeted ranges (xref + specific objects). Writes stream in chunks as produced.

### Cancel and dispose

**Native dispose sequence:**

```
1. Set cancelled=true on ALL active shared buffers
2. Signal ALL condvars (wake sleeping pool threads)
3. Send shutdown to thread pool (drop sender)
4. Wait 100ms for cooperative exit
5. Each woken thread: checks cancel → returns Err → arena dropped → exits
6. Pool threads exit naturally (30s timeout on condvar prevents stuck threads)
7. Free all shared buffers (calloc.free)
8. Kill coordinator isolate
Result: zero threads, zero native memory, zero handles
```

**Web cancel:** `Worker.terminate()` kills the JS execution context. WASM linear memory is freed instantly by the browser. The WASM instance IS the arena — `terminate()` = `drop(arena)`. No manual cleanup. If OPFS mode, temp file cleaned up by coordinator.

**Web dispose:** coordinator terminates ALL pool workers → cleans all OPFS temp files → main thread terminates the coordinator. Zero workers, zero WASM memory, zero OPFS files.

| | Native | Web |
|---|---|---|
| **Cancel one op** | AtomicBool flag → condvar wakes → engine returns Err → arena dropped | `Worker.terminate()` → WASM memory freed → OPFS cleaned |
| **Dispose all** | 8-step sequence above | Terminate all + OPFS cleanup |
| **Timeout** | `pthread_cond_timedwait` 30s on reads + writes | N/A — Atomics.wait blocks; OPFS reads are synchronous |
| **Arena cleanup** | `bumpalo::Bump` dropped → all chunks freed | Browser kills JS context → linear memory freed |

---

## 10. Off-main-thread guarantee (all three threads)

| Platform | Mechanism |
|---|---|
| Native | Coordinator isolate (Dart event loop) + Rust thread pool (raw pthreads). Engine runs on pool threads, never on the isolate or UI thread. |
| Web | Web Worker pool. Each worker has its own WASM instance. Main thread only streams OPFS chunks and receives results. |

The UI thread never does PDF work on either platform.

---

## 11. Platform differences (honest asymmetries)

Three threads on each platform. Same roles, different languages:

| Thread | Native | Web |
|---|---|---|
| **Main** (Dart, runs bridge) | `native/bridge.dart` | `web/bridge.dart` |
| **Coordinator** (receives ops, dispatches) | `native/coordinator.dart` (Dart isolate) | `coordinator.js` (JS Worker) |
| **Worker** (runs engine code) | Rust pthread (managed by `thread_pool.rs`) | `worker.js` (JS Worker + WASM) |

Honest asymmetries that can't be unified:

| Concern | Native | Web | Why different |
|---|---|---|---|
| Coordinator language | Dart isolate | JS Worker | Web can't run Dart isolates |
| Worker | Rust pthread (engine runs natively) | JS Worker loading WASM | Different runtimes |
| Wire format | Binary bytes | JS objects | C FFI speaks bytes, WASM speaks JsValue |
| Wire decoder | `native/wire.dart` (binary→Map→typed) | `web/wire.dart` (Map→typed) | Both return typed; native has extra binary step |
| I/O transport | Condvar + shared memory (Rust) | OPFS + SharedArrayBuffer (JS) | Different streaming mechanisms |
| Type wrappers | None needed | `src/wasm.rs` (WasmPdfDocument etc.) | C FFI uses raw pointers; #[wasm_bindgen] needs wrapper structs |
| FFI bindings | `bindings.dart` (dart:ffi) | N/A — JS calls WASM directly | Different binding mechanisms |
| Source/Sink servers | `source_server.dart`, `sink_server.dart` | N/A — handled in JS | Different I/O models |

---

## 12. Build system

Dual-path build hook (`hook/build.dart`):
- **Contributors** (have `vendor/pdf_oxide/` submodule): cargo compiles from source.
- **Consumers** (installed from pub.dev): downloads pre-built binary from GitHub Releases.

WASM build: `make wasm` runs `wasm-pack build` → produces `web_assets/pdf_oxide_bg.wasm` + `pdf_oxide.js`.

---

## 13. The one-line summary

> **Four layers (Consumer API / Transport / Rust Host / Engine). Three threads per platform (main → coordinator → worker). One shared dispatch.rs. Symmetric naming: bridge↔bridge, coordinator↔coordinator, wire↔wire, ffi_api↔wasm_api, ffi_encode↔wasm_encode. Native coordinator is a Dart isolate dispatching FFI to a Rust thread pool. Web coordinator is a JS Worker routing to a WASM Worker. Both wire.dart files return typed results. Neither bridge imports codec. Zero full-file buffers. Zero UI jank.**
