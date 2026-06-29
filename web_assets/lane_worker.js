// lane_worker.js — the web lane BODY. One Worker = one lane = one
// LaneState (created via lane_init in WASM linear memory).
//
// The web twin of host/native/lane.rs. DECISION-FREE by design:
// routing, pinning decisions, queuing, cancellation bookkeeping all
// live in the shared Dart Router/WebLane. This file only translates
// messages into physics — load WASM, serve host_read_at /
// host_write_chunk per I/O mode, run lane_execute, post the result.
//
// Numeric protocol codes are INJECTED via the init message (see
// protocol.dart — the single source of truth). This file
// declares no numeric constants of its own.
//
// ── Message protocol (tags pinned by protocol.dart + parity test)
//
// Dart → worker:
//   init         { protocol, ioMode, baseUrl?, wasmModule? } → ready|error
//   exec         { jobId, requestBytes, sourceLengths: number[],
//                  sinkCount, keepSources: number[], heldJobId?,
//                  opfsFiles: (string|null)[], sab?, writeSab? }
//                → result { jobId, data } | error { jobId, message }
//   readAtResult { bytes? , error?, cancelled? }    (JSPI reads)
//   chunkAck     { error?, cancelled? }             (JSPI writes)
//   releaseHeld  { heldJobId }
//   opfsWrite    { jobId, filename, chunk, offset, last } → opfsWriteAck
//   opfsDrop     { jobId }
//
// worker → Dart:
//   ready | readAt { sourceIndex, offset, count } | chunk { sinkIndex,
//   data } | result | error | opfsWriteAck
//
// ── Three I/O modes (selected by Dart, passed in init) ─────────────
//   jspi    — JSPI Promise suspension (host_read_at returns Promise)
//   atomics — per-job SAB + Atomics.wait; Dart fills + notifies
//   opfs    — sources pre-copied to OPFS; reads via SyncAccessHandle
//
// Hard kill is worker.terminate() from Dart — this file never sees it.

// ═══════════════════════════════════════════════════════════════════
// State
// ═══════════════════════════════════════════════════════════════════

let P = null;               // injected protocol codes
let wasm = null;            // raw WASM exports
let lanePtr = 0;            // LaneState pointer (lane_init)
let ioMode = 'opfs';
let initialized = false;
let precompiledModule = null;
let wasmBaseUrl = (() => {
  try { return new URL(self.location.href).href.replace(/\/[^/]*$/, '/'); }
  catch (_) { return './'; }
})();

// ═══════════════════════════════════════════════════════════════════
// Reader registry — physics only. The Dart side decides WHICH
// sources are kept (keepSources) and WHICH held set an op reuses
// (heldJobId); this registry just stores and looks up.
// ═══════════════════════════════════════════════════════════════════

const readers = new Map();   // readerId → {length, opfsHandle, opfsFile, heldJobId|null}
let nextReaderId = 1;

let activeReaderMap = null;  // sourceIndex → readerId, per exec
let activeSab = null;        // per-exec read SAB (atomics)
let activeWriteSab = null;   // per-exec write-ack SAB (atomics)
let jspiReadResolve = null;
let jspiWriteResolve = null;

async function dropReader(id, r) {
  // This worker OWNS its pre-copy files: it created them, holds their
  // handles, and is their only consumer — so close-then-delete here is
  // same-agent and race-free by construction. If this worker dies
  // before dropping, the host reclaims the whole directory via the
  // liveness lock instead; no third party ever touches a live file.
  if (r.opfsHandle) { try { r.opfsHandle.close(); } catch (_) {} }
  readers.delete(id);
  if (r.opfsFile) {
    try { (await laneDir()).removeEntry(r.opfsFile); } catch (_) {}
  }
}

// ═══════════════════════════════════════════════════════════════════
// host_read_at — called by WASM
// ═══════════════════════════════════════════════════════════════════

function hostReadAt(sourceIndex, offset, count, bufPtr) {
  let readerId = sourceIndex;
  if (activeReaderMap) {
    for (const [si, rid] of activeReaderMap) {
      if (si === sourceIndex) { readerId = rid; break; }
    }
  }
  const reader = readers.get(readerId);
  if (!reader) return P.hostIoError;

  const toRead = Math.min(count, reader.length - offset, P.sabMaxChunk);
  if (toRead <= 0) return 0;

  if (reader.opfsHandle) return readFromOpfs(reader.opfsHandle, offset, toRead, bufPtr);

  switch (ioMode) {
    case 'atomics': return readViaAtomics(sourceIndex, offset, toRead, bufPtr);
    default:        return readViaJspi(sourceIndex, offset, toRead, bufPtr);
  }
}

function readFromOpfs(handle, offset, count, bufPtr) {
  const buf = new Uint8Array(count);
  const n = handle.read(buf, { at: offset });
  new Uint8Array(wasm.memory.buffer).set(buf.subarray(0, n), bufPtr);
  return n;
}

function readViaAtomics(sourceIndex, offset, count, bufPtr) {
  if (!activeSab) return P.hostIoError;
  const status = new Int32Array(activeSab);

  Atomics.store(status, 0, P.sabPending);
  self.postMessage({ type: 'readAt', sourceIndex, offset, count });
  Atomics.wait(status, 0, P.sabPending);

  const s = Atomics.load(status, 0);
  if (s === P.sabCancelled) return P.hostIoCancelled;
  if (s !== P.sabReady) return P.hostIoError;

  const len = Atomics.load(new Int32Array(activeSab, P.sabLengthOffset, 1), 0);
  const data = new Uint8Array(activeSab, P.sabHeaderBytes);
  new Uint8Array(wasm.memory.buffer).set(data.subarray(0, len), bufPtr);
  return len;
}

function readViaJspi(sourceIndex, offset, count, bufPtr) {
  return new Promise((resolve) => {
    jspiReadResolve = (msg) => {
      if (msg.cancelled) { resolve(P.hostIoCancelled); return; }
      if (msg.error) { resolve(P.hostIoError); return; }
      const bytes = new Uint8Array(msg.bytes);
      new Uint8Array(wasm.memory.buffer).set(bytes, bufPtr);
      resolve(bytes.length);
    };
    self.postMessage({ type: 'readAt', sourceIndex, offset, count });
  });
}

// ═══════════════════════════════════════════════════════════════════
// host_write_chunk — called by WASM, with backpressure
// ═══════════════════════════════════════════════════════════════════

function hostWriteChunk(sinkIndex, bufPtr, len) {
  const chunk = new Uint8Array(wasm.memory.buffer).slice(bufPtr, bufPtr + len);
  switch (ioMode) {
    case 'atomics': return writeViaAtomics(sinkIndex, chunk);
    case 'jspi':    return writeViaJspi(sinkIndex, chunk);
    default:
      // OPFS: no blocking primitive — fire-and-forget (the producer
      // is disk-read bound, never outruns the main-thread consumer).
      self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
      return 0;
  }
}

function writeViaAtomics(sinkIndex, chunk) {
  if (!activeWriteSab) {
    self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
    return 0;
  }
  const status = new Int32Array(activeWriteSab);
  Atomics.store(status, 0, P.sabPending);
  self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
  Atomics.wait(status, 0, P.sabPending);

  const s = Atomics.load(status, 0);
  if (s === P.sabCancelled) return P.hostIoCancelled;
  if (s !== P.sabReady) return P.hostIoError;
  return 0;
}

function writeViaJspi(sinkIndex, chunk) {
  return new Promise((resolve) => {
    jspiWriteResolve = (msg) => {
      if (msg && msg.cancelled) { resolve(P.hostIoCancelled); return; }
      resolve(msg && msg.error ? P.hostIoError : 0);
    };
    self.postMessage({ type: 'chunk', sinkIndex, data: chunk.buffer }, [chunk.buffer]);
  });
}

// ═══════════════════════════════════════════════════════════════════
// WASM init
// ═══════════════════════════════════════════════════════════════════

let jspiLaneExecute = null;
const JSPI_ASYNC_IMPORTS = ['__wbg_host_read_at', '__wbg_host_write_chunk'];

async function ensureInit() {
  if (initialized) return;

  self.host_read_at = hostReadAt;
  self.host_write_chunk = hostWriteChunk;

  const jsModule = await import(wasmBaseUrl + 'pdf_oxide.js');

  // Intercept instantiation to (a) capture raw exports, (b) wrap the
  // host imports in WebAssembly.Suspending for JSPI. Restored after.
  const origInstantiate = WebAssembly.instantiate;
  const origStreaming = WebAssembly.instantiateStreaming;

  WebAssembly.instantiate = async (source, imports) => {
    if (ioMode === 'jspi' && imports) imports = wrapJspiImports(imports);
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

  if (ioMode === 'jspi') {
    jspiLaneExecute = WebAssembly.promising(wasm.lane_execute);
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

// JSPI path drives the wasm-bindgen ABI manually (the generated glue
// is synchronous; promising() needs the raw export). Mirrors the
// glue's own calling convention for lane_execute.
async function callLaneJspi(requestBytes, sourceLengthsPacked, sinkCount) {
  const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
  const ptr0 = wasm.__wbindgen_export(requestBytes.length, 1) >>> 0;
  new Uint8Array(wasm.memory.buffer).set(requestBytes, ptr0);
  const ptr1 = wasm.__wbindgen_export(sourceLengthsPacked.length, 1) >>> 0;
  new Uint8Array(wasm.memory.buffer).set(sourceLengthsPacked, ptr1);
  try {
    await jspiLaneExecute(retptr, lanePtr, ptr0, requestBytes.length, ptr1, sourceLengthsPacked.length, sinkCount);
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
// OPFS
// ═══════════════════════════════════════════════════════════════════

// Pre-copies live in this worker's OWN directory, named by the host
// at init: {root}/{sessionId}/{workerId}. This worker owns every file
// inside — creation, handles, and deletion. Its liveness lock (held
// from before the first file exists until the browser kills the
// agent) is what lets the host prove death before reclaiming the
// directory: the lock release on agent termination is a spec
// guarantee, the web's equivalent of the native lane's
// every-job-posts-exactly-once contract.
let opfsDirParts = null;
let laneDirPromise = null;

function acquireDirLock() {
  if (!opfsDirParts) return Promise.resolve();
  const name = opfsDirParts.join('/');
  return new Promise((granted) => {
    navigator.locks.request(name, () => {
      granted();
      return new Promise(() => {}); // held until this agent dies
    });
  });
}

function laneDir() {
  laneDirPromise ??= (async () => {
    let dir = await navigator.storage.getDirectory();
    for (const part of (opfsDirParts || [])) {
      dir = await dir.getDirectoryHandle(part, { create: true });
    }
    return dir;
  })();
  return laneDirPromise;
}

async function openOpfsHandle(filename) {
  const dir = await laneDir();
  const fh = await dir.getFileHandle(filename);
  return await fh.createSyncAccessHandle();
}

const opfsWriteHandles = new Map();  // filename → SyncAccessHandle
const jobFiles = new Map();          // jobId → Set(filename), pre-exec owner

async function opfsWrite(jobId, filename, chunk, offset, last) {
  let files = jobFiles.get(jobId);
  if (!files) { files = new Set(); jobFiles.set(jobId, files); }
  files.add(filename); // registered before the first byte hits disk

  let sh = opfsWriteHandles.get(filename);
  if (!sh) {
    const dir = await laneDir();
    const fh = await dir.getFileHandle(filename, { create: true });
    sh = await fh.createSyncAccessHandle();
    opfsWriteHandles.set(filename, sh);
  }
  sh.write(new Uint8Array(chunk), { at: offset });
  if (last) {
    sh.flush();
    sh.close();
    opfsWriteHandles.delete(filename);
  }
}

// A job is over: delete every file it still owns. Files that exec
// transferred to readers are no longer here — they die with their
// reader (dropReader). Idempotent; an empty set is a no-op.
async function opfsDrop(jobId) {
  const files = jobFiles.get(jobId);
  jobFiles.delete(jobId);
  if (!files) return;
  const dir = await laneDir();
  for (const f of files) {
    const sh = opfsWriteHandles.get(f);
    if (sh) { try { sh.close(); } catch (_) {} opfsWriteHandles.delete(f); }
    try { await dir.removeEntry(f); } catch (_) {}
  }
}

// ═══════════════════════════════════════════════════════════════════
// Exec — register readers, run lane_execute, promote kept readers
// ═══════════════════════════════════════════════════════════════════

async function exec(msg) {
  const { jobId, requestBytes, sourceLengths, sinkCount, keepSources,
          heldJobId, opfsFiles } = msg;

  await ensureInit();
  activeSab = msg.sab || null;
  activeWriteSab = msg.writeSab || null;

  const readerIds = [];
  const finalLengths = [];

  // Held readers of the handle this op targets come first (index 0),
  // exactly as the engine moved them into the document.
  if (heldJobId != null) {
    for (const [id, r] of readers) {
      if (r.heldJobId === heldJobId) {
        readerIds.push(id);
        finalLengths.push(r.length);
      }
    }
  }
  const heldCount = readerIds.length;

  // New readers for this op's own sources. An OPFS-backed reader
  // takes OWNERSHIP of its file from the job (the file now dies with
  // the reader, in dropReader — not with the job, in opfsDrop).
  for (let i = 0; i < sourceLengths.length; i++) {
    let handle = null;
    let length = sourceLengths[i];
    const file = (opfsFiles && opfsFiles[i]) || null;
    if (file) {
      handle = await openOpfsHandle(file);
      length = handle.getSize();
      jobFiles.get(jobId)?.delete(file);
    }
    const id = nextReaderId++;
    readers.set(id, { length, opfsHandle: handle, opfsFile: file, heldJobId: null });
    readerIds.push(id);
    finalLengths.push(length);
  }

  const packed = new Uint8Array(finalLengths.length * 8);
  const dv = new DataView(packed.buffer);
  for (let i = 0; i < finalLengths.length; i++) dv.setFloat64(i * 8, finalLengths[i], true);

  activeReaderMap = readerIds.map((rid, i) => [i, rid]);
  let responseBytes;
  try {
    if (ioMode === 'jspi') {
      responseBytes = await callLaneJspi(new Uint8Array(requestBytes), packed, sinkCount);
    } else {
      const { lane_execute } = await import(wasmBaseUrl + 'pdf_oxide.js');
      responseBytes = lane_execute(lanePtr, new Uint8Array(requestBytes), packed, sinkCount);
    }

    // Promote kept readers to held (success responses only — status 1).
    // keepSources indexes the op's OWN sources, which sit after the
    // held block in readerIds.
    const keep = new Set(keepSources || []);
    if (responseBytes.length > 0 && responseBytes[0] === 1 && keep.size > 0) {
      for (const idx of keep) {
        const rid = readerIds[heldCount + idx];
        const r = readers.get(rid);
        if (r) r.heldJobId = jobId;
      }
    }
  } finally {
    // Drop this op's non-promoted readers; held ones stay. Dropping
    // deletes each reader's OPFS file — same agent, no race possible.
    for (const [id, r] of readers) {
      if (r.heldJobId === null) await dropReader(id, r);
    }
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
      P = msg.protocol;
      opfsDirParts = msg.opfsDir || null;
      ioMode = msg.ioMode;
      if (msg.baseUrl) wasmBaseUrl = msg.baseUrl;
      if (msg.wasmModule) precompiledModule = msg.wasmModule;
      try {
        // The liveness lock must be held BEFORE ready: no file may
        // ever exist without the death signal that reclaims it.
        await acquireDirLock();
        await ensureInit();
        const { lane_init } = await import(wasmBaseUrl + 'pdf_oxide.js');
        lanePtr = lane_init();
        self.postMessage({ type: 'ready' });
      } catch (err) {
        self.postMessage({ type: 'error', error: 'WASM init failed: ' + err.message });
      }
      return;
    }

    case 'readAtResult': {
      if (jspiReadResolve) { const r = jspiReadResolve; jspiReadResolve = null; r(msg); }
      return;
    }

    case 'chunkAck': {
      if (jspiWriteResolve) { const r = jspiWriteResolve; jspiWriteResolve = null; r(msg); }
      return;
    }

    case 'releaseHeld': {
      for (const [id, r] of readers) {
        if (r.heldJobId === msg.heldJobId) await dropReader(id, r);
      }
      return;
    }

    case 'opfsDrop': {
      await opfsDrop(msg.jobId);
      return;
    }

    case 'opfsWrite': {
      try {
        await opfsWrite(msg.jobId, msg.filename, msg.chunk, msg.offset, msg.last);
        self.postMessage({ type: 'opfsWriteAck' });
      } catch (err) {
        self.postMessage({ type: 'opfsWriteAck', error: err.message || String(err) });
      }
      return;
    }

    case 'exec': {
      try {
        const responseBytes = await exec(msg);
        // Transfer only the result bytes, never the whole WASM heap
        // (wasm-bindgen may return a view into linear memory).
        const owned = responseBytes.buffer.byteLength === responseBytes.byteLength
            ? responseBytes.buffer
            : responseBytes.buffer.slice(responseBytes.byteOffset, responseBytes.byteOffset + responseBytes.byteLength);
        self.postMessage({ type: 'result', jobId: msg.jobId, data: owned }, [owned]);
      } catch (err) {
        self.postMessage({ type: 'error', jobId: msg.jobId, message: err.message || String(err) });
      }
      return;
    }
  }
};

// Handler is attached — only now is it safe for the host to post.
// Announcing readiness-to-receive (rather than relying on message
// queueing during module evaluation) makes the boot handshake
// correct on every engine: the dynamic import in the cross-origin
// blob bootstrap finishes module evaluation BEFORE this script runs,
// so un-announced messages would be delivered into the void.
self.postMessage({ type: 'booted' });
