// Runtime fallback-font registration (CJK / emoji form baking).
// Core — always built; the fonts themselves arrive at runtime.

use crate::host::binary_codec::{Request, ResponseWriter};
use crate::host::bridge_api::BoxedReader;
use crate::host::ops::op_unit;
use crate::host::ops::support::read_all_from_reader;

op_unit!(
    REGISTER_FALLBACK_FONT,
    "registerFallbackFont",
    pdf_op_register_fallback_font_anchor,
    |ctx| {
        let source = ctx.take_source(0);
        handle_register_fallback_font(ctx.req, ctx.source_bytes, source)
    }
);

fn handle_register_fallback_font(
    req: &Request<'_>,
    source_bytes: Option<&[u8]>,
    source_reader: Option<BoxedReader>,
) -> Vec<u8> {
    let kind = req.get_str("kind").unwrap_or("");
    let bytes = if let Some(b) = source_bytes {
        b.to_vec()
    } else if let Some(mut reader) = source_reader {
        match read_all_from_reader(&mut reader) {
            Ok(buf) => buf,
            Err(e) => {
                return ResponseWriter::error(&format!("reading fallback font source: {e}"))
            },
        }
    } else {
        return ResponseWriter::error("no source for registerFallbackFont");
    };
    match crate::host::dispatch::register_fallback_font(kind, bytes) {
        Ok(()) => ResponseWriter::ok().finish(),
        Err(e) => ResponseWriter::error_of(&e),
    }
}
