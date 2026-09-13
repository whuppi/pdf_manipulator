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

op_unit!(FORM_FIELDS, "formFields", pdf_op_form_fields_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, _| {
        let rows = dispatch::doc_form_fields(doc)?;
        let mut w = ResponseWriter::ok();
        w.put_map_list("fields", rows.len(), |i, item| {
            let f = &rows[i];
            item.put_str("name", &f.name);
            item.put_str("type", f.field_type);
            item.put_str("valueKind", f.value_kind);
            item.put_str("text", &f.text);
            item.put_bool("checked", f.checked);
            let choices: Vec<&str> = f.choices.iter().map(|s| s.as_str()).collect();
            item.put_string_list("choices", &choices);
            item.put_str("tooltip", &f.tooltip);
            item.put_bool("hasBounds", f.has_bounds);
            item.put_f64("x", f.x);
            item.put_f64("y", f.y);
            item.put_f64("width", f.width);
            item.put_f64("height", f.height);
            item.put_i32("maxLength", f.max_length);
            item.put_i32("alignment", f.alignment);
            item.put_bool("readOnly", f.read_only);
            item.put_bool("required", f.required);
        });
        Ok(w.finish())
    })
});

op_unit!(XFA, "xfa", pdf_op_xfa_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, _| {
        let info = dispatch::doc_xfa(doc)?;
        let mut w = ResponseWriter::ok();
        w.put_bool("has", info.has_xfa);
        w.put_i32("fieldCount", info.field_count);
        w.put_i32("pageCount", info.page_count);
        let types: Vec<&str> = info.field_types.iter().map(|s| s.as_str()).collect();
        w.put_string_list("fieldTypes", &types);
        Ok(w.finish())
    })
});

op_unit!(ATTACHMENTS, "attachments", pdf_op_attachments_anchor, |ctx| {
    handle_with_doc(ctx.state, ctx.req, |doc, _| {
        let rows = dispatch::doc_attachments(doc)?;
        let mut w = ResponseWriter::ok();
        w.put_map_list("attachments", rows.len(), |i, item| {
            let a = &rows[i];
            item.put_str("name", &a.name);
            item.put_i64("size", a.size);
            item.put_str("description", &a.description);
            item.put_str("mimeType", &a.mime_type);
        });
        Ok(w.finish())
    })
});

op_unit!(
    EXTRACT_ATTACHMENT,
    "extractAttachment",
    pdf_op_extract_attachment_anchor,
    |ctx| {
        let name = ctx.req.get_str("name").unwrap_or("").to_string();
        let sink = ctx.take_sink(0);
        let hid = req_handle(ctx.req);
        let doc = match ctx.state.documents.get_mut(&hid) {
            Some(d) => d,
            None => return ResponseWriter::error("document not found"),
        };
        let mut writer = match sink {
            Some(w) => w,
            None => return ResponseWriter::error("extractAttachment requires a sink"),
        };
        match dispatch::doc_extract_attachment(doc, &name, &mut writer) {
            Ok(()) => ok_flag("streamed"),
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
);

op_unit!(
    EXPORT_FORM_DATA,
    "exportFormData",
    pdf_op_export_form_data_anchor,
    |ctx| {
        let format = ctx.req.get_str("format").unwrap_or("xfdf").to_string();
        let sink = ctx.take_sink(0);
        let hid = req_handle(ctx.req);
        let docs = &mut ctx.state.documents;
        let doc = match docs.get_mut(&hid) {
            Some(d) => d,
            None => return ResponseWriter::error("document not found"),
        };
        let mut writer = match sink {
            Some(w) => w,
            None => return ResponseWriter::error("exportFormData requires a sink"),
        };
        match dispatch::doc_export_form_data(doc, &format, &mut writer) {
            Ok(()) => ok_flag("streamed"),
            Err(e) => ResponseWriter::error(&e.to_string()),
        }
    }
);
