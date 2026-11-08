//! End-to-end crypto round-trip for `signatures::cms_verify::verify_signer`.
//!
//! The fixtures under `tests/fixtures/signatures/` were built with
//! `openssl cms -sign -md sha256 -nodetach` against a fresh self-signed
//! RSA-2048 certificate — see the generation recipe in
//! `signatures::cms_verify` tests / the v0.3.38 handoff. The signer's
//! cert is embedded inside the SignedData, so our verifier can recover
//! the public key straight from the CMS blob.

#![cfg(feature = "signatures")]

use pdf_oxide::signatures::{verify_signer, verify_signer_detached, SignerVerify};

const VALID: &[u8] = include_bytes!("fixtures/signatures/rsa_sha256_round_trip.cms");
const TAMPERED: &[u8] = include_bytes!("fixtures/signatures/rsa_sha256_round_trip_tampered.cms");

// Detached-signature pair: `DETACHED` is the CMS blob, `DETACHED_CONTENT`
// is the raw bytes that were fed to `openssl cms -sign -binary`. A
// messageDigest signed attribute inside the blob binds to
// `sha256(DETACHED_CONTENT)`.
const DETACHED: &[u8] = include_bytes!("fixtures/signatures/rsa_sha256_detached.cms");
const DETACHED_CONTENT: &[u8] =
    include_bytes!("fixtures/signatures/rsa_sha256_detached_content.bin");

#[test]
fn rsa_sha256_round_trip_is_valid() {
    let result = verify_signer(VALID).expect("parse valid CMS blob");
    assert_eq!(
        result,
        SignerVerify::Valid,
        "openssl cms -sign RSA/SHA-256 blob must verify against its embedded cert"
    );
}

#[test]
fn detached_signature_verifies_with_matching_content() {
    let result = verify_signer_detached(DETACHED, DETACHED_CONTENT).expect("parse detached");
    assert_eq!(
        result,
        SignerVerify::Valid,
        "detached RSA/SHA-256 blob must verify when given the exact signed content"
    );
}

#[test]
fn detached_signature_is_invalid_with_wrong_content() {
    // Flip a single byte of the content — messageDigest must mismatch
    // even though the signer-attribute crypto path still passes.
    let mut tampered = DETACHED_CONTENT.to_vec();
    tampered[0] ^= 0x01;
    let result = verify_signer_detached(DETACHED, &tampered).expect("parse detached");
    assert_eq!(
        result,
        SignerVerify::Invalid,
        "wrong content must flip the verdict to Invalid via messageDigest mismatch"
    );
}

#[test]
fn detached_signature_verify_signer_alone_still_valid() {
    // The signer-attributes crypto path doesn't depend on the detached
    // content — it only proves authenticity of the attribute bundle.
    // So plain `verify_signer` must still report Valid on the detached
    // blob regardless of what the caller thinks the content is.
    let result = verify_signer(DETACHED).expect("parse detached");
    assert_eq!(result, SignerVerify::Valid);
}

#[test]
fn tampered_cms_blob_is_invalid() {
    // The fixture differs from the valid one by a single flipped bit
    // near the end of the DER (falls inside the signature OCTET STRING
    // or the trailer). It should still parse as CMS but the RSA check
    // must fail. Depending on where the flip lands, the result can be
    // `Invalid` (signature-value byte flipped) or a DER parse error
    // (structural byte flipped) — both are acceptable here.
    match verify_signer(TAMPERED) {
        Ok(SignerVerify::Invalid) => {},
        Ok(other) => panic!("tampered blob must not verify; got {other:?}"),
        Err(_) => {
            // DER parse error is also a legitimate rejection for a
            // blob whose flipped byte corrupted the ASN.1 framing.
        },
    }
}
