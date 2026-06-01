// WASM Worker — one InstanceState per worker, multiple readers per instance.
//
// ── Reader registry ──────────────────────────────────────────────
//
// Each source gets a unique readerId in the registry. host_read_at
// dispatches by sourceIndex → readerId via the activeReaderMap.
// Pinned readers survive across ops (handle lifetime). Non-pinned
// readers are cleaned after each exec.
//
// ── Three I/O modes (auto-detected: jspi > atomics > opfs) ──────
//
//   jspi    — JSPI Promise suspension (Chrome 137+ / Firefox 139+)
//   atomics — SAB + Atomics.wait (needs COOP/COEP)
//   opfs    — SyncAccessHandle on OPFS disk (universal)
//
// ── Message protocol ─────────────────────────────────────────────
//
//   init           { ioMode, baseUrl, wasmModule? }  → ready | error
//   exec           { opId, requestBytes, args, ioMode, sab?,
//                    opfsFile?, opfsFiles?, isPinnedOp,
//                    pinnedSourceOpId? }              → result | error
//                  ← readAt { reqId, sourceIndex, offset, count, mode, sab? }
//                    → readAtResponse { reqId, bytes | error }
//                  ← chunk { sinkIndex, data }
//   shutdown                                          → shutdown_done
//   releaseOpfs   { sourceOpId? }                     → releaseOpfs_done
//   readAtResponse { reqId, bytes | error }

// ═══════════════════════════════════════════════════════════════════
// WASM + instance state
// ═══════════════════════════════════════════════════════════════════

let wasm = null;
let initialized = false;
let instancePtr = 0;
let currentIoMode = 'opfs';
let wasmBaseUrl = (() => {
  try { return new URL(self.location.href).href.replace(/\/[^/]*$/, '/'); }
  catch (_) { return './'; }
})();

// ═══════════════════════════════════════════════════════════════════
// Reader registry
// ═══════════════════════════════════════════════════════════════════

/** @type {Map<number, Reader>} */
const readers = new Map();
let nextReaderId = 1;

/**
 * @typedef {Object} Reader
 * @property {number} length        — source byte length
 * @property {FileSystemSyncAccessHandle|null} opfsHandle
 * @property {string|null} opfsFile — OPFS filename for cleanup
 * @property {boolean} pinned       — survives across ops
 * @property {number|null} pinnedByOpId
 */

function registerReader(length, opfsHandle, opfsFile, pinned, pinnedByOpId) {
  const id = nextReaderId++;
  readers.set(id, { length, opfsHandle, opfsFile, pinned, pinnedByOpId });
  return id;
}

function deregisterNonPinned() {
  for (const [id, r] of readers) {
    if (!r.pinned) {
      if (r.opfsHandle) { try { r.opfsHandle.close(); } catch (_) {} }
      readers.delete(id);
    }
  }
}

function clearAllReaders() {
  for (const [, r] of readers) {
    if (r.opfsHandle) { try { r.opfsHandle.close(); } catch (_) {} }
  }
  readers.clear();
}

function findPinnedByOpId(opId) {
  for (const [id, r] of readers) {
    if (r.pinned && r.pinnedByOpId === opId) return { id, reader: r };
  }
  return null;
}

// ═══════════════════════════════════════════════════════════════════
// host_read_at — dispatched by WASM via extern import
// ═══════════════════════════════════════════════════════════════════

const SAB_HEADER = 8;

/** @type {Array<[number, number]>|null} sourceIndex → readerId */
let activeReaderMap = null;

/** @type {SharedArrayBuffer|null} */
let activeSab = null;

/** @type {((msg: any) => void)|null} JSPI read callback */
let jspiReadResolve = null;

/** @type {((msg: any) => void)|null} JSPI/async write ack callback */
let jspiWriteResolve = null;

/** @type {SharedArrayBuffer|null} SAB for write backpressure (Atomics mode) */
let activeWriteSab = null;

function hostReadAt(sourceIndex, offset, count, bufPtr) {
  const readerId = resolveReaderId(sourceIndex);
  const reader = readers.get(readerId);
  if (!reader) return -1;

  // Cap matches SAB_MAX_CHUNK in coordinator.js and chunkSize in web_transport.dart.
  const toRead = Math.min(count, reader.length - offset, 65536);
  if (toRead <= 0) return 0;

  // OPFS reader — sync read from disk, independent of I/O mode
  if (reader.opfsHandle) return readFromOpfs(reader.opfsHandle, offset, toRead, bufPtr);

  // Callback reader — mode determines sync/async mechanism
  switch (currentIoMode) {
    case 'atomics': return readViaAtomics(sourceIndex, offset, toRead, bufPtr);
    case 'jspi':    return readViaJspi(sourceIndex, offset, toRead, bufPtr);
    default:        return readViaJspi(sourceIndex, offset, toRead, bufPtr);
  }
}

function resolveReaderId(sourceIndex) {
  if (activeReaderMap) {
    for (const [si, rid] of activeReaderMap) {
      if (si === sourceIndex) return rid;
    }
  }
  return sourceIndex;
}

// ── Read: OPFS (sync, any mode) ─────────────────────────────────

function readFromOpfs(handle, offset, count, bufPtr) {
  const buf = new Uint8Array(count);
  const n = handle.read(buf, { at: offset });
  new Uint8Array(wasm.memory.buffer).set(buf.subarray(0, n), bufPtr);
  return n;
}

// ── Read: Atomics (sync block via SAB) ──────────────────────────

function readViaAtomics(sourceIndex, offset, count, bufPtr) {
  if (!activeSab) return -1;
  const status = new Int32Array(activeSab);
  const data = new Uint8Array(activeSab, SAB_HEADER);

  Atomics.store(status, 0, 0);
  self.postMessage({
    type: 'readAt', reqId: '0', sourceIndex, offset, count,
    mode: 'atomics', sab: activeSab,
  });
  Atomics.wait(status, 0, 0);

  if (Atomics.load(status, 0) === 2) return -1;
  const len = Atomics.load(new Int32Array(activeSab, 4, 1), 0);
  new Uint8Array(wasm.memory.buffer).set(data.subarray(0, len), bufPtr);
  return len;
}

// ── Read: JSPI (async via promise suspension) ───────────────────

function readViaJspi(sourceIndex, offset, count, bufPtr) {
  return new Promise((resolve) => {
    jspiReadResolve = (msg) => {
      if (msg.error) { resolve(-1); return; }
      const bytes = new Uint8Array(msg.bytes);
      new Uint8Array(wasm.memory.buffer).set(bytes, bufPtr);
      resolve(bytes.length);
    };
    self.postMessage({
      type: 'readAt', reqId: '0', sourceIndex, offset, count, mode: 'async',
    });
  });
}

// ═══════════════════════════════════════════════════════════════════
// host_write_chunk — called by WASM for output, with backpressure
// ═══════════════════════════════════════════════════════════════════
//
// Backpressure prevents the WASM producer from outrunning the Dart
// consumer. Each chunk waits for an ack before the next is produced.
//
//   Atomics: Atomics.wait on a write SAB until coordinator acks
//   JSPI:    returns Promise, resolved when coordinator acks
//   OPFS:    fire-and-forget (no blocking mechanism available)

function hostWriteChunk(sinkIndex, bufPtr, len) {
  const chunk = new Uint8Array(wasm.memory.buffer).slice(bufPtr, bufPtr + len);
  switch (currentIoMode) {
    case 'atomics':
      return writeWithAtomicsAck(sinkIndex, chunk);
    case 'jspi':
      return writeWithJspiAck(sinkIndex, chunk);
    default:
      // OPFS fallback — no backpressure available
      self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
      return 0;
  }
}

function writeWithAtomicsAck(sinkIndex, chunk) {
  if (!activeWriteSab) {
    self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
    return 0;
  }
  const status = new Int32Array(activeWriteSab);
  Atomics.store(status, 0, 0);
  self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
  Atomics.wait(status, 0, 0);
  return Atomics.load(status, 0) === 2 ? -1 : 0;
}

function writeWithJspiAck(sinkIndex, chunk) {
  return new Promise((resolve) => {
    jspiWriteResolve = (msg) => {
      resolve(msg?.error ? -1 : 0);
    };
    self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
  });
}

// ═══════════════════════════════════════════════════════════════════
// WASM initialization
// ═══════════════════════════════════════════════════════════════════

let jspiBridgeExecute = null;
let precompiledModule = null;

const JSPI_ASYNC_IMPORTS = ['__wbg_host_read_at', '__wbg_host_write_chunk'];

async function ensureInit() {
  if (initialized) return;

  self.host_read_at = hostReadAt;
  self.host_write_chunk = hostWriteChunk;

  const jsModule = await import(wasmBaseUrl + 'pdf_oxide.js');

  // Monkey-patch instantiate to intercept imports for JSPI wrapping
  // and to capture raw WASM exports. Restored in finally.
  const origInstantiate = WebAssembly.instantiate;
  const origStreaming = WebAssembly.instantiateStreaming;

  WebAssembly.instantiate = async (source, imports) => {
    if (currentIoMode === 'jspi' && imports) imports = wrapJspiImports(imports);
    const result = await origInstantiate(source, imports);
    wasm = (result instanceof WebAssembly.Instance ? result : result.instance).exports;
    return result;
  };
  if (origStreaming) {
    WebAssembly.instantiateStreaming = async (response, imports) => {
      const bytes = await (await response).arrayBuffer();
      return WebAssembly.instantiate(bytes, imports);
    };
  }

  try {
    await jsModule.default(precompiledModule || (wasmBaseUrl + 'pdf_oxide_bg.wasm'));
  } finally {
    WebAssembly.instantiate = origInstantiate;
    if (origStreaming) WebAssembly.instantiateStreaming = origStreaming;
  }

  if (currentIoMode === 'jspi') {
    jspiBridgeExecute = WebAssembly.promising(wasm.bridge_execute);
  }
  initialized = true;
}

function wrapJspiImports(imports) {
  const patched = {};
  for (const [mod, fns] of Object.entries(imports)) {
    const patchedMod = {};
    for (const [name, fn] of Object.entries(fns)) {
      patchedMod[name] = (JSPI_ASYNC_IMPORTS.some(p => name.startsWith(p)) && typeof fn === 'function')
        ? new WebAssembly.Suspending(fn) : fn;
    }
    patched[mod] = patchedMod;
  }
  return patched;
}

// ═══════════════════════════════════════════════════════════════════
// JSPI bridge_execute caller
// ═══════════════════════════════════════════════════════════════════

async function callBridgeJspi(requestBytes, sourceLengthsPacked, sinkCount) {
  const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
  const ptr0 = wasm.__wbindgen_export(requestBytes.length, 1) >>> 0;
  new Uint8Array(wasm.memory.buffer).set(requestBytes, ptr0);
  const ptr1 = wasm.__wbindgen_export(sourceLengthsPacked.length, 1) >>> 0;
  new Uint8Array(wasm.memory.buffer).set(sourceLengthsPacked, ptr1);
  try {
    await jspiBridgeExecute(retptr, instancePtr, ptr0, requestBytes.length, ptr1, sourceLengthsPacked.length, sinkCount);
    const mem = new DataView(wasm.memory.buffer);
    const r0 = mem.getInt32(retptr, true);
    const r1 = mem.getInt32(retptr + 4, true);
    const result = new Uint8Array(wasm.memory.buffer, r0, r1).slice();
    wasm.__wbindgen_export4(r0, r1, 1);
    return result;
  } finally {
    wasm.__wbindgen_add_to_stack_pointer(16);
  }
}

// ═══════════════════════════════════════════════════════════════════
// OPFS handle opening
// ═══════════════════════════════════════════════════════════════════

async function openOpfsHandle(filename) {
  const root = await navigator.storage.getDirectory();
  const fh = await root.getFileHandle(filename);
  return await fh.createSyncAccessHandle();
}

// ═══════════════════════════════════════════════════════════════════
// Exec — build readers, call bridge_execute, pin on success
// ═══════════════════════════════════════════════════════════════════

async function exec(msg) {
  const { opId, requestBytes, args, ioMode, sab, isPinnedOp, pinnedSourceOpId } = msg;
  if (!requestBytes) throw new Error('No requestBytes');

  await ensureInit();
  activeSab = (ioMode === 'atomics' && sab) ? sab : null;
  activeWriteSab = (ioMode === 'atomics' && msg.writeSab) ? msg.writeSab : null;

  // ── Parse OPFS filenames ──────────────────────────────────────
  const opfsFiles = msg.opfsFiles
    ? msg.opfsFiles.split(',')
    : (msg.opfsFile ? [msg.opfsFile] : []);

  // ── Collect source lengths from Dart args ─────────────────────
  const dartLengths = [];
  if ((args?.sourceLength ?? 0) > 0) dartLengths.push(args.sourceLength);
  for (let i = 1; ; i++) {
    const v = args?.[`source${i}Length`];
    if (v != null && v > 0) dartLengths.push(v); else break;
  }

  // ── Build reader registry for this exec ───────────────────────
  const readerIds = [];
  const finalLengths = [];

  // Pinned ops: index 0 = existing pinned reader, index 1+ = new sources
  if (isPinnedOp && pinnedSourceOpId != null) {
    const pinned = findPinnedByOpId(pinnedSourceOpId);
    if (pinned) {
      readerIds.push(pinned.id);
      finalLengths.push(pinned.reader.length);
    }
  }

  // Register new readers for Dart's sources
  for (let i = 0; i < dartLengths.length; i++) {
    let handle = null;
    let length = dartLengths[i];
    const file = opfsFiles[i] || null;
    if (file) {
      handle = await openOpfsHandle(file);
      length = handle.getSize();
    }
    readerIds.push(registerReader(length, handle, file, false, null));
    finalLengths.push(length);
  }

  // ── Pack source lengths for WASM ──────────────────────────────
  const packed = new Uint8Array(finalLengths.length * 8);
  const dv = new DataView(packed.buffer);
  for (let i = 0; i < finalLengths.length; i++) {
    dv.setFloat64(i * 8, finalLengths[i], true);
  }

  activeReaderMap = readerIds.map((rid, i) => [i, rid]);
  const sinkCount = args?.hasSink ? 1 : 0;
  const shouldPin = !!(args?.keepSource);
  let responseBytes;

  try {
    if (currentIoMode === 'jspi') {
      responseBytes = await callBridgeJspi(new Uint8Array(requestBytes), packed, sinkCount);
    } else {
      const { bridge_execute } = await import(wasmBaseUrl + 'pdf_oxide.js');
      responseBytes = bridge_execute(instancePtr, new Uint8Array(requestBytes), packed, sinkCount);
    }

    // Pin on success only (status byte 1 = ok)
    if (responseBytes.length > 0 && responseBytes[0] === 1 && shouldPin) {
      for (const rid of readerIds) {
        const r = readers.get(rid);
        if (r) { r.pinned = true; r.pinnedByOpId = opId; }
      }
    }
  } finally {
    deregisterNonPinned();
    activeReaderMap = null;
    activeSab = null;
    activeWriteSab = null;
    jspiWriteResolve = null;
  }

  return responseBytes;
}

// ═══════════════════════════════════════════════════════════════════
// Message handler
// ═══════════════════════════════════════════════════════════════════

self.onmessage = async (e) => {
  const msg = e.data;

  switch (msg.type) {
    case 'init': {
      currentIoMode = msg.ioMode;
      if (msg.baseUrl) wasmBaseUrl = msg.baseUrl;
      if (msg.wasmModule) precompiledModule = msg.wasmModule;
      try {
        await ensureInit();
        const { bridge_init } = await import(wasmBaseUrl + 'pdf_oxide.js');
        instancePtr = bridge_init();
        self.postMessage({ type: 'ready' });
      } catch (err) {
        self.postMessage({ type: 'error', error: 'WASM init failed: ' + err.message });
      }
      return;
    }

    case 'shutdown': {
      if (instancePtr !== 0) {
        try {
          const { bridge_shutdown } = await import(wasmBaseUrl + 'pdf_oxide.js');
          bridge_shutdown(instancePtr);
        } catch (_) {}
        instancePtr = 0;
      }
      clearAllReaders();
      self.postMessage({ type: 'shutdown_done' });
      return;
    }

    case 'releaseOpfs': {
      const targetOpId = msg.sourceOpId;
      for (const [id, r] of readers) {
        if (r.pinned && (targetOpId == null || r.pinnedByOpId === targetOpId)) {
          if (r.opfsHandle) { try { r.opfsHandle.close(); } catch (_) {} }
          readers.delete(id);
        }
      }
      self.postMessage({ type: 'releaseOpfs_done' });
      return;
    }

    case 'readAtResponse': {
      if (jspiReadResolve) { jspiReadResolve(msg); jspiReadResolve = null; }
      return;
    }

    case 'chunkAck': {
      if (jspiWriteResolve) { jspiWriteResolve(msg); jspiWriteResolve = null; }
      return;
    }

    case 'exec': {
      try {
        const responseBytes = await exec(msg);
        // Slice to ensure we transfer only the result, not the full WASM heap
        // (wasm-bindgen may return a view into linear memory).
        const owned = responseBytes.buffer.byteLength === responseBytes.byteLength
            ? responseBytes.buffer
            : responseBytes.buffer.slice(responseBytes.byteOffset, responseBytes.byteOffset + responseBytes.byteLength);
        self.postMessage({ type: 'result', data: owned }, [owned]);
      } catch (err) {
        self.postMessage({ type: 'error', error: err.message || String(err) });
      }
      return;
    }
  }
};
