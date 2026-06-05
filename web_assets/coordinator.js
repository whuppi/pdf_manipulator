// Coordinator Worker — WASM worker pool manager for one Pdf instance.
//
// One coordinator per Pdf(). Owns N WASM workers, routes ops to them,
// manages handle pinning, and bridges readAt/chunk between Dart and WASM.
//
// ── Message protocol ─────────────────────────────────────────────
//
// Dart → coordinator:
//   init             { workerUrl, poolSize?, forceIoMode?, wasmModule? }
//   submit           { opId, requestBytes, args, opfsFile?, opfsFiles? }
//   submitStream     { opId, requestBytes, args }
//   readAtResponse   { readId, bytes | error }
//   opfs.write       { opId, filename, chunk, offset }
//   opfs.finalize    { opId }
//   cancel           { opId }
//   dispose
//
// coordinator → Dart:
//   ready            { ioMode, poolSize, wasmModule? }
//   submitted        { opId }
//   result           { opId, data }
//   error            { opId?, message }
//   item             { opId, data }
//   done             { opId }
//   streamError      { opId, message }
//   readAt           { opId, sourceOpId, sourceIndex, readId, offset, count }
//   chunk            { opId, sinkIndex, data }
//   opfs.writeAck    { opId }
//   opfs.finalizeAck { opId }
//   cancelled        { opId }
//   disposed
//
// coordinator → worker:
//   exec             { opId, requestBytes, args, ioMode, sab?,
//                      opfsFile?, opfsFiles?, isPinnedOp, pinnedSourceOpId? }
//
// worker → coordinator:
//   readAt           { reqId, sourceIndex, offset, count, mode, sab? }
//   chunk            { sinkIndex, data }
//   result           { data }
//   error            { error }
//
// ── Handle pinning ───────────────────────────────────────────────
//
// open/editorOpen → pin worker to handle. Subsequent ops route there.
// docDispose/editorDispose → unpin. Worker returns to pool.
//
// ── Three I/O modes (auto-detected: jspi > atomics > opfs) ──────
//
//   jspi    — JSPI Promise suspension (Chrome 137+ / Firefox 139+)
//   atomics — SAB + Atomics.wait/notify (needs COOP/COEP)
//   opfs    — Pre-copy to OPFS disk (universal)

// ═══════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════

let ioMode = 'opfs';
let poolSize = 2;
let workerUrl = null;
let wasmBaseUrl = '';
let compiledWasmModule = null;

// ═══════════════════════════════════════════════════════════════════
// I/O mode detection
// ═══════════════════════════════════════════════════════════════════

function detectIoMode(forceMode) {
  if (forceMode === 'atomics' || forceMode === 'opfs' || forceMode === 'jspi') return forceMode;
  if (typeof WebAssembly !== 'undefined' && 'Suspending' in WebAssembly) return 'jspi';
  if (typeof SharedArrayBuffer !== 'undefined') return 'atomics';
  return 'opfs';
}

// ═══════════════════════════════════════════════════════════════════
// SAB for Atomics mode
// ═══════════════════════════════════════════════════════════════════

const SAB_HEADER = 8;
const SAB_MAX_CHUNK = 65536;

function createSab() {
  return new SharedArrayBuffer(SAB_HEADER + SAB_MAX_CHUNK);
}

// Write-ack SAB: 4 bytes for an Int32 flag. Worker Atomics.wait(0),
// coordinator Atomics.store(1) + Atomics.notify after Dart consumes.
function createWriteSab() {
  return new SharedArrayBuffer(4);
}

// ═══════════════════════════════════════════════════════════════════
// Worker pool
// ═══════════════════════════════════════════════════════════════════

/** @type {Worker[]} idle workers ready for work */
const idleWorkers = [];

/** @type {Map<number, {worker: Worker, opfsFile: string|null}>} opId → busy entry */
const busyWorkers = new Map();

/** @type {((w: Worker) => void)[]} queued resolvers waiting for a worker */
const waitQueue = [];

function spawnWorker(url) {
  try {
    const u = new URL(url);
    if (u.origin !== self.location.origin) {
      const blob = new Blob(
        [`importScripts(${JSON.stringify(url)});`],
        { type: 'application/javascript' },
      );
      return new Worker(URL.createObjectURL(blob));
    }
  } catch (_) {}
  return new Worker(url, { type: 'module' });
}

async function createWasmWorker() {
  const w = spawnWorker(workerUrl);
  return new Promise((resolve, reject) => {
    let settled = false;
    const timeout = setTimeout(() => {
      if (!settled) { settled = true; w.terminate(); reject(new Error('WASM worker init timed out')); }
    }, 15000);

    function onMsg(e) {
      if (e.data.type === 'ready' && !settled) {
        settled = true; clearTimeout(timeout); w.removeEventListener('message', onMsg); resolve(w);
      } else if (e.data.type === 'error' && !settled) {
        settled = true; clearTimeout(timeout); w.removeEventListener('message', onMsg);
        w.terminate(); reject(new Error('WASM worker init: ' + (e.data.error || 'unknown')));
      }
    }
    w.addEventListener('message', onMsg);
    w.addEventListener('error', (e) => {
      if (!settled) { settled = true; clearTimeout(timeout); reject(new Error('WASM worker failed: ' + (e.message || e.filename || e))); }
    });

    const initMsg = { type: 'init', ioMode, baseUrl: wasmBaseUrl };
    if (compiledWasmModule) initMsg.wasmModule = compiledWasmModule;
    w.postMessage(initMsg);
  });
}

async function acquireWorker() {
  if (idleWorkers.length > 0) return idleWorkers.pop();
  if (busyWorkers.size < poolSize) return await createWasmWorker();
  return new Promise((resolve) => waitQueue.push(resolve));
}

function releaseWorker(opId) {
  const entry = busyWorkers.get(opId);
  if (!entry) return;
  busyWorkers.delete(opId);
  writeAckState.delete(opId);

  if (entry.opfsFile && !isOpfsHeldByHandle(entry.opfsFile)) {
    opfsCleanup(entry.opfsFile);
    opfsFiles.delete(entry.opfsFile);
  }

  if (waitQueue.length > 0) waitQueue.shift()(entry.worker);
  else idleWorkers.push(entry.worker);
}

// ═══════════════════════════════════════════════════════════════════
// Handle pinning
// ═══════════════════════════════════════════════════════════════════

/** @type {Map<number, {worker: Worker, sourceOpId: number, opfsFile: string|null}>} */
const pinnedHandles = new Map();

/** @type {Map<number, {action: 'pin'|'unpin', handleId: number|null, opfsFile: string|null}>} */
const pendingHandleOps = new Map();

const HANDLE_CREATING_OPS = new Set(['open', 'editorOpen']);
const HANDLE_DISPOSING_OPS = new Set(['docDispose', 'editorDispose']);

function resolveWorkerForHandle(handleId) {
  if (handleId !== null && pinnedHandles.has(handleId)) {
    return { worker: pinnedHandles.get(handleId).worker, isPinned: true };
  }
  return null;
}

function registerHandleOp(opId, opName, handleId, opfsFile) {
  if (HANDLE_CREATING_OPS.has(opName)) {
    pendingHandleOps.set(opId, { action: 'pin', handleId: null, opfsFile: opfsFile || null });
  }
  if (HANDLE_DISPOSING_OPS.has(opName) && handleId !== null) {
    pendingHandleOps.set(opId, { action: 'unpin', handleId, opfsFile: null });
  }
}

function handlePinOnResult(opId, worker, responseData) {
  const hop = pendingHandleOps.get(opId);
  if (!hop) return;
  pendingHandleOps.delete(opId);

  if (hop.action === 'pin' && responseData) {
    const hid = extractHandleId(
      responseData instanceof ArrayBuffer ? new Uint8Array(responseData) : responseData);
    if (hid !== null) {
      pinnedHandles.set(hid, { worker, sourceOpId: opId, opfsFile: hop.opfsFile });
    }
  } else if (hop.action === 'unpin' && hop.handleId !== null) {
    const pin = pinnedHandles.get(hop.handleId);
    pinnedHandles.delete(hop.handleId);
    worker.postMessage({ type: 'releaseOpfs', sourceOpId: pin ? pin.sourceOpId : null });
    if (pin && pin.opfsFile) {
      opfsCleanup(pin.opfsFile);
      opfsFiles.delete(pin.opfsFile);
    }
  }
}

function handlePinOnError(opId) {
  pendingHandleOps.delete(opId);
}

// ═══════════════════════════════════════════════════════════════════
// readAt routing (Dart ↔ worker)
// ═══════════════════════════════════════════════════════════════════

/** @type {Map<string, {opId: number, workerPort: Worker, mode: string, sab: SharedArrayBuffer|null}>} */
const pendingReads = new Map();

/** @type {Map<number, {writeSab: SharedArrayBuffer|null, worker: Worker}>} write backpressure per opId */
const writeAckState = new Map();

function routeReadAt(msg, opId, targetHandleId, worker) {
  let sourceOpId = opId;
  if (targetHandleId !== null) {
    const pin = pinnedHandles.get(targetHandleId);
    if (pin) sourceOpId = pin.sourceOpId;
  }

  const readId = `${opId}_${msg.reqId}`;
  pendingReads.set(readId, { opId, workerPort: worker, mode: msg.mode, sab: msg.sab || null });
  self.postMessage({
    type: 'readAt', opId, sourceOpId, readId,
    sourceIndex: msg.sourceIndex ?? 0,
    offset: msg.offset, count: msg.count,
  });
}

function deliverReadAtResponse(msg) {
  const pending = pendingReads.get(msg.readId);
  if (!pending) return;
  pendingReads.delete(msg.readId);

  if (pending.mode === 'atomics' && pending.sab) {
    const view = new Int32Array(pending.sab);
    if (msg.error) {
      Atomics.store(view, 0, 2);
      Atomics.notify(view, 0);
    } else {
      const bytes = new Uint8Array(msg.bytes);
      new Uint8Array(pending.sab, SAB_HEADER).set(bytes);
      Atomics.store(new Int32Array(pending.sab, 4, 1), 0, bytes.length);
      Atomics.store(view, 0, 1);
      Atomics.notify(view, 0);
    }
  } else {
    const reqId = msg.readId.split('_').pop();
    if (msg.error) {
      pending.workerPort.postMessage({ type: 'readAtResponse', reqId, error: msg.error });
    } else {
      const transfers = msg.bytes instanceof ArrayBuffer ? [msg.bytes] : [];
      pending.workerPort.postMessage({ type: 'readAtResponse', reqId, bytes: msg.bytes }, transfers);
    }
  }
}

// ═══════════════════════════════════════════════════════════════════
// OPFS helpers
// ═══════════════════════════════════════════════════════════════════

/** @type {Set<string>} filenames currently on disk */
const opfsFiles = new Set();

async function opfsWrite(filename, chunk, offset) {
  const root = await navigator.storage.getDirectory();
  const fh = await root.getFileHandle(filename, { create: true });
  const sh = await fh.createSyncAccessHandle();
  sh.write(new Uint8Array(chunk), { at: offset });
  sh.flush();
  sh.close();
}

async function opfsCleanup(filename) {
  try { const root = await navigator.storage.getDirectory(); await root.removeEntry(filename); } catch (_) {}
}

async function opfsCleanupAll() {
  for (const f of opfsFiles) await opfsCleanup(f);
  opfsFiles.clear();
}

function isOpfsHeldByHandle(filename) {
  for (const pin of pinnedHandles.values()) {
    if (pin.opfsFile === filename) return true;
  }
  return false;
}

// ═══════════════════════════════════════════════════════════════════
// Binary protocol helpers
// ═══════════════════════════════════════════════════════════════════

function extractOpName(requestBytes) {
  if (!requestBytes || requestBytes.byteLength < 2) return null;
  const view = new Uint8Array(requestBytes);
  const opLen = view[0];
  if (opLen === 0 || view.byteLength < 1 + opLen) return null;
  return String.fromCharCode(...view.slice(1, 1 + opLen));
}

function extractHandleId(requestBytes) {
  if (!requestBytes) return null;
  let view;
  if (requestBytes instanceof ArrayBuffer) view = new Uint8Array(requestBytes);
  else if (requestBytes instanceof Uint8Array) view = requestBytes;
  else if (requestBytes.buffer) view = new Uint8Array(requestBytes.buffer);
  else return null;

  const needle = [104, 97, 110, 100, 108, 101, 73, 100]; // "handleId"
  for (let i = 0; i <= view.length - needle.length - 5; i++) {
    let match = true;
    for (let j = 0; j < needle.length; j++) {
      if (view[i + j] !== needle[j]) { match = false; break; }
    }
    if (match && view[i + needle.length] === 1) {
      const o = i + needle.length + 1;
      return view[o] | (view[o+1] << 8) | (view[o+2] << 16) | (view[o+3] << 24);
    }
  }
  return null;
}

// ═══════════════════════════════════════════════════════════════════
// Worker message listener (installed per-op)
// ═══════════════════════════════════════════════════════════════════

function setupWorkerListener(worker, opId, targetHandleId, isStream) {
  worker.onmessage = (e) => {
    const msg = e.data;
    if (msg.type === '_wlog') return;

    switch (msg.type) {
      case 'readAt':
        routeReadAt(msg, opId, targetHandleId, worker);
        break;

      case 'chunk': {
        const transfers = msg.data instanceof ArrayBuffer ? [msg.data] : [];
        // Forward chunk to Dart. Dart acks via chunkAck message (see onmessage handler below).
        // Backpressure: Atomics mode acks via writeSab (see submitOp), JSPI via chunkAck forward.
        self.postMessage({ type: 'chunk', opId, sinkIndex: msg.sinkIndex ?? 0, data: msg.data }, transfers);
        break;
      }

      case 'result': {
        handlePinOnResult(opId, worker, msg.data);
        if (isStream) {
          const transfers = msg.data instanceof ArrayBuffer ? [msg.data] : [];
          self.postMessage({ type: 'item', opId, data: msg.data }, transfers);
          self.postMessage({ type: 'done', opId });
        } else {
          const transfers = msg.data instanceof ArrayBuffer ? [msg.data] : [];
          self.postMessage({ type: 'result', opId, data: msg.data }, transfers);
        }
        releaseWorker(opId);
        break;
      }

      case 'error': {
        handlePinOnError(opId);
        self.postMessage({ type: isStream ? 'streamError' : 'error', opId, message: msg.error });
        releaseWorker(opId);
        break;
      }
    }
  };

  worker.onerror = (e) => {
    self.postMessage({ type: 'error', opId, message: e.message || 'WASM worker crashed' });
    releaseWorker(opId);
  };
}

// ═══════════════════════════════════════════════════════════════════
// Submit an op to the pool
// ═══════════════════════════════════════════════════════════════════

let nextOpId = 1;

async function submitOp(msg, isStream) {
  const opId = msg.opId ?? nextOpId++;
  const opName = extractOpName(msg.requestBytes);
  const handleId = extractHandleId(msg.requestBytes);

  // Route to pinned worker or acquire from pool
  let worker;
  let isPinnedOp = false;
  const pinned = resolveWorkerForHandle(handleId);
  if (pinned) {
    worker = pinned.worker;
    isPinnedOp = true;
  } else {
    worker = await acquireWorker();
  }

  busyWorkers.set(opId, { worker, opfsFile: msg.opfsFile || null });
  setupWorkerListener(worker, opId, handleId, isStream);
  if (!isStream) registerHandleOp(opId, opName, handleId, msg.opfsFile);

  // Build exec message for worker
  const fwdMsg = {
    type: 'exec', opId,
    requestBytes: msg.requestBytes,
    args: msg.args || {},
    ioMode, isPinnedOp,
    pinnedSourceOpId: pinned ? pinnedHandles.get(handleId)?.sourceOpId : null,
  };

  const transfers = msg.requestBytes instanceof ArrayBuffer ? [msg.requestBytes] : [];
  if (ioMode === 'atomics') {
    fwdMsg.sab = createSab();
    fwdMsg.writeSab = createWriteSab();
    writeAckState.set(opId, { writeSab: fwdMsg.writeSab, worker });
  } else if (ioMode === 'jspi') {
    writeAckState.set(opId, { writeSab: null, worker });
  }
  if (msg.opfsFile) fwdMsg.opfsFile = msg.opfsFile;
  if (msg.opfsFiles) fwdMsg.opfsFiles = msg.opfsFiles;

  worker.postMessage(fwdMsg, transfers);
  self.postMessage({ type: 'submitted', opId });
}

// ═══════════════════════════════════════════════════════════════════
// Cancel + dispose
// ═══════════════════════════════════════════════════════════════════

async function shutdownWorker(w) {
  return new Promise(resolve => {
    const timeout = setTimeout(() => { w.terminate(); resolve(); }, 2000);
    try {
      w.onmessage = (e) => {
        if (e.data.type === 'shutdown_done') { clearTimeout(timeout); w.terminate(); resolve(); }
      };
      w.postMessage({ type: 'shutdown' });
    } catch (_) { clearTimeout(timeout); w.terminate(); resolve(); }
  });
}

async function cancelWorker(opId) {
  const entry = busyWorkers.get(opId);
  if (!entry) return;
  busyWorkers.delete(opId);
  await shutdownWorker(entry.worker);
  if (entry.opfsFile && !isOpfsHeldByHandle(entry.opfsFile)) {
    await opfsCleanup(entry.opfsFile);
    opfsFiles.delete(entry.opfsFile);
  }
}

async function disposeAll() {
  const allWorkers = new Set();
  for (const [, entry] of busyWorkers) allWorkers.add(entry.worker);
  for (const w of idleWorkers) allWorkers.add(w);
  await Promise.all([...allWorkers].map(shutdownWorker));

  busyWorkers.clear();
  idleWorkers.length = 0;
  waitQueue.length = 0;
  pinnedHandles.clear();
  pendingHandleOps.clear();
  pendingReads.clear();
  writeAckState.clear();
  await opfsCleanupAll();
}

// ═══════════════════════════════════════════════════════════════════
// Main thread message handler
// ═══════════════════════════════════════════════════════════════════

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    switch (msg.type) {
      case 'init': {
        workerUrl = msg.workerUrl;
        poolSize = msg.poolSize || Math.max(2, Math.floor((navigator.hardwareConcurrency || 4) / 2));
        ioMode = detectIoMode(msg.forceIoMode);
        try {
          wasmBaseUrl = new URL(workerUrl, self.location.href).href.replace(/\/[^/]*$/, '/');
        } catch (_) { wasmBaseUrl = './'; }

        if (msg.wasmModule) compiledWasmModule = msg.wasmModule;
        if (!compiledWasmModule) {
          try {
            compiledWasmModule = await WebAssembly.compileStreaming(fetch(wasmBaseUrl + 'pdf_oxide_bg.wasm'));
          } catch (_) { compiledWasmModule = null; }
        }

        const readyMsg = { type: 'ready', ioMode, poolSize };
        if (compiledWasmModule) readyMsg.wasmModule = compiledWasmModule;
        self.postMessage(readyMsg);
        break;
      }

      case 'readAtResponse': deliverReadAtResponse(msg); break;

      case 'chunkAck': {
        const state = writeAckState.get(msg.opId);
        if (!state) break;
        if (state.writeSab) {
          // Atomics mode: set SAB flag and notify the blocked worker
          Atomics.store(new Int32Array(state.writeSab), 0, 1);
          Atomics.notify(new Int32Array(state.writeSab), 0);
        } else {
          // JSPI mode: forward ack to worker — resolves the write Promise
          state.worker.postMessage({ type: 'chunkAck' });
        }
        break;
      }

      case 'opfs.write':
        opfsFiles.add(msg.filename);
        await opfsWrite(msg.filename, msg.chunk, msg.offset);
        self.postMessage({ type: 'opfs.writeAck', opId: msg.opId });
        break;

      case 'opfs.finalize':
        self.postMessage({ type: 'opfs.finalizeAck', opId: msg.opId });
        break;

      case 'submit':       await submitOp(msg, false); break;
      case 'submitStream':  await submitOp(msg, true); break;

      case 'cancel':
        await cancelWorker(msg.opId);
        self.postMessage({ type: 'cancelled', opId: msg.opId });
        break;

      case 'dispose':
        await disposeAll();
        self.postMessage({ type: 'disposed' });
        break;
    }
  } catch (err) {
    self.postMessage({ type: 'error', opId: msg.opId || 0, message: err.message || String(err) });
  }
};
