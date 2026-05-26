// WASM Worker — per-operation engine execution.
//
// Loads WASM once via init(). Runs one operation at a time.
// Creates readFn/writeFn based on I/O mode (set at init by coordinator).
//
// Two I/O modes for reads:
//   atomics: readFn blocks via Atomics.wait on SharedArrayBuffer
//   opfs:    readFn calls SyncAccessHandle.read (pre-copied to OPFS)
//
// Output streaming (all modes): writeFn posts chunks to coordinator.
// Per-item streaming (all modes): postMessage({type:'item'}) per image/page.
//
// IMPORTANT: This file is a THIN PASS-THROUGH. All dispatch cases
// (read, edit, stream) call doc.dispatchXxx() — one WASM call per op.
// No loops. No page iteration. No conditional behavior logic.
// dispatch.rs owns the behavior. If you need a loop, add it in Rust.

import init, { WasmPdf, WasmDocumentBuilder, WasmPdfDocument, WasmFluentPageBuilder, signPdfWithPkcs12, signPdfWithPem, signPdfStreamingPkcs12, signPdfStreamingPem, planSplitByBookmarks } from './pdf_oxide.js';

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
      let bytes;
      switch (args.format) {
        case 'docx': bytes = doc.toDocxBytes(); break;
        case 'pptx': bytes = doc.toPptxBytes(); break;
        case 'xlsx': bytes = doc.toXlsxBytes(); break;
        default: throw new Error(`Unknown format: ${args.format}`);
      }
      doc.free();
      writeFn(bytes);
      return {};
    }

    case 'convertToPdf': {
      const bytes = readAllBytes(readerCtx);
      let doc;
      switch (args.format) {
        case 'docx': doc = WasmPdfDocument.openFromDocxBytes(bytes); break;
        case 'pptx': doc = WasmPdfDocument.openFromPptxBytes(bytes); break;
        case 'xlsx': doc = WasmPdfDocument.openFromXlsxBytes(bytes); break;
        default: throw new Error(`Unknown format: ${args.format}`);
      }
      doc.saveToWriter(writeFn, true, true, false);
      doc.free();
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
      const pdf = WasmPdf.fromMultipleImageBytes(images);
      const bytes = pdf.toBytes();
      writeFn(bytes);
      pdf.free();
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
        const bytes = doc.saveEncryptedToBytes(
          args.encryptUserPw,
          args.encryptOwnerPw || null,
          null, null, null, null
        );
        writeFn(bytes);
      } else {
        doc.saveToWriter(writeFn, args.compress ?? true, args.garbageCollect ?? true, args.saveMode ?? 0);
      }
      return {};
    }

    case 'editorGetMetadata': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      return {
        pageCount: doc.pageCount(),
        version: (() => { const v = doc.version(); return v ? `${v[0]}.${v[1]}` : '2.0'; })(),
        title: doc.getTitle() || '',
        author: doc.getAuthor() || '',
        subject: doc.getSubject() || '',
        keywords: doc.getKeywords() || '',
      };
    }

    case 'editorIsModified': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      return { modified: doc.isModified() };
    }

    case 'editorPageMediaBox': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const mb = doc.pageMediaBox(args.page);
      return { x: mb[0], y: mb[1], width: mb[2] - mb[0], height: mb[3] - mb[1] };
    }

    case 'editorMergeFrom': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      doc.mergeFrom(new Uint8Array(args.otherBytes));
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
      if (args.title != null) b.title(args.title);
      if (args.author != null) b.author(args.author);
      if (args.subject != null) b.subject(args.subject);
      if (args.keywords != null) b.keywords(args.keywords);
      return {};
    }

    case 'builderAddPage': {
      const b = getHandle(builderHandles, args.handleId, 'Builder');
      let page;
      if (args.pageType === 'a4') page = b.a4Page();
      else if (args.pageType === 'letter') page = b.letterPage();
      else page = b.page(args.width, args.height);
      const hid = nextHandleId++;
      pageHandles.set(hid, { page, builderId: args.handleId });
      return { handleId: hid };
    }

    case 'builderPageOp': {
      const entry = getHandle(pageHandles, args.handleId, 'Page');
      applyPageOp(entry.page, args.pageOp, args);
      return {};
    }

    case 'builderPageDone': {
      const entry = pageHandles.get(args.handleId);
      if (entry) {
        const builder = getHandle(builderHandles, entry.builderId, 'Builder');
        entry.page.done(builder);
        pageHandles.delete(args.handleId);
      }
      return {};
    }

    case 'builderSave': {
      const b = getHandle(builderHandles, args.handleId, 'Builder');
      const bytes = b.build();
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

// ── Page builder op dispatch ───────────────────────────────────────────

function applyPageOp(page, op, args) {
  switch (op) {
    case 'font': page.font(args.name, args.size); break;
    case 'at': page.at(args.x, args.y); break;
    case 'text': page.text(args.text); break;
    case 'heading': page.heading(args.level, args.text); break;
    case 'paragraph': page.paragraph(args.text); break;
    case 'space': page.space(args.points); break;
    case 'horizontalRule': page.horizontalRule(); break;
    case 'image': page.imageWithAlt(new Uint8Array(args.imageBytes), args.x, args.y, args.width, args.height, args.altText || ''); break;
    case 'watermark': page.watermark(args.text); break;
    case 'textField': page.textField(args.name, args.x, args.y, args.w, args.h, args.defaultValue || null); break;
    case 'checkbox': page.checkbox(args.name, args.x, args.y, args.w, args.h, args.checked || false); break;
    case 'comboBox': page.comboBox(args.name, args.x, args.y, args.w, args.h, args.options, args.selected || null); break;
    case 'pushButton': page.pushButton(args.name, args.x, args.y, args.w, args.h, args.caption); break;
    case 'signatureField': page.signatureField(args.name, args.x, args.y, args.w, args.h); break;
    case 'radioGroup': page.radioGroup(args.name, args.values, args.xs, args.ys, args.ws, args.hs, args.selected || null); break;
    case 'fieldKeystroke': page.fieldKeystroke(args.script); break;
    case 'fieldFormat': page.fieldFormat(args.script); break;
    case 'fieldValidate': page.fieldValidate(args.script); break;
    case 'fieldCalculate': page.fieldCalculate(args.script); break;
    case 'linkUrl': page.linkUrl(args.url); break;
    case 'linkPage': page.linkPage(args.targetPage); break;
    case 'footnote': page.footnote(args.refMark, args.noteText); break;
    case 'columns': page.columns(args.columnCount, args.gapPt, args.text); break;
    case 'newline': page.newline(); break;
    case 'newPageSameSize': page.newPageSameSize(); break;
    default: throw new Error(`Unknown page op: ${op}`);
  }
}
