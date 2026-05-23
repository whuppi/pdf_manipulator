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

import init, { WasmPdf, WasmDocumentBuilder, WasmPdfDocument, WasmFluentPageBuilder, signPdfWithPkcs12, signPdfWithPem, planSplitByBookmarks } from './pdf_oxide.js';

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
    // Close the reader UNLESS it was claimed by an editor handle.
    // Editor handles keep their reader alive until editorDispose.
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
      const pc = doc.pageCount();
      const pages = [];
      for (let i = 0; i < pc; i++) {
        const mb = doc.pageMediaBox(i);
        pages.push({
          index: i,
          width: mb[2] - mb[0],
          height: mb[3] - mb[1],
          rotation: doc.pageRotation(i),
        });
      }
      const r = {
        pageCount: pc,
        version: '2.0',
        pages,
        isTagged: doc.hasStructureTree(),
        title: null,
        author: null,
        isEncrypted: false,
      };
      doc.free();
      return r;
    }

    case 'merge': {
      const doc = openEditor(readerCtx, args.password);
      if (args.secondaries) {
        for (const secBytes of args.secondaries) {
          doc.mergeFrom(new Uint8Array(secBytes));
        }
      }
      doc.saveToWriter(writeFn, true, true, false);
      doc.free();
      return {};
    }

    case 'extractPages':
    case 'deletePages':
    case 'reorderPages':
    case 'movePage':
    case 'rotatePages':
    case 'rotateAllPages':
    case 'flattenForms':
    case 'applyRedactions':
    case 'compress':
    case 'embedFile':
    case 'eraseRegions':
    case 'watermark':
    case 'encrypt':
    case 'decrypt': {
      const doc = openEditor(readerCtx, args.password);
      applyEditOp(doc, op, args);
      doc.saveToWriter(writeFn, args.compress ?? true, args.garbageCollect ?? true, args.linearize ?? false);
      doc.free();
      return {};
    }

    case 'extract': {
      const doc = openDoc(readerCtx, args.password);
      let text;
      switch (args.format) {
        case 'markdown': text = doc.toMarkdown(args.page, true, false); break;
        case 'html': text = doc.toHtml(args.page); break;
        default: text = doc.extractText(args.page); break;
      }
      doc.free();
      return { text };
    }

    case 'search': {
      const doc = openDoc(readerCtx, args.password);
      const hits = doc.searchPage(args.page, args.query);
      doc.free();
      return { hits };
    }

    case 'render': {
      const doc = openDoc(readerCtx, args.password);
      for (const pageIdx of args.pageIndices) {
        let rendered;
        if (args.maxWidth && args.maxHeight) {
          rendered = doc.renderPageFit(pageIdx, args.maxWidth, args.maxHeight);
        } else {
          const pngBytes = doc.renderPage(pageIdx, 150);
          rendered = { width: 0, height: 0, data: pngBytes.buffer };
        }
        self.postMessage({ type: 'item', data: rendered });
      }
      self.postMessage({ type: 'itemDone' });
      doc.free();
      return {};
    }

    case 'extractImages': {
      const doc = openDoc(readerCtx, args.password);
      for (const pageIdx of args.pageIndices) {
        const images = doc.extractImageBytes(pageIdx, null);
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
      const sigs = doc.signatures();
      doc.free();
      return { signatures: sigs || [] };
    }

    case 'verifySignatures':
      return { valid: false };

    case 'validatePdfA': {
      const doc = openDoc(readerCtx, args.password);
      const levelMap = { 1: '1b', 2: '2b', 3: '3b' };
      const levelStr = typeof args.level === 'string' ? args.level : (levelMap[args.level] || '2b');
      const r = doc.validatePdfA(levelStr);
      doc.free();
      return r || { compliant: false, errors: 1, warnings: 0 };
    }

    case 'validatePdfUa': {
      const doc = openDoc(readerCtx, args.password);
      const levelStr = args.level != null ? String(args.level) : null;
      const valid = doc.validatePdfUa(levelStr);
      doc.free();
      return { valid: valid || false };
    }

    case 'planSplitByBookmarks': {
      const pdfBytes = readAllBytes(readerCtx);
      const result = planSplitByBookmarks(pdfBytes);
      return { splits: result };
    }

    case 'splitByBookmarks': {
      // Handled at Dart level using planSplitByBookmarks + extractPages
      return {};
    }

    case 'classifyPage': {
      const doc = openDoc(readerCtx, args.password);
      const result = doc.classifyPage(args.page);
      doc.free();
      return { type: result || 'unknown', confidence: 1.0 };
    }

    case 'classifyDocument': {
      const doc = openDoc(readerCtx, args.password);
      const result = doc.classifyDocument();
      doc.free();
      return { type: result || 'unknown', confidence: 1.0, pageCount: 0 };
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
      const pdfBytes = readAllBytes(readerCtx);
      let signed;
      if (args.certPem && args.keyPem) {
        signed = signPdfWithPem(pdfBytes, args.certPem, args.keyPem, args.reason || null, args.location || null);
      } else {
        signed = signPdfWithPkcs12(pdfBytes, new Uint8Array(args.certificate), args.certificatePassword, args.reason || null, args.location || null);
      }
      writeFn(signed);
      return {};
    }

    case 'addStamp':
    case 'addImageStamp': {
      const doc = openEditor(readerCtx, args.password);
      applyEditOp(doc, op, args);
      doc.saveToWriter(writeFn, true, true, false);
      doc.free();
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
      applyEditOp(doc, args.editOp, args);
      return {};
    }

    case 'editorSave': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      doc.saveToWriter(writeFn, args.compress ?? true, args.garbageCollect ?? true, args.linearize ?? false);
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

    case 'editorPageMediaBox': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const mb = doc.pageMediaBox(args.page);
      return { x: mb[0], y: mb[1], width: mb[2] - mb[0], height: mb[3] - mb[1] };
    }

    case 'editorExtractPages': {
      const doc = getHandle(editorHandles, args.handleId, 'Editor');
      const bytes = doc.extractPages(new Uint32Array(args.pages));
      writeFn(bytes);
      return {};
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
    case 'extractPages': doc.extractPages(new Uint32Array(args.pages)); break;
    case 'deletePages': {
      const sorted = [...args.pages].sort((a, b) => b - a);
      for (const p of sorted) doc.deletePage(p);
      break;
    }
    case 'reorderPages': doc.extractPages(new Uint32Array(args.order)); break;
    case 'movePage': doc.movePage(args.from, args.to); break;
    case 'rotatePages': {
      for (const [page, degrees] of Object.entries(args.rotations)) {
        doc.rotatePage(Number(page), degrees);
      }
      break;
    }
    case 'rotateAllPages': doc.rotateAllPages(args.degrees); break;
    case 'flattenForms': doc.flattenForms(); break;
    case 'applyRedactions': doc.applyAllRedactions(); break;
    case 'compress': break;
    case 'embedFile': doc.embedFile(args.name, new Uint8Array(args.fileData)); break;
    case 'eraseRegions': {
      for (const r of args.regions) doc.eraseRegion(args.page, r.x, r.y, r.width, r.height);
      break;
    }
    case 'watermark': {
      const pc = doc.pageCount();
      const pages = args.pageIndices || Array.from({ length: pc }, (_, i) => i);
      for (const p of pages) {
        doc.addWatermark(p, args.text, args.fontSize || 48, args.rotation || 45,
          args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5);
      }
      break;
    }
    case 'encrypt': break;
    case 'decrypt': break;
    case 'addStamp': doc.addStamp(args.page, args.stampType, args.customName || null, args.x, args.y, args.width, args.height, args.opacity || 1.0); break;
    case 'addImageStamp': doc.addImageStamp(args.page, new Uint8Array(args.imageBytes), args.x, args.y, args.width, args.height, args.opacity || 1.0); break;
    case 'setTitle': doc.setTitle(args.value); break;
    case 'setAuthor': doc.setAuthor(args.value); break;
    case 'setSubject': doc.setSubject(args.value); break;
    case 'setKeywords': doc.setKeywords(args.value); break;
    case 'cropMargins': doc.cropMargins(args.left || 0, args.right || 0, args.top || 0, args.bottom || 0); break;
    case 'convertToPdfA': doc.convertToPdfA(args.level || 1); break;
    case 'flattenAllAnnotations': doc.flattenAllAnnotations(); break;
    case 'setFormFieldValue': doc.setFormFieldValue(args.fieldName, args.value); break;
    case 'unembedStandardFonts': doc.unembedStandardFonts(); break;
    case 'resizeImage': doc.resizeImage(args.page, args.imageName, args.width, args.height); break;
    case 'addRedaction': doc.addRedaction(args.page, args.x, args.y, args.w, args.h, args.overlayText || null); break;
    case 'redactionCount': return { count: doc.redactionCount(args.page) };
    case 'applyRedactions': doc.applyRedactionsDestructive(); break;
    case 'scrubMetadata': doc.sanitizeDocument(true, false, false); break;
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
