// Coordinator Worker — manages WASM worker pool, I/O mode, read/write routing.
//
// Three-level architecture (symmetric with native):
//   Main thread (Dart) ←→ Coordinator (this file) ←→ WASM Worker pool
//
// The coordinator NEVER loads WASM. It's pure JS coordination:
//   - Pool management (acquire, release, cancel, dispose)
//   - I/O mode detection (Atomics / OPFS) — once at startup
//   - Read fulfillment routing: WASM worker → coordinator → main → coordinator → WASM worker
//   - Write chunk routing: WASM worker → coordinator → main
//   - Stream item routing: WASM worker → coordinator → main
//   - OPFS lifecycle (mode 3 only): write, finalize, cleanup

let ioMode = 'opfs';
let poolSize = 2;
const idleWorkers = [];
const busyWorkers = new Map();   // opId → { worker, opfsFile? }
const pendingOps = [];           // queued when pool is full
const opfsFiles = new Set();     // cleanup registry
let nextOpId = 1;
let wasmWorkerUrl = null;

// Pending read requests: opId → { resolve, reject } for read fulfillment
const pendingReads = new Map();

// ── I/O Mode Detection ─────────────────────────────────────────────────

function detectIoMode() {
  if (typeof SharedArrayBuffer !== 'undefined') return 'atomics';
  return 'opfs';
}

// ── OPFS Helpers (mode 3 only) ──────────────────────────────────────────

async function opfsWrite(filename, chunk, offset) {
  const root = await navigator.storage.getDirectory();
  const fileHandle = await root.getFileHandle(filename, { create: true });
  const syncHandle = await fileHandle.createSyncAccessHandle();
  const data = new Uint8Array(chunk);
  syncHandle.write(data, { at: offset });
  syncHandle.flush();
  syncHandle.close();
}

async function opfsCleanup(filename) {
  try {
    const root = await navigator.storage.getDirectory();
    await root.removeEntry(filename);
  } catch (_) { /* ignore */ }
}

async function opfsCleanupAll() {
  for (const f of opfsFiles) {
    await opfsCleanup(f);
  }
  opfsFiles.clear();
}

// ── SharedArrayBuffer for Atomics mode ──────────────────────────────────

// Layout: [0] = status flag (0=waiting, 1=ready, 2=error)
//         [4..8] = response length (int32)
//         [8..] = response data bytes
const SAB_HEADER = 8;
const SAB_MAX_CHUNK = 65536;

function createSab() {
  return new SharedArrayBuffer(SAB_HEADER + SAB_MAX_CHUNK);
}

// ── Worker Pool ─────────────────────────────────────────────────────────

async function createWasmWorker() {

  // Fetch the worker script and create a blob URL.
  // This handles cross-origin scenarios (e.g. dart test where the test
  // runner and asset server are on different ports). The blob worker runs
  // same-origin. Rewrite the relative './pdf_oxide.js' import to absolute
  // so the ES module loader can resolve it from the blob context.
  let workerUrl = wasmWorkerUrl;
  try {
    const resp = await fetch(wasmWorkerUrl);
    if (resp.ok) {
      let src = await resp.text();
      const base = wasmWorkerUrl.substring(0, wasmWorkerUrl.lastIndexOf('/') + 1);
      // Rewrite relative imports to absolute so blob URL can resolve them
      src = src.replace("from './pdf_oxide.js'", "from '" + base + "pdf_oxide.js'");
      const blob = new Blob([src], { type: 'application/javascript' });
      workerUrl = URL.createObjectURL(blob);
    }
  } catch (e) {
  }

  const w = new Worker(workerUrl, { type: 'module' });
  return new Promise((resolve, reject) => {
    let settled = false;
    const timeout = setTimeout(() => {
      if (!settled) {
        settled = true;
        w.terminate();
        reject(new Error('WASM worker init timed out after 10s — module import likely failed (ioMode=' + ioMode + ')'));
      }
    }, 10000);
    const onMsg = (e) => {
      if (e.data.type === 'ready') {
        settled = true;
        clearTimeout(timeout);
        w.removeEventListener('message', onMsg);
        resolve(w);
      } else if (e.data.type === 'error') {
        if (!settled) {
          settled = true;
          clearTimeout(timeout);
          w.removeEventListener('message', onMsg);
          w.terminate();
          reject(new Error('WASM worker init error: ' + (e.data.error || 'unknown')));
        }
      }
    };
    w.addEventListener('message', onMsg);
    w.addEventListener('error', (e) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        reject(new Error('WASM worker failed to start: ' + (e.message || e)));
      }
    });
    w.postMessage({ type: 'init', ioMode });
  });
}

async function acquireWorker() {
  if (idleWorkers.length > 0) {
    return idleWorkers.pop();
  }
  if (busyWorkers.size < poolSize) {
    return await createWasmWorker();
  }
  // Pool full — queue
  return new Promise((resolve) => {
    pendingOps.push(resolve);
  });
}

function releaseWorker(opId) {
  const entry = busyWorkers.get(opId);
  if (!entry) return;
  busyWorkers.delete(opId);
  const worker = entry.worker;

  // Clean OPFS temp file if mode 3
  if (entry.opfsFile) {
    opfsCleanup(entry.opfsFile);
    opfsFiles.delete(entry.opfsFile);
  }

  // If ops are queued, give the worker to the next one
  if (pendingOps.length > 0) {
    const resolve = pendingOps.shift();
    resolve(worker);
  } else {
    idleWorkers.push(worker);
  }
}

function cancelWorker(opId) {
  const entry = busyWorkers.get(opId);
  if (!entry) return;
  entry.worker.terminate();
  busyWorkers.delete(opId);
  if (entry.opfsFile) {
    opfsCleanup(entry.opfsFile);
    opfsFiles.delete(entry.opfsFile);
  }
  // Terminated workers can't be reused — pool shrinks temporarily
  // New worker created on next acquireWorker if needed
}

function disposeAll() {
  for (const [opId, entry] of busyWorkers) {
    entry.worker.terminate();
  }
  busyWorkers.clear();
  for (const w of idleWorkers) {
    w.terminate();
  }
  idleWorkers.length = 0;
  pendingOps.length = 0;
  opfsCleanupAll();
}

// ── WASM Worker Message Handler ─────────────────────────────────────────

function setupWorkerListeners(worker, opId) {
  worker.onmessage = (e) => {
    const msg = e.data;

    switch (msg.type) {
      case 'readAt': {
        // WASM worker needs bytes — forward to main thread
        const readId = `${opId}_${msg.reqId}`;
        // Store the callback for when main responds
        pendingReads.set(readId, {
          opId,
          workerPort: worker,
          mode: msg.mode,
          sab: msg.sab,   // SharedArrayBuffer (atomics mode only)
        });
        // Ask main thread for bytes
        self.postMessage({
          type: 'readAt',
          opId,
          readId,
          offset: msg.offset,
          count: msg.count,
        });
        break;
      }

      case 'chunk': {
        // Output chunk from engine — forward to main
        const transfers = msg.data instanceof ArrayBuffer ? [msg.data] : [];
        self.postMessage({ type: 'chunk', opId, data: msg.data }, transfers);
        break;
      }

      case 'item': {
        // Per-item streaming (image, rendered page) — forward to main
        const transfers = [];
        if (msg.data && msg.data.data instanceof ArrayBuffer) {
          transfers.push(msg.data.data);
        }
        self.postMessage({ type: 'item', opId, data: msg.data }, transfers);
        break;
      }

      case 'itemDone': {
        self.postMessage({ type: 'itemDone', opId });
        break;
      }

      case 'result': {
        // Operation complete — forward result, release worker
        const transfers = [];
        if (msg.result && msg.result.bytes instanceof ArrayBuffer) {
          transfers.push(msg.result.bytes);
        }
        self.postMessage({ type: 'result', opId, result: msg.result }, transfers);
        releaseWorker(opId);
        break;
      }

      case 'error': {
        self.postMessage({ type: 'error', opId, error: msg.error });
        releaseWorker(opId);
        break;
      }
    }
  };

  worker.onerror = (e) => {
    self.postMessage({ type: 'error', opId, error: e.message || 'WASM worker crashed' });
    releaseWorker(opId);
  };
}

// ── Main Thread Message Handler ─────────────────────────────────────────

self.onmessage = async (e) => {
  const msg = e.data;

  try {
    switch (msg.type) {
      case 'init': {
        wasmWorkerUrl = msg.wasmWorkerUrl;
        poolSize = msg.poolSize || Math.max(2, Math.floor((navigator.hardwareConcurrency || 4) / 2));
        ioMode = detectIoMode();
        self.postMessage({ type: 'ready', ioMode, poolSize });
        break;
      }

      case 'readAtResponse': {
        const pending = pendingReads.get(msg.readId);
        pendingReads.delete(msg.readId);

        if (pending.mode === 'atomics' && pending.sab) {
          // Atomics mode: write bytes to SAB, notify
          const view = new Int32Array(pending.sab);
          const dataView = new Uint8Array(pending.sab, SAB_HEADER);
          if (msg.error) {
            Atomics.store(view, 0, 2); // error flag
            Atomics.notify(view, 0);
          } else {
            const bytes = new Uint8Array(msg.bytes);
            dataView.set(bytes);
            Atomics.store(view, 1, bytes.length);
            Atomics.store(view, 0, 1); // ready flag
            Atomics.notify(view, 0);
          }
        } else {
          // Non-atomics: forward bytes to worker via postMessage
          if (msg.error) {
            pending.workerPort.postMessage({
              type: 'readAtResponse',
              reqId: msg.readId.split('_').pop(),
              error: msg.error,
            });
          } else {
            const transfers = msg.bytes instanceof ArrayBuffer ? [msg.bytes] : [];
            pending.workerPort.postMessage({
              type: 'readAtResponse',
              reqId: msg.readId.split('_').pop(),
              bytes: msg.bytes,
            }, transfers);
          }
        }
        break;
      }

      case 'opfs.write': {
        // Mode 3: stream source chunks to OPFS
        const filename = msg.filename;
        opfsFiles.add(filename);
        await opfsWrite(filename, msg.chunk, msg.offset);
        self.postMessage({ type: 'opfs.writeAck', opId: msg.opId });
        break;
      }

      case 'opfs.finalize': {
        // Mode 3: source fully written to OPFS
        self.postMessage({ type: 'opfs.finalizeAck', opId: msg.opId });
        break;
      }

      case 'submit': {
        const opId = nextOpId++;
        try {
          const worker = await acquireWorker();
          const entry = { worker, opfsFile: msg.opfsFile || null };
          busyWorkers.set(opId, entry);
          setupWorkerListeners(worker, opId);

          const fwdMsg = {
            type: 'exec',
            opId,
            op: msg.op,
            args: msg.args,
            ioMode,
          };

          const transfers = [];
          if (msg.args) {
            for (const key of Object.keys(msg.args)) {
              const val = msg.args[key];
              if (val instanceof ArrayBuffer) {
                transfers.push(val);
              }
            }
          }

          if (ioMode === 'atomics') {
            fwdMsg.sab = createSab();
          }

          if (msg.opfsFile) {
            fwdMsg.opfsFile = msg.opfsFile;
          }

          worker.postMessage(fwdMsg, transfers);
          self.postMessage({ type: 'submitted', opId });
        } catch (err) {
          self.postMessage({
            type: 'error',
            opId,
            error: 'Worker acquire failed: ' + (err.message || String(err)),
          });
        }
        break;
      }

      case 'cancel': {
        cancelWorker(msg.opId);
        self.postMessage({ type: 'cancelled', opId: msg.opId });
        break;
      }

      case 'dispose': {
        disposeAll();
        self.postMessage({ type: 'disposed' });
        break;
      }
    }
  } catch (err) {
    self.postMessage({
      type: 'error',
      opId: msg.opId || 0,
      error: err.message || String(err),
    });
  }
};
