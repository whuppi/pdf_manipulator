//! Signs a PDF by appending one update section through the shared
//! appender ([`super::incremental`]): a signature dictionary, its widget
//! on page 0, and the form's AcroForm and the page's /Annots, so every
//! other catalog entry and earlier signature survives the update.

#[cfg(feature = "signatures")]
use crate::error::{Error, Result};

#[cfg(feature = "signatures")]
use crate::signatures::{DigestAlgorithm, SignOptions, SigningCredentials};

/// Sign a PDF with O(1)-memory I/O: the source is opened as a
/// [`crate::document::PdfDocument`] and copied to `writer` through a
/// read-buffer-sized chunk, then one update section.
///
/// The reader is consumed — the document keeps it for every source read.
/// Takes concrete `BoxedReader` and `BoxedWriter` from bridge_api.rs,
/// the same wrapper pattern used for editor save.
#[cfg(feature = "signatures")]
pub(crate) fn sign_pdf(
    reader: super::bridge_api::BoxedReader,
    writer: &mut super::bridge_api::BoxedWriter,
    credentials: &SigningCredentials,
    opts: SignOptions,
    signed_at_unix_secs: u64,
    cancel: &crate::cancel::CancelCheck,
) -> Result<()> {
    use crate::document::PdfDocument;
    use crate::host::constants::READ_BUF_CAPACITY;
    use crate::host::incremental::{build_update, doc_mdp_level};
    use crate::object::{Object, ObjectRef};
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::io::Write;

    let doc = PdfDocument::from_external_reader(reader.0)?;
    let signer = crate::signatures::PdfSigner::new(credentials.clone(), opts);
    let contents_size = signer.placeholder_size();

    // Refusals, decided before the first byte is written.
    if doc.is_encrypted() {
        return Err(Error::Unsupported(
            "sign: an encrypted source is not supported".into(),
        ));
    }
    if doc.xref_repaired() {
        return Err(Error::Unsupported(
            "sign: the source's cross-reference was repaired; save it with a full rewrite first"
                .into(),
        ));
    }
    if doc_mdp_level(&doc) == Some(1) {
        return Err(Error::Unsupported(
            "sign: the document is certified against any change".into(),
        ));
    }

    // Ids: the first free id is the trailer's /Size.
    let next = doc
        .trailer()
        .as_dict()
        .and_then(|d| d.get("Size"))
        .and_then(|s| s.as_integer())
        .and_then(|s| u32::try_from(s.max(0)).ok())
        .ok_or_else(|| Error::InvalidPdf("sign: the source's trailer has no /Size".into()))?;
    let sig_id = next;
    let widget_id = next + 1;
    let widget_ref = ObjectRef::new(widget_id, 0);

    // The signature dictionary, written by hand so the fixed-width
    // /ByteRange and /Contents placeholders stay patchable in place.
    let sig_dict = build_sig_dict(&signer, sig_id, signed_at_unix_secs);
    let contents_in_dict = find_contents_offset(sig_dict.as_bytes())
        .ok_or_else(|| Error::InvalidPdf("sign: cannot find the /Contents placeholder".into()))?;

    let page_ref = doc.get_page_ref(0)?;
    let mut objects: Vec<(u32, Vec<u8>)> = Vec::new();

    // The widget: a zero-area invisible signature field annotation.
    let field_name = next_field_name(&doc)?;
    objects.push((
        widget_id,
        format!(
            "{widget_id} 0 obj\n<< /Type /Annot /Subtype /Widget /FT /Sig \
             /T ({field_name}) /V {sig_id} 0 R /Rect [0 0 0 0] /F 132 /P {page_ref} >>\nendobj\n"
        )
        .into_bytes(),
    ));

    // The widget joins page 0's /Annots: absent or a direct array, the
    // page object is rewritten with the new array; an indirect one is
    // rewritten on its own and the page untouched.
    let page_dict = doc
        .load_object(page_ref)?
        .as_dict()
        .cloned()
        .ok_or_else(|| Error::InvalidPdf("sign: the source's page 0 is not a dictionary".into()))?;
    let direct_annots = match page_dict.get("Annots") {
        None | Some(Object::Array(_)) => {
            let mut items = page_dict
                .get("Annots")
                .and_then(Object::as_array)
                .cloned()
                .unwrap_or_default();
            items.push(Object::Reference(widget_ref));
            Some(items)
        }
        Some(Object::Reference(r)) => {
            let arr = doc.load_object(*r)?;
            let mut items = arr
                .as_array()
                .cloned()
                .ok_or_else(|| {
                    Error::InvalidPdf("sign: the page's /Annots is not an array".into())
                })?;
            items.push(Object::Reference(widget_ref));
            push_rewrite(&doc, &mut objects, r.id, Object::Array(items))?;
            None
        }
        Some(_) => {
            return Err(Error::InvalidPdf(
                "sign: the page's /Annots is not an array".into(),
            ))
        }
    };
    if let Some(items) = direct_annots {
        let mut page = page_dict;
        page.insert("Annots".into(), Object::Array(items));
        push_rewrite(&doc, &mut objects, page_ref.id, Object::Dictionary(page))?;
    }

    // The form gains the field: a missing AcroForm is added to a copy of
    // the catalog, a direct one is rewritten in that copy, an indirect
    // one is rewritten in place and the catalog untouched.
    let root_ref = doc
        .trailer()
        .as_dict()
        .and_then(|d| d.get("Root"))
        .and_then(Object::as_reference)
        .ok_or_else(|| Error::InvalidPdf("sign: the source's trailer has no /Root".into()))?;
    let catalog_dict = doc
        .catalog()?
        .as_dict()
        .cloned()
        .ok_or_else(|| Error::InvalidPdf("sign: the source's catalog is not a dictionary".into()))?;
    match catalog_dict.get("AcroForm").cloned() {
        None => {
            let mut catalog = catalog_dict;
            catalog.insert(
                "AcroForm".into(),
                Object::Dictionary(HashMap::from([
                    (
                        "Fields".to_string(),
                        Object::Array(vec![Object::Reference(widget_ref)]),
                    ),
                    ("SigFlags".to_string(), Object::Integer(3)),
                ])),
            );
            push_rewrite(&doc, &mut objects, root_ref.id, Object::Dictionary(catalog))?;
        }
        Some(Object::Dictionary(acro)) => {
            let acro = acroform_with_field(&doc, acro, widget_ref, &mut objects)?;
            let mut catalog = catalog_dict;
            catalog.insert("AcroForm".into(), Object::Dictionary(acro));
            push_rewrite(&doc, &mut objects, root_ref.id, Object::Dictionary(catalog))?;
        }
        Some(Object::Reference(r)) => {
            let acro_obj = doc.load_object(r)?;
            let acro = acro_obj
                .as_dict()
                .cloned()
                .ok_or_else(|| {
                    Error::InvalidPdf("sign: the source's AcroForm is not a dictionary".into())
                })?;
            let acro = acroform_with_field(&doc, acro, widget_ref, &mut objects)?;
            push_rewrite(&doc, &mut objects, r.id, Object::Dictionary(acro))?;
        }
        Some(_) => {
            return Err(Error::InvalidPdf(
                "sign: the source's AcroForm is not a dictionary".into(),
            ))
        }
    }

    objects.push((sig_id, sig_dict.into_bytes()));
    objects.sort_by_key(|(id, _)| *id);
    let i_sig = objects
        .iter()
        .position(|(id, _)| *id == sig_id)
        .ok_or_else(|| Error::InvalidPdf("sign: the signature object is missing".into()))?;

    let mut section = build_update(&doc, &objects, None, widget_id + 1)?;

    // The digest covers the source, the newline and every section byte
    // outside the /Contents gap; the /ByteRange names exactly that.
    let contents_off = section.object_offsets[i_sig] + contents_in_dict;
    let gap_start = section.base + contents_off as u64;
    let gap_end = gap_start + contents_size as u64;
    let total = section.base + section.bytes.len() as u64;
    let byte_range: [i64; 4] = [
        0,
        gap_start as i64,
        gap_end as i64,
        (total - gap_end) as i64,
    ];
    if !patch_byterange(
        &mut section.bytes,
        section.object_offsets[i_sig],
        &byte_range,
    ) {
        return Err(Error::InvalidPdf(
            "sign: cannot find the /ByteRange placeholder".into(),
        ));
    }

    let len = doc.source_len()?;
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; READ_BUF_CAPACITY];
    let mut at = 0u64;
    while at < len {
        cancel.check()?;
        let n = doc.read_source_at(at, &mut chunk)?;
        if n == 0 {
            return Err(Error::InvalidPdf("sign: source ended before its length".into()));
        }
        hasher.update(&chunk[..n]);
        at += n as u64;
    }
    if section.base > len {
        hasher.update(b"\n");
    }
    hasher.update(&section.bytes[..contents_off]);
    hasher.update(&section.bytes[contents_off + contents_size..]);

    let message_digest = hasher.finalize().to_vec();
    let cms_der = build_cms_from_digest(credentials, &message_digest, DigestAlgorithm::Sha256)?;

    let hex = hex_encode(&cms_der);
    let pad_len = (contents_size - 2) - hex.len();
    let mut contents_val = String::with_capacity(contents_size);
    contents_val.push('<');
    contents_val.push_str(&hex);
    for _ in 0..pad_len {
        contents_val.push('0');
    }
    contents_val.push('>');
    section.bytes[contents_off..contents_off + contents_size]
        .copy_from_slice(contents_val.as_bytes());

    // Write output: the source, then the section.
    let mut at = 0u64;
    while at < len {
        cancel.check()?;
        let n = (len - at).min(chunk.len() as u64) as usize;
        doc.read_source_at(at, &mut chunk[..n])?;
        writer.write_all(&chunk[..n])?;
        at += n as u64;
    }
    if section.base > len {
        writer.write_all(b"\n")?;
    }
    writer.write_all(&section.bytes)?;
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
    let issuer_der = cert.tbs_certificate().issuer().to_der()
        .map_err(|e| Error::InvalidPdf(format!("encode issuer: {e}")))?;
    let serial_der = cert.tbs_certificate().serial_number().to_der()
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
fn push_rewrite(
    doc: &crate::document::PdfDocument,
    objects: &mut Vec<(u32, Vec<u8>)>,
    id: u32,
    obj: crate::object::Object,
) -> Result<()> {
    use crate::writer::ObjectSerializer;
    let gen = doc.object_generation(id).unwrap_or(0);
    let bytes = ObjectSerializer::compact().serialize_indirect(id, gen, &obj);
    objects.push((id, bytes));
    Ok(())
}

/// Adds the widget to an AcroForm dictionary: /Fields gains the widget
/// (an indirect /Fields array is rewritten as its own object and the
/// reference kept) and /SigFlags keeps its other bits.
#[cfg(feature = "signatures")]
fn acroform_with_field(
    doc: &crate::document::PdfDocument,
    mut acro: std::collections::HashMap<String, crate::object::Object>,
    widget_ref: crate::object::ObjectRef,
    objects: &mut Vec<(u32, Vec<u8>)>,
) -> Result<std::collections::HashMap<String, crate::object::Object>> {
    use crate::object::Object;
    match acro.get("Fields").cloned() {
        Some(Object::Array(mut items)) => {
            items.push(Object::Reference(widget_ref));
            acro.insert("Fields".into(), Object::Array(items));
        }
        Some(Object::Reference(r)) => {
            let arr = doc.load_object(r)?;
            let mut items = arr
                .as_array()
                .cloned()
                .ok_or_else(|| Error::InvalidPdf("sign: the form's /Fields is not an array".into()))?;
            items.push(Object::Reference(widget_ref));
            push_rewrite(doc, objects, r.id, Object::Array(items))?;
        }
        _ => {
            acro.insert(
                "Fields".into(),
                Object::Array(vec![Object::Reference(widget_ref)]),
            );
        }
    }
    let flags = acro
        .get("SigFlags")
        .and_then(Object::as_integer)
        .unwrap_or(0);
    acro.insert("SigFlags".into(), Object::Integer(flags | 3));
    Ok(acro)
}

/// `SigN`, the smallest N >= 1 that is not the /T of any top-level
/// entry of the form's /Fields.
#[cfg(feature = "signatures")]
fn next_field_name(doc: &crate::document::PdfDocument) -> Result<String> {
    use crate::object::Object;
    let fields = doc
        .catalog()
        .ok()
        .and_then(|c| c.as_dict().and_then(|d| d.get("AcroForm")).cloned())
        .map(|acro| match acro {
            Object::Reference(r) => doc.load_object(r),
            other => Ok(other),
        })
        .transpose()?
        .and_then(|acro| acro.as_dict().and_then(|d| d.get("Fields")).cloned())
        .map(|fields| match fields {
            Object::Reference(r) => doc.load_object(r),
            other => Ok(other),
        })
        .transpose()?;
    let taken: Vec<String> = fields
        .and_then(|f| f.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|f| match f {
            Object::Reference(r) => doc.load_object(*r).ok(),
            other => Some(other.clone()),
        })
        .filter_map(|f| f.as_dict().and_then(|d| d.get("T")).cloned())
        .filter_map(|t| match t {
            Object::String(bytes) => String::from_utf8(bytes).ok(),
            _ => None,
        })
        .collect();
    let mut n = 1;
    while taken.contains(&format!("Sig{n}")) {
        n += 1;
    }
    Ok(format!("Sig{n}"))
}

#[cfg(feature = "signatures")]
fn build_sig_dict(signer: &crate::signatures::PdfSigner, obj_num: u32, signed_at_unix_secs: u64) -> String {
    let placeholder = signer.generate_contents_placeholder();
    let opts = signer.options();
    let mut d = format!(
        "{obj_num} 0 obj\n<< /Type /Sig\n/Filter /Adobe.PPKLite\n\
         /SubFilter /adbe.pkcs7.detached\n\
         /ByteRange [0000000000 0000000000 0000000000 0000000000]\n\
         /Contents {placeholder}\n\
         /M ({})\n",
        crate::signatures::pdf_date_from_unix_secs(signed_at_unix_secs),
    );
    if let Some(name) = extract_cn_from_der(&signer.credentials().certificate) {
        d.push_str(&format!("/Name {}\n", pdf_literal(&name)));
    }
    if let Some(ref r) = opts.reason {
        d.push_str(&format!("/Reason {}\n", pdf_literal(r)));
    }
    if let Some(ref l) = opts.location {
        d.push_str(&format!("/Location {}\n", pdf_literal(l)));
    }
    d.push_str(">>\nendobj\n");
    d
}

/// A PDF text string literal (ISO 32000-1 §7.9.2.2): `\`, `(` and `)`
/// escaped inside `( … )`; text with any non-ASCII character written as
/// UTF-16BE with a BOM in hex form.
#[cfg(feature = "signatures")]
fn pdf_literal(s: &str) -> String {
    if s.is_ascii() {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('(');
        for c in s.chars() {
            match c {
                '\\' | '(' | ')' => {
                    out.push('\\');
                    out.push(c);
                }
                _ => out.push(c),
            }
        }
        out.push(')');
        out
    } else {
        let units: Vec<u16> = std::iter::once(0xFEFF)
            .chain(s.encode_utf16())
            .collect();
        let hex: String = units.iter().map(|u| format!("{u:04X}")).collect();
        format!("<{hex}>")
    }
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

/// Writes `br` over the fixed-width /ByteRange placeholder in `bytes`,
/// searched from `from` — same width, so no later offset moves. Returns
/// false when the placeholder is not there.
#[cfg(feature = "signatures")]
fn patch_byterange(bytes: &mut [u8], from: usize, br: &[i64; 4]) -> bool {
    let placeholder = b"0000000000 0000000000 0000000000 0000000000";
    let replacement = format!("{:>10} {:>10} {:>10} {:>10}", br[0], br[1], br[2], br[3]);
    let Some(at) = bytes[from..]
        .windows(placeholder.len())
        .position(|w| w == placeholder)
    else {
        return false;
    };
    let at = from + at;
    bytes[at..at + placeholder.len()].copy_from_slice(replacement.as_bytes());
    true
}

#[cfg(feature = "signatures")]
fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(all(test, feature = "signatures"))]
mod tests {
    use super::pdf_literal;

    #[test]
    fn pdf_literal_plain_text_stays_a_plain_string() {
        assert_eq!(pdf_literal("signed in Zurich"), "(signed in Zurich)");
    }

    #[test]
    fn pdf_literal_escapes_parens_and_backslash() {
        assert_eq!(pdf_literal("a (b) \\ c"), "(a \\(b\\) \\\\ c)");
    }

    #[test]
    fn pdf_literal_writes_non_ascii_text_as_utf16be_hex() {
        assert_eq!(pdf_literal("Zürich"), "<FEFF005A00FC0072006900630068>");
    }

    #[test]
    fn pdf_literal_escapes_inside_a_mixed_string() {
        // One non-ASCII character switches the whole literal to hex form,
        // where `(` and `)` need no escaping.
        assert_eq!(pdf_literal("Zü(ch)"), "<FEFF005A00FC0028006300680029>");
    }
}
