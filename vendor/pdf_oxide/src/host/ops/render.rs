// Rasterization ops (the render capability): page rendering and image
// extraction, both streamed through sinks[0].

use crate::host::bridge_api::{handle_extract_images_streamed, handle_render_streamed};
use crate::host::ops::op_unit;

op_unit!(RENDER, "render", pdf_op_render_anchor, |ctx| {
    let sink = ctx.take_sink(0);
    handle_render_streamed(ctx.state, ctx.req, sink)
});

op_unit!(
    EXTRACT_IMAGES,
    "extractImages",
    pdf_op_extract_images_anchor,
    |ctx| {
        let sink = ctx.take_sink(0);
        handle_extract_images_streamed(ctx.state, ctx.req, sink)
    }
);
