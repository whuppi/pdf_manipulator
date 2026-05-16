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

self.onmessage = async (event) => {
  const { id, op, args } = event.data;

  try {
    await ensureInit();

    let result;

    // ── Editor handle ops (editor.XXX) ────────────────────────────
    if (op === 'editorOpen') {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
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
      result = handleOneShot(op, args);
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

function handleOneShot(op, args) {
  switch (op) {
    // ── Inspect ──
    case 'open': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const pc = doc.pageCount();
      const pages = [];
      for (let i = 0; i < pc; i++) {
        const mb = doc.pageMediaBox(i);
        pages.push({ index: i, width: mb[2] - mb[0], height: mb[3] - mb[1], rotation: doc.pageRotation(i) });
      }
      const r = { pageCount: pc, version: '2.0', pages, isTagged: doc.hasStructureTree() };
      doc.free();
      return r;
    }

    case 'probe': {
      try {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const pc = doc.pageCount();
        doc.free();
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
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const pc = doc.pageCount();
      const chunks = [];
      for (let start = 0; start < pc; start += args.every) {
        const end = Math.min(start + args.every, pc);
        const pages = new Uint32Array(end - start);
        for (let i = 0; i < pages.length; i++) pages[i] = start + i;
        chunks.push(doc.extractPages(pages).buffer);
      }
      doc.free();
      return { chunks };
    }

    case 'extractPages': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const bytes = doc.extractPages(new Uint32Array(args.pages));
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'deletePages': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      for (const p of [...args.pages].sort((a, b) => b - a)) doc.deletePage(p);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'reorderPages': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const bytes = doc.extractPages(new Uint32Array(args.order));
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'movePage': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.movePage(args.from, args.to);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'rotatePages': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      for (const [page, degrees] of Object.entries(args.pages)) doc.rotatePage(parseInt(page), degrees);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'rotateAllPages': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.rotateAllPages(args.degrees);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Compression ──
    case 'compress': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const bytes = doc.saveWithOptions(true, true, false);
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Content ──
    case 'flattenForms': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.flattenForms();
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'applyRedactions': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.applyAllRedactions();
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Watermark ──
    case 'watermark': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const pc = doc.pageCount();
      for (const i of (args.pages || Array.from({length: pc}, (_, i) => i))) {
        doc.addWatermark(i, args.text, args.fontSize || 48, args.rotation || 45,
          args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5);
      }
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'watermarkPositioned': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const pc = doc.pageCount();
      for (const i of (args.pages || Array.from({length: pc}, (_, i) => i))) {
        doc.addWatermarkPositioned(i, args.text, args.x, args.y, args.width, args.height,
          args.fontSize || 48, args.fontName || null, args.rotation || 45,
          args.opacity || 0.3, args.r || 0.5, args.g || 0.5, args.b || 0.5);
      }
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Stamps ──
    case 'addStamp': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.addStamp(args.page, args.stampType, args.customName || null,
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'addImageStamp': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.addImageStamp(args.page, new Uint8Array(args.imageBytes),
        args.x, args.y, args.width, args.height, args.opacity || 1.0);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Security ──
    case 'encrypt': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      const bytes = doc.saveEncryptedToBytes(args.userPassword || '', args.ownerPassword || '');
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'decrypt': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password);
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Extraction ──
    case 'extractText': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const text = args.page != null
        ? (typeof doc.extractPageText(args.page) === 'string' ? doc.extractPageText(args.page) : '')
        : doc.extractAllText();
      doc.free();
      return { text };
    }

    case 'toMarkdown': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const text = args.page != null ? doc.toMarkdown(args.page) : doc.toMarkdownAll();
      doc.free();
      return { text };
    }

    case 'toHtml': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const text = doc.toHtml(args.page);
      doc.free();
      return { text };
    }

    case 'toPlainText': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const text = doc.toPlainText(args.page);
      doc.free();
      return { text };
    }

    // ── Search ──
    case 'searchPage': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const results = doc.searchPage(args.page, args.query);
      doc.free();
      return { results: results || [] };
    }

    case 'searchAll': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const results = doc.search(args.query);
      doc.free();
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
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.embedFile(args.name, new Uint8Array(args.fileData));
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    case 'eraseRegions': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
      doc.eraseRegions(args.page, new Float32Array(args.rects));
      const bytes = doc.saveToBytes();
      doc.free();
      return { bytes: bytes.buffer };
    }

    // ── Signatures ──
    case 'getSignatureCount': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const count = doc.signatureCount();
      doc.free();
      return { count };
    }

    case 'getSignatures': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const sigs = doc.signatures();
      doc.free();
      return { signatures: sigs || [] };
    }

    // ── Validation ──
    case 'validatePdfA': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const r = doc.validatePdfA(args.level || '2b');
      doc.free();
      return { compliant: r?.compliant || false, errors: r?.errors || 0, warnings: r?.warnings || 0 };
    }

    case 'validatePdfUa': {
      const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
      const r = doc.validatePdfUa();
      doc.free();
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
