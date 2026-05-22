// pdf_manipulator Web Worker — runs pdf_oxide WASM off the main thread.
//
// Three dispatch paths:
//   1. One-shot ops (top-level): load doc → operate → save → free
//   2. Editor handle ops (editor.*): persistent WasmPdfDocument per handle
//   3. Builder handle ops (builder.* / page.*): persistent WasmDocumentBuilder + WasmFluentPageBuilder
//
// Handle maps mirror the native isolate's pattern — same architecture, same lifecycle.

import init, { WasmPdf, WasmDocumentBuilder, WasmPdfDocument, WasmFluentPageBuilder } from './pdf_oxide.js';

let initialized = false;
let nextHandleId = 1;
const editorHandles = new Map();   // id → WasmPdfDocument
const builderHandles = new Map();  // id → WasmDocumentBuilder
const pageHandles = new Map();     // id → WasmFluentPageBuilder

async function ensureInit() {
  if (initialized) return;
  await init();
  initialized = true;
}

function getHandle(map, id, name) {
  const h = map.get(id);
  if (!h) throw new Error(`${name} handle ${id} not found (disposed or never created)`);
  return h;
}

// ═══════════════════════════════════════════════════════════════════════
// OPFS helpers — disk-backed I/O for streaming large PDFs
// ═══════════════════════════════════════════════════════════════════════

const opfsHandles = new Map(); // filename → SyncAccessHandle (open for writing)

async function opfsWrite(filename, chunk, offset) {
  const root = await navigator.storage.getDirectory();
  let handle = opfsHandles.get(filename);
  if (!handle) {
    const fileHandle = await root.getFileHandle(filename, { create: true });
    handle = await fileHandle.createSyncAccessHandle();
    opfsHandles.set(filename, handle);
  }
  const data = new Uint8Array(chunk);
  handle.write(data, { at: offset });
}

async function opfsFinalize(filename) {
  const handle = opfsHandles.get(filename);
  if (handle) {
    handle.flush();
    handle.close();
    opfsHandles.delete(filename);
  }
}

async function opfsRead(filename) {
  const root = await navigator.storage.getDirectory();
  const fileHandle = await root.getFileHandle(filename);
  const handle = await fileHandle.createSyncAccessHandle();
  const size = handle.getSize();
  const buf = new Uint8Array(size);
  handle.read(buf, { at: 0 });
  handle.close();
  return buf;
}

async function opfsCleanup(filename) {
  try {
    const root = await navigator.storage.getDirectory();
    await root.removeEntry(filename);
  } catch (_) { /* ignore cleanup errors */ }
}

// Resolve input bytes: from ArrayBuffer (direct) or from OPFS file
async function resolveInputBytes(args) {
  if (args.bytes) return new Uint8Array(args.bytes);
  if (args.opfsFile) {
    const data = await opfsRead(args.opfsFile);
    await opfsCleanup(args.opfsFile);
    return data;
  }
  throw new Error('No input: expected bytes or opfsFile');
}

// Open a WasmPdfDocument from OPFS via fromReader (reads on demand from disk).
async function openDocument(args) {
  const root = await navigator.storage.getDirectory();
  const fileHandle = await root.getFileHandle(args.opfsFile);
  const syncHandle = await fileHandle.createSyncAccessHandle();
  const readFn = (offset, count) => {
    const buf = new Uint8Array(count);
    const n = syncHandle.read(buf, { at: offset });
    return buf.slice(0, n);
  };
  const lengthFn = () => syncHandle.getSize();
  const doc = WasmPdfDocument.fromReader(readFn, lengthFn, args.password || null);
  return { doc, syncHandle, opfsFile: args.opfsFile };
}

// Clean up after openDocument
async function closeDocument(ctx) {
  if (ctx.syncHandle) ctx.syncHandle.close();
  if (ctx.opfsFile) await opfsCleanup(ctx.opfsFile);
}

self.onmessage = async (event) => {
  const { id, op, args } = event.data;

  try {
    // OPFS operations don't need WASM init
    if (op === 'opfs.write') {
      await opfsWrite(args.filename, args.chunk, args.offset);
      self.postMessage({ type: 'result', id, result: {} });
      return;
    }
    if (op === 'opfs.finalize') {
      await opfsFinalize(args.filename);
      self.postMessage({ type: 'result', id, result: {} });
      return;
    }
    if (op === 'opfs.cleanup') {
      await opfsCleanup(args.filename);
      self.postMessage({ type: 'result', id, result: {} });
      return;
    }

    await ensureInit();

    let result;

    // ── Editor handle ops (editor.XXX) ────────────────────────────
    if (op === 'editorOpen') {
      // Editor needs full bytes for DocumentEditor initialization (mutation ops).
      // fromReader can't be used here — raw_bytes would be empty, breaking ensure_editor.
      const inputBytes = await resolveInputBytes(args);
      const doc = new WasmPdfDocument(inputBytes, args.password || null);
      const hid = nextHandleId++;
      editorHandles.set(hid, doc);
      result = { handleId: hid };
    } else if (op.startsWith('editor.')) {
      result = handleEditorOp(op.substring(7), args);
    }
    // ── Builder handle ops (builderXXX / builder.XXX) ─────────────
    else if (op === 'builderCreate') {
      const builder = new WasmDocumentBuilder();
      const hid = nextHandleId++;
      builderHandles.set(hid, builder);
      result = { handleId: hid };
    } else if (op.startsWith('builder.')) {
      result = handleBuilderOp(op.substring(8), args);
    }
    // ── Page handle ops (page.XXX) ────────────────────────────────
    else if (op.startsWith('page.')) {
      result = handlePageOp(op.substring(5), args);
    }
    // ── One-shot ops ──────────────────────────────────────────────
    else {
      result = await handleOneShot(op, args);
    }

    // Transfer ArrayBuffers for zero-copy
    const transfers = [];
    if (result && result.bytes instanceof ArrayBuffer) transfers.push(result.bytes);
    if (result && result.chunks) {
      for (const c of result.chunks) {
        if (c instanceof ArrayBuffer) transfers.push(c);
      }
    }
    self.postMessage({ type: 'result', id, result: result || {} }, transfers);
  } catch (e) {
    self.postMessage({ type: 'error', id, error: e.message || String(e) });
  }
};

// ═══════════════════════════════════════════════════════════════════════
// EDITOR HANDLE — persistent WasmPdfDocument
// ═══════════════════════════════════════════════════════════════════════

function handleEditorOp(method, args) {
  const doc = getHandle(editorHandles, args.handleId, 'Editor');

  switch (method) {
    case 'dispose':
      doc.free();
      editorHandles.delete(args.handleId);
      return {};

    // ── Properties ──
    case 'pageCount': return { value: doc.pageCount() };
    case 'version': {
      const v = doc.version();
      return { value: v ? `${v[0]}.${v[1]}` : '1.0' };
    }
    case 'isModified': return { value: true }; // WASM always modified after open+edit

    // ── Metadata ──
    case 'getTitle': return { value: doc.title() || '' };
    case 'setTitle': doc.setTitle(args.value); return {};
    case 'getAuthor': return { value: doc.author() || '' };
    case 'setAuthor': doc.setAuthor(args.value); return {};
    case 'getSubject': return { value: doc.subject() || '' };
    case 'setSubject': doc.setSubject(args.value); return {};
    case 'getKeywords': return { value: doc.keywords() || '' };
    case 'setKeywords': doc.setKeywords(args.value); return {};

    // ── Pages ──
    case 'rotatePage': doc.rotatePage(args.page, args.degrees); return {};
    case 'rotateAllPages': doc.rotateAllPages(args.degrees); return {};
    case 'getPageMediaBox': {
      const mb = doc.pageMediaBox(args.page);
      return { x: mb[0], y: mb[1], width: mb[2] - mb[0], height: mb[3] - mb[1] };
    }
    case 'deletePage': doc.deletePage(args.page); return {};
    case 'movePage': doc.movePage(args.from, args.to); return {};
    case 'extractPages': {
      const pages = new Uint32Array(args.pages);
      return { bytes: doc.extractPages(pages).buffer };
    }
    case 'mergeFrom': {
      const other = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.mergeDocument(other);
      other.free();
      return {};
    }

    // ── Optimization ──
    case 'optimizeImages': return { value: doc.optimizeImages(args.quality) };
    case 'unembedStandardFonts': return { value: doc.unembedStandardFonts() };

    // ── Watermark + stamps ──
    case 'addWatermark':
      doc.addWatermark(args.page, args.text,
        args.fontSize || 48, args.rotation || 45, args.opacity || 0.3,
        args.r || 0.5, args.g || 0.5, args.b || 0.5);
      return {};
    case 'addWatermarkPositioned':
      doc.addWatermarkPositioned(args.page, args.text,
        args.x, args.y, args.width, args.height,
        args.fontSize || 48, args.fontName || null,
        args.rotation || 45, args.opacity || 0.3,
        args.r || 0.5, args.g || 0.5, args.b || 0.5);
      return {};
    case 'addStamp':
      doc.addStamp(args.page, args.stampType, args.customName || null,
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      return {};
    case 'addImageStamp':
      doc.addImageStamp(args.page, new Uint8Array(args.imageBytes),
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      return {};

    // ── Content ──
    case 'embedFile': doc.embedFile(args.name, new Uint8Array(args.data)); return {};
    case 'eraseRegions': {
      const rects = new Float32Array(args.rects);
      doc.eraseRegions(args.page, rects);
      return {};
    }
    case 'flattenForms': doc.flattenForms(); return {};
    case 'flattenAllAnnotations': doc.flattenAllAnnotations(); return {};
    case 'applyAllRedactions': doc.applyAllRedactions(); return {};
    case 'setFormFieldValue': doc.setFormFieldValue(args.field, args.value); return {};
    case 'cropMargins':
      doc.cropMargins(args.left, args.right, args.top, args.bottom);
      return {};
    case 'convertToPdfA': doc.convertToPdfA(args.level); return {};
    case 'resizeImage':
      doc.resizeImage(args.page, args.imageName, args.width, args.height);
      return {};

    // ── Save ──
    case 'save': return { bytes: doc.saveToBytes().buffer };
    case 'saveWithOptions':
      return { bytes: doc.saveWithOptions(
        args.compress ?? true, args.garbageCollect ?? true, args.linearize ?? false
      ).buffer };
    case 'saveEncrypted':
      return { bytes: doc.saveEncryptedToBytes(
        args.userPassword || '', args.ownerPassword || ''
      ).buffer };
    case 'saveEncryptedFull':
      return { bytes: doc.saveEncryptedFullToBytes(
        args.userPassword || '', args.ownerPassword || '',
        args.algorithm ?? 3,
        args.allowPrint ?? true, args.allowPrintHq ?? true,
        args.allowModify ?? true, args.allowCopy ?? true,
        args.allowAnnotate ?? true, args.allowFillForms ?? true,
        args.allowAccessibility ?? true, args.allowAssemble ?? true
      ).buffer };

    default:
      throw new Error(`Unknown editor operation: ${method}`);
  }
}

// ═══════════════════════════════════════════════════════════════════════
// BUILDER HANDLE — persistent WasmDocumentBuilder
// ═══════════════════════════════════════════════════════════════════════

function handleBuilderOp(method, args) {
  const builder = getHandle(builderHandles, args.handleId, 'Builder');

  switch (method) {
    case 'dispose':
      builderHandles.delete(args.handleId);
      return {};

    case 'setTitle': builder.title(args.value); return {};
    case 'setAuthor': builder.author(args.value); return {};
    case 'setSubject': builder.subject(args.value); return {};
    case 'setKeywords': builder.keywords(args.value); return {};

    case 'addA4Page': {
      const page = builder.a4Page();
      const hid = nextHandleId++;
      pageHandles.set(hid, { page, builderId: args.handleId });
      return { handleId: hid };
    }
    case 'addLetterPage': {
      const page = builder.letterPage();
      const hid = nextHandleId++;
      pageHandles.set(hid, { page, builderId: args.handleId });
      return { handleId: hid };
    }
    case 'addPage': {
      const page = builder.page(args.width, args.height);
      const hid = nextHandleId++;
      pageHandles.set(hid, { page, builderId: args.handleId });
      return { handleId: hid };
    }

    case 'build': return { bytes: builder.build().buffer };
    case 'buildEncrypted':
      return { bytes: builder.toBytes_encrypted(
        args.ownerPassword || '', args.userPassword || ''
      ).buffer };

    default:
      throw new Error(`Unknown builder operation: ${method}`);
  }
}

// ═══════════════════════════════════════════════════════════════════════
// PAGE HANDLE — persistent WasmFluentPageBuilder
// ═══════════════════════════════════════════════════════════════════════

function handlePageOp(method, args) {
  const entry = getHandle(pageHandles, args.handleId, 'Page');
  const page = entry.page;

  switch (method) {
    case 'done': {
      const builder = getHandle(builderHandles, entry.builderId, 'Builder');
      page.done(builder);
      pageHandles.delete(args.handleId);
      return {};
    }

    case 'font': page.font(args.name, args.size); return {};
    case 'at': page.at(args.x, args.y); return {};
    case 'text': page.text(args.text); return {};
    case 'heading': page.heading(args.level, args.text); return {};
    case 'paragraph': page.paragraph(args.text); return {};
    case 'space': page.space(args.points); return {};
    case 'horizontalRule': page.horizontalRule(); return {};
    case 'image':
      page.imageWithAlt(new Uint8Array(args.bytes),
        args.x, args.y, args.width, args.height, args.altText || '');
      return {};
    case 'watermark': page.watermark(args.text); return {};

    // ── Form fields ──
    case 'textField':
      page.textField(args.name, args.x, args.y, args.w, args.h, args.defaultValue || null);
      return {};
    case 'checkbox':
      page.checkbox(args.name, args.x, args.y, args.w, args.h, args.checked || false);
      return {};
    case 'comboBox':
      page.comboBox(args.name, args.x, args.y, args.w, args.h, args.options, args.selected || null);
      return {};
    case 'pushButton':
      page.pushButton(args.name, args.x, args.y, args.w, args.h, args.caption);
      return {};
    case 'signatureField':
      page.signatureField(args.name, args.x, args.y, args.w, args.h);
      return {};
    case 'radioGroup': {
      const rects = args.rects; // flat array [x0,y0,w0,h0, x1,y1,w1,h1, ...]
      const xs = [], ys = [], ws = [], hs = [];
      for (let i = 0; i < rects.length; i += 4) {
        xs.push(rects[i]); ys.push(rects[i+1]);
        ws.push(rects[i+2]); hs.push(rects[i+3]);
      }
      page.radioGroup(args.name, args.values,
        new Float64Array(xs), new Float64Array(ys),
        new Float64Array(ws), new Float64Array(hs),
        args.selected || null);
      return {};
    }

    // ── Field scripts ──
    case 'fieldKeystroke': page.fieldKeystroke(args.script); return {};
    case 'fieldFormat': page.fieldFormat(args.script); return {};
    case 'fieldValidate': page.fieldValidate(args.script); return {};
    case 'fieldCalculate': page.fieldCalculate(args.script); return {};

    // ── Links ──
    case 'linkUrl': page.linkUrl(args.url); return {};
    case 'linkPage': page.linkPage(args.targetPage); return {};

    // ── Layout ──
    case 'footnote': page.footnote(args.refMark, args.noteText); return {};
    case 'columns': page.columns(args.columnCount, args.gapPt, args.text); return {};
    case 'newline': page.newline(); return {};
    case 'newPageSameSize': page.newPageSameSize(); return {};

    default:
      throw new Error(`Unknown page operation: ${method}`);
  }
}

// ═══════════════════════════════════════════════════════════════════════
// ONE-SHOT OPS — load → operate → save → free
// ═══════════════════════════════════════════════════════════════════════

async function handleOneShot(op, args) {
  switch (op) {
    // ── Inspect ──
    case 'open': {
      const ctx = await openDocument(args);
      const pc = ctx.doc.pageCount();
      const pages = [];
      for (let i = 0; i < pc; i++) {
        const mb = ctx.doc.pageMediaBox(i);
        pages.push({ index: i, width: mb[2] - mb[0], height: mb[3] - mb[1], rotation: ctx.doc.pageRotation(i) });
      }
      const r = { pageCount: pc, version: '2.0', pages, isTagged: ctx.doc.hasStructureTree() };
      ctx.doc.free();
      await closeDocument(ctx);
      return r;
    }

    case 'probe': {
      try {
        const ctx = await openDocument(args);
        const pc = ctx.doc.pageCount();
        ctx.doc.free();
        await closeDocument(ctx);
        return { isValid: true, pageCount: pc, isEncrypted: false };
      } catch (e) {
        return { isValid: false, pageCount: null, isEncrypted: false };
      }
    }

    // ── Structural ──
    case 'merge': {
      const arrays = args.inputs.map(b => new Uint8Array(b));
      const merged = WasmPdf.merge(arrays);
      const bytes = merged.toBytes();
      merged.free();
      return { bytes: bytes.buffer };
    }

    case 'split': {
      const ctx = await openDocument(args);
      const pc = ctx.doc.pageCount();
      const chunks = [];
      for (let start = 0; start < pc; start += args.every) {
        const end = Math.min(start + args.every, pc);
        const pages = new Uint32Array(end - start);
        for (let i = 0; i < pages.length; i++) pages[i] = start + i;
        chunks.push(ctx.doc.extractPages(pages).buffer);
      }
      ctx.doc.free();
      await closeDocument(ctx);
      return { chunks };
    }

    case 'extractPages': {
      const ctx = await openDocument(args);
      const bytes = ctx.doc.extractPages(new Uint32Array(args.pages));
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'deletePages': {
      const ctx = await openDocument(args);
      for (const p of [...args.pages].sort((a, b) => b - a)) ctx.doc.deletePage(p);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'reorderPages': {
      const ctx = await openDocument(args);
      const bytes = ctx.doc.extractPages(new Uint32Array(args.order));
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'movePage': {
      const ctx = await openDocument(args);
      ctx.doc.movePage(args.from, args.to);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'rotatePages': {
      const ctx = await openDocument(args);
      for (const [page, degrees] of Object.entries(args.pages)) ctx.doc.rotatePage(parseInt(page), degrees);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'rotateAllPages': {
      const ctx = await openDocument(args);
      ctx.doc.rotateAllPages(args.degrees);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Compression ──
    case 'compress': {
      const ctx = await openDocument(args);
      const bytes = ctx.doc.saveWithOptions(true, true, false);
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Content ──
    case 'flattenForms': {
      const ctx = await openDocument(args);
      ctx.doc.flattenForms();
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'applyRedactions': {
      const ctx = await openDocument(args);
      ctx.doc.applyAllRedactions();
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Watermark ──
    case 'watermark': {
      const ctx = await openDocument(args);
      const pc = ctx.doc.pageCount();
      for (const i of (args.pages || Array.from({length: pc}, (_, i) => i))) {
        ctx.doc.addWatermark(i, args.text, args.fontSize || 48, args.rotation || 45,
          args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5);
      }
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'watermarkPositioned': {
      const ctx = await openDocument(args);
      const pc = ctx.doc.pageCount();
      for (const i of (args.pages || Array.from({length: pc}, (_, i) => i))) {
        ctx.doc.addWatermarkPositioned(i, args.text, args.x, args.y, args.width, args.height,
          args.fontSize || 48, args.fontName || null, args.rotation || 45,
          args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5);
      }
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Stamps ──
    case 'addStamp': {
      const ctx = await openDocument(args);
      ctx.doc.addStamp(args.page, args.stampType, args.customName || null,
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'addImageStamp': {
      const ctx = await openDocument(args);
      ctx.doc.addImageStamp(args.page, new Uint8Array(args.imageBytes),
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Security ──
    case 'encrypt': {
      const ctx = await openDocument(args);
      const bytes = ctx.doc.saveEncryptedToBytes(args.userPassword || '', args.ownerPassword || '');
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'decrypt': {
      const ctx = await openDocument(args);
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Extraction ──
    case 'extractText': {
      const ctx = await openDocument(args);
      const text = args.page != null
        ? (typeof ctx.doc.extractPageText(args.page) === 'string' ? ctx.doc.extractPageText(args.page) : '')
        : ctx.doc.extractAllText();
      ctx.doc.free();
      await closeDocument(ctx);
      return { text };
    }

    case 'toMarkdown': {
      const ctx = await openDocument(args);
      const text = args.page != null ? ctx.doc.toMarkdown(args.page) : ctx.doc.toMarkdownAll();
      ctx.doc.free();
      await closeDocument(ctx);
      return { text };
    }

    case 'toHtml': {
      const ctx = await openDocument(args);
      const text = ctx.doc.toHtml(args.page);
      ctx.doc.free();
      await closeDocument(ctx);
      return { text };
    }

    case 'toPlainText': {
      const ctx = await openDocument(args);
      const text = ctx.doc.toPlainText(args.page);
      ctx.doc.free();
      await closeDocument(ctx);
      return { text };
    }

    // ── Search ──
    case 'searchPage': {
      const ctx = await openDocument(args);
      const results = ctx.doc.searchPage(args.page, args.query);
      ctx.doc.free();
      await closeDocument(ctx);
      return { results: results || [] };
    }

    case 'searchAll': {
      const ctx = await openDocument(args);
      const results = ctx.doc.search(args.query);
      ctx.doc.free();
      await closeDocument(ctx);
      return { results: results || [] };
    }

    // ── Images ──
    case 'imagesToPdf': {
      const doc = WasmPdf.fromMultipleImageBytes(args.images.map(b => new Uint8Array(b)));
      const bytes = doc.toBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'embedFile': {
      const ctx = await openDocument(args);
      ctx.doc.embedFile(args.name, new Uint8Array(args.fileData));
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    case 'eraseRegions': {
      const ctx = await openDocument(args);
      ctx.doc.eraseRegions(args.page, new Float32Array(args.rects));
      const bytes = ctx.doc.saveToBytes();
      ctx.doc.free();
      await closeDocument(ctx);
      return { bytes: bytes.buffer };
    }

    // ── Signatures ──
    case 'getSignatureCount': {
      const ctx = await openDocument(args);
      const count = ctx.doc.signatureCount();
      ctx.doc.free();
      await closeDocument(ctx);
      return { count };
    }

    case 'getSignatures': {
      const ctx = await openDocument(args);
      const sigs = ctx.doc.signatures();
      ctx.doc.free();
      await closeDocument(ctx);
      return { signatures: sigs || [] };
    }

    // ── Validation ──
    case 'validatePdfA': {
      const ctx = await openDocument(args);
      const r = ctx.doc.validatePdfA(args.level || '2b');
      ctx.doc.free();
      await closeDocument(ctx);
      return { compliant: r?.compliant || false, errors: r?.errors || 0, warnings: r?.warnings || 0 };
    }

    case 'validatePdfUa': {
      const ctx = await openDocument(args);
      const r = ctx.doc.validatePdfUa();
      ctx.doc.free();
      await closeDocument(ctx);
      return { accessible: r?.accessible || false };
    }

    default:
      throw new Error(`Unknown operation: ${op}`);
  }
}

ensureInit().then(() => {
  self.postMessage({ type: 'ready' });
}).catch((e) => {
  self.postMessage({ type: 'error', id: -1, error: `Init failed: ${e.message}` });
});
