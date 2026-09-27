// RFC 3161 TimeStampToken / TSTInfo wrapper for the Node binding.
//
// Mirrors the Python PyTimestamp, WASM WasmTimestamp, Go Timestamp, and
// C# Timestamp surfaces so every binding exposes the same shape.
// Sourced from:
//   - Timestamp.parse(buffer) — a standalone DER blob
//   - A Signature object's embedded timestamp (via the signature manager)
//   - A TsaClient.requestTimestamp(...) response (see tsa-client.ts)

import { getNative } from './native-loader.js';

/** Hash-algorithm enum matching the FFI contract (0 = unknown). */
export enum TimestampHashAlgorithm {
  Unknown = 0,
  Sha1 = 1,
  Sha256 = 2,
  Sha384 = 3,
  Sha512 = 4,
}

/**
 * Parsed RFC 3161 timestamp token. The underlying native handle is
 * released by `close()` or on garbage collection (N-API finalizer in
 * the C++ binding).
 */
export class Timestamp {
  // Opaque native pointer (Napi::External<void>). Consumers should not
  // inspect this — use the getters below.
  private handle: unknown;
  private closed = false;

  /** Internal — only the static constructors and sibling classes create Timestamps. */
  private constructor(handle: unknown) {
    this.handle = handle;
  }

  /**
   * Parse a DER blob that may be either a full TimeStampToken (CMS-wrapped)
   * or the bare TSTInfo SEQUENCE. Throws on parse failure.
   */
  static parse(data: Uint8Array | Buffer): Timestamp {
    if (!data || data.length === 0) {
      throw new Error('Timestamp.parse: data must not be empty');
    }
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
    const handle = getNative().timestampParse(buf);
    if (!handle) {
      throw new Error('Timestamp.parse: native parse returned null');
    }
    return new Timestamp(handle);
  }

  /**
   * Wrap an existing native timestamp handle (produced by
   * `signatureGetTimestamp` or `tsaRequestTimestamp`). Internal use only.
   */
  static fromNativeHandle(handle: unknown): Timestamp {
    return new Timestamp(handle);
  }

  /** Generation time as Unix epoch seconds. */
  get time(): number {
    this.assertOpen();
    return Number(getNative().timestampGetTime(this.handle));
  }

  /** Serial number as a hex string (no `0x` prefix). */
  get serial(): string {
    this.assertOpen();
    return getNative().timestampGetSerial(this.handle);
  }

  /** TSA policy OID in dotted-decimal form. */
  get policyOid(): string {
    this.assertOpen();
    return getNative().timestampGetPolicyOid(this.handle);
  }

  /** TSA name from the token (may be empty). */
  get tsaName(): string {
    this.assertOpen();
    return getNative().timestampGetTsaName(this.handle);
  }

  /** Hash algorithm used for the message imprint. */
  get hashAlgorithm(): TimestampHashAlgorithm {
    this.assertOpen();
    const v = getNative().timestampGetHashAlgorithm(this.handle);
    return v as TimestampHashAlgorithm;
  }

  /** Raw message-imprint hash bytes. */
  get messageImprint(): Uint8Array {
    this.assertOpen();
    return new Uint8Array(getNative().timestampGetMessageImprint(this.handle));
  }

  /** Raw DER token bytes. */
  get token(): Uint8Array {
    this.assertOpen();
    return new Uint8Array(getNative().timestampGetToken(this.handle));
  }

  /**
   * Cryptographic verify. Today this surfaces whatever the Rust core
   * returns — real TSA-signer verification lands when the CMS signer
   * path is wired through `pdf_timestamp_verify`.
   */
  verify(): boolean {
    this.assertOpen();
    return getNative().timestampVerify(this.handle);
  }

  /** Release the native handle. Idempotent. */
  close(): void {
    if (!this.closed && this.handle) {
      getNative().timestampFree(this.handle);
      this.closed = true;
      this.handle = null;
    }
  }

  /** Internal native handle accessor for sibling classes (Signature.addTimestamp). */
  getInternalHandle(): unknown {
    this.assertOpen();
    return this.handle;
  }

  private assertOpen(): void {
    if (this.closed) {
      throw new Error('Timestamp: handle is closed');
    }
  }
}
