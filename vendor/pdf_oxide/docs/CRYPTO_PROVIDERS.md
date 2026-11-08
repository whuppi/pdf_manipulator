# Cryptographic providers

`pdf_oxide`'s encryption and signature paths are abstracted behind the
`pdf_oxide::crypto::CryptoProvider` trait so deployments can choose:

- **`RustCryptoProvider`** (default) — pure Rust, built on `sha2`,
  `sha1`, `md-5`, `aes`, `rsa`, `p256`, `p384`, `getrandom`. Permits
  every algorithm the PDF spec references, including the legacy
  MD5+RC4 path needed for ISO 32000-1 R≤4 documents.
- **`AwsLcProvider`** (opt-in) — backed by Amazon's
  [`aws-lc-rs`](https://crates.io/crates/aws-lc-rs), which is FIPS
  140-3 validated as of 2024. Refuses MD5, RC4, and SHA-1 signing
  with a clear remediation message.

Tracking issue: [#236](https://github.com/yfedoseev/pdf_oxide/issues/236).

## Picking a provider

| Need | Provider | How |
|---|---|---|
| General-purpose use | `RustCryptoProvider` | nothing — it's the default |
| FIPS 140-3 compliance | `AwsLcProvider` | build with `--features fips` and call `crypto::set_provider(...)` at startup |
| Hardware-rooted keys (HSM, PKCS#11, Cloud KMS) | custom | implement `CryptoProvider` for your backend, call `crypto::set_provider(Arc::new(YourProvider))` |
| Sovereign-jurisdiction algorithms (GOST, SM2/3/4) | custom | same — implement `CryptoProvider` against your country's crypto library |

## Switching to FIPS

```rust
use std::sync::Arc;
use pdf_oxide::crypto::{set_provider, AwsLcProvider};

fn main() {
    // Must run before any pdf_oxide operation that uses crypto.
    set_provider(Arc::new(AwsLcProvider::new()))
        .expect("crypto provider already installed");

    // Now every PDF open / signature verify routes through aws-lc-rs.
    let _doc = pdf_oxide::PdfDocument::open("encrypted.pdf").unwrap();
}
```

Build:

```bash
cargo build --features fips
# Or, with python bindings:
cargo build --features python,fips
```

The `aws-lc-rs` dependency compiles AWS-LC C source; first build
adds ~5 minutes. Subsequent builds are cached.

## Algorithm policy

| Algorithm | RustCryptoProvider | AwsLcProvider |
|---|---|---|
| MD5 | ✅ | ❌ `AlgorithmNotPermitted` |
| SHA-1 hash | ✅ | ✅ (verify-only intent — NIST SP 800-131A) |
| SHA-1 signing | ✅ | ❌ |
| SHA-256 / 384 / 512 | ✅ | ✅ |
| AES-128/256-CBC PKCS#7 | ✅ | ✅ |
| AES-128/256-CBC no-padding | ✅ | ✅ |
| RC4 | ✅ | ❌ |
| RSA-PKCS#1-v1.5 verify (SHA-256+) | ✅ | partial (Backend until aws-lc-rs exposes RSA_PKCS1_PRIM_VERIFY) |
| RSA-PKCS#1-v1.5 verify (SHA-1) | ✅ | partial (same) |
| RSA-PKCS#1-v1.5 sign | ✅ | not yet wired |
| RSA-PSS verify (SHA-256+) | ✅ | ✅ |
| RSA-PSS verify (SHA-1) | ❌ | ❌ |
| ECDSA P-256 / P-384 verify | ✅ | ✅ |

## What FIPS rejects, in PDF terms

PDF Standard Security R≤4 (V≤4) — that's PDF 1.4–1.6 documents
encrypted with a password — uses MD5 for key derivation and RC4 or
AES-128 for the actual cipher. ISO 32000-1 §7.6.3 spells out the
algorithms. NIST FIPS 140-3 forbids MD5 and RC4 for any use, so
opening one of these documents under `AwsLcProvider` fails with:

```
active CryptoProvider 'aws-lc-rs' rejects PDF Standard Security R=4
(R≤4 requires MD5; FIPS 140-3 forbids MD5).
Re-encrypt the document at R=6 (AES-256) or build pdf_oxide
without the 'fips' feature so the default 'rust-crypto'
provider stays active.
```

The remediation is to re-encrypt the document at R=6 (PDF 2.0
AES-256, ISO 32000-2 §7.6.4, "Algorithm 2.B") under any
non-FIPS-restricted environment. R=6 documents open cleanly under
`AwsLcProvider`.

## Custom providers (HSM, PKCS#11, sovereign algorithms)

The trait is intentionally narrow — five sub-traits cover everything
PDF needs:

```rust
pub trait CryptoProvider: Send + Sync + 'static {
    fn name(&self) -> &'static str;
    fn is_legacy_allowed(&self) -> bool;
    fn hasher(&self, algo: HashAlgorithm) -> Result<Box<dyn Hasher>>;
    fn symmetric(&self) -> &dyn SymmetricCipher;
    fn verifier(&self) -> &dyn SignatureVerifier;
    fn random_bytes(&self, out: &mut [u8]) -> Result<()>;
    fn signer(&self, key: &SigningKeyMaterial<'_>) -> Result<Box<dyn Signer>>;
}
```

`SigningKeyMaterial` is `#[non_exhaustive]` — adding `Pkcs11Slot {
session_id: u32, key_label: &str }` later won't be a breaking change.

For a sovereign-algorithm provider, the cleanest path is:

1. Implement `Hasher` for each of your country's hash algos
   (e.g. `Streebog256`, `SM3`).
2. Implement `SymmetricCipher::aes_cbc_encrypt` to dispatch to your
   block cipher (e.g. Kuznyechik, SM4) when the caller passes
   `AesKeySize` — *or* widen `AesKeySize` to include your block
   cipher in a downstream fork. The trait deliberately keeps
   AES-named methods to match PDF spec usage; sovereign-algorithm
   support past that point is a forking concern.
3. Implement `SignatureVerifier` to handle GOST R 34.10 / SM2 by
   adding new `EcCurve` / `RsaScheme` variants if needed.

## Runtime install

`set_provider` accepts `Arc<dyn CryptoProvider>` and is set-once:

```rust
crypto::set_provider(Arc::new(MyProvider::new()))?;
```

Subsequent calls return `SetProviderError::AlreadySet`. This is
deliberate: swapping the provider mid-process while in-flight crypto
state exists (FIPS module self-test, HSM session) would be a
soundness hazard. Tests that need a fresh provider should run in
separate process namespaces (`cargo test`'s default per-binary
isolation handles this for free).

## Cross-binding exposure

The runtime API is identical across every binding:

| Binding | Active | Available | Switch to FIPS |
|---|---|---|---|
| Python | `pdf_oxide.crypto_active_provider()` | `pdf_oxide.crypto_available_providers()` | `pdf_oxide.crypto_use_fips()` |
| Node.js / TS | `pdf_oxide.getActiveCryptoProvider()` | `pdf_oxide.isFipsCryptoAvailable()` | `pdf_oxide.useFipsCryptoProvider()` |
| C# | `PdfDocument.GetActiveCryptoProvider()` | `PdfDocument.IsFipsCryptoAvailable()` | `PdfDocument.UseFipsCryptoProvider()` |
| Go | `pdf_oxide.ActiveCryptoProvider()` | `pdf_oxide.IsFipsCryptoAvailable()` | `pdf_oxide.UseFipsCryptoProvider()` |

## Picking the right install

| Want | Install |
|---|---|
| Python + default | `pip install pdf_oxide==0.3.45` |
| Python + FIPS | `pip install pdf_oxide_fips==0.3.45` |
| Node + default | `npm install pdf-oxide@0.3.45` |
| Node + FIPS | `npm install pdf-oxide-fips@0.3.45` |
| .NET + default | `dotnet add package PdfOxide --version 0.3.45` |
| .NET + FIPS | `dotnet add package PdfOxide.Fips --version 0.3.45` |
| Go + default | `go get github.com/yfedoseev/pdf_oxide/go@v0.3.45` |
| Go + FIPS | `go get github.com/yfedoseev/pdf_oxide/go-fips@v0.3.45` |

The `-fips` distributions ship single-provider binaries (auditors
typically require this). Both default and FIPS variants of any
release tag move in lockstep — non-crypto code paths are byte-equal.

### Why not `pip install pdf_oxide[fips]`?

Python extras (`pip install pkg[extra]`) can add dependencies but
can't swap the compiled `.so` that ships inside the wheel. The
industry pattern for FIPS variants is parallel distributions
(cryptography, pyOpenSSL both ship this way); we follow suit.

### Why `go-fips` instead of a build flag?

Go modules are import-path-bound, so the FIPS variant lives at
`github.com/yfedoseev/pdf_oxide/go-fips`. Both submodules re-export
the same Go API surface — only the linked native static lib
differs. Users pick at `go get` time without needing build tags or
CGO flags.
