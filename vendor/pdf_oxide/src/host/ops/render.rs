// Rasterization ops (the render capability): page rendering and image
// extraction, both streamed through sinks[0].

use crate::host::binary_codec::{ErrorKind, Request, ResponseWriter};
use crate::host::bridge_api::BoxedWriter;
use crate::host::engine_state::EngineState;
use crate::host::ops::op_unit;
use crate::host::ops::support::{pages_of, req_handle};

op_unit!(RENDER, "render", pdf_op_render_anchor, |ctx| {
    let sink = ctx.take_sink(0);
    handle_render_streamed(ctx.state, ctx.req, sink, ctx.cancel)
});

op_unit!(
    EXTRACT_IMAGES,
    "extractImages",
    pdf_op_extract_images_anchor,
    |ctx| {
        let sink = ctx.take_sink(0);
        handle_extract_images_streamed(ctx.state, ctx.req, sink, ctx.cancel)
    }
);

fn handle_render_streamed(
    state: &mut EngineState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
    cancel: &crate::cancel::CancelCheck,
) -> Vec<u8> {
    let hid = req_handle(req);
    let docs = &mut state.documents;
    let doc = match docs.get_mut(&hid) {
        Some(d) => d,
        None => return ResponseWriter::error("document not found"),
    };

    let pages = match requested_pages(doc, req) {
        Ok(pages) => pages,
        Err(resp) => return resp,
    };
    let Some(mut writer) = sink_writer else {
        return ResponseWriter::error_kind(
            ErrorKind::InvalidArgument,
            "render writes its pages to a sink",
        );
    };
    #[cfg(feature = "rendering")]
    {
        let (size, format) = match wire_size_and_format(req) {
            Ok(parsed) => parsed,
            Err(message) => {
                return ResponseWriter::error_kind(ErrorKind::InvalidArgument, &message)
            },
        };
        match crate::host::render_stream::render_pages_streamed(
            doc, &pages, size, format, &mut writer, cancel,
        ) {
            Ok(count) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.put_i32("itemCount", count as i32);
                w.finish()
            },
            Err(e) => ResponseWriter::error_of(&e),
        }
    }
    #[cfg(not(feature = "rendering"))]
    {
        let _ = (&mut writer, &pages, doc, cancel);
        ResponseWriter::error("Rendering not enabled")
    }
}

/// The size and format a render request names. `sizeKind` is `dpi`, `fit`
/// or `thumbnail`, each with its own fields; `format` is `png` or `rgba`.
#[cfg(feature = "rendering")]
fn wire_size_and_format(
    req: &Request<'_>,
) -> std::result::Result<
    (
        crate::host::render_stream::RenderSize,
        crate::host::render_stream::RenderFormat,
    ),
    String,
> {
    use crate::host::render_stream::{RenderFormat, RenderSize};
    let positive = |key: &str| -> std::result::Result<u32, String> {
        match req.get_i32(key) {
            Some(n) if n > 0 => Ok(n as u32),
            Some(n) => Err(format!("{key} must be positive: {n}")),
            None => Err(format!("render needs {key}")),
        }
    };
    let size = match req.get_str("sizeKind") {
        Some("dpi") => RenderSize::Dpi(positive("dpi")?),
        Some("fit") => RenderSize::Fit {
            max_width: positive("maxWidth")?,
            max_height: positive("maxHeight")?,
        },
        Some("thumbnail") => RenderSize::Thumbnail(positive("size")?),
        Some(other) => return Err(format!("unknown render size kind: {other}")),
        None => return Err("render needs sizeKind".to_string()),
    };
    let format = match req.get_str("format") {
        Some(name) => RenderFormat::from_name(name)
            .ok_or_else(|| format!("unknown render format: {name}"))?,
        None => return Err("render needs format".to_string()),
    };
    Ok((size, format))
}

fn handle_extract_images_streamed(
    state: &mut EngineState,
    req: &Request<'_>,
    sink_writer: Option<BoxedWriter>,
    cancel: &crate::cancel::CancelCheck,
) -> Vec<u8> {
    let hid = req_handle(req);
    let docs = &mut state.documents;
    let doc = match docs.get_mut(&hid) {
        Some(d) => d,
        None => return ResponseWriter::error("document not found"),
    };

    let pages = match requested_pages(doc, req) {
        Ok(pages) => pages,
        Err(resp) => return resp,
    };
    let Some(mut writer) = sink_writer else {
        return ResponseWriter::error_kind(
            ErrorKind::InvalidArgument,
            "extractImages writes its images to a sink",
        );
    };
    #[cfg(feature = "rendering")]
    {
        match crate::host::extract_stream::extract_images_streamed(doc, &pages, &mut writer, cancel)
        {
            Ok(count) => {
                let mut w = ResponseWriter::ok();
                w.put_bool("streamed", true);
                w.put_i32("itemCount", count as i32);
                w.finish()
            },
            Err(e) => ResponseWriter::error_of(&e),
        }
    }
    #[cfg(not(feature = "rendering"))]
    {
        let _ = (&mut writer, &pages, doc, cancel);
        ResponseWriter::error("Image extraction not enabled")
    }
}

/// The pages a request names in `pageIndices`; an empty list is all pages
/// (Dart's `PdfPages.all()` sends `[]`).
fn requested_pages(
    doc: &mut crate::document::PdfDocument,
    req: &Request<'_>,
) -> Result<Vec<usize>, Vec<u8>> {
    let page_indices = req.get_int_list("pageIndices").unwrap_or(&[]);
    let count = doc
        .page_count()
        .map_err(|e| ResponseWriter::error_of(&e))?;
    if page_indices.is_empty() {
        Ok((0..count).collect())
    } else {
        pages_of(page_indices, count)
    }
}
