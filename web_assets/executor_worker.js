// executor_worker.js — the web executor BODY. One Worker = one executor =
// one EngineState (created via engine_state_new in WASM linear memory).
//
// The web twin of host/native/executor.rs. DECISION-FREE by design:
// routing, pinning decisions, queuing, cancellation bookkeeping all
// live in the Dart Router/WebLease. This file only translates
// messages into platform calls — load WASM, serve host_read_at /
// host_write_chunk per I/O mode, run execute, post the result.
//
// Numeric protocol codes are INJECTED via the init message (see
// protocol.dart — the single source of truth). This file
// declares no numeric constants of its own.
//
// ── Message protocol (tags pinned by protocol.dart + parity test)
//
// Every message after the boot handshake (init, booted, ready, bootfailed)
// carries `lease`, a number from one
// counter per worker, never reused. The worker serves the current lease; a
// larger one replaces it when the worker is idle; anything else is
// not served. An exec, reset or opfsWrite of a larger lease that the busy
// worker cannot admit is answered with an `error` (protocol: true) under its
// own lease; every other message of another lease is dropped unread. Every
// message the worker posts echoes the lease of the message it answers.
//
// Dart → worker:
//   init         { protocol, ioMode, baseUrl?, wasmModule? } → ready|error
//   exec         { lease, jobId, requestBytes, sourceLengths: number[],
//                  sinkCount, keepSources: number[], holdGroupId?,
//                  opfsFiles: (string|null)[], sab?, writeSab?,
//                  cancelSab?, cancelUrl? }
//                  (cancelSab: the job's cancel word, where the page has
//                   SharedArrayBuffer; cancelUrl: otherwise, a blob URL the
//                   main thread revokes to cancel, polled by sync XHR)
//                → result { lease, jobId, data, mem, peak }
//                | error { lease, jobId, message, trap?, protocol?, mem, peak }
//                  (trap: the wasm instance trapped; the lane must end;
//                   protocol: the exec was refused because the worker was
//                   busy with another exec or a reset, or not serving its
//                   lease, and never started)
//   reset        { lease } → resetDone { lease, mem, peak } | error { lease,
//                  message, protocol? }: a fresh engine state in this
//                  worker, same wasm instance. Refused while busy.
//                  (peak: the most bytes the engine's heap held since the
//                   last reset, which the host keeps an executor by; mem:
//                   the linear memory's size, a diagnostic that never falls;
//                   abandoned: the pre-copy files set aside because their
//                   handle creation never answered, not removed yet)
//   readAtResult { lease, bytes? , error?, cancelled? }    (JSPI reads)
//   chunkAck     { lease, error?, cancelled? }             (JSPI writes)
//   releaseHold  { lease, holdGroupId }
//   opfsWrite    { lease, jobId, filename, chunk, offset, last }
//                → opfsWriteAck | error { lease, jobId, protocol: true, message }
//   opfsDrop     { lease, jobId }
//
// worker → Dart:
//   ready | readAt { lease, sourceIndex, offset, count } | chunk { lease,
//   sinkIndex, data } | result | error | resetDone | opfsWriteAck { lease,
//   jobId, offset, error?, retried? } (jobId and offset echo the opfsWrite it
//   answers, so a late ack cannot complete the write of another job; retried:
//   the handle had to be created a second time)
//
// ── Three I/O modes (selected by Dart, passed in init) ─────────────
//   jspi    — JSPI Promise suspension (host_read_at returns Promise)
//   atomics — per-job SAB + Atomics.wait; Dart fills + notifies
//   opfs    — sources pre-copied to OPFS; reads via SyncAccessHandle
//
// Terminating the worker is worker.terminate() from Dart — this file never sees it.

// ═══════════════════════════════════════════════════════════════════
// State
// ═══════════════════════════════════════════════════════════════════

let P = null;               // injected protocol codes
let wasm = null;            // raw WASM exports
let statePtr = 0;            // EngineState pointer (engine_state_new)
let lease = 0;              // the lease being served; 0 before the first
let busy = null;            // 'exec' | 'reset' while one runs, else null
let ioMode = 'opfs';
let initialized = false;
let precompiledModule = null;
let wasmBaseUrl = (() => {
  try { return new URL(self.location.href).href.replace(/\/[^/]*$/, '/'); }
  catch (_) { return './'; }
})();

// ═══════════════════════════════════════════════════════════════════
// Reader registry — no decisions. The Dart side decides WHICH
// sources are kept (keepSources) and WHICH held set an op reuses
// (holdGroupId); this registry just stores and looks up.
// ═══════════════════════════════════════════════════════════════════

const readers = new Map();   // readerId → {length, opfsHandle, opfsFile, holdGroupId|null}
let nextReaderId = 1;

// ═══════════════════════════════════════════════════════════════════
// Lease and single-exec wall
// ═══════════════════════════════════════════════════════════════════

// A message belongs to the lease being served, or replaces it on
// an idle worker. Ids only count up, so a smaller one is a message from a
// lease that has ended.
function admit(msg) {
  if (msg.lease === lease) return true;
  if (typeof msg.lease === 'number' && msg.lease > lease && busy === null) {
    lease = msg.lease;
    return true;
  }
  return false;
}

// A message of a lease that has ended gets no answer: its adapter is gone.
// A newer one that is not admitted (the worker is busy) is a live adapter
// waiting for an answer, so it gets one under its own lease.
function refuseLease(msg, what) {
  if (typeof msg.lease !== 'number' || msg.lease < lease) return;
  self.postMessage({
    type: 'error', lease: msg.lease, jobId: msg.jobId, protocol: true,
    message: what + ' refused: lease ' + msg.lease + ' is not admitted while the worker serves ' + lease,
  });
}

// A handler is not re-entrant: under JSPI a second exec arrives while the
// first is suspended, and a reset would pull the engine state from under
// it. The adapter never sends one, so this answers a bug, not a case.
function refuseBusy(msg, what) {
  self.postMessage({
    type: 'error', lease: msg.lease, jobId: msg.jobId, protocol: true,
    message: what + ' refused: the worker is busy with an ' + busy,
  });
}

// Housekeeping handlers that await OPFS; a reset waits for them to settle.
// A pre-copy write is none of them: it can wait on a browser call that never
// answers, and a reset must not wait for that.
const housekeeping = new Set();

function track(promise) {
  housekeeping.add(promise);
  promise.finally(() => housekeeping.delete(promise));
  return promise;
}

let activeReaderMap = null;  // sourceIndex → readerId, per exec
let activeSab = null;        // per-exec read SAB (atomics)
let activeWriteSab = null;   // per-exec write-ack SAB (atomics)
let activeCancelSab = null;  // per-exec cancel word, any mode with SharedArrayBuffer
let activeCancelUrl = null;  // per-exec blob URL, revoked by the main thread to cancel
let cancelSeen = 0;          // latched once the URL stopped answering
let lastPollAt = 0;          // performance.now() of the last URL poll

// One sync XHR costs about 0.1 ms in Chrome, so polling at most every 10 ms
// takes about 1% of a CPU-bound job and keeps the cancel latency near 10 ms.
const CANCEL_POLL_MS = 10;
// A poll that has not answered by now is a browser too busy to serve it, not
// a revoked URL (that fails at once): the job is not cancelled by it.
const CANCEL_POLL_TIMEOUT_MS = 500;
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
    // Awaited: whoever drops a reader (exec's finally, releaseHold, reset)
    // must not return while its file is still being deleted.
    try { await (await laneDir()).removeEntry(r.opfsFile); } catch (_) {}
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
  self.postMessage({ type: 'readAt', lease, sourceIndex, offset, count });
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
    self.postMessage({ type: 'readAt', lease, sourceIndex, offset, count });
  });
}

// ═══════════════════════════════════════════════════════════════════
// host_cancelled — called by WASM from inside the engine's CPU loops
// ═══════════════════════════════════════════════════════════════════

// Synchronous in every mode (never in JSPI_ASYNC_IMPORTS): it reads
// the job's cancel word, or polls its cancel URL, and never suspends.
function hostCancelled() {
  if (activeCancelSab) return Atomics.load(new Int32Array(activeCancelSab), 0);
  if (!activeCancelUrl) return 0;
  if (cancelSeen) return 1;
  const now = performance.now();
  if (now - lastPollAt >= CANCEL_POLL_MS) {
    try {
      const xhr = new XMLHttpRequest();
      xhr.open('GET', activeCancelUrl, false);
      xhr.timeout = CANCEL_POLL_TIMEOUT_MS;
      xhr.send();
      if (xhr.status !== 200) cancelSeen = 1;
    } catch (err) {
      if (!(err && err.name === 'TimeoutError')) cancelSeen = 1;
    }
    lastPollAt = now;
  }
  return cancelSeen;
}

// ═══════════════════════════════════════════════════════════════════
// host_write_chunk — called by WASM, with backpressure
// ═══════════════════════════════════════════════════════════════════

function hostWriteChunk(sinkIndex, bufPtr, len) {
  if (hostCancelled() !== 0) return P.hostIoCancelled;
  const chunk = new Uint8Array(wasm.memory.buffer).slice(bufPtr, bufPtr + len);
  switch (ioMode) {
    case 'atomics': return writeViaAtomics(sinkIndex, chunk);
    case 'jspi':    return writeViaJspi(sinkIndex, chunk);
    default:
      // OPFS: no blocking primitive — fire-and-forget (the producer
      // is disk-read bound, never outruns the main-thread consumer).
      self.postMessage({ type: 'chunk', lease, sinkIndex, data: chunk.buffer }, [chunk.buffer]);
      return 0;
  }
}

function writeViaAtomics(sinkIndex, chunk) {
  if (!activeWriteSab) {
    self.postMessage({ type: 'chunk', lease, sinkIndex, data: chunk.buffer }, [chunk.buffer]);
    return 0;
  }
  const status = new Int32Array(activeWriteSab);
  Atomics.store(status, 0, P.sabPending);
  self.postMessage({ type: 'chunk', lease, sinkIndex, data: chunk.buffer }, [chunk.buffer]);
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
    self.postMessage({ type: 'chunk', lease, sinkIndex, data: chunk.buffer }, [chunk.buffer]);
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
  self.host_cancelled = hostCancelled;

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
    jspiLaneExecute = WebAssembly.promising(wasm.execute);
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
// glue's own calling convention for execute.
//
// Buffers go through executor_alloc/executor_dealloc — never the glue's numbered
// __wbindgen_export_N aliases: those are renumbered whenever the module's
// export set changes, which turns into memory corruption here.
// (__wbindgen_add_to_stack_pointer is a fixed name and safe.)
async function callLaneJspi(requestBytes, sourceLengthsPacked, sinkCount) {
  const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
  const ptr0 = wasm.executor_alloc(requestBytes.length) >>> 0;
  if (requestBytes.length && !ptr0) throw new Error('executor_alloc failed');
  new Uint8Array(wasm.memory.buffer).set(requestBytes, ptr0);
  const ptr1 = wasm.executor_alloc(sourceLengthsPacked.length) >>> 0;
  if (sourceLengthsPacked.length && !ptr1) throw new Error('executor_alloc failed');
  new Uint8Array(wasm.memory.buffer).set(sourceLengthsPacked, ptr1);
  try {
    await jspiLaneExecute(retptr, statePtr, ptr0, requestBytes.length, ptr1, sourceLengthsPacked.length, sinkCount);
    const mem = new DataView(wasm.memory.buffer);
    const r0 = mem.getInt32(retptr, true);
    const r1 = mem.getInt32(retptr + 4, true);
    const result = new Uint8Array(wasm.memory.buffer, r0, r1).slice();
    wasm.executor_dealloc(r0, r1);
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
// from before the first file exists until the browser terminates the
// agent) is what lets the host prove death before reclaiming the
// directory: the lock release on agent termination is a spec
// guarantee, the web's equivalent of the native executor's
// every-job-posts-exactly-once contract.
let opfsDirParts = null;
let livenessLockName = null;
let laneDirPromise = null;

// Held from before `ready` until this agent dies, in every I/O mode.
// The host's worker budget frees this worker's slot only when it can
// acquire the lock — the browser's own word that the worker, and the
// WASM memory behind it, is gone. OPFS reclaim keys on the same grant.
function acquireLivenessLock() {
  if (!livenessLockName || !('locks' in navigator)) return Promise.resolve();
  return new Promise((granted) => {
    navigator.locks.request(livenessLockName, () => {
      granted();
      return new Promise(() => {}); // held until this agent dies
    });
  });
}

// A rejection is not kept: the next call asks the browser again.
function laneDir() {
  if (!laneDirPromise) {
    const attempt = (async () => {
      let dir = await navigator.storage.getDirectory();
      for (const part of (opfsDirParts || [])) {
        dir = await dir.getDirectoryHandle(part, { create: true });
      }
      return dir;
    })();
    attempt.catch(() => { if (laneDirPromise === attempt) laneDirPromise = null; });
    laneDirPromise = attempt;
  }
  return laneDirPromise;
}

// filename → SyncAccessHandle: opened by the pre-copy, kept open after its last
// chunk, and handed to the reader of that file by exec. A handle that is closed
// and opened again is refused while the browser still releases the first.
const opfsWriteHandles = new Map();
const jobFiles = new Map();          // jobId → Set(filename), pre-exec owner

// createSyncAccessHandle on a fresh file sometimes returns a promise that never
// settles while the thread runs on. Nothing may wait for such a call but the
// write that made it: the file is set aside, and the call's own continuation
// closes the handle and removes the file if it ever settles.
const opening = new Map();           // file name → { jobId, call, created, began }, no handle yet
const abandonedFiles = new Map();    // file name → jobId, set aside until its call settles
const physicalNames = new Map();     // pre-copy name → file name, once a retry renamed it
const TIMED_OUT = Symbol('timed out');

function startOpen(jobId, name) {
  const open = { jobId, call: 'getFileHandle', created: null, began: null, cleanup: null };
  let begin;
  open.began = new Promise((resolve) => { begin = resolve; });
  open.created = (async () => {
    const dir = await laneDir();
    const fh = await dir.getFileHandle(name, { create: true });
    open.call = 'createSyncAccessHandle';
    begin();
    return fh.createSyncAccessHandle();
  })();
  opening.set(name, open);
  return open;
}

function setAside(name) {
  const open = opening.get(name);
  opening.delete(name);
  abandonedFiles.set(name, open.jobId);
  open.cleanup = finishAbandoned(name, open.created);
}

async function finishAbandoned(name, created) {
  try { (await created).close(); } catch (_) {}
  try { await (await laneDir()).removeEntry(name); } catch (_) {}
  abandonedFiles.delete(name);
}

// The handle of a fresh pre-copy file. The create is given kCreateHandleWait
// to answer; past it the file is set aside and the write tries once more on a
// fresh file. Null when the job was dropped meanwhile.
async function openHandle(jobId, filename) {
  let retried = false;
  for (let attempt = 0; ; attempt++) {
    const name = attempt === 0 ? filename : filename + '_retry';
    physicalNames.set(filename, name);
    const open = startOpen(jobId, name);
    let timer;
    const late = open.began.then(() => new Promise((resolve) => {
      timer = setTimeout(resolve, P.createHandleWaitMs, TIMED_OUT);
    }));
    let sh;
    try {
      sh = await Promise.race([open.created, late]);
    } catch (err) {
      if (opening.get(name) === open) opening.delete(name); else await open.cleanup;
      throw err;
    } finally {
      clearTimeout(timer);
    }
    if (sh === TIMED_OUT) {
      if (opening.get(name) !== open) return { sh: null, retried };
      setAside(name);
      if (attempt > 0) {
        const err = new Error(
          'createSyncAccessHandle did not answer for ' + P.createHandleWaitMs + ' ms, twice, on ' +
          filename + ' and ' + name);
        err.retried = true;
        throw err;
      }
      retried = true;
      continue;
    }
    if (opening.get(name) !== open) { await open.cleanup; return { sh: null, retried }; }
    opening.delete(name);
    opfsWriteHandles.set(filename, sh);
    return { sh, retried };
  }
}

// Resolves with whether the handle was created twice.
async function opfsWrite(jobId, filename, chunk, offset, last) {
  let files = jobFiles.get(jobId);
  if (!files) { files = new Set(); jobFiles.set(jobId, files); }
  files.add(filename); // registered before the first byte hits disk

  let sh = opfsWriteHandles.get(filename);
  let retried = false;
  if (!sh) {
    ({ sh, retried } = await openHandle(jobId, filename));
    if (!sh) return retried;
  }
  sh.write(new Uint8Array(chunk), { at: offset });
  if (last) sh.flush();
  return retried;
}

// A job is over: delete every file it still owns. Files that exec
// transferred to readers are no longer here — they die with their
// reader (dropReader). Idempotent; an empty set is a no-op. A file whose
// handle is still being created is set aside, never waited for.
async function opfsDrop(jobId) {
  for (const [name, open] of opening) {
    if (open.jobId === jobId) setAside(name);
  }
  const files = jobFiles.get(jobId);
  jobFiles.delete(jobId);
  if (!files) return;
  const dir = await laneDir();
  for (const f of files) {
    const sh = opfsWriteHandles.get(f);
    if (sh) { try { sh.close(); } catch (_) {} opfsWriteHandles.delete(f); }
    const name = physicalNames.get(f) ?? f;
    physicalNames.delete(f);
    if (abandonedFiles.has(name)) continue;
    try { await dir.removeEntry(name); } catch (_) {}
  }
}

// Deletes every file in this worker's own directory but those set aside,
// which their pending call removes. Every other handle is closed first, so no
// reaping is pending and nothing needs retrying.
async function emptyLaneDir() {
  const dir = await laneDir();
  const names = [];
  for await (const name of dir.keys()) names.push(name);
  for (const name of names) {
    if (!abandonedFiles.has(name)) await dir.removeEntry(name);
  }
}

// ═══════════════════════════════════════════════════════════════════
// Reset — a fresh engine state in the same worker
// ═══════════════════════════════════════════════════════════════════

// Drops every document, held reader and job file, and builds a new
// EngineState. The wasm instance stays: its heap does not shrink here.
async function resetLane() {
  for (const name of [...opening.keys()]) setAside(name);
  await Promise.allSettled([...housekeeping]);
  for (const [id, r] of [...readers]) await dropReader(id, r);
  for (const sh of opfsWriteHandles.values()) { try { sh.close(); } catch (_) {} }
  opfsWriteHandles.clear();
  physicalNames.clear();
  jobFiles.clear();
  if (opfsDirParts) await emptyLaneDir();
  jspiReadResolve = null;
  jspiWriteResolve = null;
  const { engine_state_new, engine_state_drop } = await import(wasmBaseUrl + 'pdf_oxide.js');
  engine_state_drop(statePtr);
  statePtr = engine_state_new();
}

// The engine's peak heap since the last reset. After a trap the instance
// cannot be called: the peak is then null.
function heapPeak() {
  try {
    return wasm.heap_peak() >>> 0;
  } catch (_) {
    return null;
  }
}

// ═══════════════════════════════════════════════════════════════════
// Exec — register readers, run execute, promote kept readers
// ═══════════════════════════════════════════════════════════════════

async function exec(msg) {
  const { jobId, requestBytes, sourceLengths, sinkCount, keepSources,
          holdGroupId, opfsFiles } = msg;

  await ensureInit();
  activeSab = msg.sab || null;
  activeWriteSab = msg.writeSab || null;
  activeCancelSab = msg.cancelSab || null;
  activeCancelUrl = msg.cancelUrl || null;
  cancelSeen = 0;
  lastPollAt = performance.now();

  const readerIds = [];
  const finalLengths = [];

  // Held readers of the handle this op targets come first (index 0),
  // exactly as the engine moved them into the document.
  if (holdGroupId != null) {
    for (const [id, r] of readers) {
      if (r.holdGroupId === holdGroupId) {
        readerIds.push(id);
        finalLengths.push(r.length);
      }
    }
  }
  const heldCount = readerIds.length;

  // New readers for this op's own sources. An OPFS-backed reader
  // takes OWNERSHIP of its file, and of the handle that wrote it, from the
  // job (both end with the reader, in dropReader — not with the job, in
  // opfsDrop).
  for (let i = 0; i < sourceLengths.length; i++) {
    let handle = null;
    let length = sourceLengths[i];
    const file = (opfsFiles && opfsFiles[i]) || null;
    let onDisk = file;
    if (file) {
      handle = opfsWriteHandles.get(file);
      if (!handle) throw new Error('pre-copy file ' + file + ' was not written');
      opfsWriteHandles.delete(file);
      onDisk = physicalNames.get(file) ?? file;
      physicalNames.delete(file);
      length = handle.getSize();
      jobFiles.get(jobId)?.delete(file);
    }
    const id = nextReaderId++;
    readers.set(id, { length, opfsHandle: handle, opfsFile: onDisk, holdGroupId: null });
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
      const { execute } = await import(wasmBaseUrl + 'pdf_oxide.js');
      responseBytes = execute(statePtr, new Uint8Array(requestBytes), packed, sinkCount);
    }

    // Promote kept readers to held (success responses only — status 1).
    // keepSources indexes the op's OWN sources, which sit after the
    // held block in readerIds. They join the handle's group when the
    // op targets an existing handle, else the job starts the group.
    const keep = new Set(keepSources || []);
    if (responseBytes.length > 0 && responseBytes[0] === 1 && keep.size > 0) {
      for (const idx of keep) {
        const rid = readerIds[heldCount + idx];
        const r = readers.get(rid);
        if (r) r.holdGroupId = holdGroupId ?? jobId;
      }
    }
  } finally {
    // Drop this op's non-promoted readers; held ones stay. Dropping
    // deletes each reader's OPFS file — same agent, no race possible.
    for (const [id, r] of readers) {
      if (r.holdGroupId === null) await dropReader(id, r);
    }
    activeReaderMap = null;
    activeSab = null;
    activeWriteSab = null;
    activeCancelSab = null;
    activeCancelUrl = null;
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
      livenessLockName = msg.livenessLock || null;
      ioMode = msg.ioMode;
      if (msg.baseUrl) wasmBaseUrl = msg.baseUrl;
      if (msg.wasmModule) precompiledModule = msg.wasmModule;
      try {
        // The liveness lock must be held BEFORE the WASM instance
        // exists: the host's budget slot, and any OPFS file, must
        // never outlive the death signal that releases them.
        await acquireLivenessLock();
        await ensureInit();
        const { engine_state_new } = await import(wasmBaseUrl + 'pdf_oxide.js');
        statePtr = engine_state_new();
        self.postMessage({ type: 'ready' });
      } catch (err) {
        self.postMessage({ type: 'error', error: 'WASM init failed: ' + err.message });
      }
      return;
    }

    case 'readAtResult': {
      if (!admit(msg)) return;
      if (jspiReadResolve) { const r = jspiReadResolve; jspiReadResolve = null; r(msg); }
      return;
    }

    case 'chunkAck': {
      if (!admit(msg)) return;
      if (jspiWriteResolve) { const r = jspiWriteResolve; jspiWriteResolve = null; r(msg); }
      return;
    }

    case 'releaseHold': {
      if (!admit(msg)) return;
      await track((async () => {
        for (const [id, r] of readers) {
          if (r.holdGroupId === msg.holdGroupId) await dropReader(id, r);
        }
      })());
      return;
    }

    case 'opfsDrop': {
      if (!admit(msg)) return;
      await track(opfsDrop(msg.jobId));
      return;
    }

    case 'opfsWrite': {
      if (!admit(msg)) { refuseLease(msg, 'opfsWrite'); return; }
      // The ack carries the lease of its write: one set aside answers late,
      // when another lease may be waiting for a write of the same job and offset.
      try {
        const retried = await opfsWrite(msg.jobId, msg.filename, msg.chunk, msg.offset, msg.last);
        self.postMessage({ type: 'opfsWriteAck', lease: msg.lease, jobId: msg.jobId, offset: msg.offset, retried: retried || undefined });
      } catch (err) {
        self.postMessage({ type: 'opfsWriteAck', lease: msg.lease, jobId: msg.jobId, offset: msg.offset, error: err.message || String(err), retried: err.retried || undefined });
      }
      return;
    }

    case 'reset': {
      if (busy !== null) { refuseBusy(msg, 'reset'); return; }
      if (!admit(msg)) { refuseLease(msg, 'reset'); return; }
      busy = 'reset';
      try {
        await resetLane();
        self.postMessage({ type: 'resetDone', lease, mem: wasm.memory.buffer.byteLength, peak: heapPeak(), abandoned: [...abandonedFiles.keys()] });
      } catch (err) {
        self.postMessage({ type: 'error', lease, message: 'reset failed: ' + (err.message || String(err)) });
      } finally {
        busy = null;
      }
      return;
    }

    case 'exec': {
      if (busy !== null) { refuseBusy(msg, 'exec'); return; }
      if (!admit(msg)) { refuseLease(msg, 'exec'); return; }
      busy = 'exec';
      try {
        const responseBytes = await exec(msg);
        // Transfer only the result bytes, never the whole WASM heap
        // (wasm-bindgen may return a view into linear memory).
        const owned = responseBytes.buffer.byteLength === responseBytes.byteLength
            ? responseBytes.buffer
            : responseBytes.buffer.slice(responseBytes.byteOffset, responseBytes.byteOffset + responseBytes.byteLength);
        // mem is the worker's lifetime peak: linear memory never shrinks.
        // The host keeps or retires the worker by peak.
        self.postMessage({ type: 'result', lease, jobId: msg.jobId, data: owned, mem: wasm.memory.buffer.byteLength, peak: heapPeak() }, [owned]);
      } catch (err) {
        // A wasm trap (a Rust panic compiles to `unreachable`) leaves the
        // engine's state unusable. Report it as a trap, apart from every
        // other failure, so the host ends the lane instead of reusing it.
        const trap = err instanceof WebAssembly.RuntimeError;
        self.postMessage({ type: 'error', lease, jobId: msg.jobId, message: err.message || String(err), trap: trap ? true : undefined, mem: wasm ? wasm.memory.buffer.byteLength : null, peak: wasm ? heapPeak() : null });
      } finally {
        busy = null;
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
