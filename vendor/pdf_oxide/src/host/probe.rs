//! Out-of-crate measurement door — runs one wire request the way an executor
//! does, for memory probes that live outside this crate.

use crate::host::bridge_api::{self, BoxedReader, BoxedWriter};
use crate::host::engine_state::EngineState;
use std::fs::File;

/// Runs one wire request against `state`, wrapping each `File` in
/// the bridge's boxed reader/writer exactly as the transports do.
pub fn run(
    state: &mut EngineState,
    request: &[u8],
    sources: Vec<File>,
    sinks: Vec<File>,
) -> Vec<u8> {
    let sources = sources.into_iter().map(|f| BoxedReader(Box::new(f))).collect();
    let sinks = sinks.into_iter().map(BoxedWriter::new).collect();
    bridge_api::handle_request(state, request, None, sources, sinks, &crate::cancel::CancelCheck::none())
}

// ── Entry points tool/fuelprobe runs under `CancelCheck::probe()`. Each is the
// function the host op of the same name calls, made reachable from outside
// the crate. ──

use crate::cancel::CancelCheck;
use crate::document::PdfDocument;
use crate::editor::{DocumentEditor, SaveOptions};
use crate::error::Result;

/// `render`: `pages` at `dpi`, as PNG chunks, to `writer`.
#[cfg(feature = "rendering")]
pub fn render_streamed(
    doc: &PdfDocument,
    pages: &[usize],
    dpi: u32,
    writer: &mut impl std::io::Write,
    cancel: &CancelCheck,
) -> Result<usize> {
    use crate::host::render_stream::{render_pages_streamed, RenderFormat, RenderSize};
    let _job = crate::cancel::enter(cancel);
    render_pages_streamed(doc, pages, RenderSize::Dpi(dpi), RenderFormat::Png, writer, cancel)
}

/// `extractImages`: the images drawn on `pages`, as chunked items, to `writer`.
#[cfg(feature = "rendering")]
pub fn extract_images_streamed(
    doc: &PdfDocument,
    pages: &[usize],
    writer: &mut impl std::io::Write,
    cancel: &CancelCheck,
) -> Result<usize> {
    let _job = crate::cancel::enter(cancel);
    crate::host::extract_stream::extract_images_streamed(doc, pages, writer, cancel)
}

/// `editorSave` in incremental mode.
pub fn save_incremental(
    editor: &mut DocumentEditor,
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    options: &SaveOptions,
    cancel: &CancelCheck,
) -> Result<()> {
    let _job = crate::cancel::enter(cancel);
    crate::host::incremental::write(editor, writer, options, cancel).map_err(|e| match e {
        crate::host::incremental::IncrementalError::Refused(why) => {
            crate::error::Error::InvalidOperation(why)
        },
        crate::host::incremental::IncrementalError::Failed(e) => e,
    })
}

/// `editorMergeFrom`: `pages` (all when `None`) of the PDF in `bytes`.
pub fn merge_pages(
    editor: &mut DocumentEditor,
    bytes: Vec<u8>,
    pages: Option<&[usize]>,
    cancel: &CancelCheck,
) -> Result<()> {
    let _job = crate::cancel::enter(cancel);
    let source = DocumentEditor::open_merge_source(Box::new(std::io::Cursor::new(bytes)))?;
    editor.merge_document(source, pages, cancel).map(|_| ())
}

/// `editorExtractPages` (split): a full-rewrite save of `pages` alone.
pub fn extract_pages(
    editor: &mut DocumentEditor,
    pages: &[usize],
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    cancel: &CancelCheck,
) -> Result<()> {
    let _job = crate::cancel::enter(cancel);
    editor
        .with_pages_selected(pages, |editor| {
            crate::host::dispatch::edit_save_with_options(
                editor,
                writer,
                &SaveOptions::full_rewrite(),
                cancel,
            )
        })
        .and_then(|saved| saved)
}

/// `open` / `editorOpen`'s source: the PDF in `bytes`, read through the
/// external-reader path the executors use, not the in-memory one.
pub fn open_source(bytes: Vec<u8>, cancel: &CancelCheck) -> Result<PdfDocument> {
    let _job = crate::cancel::enter(cancel);
    PdfDocument::from_external_reader_cancellable(Box::new(std::io::Cursor::new(bytes)), cancel)
}

/// `builderAddImagePage` then `builderSave`: the PNG or JPEG in `image` is
/// taken from a source as the op takes it, placed on a page of its own size,
/// and the builder saved to `writer`, which is where the save reads it.
pub fn build_image_page(
    image: Vec<u8>,
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    cancel: &CancelCheck,
) -> Result<()> {
    use crate::host::dispatch::{self, PageOp};
    use crate::host::image_embed::{take, Taken};
    use crate::writer::PageSize;
    let _job = crate::cancel::enter(cancel);
    let source = BoxedReader(Box::new(std::io::Cursor::new(image)));
    let Taken::Image(image) = take(source)? else {
        return Err(crate::error::Error::InvalidOperation("the bytes are no PNG or JPEG image".into()));
    };
    let Some((width, height)) = image.pixel_dimensions() else {
        return Err(crate::error::Error::InvalidOperation("the image has no size".into()));
    };
    let mut builder = dispatch::builder_new();
    builder.begin_page(PageSize::Custom(width as f32, height as f32));
    let op = PageOp::Image { data: image, x: 0.0, y: 0.0, w: width as f32, h: height as f32, alt: String::new() };
    dispatch::apply_page_op(&mut builder, op)?;
    builder.end_page();
    dispatch::builder_save_to_writer(builder.compress_streams(true), writer)
}
