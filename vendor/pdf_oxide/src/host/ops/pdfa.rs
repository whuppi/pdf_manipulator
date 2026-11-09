// PDF/A + PDF/UA validation ops (the pdfa capability). The dispatch fns
// answer a typed not-enabled error when the pdfa feature is off.

use crate::host::binary_codec::ResponseWriter;
use crate::host::dispatch;
use crate::host::ops::op_unit;
use crate::host::ops::support::handle_with_doc;

op_unit!(VALIDATE_PDF_A, "validatePdfA", pdf_op_validate_pdf_a_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, req| {
        let name = req.get_str("level").unwrap_or("a2");
        let level = dispatch::pdfa_level_from_name(name).ok_or_else(|| {
            crate::error::Error::InvalidOperation(format!("unknown PDF/A level: {name}"))
        })?;
        let result = dispatch::validate_pdf_a(doc, level, ctx.cancel)?;
        let mut w = ResponseWriter::ok();
        w.put_bool("compliant", result.compliant);
        w.put_i32("errors", result.errors);
        w.put_i32("warnings", result.warnings);
        Ok(w.finish())
    })
});

op_unit!(
    VALIDATE_PDF_UA,
    "validatePdfUa",
    pdf_op_validate_pdf_ua_anchor,
    |ctx| {
        handle_with_doc(ctx.state, ctx.req, |doc, req| {
            let name = req.get_str("level").unwrap_or("ua1");
            let level = dispatch::pdfua_level_from_name(name).ok_or_else(|| {
                crate::error::Error::InvalidOperation(format!("unknown PDF/UA level: {name}"))
            })?;
            let result = dispatch::validate_pdf_ua(doc, level, ctx.cancel)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("compliant", result.compliant);
            w.put_i32("errors", result.errors);
            w.put_i32("warnings", result.warnings);
            Ok(w.finish())
        })
    }
);
