// RFC 3161 Time Stamp Authority HTTP client wrapper for the Node binding.
//
// Mirrors the Python PyTsaClient, Go TsaClient, and C# TsaClient surfaces.
// (WASM TsaClient is intentionally unshipped — ureq, the HTTP driver in
// the Rust core, does not run under wasm32-unknown-unknown.)

import { getNative } from './native-loader.js';
import { Timestamp, TimestampHashAlgorithm } from './timestamp.js';

/** Constructor options for a TsaClient. */
export interface TsaClientOptions {
  /** TSA server URL, e.g. `https://freetsa.org/tsr`. */
  url: string;
  /** HTTP Basic auth username (optional). */
  username?: string;
  /** HTTP Basic auth password (optional). */
  password?: string;
  /** Request timeout in seconds. Defaults to 30. */
  timeoutSeconds?: number;
  /** Hash algorithm to request the TSA use. Defaults to SHA-256. */
  hashAlgorithm?: TimestampHashAlgorithm;
  /** Include a nonce in the request (RFC 3161 §2.4.1). Defaults to true. */
  useNonce?: boolean;
  /** Ask the TSA to include its certificate in the response. Defaults to true. */
  certReq?: boolean;
}

/**
 * HTTP client for an RFC 3161 Time Stamp Authority. Construct with a
 * URL (+ optional HTTP Basic auth), then call `requestTimestamp()` to
 * turn a blob of data into a Timestamp.
 */
export class TsaClient {
  private handle: unknown;
  private closed = false;

  constructor(options: TsaClientOptions) {
    if (!options || typeof options.url !== 'string' || options.url.length === 0) {
      throw new Error('TsaClient: options.url is required');
    }
    const handle = getNative().tsaClientCreate(
      options.url,
      options.username ?? '',
      options.password ?? '',
      options.timeoutSeconds ?? 30,
      options.hashAlgorithm ?? TimestampHashAlgorithm.Sha256,
      options.useNonce ?? true,
      options.certReq ?? true
    );
    if (!handle) {
      throw new Error('TsaClient: native create returned null');
    }
    this.handle = handle;
  }

  /**
   * Request a timestamp over `data`. The TSA hashes the data using its
   * configured algorithm and returns a TimeStampToken. Networking and
   * hashing both happen inside the native call — this method blocks.
   */
  requestTimestamp(data: Uint8Array | Buffer): Timestamp {
    this.assertOpen();
    const buf = Buffer.isBuffer(data) ? data : Buffer.from(data);
    const tsHandle = getNative().tsaRequestTimestamp(this.handle, buf);
    if (!tsHandle) {
      throw new Error('TsaClient.requestTimestamp: native returned null');
    }
    return Timestamp.fromNativeHandle(tsHandle);
  }

  /**
   * Request a timestamp over an already-computed hash. Use this when the
   * caller wants to hash the data themselves (e.g. signing the same
   * bytes end-to-end). `hashAlgorithm` overrides the client's default
   * for this single request.
   */
  requestTimestampHash(
    hash: Uint8Array | Buffer,
    hashAlgorithm: TimestampHashAlgorithm = TimestampHashAlgorithm.Sha256
  ): Timestamp {
    this.assertOpen();
    const buf = Buffer.isBuffer(hash) ? hash : Buffer.from(hash);
    const tsHandle = getNative().tsaRequestTimestampHash(this.handle, buf, hashAlgorithm);
    if (!tsHandle) {
      throw new Error('TsaClient.requestTimestampHash: native returned null');
    }
    return Timestamp.fromNativeHandle(tsHandle);
  }

  /** Release the native client. Idempotent. */
  close(): void {
    if (!this.closed && this.handle) {
      getNative().tsaClientFree(this.handle);
      this.closed = true;
      this.handle = null;
    }
  }

  private assertOpen(): void {
    if (this.closed) {
      throw new Error('TsaClient: handle is closed');
    }
  }
}
