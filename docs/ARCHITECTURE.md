# pdf_manipulator — Architecture

How the package is wired. Three layers, two platforms, full streaming I/O.

For the bridge layer internals (thread pools, condvars, arena allocators,
shared buffers, OPFS) see [`BRIDGE_ARCHITECTURE.md`](BRIDGE_ARCHITECTURE.md).
For the public API (every type, method, parameter) see [`API_GOLD.md`](API_GOLD.md).
For what's shipped see [`CAPABILITY_ROADMAP.md`](CAPABILITY_ROADMAP.md).

---

## 1. The contract

- **Three layers.** Public API (Dart) → Bridge (Dart+Rust+JS) → Engine (Rust). Each layer has one job. No layer imports from a layer above it.
- **`PdfSource` in, `PdfSink` out.** The consumer implements two interfaces. The engine reads targeted ranges via callback (never the full file). The engine writes chunks as it produces them. No full-file buffers. No `dart:io`.
- **Thread pool, not isolate thread.** The Rust engine runs on raw pthreads in a fixed-size pool (native) or Web Workers in a pool (web). The Dart isolate never blocks. The UI never jank.
- **Arena allocator per operation.** Every operation gets a `bumpalo::Bump`. Drop the arena = free ALL engine memory. No leaks on cancel or force-kill.
- **Instant dispose.** Cancel flags + condvar signals wake sleeping threads. 100ms grace for cooperative exit. Force-kill as last resort. Arena drop cleans memory. Zero stuck threads.
- **Sealed types, not nulls.** `PdfPages.all()`, `PdfPages.single(0)`, `PdfPages.range(5, 10)` — compiler-enforced exhaustive handling. No magic numbers, no null-means-all.
- **One `save()` with `PdfSaveOptions`.** Compression, garbage collection, linearization, encryption — all in one options class. No separate `saveEncrypted`, `saveWithOptions`.
- **Stream\<T\> for multi-item ops.** `render()`, `extractImages()` yield items one at a time. Consumer processes one, GC collects it, next arrives. No list accumulation.
- **Vendor via submodule.** pdf_oxide is a git submodule at `vendor/pdf_oxide`, pointing at a fork with additive patches. Each patch has a removal trigger.

---

## 2. Source tree

```
lib/
├── pdf_manipulator.dart                 ← barrel (Layer 1 only)
└── src/
    ├── api/                             ← LAYER 1: Public API
    │   ├── pdf.dart                     ← Pdf (36 methods)
    │   ├── pdf_editor.dart              ← PdfEditor (33 methods)
    │   ├── pdf_builder.dart             ← PdfBuilder + PdfPageBuilder
    │   ├── pdf_source.dart              ← PdfSource interface
    │   ├── pdf_sink.dart                ← PdfSink interface
    │   └── types/                       ← sealed types, enums, params, errors
    │
    ├── bridge/                          ← LAYER 2: The plumbing
    │   ├── bridge.dart                  ← abstract PdfBridge
    │   ├── bridge_factory.dart          ← conditional import router
    │   ├── native/                      ← NativeBridge (isolate + Rust FFI)
    │   ├── web/                         ← WebBridge (Web Worker + OPFS + WASM)
    │   └── ffi/                         ← @Native FFI declarations
    │
    └── _internal.dart                   ← internal barrel

vendor/pdf_oxide/src/
├── bridge/                              ← Rust bridge module
│   ├── thread_pool.rs                   ← fixed-size pool + queue + cancel
│   ├── callback_reader.rs              ← Read+Seek via condvar
│   ├── callback_writer.rs              ← Write via condvar
│   ├── arena.rs                         ← per-operation bumpalo arena
│   ├── shared_buffer.rs                ← shared memory layout
│   └── ffi_api.rs                       ← C API for bridge operations
├── wasm.rs                              ← WASM API + JsCallbackReader
└── ...existing pdf_oxide engine code
```

---

## 3. The three layers

### Layer 1 — Public API (`lib/src/api/`)

What the consumer sees. Pure Dart. No FFI, no isolates, no threads.

- `PdfSource` / `PdfSink` — the two interfaces the consumer implements
- `Pdf` — one-shot operations (merge, split, extract, render, etc.)
- `PdfEditor` — open once, mutate many, save once
- `PdfBuilder` — create PDFs from scratch
- Sealed types: `PdfPages`, `PdfEncryptionAlgorithm`, `PdfExtractionFormat`
- Parameter groups: `PdfSaveOptions`, `PdfEncryptionConfig`, `PdfWatermarkStyle`

Layer 1 holds a `PdfBridge` and forwards every call. It doesn't know which bridge it has.

### Layer 2 — Bridge (`lib/src/bridge/`)

The plumbing. Two implementations of `PdfBridge`:

**NativeBridge** — worker isolate + Rust thread pool. Engine reads via `CallbackReader` (condvar + `NativeCallable.listener`). Engine writes via `CallbackWriter` (same mechanism). Results posted via `allo-isolate` (`Dart_PostCObject`). Shared buffers for the condvar dance. Arena allocator per operation.

**WebBridge** — Web Worker pool. PdfSource data streams to OPFS. Engine reads from OPFS `SyncAccessHandle` via `JsCallbackReader` (synchronous, disk-backed). Output streams back via `postMessage`. Cancel via `Worker.terminate()`. WASM linear memory IS the arena.

### Layer 3 — Engine (`vendor/pdf_oxide/`)

pdf_oxide. Parses PDFs, renders, extracts, edits. Knows nothing about Dart. Reads via `Read + Seek`. Writes via `Write`. Allocates via bumpalo arena. Pure Rust.

The `bridge/` module inside pdf_oxide provides the thread pool, callback reader/writer, arena wrapper, shared buffer layout, and C API. These are additive patches on the fork.

---

## 4. Data flow — native

```
Consumer calls pdf.merge([sourceA, sourceB], outputSink)
  ↓
Layer 1: Pdf forwards to bridge.merge(...)
  ↓
Layer 2: NativeBridge
  Main isolate: SourceServer for sourceA, SinkServer for outputSink
  Worker isolate: allocate shared buffers, create listeners, call Rust FFI
  ↓
Layer 3: Rust thread pool
  Pool thread: CallbackReader reads sourceA via condvar dance
    (only reads xref + page objects — few KB, not the full file)
  Pool thread: engine merges pages
  Pool thread: CallbackWriter streams output chunks via condvar dance
    (each chunk goes: engine → shared buffer → isolate → SinkServer → PdfSink)
  Pool thread: posts completion via Dart_PostCObject
  Pool thread: arena dropped — all engine memory freed
  ↓
Layer 2: result arrives at ReceivePort → WorkerResult → main isolate
  ↓
Layer 1: Future completes. Consumer's PdfSink has received all output chunks.
```

---

## 5. Data flow — web

```
Consumer calls pdf.merge([sourceA, sourceB], outputSink)
  ↓
Layer 1: Pdf forwards to bridge.merge(...)
  ↓
Layer 2: WebBridge
  Acquires a worker session from the pool
  Streams sourceA to OPFS in 256KB chunks
  Sends operation to worker
  ↓
Layer 3 (in Web Worker): WASM engine
  JsCallbackReader reads from OPFS SyncAccessHandle
    (synchronous, disk-backed, only reads what engine needs)
  Engine merges pages
  Output chunks posted back via postMessage (zero-copy transfer)
  ↓
Layer 2: receives chunks, calls outputSink.write(chunk) per chunk
  Releases worker session back to pool
  ↓
Layer 1: Future completes.
```

---

## 6. Off-main-thread guarantee

| Platform | Mechanism |
|---|---|
| Native | Worker isolate (Dart event loop) + Rust thread pool (raw pthreads). Engine runs on pool threads, never on isolate or UI thread. |
| Web | Web Worker pool. Each worker has its own WASM instance. Main thread only streams OPFS chunks and receives results. |

The UI thread never does PDF work on either platform. The Dart isolate's event loop never blocks — it only processes async listener callbacks.

---

## 7. Memory model

| Operation | Native peak | Web peak |
|---|---|---|
| 500MB PDF open | ~14MB (callbacks + arena working set) | ~50MB (OPFS on disk + WASM working set) |
| 500MB PDF merge | ~14MB | ~50MB |
| 50 image extraction | 1 image at a time (Stream\<PdfImage\>) | 1 image at a time |

No full-file buffers anywhere. The engine reads targeted ranges and writes chunks as it produces them.

---

## 8. Build system

Dual-path build hook (`hook/build.dart`):
- **Contributors** (have `vendor/pdf_oxide/` submodule): cargo compiles from source. Cargo handles incremental caching — fast when nothing changed. Rust source directory is in the hook's dependency list — changes trigger recompile automatically.
- **Consumers** (installed from pub.dev, no vendor/): downloads pre-built binary from GitHub Releases. Zero Rust toolchain required.

---

## 9. Test strategy

| Suite | Count | What it proves |
|---|---|---|
| Rust bridge unit + integration | 27 | Thread pool, shared buffer, condvar dance, arena, FFI API |
| Dart native bridge e2e | 44 | Full pipeline: Dart → isolate → Rust pool → engine → result |
| Dart Layer 1 API | 44 | Consumer-facing API (Pdf, PdfEditor, PdfBuilder) |
| **Total** | **115** | |

Edge cases tested: dispose during operation, failing PdfSource, failing PdfSink, slow PdfSource (100ms/500ms delay), hanging PdfSource + dispose, editor lifecycle, encrypted save, double dispose.

---

## 10. The one-line summary

> **Three layers (API / Bridge / Engine). PdfSource reads targeted ranges via condvar+listener (native) or OPFS SyncAccessHandle (web). PdfSink receives output chunks as the engine produces them. Thread pool of pthreads (native) or Web Workers (web). Arena allocator per operation. Instant dispose kills everything. 500MB PDF → ~14MB peak. Zero full-file buffers. Zero UI jank. Zero leaks.**
