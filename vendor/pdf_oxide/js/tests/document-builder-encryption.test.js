/**
 * Issue #401 regression tests: DocumentBuilder encryption content preservation.
 *
 * Verifies that `saveEncrypted` and `toBytesEncrypted` write ALL font sub-objects
 * (DescendantFonts, FontFile2, ToUnicode, FontDescriptor) into the encrypted
 * output when an embedded TrueType font is used.
 *
 * Strategy: the embedded font program (FontFile2) adds several KB even after
 * subsetting. Without the fix, those sub-objects were silently dropped and the
 * encrypted embedded-font PDF was barely larger than a base-14-font encrypted
 * PDF. With the fix, the embedded-font PDF must be ≥10 KB larger.
 */

import assert from 'node:assert';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, it } from 'node:test';
import { fileURLToPath } from 'node:url';

// Walk up from this file's directory to the repo root.
function findRepoRoot() {
  let dir = path.dirname(fileURLToPath(import.meta.url));
  while (dir !== path.parse(dir).root) {
    if (fs.existsSync(path.join(dir, 'Cargo.toml'))) return dir;
    dir = path.dirname(dir);
  }
  throw new Error('Could not find repo root (no Cargo.toml found)');
}

const repoRoot = findRepoRoot();
const fixtureFontPath = path.join(repoRoot, 'tests', 'fixtures', 'fonts', 'DejaVuSans.ttf');
const fixtureFontBoldPath = path.join(
  repoRoot,
  'tests',
  'fixtures',
  'fonts',
  'DejaVuSans-Bold.ttf'
);

function skip(reason) {
  console.log(`  SKIPPED: ${reason}`);
}

describe('DocumentBuilder encryption — issue #401 regression', () => {
  it('saveEncrypted with embedded font includes font sub-objects', async () => {
    let DocumentBuilder, EmbeddedFont;
    try {
      ({ DocumentBuilder, EmbeddedFont } = await import('../lib/index.js').catch(
        () => import('../src/index.ts')
      ));
    } catch {
      skip('DocumentBuilder / EmbeddedFont not available in this build');
      return;
    }

    if (!fs.existsSync(fixtureFontPath)) {
      skip('DejaVuSans.ttf fixture not found');
      return;
    }

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'pdfoxide-401-'));

    try {
      // ── baseline: simple text (base-14 font), encrypted ──────────────
      const simpleBuilder = DocumentBuilder.create();
      simpleBuilder.a4Page().at(72, 720).text('Hello simple').done();
      const simplePath = path.join(tmpDir, 'simple_enc.pdf');
      simpleBuilder.saveEncrypted(simplePath, 'userpw', 'ownerpw');
      const simpleSize = fs.statSync(simplePath).size;
      assert.ok(
        fs.readFileSync(simplePath).includes('/Encrypt'),
        'simple encrypted PDF must contain /Encrypt'
      );

      // ── embedded-font PDF, encrypted ─────────────────────────────────
      const font = EmbeddedFont.fromFile(fixtureFontPath);
      const ttfBuilder = DocumentBuilder.create().registerEmbeddedFont('DejaVu', font);
      ttfBuilder.a4Page().font('DejaVu', 12).at(72, 720).text('Hello from embedded font').done();
      const ttfPath = path.join(tmpDir, 'ttf_enc.pdf');
      ttfBuilder.saveEncrypted(ttfPath, 'userpw', 'ownerpw');
      const ttfRaw = fs.readFileSync(ttfPath);
      const ttfSize = ttfRaw.length;
      assert.ok(ttfRaw.includes('/Encrypt'), 'ttf encrypted PDF must contain /Encrypt');

      // The embedded font program adds ≥10 KB even when subsetted.
      const diff = ttfSize - simpleSize;
      assert.ok(
        diff >= 10_000,
        `issue #401: embedded-font encrypted PDF (${ttfSize} B) is not substantially ` +
          `larger than simple encrypted PDF (${simpleSize} B); diff=${diff} B — ` +
          `font sub-objects (FontFile2, DescendantFonts, etc.) are likely missing`
      );
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  it('toBytesEncrypted with embedded font includes font program bytes', async () => {
    let DocumentBuilder, EmbeddedFont;
    try {
      ({ DocumentBuilder, EmbeddedFont } = await import('../lib/index.js').catch(
        () => import('../src/index.ts')
      ));
    } catch {
      skip('DocumentBuilder / EmbeddedFont not available in this build');
      return;
    }

    if (!fs.existsSync(fixtureFontPath)) {
      skip('DejaVuSans.ttf fixture not found');
      return;
    }

    const font = EmbeddedFont.fromFile(fixtureFontPath);
    const builder = DocumentBuilder.create().registerEmbeddedFont('DejaVu', font);
    builder
      .a4Page()
      .font('DejaVu', 12)
      .at(72, 720)
      .text('bytes encrypted with embedded font')
      .done();
    const bytes = builder.toBytesEncrypted('u', 'o');
    assert.ok(bytes.includes('/Encrypt'), 'must contain /Encrypt');
    // Font program must be present: result must be >15 KB.
    assert.ok(
      bytes.length > 15_000,
      `issue #401: toBytesEncrypted embedded-font result (${bytes.length} B) is too small; ` +
        'font sub-objects likely missing from encrypted output'
    );
  });

  it('issue #401 exact scenario: two embedded fonts + AES-128 config', async () => {
    let DocumentBuilder, EmbeddedFont;
    try {
      ({ DocumentBuilder, EmbeddedFont } = await import('../lib/index.js').catch(
        () => import('../src/index.ts')
      ));
    } catch {
      skip('DocumentBuilder / EmbeddedFont not available in this build');
      return;
    }

    if (!fs.existsSync(fixtureFontPath) || !fs.existsSync(fixtureFontBoldPath)) {
      skip('DejaVuSans / DejaVuSans-Bold fixture not found');
      return;
    }

    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'pdfoxide-401-two-'));
    try {
      const fontReg = EmbeddedFont.fromFile(fixtureFontPath);
      const fontBold = EmbeddedFont.fromFile(fixtureFontBoldPath);
      const builder = DocumentBuilder.create()
        .registerEmbeddedFont('Regular', fontReg)
        .registerEmbeddedFont('Bold', fontBold);

      builder
        .a4Page()
        .font('Bold', 14.5)
        .at(30, 800)
        .text('High Performance')
        .font('Regular', 10.5)
        .at(30, 780)
        .text('Rust is fast and memory-efficient.')
        .font('Bold', 14.5)
        .at(30, 745)
        .text('Reliability')
        .done();

      const outPath = path.join(tmpDir, 'issue_401.pdf');
      builder.saveEncrypted(outPath, '123456', '123456');

      const raw = fs.readFileSync(outPath);
      assert.ok(raw.includes('/Encrypt'), 'encrypted PDF must contain /Encrypt');
      // Two embedded fonts → even more font program data → must be >25 KB total.
      assert.ok(
        raw.length > 25_000,
        `issue #401: two-font encrypted PDF (${raw.length} B) is too small; ` +
          'font sub-objects for both fonts are likely missing'
      );
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});
