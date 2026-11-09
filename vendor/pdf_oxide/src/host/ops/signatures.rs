// Digital-signature ops (the signatures capability): signing plus
// signature reads on an open document.

use crate::host::binary_codec::{ErrorKind, Request, ResponseWriter};
use crate::host::bridge_api::{BoxedReader, BoxedWriter};
use crate::host::dispatch;
use crate::host::ops::op_unit;
use crate::host::ops::support::handle_with_doc;

op_unit!(SIGN, "sign", pdf_op_sign_anchor, |ctx| {
    let source = ctx.take_source(0);
    let sink = ctx.take_sink(0);
    handle_sign(ctx.req, source, ctx.source_bytes, sink, ctx.cancel)
});

op_unit!(
    GET_SIGNATURES,
    "getSignatures",
    pdf_op_get_signatures_anchor,
    |ctx| {
        handle_with_doc(ctx.state, ctx.req, |doc, _| {
            let result = dispatch::get_signatures(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_map_list("signatures", result.signatures.len(), |i, item| {
                let s = &result.signatures[i];
                item.put_str("signerName", &s.signer_name);
                item.put_str("reason", &s.reason);
                item.put_str("location", &s.location);
                item.put_bool("isValid", s.is_valid);
                item.put_bool("coversWholeDocument", s.covers_whole_document);
                if let Some(secs) = s.signing_time {
                    item.put_i64("signingTime", secs);
                }
            });
            Ok(w.finish())
        })
    }
);

op_unit!(
    VERIFY_SIGNATURES,
    "verifySignatures",
    pdf_op_verify_signatures_anchor,
    |ctx| {
        handle_with_doc(ctx.state, ctx.req, |doc, _| {
            let result = dispatch::verify_signatures(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("valid", result);
            Ok(w.finish())
        })
    }
);

#[cfg(feature = "signatures")]
fn handle_sign(
    req: &Request<'_>,
    source_reader: Option<BoxedReader>,
    source_bytes: Option<&[u8]>,
    sink_writer: Option<BoxedWriter>,
    cancel: &crate::cancel::CancelCheck,
) -> Vec<u8> {
    use crate::host::ops::support::ok_flag;
    use crate::signatures::{SignOptions, SigningCredentials};

    let credentials = if let (Some(cert), Some(pw)) = (req.get_bytes("certificate"), req.get_str("certificatePassword")) {
        SigningCredentials::from_pkcs12(cert, pw)
    } else if let (Some(cert_pem), Some(key_pem)) = (req.get_str("certPem"), req.get_str("keyPem")) {
        SigningCredentials::from_pem(cert_pem, key_pem)
    } else {
        return ResponseWriter::error("sign: missing credentials (certificate+password or certPem+keyPem)");
    };

    let credentials = match credentials {
        Ok(c) => c,
        Err(e) => {
            return ResponseWriter::error_kind(
                ErrorKind::Crypto,
                &format!("sign: invalid credentials: {e}"),
            )
        }
    };

    let opts = SignOptions {
        reason: req.get_str("reason").map(|s| s.to_string()),
        location: req.get_str("location").map(|s| s.to_string()),
        ..Default::default()
    };

    if let (Some(reader), Some(mut writer)) = (source_reader, sink_writer) {
        let Some(signed_at) = req.get_i64("signedAt").and_then(|s| u64::try_from(s).ok()) else {
            return ResponseWriter::error_kind(ErrorKind::InvalidArgument, "sign: missing signedAt");
        };
        match crate::host::sign::sign_pdf(reader, &mut writer, &credentials, opts, signed_at, cancel) {
            Ok(()) => {
                ok_flag("signed")
            }
            Err(e) => ResponseWriter::error_of(&e),
        }
    } else if let Some(bytes) = source_bytes {
        match crate::signatures::sign_pdf_bytes(bytes, &credentials, opts) {
            Ok(signed) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &signed);
                w.finish()
            }
            Err(e) => ResponseWriter::error_of(&e),
        }
    } else {
        ResponseWriter::error("sign: no source provided")
    }
}

#[cfg(not(feature = "signatures"))]
fn handle_sign(
    _req: &Request<'_>,
    _source_reader: Option<BoxedReader>,
    _source_bytes: Option<&[u8]>,
    _sink_writer: Option<BoxedWriter>,
    _cancel: &crate::cancel::CancelCheck,
) -> Vec<u8> {
    ResponseWriter::error_kind(ErrorKind::Unsupported, "signatures feature not enabled")
}
