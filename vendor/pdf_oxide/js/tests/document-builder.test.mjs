// Integration tests for the Node/N-API write-side API.
//
// Imports the native addon directly (same way the compiled TS wrapper does)
// so the test doesn't depend on `npm run build:ts` having run. Covers:
//
//   * DocumentBuilder lifecycle + metadata
//   * EmbeddedFont from file and bytes
//   * PageBuilder content + annotation methods
//   * AES-256 encryption (saveEncrypted / toBytesEncrypted)
//   * CJK round-trip via extract_text (the #382 cross-language gate)
//   * Subset pipeline is wired (PDF much smaller than face)
//   * HTML+CSS pipeline (Phase 2)
//
// Run with:  node --test tests/document-builder.test.mjs
// Requires:  npm run build:native (produces build/Release/pdf_oxide.node)

import assert from 'node:assert';
import {
  existsSync,
  mkdtempSync,
  readFileSync as readFile,
  readFileSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const native = require('../build/Release/pdf_oxide.node');

// Locate the DejaVuSans fixture by walking up from this test file.
const __dirname = dirname(fileURLToPath(import.meta.url));
function findFixture() {
  let dir = __dirname;
  for (let i = 0; i < 6; i++) {
    const candidate = join(dir, 'tests', 'fixtures', 'fonts', 'DejaVuSans.ttf');
    if (existsSync(candidate)) return candidate;
    dir = dirname(dir);
  }
  throw new Error('DejaVuSans.ttf fixture not found');
}
const FIXTURE = findFixture();

function tmp(ext = '.pdf') {
  const dir = mkdtempSync(join(tmpdir(), 'pdfoxide-'));
  return join(dir, `out${ext}`);
}

describe('DocumentBuilder native bindings', () => {
  it('minimal ASCII produces a valid PDF', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'Hello, world.');
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    assert.ok(Buffer.isBuffer(buf), 'build returns a Buffer');
    assert.ok(buf.length > 256, `suspiciously small: ${buf.length}`);
    assert.strictEqual(buf.slice(0, 5).toString(), '%PDF-');
    native.documentBuilderFree(b);
  });

  it('CJK round-trip via extract_text (Cyrillic + Greek)', () => {
    const font = native.embeddedFontFromFile(FIXTURE);
    const b = native.documentBuilderCreate();
    native.documentBuilderRegisterEmbeddedFont(b, 'DejaVu', font);
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderFont(p, 'DejaVu', 12);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'Привет, мир!');
    native.pageBuilderAt(p, 72, 700);
    native.pageBuilderText(p, 'Καλημέρα κόσμε');
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);

    // Round-trip through the existing read-side native bindings.
    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      const text = native.extractText(doc, 0);
      assert.ok(text.includes('Привет, мир!'), `Cyrillic missing: ${text}`);
      assert.ok(text.includes('Καλημέρα κόσμε'), `Greek missing: ${text}`);
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('output is subsetted (PDF much smaller than face)', () => {
    const faceSize = statSync(FIXTURE).size;
    const font = native.embeddedFontFromFile(FIXTURE);
    const b = native.documentBuilderCreate();
    native.documentBuilderRegisterEmbeddedFont(b, 'DejaVu', font);
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderFont(p, 'DejaVu', 12);
    native.pageBuilderAt(p, 72, 700);
    native.pageBuilderText(p, 'Hello world');
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    assert.ok(
      buf.length * 10 < faceSize,
      `expected PDF (${buf.length}) to be >= 10x smaller than face (${faceSize})`
    );
  });

  it('saveEncrypted produces /Encrypt + /V 5', () => {
    const path = tmp();
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'secret');
    native.pageBuilderDone(p);
    native.documentBuilderSaveEncrypted(b, path, 'userpw', 'ownerpw');
    native.documentBuilderFree(b);
    const raw = readFile(path);
    assert.ok(raw.includes('/Encrypt'), 'missing /Encrypt');
    assert.ok(raw.includes('/V 5'), 'missing /V 5 (AES-256)');
    unlinkSync(path);
  });

  it('toBytesEncrypted returns AES-256 encrypted buffer', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'x');
    native.pageBuilderDone(p);
    const buf = native.documentBuilderToBytesEncrypted(b, 'u', 'o');
    native.documentBuilderFree(b);
    assert.ok(buf.includes('/Encrypt'));
    assert.ok(buf.includes('/V 5'));
  });

  it('double-open-page throws', () => {
    const b = native.documentBuilderCreate();
    native.documentBuilderA4Page(b);
    assert.throws(() => native.documentBuilderA4Page(b));
    native.documentBuilderFree(b);
  });

  it('build consumes the builder handle', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'x');
    native.pageBuilderDone(p);
    native.documentBuilderBuild(b);
    // Second build on the same handle must fail (the inner builder was consumed).
    assert.throws(() => native.documentBuilderBuild(b));
    native.documentBuilderFree(b);
  });

  it('multi-page build', () => {
    const b = native.documentBuilderCreate();
    for (const s of ['page one', 'page two', 'page three']) {
      const p = native.documentBuilderA4Page(b);
      native.pageBuilderAt(p, 72, 720);
      native.pageBuilderText(p, s);
      native.pageBuilderDone(p);
    }
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);

    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      assert.strictEqual(native.getPageCount(doc), 3);
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('annotations do not break text extraction', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderAt(p, 72, 720);
    native.pageBuilderText(p, 'click me');
    native.pageBuilderLinkUrl(p, 'https://example.com');
    native.pageBuilderAt(p, 72, 700);
    native.pageBuilderText(p, 'important');
    native.pageBuilderHighlight(p, 1.0, 1.0, 0.0);
    native.pageBuilderAt(p, 72, 680);
    native.pageBuilderText(p, 'revisit');
    native.pageBuilderStickyNote(p, 'please review');
    native.pageBuilderWatermarkDraft(p);
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);

    const path = tmp();
    writeFileSync(path, buf);
    const doc = native.openDocument(path);
    try {
      const text = native.extractText(doc, 0);
      for (const w of ['click me', 'important', 'revisit']) {
        assert.ok(text.includes(w), `missing ${w}: ${text}`);
      }
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });

  it('strokeRectDashed and strokeLineDashed produce PDF with dash operator', () => {
    const b = native.documentBuilderCreate();
    const p = native.documentBuilderA4Page(b);
    native.pageBuilderStrokeRectDashed(p, 50, 100, 200, 150, 1.5, 0, 0, 0.8, [3, 2], 0);
    native.pageBuilderStrokeLineDashed(p, 50, 80, 250, 80, 1.0, 0.8, 0, 0, [5, 3], 1);
    native.pageBuilderDone(p);
    const buf = native.documentBuilderBuild(b);
    native.documentBuilderFree(b);
    assert.ok(buf.length > 100, `PDF suspiciously small: ${buf.length}`);
    const text = buf.toString('latin1');
    assert.ok(
      text.includes(' d\n') || text.includes(' d '),
      "PDF content stream missing dash operator 'd'"
    );
  });

  it('Phase 2 — Pdf.fromHtmlCss round-trips', () => {
    const fontBytes = readFileSync(FIXTURE);
    const pdf = native.pdfFromHtmlCss(
      '<h1>Hello</h1><p>World</p>',
      'h1 { color: blue; font-size: 24pt }',
      fontBytes
    );
    // pdf is an External handle to a Rust Pdf; serialize via pdf_save_to_bytes.
    const saved = native.pdfSaveToBytes(pdf);
    assert.ok(Buffer.isBuffer(saved));
    assert.strictEqual(saved.slice(0, 5).toString(), '%PDF-');
    native.pdfFree(pdf);

    const path = tmp();
    writeFileSync(path, saved);
    const doc = native.openDocument(path);
    try {
      const text = native.extractText(doc, 0);
      assert.ok(text.includes('Hello'));
      assert.ok(text.includes('World'));
    } finally {
      native.closeDocument(doc);
      unlinkSync(path);
    }
  });
});
