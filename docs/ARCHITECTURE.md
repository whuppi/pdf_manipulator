# pdf_manipulator — Architecture

How the package is wired. Four layers, two platforms, one binary
format end to end, O(1)-memory streaming I/O everywhere.

---

## 1. The contract

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

5. **O(1) memory.** All ops stream through fixed-size buffers
   (64KB read, 256KB write). A 7GB file uses the same memory as a
   7KB file. Every op. Both platforms. Enforced mechanically — test
   guards throw on any `readAt > 64KB` or `write > 256KB`.

6. **Upstream untouched.** `ffi.rs` (12,147 lines) and `wasm.rs`
   (7,210 lines) are upstream code. We never call them. We never
   modify them. Upstream merges stay clean.

7. **Thread pool, not isolate thread.** Rust thread pool with
   std::sync Mutex+Condvar (native) or Web Workers (web). Dart
   isolate never blocks. UI never janks.

8. **Patches are marked.** Every modification to upstream files
   carries a `── pdf_manipulator patch ──` boundary comment.

---

## 2. The four layers

**Layer 1 — Consumer API (Dart)**

    pdf.dart            — lifecycle, handle creation, ensureInit
    pdf_doc.dart        — read-only queries (PdfDoc handle)
    pdf_editor.dart     — mutations only (PdfEditor handle)
    pdf_builder.dart    — create from scratch (PdfBuilder)
    pdf_standalone.dart — source in, sink out, no handle
    pdf_sugar.dart      — one-shot convenience wrappers

    Platform-blind. Calls PdfBridge (abstract).
            |
            v
**Layer 2 — Shared Bridge (Dart)**

    shared_bridge.dart  — ONE bridge class, both platforms
    pdf_transport.dart  — PdfTransport interface
    protocol/
      binary_codec.dart — encode request, decode response
      codec.dart        — decode Map -> typed Dart objects
      op.dart           — EngineOp enum (35 wire names)

    Platform-blind. Encodes args to binary, sends via
    transport, decodes binary response.
            |
            v
**Layer 3 — Transport (Dart, per-platform)**

    Native:
      native_transport.dart — isolate + FFI
      coordinator.dart      — isolate entry, bridge_execute
      source_server.dart    — per-source condvar I/O
      sink_server.dart      — per-sink condvar I/O
      shared_buffer.dart    — condvar shared memory layout
      bindings.dart         — dart:ffi @Native binding

    Web:
      web_transport.dart    — JS Worker pool
      coordinator.js        — pool manager, handle pinning
      worker.js             — WASM exec, reader registry

    Both: move binary bytes. Route readAt/chunk callbacks.
    Zero PDF knowledge.
            |
            v
**Layer 4 — Rust**

    OUR CODE (src/host/ — entirely ours, not upstream):
      bridge_api.rs       — single entry, cfg-gated FFI + WASM
      binary_codec.rs     — parse request, encode response
      dispatch.rs         — every op as a typed function
      positioned_write.rs — Write + position tracking
      sign.rs             — O(1)-memory PDF signing
      image_optimizer.rs  — JPEG recompress
      font_optimizer.rs   — Standard 14 font unembedding
      constants.rs        — buffer sizes (64KB / 256KB)
      native/             — thread pool, Mutex+Condvar I/O
      wasm/               — JsCallbackReader/Writer

    UPSTREAM PATCHES (8 files, see §9)
    UPSTREAM (untouched): ffi.rs, wasm.rs, engine modules

---

## 3. Data flow

### Simple op (one source, no output)

```
pdf.open(source)
  ↓
SharedBridge.open()
  │  encodeRequest('open', {sourceLength: N})  →  Uint8List
  ↓
PdfTransport.execute(bytes, sources: [source], keepSources: {0})
  │
  ├── Native: isolate → bridge_execute via FFI
  └── Web:    postMessage → coordinator.js → worker.js → bridge_execute via WASM
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
PdfTransport.execute(bytes,
    sources: [otherPdf])     ← transport serves readAt by sourceIndex
  ↓
bridge_api.rs
  │  source[0] = otherPdf via readAt callback (64KB chunks)
  │  editor.merge_from_reader(BoxedReader for source[0])
  ↓
response bytes
```

---

## 4. Source tree

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
    ├── transport/                           ← BRIDGE + TRANSPORT
    │   ├── pdf_bridge.dart                 ← abstract PdfBridge + handle contracts
    │   ├── pdf_transport.dart              ← PdfTransport interface
    │   ├── shared_bridge.dart              ← ONE bridge, both platforms
    │   ├── create.dart                     ← conditional import router
    │   ├── _create_native.dart             ← → SharedBridge(NativeTransport)
    │   ├── _create_web.dart                ← → SharedBridge(WebTransport)
    │   │
    │   ├── protocol/
    │   │   ├── binary_codec.dart           ← binary request/response encoding
    │   │   ├── codec.dart                  ← typed request builders + response decoders
    │   │   └── op.dart                     ← EngineOp enum (35 wire names)
    │   │
    │   ├── native/
    │   │   ├── native_transport.dart       ← isolate + multi-source/sink servers
    │   │   ├── coordinator.dart            ← isolate entry point
    │   │   ├── bindings.dart               ← @Native FFI binding
    │   │   ├── shared_buffer.dart          ← condvar memory layout
    │   │   ├── source_server.dart          ← per-source condvar server
    │   │   └── sink_server.dart            ← per-sink condvar server
    │   │
    │   └── web/
    │       └── web_transport.dart           ← JS Worker pool + OPFS pre-copy
    │
    ├── types/                              ← shared types (all exported)
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
    │   └── search_result.dart              ← SearchResult
    │
    └── hook/                               ← build hook support (not consumer API)
        └── asset_hashes.dart               ← SHA256 hashes for pre-built binaries

web_assets/                                 ← committed in git, shipped to web
├── coordinator.js                          ← WASM worker pool + handle pinning
├── worker.js                               ← reader registry, 3 I/O modes, bridge_execute
├── pdf_oxide.js                            ← wasm-bindgen glue
└── pdf_oxide_bg.wasm                       ← WASM binary (one binary, all 3 modes)

hook/
└── build.dart                              ← binary-first → source-fallback → submodule-init

tool/
├── compile_rust.sh                         ← Rust → native / wasm / per-platform / both
├── stamp_release.sh                        ← 5 modes: --stamp-tag, --github-notes, (default),
│                                              --add-git-install, --add-pub-install
├── run_web_test.sh                         ← flutter drive wrapper with clean exit
└── chrome_with_sab.sh                      ← Chrome launcher with SharedArrayBuffer

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
│   ├── native/                             ← arena, thread pool, condvar I/O
│   │   ├── arena.rs, callback_reader.rs, callback_writer.rs
│   │   ├── shared_buffer.rs, thread_pool.rs
│   └── wasm/                               ← JS callback reader/writer
│       ├── js_reader.rs                    ← Read+Seek via host_read_at
│       └── js_writer.rs                    ← Write via host_write_chunk
│
├── compliance/fonts/                       ← 12 Liberation TTF files (SIL OFL)
├── ffi.rs                                  ← UPSTREAM (12,147 lines, untouched)
├── wasm.rs                                 ← UPSTREAM (7,210 lines, untouched)
└── (engine: document.rs, editor/, renderer/, search/, signatures/, ...)
```

---

## 5. The binary wire format

Same layout in both directions. Op args only — PDF bytes travel
through the transport's I/O channels, never through the codec.

```
Request:  [op_len: u8] [op: UTF-8] [field_count: u16 LE] [fields...]
Response: [status: u8]  [field_count: u16 LE] [fields...]
            status 0 → [msg_len: u32 LE] [msg: UTF-8]  (error)
            status 1 → fields                           (ok)

Field:    [key_len: u8] [key: UTF-8] [type: u8] [value]

Types:    0=null  1=i32  2=i64  3=f64  4=bool  5=string  6=bytes
          7=int_list  8=float_list  9=string_list  10=map_list
```

Op names are the EngineOp `.wire` strings (`"open"`, `"extract"`,
`"editorMutate"`, etc.). No numeric mapping to keep in sync — the
enum IS the protocol.

---

## 6. Streaming I/O

### The rule

O(1) memory on every path. No full-file buffers on either platform.
Memory bounded by buffer size (64KB read, 256KB write), never by
file size. Test guards enforce this mechanically — see §8.

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

Each source gets its own `CallbackReader` + condvar buffer on the
coordinator isolate. Each sink gets its own `CallbackWriter` + buffer.
Engine reads/writes via shared memory with condvar synchronization.
Dart serves source bytes from the main thread's DataSource.

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

**Web reader registry:** each source gets a unique `readerId` in
the worker's registry. `host_read_at` dispatches by `sourceIndex →
readerId` via an `activeReaderMap` built per exec. Pinned readers
(from handle-creating ops) survive across execs. Non-pinned readers
are cleaned in `finally`. Exactly symmetric with native's per-source
condvar servers.

### Backpressure

**Sources (input):** naturally backpressured on all platforms. The
engine calls `readAt` and blocks until the host returns bytes. The
engine never reads faster than the host can serve.

**Sinks (output):** backpressured per mode:

- **Native:** condvar shared buffer blocks the Rust writer when full.
  The Dart coordinator drains it before the writer can continue.
- **JSPI:** `host_write_chunk` returns a Promise. JSPI suspends the
  WASM stack. The coordinator forwards the chunk to Dart, Dart's
  sink consumes it, Dart acks. The coordinator resolves the Promise.
  WASM resumes. One chunk in flight at a time.
- **Atomics:** `host_write_chunk` posts the chunk then
  `Atomics.wait` on a dedicated write SAB. The coordinator forwards
  to Dart, Dart acks, coordinator `Atomics.store + notify`. Worker
  wakes. One chunk in flight at a time.
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

"O(1) memory" means the engine never buffers the full file — only
the backing store holds it. Each `readAt` returns at most 64KB.

### OPFS mode: O(N) disk + latency

OPFS is the universal fallback (what users get without COOP/COEP
headers on browsers older than Chrome 137). It pre-copies each
source file to OPFS disk before processing — O(N) disk space, O(N)
time before the first output byte, subject to browser storage quota.

Use `await pdf.ensureInitialized()` at startup to detect the mode.
If `PdfIoMode.opfs` is returned, consider warning the user or
prompting for COOP/COEP headers.

---

## 7. Instance architecture

```
Pdf()       = isolated engine instance (pool + handles + children)
PdfDoc      = read-only document session     (pdf.open)
PdfEditor   = mutation session               (pdf.edit)
PdfBuilder  = creation session               (pdf.build)
```

### Lifecycle

```dart
final pdf = Pdf();

// Eagerly init — returns detected I/O mode without running an op
final mode = await pdf.ensureInitialized();
print(mode);  // PdfIoMode.jspi, .atomics, .opfs, or .native

// Sessions are children of the engine
final doc = await pdf.open(source);
await doc.dispose();        // frees ONE doc

await pdf.dispose();        // frees EVERYTHING — cascades to all children
```

### Dispose rules

- `doc.dispose()` / `editor.dispose()` / `builder.dispose()` — frees
  one session. Others + engine stay alive.
- `pdf.dispose()` — cascades to all children. Zero Rust memory
  retained, zero threads alive.
- Double dispose is safe (no-op). Using a disposed object throws
  `StateError`.

### Parallel ops

All ops from one `Pdf` share its pool. Pool size adapts automatically
(`max(2, cores / 2)`). Multiple `Pdf` instances are fully independent.

### Platform internals

**Native:** all pool threads share one `InstanceState` (Mutex).
Created by `bridge_init()`, destroyed by `bridge_shutdown()`. Holds
HashMaps for documents, editors, builders, page ops, plus a cancel
flag and the thread pool.

**Web:** coordinator routes ops to WASM workers. Documents are pinned
to workers (one worker per open doc). Different documents on different
workers = truly parallel. Same document = same worker = serial.

---

## 8. Test architecture

### Rules

1. **Mirror source files.** `pdf_doc.dart` → `pdf_doc_test.dart`.
2. **Core + stress for every method.** Core = small fixtures (1–3
   pages). Stress = 1000-page PDFs.
3. **All 4 platforms run the same tests.** Shared test files define
   `registerXxxTests(Pdf Function() createPdf)`. Runners call them.
   Same pass count on native, OPFS, JSPI, and Atomics.
4. **Per-test timeouts.** Every `test()` has its own `timeout:`.
   Exceeds budget → fix the algorithm, never the timeout.
5. **O(1) guards always on.** `TestSource` throws on `readAt > 64KB`.
   `TestSink` throws on `write > 256KB`. Every test uses these.

### Structure

```
test/
├── helpers/
│   ├── asset_server.dart           HTTP server for web test assets
│   ├── fixtures.dart               Minimal PDFs, certs, byte data
│   ├── generators.dart             Build N-page PDFs at runtime
│   ├── photo_png.dart              128×128 test PNG
│   └── test_source_sink.dart       TestSource + TestSink (O(1) guards)
│
├── ops/
│   ├── core/                       7 core test files
│   ├── stress/                     6 stress test files (1000-page)
│   └── runners/                    4 platform runners
│       ├── native_runner_test.dart
│       ├── web_opfs_runner_test.dart
│       ├── web_jspi_runner_test.dart
│       └── web_atomics_runner_test.dart
│
├── transport/                      Unit tests (no engine)
│   ├── bridge_contract_test.dart
│   ├── shared_bridge_test.dart
│   ├── streaming_guard_test.dart
│   ├── native/shared_buffer_test.dart
│   └── protocol/
│       ├── binary_codec_test.dart
│       ├── codec_test.dart
│       └── wire_sync_test.dart     ← parity guard (see below)
│
└── types/                          3 type tests
```

### wire_sync_test — the parity guard

Parses `bridge_api.rs`, `worker.js`, and `coordinator.js` from disk.
Extracts every match/case arm **programmatically** — zero hardcoded
allowlists. Verifies:

- Every `EngineOp` has a Rust match arm
- Every Rust case maps to an `EngineOp` or a known sub-dispatch
- Edit sub-dispatch and page sub-dispatch are disjoint
- No sub-dispatch ops leak into top-level

Adding a Dart op without a Rust handler → test fails. Adding a Rust
handler without a Dart op → test fails.

---

## 9. Upstream patches

### pdf_oxide — 7 files patched

Every modification marked with `── pdf_manipulator patch ──`.

| File | What we added |
|---|---|
| `Cargo.toml` | `native-bridge` deps + `office_oxide` as path dependency |
| `lib.rs` | `pub mod host;` |
| `document.rs` | External reader variant, `from_external_reader()`, info/encryption accessors, `collect_refs_of()` (zero-clone GC), streaming `to_docx/pptx/xlsx_writer_flow()` |
| `editor/document_editor.rs` | State accessors, `merge_from_reader()`, zero-clone GC BFS, `stage_trimmed_pages_for_gc()`, `write_full_to_writer(PositionedWrite)`, `add_page_annotation()`, `all_media_boxes()` |
| `compliance/converter.rs` | Expose `convert_with_editor`, bundled 12 Liberation fonts |
| `writer/pdf_writer.rs` | `finish_to_writer(PositionedWrite)` — streaming save |
| `writer/document_builder.rs` | `build_to_writer(PositionedWrite)` + `assemble_writer()` |
| `converters/office/mod.rs` | Finalize-callback pattern for streaming, `convert_X_reader_to_writer()`, `ir_to_pdf_writer()` |

### office_oxide — streaming OPC writer

Branch `office_kit/0.1.2-patches`. Marked with `── office_kit patch ──`.

| File | Patch |
|---|---|
| `core/opc.rs` | `OpcWriter<W: Write>` with `ZipWriter::new_stream` (streaming ZIP, no Seek) |
| `core/editable.rs` | `write_to` uses streaming ZIP |
| 10 other files | `Write + Seek` → `Write` bounds throughout |

Everything in `src/host/` is entirely ours — no patch markers needed.

---

## 10. The one-line summary

> **Four layers. One binary format. Multi-source/multi-sink transport.
> Three web I/O modes auto-detected (JSPI > Atomics > OPFS), one WASM
> binary. O(1) memory on every path — enforced by test guards. Same
> tests, same pass count, all 4 platforms.**
