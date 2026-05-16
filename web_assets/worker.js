// pdf_manipulator Web Worker — runs pdf_oxide WASM off the main thread.
//
// Loaded by _web.dart via: new Worker('pdf_manipulator/worker.js')
// Receives operation messages via postMessage, executes via WASM,
// sends results back. ArrayBuffers are transferred (zero-copy).

import init, { WasmPdf, WasmDocumentBuilder, WasmPdfDocument } from './pdf_oxide.js';

let initialized = false;

async function ensureInit() {
  if (initialized) return;
  await init();
  initialized = true;
}

self.onmessage = async (event) => {
  const { id, op, args } = event.data;

  try {
    await ensureInit();

    let result;
    switch (op) {
      // ── Inspect ──
      case 'open': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const pc = doc.pageCount();
        const pages = [];
        for (let i = 0; i < pc; i++) {
          // pageMediaBox returns Float32Array [llx, lly, urx, ury]
          const mb = doc.pageMediaBox(i);
          const width = mb[2] - mb[0];
          const height = mb[3] - mb[1];
          const rotation = doc.pageRotation(i);
          pages.push({ index: i, width, height, rotation });
        }
        result = {
          pageCount: pc,
          version: '2.0',
          pages,
          isTagged: doc.hasStructureTree(),
        };
        doc.free();
        break;
      }

      case 'probe': {
        try {
          const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
          const pc = doc.pageCount();
          doc.free();
          result = { isValid: true, pageCount: pc, isEncrypted: false };
        } catch (e) {
          result = { isValid: false, pageCount: null, isEncrypted: false };
        }
        break;
      }

      // ── Structural ──
      case 'merge': {
        const arrays = args.inputs.map(b => new Uint8Array(b));
        const merged = WasmPdf.merge(arrays);
        const bytes = merged.toBytes();
        merged.free();
        result = { bytes: bytes.buffer };
        break;
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
        result = { chunks };
        break;
      }

      case 'extractPages': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const pages = new Uint32Array(args.pages);
        const extracted = doc.extractPages(pages);
        doc.free();
        result = { bytes: extracted.buffer };
        break;
      }

      case 'deletePages': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const sorted = [...args.pages].sort((a, b) => b - a);
        for (const p of sorted) doc.deletePage(p);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'reorderPages': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const pages = new Uint32Array(args.order);
        const extracted = doc.extractPages(pages);
        doc.free();
        result = { bytes: extracted.buffer };
        break;
      }

      case 'movePage': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.movePage(args.from, args.to);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'rotatePages': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        for (const [page, degrees] of Object.entries(args.pages)) {
          doc.rotatePage(parseInt(page), degrees);
        }
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'rotateAllPages': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.rotateAllPages(args.degrees);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Compression ──
      case 'compress': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const bytes = doc.saveWithOptions(true, true, false);
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Content ──
      case 'flattenForms': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.flattenForms();
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'applyRedactions': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.applyAllRedactions();
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Watermark ──
      case 'watermark': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const pc = doc.pageCount();
        const pages = args.pages || Array.from({length: pc}, (_, i) => i);
        for (const i of pages) {
          doc.addWatermark(i, args.text,
            args.fontSize || 48, args.rotation || 45, args.opacity || 0.3,
            args.r || 0.5, args.g || 0.5, args.b || 0.5);
        }
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'watermarkPositioned': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const pc = doc.pageCount();
        const pages = args.pages || Array.from({length: pc}, (_, i) => i);
        for (const i of pages) {
          doc.addWatermarkPositioned(i, args.text,
            args.x, args.y, args.width, args.height,
            args.fontSize || 48, args.fontName || null,
            args.rotation || 45, args.opacity || 0.3,
            args.r || 0.5, args.g || 0.5, args.b || 0.5);
        }
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Stamps ──
      case 'addStamp': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.addStamp(args.page, args.stampType, args.customName || null,
          args.x, args.y, args.width, args.height, args.opacity || 1.0);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'addImageStamp': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.addImageStamp(args.page, new Uint8Array(args.imageBytes),
          args.x, args.y, args.width, args.height, args.opacity || 1.0);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Security ──
      case 'encrypt': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const bytes = doc.saveEncryptedToBytes(
          args.userPassword || '', args.ownerPassword || '');
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'decrypt': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Extraction ──
      case 'extractText': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        let text;
        if (args.page != null) {
          const pageResult = doc.extractPageText(args.page);
          text = typeof pageResult === 'string' ? pageResult : pageResult.text || '';
        } else {
          text = doc.extractAllText();
        }
        doc.free();
        result = { text };
        break;
      }

      case 'toMarkdown': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const md = args.page != null
          ? doc.toMarkdown(args.page)
          : doc.toMarkdownAll();
        doc.free();
        result = { text: md };
        break;
      }

      case 'toHtml': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const html = doc.toHtml(args.page);
        doc.free();
        result = { text: html };
        break;
      }

      case 'toPlainText': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const text = doc.toPlainText(args.page);
        doc.free();
        result = { text };
        break;
      }

      // ── Search ──
      case 'searchPage': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const results = doc.searchPage(args.page, args.query);
        doc.free();
        result = { results: results || [] };
        break;
      }

      case 'searchAll': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const results = doc.search(args.query);
        doc.free();
        result = { results: results || [] };
        break;
      }

      // ── Images ──
      case 'imagesToPdf': {
        const arrays = args.images.map(b => new Uint8Array(b));
        const doc = WasmPdf.fromMultipleImageBytes(arrays);
        const bytes = doc.toBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'embedFile': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        doc.embedFile(args.name, new Uint8Array(args.fileData));
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      case 'eraseRegions': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes));
        const rects = new Float32Array(args.rects);
        doc.eraseRegions(args.page, rects);
        const bytes = doc.saveToBytes();
        doc.free();
        result = { bytes: bytes.buffer };
        break;
      }

      // ── Signatures ──
      case 'getSignatureCount': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const count = doc.signatureCount();
        doc.free();
        result = { count };
        break;
      }

      case 'getSignatures': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const sigs = doc.signatures();
        doc.free();
        result = { signatures: sigs || [] };
        break;
      }

      // ── Validation ──
      case 'validatePdfA': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const r = doc.validatePdfA(args.level || '2b');
        doc.free();
        result = { compliant: r?.compliant || false, errors: r?.errors || 0, warnings: r?.warnings || 0 };
        break;
      }

      case 'validatePdfUa': {
        const doc = new WasmPdfDocument(new Uint8Array(args.bytes), args.password || null);
        const r = doc.validatePdfUa();
        doc.free();
        result = { accessible: r?.accessible || false };
        break;
      }

      default:
        throw new Error(`Unknown operation: ${op}`);
    }

    // Transfer ArrayBuffers for zero-copy
    const transfers = [];
    if (result.bytes instanceof ArrayBuffer) transfers.push(result.bytes);
    if (result.chunks) {
      for (const c of result.chunks) {
        if (c instanceof ArrayBuffer) transfers.push(c);
      }
    }
    self.postMessage({ type: 'result', id, result }, transfers);
  } catch (e) {
    self.postMessage({ type: 'error', id, error: e.message || String(e) });
  }
};

ensureInit().then(() => {
  self.postMessage({ type: 'ready' });
}).catch((e) => {
  self.postMessage({ type: 'error', id: -1, error: `Init failed: ${e.message}` });
});
