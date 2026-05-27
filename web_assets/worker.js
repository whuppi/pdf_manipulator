// WASM Worker — per-operation engine execution.
//
// Loads WASM once via init(). Runs one operation at a time.
// Creates readFn/writeFn based on I/O mode (set at init by coordinator).
//
// RULE: EVERY call goes through dispatch.rs. No exceptions.
//
// - Read/edit/query ops: call doc.dispatchXxx() methods (wasm_api.rs → dispatch.rs).
// - Builder metadata: call b.dispatchSetTitle(), b.dispatchBuild() (wasm_api.rs → dispatch.rs).
// - Builder page ops: call page.dispatchPageOp(opCode, args) which buffers dispatch::PageOp,
//   then page.dispatchDone(builder) replays via dispatch::replay_page_ops.
//   Same PageOp enum, same replay function as native. Zero divergence possible.
// - Sign: calls signPdfStreamingPkcs12/Pem which internally use dispatch::sign_via_editor.
//
// NEVER call wasm.rs wrapper methods (doc.pageCount(), doc.mergeFrom(), b.title(),
// page.font(), etc.) directly. They bypass dispatch and cause web-only bugs.
//
// Two I/O modes for reads:
//   atomics: readFn blocks via Atomics.wait on SharedArrayBuffer
//   opfs:    readFn calls SyncAccessHandle.read (pre-copied to OPFS)
//
// Output streaming (all modes): writeFn posts chunks to coordinator.
// Per-item streaming (all modes): postMessage({type:'item'}) per image/page.
//
// IMPORTANT: This file is a THIN PASS-THROUGH. All cases call
// dispatchXxx() methods. The only loops are for streaming (render/
// extractImages iterate page indices, posting one item per page).
// dispatch.rs owns ALL behavior. No PDF logic here.

import init, { WasmDocumentBuilder, WasmPdfDocument, DispatchPageBuilder, signPdfStreamingPkcs12, signPdfStreamingPem } from './pdf_oxide.js';

let initialized = false;
let currentIoMode = 'opfs';
let nextReqId = 1;

const editorHandles = new Map();   // handleId → WasmPdfDocument
const editorReaders = new Map();   // handleId → readerCtx (kept alive until editorDispose)
const builderHandles = new Map();
const pageHandles = new Map();
let nextHandleId = 1;

async function ensureInit() {
  if (initialized) return;
  await init();
  initialized = true;
}

function getHandle(map, id, name) {
  const h = map.get(id);
  if (!h) throw new Error(`${name} handle ${id} not found`);
  return h;
}

// ── readFn creators per I/O mode ────────────────────────────────────────

function createReadFnAtomics(sab, sourceLength) {
  const statusView = new Int32Array(sab);
  const lengthView = new Int32Array(sab, 4, 1);
  const dataView = new Uint8Array(sab, 8);

  const readFn = (offset, count) => {
    Atomics.store(statusView, 0, 0);
    self.postMessage({ type: 'readAt', reqId: '0', offset, count, mode: 'atomics', sab });
    Atomics.wait(statusView, 0, 0);
    const status = Atomics.load(statusView, 0);
    if (status === 2) throw new Error('Read failed');
    const len = Atomics.load(lengthView, 0);
    const result = new Uint8Array(len);
    result.set(dataView.subarray(0, len));
    return result;
  };
  const lengthFn = () => sourceLength;
  return { readFn, lengthFn };
}

function createReadFnOpfs(opfsFile) {
  let syncHandle = null;
  let fileLength = 0;

  const openHandle = async () => {
    const root = await navigator.storage.getDirectory();
    const fileHandle = await root.getFileHandle(opfsFile);
    syncHandle = await fileHandle.createSyncAccessHandle();
    fileLength = syncHandle.getSize();
  };

  const readFn = (offset, count) => {
    const buf = new Uint8Array(count);
    const n = syncHandle.read(buf, { at: offset });
    return buf.slice(0, n);
  };

  const lengthFn = () => fileLength;
  const close = () => { if (syncHandle) { syncHandle.close(); syncHandle = null; } };

  return { readFn, lengthFn, openHandle, close };
}

// ── writeFn (all modes) ────────────────────────────────────────────────

function createWriteFn() {
  return (chunk) => {
    const buf = chunk instanceof Uint8Array ? chunk : new Uint8Array(chunk);
    self.postMessage({ type: 'chunk', data: buf.buffer }, [buf.buffer]);
  };
}

// ── Message handler ────────────────────────────────────────────────────

self.onmessage = async (e) => {
  const msg = e.data;

  if (msg.type === 'init') {
    currentIoMode = msg.ioMode;
    try {
      await ensureInit();
    } catch (err) {
      self.postMessage({ type: 'error', error: 'WASM init failed: ' + err.message });
      return;
    }
    self.postMessage({ type: 'ready' });
    return;
  }

  if (msg.type !== 'exec') return;

  const { opId, op, args, ioMode, sab, opfsFile } = msg;

  try {
    await ensureInit();
    const result = await executeOp(op, args, ioMode, sab, opfsFile);
    self.postMessage({ type: 'result', result: result || {} });
  } catch (err) {
    self.postMessage({ type: 'error', error: err.message || String(err) });
  }
};

// ── Operation execution ────────────────────────────────────────────────

async function executeOp(op, args, ioMode, sab, opfsFile) {
  let readerCtx = null;
  if (args.sourceLength != null) {
    if (opfsFile) {
      readerCtx = createReadFnOpfs(opfsFile);
      await readerCtx.openHandle();
    } else if (ioMode === 'atomics' && sab) {
      readerCtx = createReadFnAtomics(sab, args.sourceLength);
    }
  }

  const writeFn = createWriteFn();

  try {
    return await dispatch(op, args, readerCtx, writeFn);
  } finally {
    if (readerCtx && readerCtx.close && !readerCtx._claimed) {
      readerCtx.close();
    }
  }
}

// ── Helpers ────────────────────────────────────────────────────────────

function readAllBytes(readerCtx) {
  const len = readerCtx.lengthFn();
  return readerCtx.readFn(0, len);
}

function openDoc(readerCtx, password) {
  return WasmPdfDocument.fromReader(readerCtx.readFn, readerCtx.lengthFn, password || null);
}

function openEditor(readerCtx, password) {
  return WasmPdfDocument.editorFromReader(readerCtx.readFn, readerCtx.lengthFn, password || null);
}

// ── Op dispatch ────────────────────────────────────────────────────────

async function dispatch(op, args, readerCtx, writeFn) {
  switch (op) {
    case 'open': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchOpen();
      doc.free();
      return r;
    }

    case 'extract': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchExtractText(args.page ?? null, args.format ?? null);
      doc.free();
      return r;
    }

    case 'search': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchSearch(args.query, args.page ?? null);
      doc.free();
      return r;
    }

    case 'render': {
      const doc = openDoc(readerCtx, args.password);
      for (const pageIdx of args.pageIndices) {
        const rendered = doc.dispatchRenderPage(pageIdx, args.maxWidth ?? null, args.maxHeight ?? null);
        self.postMessage({ type: 'item', data: rendered });
      }
      self.postMessage({ type: 'itemDone' });
      doc.free();
      return {};
    }

    case 'extractImages': {
      const doc = openDoc(readerCtx, args.password);
      for (const pageIdx of args.pageIndices) {
        const images = doc.dispatchExtractImages(pageIdx);
        if (images) {
          for (let i = 0; i < images.length; i++) {
            self.postMessage({ type: 'item', data: images[i] });
          }
        }
      }
      self.postMessage({ type: 'itemDone' });
      doc.free();
      return {};
    }

    case 'getSignatures': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchGetSignatures();
      doc.free();
      return r;
    }

    case 'verifySignatures': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchVerifySignatures();
      doc.free();
      return r;
    }

    case 'validatePdfA': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchValidatePdfA(args.level ?? null);
      doc.free();
      return r;
    }

    case 'validatePdfUa': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchValidatePdfUa(args.level ?? null);
      doc.free();
      return r;
    }

    case 'planSplitByBookmarks': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchPlanSplitByBookmarks();
      doc.free();
      return r;
    }

    case 'classifyPage': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchClassifyPage(args.page);
      doc.free();
      return r;
    }

    case 'classifyDocument': {
      const doc = openDoc(readerCtx, args.password);
      const r = doc.dispatchClassifyDocument();
      doc.free();
      return r;
    }

    case 'convertTo': {
      const doc = openDoc(readerCtx, args.password);
      const bytes = doc.dispatchConvertToFormat(args.format);
      doc.free();
      writeFn(bytes);
      return {};
    }

    case 'convertToPdf': {
      const bytes = readAllBytes(readerCtx);
      WasmPdfDocument.dispatchConvertFromFormat(bytes, args.format, writeFn);
      return {};
    }

    case 'sign': {
      if (args.certPem && args.keyPem) {
        signPdfStreamingPem(readerCtx.readFn, readerCtx.lengthFn, writeFn,
            args.certPem, args.keyPem, args.reason || null, args.location || null);
      } else {
        signPdfStreamingPkcs12(readerCtx.readFn, readerCtx.lengthFn, writeFn,
            new Uint8Array(args.certificate), args.certificatePassword,
            args.reason || null, args.location || null);
      }
      return {};
    }

    case 'imagesToPdf': {
      const images = args.images.map(b => new Uint8Array(b));
      const bytes = WasmPdfDocument.dispatchImagesToPdf(images);
      writeFn(bytes);
      return {};
    }

    case 'editorOpen': {
      const doc = openEditor(readerCtx, args.password);
      const hid = nextHandleId++;
      editorHandles.set(hid, doc);
      if (readerCtx) {
        readerCtx._claimed = true;
        editorReaders.set(hid, readerCtx);
      }
      return { handleId: hid };
    }

    case 'editorDispose': {
      const doc = editorHandles.get(args.handleId);
      if (doc) { doc.free(); editorHandles.delete(args.handleId); }
      const reader = editorReaders.get(args.handleId);
      if (reader && reader.close) { reader.close(); editorReaders.delete(args.handleId); }
      return {};
    }

    case 'editorMutate': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const editResult = applyEditOp(doc, args.editOp, args);
      return editResult || {};
    }

    case 'editorSave': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const encryptMode = args.encryptMode ?? 0;
      if (encryptMode === 2) {
        const bytes = doc.dispatchEditSaveEncrypted(
          args.encryptUserPw,
          args.encryptOwnerPw || null
        );
        writeFn(bytes);
      } else {
        doc.dispatchEditSave(writeFn, args.compress ?? true, args.garbageCollect ?? true, args.saveMode ?? 0);
      }
      return {};
    }

    case 'editorGetMetadata': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      // Use dispatch methods that read from the EDITOR (not the original PdfDocument).
      // dispatchOpen reads from PdfDocument — wrong after mutations.
      const meta = doc.dispatchEditorGetMetadata();
      return meta;
    }

    case 'editorIsModified': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      return { modified: doc.dispatchEditIsModified() };
    }

    case 'editorPageMediaBox': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const mb = doc.dispatchEditPageMediaBox(args.page);
      return { x: mb[0], y: mb[1], width: mb[2] - mb[0], height: mb[3] - mb[1] };
    }

    case 'editorMergeFrom': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const raw = args.otherBytes;
      const arr = new Uint8Array(raw instanceof ArrayBuffer ? raw : (raw?.buffer || raw));
      doc.dispatchEditMerge([arr]);
      return {};
    }

    case 'builderCreate': {
      const builder = new WasmDocumentBuilder();
      const hid = nextHandleId++;
      builderHandles.set(hid, builder);
      return { handleId: hid };
    }

    case 'builderDispose': {
      const b = builderHandles.get(args.handleId);
      if (b) builderHandles.delete(args.handleId);
      return {};
    }

    case 'builderSetMetadata': {
      const b = getHandle(builderHandles, args.handleId, 'Builder');
      if (args.title != null) b.dispatchSetTitle(args.title);
      if (args.author != null) b.dispatchSetAuthor(args.author);
      if (args.subject != null) b.dispatchSetSubject(args.subject);
      if (args.keywords != null) b.dispatchSetKeywords(args.keywords);
      return {};
    }

    case 'builderAddPage': {
      let w, h;
      if (args.pageType === 'a4') { w = 595.28; h = 841.89; }
      else if (args.pageType === 'letter') { w = 612; h = 792; }
      else { w = args.width; h = args.height; }
      const page = new DispatchPageBuilder(w, h);
      const hid = nextHandleId++;
      pageHandles.set(hid, { page, builderId: args.handleId });
      return { handleId: hid };
    }

    case 'builderPageOp': {
      const entry = getHandle(pageHandles, args.handleId, 'Page');
      const opCode = PAGE_OP_CODES[args.pageOp];
      if (opCode === undefined) throw new Error(`Unknown page op: ${args.pageOp}`);
      entry.page.dispatchPageOp(opCode, args);
      return {};
    }

    case 'builderPageDone': {
      const entry = pageHandles.get(args.handleId);
      if (entry) {
        const builder = getHandle(builderHandles, entry.builderId, 'Builder');
        entry.page.dispatchDone(builder);
        pageHandles.delete(args.handleId);
      }
      return {};
    }

    case 'builderSave': {
      const b = getHandle(builderHandles, args.handleId, 'Builder');
      const bytes = b.dispatchBuild();
      writeFn(bytes);
      return {};
    }

    default:
      throw new Error(`Unknown op: ${op}`);
  }
}

// ── Edit op dispatch ───────────────────────────────────────────────────

function applyEditOp(doc, op, args) {
  switch (op) {
    case 'selectPages': doc.dispatchEditSelectPages(new Uint32Array(args.pages)); break;
    case 'deletePages': doc.dispatchEditDeletePages(new Uint32Array(args.pages)); break;
    case 'reorderPages': doc.dispatchEditSelectPages(new Uint32Array(args.order)); break;
    case 'movePage': doc.dispatchEditMovePage(args.from, args.to); break;
    case 'rotatePages': {
      const pages = []; const degrees = [];
      for (const [p, d] of Object.entries(args.rotations)) { pages.push(Number(p)); degrees.push(d); }
      doc.dispatchEditRotatePages(new Uint32Array(pages), new Int32Array(degrees));
      break;
    }
    case 'rotateAllPages': doc.dispatchEditRotateAll(args.degrees); break;
    case 'flattenForms': doc.dispatchEditFlattenForms(); break;
    case 'applyRedactions': doc.dispatchEditApplyRedactions(); break;
    case 'compress': doc.dispatchEditCompress(args.imageQuality || 75); break;
    case 'optimizeImages': return { count: doc.dispatchEditOptimizeImages(args.quality || 75) };
    case 'embedFile': doc.dispatchEditEmbedFile(args.name, new Uint8Array(args.fileData)); break;
    case 'eraseRegions': {
      const flat = [];
      for (const r of args.regions) { flat.push(r.x, r.y, r.width, r.height); }
      doc.dispatchEditEraseRegions(args.page, new Float32Array(flat));
      break;
    }
    case 'watermark': {
      const posType = args.posType || 0;
      const posFields = new Float32Array(
        posType === 1 ? [args.corner || 0, args.marginX || 20, args.marginY || 20] :
        posType === 2 ? [args.columns || 3, args.rows || 4] :
        posType === 3 ? [args.posX || 0, args.posY || 0, args.posW || 100, args.posH || 50] :
        []
      );
      doc.dispatchEditWatermark(args.page, args.text, args.fontSize || 48, args.rotation || 45,
        args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5,
        args.layer || 0, posType, posFields);
      break;
    }
    case 'encrypt': break;
    case 'decrypt': break;
    case 'addStamp': doc.dispatchEditAddStamp(args.page, args.stampType, args.x, args.y, args.width, args.height, args.opacity || 1.0); break;
    case 'addImageStamp': doc.dispatchEditAddImageStamp(args.page, new Uint8Array(args.imageBytes), args.x, args.y, args.width, args.height, args.opacity || 1.0); break;
    case 'setTitle': doc.dispatchEditSetTitle(args.value); break;
    case 'setAuthor': doc.dispatchEditSetAuthor(args.value); break;
    case 'setSubject': doc.dispatchEditSetSubject(args.value); break;
    case 'setKeywords': doc.dispatchEditSetKeywords(args.value); break;
    case 'cropMargins': doc.dispatchEditCropMargins(args.left || 0, args.right || 0, args.top || 0, args.bottom || 0); break;
    case 'convertToPdfA': doc.dispatchEditConvertToPdfA(args.level || 1); break;
    case 'flattenAllAnnotations': doc.dispatchEditFlattenAllAnnotations(); break;
    case 'setFormFieldValue': doc.dispatchEditSetFormFieldValue(args.fieldName, args.value); break;
    case 'unembedStandardFonts': doc.dispatchEditUnembedStandardFonts(); break;
    case 'resizeImage': doc.dispatchEditResizeImage(args.page, args.imageName, args.width, args.height); break;
    case 'addRedaction': doc.dispatchEditAddRedaction(args.page, args.x, args.y, args.w, args.h); break;
    case 'redactionCount': return { count: doc.dispatchEditRedactionCount(args.page) };
    case 'applyRedactions': doc.dispatchEditApplyRedactionsDestructive(); break;
    case 'scrubMetadata': doc.dispatchEditScrubMetadata(); break;
    default: throw new Error(`Unknown edit op: ${op}`);
  }
}

// ── Page op codes — maps Dart op name → dispatch::PageOp variant number ──
// Must match the op_code numbers in wasm_api.rs DispatchPageBuilder.dispatchPageOp

const PAGE_OP_CODES = {
  'font': 1, 'at': 2, 'text': 3, 'heading': 4, 'paragraph': 5,
  'space': 6, 'horizontalRule': 7, 'image': 8, 'watermark': 9,
  'textField': 10, 'checkbox': 11, 'comboBox': 12, 'pushButton': 13,
  'signatureField': 14, 'newline': 15, 'newPageSameSize': 16, 'done': 17,
  'radioGroup': 18, 'fieldKeystroke': 19, 'fieldFormat': 20,
  'fieldValidate': 21, 'fieldCalculate': 22, 'linkUrl': 23, 'linkPage': 24,
  'footnote': 25, 'columns': 26,
};
