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
//   init         { protocol, ioMode, baseUrl?, wasmModule?, wasmEntry } → ready|error
//                  (wasmEntry: the manifest entry of the module this mode runs; the
//                   worker compiles from it when the host sent no wasmModule)
//   exec         { lease, jobId, requestBytes, sourceLengths: number[],
//                  sinkCount, keepSources: number[], holdGroupId?,
//                  opfsFiles: (string|null)[], blobs?: (Blob|null)[],
//                  buffers?: (ArrayBuffer|null)[], sab?, writeSab?,
//                  cancelSab?, cancelUrl? }
//                  (blobs: a source read in place with FileReaderSync, or in OPFS
//                   mode, where its opfsFiles entry is set, copied by the worker
//                   into that file before the job; buffers:
//                   a source copied whole and transferred with the job; a
//                   source in neither is read by the mode, or from opfsFiles)
//                  (cancelSab: the job's cancel word, where the page has
//                   SharedArrayBuffer; cancelUrl: otherwise, a blob URL the
//                   main thread revokes to cancel, polled by sync XHR)
//                → result { lease, jobId, data, mem, peak, held }
//                | error { lease, jobId, message, trap?, protocol?, mem, peak, held }
//                  (trap: the wasm instance trapped; the lane must end;
//                   protocol: the exec was refused because the worker was
//                   busy with another exec or a reset, or not serving its
//                   lease, and never started)
//   reset        { lease } → resetDone { lease, mem, peak, held } | error { lease,
//                  message, protocol? }: a fresh engine state in this
//                  worker, same wasm instance. Refused while busy.
//                  (peak: the most bytes the engine's heap held since the
//                   last reset, which the host keeps an executor by; held: the
//                   bytes of the readers backed by a copy of their source; mem:
//                   the linear memory's size, a diagnostic that never falls;
//                   abandoned: the pre-copy files set aside because their
//                   handle creation never answered, not removed yet)
//   readAtResult { lease, bytes? , error?, cancelled? }    (JSPI and Asyncify reads)
//   chunkAck     { lease, error?, cancelled? }             (JSPI and Asyncify writes)
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
// ── Four I/O modes (selected by Dart, passed in init) ──────────────
//   jspi     — JSPI Promise suspension (host_read_at returns Promise)
//   atomics  — per-job SAB + Atomics.wait; Dart fills + notifies
//   asyncify — the instrumented wasm (pdf_oxide_asyncify_bg) unwinds
//              at host_read_at / host_write_chunk, the worker awaits the
//              answer and rewinds; the messages are JSPI's
//   opfs     — sources pre-copied to OPFS; reads via SyncAccessHandle
// A reader backed by a transferred copy, or by a Blob in the three streaming
// modes, reads in the worker and never reaches the mode's path. In OPFS mode a
// Blob is copied to the worker's own pre-copy file with FileReaderSync, and that
// file backs the reader as any pre-copy does.
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
let wasmEntry = null;        // the manifest entry of the module this mode runs
let wasmBaseUrl = (() => {
  try { return new URL(self.location.href).href.replace(/\/[^/]*$/, '/'); }
  catch (_) { return './'; }
})();

// ═══════════════════════════════════════════════════════════════════
// Reader registry — no decisions. The Dart side decides WHICH
// sources are kept (keepSources) and WHICH held set an op reuses
// (holdGroupId); this registry just stores and looks up.
// ═══════════════════════════════════════════════════════════════════

// A reader has at most one backing, and the mode's read path serves a reader
// with none: `blob` (read in place with FileReaderSync), `bytes` (a copy of the
// source, transferred with the job) or `opfsHandle` (the pre-copy).
const readers = new Map();   // readerId → {length, blob, bytes, opfsHandle, opfsFile, holdGroupId|null}
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
// Asyncify: where the unwound stack is saved, the host call the worker is
// waiting for while the wasm is unwound, and the answer the rewound import
// returns.
let asyncifyData = 0;
let asyncifyPending = null;
let asyncifyAnswer = 0;

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
  if (resumingAsyncify()) return asyncifyAnswer;
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

  if (reader.bytes) return readFromBytes(reader.bytes, offset, toRead, bufPtr);
  if (reader.blob) return readFromBlob(reader.blob, offset, toRead, bufPtr);
  if (reader.opfsHandle) return readFromOpfs(reader.opfsHandle, offset, toRead, bufPtr);

  switch (ioMode) {
    case 'atomics':  return readViaAtomics(sourceIndex, offset, toRead, bufPtr);
    case 'asyncify': return suspendAsyncify(readViaJspi(sourceIndex, offset, toRead, bufPtr));
    default:         return readViaJspi(sourceIndex, offset, toRead, bufPtr);
  }
}

// Asyncify. The import is called in one of two states. Running, it starts the
// unwind and returns at once with a dummy; the wasm unwinds to `execute`,
// which returns to callLaneAsyncify with the host call still pending. After
// the answer the driver rewinds: `execute` is called again, the wasm replays
// down to this same call, and the import, now rewinding, returns the answer.
function resumingAsyncify() {
  if (ioMode !== 'asyncify' || wasm.asyncify_get_state() !== 2) return false;
  wasm.asyncify_stop_rewind();
  return true;
}

function suspendAsyncify(pending) {
  asyncifyPending = pending;
  wasm.asyncify_start_unwind(asyncifyData);
  return 0;
}

function readFromBytes(bytes, offset, count, bufPtr) {
  new Uint8Array(wasm.memory.buffer).set(new Uint8Array(bytes, offset, count), bufPtr);
  return count;
}

// A Blob that cannot be read (a File changed on disk) throws; the engine gets
// the host's I/O error, never wrong bytes.
function readFromBlob(blob, offset, count, bufPtr) {
  let data;
  try {
    data = new Uint8Array(new FileReaderSync().readAsArrayBuffer(blob.slice(offset, offset + count)));
  } catch (_) {
    return P.hostIoError;
  }
  if (data.length !== count) return P.hostIoError;
  new Uint8Array(wasm.memory.buffer).set(data, bufPtr);
  return count;
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
  if (resumingAsyncify()) return asyncifyAnswer;
  if (hostCancelled() !== 0) return P.hostIoCancelled;
  const chunk = new Uint8Array(wasm.memory.buffer).slice(bufPtr, bufPtr + len);
  switch (ioMode) {
    case 'atomics': return writeViaAtomics(sinkIndex, chunk);
    case 'jspi':    return writeViaJspi(sinkIndex, chunk);
    case 'asyncify': return suspendAsyncify(writeViaJspi(sinkIndex, chunk));
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

  // Intercept instantiation to capture the raw exports and, for JSPI, wrap the
  // host imports in WebAssembly.Suspending. Restored after.
  const origInstantiate = WebAssembly.instantiate;

  WebAssembly.instantiate = async (source, imports) => {
    if (ioMode === 'jspi' && imports) imports = wrapJspiImports(imports);
    const result = await origInstantiate(source, imports);
    wasm = (result instanceof WebAssembly.Instance ? result : result.instance).exports;
    return result;
  };

  try {
    await jsModule.default(precompiledModule || await compileParts(wasmBaseUrl, wasmEntry));
  } finally {
    WebAssembly.instantiate = origInstantiate;
  }

  if (ioMode === 'jspi') {
    jspiLaneExecute = WebAssembly.promising(wasm.execute);
  }
  if (ioMode === 'asyncify') {
    if (typeof wasm.asyncify_start_unwind !== 'function') {
      throw new Error('the wasm of the manifest entry is not an Asyncify build');
    }
    // The unwound stack is saved outside the engine's heap: fresh pages, which
    // its allocator neither owns nor counts.
    const bytes = P.asyncifyStackBytes;
    asyncifyData = wasm.memory.grow(bytes / 65536) * 65536;
    const header = new Int32Array(wasm.memory.buffer, asyncifyData, 2);
    header[0] = asyncifyData + 8;
    header[1] = asyncifyData + bytes;
  }
  initialized = true;
}

// The one loader of a wasm module. [entry] is a module of
// pdf_oxide_manifest.json: the module is cut into parts, and nothing fetches a
// whole `.wasm`. Every part is fetched at once and the parts are joined, in
// order, into one stream the browser compiles while it arrives. Each part
// passes through a transform that counts its bytes and checks its SHA-256
// against the manifest before the stream goes on, so a short or corrupt part
// errors the stream and the compile rejects with a message naming the part.
// wasm_loader.dart's _compileParts is the same loader for the main thread; the web
// battery pins both against the same manifest.
async function compileParts(baseUrl, entry) {
  if (!entry || !Array.isArray(entry.parts)) {
    throw new Error('no manifest entry to compile from');
  }
  const abort = new AbortController();
  const fetches = entry.parts.map((part) => {
    const response = fetch(baseUrl + part.file, { signal: abort.signal });
    response.catch(() => {});
    return response;
  });
  let failure = null;
  const stream = new ReadableStream({
    async start(controller) {
      try {
        for (let i = 0; i < entry.parts.length; i++) {
          const part = entry.parts[i];
          const response = await fetches[i];
          if (!response.ok || !response.body) {
            throw new Error(part.file + ': HTTP ' + response.status);
          }
          const reader = response.body.pipeThrough(verifyPart(part)).getReader();
          for (;;) {
            const { done, value } = await reader.read();
            if (done) break;
            controller.enqueue(value);
          }
        }
        controller.close();
      } catch (error) {
        failure = error;
        abort.abort();
        controller.error(error);
      }
    },
  });
  try {
    return await WebAssembly.compileStreaming(
      new Response(stream, { headers: { 'content-type': 'application/wasm' } }));
  } catch (error) {
    throw failure || error;
  }
}

// Passes a part's bytes through, and at its end refuses it unless it is the
// size and the SHA-256 the manifest lists. The hash is computed as the bytes
// pass (Sha256 below), so no part is held whole and a page without
// crypto.subtle, which only a secure context has, is checked the same way.
function verifyPart(part) {
  const hash = new Sha256();
  let count = 0;
  return new TransformStream({
    transform(chunk, controller) {
      count += chunk.byteLength;
      if (count > part.bytes) {
        throw new Error(part.file + ': more than the ' + part.bytes + ' bytes the manifest lists');
      }
      hash.update(chunk);
      controller.enqueue(chunk);
    },
    flush() {
      if (count !== part.bytes) {
        throw new Error(part.file + ': ' + count + ' bytes, the manifest lists ' + part.bytes);
      }
      const hex = hash.hex();
      if (hex !== part.sha256) {
        throw new Error(part.file + ': sha256 ' + hex + ' is not the ' + part.sha256 + ' the manifest lists');
      }
    },
  });
}

// SHA-256 (FIPS 180-4) over bytes given in pieces of any size.
class Sha256 {
  constructor() {
    this.h = new Uint32Array([
      0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
      0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ]);
    this.block = new Uint8Array(64);
    this.filled = 0;
    this.length = 0;
    this.w = new Uint32Array(64);
  }

  update(bytes) {
    this.length += bytes.length;
    let at = 0;
    if (this.filled > 0) {
      const take = Math.min(64 - this.filled, bytes.length);
      this.block.set(bytes.subarray(0, take), this.filled);
      this.filled += take;
      at = take;
      if (this.filled < 64) return;
      this.compress(this.block, 0);
      this.filled = 0;
    }
    for (; at + 64 <= bytes.length; at += 64) this.compress(bytes, at);
    if (at < bytes.length) {
      this.block.set(bytes.subarray(at), 0);
      this.filled = bytes.length - at;
    }
  }

  hex() {
    const bits = this.length * 8;
    const tail = new Uint8Array(this.filled < 56 ? 64 : 128);
    tail.set(this.block.subarray(0, this.filled));
    tail[this.filled] = 0x80;
    const view = new DataView(tail.buffer);
    view.setUint32(tail.length - 8, Math.floor(bits / 0x100000000));
    view.setUint32(tail.length - 4, bits >>> 0);
    for (let at = 0; at < tail.length; at += 64) this.compress(tail, at);
    return Array.from(this.h, (v) => v.toString(16).padStart(8, '0')).join('');
  }

  compress(bytes, at) {
    const w = this.w;
    for (let i = 0; i < 16; i++) {
      const j = at + i * 4;
      w[i] = (bytes[j] << 24) | (bytes[j + 1] << 16) | (bytes[j + 2] << 8) | bytes[j + 3];
    }
    for (let i = 16; i < 64; i++) {
      const a = w[i - 15], b = w[i - 2];
      const s0 = ((a >>> 7) | (a << 25)) ^ ((a >>> 18) | (a << 14)) ^ (a >>> 3);
      const s1 = ((b >>> 17) | (b << 15)) ^ ((b >>> 19) | (b << 13)) ^ (b >>> 10);
      w[i] = (w[i - 16] + s0 + w[i - 7] + s1) | 0;
    }
    let [a, b, c, d, e, f, g, h] = this.h;
    for (let i = 0; i < 64; i++) {
      const S1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
      const ch = (e & f) ^ (~e & g);
      const t1 = (h + S1 + ch + SHA256_K[i] + w[i]) | 0;
      const S0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const t2 = (S0 + maj) | 0;
      h = g; g = f; f = e; e = (d + t1) | 0;
      d = c; c = b; b = a; a = (t1 + t2) | 0;
    }
    const s = this.h;
    s[0] += a; s[1] += b; s[2] += c; s[3] += d;
    s[4] += e; s[5] += f; s[6] += g; s[7] += h;
  }
}

const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);

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

// The JSPI and Asyncify paths drive the wasm-bindgen ABI manually (the generated glue
// is synchronous; promising() needs the raw export). Mirrors the
// glue's own calling convention for execute.
//
// Buffers go through executor_alloc/executor_dealloc — never the glue's numbered
// __wbindgen_export_N aliases: those are renumbered whenever the module's
// export set changes, which turns into memory corruption here.
// (__wbindgen_add_to_stack_pointer is a fixed name and safe.)
async function callLane(requestBytes, sourceLengthsPacked, sinkCount, run) {
  const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
  const ptr0 = wasm.executor_alloc(requestBytes.length) >>> 0;
  if (requestBytes.length && !ptr0) throw new Error('executor_alloc failed');
  new Uint8Array(wasm.memory.buffer).set(requestBytes, ptr0);
  const ptr1 = wasm.executor_alloc(sourceLengthsPacked.length) >>> 0;
  if (sourceLengthsPacked.length && !ptr1) throw new Error('executor_alloc failed');
  new Uint8Array(wasm.memory.buffer).set(sourceLengthsPacked, ptr1);
  try {
    await run(retptr, statePtr, ptr0, requestBytes.length, ptr1, sourceLengthsPacked.length, sinkCount);
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

// Runs `execute` to its end across the pauses of an Asyncify wasm. Every call
// repeats the first one's arguments: a rewound `execute` replays its frames
// from the saved stack and ignores them, but the shadow stack and the request
// buffers must be the ones the paused frames hold.
async function runAsyncify(...args) {
  new Int32Array(wasm.memory.buffer, asyncifyData, 1)[0] = asyncifyData + 8;
  for (;;) {
    wasm.execute(...args);
    if (wasm.asyncify_get_state() !== 1) return;
    wasm.asyncify_stop_unwind();
    // The pass writes the frames without a bound, so an overflow is found
    // here, after the fact: the engine is not trusted again.
    const [used, end] = new Int32Array(wasm.memory.buffer, asyncifyData, 2);
    if (used > end) {
      throw new WebAssembly.RuntimeError(
        'the Asyncify stack of ' + P.asyncifyStackBytes + ' bytes overflowed');
    }
    asyncifyAnswer = await asyncifyPending;
    asyncifyPending = null;
    wasm.asyncify_start_rewind(asyncifyData);
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

// The bytes of the readers backed by a copy of their source.
function heldBytes() {
  let sum = 0;
  for (const r of readers.values()) if (r.bytes) sum += r.length;
  return sum;
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

// The pre-copy of a Blob, made here: the slices come from FileReaderSync and go
// through the same write as a page-sent chunk, so the file is owned and dropped
// as any pre-copy is. A Blob that cannot be read is the host's I/O error.
async function copyBlobToOpfs(jobId, filename, blob) {
  const reader = new FileReaderSync();
  for (let offset = 0; offset < blob.size; offset += P.sabMaxChunk) {
    if (hostCancelled() !== 0) throw new Error('cancelled');
    let chunk;
    try {
      chunk = reader.readAsArrayBuffer(blob.slice(offset, offset + P.sabMaxChunk));
    } catch (err) {
      const failure = new Error('Failed to read file: ' + (err.message || String(err)));
      failure.hostIo = true;
      throw failure;
    }
    await opfsWrite(jobId, filename, chunk, offset, offset + P.sabMaxChunk >= blob.size);
  }
}

async function exec(msg) {
  const { jobId, requestBytes, sourceLengths, sinkCount, keepSources,
          holdGroupId, opfsFiles, blobs, buffers } = msg;

  await ensureInit();
  activeSab = msg.sab || null;
  activeWriteSab = msg.writeSab || null;
  activeCancelSab = msg.cancelSab || null;
  activeCancelUrl = msg.cancelUrl || null;
  cancelSeen = 0;
  lastPollAt = performance.now();

  if (ioMode === 'opfs' && blobs) {
    for (let i = 0; i < blobs.length; i++) {
      if (blobs[i] && opfsFiles && opfsFiles[i]) await copyBlobToOpfs(jobId, opfsFiles[i], blobs[i]);
    }
  }

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
    const blob = (ioMode !== 'opfs' && blobs && blobs[i]) || null;
    const bytes = (buffers && buffers[i]) || null;
    if (blob) length = blob.size;
    if (bytes) length = bytes.byteLength;
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
    readers.set(id, { length, blob, bytes, opfsHandle: handle, opfsFile: onDisk, holdGroupId: null });
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
      responseBytes = await callLane(new Uint8Array(requestBytes), packed, sinkCount, jspiLaneExecute);
    } else if (ioMode === 'asyncify') {
      responseBytes = await callLane(new Uint8Array(requestBytes), packed, sinkCount, runAsyncify);
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
    asyncifyPending = null;
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
      if (msg.wasmEntry) wasmEntry = msg.wasmEntry;
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
        self.postMessage({ type: 'resetDone', lease, mem: wasm.memory.buffer.byteLength, peak: heapPeak(), held: heldBytes(), abandoned: [...abandonedFiles.keys()] });
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
        self.postMessage({ type: 'result', lease, jobId: msg.jobId, data: owned, mem: wasm.memory.buffer.byteLength, peak: heapPeak(), held: heldBytes() }, [owned]);
      } catch (err) {
        // A wasm trap (a Rust panic compiles to `unreachable`) leaves the
        // engine's state unusable. Report it as a trap, apart from every
        // other failure, so the host ends the lane instead of reusing it.
        const trap = err instanceof WebAssembly.RuntimeError;
        self.postMessage({ type: 'error', lease, jobId: msg.jobId, message: err.message || String(err), trap: trap ? true : undefined, io: err.hostIo ? true : undefined, mem: wasm ? wasm.memory.buffer.byteLength : null, peak: wasm ? heapPeak() : null, held: heldBytes() });
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
