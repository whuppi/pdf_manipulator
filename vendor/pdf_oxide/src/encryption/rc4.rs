//! RC4 encryption/decryption for PDF.
//!
//! RC4 is a stream cipher used in PDF 1.4 and 1.5 for encryption.
//! While cryptographically weak by modern standards, it's still widely used
//! in legacy PDFs.
//!
//! PDF Spec: Section 7.6.2 - General Encryption Algorithm
//!
//! This is a simple, straightforward implementation of RC4 for PDF decryption.

#[cfg(feature = "legacy-crypto")]
/// Simple RC4 cipher implementation.
///
/// RC4 is a stream cipher that generates a pseudorandom keystream based on the key.
// ── pdf_manipulator patch: the provider builds the cipher as a keystream ──
pub(crate) struct Rc4Cipher {
    // ── end pdf_manipulator patch ──
    s: [u8; 256],
    i: u8,
    j: u8,
}

#[cfg(feature = "legacy-crypto")]
impl Rc4Cipher {
    /// Initialize RC4 cipher with a key.
    ///
    /// PDF Spec: RC4 key length is 5-16 bytes (40-128 bits)
    // ── pdf_manipulator patch: the provider builds the cipher as a keystream ──
    pub(crate) fn new(key: &[u8]) -> Self {
        // ── end pdf_manipulator patch ──
        let mut s = [0u8; 256];
        for (i, val) in s.iter_mut().enumerate() {
            *val = i as u8;
        }

        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(s[i]).wrapping_add(key[i % key.len()]);
            s.swap(i, j as usize);
        }

        Self { s, i: 0, j: 0 }
    }

    /// Generate the next byte of keystream.
    fn next_byte(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.s[self.i as usize]);
        self.s.swap(self.i as usize, self.j as usize);
        let k = self.s[self.i as usize].wrapping_add(self.s[self.j as usize]);
        self.s[k as usize]
    }

    /// Apply keystream to data (XOR operation).
    fn apply_keystream(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            *byte ^= self.next_byte();
        }
    }
}

// ── pdf_manipulator patch: the provider builds the cipher as a keystream ──
/// Only the [`crate::crypto::RustCryptoProvider`] builds this directly; PDF
/// callers go through [`rc4_crypt`] or `rc4_stream` on the active provider,
/// so a FIPS provider can reject them.
#[cfg(feature = "legacy-crypto")]
impl crate::crypto::StreamCipher for Rc4Cipher {
    fn apply(&mut self, data: &mut [u8]) {
        self.apply_keystream(data);
    }
}
// ── end pdf_manipulator patch ──

/// Encrypt or decrypt data using RC4 via the active
/// [`CryptoProvider`].
///
/// RC4 is symmetric, so encryption and decryption are the same operation.
///
/// Required by PDF Standard Security Handler R≤4 (ISO 32000-1
/// §7.6.3 Algorithm 1). Under the default
/// [`crate::crypto::RustCryptoProvider`] this succeeds; under the
/// FIPS-validated `AwsLcProvider` (or any other non-legacy provider)
/// it returns [`crate::Error::InvalidPdf`] preserving the underlying
/// `crypto::Error` cause so debugging stays actionable for *all*
/// failure modes — wrong key length (`InvalidInput`), backend failure
/// (`Backend`), and the FIPS-policy `AlgorithmNotPermitted`. Only the
/// last one carries the "rebuild without fips" remediation
/// hint.
///
/// [`CryptoProvider`]: crate::crypto::CryptoProvider
pub fn rc4_crypt(key: &[u8], data: &[u8]) -> crate::Result<Vec<u8>> {
    crate::crypto::active()
        .symmetric()
        .rc4(key, data)
        // ── pdf_manipulator patch: RC4 as a keystream, for readers that decrypt in pieces ──
        .map_err(refusal)
}

/// RC4 as a keystream from the active [`CryptoProvider`]; refused exactly
/// as [`rc4_crypt`] is.
///
/// [`CryptoProvider`]: crate::crypto::CryptoProvider
pub(crate) fn rc4_stream(key: &[u8]) -> crate::Result<Box<dyn crate::crypto::StreamCipher>> {
    crate::crypto::active()
        .symmetric()
        .rc4_stream(key)
        .map_err(refusal)
}

fn refusal(e: crate::crypto::Error) -> crate::Error {
    match &e {
        crate::crypto::Error::AlgorithmNotPermitted { .. } => {
            crate::Error::InvalidPdf(format!(
                "RC4 rejected by active CryptoProvider '{}': {}. \
                 RC4 is required for PDF Standard Security R≤4. \
                 Re-encrypt at R=6 (AES-256) or build pdf_oxide \
                 without the 'fips' feature so the default \
                 'rust-crypto' provider stays active.",
                crate::crypto::active().name(),
                e
            ))
        },
        crate::crypto::Error::InvalidInput(s) => {
            crate::Error::InvalidPdf(format!("RC4 invalid input: {s}"))
        },
        crate::crypto::Error::Backend(s) => {
            crate::Error::InvalidPdf(format!("RC4 backend error: {s}"))
        },
        crate::crypto::Error::Verification(s) => {
            crate::Error::InvalidPdf(format!("RC4 verification error: {s}"))
        },
    }
}
// ── end pdf_manipulator patch ──

#[cfg(all(test, feature = "legacy-crypto"))]
mod tests {
    use super::*;

    #[test]
    fn test_rc4_symmetric() {
        let key = b"testkey";
        let plaintext = b"Hello, World!";

        let ciphertext = rc4_crypt(key, plaintext).unwrap();
        let decrypted = rc4_crypt(key, &ciphertext).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
        assert_ne!(plaintext, &ciphertext[..]);
    }

    #[test]
    fn test_rc4_empty() {
        let result = rc4_crypt(b"testkey", b"").unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_rc4_different_keys() {
        let plaintext = b"Secret message";
        let encrypted1 = rc4_crypt(b"key1", plaintext).unwrap();
        let encrypted2 = rc4_crypt(b"key2", plaintext).unwrap();
        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_rc4_known_vector() {
        let key = b"Key";
        let plaintext = b"Plaintext";
        let ciphertext = rc4_crypt(key, plaintext).unwrap();
        assert_ne!(plaintext, &ciphertext[..]);
        let decrypted = rc4_crypt(key, &ciphertext).unwrap();
        assert_eq!(plaintext, &decrypted[..]);
    }
}
