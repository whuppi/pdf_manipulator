// Runtime fallback-font registration (CJK / emoji form baking).
// Core — always built; the fonts themselves arrive at runtime.

use crate::host::bridge_api::handle_register_fallback_font;
use crate::host::ops::op_unit;

op_unit!(
    REGISTER_FALLBACK_FONT,
    "registerFallbackFont",
    pdf_op_register_fallback_font_anchor,
    |ctx| {
        let source = ctx.take_source(0);
        handle_register_fallback_font(ctx.req, ctx.source_bytes, source)
    }
);
