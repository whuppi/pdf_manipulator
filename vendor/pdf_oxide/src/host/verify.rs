//! Digest verification of the signatures a document already carries.
//!
//! Compares the `/Contents` CMS blob against the bytes its `/ByteRange`
//! covers, read from the open source in 64 KiB chunks. This is not the
//! upstream verifier — the host streams the hash instead of holding the
//! whole file in memory, and it does not check certificate trust or
//! revocation.

#[cfg(feature = "signatures")]
use crate::error::Result;
#[cfg(feature = "signatures")]
use crate::document::PdfDocument;

/// Outcome of the digest check on one signature.
#[cfg(feature = "signatures")]
pub(crate) enum SigCheck {
    /// The digest matches and the signer's key verifies it.
    Valid,
    /// The digest does not match, the byte range is malformed, or the
    /// CMS blob does not parse.
    Invalid,
    /// The crypto path could not run at all (unknown digest algorithm,
    /// ECDSA, PSS, a missing `messageDigest` attribute).
    Unsupported,
}

/// One signature's verdict plus whether its byte ranges reach the last
/// byte of the file.
#[cfg(feature = "signatures")]
pub(crate) struct SigStatus {
    pub check: SigCheck,
    pub covers_whole_document: bool,
}

/// Verify every signature in order, reading the covered bytes from the
/// open document's source.
#[cfg(feature = "signatures")]
pub(crate) fn verify_all(
    doc: &mut PdfDocument,
    sigs: &[crate::signatures::SignatureInfo],
) -> Result<Vec<SigStatus>> {
    let len = doc.source_len()?;
    let mut statuses = Vec::with_capacity(sigs.len());
    for sig in sigs {
        statuses.push(verify_one(doc, sig, len)?);
    }
    Ok(statuses)
}

#[cfg(feature = "signatures")]
fn verify_one(
    doc: &mut PdfDocument,
    sig: &crate::signatures::SignatureInfo,
    len: u64,
) -> Result<SigStatus> {
    let Some(contents) = sig.contents() else {
        return Ok(SigStatus { check: SigCheck::Invalid, covers_whole_document: false });
    };
    // `/Contents` holds the CMS blob zero-padded to the width the
    // signer reserved; the DER parser refuses trailing bytes, so cut at the blob's
    // own definite length.
    let Some(der) = trimmed_der(contents) else {
        return Ok(SigStatus { check: SigCheck::Invalid, covers_whole_document: false });
    };

    let Some(covers_whole_document) = check_byte_range(sig.byte_range(), len) else {
        return Ok(SigStatus { check: SigCheck::Invalid, covers_whole_document: false });
    };

    let br = sig.byte_range();
    let verdict = crate::signatures::verify_signer_detached_with(der, |oid| {
        let mut hasher = crate::signatures::digest_for(oid)?;
        let mut chunk = vec![0u8; 65536];
        for (start, end) in [(br[0], br[0] + br[1]), (br[2], br[2] + br[3])] {
            let mut at = start as u64;
            while at < end as u64 {
                let want = ((end as u64 - at) as usize).min(chunk.len());
                let filled = match doc.read_source_at(at, &mut chunk[..want]) {
                    Ok(n) => n,
                    // A read error means we cannot hash the covered
                    // bytes; report Unknown rather than a false Invalid.
                    Err(_) => return None,
                };
                if filled == 0 {
                    return None;
                }
                hasher.update(&chunk[..filled]);
                at += filled as u64;
            }
        }
        Some(hasher.finalize().to_vec())
    });

    let check = match verdict {
        Ok(crate::signatures::SignerVerify::Valid) => SigCheck::Valid,
        Ok(crate::signatures::SignerVerify::Invalid) => SigCheck::Invalid,
        Ok(crate::signatures::SignerVerify::Unknown) => SigCheck::Unsupported,
        // The CMS blob does not parse: that is a broken signature, not
        // an unsupported one.
        Err(_) => SigCheck::Invalid,
    };
    Ok(SigStatus { check, covers_whole_document })
}

/// Cut a zero-padded `/Contents` blob at its own DER length. Returns
/// `None` when the prefix is not a DER definite-length header or the
/// body overruns the array.
#[cfg(feature = "signatures")]
fn trimmed_der(contents: &[u8]) -> Option<&[u8]> {
    if contents.len() < 2 {
        return None;
    }
    let first = contents[1] as usize;
    let (len_octets, body) = if first < 0x80 {
        (0, first)
    } else {
        let n = first & 0x7f;
        // More than four length octets claims a blob past 4 GiB: no
        // signature is that large, and the shift below would overflow.
        if n == 0 || n > 4 || contents.len() < 2 + n {
            return None;
        }
        let mut body = 0usize;
        for b in &contents[2..2 + n] {
            body = (body << 8) + *b as usize;
        }
        (n, body)
    };
    let total = 2 + len_octets + body;
    if total > contents.len() {
        return None;
    }
    Some(&contents[..total])
}

/// Validate a `/ByteRange` array against the source length. Returns
/// `Some(true)` when the ranges reach the last byte, `Some(false)` when
/// they are valid but leave bytes outside, `None` when the array is not
/// usable (wrong shape, negative, or covering bytes past the source).
#[cfg(feature = "signatures")]
fn check_byte_range(br: &[i64], len: u64) -> Option<bool> {
    if br.len() != 4 || br.iter().any(|&v| v < 0) {
        return None;
    }
    let (first_end, second_start, second_end) = (br[1] as u64, br[2] as u64, br[2].checked_add(br[3])? as u64);
    if br[0] != 0 || first_end > second_start {
        return None;
    }
    if second_end > len {
        return None;
    }
    Some(second_end == len)
}

#[cfg(all(test, feature = "signatures"))]
mod tests {
    use super::*;

    #[test]
    fn byte_range_covering_the_whole_file() {
        assert_eq!(check_byte_range(&[0, 10, 20, 30], 50), Some(true));
    }

    #[test]
    fn byte_range_leaving_a_gap() {
        assert_eq!(check_byte_range(&[0, 10, 20, 25], 50), Some(false));
    }

    #[test]
    fn byte_range_past_the_end() {
        assert_eq!(check_byte_range(&[0, 10, 20, 31], 50), None);
    }

    #[test]
    fn byte_range_negative() {
        assert_eq!(check_byte_range(&[0, -10, 20, 30], 50), None);
    }

    #[test]
    fn byte_range_wrong_count() {
        assert_eq!(check_byte_range(&[0, 10, 20], 50), None);
        assert_eq!(check_byte_range(&[], 50), None);
    }

    #[test]
    fn byte_range_overflowing_i64() {
        assert_eq!(check_byte_range(&[0, 10, 20, i64::MAX], 50), None);
    }

    #[test]
    fn der_trimmed_at_its_own_length() {
        // SEQUENCE, long form with one length octet: 0x30 0x81 0x03, 3 body bytes, then padding.
        assert_eq!(trimmed_der(&[0x30, 0x81, 3, 1, 2, 3, 0, 0]), Some(&[0x30, 0x81, 3, 1, 2, 3][..]));
        assert_eq!(trimmed_der(&[0x30, 2, 9, 9, 0]), Some(&[0x30, 2, 9, 9][..]));
    }

    #[test]
    fn der_refused_when_its_length_cannot_be_read() {
        assert_eq!(trimmed_der(&[0x30, 0x85, 1, 1, 1, 1, 1]), None);
        assert_eq!(trimmed_der(&[0x30, 0x82, 0xff, 0xff, 0]), None);
        assert_eq!(trimmed_der(&[0x30]), None);
    }

    #[test]
    fn byte_range_overlapping_segments() {
        assert_eq!(check_byte_range(&[0, 25, 20, 30], 50), None);
    }
}

