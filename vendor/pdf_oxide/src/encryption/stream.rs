//! Stream bodies through the standard security handler, in pieces.
//!
//! This file is fork-only code (NOT upstream).
//!
//! [`Cipher`] is what the handler derives for one object (its key, with no
//! data); [`Decrypt`] reads a stream's ciphertext through it and [`Encrypt`]
//! writes plaintext out through it. Neither holds more than a chunk, so a
//! stream of any size decrypts or encrypts in constant memory. The one-shot
//! `EncryptionHandler::decrypt_stream` stays for strings, and for the
//! equality proofs that tie the two together.

use crate::crypto::{active, AesKeySize, Padding, StreamCipher};
use crate::error::{Error, Result};
use crate::object::Object;
use std::collections::HashMap;
use std::io::{self, Read, Write};

const BLOCK: usize = 16;
const CHUNK: usize = 64 * 1024;

/// How the bytes of one object are enciphered.
pub(crate) enum Cipher {
    /// Stored as they are: no encryption, or a `/Crypt /Identity` filter.
    Identity,
    /// AES-CBC; the 16-byte IV leads the stream.
    Aes { size: AesKeySize, key: Vec<u8> },
    /// RC4 under the per-object key.
    Rc4 { key: Vec<u8> },
}

impl Cipher {
    /// `inner`'s bytes, deciphered.
    pub(crate) fn decrypting<R: Read>(&self, inner: R) -> Result<Decrypt<R>> {
        Ok(match self {
            Cipher::Identity => Decrypt::Identity(inner),
            Cipher::Aes { size, key } => Decrypt::Aes(Box::new(AesDecrypt {
                inner,
                size: *size,
                key: key.clone(),
                iv: None,
                held: Vec::new(),
                plain: Vec::new(),
                scratch: vec![0u8; CHUNK],
                at: 0,
                done: false,
            })),
            Cipher::Rc4 { key } => Decrypt::Rc4(inner, super::rc4::rc4_stream(key)?),
        })
    }

    /// A writer that enciphers what is written to it into `out`, with `iv`
    /// as the leading block when the cipher is AES.
    pub(crate) fn encrypting<W: Write>(&self, out: W, iv: [u8; BLOCK]) -> Result<Encrypt<W>> {
        Ok(match self {
            Cipher::Identity => Encrypt::Identity(out),
            Cipher::Aes { size, key } => Encrypt::Aes(Box::new(AesEncrypt {
                out,
                size: *size,
                key: key.clone(),
                iv,
                started: false,
                held: Vec::new(),
            })),
            Cipher::Rc4 { key } => Encrypt::Rc4(out, super::rc4::rc4_stream(key)?),
        })
    }

    /// The length of `plain` bytes once enciphered.
    pub(crate) fn encrypted_len(&self, plain: u64) -> u64 {
        match self {
            Cipher::Aes { .. } => BLOCK as u64 + (plain / BLOCK as u64 + 1) * BLOCK as u64,
            _ => plain,
        }
    }
}

/// Whether the handler leaves this stream as it is: an object stream or a
/// cross-reference stream (ISO 32000-2 §7.6.3), or one whose first filter is
/// `/Crypt` naming `/Identity` or nothing (§7.4.10).
pub(crate) fn is_exempt(dict: &HashMap<String, Object>) -> bool {
    if matches!(
        dict.get("Type").and_then(|t| t.as_name()),
        Some("ObjStm") | Some("XRef")
    ) {
        return true;
    }
    let first = |o: &Object| -> Option<Object> {
        match o {
            Object::Array(items) => items.first().cloned(),
            other => Some(other.clone()),
        }
    };
    let crypt_first = dict
        .get("Filter")
        .and_then(first)
        .is_some_and(|f| f.as_name() == Some("Crypt"));
    if !crypt_first {
        return false;
    }
    let name = dict
        .get("DecodeParms")
        .and_then(first)
        .and_then(|p| p.as_dict().and_then(|d| d.get("Name")).and_then(|n| n.as_name().map(str::to_string)));
    name.is_none_or(|n| n == "Identity")
}

/// The typed error back out of a reader or writer that carried it as an
/// `io::Error`.
pub(crate) fn from_io(e: io::Error) -> Error {
    if e.get_ref().is_some_and(|inner| inner.is::<Error>()) {
        let inner = e.into_inner().expect("checked above");
        return *inner.downcast::<Error>().expect("checked above");
    }
    Error::Io(e)
}

fn io_err(e: Error) -> io::Error {
    io::Error::other(e)
}

fn bad_crypto(what: &str, e: impl std::fmt::Display) -> io::Error {
    io_err(Error::InvalidPdf(format!("{what}: {e}")))
}

/// Ciphertext in, plaintext out.
pub(crate) enum Decrypt<R> {
    Identity(R),
    Aes(Box<AesDecrypt<R>>),
    Rc4(R, Box<dyn StreamCipher>),
}

impl<R: Read> Read for Decrypt<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Decrypt::Identity(inner) => inner.read(buf),
            Decrypt::Rc4(inner, cipher) => {
                let n = inner.read(buf)?;
                cipher.apply(&mut buf[..n]);
                Ok(n)
            },
            Decrypt::Aes(aes) => aes.read(buf),
        }
    }
}

/// AES-CBC with the last block held back until the source ends, so the
/// PKCS#7 padding is stripped from the block that carries it.
pub(crate) struct AesDecrypt<R> {
    inner: R,
    size: AesKeySize,
    key: Vec<u8>,
    /// The chaining block: the stream's IV, then the last ciphertext block
    /// decrypted. `None` until the IV is read.
    iv: Option<[u8; BLOCK]>,
    /// Ciphertext read and not yet decrypted.
    held: Vec<u8>,
    plain: Vec<u8>,
    /// Where the source's pieces land before they join `held`.
    scratch: Vec<u8>,
    at: usize,
    done: bool,
}

impl<R: Read> AesDecrypt<R> {
    fn decrypt_blocks(&mut self, count: usize) -> io::Result<()> {
        let iv = self.iv.expect("the IV is read before any block");
        let blocks: Vec<u8> = self.held.drain(..count).collect();
        self.plain = active()
            .symmetric()
            .aes_cbc_decrypt(self.size, &self.key, &iv, &blocks, Padding::None)
            .map_err(|e| bad_crypto("AES decryption failed", e))?;
        self.iv = Some(blocks[blocks.len() - BLOCK..].try_into().expect("a block"));
        self.at = 0;
        Ok(())
    }

    /// Reads and deciphers until `plain` holds bytes or the stream ends.
    fn refill(&mut self) -> io::Result<()> {
        if self.iv.is_none() {
            let mut iv = [0u8; BLOCK];
            self.inner.read_exact(&mut iv).map_err(|e| {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    io_err(Error::InvalidPdf("AES encrypted data too short".to_string()))
                } else {
                    e
                }
            })?;
            self.iv = Some(iv);
        }
        loop {
            let n = self.inner.read(&mut self.scratch)?;
            if n == 0 {
                return self.finish();
            }
            self.held.extend_from_slice(&self.scratch[..n]);
            // Decrypt every block that is surely not the last: one more byte
            // is held behind it.
            let ready = (self.held.len() - 1) / BLOCK * BLOCK;
            if ready > 0 {
                return self.decrypt_blocks(ready);
            }
        }
    }

    fn finish(&mut self) -> io::Result<()> {
        self.done = true;
        self.plain.clear();
        self.at = 0;
        if self.held.is_empty() {
            return Ok(());
        }
        if self.held.len() % BLOCK != 0 {
            return Err(bad_crypto(
                "AES decryption failed",
                "ciphertext is not a whole number of blocks",
            ));
        }
        let all = self.held.len();
        self.decrypt_blocks(all)?;
        let pad = *self.plain.last().expect("a block was decrypted") as usize;
        if pad == 0 || pad > BLOCK || self.plain[self.plain.len() - pad..].iter().any(|&b| b as usize != pad)
        {
            return Err(bad_crypto("AES decryption failed", "invalid PKCS#7 padding"));
        }
        self.plain.truncate(self.plain.len() - pad);
        Ok(())
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.at == self.plain.len() {
            if self.done {
                return Ok(0);
            }
            self.refill()?;
        }
        let n = (self.plain.len() - self.at).min(buf.len());
        buf[..n].copy_from_slice(&self.plain[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

/// Plaintext in, ciphertext out. `finish` must be called: AES writes its
/// padding block there.
pub(crate) enum Encrypt<W> {
    Identity(W),
    Aes(Box<AesEncrypt<W>>),
    Rc4(W, Box<dyn StreamCipher>),
}

pub(crate) struct AesEncrypt<W> {
    out: W,
    size: AesKeySize,
    key: Vec<u8>,
    iv: [u8; BLOCK],
    started: bool,
    /// Plaintext not yet a whole block.
    held: Vec<u8>,
}

impl<W: Write> AesEncrypt<W> {
    fn start(&mut self) -> io::Result<()> {
        if !self.started {
            self.out.write_all(&self.iv)?;
            self.started = true;
        }
        Ok(())
    }

    fn encrypt_blocks(&mut self, blocks: &[u8]) -> io::Result<()> {
        let cipher = active()
            .symmetric()
            .aes_cbc_encrypt(self.size, &self.key, &self.iv, blocks, Padding::None)
            .map_err(|e| bad_crypto("AES encryption failed", e))?;
        self.iv = cipher[cipher.len() - BLOCK..].try_into().expect("a block");
        self.out.write_all(&cipher)
    }
}

impl<W: Write> Encrypt<W> {
    /// Writes what is held, padded, and gives the writer back.
    pub(crate) fn finish(self) -> io::Result<W> {
        match self {
            Encrypt::Identity(out) | Encrypt::Rc4(out, _) => Ok(out),
            Encrypt::Aes(mut aes) => {
                aes.start()?;
                let pad = BLOCK - aes.held.len();
                let mut last = std::mem::take(&mut aes.held);
                last.extend(std::iter::repeat(pad as u8).take(pad));
                aes.encrypt_blocks(&last)?;
                Ok(aes.out)
            },
        }
    }
}

impl<W: Write> Write for Encrypt<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        match self {
            Encrypt::Identity(out) => out.write(data),
            Encrypt::Rc4(out, cipher) => {
                let mut piece = data.to_vec();
                cipher.apply(&mut piece);
                out.write_all(&piece)?;
                Ok(data.len())
            },
            Encrypt::Aes(aes) => {
                aes.start()?;
                aes.held.extend_from_slice(data);
                let whole = aes.held.len() / BLOCK * BLOCK;
                if whole > 0 {
                    let blocks: Vec<u8> = aes.held.drain(..whole).collect();
                    aes.encrypt_blocks(&blocks)?;
                }
                Ok(data.len())
            },
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Encrypt::Identity(out) | Encrypt::Rc4(out, _) => out.flush(),
            Encrypt::Aes(aes) => aes.out.flush(),
        }
    }
}
