/**
 * Smoke tests for the Node Timestamp + TsaClient surfaces.
 *
 * Scope: API shape + close-idempotence + constructor-argument validation.
 * These run without network access; a full RFC 3161 round-trip against a
 * live TSA is exercised in the Rust test suite (tests/test_tsa_client.rs)
 * and replayed by CI when a TSA is reachable.
 */

import assert from 'node:assert/strict';
import { before, describe, it } from 'node:test';

let Timestamp, TimestampHashAlgorithm, TsaClient;

describe('Timestamp + TsaClient smoke tests', () => {
  before(async () => {
    try {
      const mod = await import('../lib/index.js');
      Timestamp = mod.Timestamp;
      TimestampHashAlgorithm = mod.TimestampHashAlgorithm;
      TsaClient = mod.TsaClient;
    } catch (err) {
      throw new Error(
        `Failed to import compiled Node binding — run 'npm run build' first. (${err?.message})`
      );
    }
  });

  it('exports Timestamp, TimestampHashAlgorithm, TsaClient', () => {
    assert.equal(typeof Timestamp, 'function', 'Timestamp should be a class');
    assert.equal(typeof Timestamp.parse, 'function', 'Timestamp.parse should exist');
    assert.equal(typeof TsaClient, 'function', 'TsaClient should be a class');
    assert.equal(TimestampHashAlgorithm.Sha256, 2);
    assert.equal(TimestampHashAlgorithm.Sha512, 4);
  });

  it('Timestamp.parse rejects empty input', () => {
    assert.throws(() => Timestamp.parse(new Uint8Array(0)), /must not be empty/i);
  });

  it('Timestamp.parse rejects garbage', () => {
    // Random bytes → not a valid DER token → native parse throws.
    const junk = Buffer.from('not a timestamp token'.repeat(10));
    assert.throws(() => Timestamp.parse(junk));
  });

  it('TsaClient requires options.url', () => {
    assert.throws(() => new TsaClient({ url: '' }), /url is required/i);
    assert.throws(() => new TsaClient({}), /url is required/i);
  });

  it('TsaClient constructor accepts a minimal config', () => {
    // Constructor should not throw for a well-formed URL even without
    // making a network call — the TSA is only contacted on requestTimestamp().
    const c = new TsaClient({ url: 'https://freetsa.org/tsr' });
    try {
      assert.ok(c);
    } finally {
      c.close();
      // Idempotent close.
      c.close();
    }
  });

  it('TsaClient.requestTimestamp throws after close', () => {
    const c = new TsaClient({ url: 'https://freetsa.org/tsr' });
    c.close();
    assert.throws(() => c.requestTimestamp(Buffer.from('hello')), /handle is closed/i);
  });
});
