// Tests for new DocumentEditor methods added in v0.3.39:
//   openFromBytes, saveToBytes, saveToBytesWithOptions,
//   getKeywords / setKeywords, mergeFromBytes, embedFile,
//   applyPageRedactions, applyAllRedactions,
//   rotateAllPages, rotatePageBy,
//   getPageMediaBox / setPageMediaBox,
//   getPageCropBox / setPageCropBox,
//   eraseRegions / clearEraseRegions,
//   isPageMarkedForFlatten / unmarkPageForFlatten,
//   isPageMarkedForRedaction / unmarkPageForRedaction.

import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

import { DocumentEditor, Pdf } from '../lib/index.js';

function makeTestPdfBytes(markdown = '# Test\n\nContent.') {
  return Buffer.from(Pdf.fromMarkdown(markdown).saveToBytes());
}

function writeTestPdf(path, markdown = '# Test\n\nContent.') {
  writeFileSync(path, makeTestPdfBytes(markdown));
}

function withEditor(markdown, fn) {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ednew-'));
  const path = join(dir, 'a.pdf');
  writeTestPdf(path, markdown);
  const editor = DocumentEditor.open(path);
  try {
    fn(editor, dir, path);
  } finally {
    editor.close();
    rmSync(dir, { recursive: true, force: true });
  }
}

// ── saveToBytes ────────────────────────────────────────────────────────────────

test('saveToBytes returns valid PDF bytes', () => {
  withEditor('# saveToBytes', (editor) => {
    const bytes = editor.saveToBytes();
    assert.ok(bytes instanceof Buffer || bytes instanceof Uint8Array);
    assert.ok(bytes.length > 100);
    assert.equal(String.fromCharCode(bytes[0], bytes[1], bytes[2], bytes[3], bytes[4]), '%PDF-');
  });
});

// ── saveToBytesWithOptions ─────────────────────────────────────────────────────

test('saveToBytesWithOptions returns valid PDF bytes', () => {
  withEditor('# saveToBytesWithOptions', (editor) => {
    const bytes = editor.saveToBytesWithOptions(true, true, false);
    assert.ok(bytes.length > 100);
    assert.equal(String.fromCharCode(bytes[0]), '%');
  });
});

// ── openFromBytes ──────────────────────────────────────────────────────────────

test('openFromBytes round-trip preserves page count', () => {
  withEditor('# openFromBytes', (editor) => {
    const pagesBefore = editor.pageCount();
    const bytes = editor.saveToBytes();
    const editor2 = DocumentEditor.openFromBytes(bytes);
    try {
      assert.equal(editor2.pageCount(), pagesBefore);
    } finally {
      editor2.close();
    }
  });
});

test('openFromBytes throws for empty buffer', () => {
  assert.throws(() => DocumentEditor.openFromBytes(Buffer.alloc(0)), /non-empty/);
});

test('openFromBytes throws for invalid data', () => {
  assert.throws(() => DocumentEditor.openFromBytes(Buffer.from([0x00, 0x01, 0x02, 0x03])));
});

// ── keywords ──────────────────────────────────────────────────────────────────

test('setKeywords / getKeywords round-trips', () => {
  withEditor('# keywords', (editor) => {
    editor.setKeywords('node, test, pdf');
    const kw = editor.getKeywords();
    assert.equal(kw, 'node, test, pdf');
  });
});

// ── mergeFromBytes ────────────────────────────────────────────────────────────

test('mergeFromBytes increases page count', () => {
  withEditor('# mergeFromBytes', (editor) => {
    const before = editor.pageCount();
    const extra = makeTestPdfBytes('# Extra page');
    const added = editor.mergeFromBytes(extra);
    assert.ok(added >= 1, `expected at least 1 page added, got ${added}`);
    assert.ok(editor.pageCount() > before);
  });
});

// ── embedFile ─────────────────────────────────────────────────────────────────

test('embedFile does not throw', () => {
  withEditor('# embedFile', (editor) => {
    editor.embedFile('hello.txt', Buffer.from('hello embedded world'));
  });
});

// ── applyPageRedactions / applyAllRedactions ──────────────────────────────────

test('applyPageRedactions (no-op) does not throw', () => {
  withEditor('# applyPageRedactions', (editor) => {
    editor.applyPageRedactions(0);
  });
});

test('applyAllRedactions (no-op) does not throw', () => {
  withEditor('# applyAllRedactions', (editor) => {
    editor.applyAllRedactions();
  });
});

// ── rotateAllPages ────────────────────────────────────────────────────────────

test('rotateAllPages does not throw', () => {
  withEditor('# rotateAllPages', (editor) => {
    editor.rotateAllPages(90);
  });
});

// ── rotatePageBy ──────────────────────────────────────────────────────────────

test('rotatePageBy does not throw', () => {
  withEditor('# rotatePageBy', (editor) => {
    editor.rotatePageBy(0, 180);
  });
});

// ── getPageMediaBox / setPageMediaBox ─────────────────────────────────────────

test('getPageMediaBox returns positive dimensions', () => {
  withEditor('# mediaBox', (editor) => {
    const box = editor.getPageMediaBox(0);
    assert.ok(typeof box.width === 'number');
    assert.ok(box.width > 0, `expected width > 0, got ${box.width}`);
    assert.ok(box.height > 0, `expected height > 0, got ${box.height}`);
  });
});

test('setPageMediaBox does not throw', () => {
  withEditor('# setMediaBox', (editor) => {
    const box = editor.getPageMediaBox(0);
    editor.setPageMediaBox(0, box.x, box.y, box.width, box.height);
  });
});

// ── getPageCropBox / setPageCropBox ───────────────────────────────────────────

test('getPageCropBox does not throw', () => {
  withEditor('# cropBox', (editor) => {
    const box = editor.getPageCropBox(0);
    assert.ok(typeof box.width === 'number');
  });
});

test('setPageCropBox does not throw', () => {
  withEditor('# setCropBox', (editor) => {
    editor.setPageCropBox(0, 10, 10, 500, 700);
  });
});

// ── eraseRegions / clearEraseRegions ─────────────────────────────────────────

test('eraseRegions does not throw', () => {
  withEditor('# eraseRegions', (editor) => {
    editor.eraseRegions(0, [
      [10, 10, 100, 50],
      [200, 200, 80, 40],
    ]);
  });
});

test('clearEraseRegions does not throw', () => {
  withEditor('# clearEraseRegions', (editor) => {
    editor.clearEraseRegions(0);
  });
});

// ── isPageMarkedForFlatten / unmarkPageForFlatten ─────────────────────────────

test('isPageMarkedForFlatten is false by default', () => {
  withEditor('# flattenMark', (editor) => {
    assert.equal(editor.isPageMarkedForFlatten(0), false);
  });
});

test('unmarkPageForFlatten does not throw', () => {
  withEditor('# unmarkFlatten', (editor) => {
    editor.unmarkPageForFlatten(0);
  });
});

// ── isPageMarkedForRedaction / unmarkPageForRedaction ────────────────────────

test('isPageMarkedForRedaction is false by default', () => {
  withEditor('# redactionMark', (editor) => {
    assert.equal(editor.isPageMarkedForRedaction(0), false);
  });
});

test('unmarkPageForRedaction does not throw', () => {
  withEditor('# unmarkRedaction', (editor) => {
    editor.unmarkPageForRedaction(0);
  });
});
