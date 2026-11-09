// Standalone conversion ops — source in, sink out, no handle.
//
// PDF ↔ office is the office capability: those handlers answer a typed
// not-enabled error when the office feature is off. XFA → AcroForm is
// core and always built.

use crate::host::binary_codec::{Request, ResponseWriter};
use crate::host::bridge_api::{BoxedReader, BoxedWriter};
use crate::host::dispatch;
use crate::host::engine_state::EngineState;
use crate::host::ops::op_unit;
use crate::host::ops::support::{
    authenticate_or_refuse, ok_flag, open_source, req_handle,
};

use crate::document::PdfDocument;

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

fn handle_convert_to(
    state: &mut EngineState,
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let format = req.get_str("format").unwrap_or("docx");

    // Try handle-based path first (if caller opened the doc already).
    let hid = req_handle(req);
    if hid > 0 {
        let docs = &mut state.documents;
        if let Some(doc) = docs.get_mut(&hid) {
            return convert_to_with_doc(doc, format, sink_writer);
        }
    }

    // One-shot path: open from source, convert, no handle needed.
    // O(1)-memory via BoxedReader when available.
    let mut doc = match open_source(source_bytes, source_reader, "convertTo") {
        Ok(d) => d,
        Err(resp) => return resp,
    };

    if let Some(resp) = authenticate_or_refuse(&doc, req.get_str("password")) {
        return resp;
    }

    convert_to_with_doc(&mut doc, format, sink_writer)
}

fn convert_to_with_doc(
    doc: &mut PdfDocument,
    format: &str,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    if let Some(mut writer) = sink_writer {
        match dispatch::convert_to_format_writer(doc, format, &mut writer) {
            Ok(()) => {
                ok_flag("streamed")
            }
            Err(e) => ResponseWriter::error_of(&e),
        }
    } else {
        match dispatch::convert_to_format(doc, format) {
            Ok(bytes) => {
                let mut w = ResponseWriter::ok();
                w.put_bytes("data", &bytes);
                w.finish()
            }
            Err(e) => ResponseWriter::error_of(&e),
        }
    }
}

/// Convert an XFA document to a plain AcroForm document. One-shot: the
/// source is opened here, converted, and streamed to the sink.
fn handle_convert_xfa_to_acroform(
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let mut doc = match open_source(source_bytes, source_reader, "convertXfaToAcroForm") {
        Ok(d) => d,
        Err(resp) => return resp,
    };
    if let Some(resp) = authenticate_or_refuse(&doc, req.get_str("password")) {
        return resp;
    }
    let Some(mut writer) = sink_writer else {
        return ResponseWriter::error("convertXfaToAcroForm requires a sink");
    };
    match dispatch::convert_xfa_to_acroform(&mut doc, &mut writer) {
        Ok(()) => ok_flag("streamed"),
        Err(e) => ResponseWriter::error_of(&e),
    }
}

fn handle_convert_to_pdf(
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
    sink_writer: Option<BoxedWriter>,
) -> Vec<u8> {
    let format = req.get_str("format").unwrap_or("docx");

    let Some(mut writer) = sink_writer else {
        return ResponseWriter::error("convertToPdf requires a sink");
    };
    if let Some(reader) = source_reader {
        match dispatch::convert_from_format_writer(reader, format, &mut writer) {
            Ok(()) => ok_flag("streamed"),
            Err(e) => ResponseWriter::error_of(&e),
        }
    } else if let Some(bytes) = source_bytes {
        match dispatch::convert_from_format_writer(std::io::Cursor::new(bytes.to_vec()), format, &mut writer) {
            Ok(()) => ok_flag("streamed"),
            Err(e) => ResponseWriter::error_of(&e),
        }
    } else {
        ResponseWriter::error("no source for convertToPdf")
    }
}
