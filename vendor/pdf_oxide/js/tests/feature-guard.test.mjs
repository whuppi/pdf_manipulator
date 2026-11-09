// TDD tests for feature-gated phantom APIs in the Node.js binding.
//
// render, barcode, and signature operations are feature-gated in the
// Rust FFI (rendering / barcodes / signatures features). When the native
// lib is compiled without a feature, calls return error code 8 which the
// binding surfaces as a PdfError with code '5000'.
//
// Each test accepts two outcomes:
//   - feature ON  → the operation succeeds and the assertion holds
//   - feature OFF → a PdfError is thrown; the test passes vacuously
//
// This mirrors the OcrEngineTests.cs / SignatureTests.cs pattern from C#.
// The bare-features CI job will exercise the "feature OFF" path; the
// full-features CI job exercises the "feature ON" path.

import assert from 'node:assert/strict';
import { dirname } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

const __dir = dirname(fileURLToPath(import.meta.url));

let Pdf, PdfDocument, PdfError;
try {
  ({ Pdf, PdfDocument } = await import('../lib/index.js'));
  // PdfError may be exported under different names — try both
  try {
    ({ PdfError } = await import('../lib/index.js'));
  } catch {
    PdfError = null;
  }
} catch {
  console.warn('[feature-guard] skipping — compiled library not available');
}

// Returns true if the error indicates a feature-not-compiled condition.
// The Node.js binding surfaces FFI error code 8 as "unknown error code 8"
// in the message; higher-level wrappers may surface it as "5000" or
// "Unsupported feature".
function isUnsupported(err) {
  if (!err) return false;
  const msg = String(err?.message ?? err);
  return (
    msg.includes('5000') ||
    msg.includes('error code 8') ||
    msg.includes('Unsupported feature') ||
    msg.includes('not compiled') ||
    msg.includes('UnsupportedFeature') ||
    err?.code === '5000'
  );
}

function makeDoc() {
  const bytes = Pdf.fromMarkdown('# test\n\nBody.').saveToBytes();
  return PdfDocument.openFromBuffer(Buffer.from(bytes));
}

function isPng(b) {
  return b?.length >= 4 && b[0] === 0x89 && b[1] === 0x50 && b[2] === 0x4e && b[3] === 0x47;
}

// ── Rendering ──────────────────────────────────────────────────────────────

test('renderPageWithOptions: succeeds or throws UnsupportedFeature', { skip: !PdfDocument }, () => {
  const doc = makeDoc();
  try {
    const bytes = doc.renderPageWithOptions(0, {});
    assert.ok(isPng(bytes), 'should produce PNG when rendering is available');
  } catch (err) {
    assert.ok(isUnsupported(err), `unexpected error: ${err?.message}`);
  }
});

test('renderPageWithOptions argument validation always works', { skip: !PdfDocument }, () => {
  const doc = makeDoc();
  // C++ validates dpi=0 before calling the feature-gated render path,
  // so this should always throw even without the rendering feature.
  assert.throws(() => doc.renderPageWithOptions(0, { dpi: 0 }), /dpi/i);
});

// ── Barcodes ───────────────────────────────────────────────────────────────

test('Pdf.fromBarcode: succeeds or throws UnsupportedFeature', { skip: !Pdf }, () => {
  // fromBarcode is exposed on Pdf (static factory) in some builds
  if (typeof Pdf.fromBarcode !== 'function') return; // not yet wrapped — skip
  try {
    const pdf = Pdf.fromBarcode('HELLO', 0);
    assert.ok(pdf, 'should return a Pdf when barcodes feature is available');
    pdf.close();
  } catch (err) {
    assert.ok(isUnsupported(err), `unexpected error: ${err?.message}`);
  }
});

// ── Signatures ─────────────────────────────────────────────────────────────

test('PdfDocument.signatureCount: succeeds or throws UnsupportedFeature', {
  skip: !PdfDocument,
}, () => {
  const doc = makeDoc();
  if (typeof doc.signatureCount !== 'function' && doc.signatureCount === undefined) return;
  try {
    const count =
      typeof doc.signatureCount === 'function' ? doc.signatureCount() : doc.signatureCount;
    assert.equal(typeof count, 'number');
    assert.ok(count >= 0);
  } catch (err) {
    assert.ok(isUnsupported(err), `unexpected error: ${err?.message}`);
  }
});

// ── render-options.test.mjs guard migration ────────────────────────────────
// The existing render-options.test.mjs assumes rendering is always available.
// Verify the key assertions also tolerate the feature-off path.

test('estimateRenderTime: returns number or throws UnsupportedFeature', {
  skip: !PdfDocument,
}, () => {
  const doc = makeDoc();
  try {
    const ms = doc.estimateRenderTime(0, 150);
    assert.equal(typeof ms, 'number');
    assert.ok(ms >= 0);
  } catch (err) {
    assert.ok(isUnsupported(err), `unexpected error: ${err?.message}`);
  }
});
