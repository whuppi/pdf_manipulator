// Editor-handle ops: lifecycle, metadata reads, mutation, merge, save.
// Core — always built.

use crate::host::binary_codec::ResponseWriter;
use crate::host::bridge_api::{
    handle_editor_extract_pages, handle_editor_merge_from, handle_editor_mutate,
    handle_editor_open, handle_editor_save, handle_with_editor, ok_flag, req_handle,
};
use crate::host::dispatch;
use crate::host::ops::op_unit;

op_unit!(OPEN, "editorOpen", pdf_op_editor_open_anchor, |ctx| {
    let source = ctx.take_source(0);
    handle_editor_open(ctx.state, ctx.req, ctx.source_bytes, source)
});

op_unit!(DISPOSE, "editorDispose", pdf_op_editor_dispose_anchor, |ctx| {
    ctx.state.editors.remove(&req_handle(ctx.req));
    ok_flag("disposed")
});

op_unit!(
    GET_METADATA,
    "editorGetMetadata",
    pdf_op_editor_get_metadata_anchor,
    |ctx| {
        handle_with_editor(ctx.state, ctx.req, |editor, _| {
            let m = dispatch::edit_get_metadata(editor);
            let mut w = ResponseWriter::ok();
            w.put_i32("pageCount", m.page_count as i32);
            w.put_str("version", &format!("{}.{}", m.version_major, m.version_minor));
            w.put_str("title", &m.title);
            w.put_str("author", &m.author);
            w.put_str("subject", &m.subject);
            w.put_str("keywords", &m.keywords);
            w.put_str("producer", &m.producer);
            w.put_str("creationDate", &m.creation_date);
            Ok(w.finish())
        })
    }
);

op_unit!(
    IS_MODIFIED,
    "editorIsModified",
    pdf_op_editor_is_modified_anchor,
    |ctx| {
        handle_with_editor(ctx.state, ctx.req, |editor, _| {
            let modified = dispatch::edit_is_modified(editor);
            let mut w = ResponseWriter::ok();
            w.put_bool("modified", modified);
            Ok(w.finish())
        })
    }
);

op_unit!(
    PAGE_MEDIA_BOX,
    "editorPageMediaBox",
    pdf_op_editor_page_media_box_anchor,
    |ctx| {
        handle_with_editor(ctx.state, ctx.req, |editor, req| {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let (x, y, w2, h) = dispatch::edit_page_media_box(editor, page)?;
            let mut w = ResponseWriter::ok();
            w.put_f64("x", x as f64);
            w.put_f64("y", y as f64);
            w.put_f64("width", w2 as f64);
            w.put_f64("height", h as f64);
            Ok(w.finish())
        })
    }
);

op_unit!(
    REDACTION_COUNT,
    "editorRedactionCount",
    pdf_op_editor_redaction_count_anchor,
    |ctx| {
        handle_with_editor(ctx.state, ctx.req, |editor, req| {
            let page = req.get_i32("page").unwrap_or(0) as usize;
            let count = dispatch::edit_redaction_count(editor, page)?;
            let mut w = ResponseWriter::ok();
            w.put_i32("count", count as i32);
            Ok(w.finish())
        })
    }
);

op_unit!(MUTATE, "editorMutate", pdf_op_editor_mutate_anchor, |ctx| {
    let data = ctx.take_data_reader();
    handle_editor_mutate(ctx.state, ctx.req, data)
});

op_unit!(
    MERGE_FROM,
    "editorMergeFrom",
    pdf_op_editor_merge_from_anchor,
    |ctx| {
        let data = ctx.take_data_reader();
        handle_editor_merge_from(ctx.state, ctx.req, data)
    }
);

op_unit!(SAVE, "editorSave", pdf_op_editor_save_anchor, |ctx| {
    let sink = ctx.take_sink(0);
    handle_editor_save(ctx.state, ctx.req, sink)
});

op_unit!(
    EXTRACT_PAGES,
    "editorExtractPages",
    pdf_op_editor_extract_pages_anchor,
    |ctx| {
        let sink = ctx.take_sink(0);
        handle_editor_extract_pages(ctx.state, ctx.req, sink)
    }
);
