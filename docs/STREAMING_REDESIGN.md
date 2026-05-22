# Streaming I/O Redesign

> The plan to eliminate memory peaks in pdf_manipulator. Clean break from
> the `Uint8List` API. `PdfSource` for input, `PdfSink` for output,
> `Stream<T>` for per-item iteration. Zero `dart:io`. Zero helper wrappers.

---

## 1. The problem

Every operation loads the entire PDF into Dart's GC heap as `Uint8List`,
copies it across the isolate boundary via `TransferableTypedData`, and
pdf_oxide internally clones it again (`source_bytes = data.clone()`).
A 500MB merge peaks at 2-3GB. This is unacceptable.

The old v0 package (Android, iText7, method-channel) handled any file size
by passing file paths — the engine read from disk directly. Zero Dart memory.
The rewrite to cross-platform Rust + FFI + isolates lost that property.

---

## 2. What pdf_oxide actually supports

Investigated the Rust source. Findings:

### Input

- `PdfReader` is an internal enum wrapping `BufReader<Cursor<Vec<u8>>>`.
  Today it's memory-only, but the enum was designed for a file-backed variant.
- The xref parser, document parser, and all sub-parsers are generic over
  `Read + Seek`. They seek to specific offsets and read small amounts.
  They do NOT need the full file in contiguous memory.
- `from_bytes(data)` clones the input (`source_bytes = data.clone()`) so
  compliance operations can reconstruct a `DocumentEditor`. This clone is
  avoidable if we supply a reader that the editor can re-read from.
- `pdf_document_open(path)` calls `fs::read(path)` then `from_bytes` —
  it reads everything into memory. NOT memory-mapped.
- **Opportunity:** add a `PdfReader::Callback` variant that implements
  `Read + Seek` by calling back into the host (Dart) for bytes. The engine
  reads only what it needs. Dart never holds the full file.

### Output

- `write_full_to_writer(&mut (impl Write + Seek), &SaveOptions)` is the
  core save pipeline. It writes sequentially — header, objects, xref, trailer.
- The `Seek` bound is used ONLY for `stream_position()` — reading the
  current byte offset to record in the xref table. It NEVER seeks backward.
  A `PositionTracker<W: Write>` wrapper satisfies the bound without real seeking.
- `save_to_bytes()` wraps this with `Cursor<Vec<u8>>`. `save(path)` wraps
  with `BufWriter<File>`.
- **Opportunity:** add a C API function that writes to a callback.
  The host receives chunks as the engine produces them.

### Per-item iteration (images, pages, splits)

- Image extraction: `pdf_document_get_embedded_images` returns an opaque handle.
  `pdf_oxide_image_count` + `pdf_oxide_image_get_data(handle, index)` = per-image
  iteration. The Dart layer currently collects all into `List<PdfImage>`.
- Rendering: per-page render functions already exist. `renderAllPages` is
  the Dart layer batching them.
- Split: `document_editor_extract_pages_to_bytes` extracts a subset. Dart
  currently splits by calling this in a loop and collecting all results.
- **Opportunity:** yield one item at a time instead of collecting into lists.

---

## 3. The two interfaces

The package defines these. The consumer implements them. No `dart:io`.
Only `dart:typed_data` + `dart:async`.

```dart
/// Random-access byte source.
///
/// The engine calls readAt when it needs data — typically small reads
/// (a few KB) at specific offsets. For a 500MB PDF, the engine might
/// make ~100 readAt calls totaling <1MB to parse the structure, then
/// targeted reads for each page it processes.
///
/// Consumer implements this with whatever backing they have:
/// file, network (HTTP Range), cellar, IndexedDB blob, memory buffer.
abstract interface class PdfSource {
  /// Total size in bytes. Must be known upfront (PDFs need this for
  /// xref parsing). Sync because every backing store knows its size.
  int get length;

  /// Read [count] bytes starting at byte [offset].
  ///
  /// Returns exactly [count] bytes, or fewer only if offset + count
  /// exceeds length (last read at the end of the source).
  ///
  /// FutureOr: sync for memory-backed sources, async for file/network.
  FutureOr<Uint8List> readAt(int offset, int count);
}

/// Sequential byte sink.
///
/// The engine calls write with chunks as it produces output. Chunks
/// are typically 4KB-256KB depending on the PDF object being written.
/// Total call count for a typical save is O(object_count), not O(file_size).
///
/// Consumer implements this with whatever destination they have:
/// file, upload stream, cellar writeStream, memory builder.
abstract interface class PdfSink {
  /// Write a chunk of output bytes.
  ///
  /// Called multiple times in sequence. The engine never seeks backward —
  /// each write appends after the previous one. The consumer does not
  /// need to support random access.
  ///
  /// FutureOr: sync for memory-backed sinks, async for file/network.
  FutureOr<void> write(Uint8List chunk);
}
```

### Why `PdfSource.readAt(offset, count)` and not `Read + Seek`

Go's `io.ReaderAt` pattern. Stateless per call — no cursor, no `seek()`
then `read()`. Each call is independent. Reasons:

- Maps directly to `pread()` syscall (file I/O without shared cursor)
- Maps directly to `Blob.slice(offset, offset + count)` (web)
- Maps directly to HTTP `Range: bytes=offset-(offset+count-1)` (network)
- Maps directly to `Uint8List.sublistView(bytes, offset, offset + count)` (memory)
- Concurrent-safe — no shared cursor state between async reads
- The Rust side wraps it into a `Read + Seek` impl that tracks offset internally

### Why `PdfSink.write(chunk)` and not `Stream<List<int>>`

Symmetry with `PdfSource`. Both are interfaces the consumer implements.
The package calls the consumer — the consumer doesn't pull from the package.
This push model matches how the engine naturally works (it drives the I/O).

A `Stream<List<int>>` return would require the package to manage a
`StreamController`, buffer engine output, and handle backpressure.
The `PdfSink` callback model is simpler — the engine calls write when
it has data, the consumer handles it. If the consumer wants a stream,
they bridge in their implementation of `PdfSink` (one `StreamController`).

### Why `FutureOr` and not just `Future`

In-memory sources and sinks are synchronous. Wrapping `Uint8List.sublistView`
in a `Future` adds unnecessary overhead and microtask scheduling for the
common case of small PDFs. `FutureOr` lets memory-backed implementations
be zero-overhead sync, while file/network implementations return `Future`.

---

## 4. The API

Clean break. No backward compatibility layer. No `Uint8List` methods.
No convenience wrappers.

```dart
class Pdf {
  Pdf();

  // ── Inspect ───────────────────────────────────────────────
  Future<PdfDoc> open(PdfSource input, {String? password});
  Future<PdfInfo> probe(PdfSource input);

  // ── Structural ────────────────────────────────────────────
  Future<void> merge(List<PdfSource> inputs, PdfSink output);
  Future<void> split(PdfSource input, {required int every,
      required PdfSink Function(int index) createSink});
  Future<void> splitBySize(PdfSource input, {required int maxBytes,
      required PdfSink Function(int index) createSink});
  Future<void> extractPages(PdfSource input, PdfSink output,
      {required List<int> pages});
  Future<void> deletePages(PdfSource input, PdfSink output,
      {required List<int> pages});
  Future<void> reorderPages(PdfSource input, PdfSink output,
      {required List<int> order});

  // ── Content ───────────────────────────────────────────────
  Future<void> compress(PdfSource input, PdfSink output,
      {int imageQuality = 75, bool garbageCollect = true});
  Future<void> watermark(PdfSource input, PdfSink output,
      {required String text, List<int>? pages, double opacity = 0.3,
       double fontSize = 48, double rotation = 45,
       double r = 0.5, double g = 0.5, double b = 0.5});
  Future<void> watermarkPositioned(PdfSource input, PdfSink output,
      {required String text,
       required double x, required double y,
       required double width, required double height,
       List<int>? pages, double fontSize = 48, String? fontName,
       double rotation = 45, double opacity = 0.3,
       double r = 0.5, double g = 0.5, double b = 0.5,
       bool fixedPrint = false,
       double fixedPrintH = 0.0, double fixedPrintV = 0.0});
  Future<void> flattenForms(PdfSource input, PdfSink output);
  Future<void> applyRedactions(PdfSource input, PdfSink output);
  Future<void> embedFile(PdfSource input, PdfSink output,
      {required String name, required PdfSource fileData});
  Future<void> eraseRegions(PdfSource input, PdfSink output,
      {required int page, required List<PdfRect> regions});

  // ── Extraction ────────────────────────────────────────────
  Future<String> extractText(PdfSource input, {int? page});
  Future<String> toMarkdown(PdfSource input, {int? page});

  // ── Search ────────────────────────────────────────────────
  Future<List<SearchResult>> searchPage(PdfSource input,
      {required int page, required String query, String? password});
  Future<List<SearchResult>> searchAll(PdfSource input,
      {required String query, String? password});

  // ── Security ──────────────────────────────────────────────
  Future<void> encrypt(PdfSource input, PdfSink output,
      {required String ownerPassword, String userPassword = ''});
  Future<void> encryptFull(PdfSource input, PdfSink output,
      {required String ownerPassword, String userPassword = '',
       int algorithm = 3, bool allowPrint = true, bool allowPrintHq = true,
       bool allowModify = true, bool allowCopy = true,
       bool allowAnnotate = true, bool allowFillForms = true,
       bool allowAccessibility = true, bool allowAssemble = true});
  Future<void> decrypt(PdfSource input, PdfSink output,
      {required String password});
  Future<void> sign(PdfSource input, PdfSink output,
      {required PdfSource certificate, required String certificatePassword,
       String? reason, String? location});

  // ── Creation ───────────────────────────────────────────────
  Future<void> imagesToPdf(List<PdfSource> images, PdfSink output);

  // ── Rendering (per-page, not bulk) ────────────────────────
  Future<RenderedPage> renderPage(PdfSource input, int pageIndex,
      {String? password});
  Future<RenderedPage> renderPageFit(PdfSource input, int pageIndex,
      {required int width, required int height, String? password});
  Future<RenderedPage> renderPageThumbnail(PdfSource input, int pageIndex,
      {required int size, String? password});

  // ── Image extraction (per-image iteration) ────────────────
  Stream<PdfImage> extractImages(PdfSource input, int pageIndex);
  Stream<PdfImage> extractAllImages(PdfSource input);

  // ── Rendering (per-page iteration) ────────────────────────
  Stream<RenderedPage> renderAllPages(PdfSource input,
      {required int width, required int height, String? password});

  // ── Signatures ────────────────────────────────────────────
  Future<int> getSignatureCount(PdfSource input, {String? password});
  Future<List<PdfSignatureInfo>> getSignatures(PdfSource input,
      {String? password});
  Future<bool> verifySignatures(PdfSource input, {String? password});

  // ── Validation ────────────────────────────────────────────
  Future<({bool compliant, int errors, int warnings})> validatePdfA(
      PdfSource input, {int level = 2, String? password});
  Future<bool> validatePdfUa(PdfSource input,
      {int level = 1, String? password});

  // ── Encryption info ───────────────────────────────────────
  Future<({bool print, bool printHq, bool modify, bool copy,
      bool annotate, bool fillForms, bool accessibility, bool assemble})>
    getPermissions(PdfSource input, {String? password});
  Future<int> getEncryptionAlgorithm(PdfSource input, {String? password});

  // ── Lifecycle ─────────────────────────────────────────────
  void dispose();
}
```

### Split's `createSink` factory

Split produces multiple PDFs. The consumer provides a factory that creates
a new `PdfSink` for each split document. The factory receives the split index
so the consumer can name files or route output per-split.

```dart
await pdf.split(source, every: 10, createSink: (index) {
  // Consumer decides where each split goes
  return MyFileSink(File('split_$index.pdf').openWrite());
});
```

This avoids the package returning `List<Uint8List>` (all splits in memory)
or `Stream<Uint8List>` (ambiguous boundary between splits).

### `Stream<T>` for per-item iteration

Operations that produce multiple independent items (`extractImages`,
`extractAllImages`, `renderAllPages`) return `Stream<T>`. Each item is
yielded as soon as the engine produces it. The consumer processes one at
a time — no `List<PdfImage>` holding all images in memory.

### PdfEditor and PdfBuilder

```dart
class PdfEditor {
  static Future<PdfEditor> open(PdfSource input);

  // ... all mutation methods (unchanged signatures — they operate
  //     on the engine's internal state, not on bytes) ...

  Future<void> save(PdfSink output);
  Future<void> saveEncrypted(PdfSink output, {required String ownerPassword});

  void dispose();
}

class PdfBuilder {
  static Future<PdfBuilder> create();

  // ... all builder methods (unchanged) ...

  Future<void> save(PdfSink output);

  void dispose();
}
```

The editor takes `PdfSource` to open. Save takes `PdfSink`. Between open
and save, all mutations are in-engine (no bytes cross the boundary).

---

## 5. The isolate bridging — how `PdfSource` works across boundaries

The hard problem: `PdfSource` lives on the main isolate. The engine runs
on the worker isolate. When Rust calls `read(offset, count)`, it's a
synchronous C call. Dart's main isolate is async. How do we bridge?

### Native — the `Dart_ExitIsolate` + mutex pattern

When the worker's Rust code needs bytes via the callback reader:

1. Rust calls the C callback `read_fn(ctx, offset, buf, count)`.
2. The C callback is implemented by a Dart `NativeCallable.isolateLocal`.
3. Inside the callback (running on the worker's Dart isolate):
   a. Post a `ReadRequest(offset, count)` message to the main isolate via `SendPort`.
   b. Call `Dart_ExitIsolate()` (releases the isolate so its event loop can process).
   c. Wait on a C mutex (pure C, not Dart — doesn't block the isolate).
   d. The main isolate receives the request, calls `PdfSource.readAt(offset, count)`,
      sends the bytes back via `SendPort`.
   e. A `ReceivePort` listener on the worker wakes up (because `Dart_ExitIsolate`
      let the event loop spin), signals the C mutex with the result bytes.
   f. The C callback re-enters the isolate with `Dart_EnterIsolate()`.
   g. Copies the result bytes into `buf`.
   h. Returns to Rust.

This is the same pattern `dart:io`'s synchronous `File.readSync` uses internally.
The key: `Dart_ExitIsolate` lets the worker's event loop process messages
while the C code blocks on the mutex. No deadlock.

### Web — OPFS SyncAccessHandle (disk-backed synchronous I/O)

OPFS (Origin Private File System) provides `FileSystemSyncAccessHandle`
— synchronous `read(buf, {at: offset})` inside Web Workers. Available
since March 2023 in ALL browsers (Chrome 102, Firefox 111, Safari 15.2,
Edge 102). MDN "Baseline Widely Available." This is what SQLite WASM
uses for its database files.

The flow:
1. Main thread streams consumer's `PdfSource` data into OPFS in chunks
   via `postMessage` to worker (only one chunk in flight at a time).
2. Worker writes chunks to OPFS via `SyncAccessHandle.write()`.
3. Once source is in OPFS, worker opens a read `SyncAccessHandle`.
4. Rust/WASM engine's `read_fn` calls JS glue → JS calls
   `syncHandle.read(buf, {at: offset})` — **synchronous, random-access,
   from disk not RAM.**
5. Engine reads exactly what it needs — xref (few KB), page objects
   (few KB each), never the full file.
6. Output: engine writes to a second OPFS file via `SyncAccessHandle`,
   worker streams chunks back to main via `postMessage`.
7. OPFS files cleaned up after operation.

```javascript
// In the Web Worker
const opfsRoot = await navigator.storage.getDirectory();
const handle = await opfsRoot.getFileHandle('input.pdf');
const sync = await handle.createSyncAccessHandle();

// Synchronous random-access read — from disk, not RAM
const buf = new Uint8Array(4096);
sync.read(buf, { at: 499 * 1024 });  // read 4KB at offset 499K
// buf now has the bytes. No async. No Promise. Instant.
```

**Properties:**
- Synchronous: `read()` and `write()` block (in worker, not main thread)
- Random access: `{at: offset}` parameter on every call
- Disk-backed: file lives in browser-managed storage, NOT in RAM
- No special headers: unlike SharedArrayBuffer, no COOP/COEP needed
- All browsers: Safari 15.2+ (Dec 2021), Chrome 102+ (Oct 2022),
  Firefox 111+ (Mar 2023)
- HTTPS required: standard for any modern web app

**The one trade-off vs native:** the source must be fully written to
OPFS before the engine can start random-access reading. On native, the
callback bridge lets the engine read on-demand from the first byte. On
web, there's a "stream to OPFS" phase (~few seconds for large files).
After that, engine performance is identical — synchronous random access.

**500MB PDF on 400MB RAM device:** the PDF lives in OPFS (browser
storage, disk-backed). Only the chunk in flight (~256KB) + engine
working set (~1-50MB) is in RAM. Works.

### Why `PdfSource` lives on the main isolate

The consumer creates `PdfSource` and passes it to the package as a
parameter. It's an interface — an object with state and behavior the
consumer defined. Dart can't send arbitrary objects across isolate
boundaries. The object stays where the consumer created it (main isolate).
The worker asks for bytes via message passing; the main isolate calls
`readAt` on the consumer's object and sends the result back.

The package is fully unaware of what's behind `PdfSource`. It just calls
`readAt` when the engine needs bytes. The consumer implements it their way.

---

## 6. Changes to pdf_oxide (Rust)

### Phase 1 — Callback reader

Add a new `PdfReader` variant backed by C function pointers:

```rust
// In src/document.rs — new PdfReader variant
enum PdfReader {
    Memory(BufReader<Cursor<Vec<u8>>>),
    Callback(CallbackReader),
}

struct CallbackReader {
    ctx: *mut c_void,
    read_fn: extern "C" fn(ctx: *mut c_void, offset: i64, buf: *mut u8, count: i64) -> i64,
    length_fn: extern "C" fn(ctx: *mut c_void) -> i64,
    position: u64,
}

impl Read for CallbackReader { ... }
impl Seek for CallbackReader { ... }
```

New C API:

```c
void* pdf_document_open_from_reader(
    void* ctx,
    int64_t (*read_fn)(void* ctx, int64_t offset, uint8_t* buf, int64_t count),
    int64_t (*length_fn)(void* ctx),
    int* error_code
);

void* document_editor_open_from_reader(
    void* ctx,
    int64_t (*read_fn)(void* ctx, int64_t offset, uint8_t* buf, int64_t count),
    int64_t (*length_fn)(void* ctx),
    int* error_code
);
```

### Phase 2 — Callback writer

Add a `PositionTracker<CallbackWriter>` that wraps a callback-based writer:

```rust
struct CallbackWriter {
    ctx: *mut c_void,
    write_fn: extern "C" fn(ctx: *mut c_void, data: *const u8, len: i64) -> i64,
}

struct PositionTracker<W: Write> {
    inner: W,
    position: u64,
}

impl<W: Write> Write for PositionTracker<W> { ... }
impl<W: Write> Seek for PositionTracker<W> {
    fn stream_position(&mut self) -> io::Result<u64> { Ok(self.position) }
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        // write_full_to_writer only calls stream_position(), never seek()
        unreachable!("pdf_oxide writer should not seek backward")
    }
}
```

New C API:

```c
int document_editor_save_to_writer(
    void* handle,
    void* writer_ctx,
    int64_t (*write_fn)(void* ctx, const uint8_t* data, int64_t len),
    int* error_code
);
```

### Phase 3 — Eliminate source_bytes clone

The `source_bytes = data.clone()` in `from_bytes` exists because:
- `compliance::convert_to_pdf_a` constructs a new `DocumentEditor` from `source_bytes`
- Some operations need to re-read the original bytes

With the callback reader, the engine can re-read from the source on demand.
The `source_bytes` field becomes unnecessary for callback-opened documents.
Add a `re_read_from_source` method that calls the reader instead of
cloning the entire file.

---

## 7. Changes to pdf_manipulator (Dart)

### Phase 1 — Interfaces + native reader bridge

1. Add `PdfSource` and `PdfSink` interfaces to `lib/src/core/`.
2. Export them from the barrel.
3. Add `NativeCallable`-based FFI reader in `bindings.dart`:
   - `NativeCallable.isolateLocal` for the `read_fn` callback
   - Mutex + `Dart_ExitIsolate` bridge for synchronous callback to main isolate
4. Add `NativeCallable`-based FFI writer in `bindings.dart`.
5. Update `PdfPlatform` interface — all methods take `PdfSource`/`PdfSink`.
6. Update `NativePdfPlatform` — bridge `PdfSource` to the callback reader.
7. Update `Op` enum and `WorkerMsg` for reader/writer message protocol.

### Phase 2 — Web bridge

1. Chunked transfer of `PdfSource` data to Web Worker.
2. Web Worker accumulates into WASM memory (not Dart heap).
3. `PdfSink` callbacks proxied via `postMessage` from worker to main.
4. Update `WebPdfPlatform` with the new protocol.

### Phase 3 — API surface

1. Rewrite `Pdf` class with `PdfSource`/`PdfSink` API (§4).
2. Rewrite `PdfEditor` and `PdfBuilder` open/save signatures.
3. Add `Stream<T>` returns for per-item operations.
4. Delete old `Uint8List` API entirely. Clean break.
5. Update barrel exports.

### Phase 4 — Tests and docs

1. Rewrite all tests with `PdfSource`/`PdfSink`.
2. Tests use their own in-memory `PdfSource`/`PdfSink` implementations
   inside `test/`. These are test utilities, NOT shipped in the package.
   They live in `test/helpers/` and are never exported.
3. Update `ARCHITECTURE.md` — new sections on streaming I/O, the bridge,
   the two interfaces.
4. Update `CAPABILITY_ROADMAP.md`.
5. Update example app — the example implements its own file-backed
   `PdfSource`/`PdfSink` to demonstrate the pattern.

---

## 8. Memory profiles — before and after

### Merge two 500MB PDFs

| Phase | Main isolate peak | Worker peak | Total |
|---|---|---|---|
| **Current** | ~1.5GB (two Uint8Lists + TransferableTypedData copies) | ~1.5GB (received bytes + pdf_oxide clone + output bytes) | ~3GB |
| **After (native)** | ~1MB (PdfSource callbacks, small chunks) | ~10-50MB (parsed xref table + object map + per-page structures; scales with object count, not file size) + output chunks streaming out via PdfSink | ~15-55MB |
| **After (web, OPFS)** | ~1MB (PdfSource on main, chunks stream to OPFS) | ~10-50MB (engine reads from OPFS SyncAccessHandle, disk-backed) | ~15-55MB |

### Extract text from 500MB PDF

| Phase | Main isolate peak | Worker peak |
|---|---|---|
| **Current** | ~500MB (Uint8List) + ~500MB (TransferableTypedData) | ~1GB (bytes + clone) |
| **After** | ~1MB | ~engine working set for text parsing (~10-50MB) |

### Extract all images from 100-page PDF with 50 images

| Phase | Main isolate peak |
|---|---|
| **Current** | All 50 images in `List<PdfImage>` simultaneously |
| **After** | One `PdfImage` at a time via `Stream<PdfImage>` |

---

## 9. What the consumer's code looks like

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';

// ── Merge two files (consumer implements source + sink) ────────
final pdf = Pdf();
await pdf.merge(
  [myFileSource('a.pdf'), myFileSource('b.pdf')],
  myFileSink('merged.pdf'),
);
pdf.dispose();

// ── Extract text (no sink needed — returns String) ──────────────
final text = await pdf.extractText(myFileSource('doc.pdf'));

// ── Edit and save ───────────────────────────────────────────────
final editor = await PdfEditor.open(myFileSource('doc.pdf'));
await editor.setTitle('Report');
await editor.save(myFileSink('edited.pdf'));
editor.dispose();

// ── Extract images one at a time ────────────────────────────────
await for (final image in pdf.extractImages(myFileSource('doc.pdf'), 0)) {
  // Process one image, then it's GC'd before the next one arrives
  await saveImage(image);
}

// ── Split — consumer decides where each split goes ──────────────
await pdf.split(myFileSource('big.pdf'), every: 10,
    createSink: (i) => myFileSink('split_$i.pdf'));
```

`myFileSource` and `myFileSink` are the consumer's implementations.
The package doesn't provide them. The package doesn't know what they do.

---

## 10. What the package does NOT ship

- No `BytesPdfSource` helper wrapping `Uint8List`
- No `FilePdfSource` helper wrapping `dart:io`
- No `BlobPdfSource` helper wrapping `dart:html`
- No `StreamPdfSink` helper wrapping `StreamController`
- No backward-compatible `Uint8List` API
- No deprecated methods
- No migration adapter

Clean break. The consumer implements two interfaces. That's the contract.

---

## 11. Phased execution order

| Phase | Scope | Depends on |
|---|---|---|
| **P0** | Define `PdfSource` + `PdfSink` interfaces in Dart. Export from barrel. | **DONE** |
| **P0.5** | Proof-of-concept: bridge verification. 16/16 tests pass (native FFI + web OPFS + memory profile). | **DONE** |
| **P1** | Rust: callback reader (`PdfReader::Callback`), C API `pdf_document_open_from_reader` + `_with_password`, `document_editor_open_from_reader`. | **DONE** |
| **P2** | Rust: callback writer (`CallbackWriter` + `PositionTracker<W>`), C API `document_editor_save_to_writer` + `_with_options`. | **DONE** |
| **P3** | Dart native: `SourceBridge` + `SinkBridge` via `NativeCallable.isolateLocal`. `StreamingSourceBridge` for cross-isolate reads via `Dart_ExitIsolate` + pthread condvar. | **DONE** |
| **P4** | Dart native: all ops use streaming. `_openEditorFromMsg` opens via `StreamingSourceBridge`. `_resolveBytes` reads from `SourceServer`. `_saveEditor` writes via `StreamingSinkBridge`. Zero `_readAll`, zero fallbacks on native. | **DONE** |
| **P5** | Dart web: OPFS `SyncAccessHandle` bridge. `_sendWithSource` streams >4MB to OPFS, worker reads via `resolveInputBytes`. `worker.js` supports `opfs.write` / `opfs.finalize` / `opfs.cleanup`. | **DONE** |
| **P6** | Dart: `PdfPlatform` interface rewritten — `PdfSource` input, `PdfSink` output, `Stream<T>` multi-item returns. Both `NativePdfPlatform` and `WebPdfPlatform` conform. | **DONE** |
| **P7** | Dart: `Pdf`, `PdfEditor`, `PdfBuilder` public API rewritten. Clean break — no backward compat. | **DONE** |
| **P8** | `Stream<T>` for `extractImages`, `extractAllImages`, `renderAllPages`. | **DONE** |
| **P9** | `source_bytes` clone eliminated — `from_callback_reader` leaves it empty. Automatic from P1. | **DONE** |
| **P10** | Tests (34 files), example app, docs (ARCHITECTURE, CAPABILITY_ROADMAP, README, AGENTS, CHANGELOG, MIGRATION) all updated. | **DONE** |

Each phase is independently mergeable. P0-P0.5 are proof. P1-P5 are
foundational. P6-P8 are the API rewrite. P9-P10 are optimization + polish.
Web (P5 — OPFS) and native (P3-P4 — NativeCallable) are built in
parallel — no platform is an afterthought.

---

## 12. Risks and mitigations

### Risk 1: the synchronous callback bridge

The bridge is the hardest piece on both platforms. P0.5 exists to
prove it works before anything is built on top. Specific risks:

**Native — `Dart_ExitIsolate` stability:**
`Dart_ExitIsolate` / `Dart_EnterIsolate` are Dart VM internal APIs.
Used by `dart:io` internally but not public API. Could change.
P0.5 tests this on the current Dart SDK. If it fails, the alternative
is `NativeCallable.listener` (Dart 3.1+ public API) which runs callbacks
on the isolate's event loop without blocking — but requires the Rust
FFI to yield between reads (cooperative async). P0.5 tests BOTH
approaches and picks the one that works.

**Web — OPFS SyncAccessHandle availability:**
`FileSystemSyncAccessHandle` is Baseline Widely Available (MDN) since
March 2023. Safari 15.2+ (Dec 2021), Chrome 102+ (Oct 2022), Firefox 111+
(Mar 2023). Used by SQLite WASM in production. Requires HTTPS (standard
for any modern web app). No COOP/COEP headers needed (unlike
SharedArrayBuffer). The one trade-off vs native: source must be fully
written to OPFS before the engine can start random-access reading —
adds a "stream to OPFS" phase for large files (~few seconds). After
that, engine performance is identical to native.

### Risk 2: round-trip latency

Each `readAt` is a round-trip between isolates/threads. For PDFs with
many small objects, this could be hundreds of round-trips.

Mitigations (built into the Rust `CallbackReader`, not Dart):
- **Read-ahead buffer:** when the reader reads offset N, request
  `max(count, 64KB)` from the host. Cache the extra bytes. Subsequent
  reads that fall within the buffer are served locally — zero round-trips.
  This is how every buffered reader in every language works.
- **Batch xref read:** on document open, the reader detects the
  `startxref` pointer (last 1KB of file), then reads the entire xref
  table in one call. The xref tells the engine where every object lives.
  Subsequent reads are for specific objects at known offsets.

---

## 13. The one-line summary

> **`PdfSource` (random-access reader) for input, `PdfSink` (sequential
> writer) for output, `Stream<T>` for per-item iteration. The engine
> reads what it needs via callback, writes chunks as it produces them.
> Memory is bounded by the engine's working set, not the file size.
> No `dart:io`. No `Uint8List` API. Clean break.**
