// Document-handle ops: open/dispose plus read ops that reuse the parsed
// document via handleId. Core — always built.

use crate::host::binary_codec::ResponseWriter;
use crate::host::bridge_api::{handle_open, handle_with_doc, ok_flag, req_handle};
use crate::host::dispatch;
use crate::host::ops::op_unit;

op_unit!(OPEN, "open", pdf_op_open_anchor, |ctx| {
    let source = ctx.take_source(0);
    handle_open(ctx.state, ctx.req, ctx.source_bytes, source)
});

op_unit!(DISPOSE, "docDispose", pdf_op_doc_dispose_anchor, |ctx| {
    ctx.state.documents.remove(&req_handle(ctx.req));
    ok_flag("disposed")
});

op_unit!(EXTRACT, "extract", pdf_op_extract_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, req| {
        let page = req.get_i32("page").map(|p| p as usize);
        let format = req.get_str("format").unwrap_or("plainText");
        let result = dispatch::extract_text(doc, page, format)?;
        let mut w = ResponseWriter::ok();
        w.put_str("text", &result.text);
        Ok(w.finish())
    })
});

op_unit!(SEARCH, "search", pdf_op_search_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, req| {
        let query = req.get_str("query").unwrap_or("");
        let page = req.get_i32("page").map(|p| p as usize);
        let result = dispatch::search_text(doc, query, page)?;
        let mut w = ResponseWriter::ok();
        w.put_map_list("hits", result.hits.len(), |i, item| {
            let h = &result.hits[i];
            item.put_i32("page", h.page as i32);
            item.put_str("text", &h.text);
            item.put_f64("x", h.x as f64);
            item.put_f64("y", h.y as f64);
            item.put_f64("width", h.width as f64);
            item.put_f64("height", h.height as f64);
        });
        Ok(w.finish())
    })
});

op_unit!(
    PLAN_SPLIT_BY_BOOKMARKS,
    "planSplitByBookmarks",
    pdf_op_plan_split_by_bookmarks_anchor,
    |ctx| {
        handle_with_doc(ctx.state, ctx.req, |doc, _| {
            let result = dispatch::plan_split_by_bookmarks(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_map_list("splits", result.len(), |i, item| {
                let s = &result[i];
                item.put_str("title", &s.title);
                item.put_i32("startPage", s.start_page as i32);
                item.put_i32("endPage", s.end_page as i32);
            });
            Ok(w.finish())
        })
    }
);

op_unit!(CLASSIFY_PAGE, "classifyPage", pdf_op_classify_page_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, req| {
        let page = req.get_i32("page").unwrap_or(0) as usize;
        let result = dispatch::classify_page(doc, page)?;
        let mut w = ResponseWriter::ok();
        w.put_str("type", &result.type_name);
        Ok(w.finish())
    })
});

op_unit!(
    CLASSIFY_DOCUMENT,
    "classifyDocument",
    pdf_op_classify_document_anchor,
    |ctx| {
        handle_with_doc(ctx.state, ctx.req, |doc, _| {
            let result = dispatch::classify_document(doc)?;
            let mut w = ResponseWriter::ok();
            w.put_str("type", &result.type_name);
            Ok(w.finish())
        })
    }
);
