# SharedBridge Plan — Kill Bridge Asymmetry Forever

## The problem

Two bridge files hand-write the same logic separately:
- `native/bridge.dart` (1166 lines)
- `web/bridge.dart` (907 lines)

They drift silently. Known asymmetries found in audit:

| # | Bug | Impact |
|---|---|---|
| 1 | Web `_resolvePages()` calls `open()` before render/extractImages. Native sends page spec as bytes. | 2x open latency on web for streaming ops |
| 2 | Web makes SEPARATE call for each editor metadata getter. Native batches one call. | 6 round-trips on web vs 1 |
| 3 | Web timeout 15s. Native 60s. | Web times out on heavy ops |
| 4 | Native uses binary-opcode protocol. Web uses string-key protocol. | Two completely different coordinator protocols |
| 5 | Native editor keeps SourceServer alive for session. Web has no persistent source. | Different source lifecycle |
| 6 | optimizeImages/unembedStandardFonts/redactionCount route through editorQuery on native, editorMutate on web. | Same feature, different coordinator ops |

Root cause: **2073 lines of duplicated logic.**

## The goal

One SharedBridge. Both platforms use it. Platform-specific transport underneath.
All logic written ONCE. Drift structurally impossible.

## Principles preserved (non-negotiable)

- **Zero memory**: engine reads targeted ranges via callback, never full file
- **True streaming**: render/extractImages yield one item at a time, GC between items
- **Off-main-thread**: three-thread model intact (Main -> Coordinator -> Worker)
- **Condvar I/O (native)**: SharedReadBuffer/SharedWriteBuffer with pthread mutex/condvar
- **OPFS/SAB I/O (web)**: pre-copy or Atomics, detected at startup
- **Arena allocator**: per-op bumpalo Bump, drop = free all
- **30s condvar timeout**: prevents stuck threads
- **DataSource/DataSink push model**: consumer implements, engine drives

## Architecture after

```
Consumer API (Pdf, PdfEditor, PdfBuilder)
    |
    v
SharedBridge (ONE class, implements PdfBridge)
    |
    | Uses codec.dart EngineRequest for ALL encoding
    | Uses wire.dart for ALL result decoding
    | Owns: page resolution, metadata batching, timeouts, error handling
    |
    | Calls _transport.execute() or _transport.executeStream()
    |
    +-- NativeTransport (implements PdfTransport)
    |     Converts EngineRequest args Map -> binary protocol for coordinator.dart
    |     Converts editOp strings -> opCode ints
    |     Manages: isolate, SourceServer/SinkServer, SharedBuffer lifecycle
    |     Converts: binary Uint8List result -> Map via native/wire.dart
    |     Special: keeps SourceServer alive per editor handle
    |     Special: routes optimizeImages/unembedStandardFonts/redactionCount
    |              through editorQuery instead of editorMutate
    |
    +-- WebTransport (implements PdfTransport)
          Passes EngineRequest args Map through as-is to coordinator.js
          Manages: Worker spawn, OPFS/SAB, chunk forwarding, readAt fulfillment
          Returns: Map result as-is (already Map from JS)
          CHANGE: removes its own 15s result timeout — SharedBridge's 60s is authority.
          Keeps: 30s init timeout, 10s OPFS ack timeout (setup timeouts, not op timeouts).
```

## Transport interface

```dart
// lib/src/transport/transport.dart

abstract class PdfTransport {
  /// Execute a one-shot op.
  /// [input]: engine reads bytes from here (null for source-free ops).
  /// [output]: engine writes byte chunks here (null for read-only ops).
  /// Returns result as normalized Map<String, Object?>.
  /// Transport throws on engine errors — SharedBridge never gets error Maps.
  Future<Map<String, Object?>> execute(
    String op,
    Map<String, Object?> args, {
    DataSource? input,
    DataSink? output,
  });

  /// Execute a streaming op that yields multiple result items.
  Stream<Map<String, Object?>> executeStream(
    String op,
    Map<String, Object?> args, {
    DataSource? input,
  });

  /// Clean up all resources.
  Future<void> dispose();
}
```

Two methods + dispose. Transport knows nothing about PDF.
`DataSource` is just `readAt(offset, count)`. `DataSink` is just `write(chunk)`.
Error handling: transport throws exceptions, SharedBridge never sees error Maps.

**Timeout discipline:** SharedBridge wraps every `execute()`/`executeStream()` with
`.timeout(Duration(seconds: 60))`. This is the SINGLE timeout authority.
NativeTransport removes its current 60s `_send` timeout (added this session).
WebTransport removes its current 15s result timeout.
Both keep only infrastructure timeouts:
- Native: condvar 30s (prevents stuck Rust threads — infrastructure, not op level)
- Web: 30s init, 10s OPFS ack (setup failures, not op level)

## The two coordinator protocols (critical detail)

### Native coordinator (coordinator.dart) expects:

| Op type | argsMap shape |
|---|---|
| Read ops | `{sourcePort: SendPort, sourceLength: int, password, params: Uint8List?}` |
| Edit (source+sink) | `{sourcePort, sourceLength, sinkPort, params: Uint8List?, secondaries: List<Uint8List>?}` |
| Editor open | `{sourcePort, sourceLength, password}` |
| Editor mutate | `{handleId: int, opCode: int, params: Uint8List?, secondaries: List<Uint8List>?}` |
| Editor save | `{handleId, sinkPort, compress, garbageCollect, saveMode, encryptMode, ...}` |
| Editor query | `{handleId, queryCode: int, param: int}` |
| Editor metadata | `{handleId}` |
| Builder page ops | `{handleId, opCode: int, params: Uint8List?, secondary: Uint8List?}` |

**Key: integer opCodes + binary Uint8List params.**

### Web coordinator (coordinator.js -> worker.js) expects:

| Op type | args shape |
|---|---|
| Read ops | `{sourceLength, password}` |
| Editor mutate | `{handleId, editOp: string, ...extra}` (named Map fields) |
| Editor save | `{handleId, compress, garbageCollect, saveMode, encryptMode, ...}` |
| Editor metadata | `{handleId}` |
| Builder page ops | `{handleId, pageOp: string, ...extra}` |

**Key: string editOp/pageOp names + Map fields.**

### How SharedBridge speaks to both

SharedBridge uses codec.dart `EngineRequest(op, argsMap)` — the web-style Map format.

**NativeTransport** converts Map -> binary:
- `editOp` string -> opCode int via lookup table
- Map fields -> binary Uint8List via `_encode*` helpers
- ~370 lines of encoding (moved from current native/bridge.dart)

**WebTransport** passes Map through as-is.

## Every operation mapped

| Operation | SharedBridge sends | NativeTransport does | WebTransport does |
|---|---|---|---|
| open | `execute('open', {password}, input: source)` | SourceServer setup, binary result -> Map | OPFS/SAB, Map pass-through |
| extract | `execute('extract', {page, format, password}, input: source)` | Same pattern | Same |
| search | `execute('search', {query, page, password}, input: source)` | Same | Same |
| render | `executeStream('render', {pageIndices, maxWidth, maxHeight, password}, input: source)` | Binary items -> Map items | Map items pass-through |
| extractImages | `executeStream('extractImages', {pageIndices, password}, input: source)` | Same | Same |
| sign (both) | `execute('sign', {creds...}, input: source, output: sink)` — always op `'sign'` | NativeTransport checks args: if `certPem` present → sends `'signPem'` to coordinator (opCode 29), else `'sign'` (opCode 16). SourceServer + SinkServer. | worker.js `case 'sign'` checks `args.certPem` internally. Map pass-through. |
| editorOpen | `execute('editorOpen', {password}, input: source)` | SourceServer KEPT ALIVE per handle | OPFS one-time, source not kept |
| editorMutate | `execute('editorMutate', {handleId, editOp, ...extra})` | editOp -> opCode, Map -> binary params | Map pass-through |
| editorSave | `execute('editorSave', {handleId, ...saveOpts}, output: sink)` | SinkServer setup | chunk forwarding |
| editorDispose | `execute('editorDispose', {handleId})` | SourceServer cleanup | no-op |
| optimizeImages | `_mutate('optimizeImages', {quality})` — NativeTransport intercepts → editorQuery(4) | editorQuery FFI | editorMutate pass-through |
| unembedStandardFonts | `_mutate('unembedStandardFonts')` — NativeTransport intercepts → editorQuery(5) | editorQuery FFI | editorMutate pass-through |
| redactionCount | `_mutate('redactionCount', {page})` — NativeTransport intercepts → editorRedactionCount | separate FFI | editorMutate pass-through |
| builderPageOp | `execute('builderPageOp', {handleId, pageOp, ...extra})` | pageOp -> opCode, Map -> binary | Map pass-through |

**CRITICAL: optimizeImages/unembedStandardFonts/redactionCount**

These use DIFFERENT coordinator ops on each platform today:
- Native: `editorQuery` (separate FFI function, queryCode 4/5) or `editorRedactionCount`
- Web: `editorMutate` with string editOp

SharedBridge sends them as specific named ops. Each transport routes to its
platform's coordinator in the correct way:

```dart
// SharedBridge:
@override Future<int> optimizeImages({int quality = 75}) async {
    _invalidateCache();
    final r = await _transport.execute(
        EngineOp.editorQuery.wire,
        {'handleId': _hid, 'queryCode': 4, 'param': quality},
    ).timeout(_timeout);
    return r['count'] as int? ?? 0;
}
```

NativeTransport sees `editorQuery` -> sends to coordinator as-is (coordinator handles it).
WebTransport sees `editorQuery` -> converts to `editorMutate` with `editOp: 'optimizeImages'`
(because worker.js handles it that way).

Wait — that means WebTransport has op-specific conversion too. Let me think...

Actually the cleanest approach: SharedBridge sends through `_mutate` for ALL three.
NativeTransport detects these three editOps and routes to editorQuery/editorRedactionCount
instead of editorMutate. WebTransport passes through as-is (editorMutate works for web).

```dart
// SharedBridge _EditorHandle:
@override Future<int> optimizeImages({int quality = 75}) async {
    final r = await _mutate('optimizeImages', {'quality': quality});
    return r['count'] as int? ?? 0;
}
@override Future<int> unembedStandardFonts() async {
    final r = await _mutate('unembedStandardFonts');
    return r['value'] as int? ?? 0;
}
@override Future<int> redactionCount(int page) async {
    final r = await _mutate('redactionCount', {'page': page});
    return r['count'] as int? ?? 0;
}
```

NativeTransport intercepts in its editorMutate handler:

```dart
// NativeTransport, inside execute():
if (op == EngineOp.editorMutate.wire) {
    final editOp = args['editOp'] as String;
    if (editOp == 'optimizeImages') {
        // Route to editorQuery path
        return _sendEditorQuery(args['handleId'], 4, args['quality'] ?? 75);
    }
    if (editOp == 'unembedStandardFonts') {
        return _sendEditorQuery(args['handleId'], 5, 0);
    }
    if (editOp == 'redactionCount') {
        return _sendEditorRedactionCount(args['handleId'], args['page']);
    }
    // Normal mutation path
    return _sendEditorMutate(args);
}
```

This keeps SharedBridge clean — all mutations go through `_mutate`. NativeTransport
handles the platform-specific routing internally. WebTransport doesn't need to
know about this at all.

## Page resolution

```dart
// SharedBridge:
Future<List<int>> _resolvePages(DataSource source, PdfPages pages, {String? password}) async {
    return switch (pages) {
        PdfAllPages() => List.generate(
            (await open(source, password: password)).pageCount, (i) => i),
        PdfSinglePage(:final index) => [index],
        PdfPageList(:final indices) => indices,
        PdfPageRange(:final start, :final end) =>
            List.generate(end - start, (i) => start + i),
    };
}
```

`PdfPages.all()` calls `open()` to get pageCount (~10ms), then sends the
full index list. Both platforms do this identically. ZERO worker.js changes.
ZERO coordinator changes.

The native bridge previously avoided this by sending page spec bytes to Rust.
That optimization is sacrificed for symmetry. The cost is ~10ms per streaming
call with `PdfPages.all()`. If this becomes a bottleneck, a future optimization
can add page-spec support to the coordinator/worker without changing SharedBridge.

## Editor metadata — FIX for Hole 2

```dart
class _EditorHandle implements BridgeEditorHandle {
    _Metadata? _cached;

    Future<_Metadata> _getMetadata() async {
        if (_cached != null) return _cached!;
        final r = await _transport.execute(
            EngineOp.editorGetMetadata.wire, {'handleId': _hid},
        ).timeout(_timeout);
        _cached = wireDecodeEditorMetadata(r);
        return _cached!;
    }

    void _invalidateCache() => _cached = null;

    @override Future<int> get pageCount async => (await _getMetadata()).pageCount;
    @override Future<String> get version async => (await _getMetadata()).version;
    @override Future<String> getTitle() async => (await _getMetadata()).title;
    @override Future<String> getAuthor() async => (await _getMetadata()).author;
    @override Future<String> getSubject() async => (await _getMetadata()).subject;
    @override Future<String> getKeywords() async => (await _getMetadata()).keywords;
```

Every mutation calls `_invalidateCache()`. First getter after mutation
makes one round-trip. Subsequent getters read cache. Both platforms identical.

## Editor source lifecycle

NativeTransport keeps SourceServer alive per editor handle:

```dart
class NativeTransport implements PdfTransport {
    final _editorSources = <int, SourceServer>{};

    @override
    Future<Map<String, Object?>> execute(String op, Map<String, Object?> args, {
        DataSource? input, DataSink? output,
    }) async {
        // ... standard setup ...

        if (op == EngineOp.editorOpen.wire && input != null) {
            final server = SourceServer(input);
            final port = server.start();
            // add sourcePort + sourceLength to args for coordinator
            final result = await _send(op, modifiedArgs);
            // extract handleId from binary result
            final handleId = _extractHandleId(resultBytes);
            _editorSources[handleId] = server;  // keep alive
            return resultMap;
        }

        if (op == EngineOp.editorDispose.wire) {
            final handleId = args['handleId'] as int;
            _editorSources.remove(handleId)?.stop();  // cleanup
        }

        // ... rest of ops ...
    }
}
```

WebTransport doesn't need this — OPFS/SAB source is consumed once at open
time, WASM worker holds the document internally after that.

## NativeTransport editOp -> opCode conversion

```dart
static const _editOpCodes = <String, int>{
    'selectPages': 2,
    'deletePages': 3,
    'rotatePages': 5,
    'rotateAllPages': 6,
    'flattenForms': 7,
    'applyRedactions': 8,
    'compress': 9,
    'movePage': 10,
    'embedFile': 11,
    'eraseRegions': 12,
    'encrypt': 13,
    'decrypt': 14,
    'watermark': 15,
    'addStamp': 17,
    'addImageStamp': 18,
    'setTitle': 19,
    'setAuthor': 20,
    'setSubject': 21,
    'setKeywords': 22,
    // NOTE: unembedStandardFonts NOT here — intercepted → editorQuery(5)
    'flattenAllAnnotations': 24,
    'setFormFieldValue': 25,
    'cropMargins': 26,
    'convertToPdfA': 27,
    'resizeImage': 28,
    'addRedaction': 30,
    'applyRedactionsDestructive': 32,
    'scrubMetadata': 33,
};
```

Plus Map fields -> binary Uint8List encoding per opCode. The `_encode*` helpers
from current native/bridge.dart (lines 737-881) move here unchanged.

**Three ops that DON'T use this table:**
- `optimizeImages` -> routed to `editorQuery` with queryCode 4
- `unembedStandardFonts` -> routed to `editorQuery` with queryCode 5
- `redactionCount` -> routed to `editorRedactionCount`

NativeTransport intercepts these before the opCode lookup and routes to the
correct coordinator op.

**Decoder note:** All three intercepted ops return `[1, i32_le]` = a count.
The intercept handler decodes to `{'count': count}` directly — it does NOT
use the general `binaryToResultMap` for these. If a future op is added that
routes through editorQuery with a different result format, the intercept
handler must be updated. Document this in NativeTransport's header comment.

## NativeTransport builder pageOp -> opCode conversion

```dart
static const _pageOpCodes = <String, int>{
    'font': 1, 'at': 2, 'text': 3, 'heading': 4, 'paragraph': 5,
    'space': 6, 'horizontalRule': 7, 'image': 8, 'watermark': 9,
    'textField': 10, 'checkbox': 11, 'comboBox': 12, 'pushButton': 13,
    'signatureField': 14, 'newline': 15, 'newPageSameSize': 16, 'done': 17,
    'radioGroup': 18, 'fieldKeystroke': 19, 'fieldFormat': 20,
    'fieldValidate': 21, 'fieldCalculate': 22, 'linkUrl': 23, 'linkPage': 24,
    'footnote': 25, 'columns': 26,
};
```

Plus per-opCode Map -> binary param encoding. The builder page op
encoders from current native/bridge.dart (lines 977-1165) move here unchanged.

## native/wire.dart changes

Before: exports `wireDecodeOpen(Uint8List)`, `wireDecodeText(Uint8List)`, etc.
Each does binary -> Map -> typed in one step.

After: exports `binaryToResultMap(String op, Uint8List bytes)`.
Returns `Map<String, Object?>`. The Map -> typed step moves to shared wire.dart.

New `_binaryToXxxMap` functions needed (currently go straight to typed):
- `_binaryToTextMap(bytes)` -> `{'text': '...'}`
- `_binaryToVerifyMap(bytes)` -> `{'valid': true/false}`
- `_binaryToValidateUaMap(bytes)` -> `{'accessible': true/false}`
- `_binaryToClassifyPageMap(bytes)` -> `{'type': '...', 'confidence': 1}`
- `_binaryToClassifyDocMap(bytes)` -> `{'type': '...', 'confidence': 1, 'pageCount': 0}`
- `_binaryToEditorMetadataMap(bytes)` -> `{'pageCount': N, 'version': '...', 'title': '...', ...}`
- `_binaryToIsModifiedMap(bytes)` -> `{'modified': true/false}`
- `_binaryToMediaBoxMap(bytes)` -> `{'x': N, 'y': N, 'width': N, 'height': N}`
- `_binaryToMutateResultMap(bytes)` -> `{}` (empty map on success, throw on error). Mutations are void — no data in result.
- `_binaryToEditorQueryMap(bytes)` -> `{'count': N}`

Existing `_binaryToOpenMap`, `_binaryToSearchMap`, `_binaryToSignaturesMap`,
`_binaryToValidationMap`, `_binaryToBookmarksMap`, `_binaryToRenderedPageMap`,
`_binaryToImageMap` stay unchanged.

Error detection: if `bytes[0] == 0`, `binaryToResultMap` throws a `PdfError`
(using existing `wireDecodeError` logic). SharedBridge never sees error Maps.

Also needed: `binaryToStreamItemMap(String op, Uint8List itemBytes)` for
streaming item decoding (render → `_binaryToRenderedPageMap`, extractImages →
`_binaryToImageMap`). Separate from `binaryToResultMap` because item binary
format differs from result binary format.

### Non-Uint8List result normalization

Some coordinator ops return non-Uint8List results:
- `builderCreate` returns `int` handleId directly
- `editorOpen` returns `Uint8List` with handleId embedded

NativeTransport's `execute()` must normalize ALL return types to Map:
```dart
final raw = await _send(op, encodedArgs);
if (raw is Uint8List) return nativeWire.binaryToResultMap(op, raw);
if (raw is int) return {'handleId': raw};
return raw as Map<String, Object?>;
```

## Shared wire.dart

Copy of current `web/wire.dart` (42 lines). All functions take `Map<String, Object?>`:

```dart
PdfDoc wireDecodeOpen(Map<String, Object?> r) => decodeOpenResult(r);
String wireDecodeText(Map<String, Object?> r) => r['text'] as String? ?? '';
List<SearchResult> wireDecodeSearch(Map<String, Object?> r) => decodeSearchResults(r);
List<PdfSignatureInfo> wireDecodeSignatures(Map<String, Object?> r) => decodeSignatures(r);
bool wireDecodeVerifySignatures(Map<String, Object?> r) => r['valid'] as bool? ?? false;
PdfValidationResult wireDecodeValidation(Map<String, Object?> r) => decodeValidationResult(r);
bool wireDecodeValidatePdfUa(Map<String, Object?> r) => r['accessible'] as bool? ?? false;
List<PdfBookmarkSplit> wireDecodeBookmarkSplits(Map<String, Object?> r) => decodeBookmarkSplits(r);
PdfPageClassification wireDecodeClassifyPage(Map<String, Object?> r) => decodeClassifyPage(r);
PdfDocumentClassification wireDecodeClassifyDocument(Map<String, Object?> r) => decodeClassifyDocument(r);
RenderedPage wireDecodeRenderedPage(Map<String, Object?> r) => decodeRenderedPage(r);
PdfImage wireDecodeImage(Map<String, Object?> r) => decodePdfImage(r);
({int pageCount, String version, String title, String author, String subject, String keywords})
    wireDecodeEditorMetadata(Map<String, Object?> r) => decodeEditorMetadata(r);
```

## Files — complete list

### New files (3)

| File | ~Lines |
|---|---|
| `lib/src/transport/transport.dart` | 25 |
| `lib/src/transport/shared_bridge.dart` | 500 |
| `lib/src/transport/wire.dart` | 45 |

### Gutted files (3)

| File | Before | After | ~Lines |
|---|---|---|---|
| `native/bridge.dart` | NativeBridge (1166) | NativeTransport | 540 |
| `web/bridge.dart` | WebBridge (907) | WebTransport | 390 |
| `native/wire.dart` | binary->Map->typed (308) | binary->Map only + `binaryToResultMap` | 280 |

### Modified files (3)

| File | Change |
|---|---|
| `_create_native.dart` | Return `SharedBridge(NativeTransport())` |
| `_create_web.dart` | Return `SharedBridge(WebTransport())` |
| `create.dart` | Import update |

### Deleted files (1)

| File | Reason |
|---|---|
| `web/wire.dart` | Content -> shared `wire.dart` |

### ZERO changes (everything else)

- Consumer API: `ops/pdf.dart`, `ops/pdf_editor.dart`, `ops/pdf_builder.dart`, `ops/pdf_operations.dart`
- Abstract bridge: `transport/pdf_bridge.dart`
- Protocol: `transport/protocol/codec.dart`, `transport/protocol/op.dart`
- Native infra: `native/coordinator.dart`, `native/bindings.dart`, `native/shared_buffer.dart`, `native/source_server.dart`, `native/sink_server.dart`
- Web infra: `web_assets/coordinator.js`, `web_assets/worker.js`
- ALL Rust files
- ALL test files
- `Makefile`, `dart_test.yaml`, `pubspec.yaml`

## Asymmetries killed permanently

| Asymmetry | How it dies |
|---|---|
| Page resolution double-open | `_resolvePages` in SharedBridge. Written ONCE. Both platforms identical. |
| Metadata N+1 | `_getMetadata()` with `_cached`. Written ONCE. Invalidated on mutation. |
| Timeout mismatch | `_timeout = 60s`. Written ONCE. |
| Encoding approach | codec.dart Maps. Written ONCE. NativeTransport converts to binary. |
| Decoding approach | shared wire.dart. Written ONCE. NativeTransport normalizes to Map. |
| optimizeImages routing | SharedBridge sends through `_mutate`. NativeTransport intercepts and routes to editorQuery. Written ONCE in SharedBridge. |
| Any future drift | New op goes in SharedBridge. Only ONE bridge exists. |

## Implementation order

1. Create `transport.dart` (interface)
2. Create `wire.dart` (copy web/wire.dart)
3. Create `shared_bridge.dart` (implement every PdfBridge method)
4. Rewrite `native/wire.dart` (add `binaryToResultMap` + new `_binaryToXxxMap` functions)
5. Rewrite `native/bridge.dart` -> NativeTransport
6. Rewrite `web/bridge.dart` -> WebTransport
7. Delete `web/wire.dart`
8. Update `create.dart`, `_create_native.dart`, `_create_web.dart`
9. `fvm flutter analyze .`
10. `make test` (190 native)
11. `make test-web` (130 web)
12. Update `docs/ARCHITECTURE.md`

## Verification

```bash
fvm flutter analyze .                    # zero issues
make test                                # 190 native pass
make test-web                            # 130 web pass

# No bridge logic in transport files:
grep -c 'wireDecodeOpen\|wireDecodeText\|_resolvePages\|_getMetadata\|_encodeWatermark' \
    lib/src/transport/native/bridge.dart lib/src/transport/web/bridge.dart
# Expected: 0 in each

# SharedBridge is the only bridge:
grep -rn 'implements PdfBridge' lib/src/transport/
# Expected: only shared_bridge.dart

# Transport files are transport only:
grep -c '@override' lib/src/transport/native/bridge.dart lib/src/transport/web/bridge.dart
# Expected: only PdfTransport overrides (execute, executeStream, dispose)
```

## Known trade-offs (honest)

| Trade-off | Cost | Why acceptable |
|---|---|---|
| `PdfPages.all()` calls `open()` Dart-side | ~10ms per streaming call | Symmetry > 10ms. Future optimization can add spec support without changing SharedBridge. |
| NativeTransport has op-specific routing (editorQuery for 3 ops) | ~15 lines of if/else | These 3 ops use different FFI paths on native. Transport is the right place for platform routing. |
| NativeTransport peeks into editorOpen result for handleId | ~5 lines | Source lifecycle requires knowing which handle to track. Transport owns source lifecycle. |
| mergeFrom reads all bytes Dart-side | Memory proportional to merge source | Same as today. Both bridges already do this. Not a regression. |
| imagesToPdf reads all images Dart-side | Memory proportional to total image size | Same as today. Both bridges already do this. Not a regression. |
| NativeTransport is 540 lines (not tiny) | Encoding complexity | The ~370 lines of binary encoding helpers are mechanical translations. They moved from bridge, not created. |
| `PdfPages.all()` sends 4004 bytes (1000 indices) instead of 1 byte (`[0]` = all) | Slightly more data over isolate port for native | The coordinator decodes both formats the same way. Not a performance issue — isolate SendPort handles this in microseconds. |
| NativeTransport editorQuery decoder assumes count result format | If a 4th editorQuery usage is added via `_mutate`, decoder needs updating | Low risk — only 3 ops use this path. Document in NativeTransport header comment. |
| WebTransport loses its 15s result timeout | Relies on SharedBridge 60s timeout only | SharedBridge timeout is the single source of truth. Transport setup timeouts (init 30s, OPFS 10s) remain for infrastructure failures. |
| Streaming items need separate decoder path in native/wire.dart | `binaryToStreamItemMap` vs `binaryToResultMap` | Item binary format (no status byte, just raw data) differs from result binary format (status byte + data). Two decoders, same file. |
| Sign always sends op `'sign'` — NativeTransport checks args for PEM | NativeTransport has 3-line if/else for sign routing | SharedBridge sends `'sign'` for both credential types (worker.js handles both under one case). NativeTransport checks if `certPem` is in args → sends `'signPem'` to native coordinator (opCode 29), else sends `'sign'` (opCode 16). |
| `editorMergeFrom` passes through NativeTransport without binary encoding | Coordinator already handles conversion internally | coordinator.dart `_handleEditorMergeFrom` extracts `otherBytes` and packs as secondaries with opCode 1. NativeTransport sends as-is. |
| Mutations return empty Map | SharedBridge doesn't read mutation results (void ops) | `_binaryToMutateResultMap` returns `{}` on success, throws on error. SharedBridge's `_mutate` returns `Map` but callers ignore it (except optimizeImages/unembedStandardFonts/redactionCount which are intercepted). |
| NativeTransport removes its own 60s `_send` timeout | SharedBridge's 60s is the single authority | Current native bridge has 60s on `_send` (added this session). After refactor, that moves to SharedBridge. Double-timeout would produce confusing errors. |

## Pre-existing bugs found during plan audit (fix separately)

| Bug | Where | Impact |
|---|---|---|
| worker.js has duplicate `case 'applyRedactions'` — second (destructive) is dead code | worker.js line 464 + 503 | `applyRedactionsDestructive` calls non-destructive on web. Fix: rename to `'applyRedactionsDestructive'` in worker.js + web bridge. |
