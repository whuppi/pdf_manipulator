// Integration tests for the v0.3.39 DocumentBuilder table surface
// (#393). Exercises the new PageBuilder primitives through the native
// addon directly, then re-opens the produced PDFs through the read
// side to verify header/cell text survives round-trip.
//
// Run with:  node --test tests/tables.test.mjs
// Requires:  npm run build:native

import assert from 'node:assert';
import { mkdtempSync, unlinkSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, it } from 'node:test';

const require = createRequire(import.meta.url);
const native = require('../build/Release/pdf_oxide.node');

// Import the compiled TS wrappers when available; fall back to driving
// the native bindings directly for tests that only need the N-API
// surface (so the file is runnable without a prior `npm run build:ts`).
let builders;
try {
  builders = require('../lib/builders/index.js');
} catch {
  builders = null;
}

function tmp(ext = '.pdf') {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-tables-'));
  return join(dir, `out${ext}`);
}

// Helper: open a streaming table, push N rows via native, return the page handle.
function makeNativeStreamingTable(n_cols, batch_size) {
  const b = native.documentBuilderCreate();
  const p = native.documentBuilderLetterPage(b);
  native.pageBuilderFont(p, 'Helvetica', 8);
  native.pageBuilderAt(p, 72, 720);
  const headers = Array.from({ length: n_cols }, (_, i) => `H${i}`);
  const widths = Array.from({ length: n_cols }, () => 80);
  const aligns = Array.from({ length: n_cols }, () => 0);
  native.pageBuilderStreamingTableBeginV2(p, headers, widths, aligns, true, 0, 20, 0, 9999, 1);
  if (batch_size != null) native.pageBuilderStreamingTableSetBatchSize(p, batch_size);
  return { builder: b, page: p, n_cols };
}

describe('DocumentBuilder tables — native primitives', () => {
  it('strokeRect / strokeLine round-trip without error', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderLetterPage(b);
    native.pageBuilderStrokeRect(p, 50, 50, 200, 100, 2.0, 0.5, 0.5, 0.5);
    native.pageBuilderStrokeLine(p, 50, 50, 250, 50, 1.0, 0.2, 0.2, 0.2);
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    assert.ok(Buffer.isBuffer(buf));
    assert.ok(buf.length > 256);
    assert.strictEqual(buf.slice(0, 5).toString(), '%PDF-');
  });

  it('textInRect wraps text inside the rect and is extractable', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderLetterPage(b);
    native.pageBuilderFont(p, 'Helvetica', 10);
    native.pageBuilderTextInRect(
      p,
      72,
      600,
      200,
      100,
      'Lorem ipsum dolor sit amet, consectetur adipiscing elit — this should wrap across several lines when rendered inside a 200pt-wide rectangle.',
      1 // Align.Center
    );
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      const text = native.extractText(doc, 0);
      // "Lorem" must survive the round-trip whether the wrapper splits
      // the line or not.
      assert.ok(text.includes('Lorem'), `Lorem missing: ${text.slice(0, 120)}`);
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('newPageSameSize advances to a new letter-sized page', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderLetterPage(b);
    native.pageBuilderFont(p, 'Helvetica', 12);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'page one');
    native.pageBuilderNewPageSameSize(p);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'page two');
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      assert.strictEqual(native.getPageCount(doc), 2);
      assert.ok(native.extractText(doc, 0).includes('page one'));
      assert.ok(native.extractText(doc, 1).includes('page two'));
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('buffered pageBuilderTable round-trips header + body', () => {
    const widths = [100, 60];
    const aligns = [0, 2]; // Left, Right
    const cells = ['SKU', 'Qty', 'A-1', '12', 'B-2', '3'];
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderLetterPage(b);
    native.pageBuilderFont(p, 'Helvetica', 10);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderTable(p, widths, aligns, 3, cells, true);
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      const text = native.extractText(doc, 0);
      for (const token of ['SKU', 'Qty', 'A-1', 'B-2', '12', '3']) {
        assert.ok(text.includes(token), `missing ${token} in: ${text.slice(0, 200)}`);
      }
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('streamingTable batchSize=3: 7 rows → batchCount=2, pendingRowCount=1', () => {
    const { builder: b, page: p, n_cols } = makeNativeStreamingTable(2, 3);
    for (let i = 0; i < 7; i++) {
      native.pageBuilderStreamingTablePushRow(p, [`r${i}-a`, `r${i}-b`]);
    }
    assert.strictEqual(native.pageBuilderStreamingTableBatchCount(p), 2);
    assert.strictEqual(native.pageBuilderStreamingTablePendingRowCount(p), 1);
    native.pageBuilderStreamingTableFinish(p);
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    assert.ok(buf.length > 256);
  });

  it('streamingTable flush() marks batch boundary explicitly', () => {
    const { builder: b, page: p } = makeNativeStreamingTable(1, 100);
    native.pageBuilderStreamingTablePushRow(p, ['first']);
    native.pageBuilderStreamingTablePushRow(p, ['second']);
    assert.strictEqual(native.pageBuilderStreamingTableBatchCount(p), 0);
    assert.strictEqual(native.pageBuilderStreamingTablePendingRowCount(p), 2);
    native.pageBuilderStreamingTableFlush(p);
    assert.strictEqual(native.pageBuilderStreamingTableBatchCount(p), 1);
    assert.strictEqual(native.pageBuilderStreamingTablePendingRowCount(p), 0);
    native.pageBuilderStreamingTableFinish(p);
    native.pageBuilderDone(p);
    native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
  });

  it('table with mismatched aligns length throws TypeError', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderLetterPage(b);
    assert.throws(
      () =>
        native.pageBuilderTable(
          p,
          [80, 80],
          [0], // wrong length
          1,
          ['a', 'b'],
          false
        ),
      /aligns length/
    );
    native.pageBuilderFree(p);
    native.documentBuilderFree(b);
  });
});

// Exercise the TypeScript wrapper classes when they're available. The
// TS build step emits them under lib/builders/, so this block is
// automatically skipped in pure N-API-only CI runs.
if (builders) {
  const { Align, DocumentBuilder } = builders;

  describe('DocumentBuilder tables — TypeScript wrappers', () => {
    it('PageBuilder.table — buffered surface', () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 10).at(72, 720);
      page.table({
        columns: [
          { header: 'SKU', width: 100 },
          { header: 'Qty', width: 60, align: Align.Right },
        ],
        rows: [
          ['A-1', '12'],
          ['B-2', '3'],
        ],
        hasHeader: true,
      });
      page.done();
      const buf = doc.build();
      assert.ok(Buffer.isBuffer(buf));
      assert.ok(buf.length > 256);
    });

    it('PageBuilder.measure returns a positive width', () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 10);
      const w = page.measure('Hello');
      assert.ok(typeof w === 'number' && w > 0, `measure returned ${w}`);
      page.done();
      doc.close();
    });

    it('PageBuilder.textInRect + strokeRect chain without error', () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 10);
      page
        .textInRect(72, 600, 200, 100, 'wraps', Align.Center)
        .strokeRect(50, 50, 200, 100, { width: 2, color: [0.5, 0.5, 0.5] })
        .strokeLine(50, 50, 250, 50, { width: 1, color: [0.2, 0.2, 0.2] })
        .newPageSameSize()
        .at(72, 720)
        .text('page two');
      page.done();
      const buf = doc.build();
      assert.ok(buf.length > 256);
    });

    it('streamingTable handles 1000 rows', async () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 8).at(72, 720);
      const t = page.streamingTable({
        columns: [
          { header: 'SKU', width: 72 },
          { header: 'Item', width: 200 },
          { header: 'Qty', width: 48, align: Align.Right },
        ],
        repeatHeader: true,
      });
      for (let i = 0; i < 1000; i++) {
        t.pushRow([`SKU-${i}`, `Item ${i}`, String(i)]);
      }
      assert.strictEqual(t.rowCount, 1000);
      await t.finish();
      page.done();
      const buf = doc.build();
      assert.ok(Buffer.isBuffer(buf));
      assert.ok(buf.length > 1024);
    });

    it('streamingTable drains an async iterable via pushAll', async () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 10).at(72, 720);
      async function* source() {
        for (let i = 0; i < 5; i++) yield [`row-${i}`, String(i)];
      }
      const t = page.streamingTable({
        columns: [
          { header: 'Key', width: 100 },
          { header: 'N', width: 60, align: Align.Right },
        ],
      });
      await t.pushAll(source());
      assert.strictEqual(t.rowCount, 5);
      await t.finish();
      page.done();
      const buf = doc.build();
      assert.ok(buf.length > 256);
    });

    it('pushRow after finish throws', async () => {
      const doc = DocumentBuilder.create();
      const page = doc.letterPage().font('Helvetica', 10).at(72, 720);
      const t = page.streamingTable({
        columns: [{ header: 'K', width: 72 }],
      });
      t.pushRow(['one']);
      await t.finish();
      assert.throws(() => t.pushRow(['two']), /already finished/);
      page.done();
      doc.close();
    });
  });
}
