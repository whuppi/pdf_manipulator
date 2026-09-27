// Tests for the DocumentEditor TS wrapper. binding.cc already exports
// every editor operation; these cover the JS-side surface.

import assert from 'node:assert/strict';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';

let Pdf, DocumentEditor;
try {
  ({ Pdf, DocumentEditor } = await import('../lib/index.js'));
} catch {
  // library not built — all tests will be skipped
}

const skip = !Pdf;

function writeTestPdf(path, markdown = '# Edit me\n\nBody.') {
  const bytes = Pdf.fromMarkdown(markdown).saveToBytes();
  writeFileSync(path, Buffer.from(bytes));
}

test('DocumentEditor.open returns a usable editor with expected page count', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ed-'));
  const path = join(dir, 'a.pdf');
  writeTestPdf(path);
  try {
    const editor = DocumentEditor.open(path);
    assert.ok(editor.pageCount() >= 1);
    assert.equal(editor.isModified(), false);
    editor.close();
    assert.equal(editor.closed, true);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('DocumentEditor.mergeFrom marks the editor as modified', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ed-'));
  const a = join(dir, 'a.pdf');
  const b = join(dir, 'b.pdf');
  writeTestPdf(a, '# A\n\nOne.');
  writeTestPdf(b, '# B\n\nTwo.');
  try {
    const editor = DocumentEditor.open(a);
    editor.mergeFrom(b);
    assert.equal(editor.isModified(), true);
    editor.close();
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('DocumentEditor.setPageRotation + save round-trips', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ed-'));
  const path = join(dir, 'a.pdf');
  const out = join(dir, 'out.pdf');
  writeTestPdf(path);
  try {
    const editor = DocumentEditor.open(path);
    editor.setPageRotation(0, 90);
    editor.save(out);
    editor.close();
    assert.ok(existsSync(out));
    const bytes = readFileSync(out);
    assert.ok(bytes.length > 0);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('DocumentEditor.saveEncrypted produces an AES-encrypted PDF', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ed-'));
  const src = join(dir, 'a.pdf');
  const out = join(dir, 'out.pdf');
  writeTestPdf(src);
  try {
    const editor = DocumentEditor.open(src);
    editor.saveEncrypted(out, 'user-pw', 'owner-pw');
    editor.close();
    const bytes = readFileSync(out);
    const text = bytes.toString('binary');
    assert.ok(text.includes('/Encrypt'), 'expected /Encrypt dict');
    assert.ok(text.includes('/V 5'), 'expected /V 5 (AES-256) marker');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('DocumentEditor.open rejects empty path', { skip }, () => {
  assert.throws(() => DocumentEditor.open(''), /non-empty/);
});

test('DocumentEditor methods throw after close', { skip }, () => {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-ed-'));
  const path = join(dir, 'a.pdf');
  writeTestPdf(path);
  try {
    const editor = DocumentEditor.open(path);
    editor.close();
    assert.throws(() => editor.pageCount(), /closed/);
    assert.throws(() => editor.deletePage(0), /closed/);
    assert.throws(() => editor.save('x'), /closed/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
