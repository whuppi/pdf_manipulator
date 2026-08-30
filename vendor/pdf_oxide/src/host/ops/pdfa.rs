// PDF/A + PDF/UA validation ops (the pdfa capability). The dispatch fns
// answer a typed not-enabled error when the pdfa feature is off.

use crate::host::binary_codec::ResponseWriter;
use crate::host::bridge_api::handle_with_doc;
use crate::host::dispatch;
use crate::host::ops::op_unit;

op_unit!(VALIDATE_PDF_A, "validatePdfA", pdf_op_validate_pdf_a_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, req| {
        let level = req.get_i32("level").unwrap_or(2);
        let result = dispatch::validate_pdf_a(doc, level)?;
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
            let level = req.get_i32("level").unwrap_or(1);
            let result = dispatch::validate_pdf_ua(doc, level)?;
            let mut w = ResponseWriter::ok();
            w.put_bool("valid", result);
            Ok(w.finish())
        })
    }
);
