// Standalone conversion ops — source in, sink out, no handle.
//
// PDF ↔ office is the office capability: those handlers answer a typed
// not-enabled error when the office feature is off. XFA → AcroForm is
// core and always built.

use crate::host::bridge_api::{
    handle_convert_to, handle_convert_to_pdf, handle_convert_xfa_to_acroform,
};
use crate::host::ops::op_unit;

op_unit!(CONVERT_TO, "convertTo", pdf_op_convert_to_anchor, |ctx| {
    let source = ctx.take_source(0);
    let sink = ctx.take_sink(0);
    handle_convert_to(ctx.state, ctx.req, ctx.source_bytes, source, sink)
});

op_unit!(
    CONVERT_TO_PDF,
    "convertToPdf",
    pdf_op_convert_to_pdf_anchor,
    |ctx| {
        let source = ctx.take_source(0);
        let sink = ctx.take_sink(0);
        handle_convert_to_pdf(ctx.req, ctx.source_bytes, source, sink)
    }
);

op_unit!(
    CONVERT_XFA_TO_ACROFORM,
    "convertXfaToAcroForm",
    pdf_op_convert_xfa_to_acroform_anchor,
    |ctx| {
        let source = ctx.take_source(0);
        let sink = ctx.take_sink(0);
        handle_convert_xfa_to_acroform(ctx.req, ctx.source_bytes, source, sink)
    }
);
