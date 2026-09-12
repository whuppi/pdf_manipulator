// Builder-handle ops: create pages from scratch. Core — always built.

use crate::host::bridge_api::{
    handle_builder_add_page, handle_builder_create, handle_builder_page_op,
    handle_builder_save, handle_builder_set_metadata, ok_flag, req_handle,
};
use crate::host::dispatch;
use crate::host::ops::op_unit;

op_unit!(CREATE, "builderCreate", pdf_op_builder_create_anchor, |ctx| {
    handle_builder_create(ctx.state)
});

op_unit!(DISPOSE, "builderDispose", pdf_op_builder_dispose_anchor, |ctx| {
    let hid = req_handle(ctx.req);
    ctx.state.builders.remove(&hid);
    ctx.state.page_ops.remove(&hid);
    ok_flag("disposed")
});

op_unit!(
    SET_METADATA,
    "builderSetMetadata",
    pdf_op_builder_set_metadata_anchor,
    |ctx| handle_builder_set_metadata(ctx.state, ctx.req)
);

op_unit!(
    ADD_PAGE,
    "builderAddPage",
    pdf_op_builder_add_page_anchor,
    |ctx| handle_builder_add_page(ctx.state, ctx.req)
);

op_unit!(PAGE_OP, "builderPageOp", pdf_op_builder_page_op_anchor, |ctx| {
    let source = ctx.take_source(0);
    handle_builder_page_op(ctx.state, ctx.req, source)
});

op_unit!(
    PAGE_DONE,
    "builderPageDone",
    pdf_op_builder_page_done_anchor,
    |ctx| {
        ctx.state
            .page_ops
            .entry(req_handle(ctx.req))
            .or_default()
            .push(dispatch::PageOp::Done);
        ok_flag("done")
    }
);

op_unit!(SAVE, "builderSave", pdf_op_builder_save_anchor, |ctx| {
    let sink = ctx.take_sink(0);
    handle_builder_save(ctx.state, ctx.req, sink)
});
