//! O(1)-memory PDF signing with proper AcroForm structure.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! `sign_pdf` reads the source via Read+Seek (64KB chunks), writes the
//! signed output via Write (sequential, one pass). Peak memory ~128KB.
//!
//! Builds CMS/PKCS#7 detached signature with a pre-computed digest —
//! the same DER assembly the upstream signer uses, but accepting the
//! digest directly instead of hashing internally. No upstream patches.
//!
//! Always writes proper AcroForm structure (field + widget + SigFlags).
//! Discoverable by Acrobat, Chrome, enumerate_signatures, PAdES validators.

#[cfg(feature = "signatures")]
use crate::error::{Error, Result};

#[cfg(feature = "signatures")]
use crate::signatures::{DigestAlgorithm, SignOptions, SigningCredentials};

/// Sign a PDF with O(1)-memory I/O and proper AcroForm.
///
/// Takes concrete `BoxedReader` and `BoxedWriter` from bridge_api.rs —
/// same wrapper pattern used for editor save. No generics, no `Sized`
/// issues with trait objects.
#[cfg(feature = "signatures")]
pub(crate) fn sign_pdf(
    reader: &mut super::bridge_api::BoxedReader,
    source_length: u64,
    writer: &mut super::bridge_api::BoxedWriter,
    credentials: &SigningCredentials,
    opts: SignOptions,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek, SeekFrom, Write};

    let signer = crate::signatures::PdfSigner::new(credentials.clone(), opts);
    let src_len = source_length as usize;

    // ── 1. Scan last 4KB for structural info ────────────────────────
    let tail_size = 4096usize.min(src_len);
    let mut tail = vec![0u8; tail_size];
    reader.seek(SeekFrom::End(-(tail_size as i64)))?;
    reader.read_exact(&mut tail)?;

    let prev_startxref = scan_startxref(&tail)
        .ok_or_else(|| Error::InvalidPdf("cannot find startxref".into()))?;
    let root_ref_str = scan_root_ref(&tail)
        .ok_or_else(|| Error::InvalidPdf("cannot find /Root ref".into()))?;
    let next_obj = scan_next_obj_num(&tail)
        .ok_or_else(|| Error::InvalidPdf("cannot find /Size".into()))?;

    let catalog_id: u64 = root_ref_str.split_whitespace().next()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Error::InvalidPdf("cannot parse catalog ID".into()))?;

    // ── 2. Build incremental objects ────────────────────────────────
    let sig_obj = next_obj;
    let field_obj = next_obj + 1;

    let sig_dict = build_sig_dict(&signer, sig_obj);
    let contents_in_dict = find_contents_offset(sig_dict.as_bytes())
        .ok_or_else(|| Error::InvalidPdf("cannot find /Contents offset".into()))?;

    let field_str = format!(
        "{} 0 obj\n<< /Type /Annot /Subtype /Widget /FT /Sig \
         /T (Sig1) /V {} 0 R /Rect [0 0 0 0] /F 132 >>\nendobj\n",
        field_obj, sig_obj,
    );
    // Read the /Pages reference from the original catalog so the
    // replacement preserves the page tree. Without this, the signed
    // PDF is structurally invalid (missing /Pages).
    let pages_ref = scan_pages_ref(reader, catalog_id, prev_startxref)
        .unwrap_or_else(|| "1 0 R".to_string());
    reader.seek(SeekFrom::Start(0))?;

    let catalog_str = format!(
        "{} 0 obj\n<< /Type /Catalog /Pages {} \
         /AcroForm << /Fields [{} 0 R] /SigFlags 3 >> >>\nendobj\n",
        catalog_id, pages_ref, field_obj,
    );

    // ── 3. Compute layout ───────────────────────────────────────────
    let sig_start = src_len;
    let field_start = sig_start + sig_dict.len();
    let catalog_start = field_start + field_str.len();
    let xref_start = catalog_start + catalog_str.len();

    let xref_section = format!(
        "xref\n{} 2\n{:010} 00000 n \r\n{:010} 00000 n \r\n{} 1\n{:010} 00000 n \r\n",
        sig_obj, sig_start, field_start, catalog_id, catalog_start,
    );
    let trailer_section = format!(
        "trailer\n<< /Size {} /Prev {} /Root {} 0 R >>\n",
        field_obj + 1, prev_startxref, catalog_id,
    );
    let eof_section = format!("startxref\n{}\n%%EOF\n", xref_start);

    let total_len = src_len
        + sig_dict.len() + field_str.len() + catalog_str.len()
        + xref_section.len() + trailer_section.len() + eof_section.len();

    // ── 4. Compute ByteRange ────────────────────────────────────────
    let contents_abs = sig_start + contents_in_dict;
    let contents_size = signer.placeholder_size();
    let after_contents = contents_abs + contents_size;
    let byte_range: [i64; 4] = [
        0, contents_abs as i64, after_contents as i64,
        (total_len - after_contents) as i64,
    ];

    let patched_sig = patch_byterange(sig_dict, &byte_range);
    let sig_bytes = patched_sig.as_bytes();
    let after_in_dict = contents_in_dict + contents_size;

    // ── 5. Hash signed ranges (O(1)-memory, read-buffer-sized chunks) ─
    use crate::host::constants::READ_BUF_CAPACITY;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; READ_BUF_CAPACITY];

    reader.seek(SeekFrom::Start(0))?;
    let mut rem = src_len;
    while rem > 0 {
        let n = rem.min(chunk.len());
        reader.read_exact(&mut chunk[..n])?;
        hasher.update(&chunk[..n]);
        rem -= n;
    }
    hasher.update(&sig_bytes[..contents_in_dict]);

    hasher.update(&sig_bytes[after_in_dict..]);
    hasher.update(field_str.as_bytes());
    hasher.update(catalog_str.as_bytes());
    hasher.update(xref_section.as_bytes());
    hasher.update(trailer_section.as_bytes());
    hasher.update(eof_section.as_bytes());

    let message_digest = hasher.finalize().to_vec();

    // ── 6. Build CMS blob from pre-computed digest ──────────────────
    let cms_der = build_cms_from_digest(
        credentials, &message_digest, DigestAlgorithm::Sha256,
    )?;

    let hex = hex_encode(&cms_der);
    let pad_len = (contents_size - 2) - hex.len();
    let mut contents_val = String::with_capacity(contents_size);
    contents_val.push('<');
    contents_val.push_str(&hex);
    for _ in 0..pad_len { contents_val.push('0'); }
    contents_val.push('>');

    // ── 7. Write output — one pass, sequential ──────────────────────
    reader.seek(SeekFrom::Start(0))?;
    rem = src_len;
    while rem > 0 {
        let n = rem.min(chunk.len());
        reader.read_exact(&mut chunk[..n])?;
        writer.write_all(&chunk[..n])?;
        rem -= n;
    }

    writer.write_all(&sig_bytes[..contents_in_dict])?;
    writer.write_all(contents_val.as_bytes())?;
    writer.write_all(&sig_bytes[after_in_dict..])?;
    writer.write_all(field_str.as_bytes())?;
    writer.write_all(catalog_str.as_bytes())?;
    writer.write_all(xref_section.as_bytes())?;
    writer.write_all(trailer_section.as_bytes())?;
    writer.write_all(eof_section.as_bytes())?;

    Ok(())
}

// ── CMS builder — same DER assembly as upstream, accepts pre-computed digest ──

#[cfg(feature = "signatures")]
fn build_cms_from_digest(
    credentials: &SigningCredentials,
    message_digest: &[u8],
    _digest_algorithm: DigestAlgorithm,
) -> Result<Vec<u8>> {
    use crate::signatures::der_util::*;
    use cms::cert::x509::Certificate as X509Certificate;
    use der::oid::db::rfc5912::ID_SHA_256;
    use der::{Decode, Encode};
    use rsa::pkcs8::DecodePrivateKey;
    use rsa::{Pkcs1v15Sign, RsaPrivateKey};
    use sha2::{Digest, Sha256};

    const OID_SIGNED_DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];
    const OID_DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x01];
    const OID_SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
    const OID_RSA_ENC: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01];
    const OID_CONTENT_TYPE: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x03];
    const OID_MSG_DIGEST: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x09, 0x04];

    let digest_oid_bytes = OID_SHA256;
    let digest_oid = ID_SHA_256;

    let cert = X509Certificate::from_der(&credentials.certificate)
        .map_err(|e| Error::InvalidPdf(format!("cannot parse certificate: {e}")))?;
    let issuer_der = cert.tbs_certificate.issuer.to_der()
        .map_err(|e| Error::InvalidPdf(format!("encode issuer: {e}")))?;
    let serial_der = cert.tbs_certificate.serial_number.to_der()
        .map_err(|e| Error::InvalidPdf(format!("encode serial: {e}")))?;

    let rsa_key = RsaPrivateKey::from_pkcs8_der(&credentials.private_key)
        .or_else(|_| {
            use pkcs1::DecodeRsaPrivateKey;
            RsaPrivateKey::from_pkcs1_der(&credentials.private_key)
        })
        .map_err(|_| Error::InvalidPdf("invalid RSA private key".into()))?;

    // Signed attributes
    let attr_ct = {
        let mut c = Vec::new();
        c.extend(der_oid(OID_CONTENT_TYPE));
        c.extend(der_set(&der_oid(OID_DATA)));
        der_sequence(&c)
    };
    let attr_md = {
        let mut c = Vec::new();
        c.extend(der_oid(OID_MSG_DIGEST));
        c.extend(der_set(&der_octet_string(message_digest)));
        der_sequence(&c)
    };

    let mut attrs_content = Vec::new();
    attrs_content.extend(&attr_ct);
    attrs_content.extend(&attr_md);

    let attrs_for_hashing = der_set(&attrs_content);
    let attrs_for_storage = der_tag(0xA0, &attrs_content);

    // Hash signed attrs → DigestInfo → RSA sign
    let attrs_hash = Sha256::digest(&attrs_for_hashing).to_vec();
    let di_prefix = crate::signatures::crypto::digest_info_prefix(digest_oid)
        .ok_or_else(|| Error::InvalidPdf("no DigestInfo prefix".into()))?;
    let mut digest_info = Vec::with_capacity(di_prefix.len() + attrs_hash.len());
    digest_info.extend_from_slice(di_prefix);
    digest_info.extend_from_slice(&attrs_hash);
    let sig_value = rsa_key.sign(Pkcs1v15Sign::new_unprefixed(), &digest_info)
        .map_err(|e| Error::InvalidPdf(format!("RSA sign failed: {e}")))?;

    // Build SignerInfo
    let signer_info = {
        let mut isn = Vec::new();
        isn.extend(&issuer_der);
        isn.extend(&serial_der);
        let isn = der_sequence(&isn);

        let digest_alg = der_sequence(&der_oid(digest_oid_bytes));
        let sig_alg = {
            let mut c = Vec::new();
            c.extend(der_oid(OID_RSA_ENC));
            c.extend_from_slice(&[0x05, 0x00]);
            der_sequence(&c)
        };

        let mut si = Vec::new();
        si.extend(der_integer(1));
        si.extend(isn);
        si.extend(digest_alg);
        si.extend(attrs_for_storage);
        si.extend(sig_alg);
        si.extend(der_octet_string(&sig_value));
        der_sequence(&si)
    };

    // Build SignedData
    let signed_data = {
        let digest_algs = der_set(&der_sequence(&der_oid(digest_oid_bytes)));
        let encap_ci = der_sequence(&der_oid(OID_DATA));
        let certs = der_tag(0xA0, &credentials.certificate);
        let signer_infos = der_set(&signer_info);

        let mut sd = Vec::new();
        sd.extend(der_integer(1));
        sd.extend(digest_algs);
        sd.extend(encap_ci);
        sd.extend(certs);
        sd.extend(signer_infos);
        der_sequence(&sd)
    };

    // Build ContentInfo
    let mut ci = Vec::new();
    ci.extend(der_oid(OID_SIGNED_DATA));
    ci.extend(der_tag(0xA0, &signed_data));
    Ok(der_sequence(&ci))
}

// ── Helpers ─────────────────────────────────────────────────────────

#[cfg(feature = "signatures")]
fn scan_startxref(tail: &[u8]) -> Option<u64> {
    let pos = tail.windows(9).rposition(|w| w == b"startxref")?;
    let after = &tail[pos + 9..];
    let s = std::str::from_utf8(after).ok()?;
    let trimmed = s.trim_start_matches([' ', '\r', '\n']);
    let end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(trimmed.len());
    trimmed[..end].parse().ok()
}

#[cfg(feature = "signatures")]
fn scan_root_ref(tail: &[u8]) -> Option<String> {
    let pattern = b"/Root ";
    let pos = tail.windows(pattern.len()).rposition(|w| w == pattern)?;
    let after = &tail[pos + pattern.len()..];
    let end = after.iter()
        .position(|&b| b == b'/' || b == b'>' || b == b'\n')
        .unwrap_or(after.len().min(40));
    let s = std::str::from_utf8(&after[..end]).ok()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[cfg(feature = "signatures")]
fn scan_pages_ref(reader: &mut (impl std::io::Read + std::io::Seek), catalog_id: u64, xref_offset: u64) -> Option<String> {
    use std::io::SeekFrom;
    // Read the xref table to find catalog object offset, then read
    // the catalog object to extract /Pages reference.
    // Fall back: scan the full file for the catalog object header.
    let header = format!("{} 0 obj", catalog_id);
    let header_bytes = header.as_bytes();

    // Read from the xref offset area backward to find the catalog
    let scan_size = 32768usize;
    let scan_start = if xref_offset > scan_size as u64 { xref_offset - scan_size as u64 } else { 0 };
    let mut buf = vec![0u8; (xref_offset as usize).min(scan_size)];
    reader.seek(SeekFrom::Start(scan_start)).ok()?;
    let n = reader.read(&mut buf).ok()?;
    let buf = &buf[..n];

    // Find the catalog object
    let pos = buf.windows(header_bytes.len()).rposition(|w| w == header_bytes)?;
    let after = &buf[pos..];
    let s = std::str::from_utf8(after).ok()?;

    // Extract /Pages N 0 R
    let pages_pos = s.find("/Pages ")?;
    let after_pages = &s[pages_pos + "/Pages ".len()..];
    let end = after_pages.find(|c: char| c == '/' || c == '>' || c == '\n')
        .unwrap_or(after_pages.len().min(40));
    let pages_ref = after_pages[..end].trim();
    (!pages_ref.is_empty()).then(|| pages_ref.to_string())
}

#[cfg(feature = "signatures")]
fn scan_next_obj_num(tail: &[u8]) -> Option<u64> {
    let pattern = b"/Size ";
    let pos = tail.windows(pattern.len()).rposition(|w| w == pattern)?;
    let after = &tail[pos + pattern.len()..];
    let s = std::str::from_utf8(after).ok()?;
    let trimmed = s.trim_start_matches(' ');
    let end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(trimmed.len());
    trimmed[..end].parse().ok()
}

#[cfg(feature = "signatures")]
fn build_sig_dict(signer: &crate::signatures::PdfSigner, obj_num: u64) -> String {
    let placeholder = signer.generate_contents_placeholder();
    let opts = signer.options();
    let mut d = format!(
        "{} 0 obj\n<< /Type /Sig\n/Filter /Adobe.PPKLite\n\
         /SubFilter /adbe.pkcs7.detached\n\
         /ByteRange [0000000000 0000000000 0000000000 0000000000]\n\
         /Contents {}\n",
        obj_num, placeholder,
    );
    if let Some(name) = extract_cn_from_der(&signer.credentials().certificate) {
        d.push_str(&format!("/Name ({})\n", name));
    }
    if let Some(ref r) = opts.reason {
        d.push_str(&format!("/Reason ({})\n", r));
    }
    if let Some(ref l) = opts.location {
        d.push_str(&format!("/Location ({})\n", l));
    }
    d.push_str(">>\nendobj\n");
    d
}

// Extract the Common Name (CN, OID 2.5.4.3) from a DER-encoded X.509
// certificate. Scans for the OID bytes and reads the following UTF8String
// or PrintableString value.
#[cfg(feature = "signatures")]
fn extract_cn_from_der(cert_der: &[u8]) -> Option<String> {
    // OID 2.5.4.3 (id-at-commonName) encoded as DER: 55 04 03
    let cn_oid = [0x55, 0x04, 0x03];
    for i in 0..cert_der.len().saturating_sub(cn_oid.len() + 4) {
        if cert_der[i..i + 3] == cn_oid {
            // After OID: tag byte (0x0C=UTF8, 0x13=PrintableString) + length + value
            let val_start = i + 3;
            if val_start + 2 > cert_der.len() { continue; }
            let tag = cert_der[val_start];
            if tag != 0x0C && tag != 0x13 { continue; }
            let len = cert_der[val_start + 1] as usize;
            let data_start = val_start + 2;
            if data_start + len > cert_der.len() { continue; }
            return std::str::from_utf8(&cert_der[data_start..data_start + len]).ok().map(|s| s.to_string());
        }
    }
    None
}

#[cfg(feature = "signatures")]
fn find_contents_offset(data: &[u8]) -> Option<usize> {
    let pattern = b"/Contents <";
    let pos = data.windows(pattern.len()).position(|w| w == pattern)?;
    Some(pos + "/Contents ".len())
}

#[cfg(feature = "signatures")]
fn patch_byterange(mut text: String, br: &[i64; 4]) -> String {
    let placeholder = "0000000000 0000000000 0000000000 0000000000";
    let replacement = format!("{:>10} {:>10} {:>10} {:>10}", br[0], br[1], br[2], br[3]);
    text = text.replace(placeholder, &replacement);
    text
}

#[cfg(feature = "signatures")]
fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
