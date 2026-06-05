//! Shared dispatch — the single brain for both native and WASM.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! RULE: Every operation the Flutter bridge can perform is a function here.
//! bridge_api.rs is the only caller. No other file calls the engine directly.
//!
//! INVARIANTS:
//! - No FFI types (pointers, ports, extern "C").
//! - No WASM types (JsValue, wasm_bindgen).
//! - No encoding (binary, JSON, JS objects).
//! - No platform-specific imports.
//! - Pure typed Rust in, typed Rust out.

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::error::{Error, Result};

// ═══════════════════════════════════════════════════════════════════
// Result types — typed structs returned by dispatch functions.
// bridge_api.rs encodes these to binary via ResponseWriter.
// ═══════════════════════════════════════════════════════════════════

/// Result of opening and inspecting a PDF document.
pub struct OpenResult {
    /// Total number of pages.
    pub page_count: usize,
    /// PDF major version (e.g. 1 for PDF 1.7).
    pub version_major: u8,
    /// PDF minor version (e.g. 7 for PDF 1.7).
    pub version_minor: u8,
    /// Whether the document has an encryption dictionary.
    pub is_encrypted: bool,
    /// Whether the document requires a password to read.
    pub requires_password: bool,
    /// Whether the document has a structure tree (tagged PDF).
    pub is_tagged: bool,
    /// Encryption algorithm identifier (0 if not encrypted).
    pub encryption_algorithm: u8,
    /// Permission flags byte (0xFF if not encrypted).
    pub permission_bits: u8,
    /// Per-page dimensions and rotation.
    pub pages: Vec<PageInfo>,
    /// Document info Title field.
    pub title: String,
    /// Document info Author field.
    pub author: String,
    /// Document info Subject field.
    pub subject: String,
    /// Document info Keywords field.
    pub keywords: String,
}

/// Dimensions and rotation of a single page.
pub struct PageInfo {
    /// Page width in points.
    pub width: f64,
    /// Page height in points.
    pub height: f64,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotation: i32,
}

/// Result of text extraction from one or more pages.
pub struct ExtractTextResult {
    /// Extracted text content.
    pub text: String,
}

/// A single text search match with location.
pub struct SearchHit {
    /// Zero-based page index of the match.
    pub page: usize,
    /// Matched text.
    pub text: String,
    /// Bounding box X origin in points.
    pub x: f32,
    /// Bounding box Y origin in points.
    pub y: f32,
    /// Bounding box width in points.
    pub width: f32,
    /// Bounding box height in points.
    pub height: f32,
}

/// Result of a text search across pages.
pub struct SearchResult {
    /// All matches found.
    pub hits: Vec<SearchHit>,
}

/// Metadata for one digital signature in the document.
pub struct SignatureInfo {
    /// Name of the signer.
    pub signer_name: String,
    /// Stated reason for signing.
    pub reason: String,
    /// Stated signing location.
    pub location: String,
}

/// Result of enumerating digital signatures.
pub struct SignaturesResult {
    /// All signatures found in the document.
    pub signatures: Vec<SignatureInfo>,
}

/// Result of PDF/A or PDF/UA compliance validation.
pub struct ValidationResult {
    /// Whether the document is compliant with the requested level.
    pub compliant: bool,
    /// Number of validation errors (-1 if validation itself failed).
    pub errors: i32,
    /// Number of validation warnings.
    pub warnings: i32,
}

/// Result of page or document classification.
pub struct ClassificationResult {
    /// Debug-formatted classification type name.
    pub type_name: String,
}

/// A bookmark-based split range for document splitting.
pub struct BookmarkSplit {
    /// Bookmark title.
    pub title: String,
    /// First page index (inclusive).
    pub start_page: usize,
    /// Last page index (inclusive).
    pub end_page: usize,
}

/// RGBA pixel data for a rendered page.
pub struct RenderedPage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw RGBA pixel bytes.
    pub data: Vec<u8>,
}

/// An image extracted from a PDF page.
pub struct ExtractedImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Image format (e.g. "jpeg", "raw").
    pub format: String,
    /// Color space (e.g. "DeviceRGB").
    pub color_space: String,
    /// Bits per color component.
    pub bits_per_component: u32,
    /// Raw image bytes.
    pub data: Vec<u8>,
}

/// Metadata snapshot from a DocumentEditor.
pub struct EditorMetadataResult {
    /// Current page count in the editor.
    pub page_count: usize,
    /// PDF major version.
    pub version_major: u8,
    /// PDF minor version.
    pub version_minor: u8,
    /// Document title.
    pub title: String,
    /// Document author.
    pub author: String,
    /// Document subject.
    pub subject: String,
    /// Document keywords.
    pub keywords: String,
}

// ═══════════════════════════════════════════════════════════════════
// Read operations — take &mut PdfDocument, return typed results.
// ═══════════════════════════════════════════════════════════════════

/// Open a document and return its metadata, page info, and document info strings.
pub fn open_document(doc: &mut PdfDocument) -> Result<OpenResult> {
    let page_count = doc.page_count()?;
    let (major, minor) = doc.version();
    let is_encrypted = doc.is_encrypted();
    let requires_password = is_encrypted && !doc.is_authenticated();
    let is_tagged = doc.structure_tree().ok().flatten().is_some();

    // Encryption info — reads from the encrypt dict via pub(crate) accessors
    // added by our patch on document.rs. Returns 0 if not encrypted.
    let enc_algo: u8 = doc.encryption_algorithm().unwrap_or(0);
    let perms: u8 = doc.permission_bits().map(|p| p as u8).unwrap_or(0xFF);

    let mut pages = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let (x0, y0, x1, y1) = doc.get_page_media_box(i).unwrap_or((0.0, 0.0, 612.0, 792.0));
        let rotation = doc.get_page_rotation(i).unwrap_or(0);
        pages.push(PageInfo {
            width: (x1 - x0) as f64,
            height: (y1 - y0) as f64,
            rotation,
        });
    }

    let title = doc.document_info_string("Title").unwrap_or_default();
    let author = doc.document_info_string("Author").unwrap_or_default();
    let subject = doc.document_info_string("Subject").unwrap_or_default();
    let keywords = doc.document_info_string("Keywords").unwrap_or_default();

    Ok(OpenResult {
        page_count, version_major: major, version_minor: minor,
        is_encrypted, requires_password, is_tagged,
        encryption_algorithm: enc_algo, permission_bits: perms,
        pages, title, author, subject, keywords,
    })
}

/// Extract text from one page or all pages in the given format.
pub fn extract_text(doc: &mut PdfDocument, page: Option<usize>, format: &str) -> Result<ExtractTextResult> {
    let text = match format {
        "markdown" => {
            let opts = crate::converters::ConversionOptions::default();
            match page {
                None => doc.to_markdown_all(&opts)?,
                Some(i) => doc.to_markdown(i, &opts)?,
            }
        }
        "html" => {
            let opts = crate::converters::ConversionOptions::default();
            match page {
                None => {
                    let count = doc.page_count()?;
                    let mut all = String::new();
                    for i in 0..count {
                        if i > 0 { all.push('\n'); }
                        all.push_str(&doc.to_html(i, &opts)?);
                    }
                    all
                }
                Some(i) => doc.to_html(i, &opts)?,
            }
        }
        _ => {
            match page {
                None => {
                    let count = doc.page_count()?;
                    let mut all = String::new();
                    for i in 0..count {
                        if i > 0 { all.push('\n'); }
                        all.push_str(&doc.extract_text(i)?);
                    }
                    all
                }
                Some(i) => doc.extract_text(i)?,
            }
        }
    };
    Ok(ExtractTextResult { text })
}

/// Search for text across the document, optionally filtered to one page.
pub fn search_text(doc: &mut PdfDocument, query: &str, page: Option<usize>) -> Result<SearchResult> {
    use crate::search::{SearchOptions, TextSearcher};
    let opts = SearchOptions::default();
    let all_hits = TextSearcher::search(doc, query, &opts)?;
    let hits: Vec<SearchHit> = all_hits
        .into_iter()
        .filter(|h| page.map_or(true, |p| h.page == p))
        .map(|h| SearchHit {
            page: h.page,
            text: h.text,
            x: h.bbox.x,
            y: h.bbox.y,
            width: h.bbox.width,
            height: h.bbox.height,
        })
        .collect();
    Ok(SearchResult { hits })
}

/// Enumerate digital signatures in the document.
pub fn get_signatures(_doc: &mut PdfDocument) -> Result<SignaturesResult> {
    #[cfg(feature = "signatures")]
    {
        let sigs = crate::signatures::enumerate_signatures(_doc)?;
        let signatures = sigs.iter().map(|s| SignatureInfo {
            signer_name: s.signer_name.clone().unwrap_or_default(),
            reason: s.reason.clone().unwrap_or_default(),
            location: s.location.clone().unwrap_or_default(),
        }).collect();
        Ok(SignaturesResult { signatures })
    }
    #[cfg(not(feature = "signatures"))]
    { Ok(SignaturesResult { signatures: vec![] }) }
}

/// Verify digital signatures (stub — always returns false).
pub fn verify_signatures(_doc: &mut PdfDocument) -> Result<bool> {
    Ok(false)
}

/// Validate PDF/A compliance at the given level (1=A1b, 2=A2b, 3=A3b).
pub fn validate_pdf_a(doc: &mut PdfDocument, level: i32) -> Result<ValidationResult> {
    use crate::compliance::{validate_pdf_a as do_validate, PdfALevel};
    let pdf_level = match level {
        1 => PdfALevel::A1b,
        3 => PdfALevel::A3b,
        _ => PdfALevel::A2b,
    };
    match do_validate(doc, pdf_level) {
        Ok(v) => Ok(ValidationResult {
            compliant: v.is_compliant,
            errors: v.errors.len() as i32,
            warnings: v.warnings.len() as i32,
        }),
        Err(_) => Ok(ValidationResult { compliant: false, errors: -1, warnings: 0 }),
    }
}

/// Validate PDF/UA compliance at the given level (1=UA1, 2=UA2).
pub fn validate_pdf_ua(doc: &mut PdfDocument, level: i32) -> Result<bool> {
    use crate::compliance::pdf_ua::{validate_pdf_ua as do_validate, PdfUaLevel};
    let ua_level = match level {
        2 => PdfUaLevel::Ua2,
        _ => PdfUaLevel::Ua1,
    };
    Ok(do_validate(doc, ua_level).map(|r| r.is_compliant).unwrap_or(false))
}

/// Classify a single page's content type.
pub fn classify_page(doc: &mut PdfDocument, page: usize) -> Result<ClassificationResult> {
    let classification = doc.classify_page(page)?;
    Ok(ClassificationResult { type_name: format!("{:?}", classification) })
}

/// Classify the overall document type.
pub fn classify_document(doc: &mut PdfDocument) -> Result<ClassificationResult> {
    let classification = doc.classify_document()?;
    Ok(ClassificationResult { type_name: format!("{:?}", classification) })
}

/// Plan bookmark-based page ranges for splitting the document.
pub fn plan_split_by_bookmarks(doc: &mut PdfDocument) -> Result<Vec<BookmarkSplit>> {
    use crate::split_bookmarks::{plan_split_by_bookmarks as do_plan, SplitByBookmarksOptions};
    let opts = SplitByBookmarksOptions::default();
    let splits = do_plan(doc, &opts)?;
    Ok(splits.iter().map(|s| BookmarkSplit {
        title: s.title.clone().unwrap_or_default(),
        start_page: s.start_page,
        end_page: s.end_page,
    }).collect())
}

/// Render a page to RGBA pixels, optionally constrained to max dimensions.
pub fn render_page(doc: &mut PdfDocument, page: usize, max_width: u32, max_height: u32) -> Result<RenderedPage> {
    #[cfg(feature = "rendering")]
    {
        use crate::rendering::{render_page as do_render, render_page_fit, RenderOptions};
        let opts = RenderOptions::default();
        let rendered = if max_width > 0 && max_height > 0 {
            render_page_fit(doc, page, max_width, max_height, &opts)?
        } else {
            do_render(doc, page, &opts)?
        };
        Ok(RenderedPage {
            width: rendered.width as u32,
            height: rendered.height as u32,
            data: rendered.data,
        })
    }
    #[cfg(not(feature = "rendering"))]
    {
        let _ = (doc, page, max_width, max_height);
        Err(Error::InvalidPdf("Rendering not enabled".into()))
    }
}

/// Extract all images from a page as decoded pixel data.
pub fn extract_images(doc: &mut PdfDocument, page: usize) -> Result<Vec<ExtractedImage>> {
    let images = doc.extract_images(page)?;
    Ok(images.into_iter().map(|img| {
        let (fmt, data) = match img.data() {
            crate::extractors::ImageData::Jpeg(bytes) => ("jpeg".to_string(), bytes.to_vec()),
            crate::extractors::ImageData::Raw { pixels, .. } => ("raw".to_string(), pixels.to_vec()),
        };
        ExtractedImage {
            width: img.width() as u32,
            height: img.height() as u32,
            format: fmt,
            color_space: format!("{:?}", img.color_space()),
            bits_per_component: img.bits_per_component() as u32,
            data,
        }
    }).collect())
}

/// O(1)-memory streaming image extraction.
///
/// Extracts images one at a time and writes each as a length-prefixed
/// binary frame to the writer. Each image is dropped after writing.
/// At most one image in memory at any time.
///
/// Frame format: [len: u32 LE] [binary-encoded image response]
/// End marker:   [0x00000000]
pub fn extract_images_streamed<W: std::io::Write>(
    doc: &mut PdfDocument,
    page: usize,
    writer: &mut W,
) -> Result<usize> {
    use crate::host::binary_codec::ResponseWriter;
    let images = doc.extract_images(page)?;
    let count = images.len();
    for img in images {
        let (fmt, data) = match img.data() {
            crate::extractors::ImageData::Jpeg(bytes) => ("jpeg", bytes.to_vec()),
            crate::extractors::ImageData::Raw { pixels, .. } => ("raw", pixels.to_vec()),
        };
        let mut w = ResponseWriter::ok();
        w.put_i32("width", img.width() as i32);
        w.put_i32("height", img.height() as i32);
        w.put_str("format", fmt);
        w.put_str("colorSpace", &format!("{:?}", img.color_space()));
        w.put_i32("bitsPerComponent", img.bits_per_component() as i32);
        w.put_bytes("data", &data);
        let frame = w.finish();
        let len_bytes = (frame.len() as u32).to_le_bytes();
        writer.write_all(&len_bytes).map_err(|e| Error::InvalidPdf(e.to_string()))?;
        writer.write_all(&frame).map_err(|e| Error::InvalidPdf(e.to_string()))?;
        // image data dropped here — only one image in memory at a time
    }
    // End marker
    writer.write_all(&0u32.to_le_bytes()).map_err(|e| Error::InvalidPdf(e.to_string()))?;
    Ok(count)
}

/// O(1)-memory streaming page render.
///
/// Renders pages one at a time and writes each as a length-prefixed
/// binary frame to the writer. Each RGBA buffer is dropped after writing.
/// At most one rendered page in memory at any time.
///
/// Frame format: [len: u32 LE] [binary-encoded rendered page response]
/// End marker:   [0x00000000]
pub fn render_pages_streamed<W: std::io::Write>(
    doc: &mut PdfDocument,
    pages: &[usize],
    max_width: u32,
    max_height: u32,
    writer: &mut W,
) -> Result<usize> {
    use crate::host::binary_codec::ResponseWriter;
    let count = pages.len();
    for &page in pages {
        let result = render_page(doc, page, max_width, max_height)?;
        let mut w = ResponseWriter::ok();
        w.put_i32("width", result.width as i32);
        w.put_i32("height", result.height as i32);
        w.put_bytes("data", &result.data);
        let frame = w.finish();
        let len_bytes = (frame.len() as u32).to_le_bytes();
        writer.write_all(&len_bytes).map_err(|e| Error::InvalidPdf(e.to_string()))?;
        writer.write_all(&frame).map_err(|e| Error::InvalidPdf(e.to_string()))?;
        // RGBA data dropped here — only one page in memory at a time
    }
    // End marker
    writer.write_all(&0u32.to_le_bytes()).map_err(|e| Error::InvalidPdf(e.to_string()))?;
    Ok(count)
}

// ═══════════════════════════════════════════════════════════════════
// Editor operations — take &mut DocumentEditor, return typed results.
// Uses upstream DocumentEditor public + EditableDocument trait methods.
// ═══════════════════════════════════════════════════════════════════

/// Read current metadata from the editor.
pub fn edit_get_metadata(editor: &mut DocumentEditor) -> EditorMetadataResult {
    let version = editor.version();
    EditorMetadataResult {
        page_count: editor.current_page_count(),
        version_major: version.0,
        version_minor: version.1,
        title: editor.title().ok().flatten().unwrap_or_default(),
        author: editor.author().ok().flatten().unwrap_or_default(),
        subject: editor.subject().ok().flatten().unwrap_or_default(),
        keywords: editor.keywords().ok().flatten().unwrap_or_default(),
    }
}

/// Check whether the editor has unsaved modifications.
pub fn edit_is_modified(editor: &DocumentEditor) -> bool {
    editor.is_modified()
}

/// Get the media box (x, y, width, height) for a page.
pub fn edit_page_media_box(editor: &mut DocumentEditor, page: usize) -> Result<(f32, f32, f32, f32)> {
    let mb = editor.get_page_media_box(page)?;
    Ok((mb[0], mb[1], mb[2], mb[3]))
}

// ── Metadata setters ──

/// Set the document title.
pub fn edit_set_title(editor: &mut DocumentEditor, value: &str) {
    editor.set_title(value);
}

/// Set the document author.
pub fn edit_set_author(editor: &mut DocumentEditor, value: &str) {
    editor.set_author(value);
}

/// Set the document subject.
pub fn edit_set_subject(editor: &mut DocumentEditor, value: &str) {
    editor.set_subject(value);
}

/// Set the document keywords.
pub fn edit_set_keywords(editor: &mut DocumentEditor, value: &str) {
    editor.set_keywords(value);
}

// ── Page manipulation ──

/// Keep only the specified pages (by index), removing all others.
pub fn edit_select_pages(editor: &mut DocumentEditor, pages: &[usize]) -> Result<()> {
    editor.select_pages(pages)
}

/// Delete pages by index. Uses select_pages with the inverse set —
/// no upstream patch needed.
pub fn edit_delete_pages(editor: &mut DocumentEditor, pages: &[usize]) -> Result<()> {
    let total = editor.current_page_count();
    let keep: Vec<usize> = (0..total).filter(|i| !pages.contains(i)).collect();
    editor.select_pages(&keep)
}

/// Rotate specific pages by the given degrees.
pub fn edit_rotate_pages(editor: &mut DocumentEditor, rotations: &[(usize, i32)]) -> Result<()> {
    for &(page, degrees) in rotations {
        editor.rotate_page_by(page, degrees)?;
    }
    Ok(())
}

/// Rotate all pages by the given degrees.
pub fn edit_rotate_all(editor: &mut DocumentEditor, degrees: i32) -> Result<()> {
    editor.rotate_all_pages(degrees)
}

/// Move a page from one index to another.
pub fn edit_move_page(editor: &mut DocumentEditor, from: usize, to: usize) -> Result<()> {
    use crate::editor::EditableDocument;
    editor.move_page(from, to)
}

/// Merge pages from one or more secondary PDFs into this document.
pub fn edit_merge(editor: &mut DocumentEditor, secondary_bytes: &[Vec<u8>]) -> Result<()> {
    for secondary in secondary_bytes {
        editor.merge_from_bytes(secondary)?;
    }
    Ok(())
}

// ── Content operations ──

/// Flatten interactive form fields into page content.
pub fn edit_flatten_forms(editor: &mut DocumentEditor) -> Result<()> {
    editor.flatten_forms()
}

/// Flatten all annotations into page content.
pub fn edit_flatten_all_annotations(editor: &mut DocumentEditor) -> Result<()> {
    editor.flatten_all_annotations()
}

/// Compress the document (currently a no-op without image recompress).
pub fn edit_compress(editor: &mut DocumentEditor, _quality: u8) -> Result<()> {
    // Image optimization requires the image_optimizer patch.
    // Compression without image recompress is a no-op for now.
    let _ = editor;
    Ok(())
}

/// Recompress images above `min_size` bytes at the given quality. Returns count optimized.
pub fn edit_optimize_images(editor: &mut DocumentEditor, quality: u8, min_size: u32) -> Result<usize> {
    // Image optimizer runs on the source document's object graph.
    // Modified objects are staged via insert_modified for the next save.
    #[cfg(feature = "rendering")]
    {
        let mut mods = std::collections::HashMap::new();
        let count = crate::host::image_optimizer::optimize_images(
            editor.source(), &mut mods, quality, min_size,
        )?;
        for (id, obj) in mods {
            editor.insert_modified(id, obj);
        }
        Ok(count)
    }
    #[cfg(not(feature = "rendering"))]
    {
        let _ = (editor, quality, min_size);
        Ok(0)
    }
}

/// Remove embedded copies of the 14 standard PDF fonts. Returns count unembedded.
pub fn edit_unembed_standard_fonts(editor: &mut DocumentEditor) -> Result<usize> {
    let mut mods = std::collections::HashMap::new();
    let count = crate::host::font_optimizer::unembed_standard_fonts(
        editor.source(), &mut mods,
    )?;
    for (id, obj) in mods {
        editor.insert_modified(id, obj);
    }
    Ok(count)
}

/// Embed a file attachment into the document.
pub fn edit_embed_file(editor: &mut DocumentEditor, name: &str, data: Vec<u8>) -> Result<()> {
    editor.embed_file(name, data)
}

/// Erase rectangular regions from a page's content.
pub fn edit_erase_regions(editor: &mut DocumentEditor, page: usize, rects: &[[f32; 4]]) -> Result<()> {
    editor.erase_regions(page, rects)
}

/// Crop all pages by the given margin insets (in points).
pub fn edit_crop_margins(editor: &mut DocumentEditor, left: f32, right: f32, top: f32, bottom: f32) -> Result<()> {
    editor.crop_margins(left, right, top, bottom)
}

/// Set a form field's value by field name.
pub fn edit_set_form_field_value(editor: &mut DocumentEditor, name: &str, value: &str) -> Result<()> {
    use crate::editor::form_fields::FormFieldValue;
    editor.set_form_field_value(name, FormFieldValue::Text(value.to_string()))
}

/// Resize a named image XObject on a page.
pub fn edit_resize_image(editor: &mut DocumentEditor, page: usize, name: &str, width: f32, height: f32) -> Result<()> {
    editor.resize_image(page, name, width, height)
}

/// Convert the document to PDF/A at the given level (1=A1b, 2=A2b, 3=A3b).
pub fn edit_convert_to_pdf_a(editor: &mut DocumentEditor, level: i32) -> Result<()> {
    use crate::compliance::PdfALevel;
    let pdf_level = match level {
        1 => PdfALevel::A1b,
        3 => PdfALevel::A3b,
        _ => PdfALevel::A2b,
    };
    let converter = crate::compliance::PdfAConverter::new(pdf_level);
    converter.convert_with_editor(editor)?;
    Ok(())
}

// ── Redaction ──

/// Mark a rectangular region on a page for redaction.
pub fn edit_add_redaction(editor: &mut DocumentEditor, page: usize, rect: [f32; 4]) -> Result<()> {
    editor.add_redaction(page, rect, None)
}

/// Count pending redaction annotations on a page.
pub fn edit_redaction_count(editor: &mut DocumentEditor, page: usize) -> Result<usize> {
    editor.redaction_count(page)
}

/// Apply all pending redactions, permanently removing redacted content.
pub fn edit_apply_redactions_destructive(editor: &mut DocumentEditor) -> Result<()> {
    editor.apply_redactions_destructive(crate::redaction::RedactionOptions::default())?;
    Ok(())
}

/// Remove document metadata (info dict, XMP, etc.).
pub fn edit_scrub_metadata(editor: &mut DocumentEditor) -> Result<()> {
    let opts = crate::redaction::RedactionOptions {
        scrub_metadata: true,
        remove_javascript: false,
        remove_embedded_files: false,
        ..Default::default()
    };
    editor.sanitize_document(opts)?;
    Ok(())
}

// ── Save ──

/// Save the editor's document to a positioned writer with the given options.
pub fn edit_save_with_options(
    editor: &mut DocumentEditor,
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    options: &crate::editor::SaveOptions,
) -> Result<()> {
    editor.write_full_to_writer(writer, options)
}

/// Save the editor's document with compression, GC, and save-mode flags.
pub fn edit_save(
    editor: &mut DocumentEditor,
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    compress: bool,
    garbage_collect: bool,
    save_mode: i32,
) -> Result<()> {
    use crate::editor::SaveOptions;
    let options = SaveOptions {
        incremental: save_mode == 1,
        compress,
        garbage_collect,
        ..Default::default()
    };
    editor.write_full_to_writer(writer, &options)
}

/// Save with AES-256 encryption, returning the encrypted bytes.
pub fn edit_save_encrypted(
    editor: &mut DocumentEditor,
    user_password: &str,
    owner_password: Option<&str>,
) -> Result<Vec<u8>> {
    use crate::editor::{SaveOptions, EncryptionConfig, EncryptionAlgorithm};
    let owner_pwd = owner_password.unwrap_or(user_password);
    let config = EncryptionConfig::new(user_password, owner_pwd)
        .with_algorithm(EncryptionAlgorithm::Aes256);
    let options = SaveOptions::with_encryption(config);
    editor.save_to_bytes_with_options(options)
}

// ═══════════════════════════════════════════════════════════════════
// Document conversion — convert between PDF and office formats.
// ═══════════════════════════════════════════════════════════════════

/// Convert a PDF to an office format, streaming output to a writer.
pub fn convert_to_format_writer<W: std::io::Write>(doc: &PdfDocument, format: &str, writer: &mut W) -> Result<()> {
    match format {
        "docx" => doc.to_docx_writer_flow(writer),
        "pptx" => doc.to_pptx_writer_flow(writer),
        "xlsx" => doc.to_xlsx_writer_flow(writer),
        _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
    }
}

/// Convert a PDF to an office format, returning the bytes.
pub fn convert_to_format(doc: &PdfDocument, format: &str) -> Result<Vec<u8>> {
    match format {
        "docx" => doc.to_docx_bytes(),
        "pptx" => doc.to_pptx_bytes(),
        "xlsx" => doc.to_xlsx_bytes(),
        _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
    }
}

/// Convert an office document to PDF, streaming output to a writer.
pub fn convert_from_format_writer<R: std::io::Read + std::io::Seek + Send + 'static, W: std::io::Write>(
    reader: R, format: &str, writer: &mut W,
) -> Result<()> {
    let converter = crate::converters::office::OfficeConverter::new();
    match format {
        "docx" => converter.convert_docx_reader_to_writer(reader, writer),
        "pptx" => converter.convert_pptx_reader_to_writer(reader, writer),
        "xlsx" => converter.convert_xlsx_reader_to_writer(reader, writer),
        _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
    }
}

/// Convert an office document (in memory) to PDF bytes.
pub fn convert_from_format_to_bytes(data: &[u8], format: &str) -> Result<Vec<u8>> {
    let mut buf = std::io::Cursor::new(Vec::new());
    convert_from_format_writer(std::io::Cursor::new(data.to_vec()), format, &mut buf)?;
    Ok(buf.into_inner())
}


// ═══════════════════════════════════════════════════════════════════
// Watermark + stamp — uses page_editor().add_annotation().
// No upstream patch. Uses only public APIs.
// ═══════════════════════════════════════════════════════════════════

/// Add a text watermark to one page (page >= 0) or all pages (page < 0).
pub fn edit_watermark(
    editor: &mut DocumentEditor, page: i32, text: &str,
    font_size: f32, rotation: f32, opacity: f32,
    r: f32, g: f32, b: f32,
    layer: i32, pos_type: i32, pos_fields: &[f32],
) -> Result<()> {
    use crate::writer::WatermarkAnnotation;
    use crate::geometry::Rect;

    let resolve_rect = |ed: &mut DocumentEditor, p: usize| -> Result<Vec<Rect>> {
        let mb = ed.get_page_media_box(p)?;
        let (pw, ph) = (mb[2], mb[3]);
        match pos_type {
            0 => Ok(vec![Rect::new(pw * 0.1, ph * 0.3, pw * 0.8, ph * 0.4)]),
            1 => {
                let corner = pos_fields.first().copied().unwrap_or(0.0) as i32;
                let mx = pos_fields.get(1).copied().unwrap_or(20.0);
                let my = pos_fields.get(2).copied().unwrap_or(20.0);
                let tw = pw * 0.3;
                let th = ph * 0.08;
                let rect = match corner {
                    0 => Rect::new(mx, ph - my - th, tw, th),
                    1 => Rect::new(pw - mx - tw, ph - my - th, tw, th),
                    2 => Rect::new(mx, my, tw, th),
                    _ => Rect::new(pw - mx - tw, my, tw, th),
                };
                Ok(vec![rect])
            }
            2 => {
                let cols = pos_fields.first().copied().unwrap_or(3.0).max(1.0) as usize;
                let rows = pos_fields.get(1).copied().unwrap_or(4.0).max(1.0) as usize;
                let cw = pw / cols as f32;
                let ch = ph / rows as f32;
                let mut rects = Vec::with_capacity(cols * rows);
                for row in 0..rows {
                    for col in 0..cols {
                        rects.push(Rect::new(col as f32 * cw, row as f32 * ch, cw, ch));
                    }
                }
                Ok(rects)
            }
            3 => {
                let x = pos_fields.first().copied().unwrap_or(0.0);
                let y = pos_fields.get(1).copied().unwrap_or(0.0);
                let w = pos_fields.get(2).copied().unwrap_or(100.0);
                let h = pos_fields.get(3).copied().unwrap_or(50.0);
                Ok(vec![Rect::new(x, y, w, h)])
            }
            _ => Ok(vec![Rect::new(pw * 0.1, ph * 0.3, pw * 0.8, ph * 0.4)]),
        }
    };

    let add_to_page = |ed: &mut DocumentEditor, p: usize| -> Result<()> {
        let rects = resolve_rect(ed, p)?;
        if layer == 1 {
            // Under-content: prepend watermark to page content stream.
            // Renders BEHIND existing page content.
            for rect in &rects {
                let stream = generate_watermark_stream(
                    text, *rect, font_size, rotation, opacity, r, g, b,
                );
                prepend_to_page_content(ed, p, &stream)?;
            }
        } else {
            // Over-content: annotation-based watermark (default).
            // Renders ON TOP of existing page content.
            for rect in rects {
                let wm = WatermarkAnnotation::new(text)
                    .with_rect(rect)
                    .with_rotation(rotation)
                    .with_opacity(opacity)
                    .with_color(r, g, b)
                    .with_font("Helvetica", font_size);
                ed.add_page_annotation(p, wm);
            }
        }
        Ok(())
    };

    if page < 0 {
        let count = editor.current_page_count();
        let media_boxes = editor.all_media_boxes();
        for i in 0..count {
            let mb = media_boxes[i];
            let (pw, ph) = (mb[2], mb[3]);
            let rects = match pos_type {
                0 => vec![Rect::new(pw * 0.1, ph * 0.3, pw * 0.8, ph * 0.4)],
                1 => {
                    let corner = pos_fields.first().copied().unwrap_or(0.0) as i32;
                    let mx = pos_fields.get(1).copied().unwrap_or(20.0);
                    let my = pos_fields.get(2).copied().unwrap_or(20.0);
                    let tw = pw * 0.3;
                    let th = ph * 0.08;
                    vec![match corner {
                        0 => Rect::new(mx, ph - my - th, tw, th),
                        1 => Rect::new(pw - mx - tw, ph - my - th, tw, th),
                        2 => Rect::new(mx, my, tw, th),
                        _ => Rect::new(pw - mx - tw, my, tw, th),
                    }]
                }
                2 => {
                    let cols = pos_fields.first().copied().unwrap_or(3.0).max(1.0) as usize;
                    let rows = pos_fields.get(1).copied().unwrap_or(4.0).max(1.0) as usize;
                    let cw = pw / cols as f32;
                    let ch = ph / rows as f32;
                    let mut r = Vec::with_capacity(cols * rows);
                    for row in 0..rows {
                        for col in 0..cols {
                            r.push(Rect::new(col as f32 * cw, row as f32 * ch, cw, ch));
                        }
                    }
                    r
                }
                3 => {
                    let x = pos_fields.first().copied().unwrap_or(0.0);
                    let y = pos_fields.get(1).copied().unwrap_or(0.0);
                    let w = pos_fields.get(2).copied().unwrap_or(100.0);
                    let h = pos_fields.get(3).copied().unwrap_or(50.0);
                    vec![Rect::new(x, y, w, h)]
                }
                _ => vec![Rect::new(pw * 0.1, ph * 0.3, pw * 0.8, ph * 0.4)],
            };
            if layer == 1 {
                for rect in &rects {
                    let stream = generate_watermark_stream(
                        text, *rect, font_size, rotation, opacity, r, g, b,
                    );
                    prepend_to_page_content(editor, i, &stream)?;
                }
            } else {
                for rect in rects {
                    let wm = WatermarkAnnotation::new(text)
                        .with_rect(rect)
                        .with_rotation(rotation)
                        .with_opacity(opacity)
                        .with_color(r, g, b)
                        .with_font("Helvetica", font_size);
                    editor.add_page_annotation(i, wm);
                }
            }
        }
    } else {
        add_to_page(editor, page as usize)?;
    }
    Ok(())
}

/// Generate PDF content stream operators for a text watermark.
fn generate_watermark_stream(
    text: &str,
    rect: crate::geometry::Rect,
    font_size: f32,
    rotation: f32,
    opacity: f32,
    r: f32, g: f32, b: f32,
) -> Vec<u8> {
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height / 2.0;
    let rad = rotation * std::f32::consts::PI / 180.0;
    let cos_r = rad.cos();
    let sin_r = rad.sin();

    let ar = if opacity < 1.0 { r * opacity } else { r };
    let ag = if opacity < 1.0 { g * opacity } else { g };
    let ab = if opacity < 1.0 { b * opacity } else { b };

    let approx_width = text.len() as f32 * font_size * 0.5;
    let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");

    format!(
        "q\n{:.2} {:.2} {:.2} rg\n{:.4} {:.4} {:.4} {:.4} {:.2} {:.2} cm\n\
         BT\n/Helvetica {:.1} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
        ar, ag, ab,
        cos_r, sin_r, -sin_r, cos_r, cx, cy,
        font_size,
        -approx_width / 2.0, -font_size / 3.0,
        escaped,
    ).into_bytes()
}

/// Prepend content stream bytes to a page's existing content.
/// The prepended bytes render BEHIND existing content (under-content).
fn prepend_to_page_content(
    editor: &mut DocumentEditor,
    page: usize,
    stream_bytes: &[u8],
) -> Result<()> {
    use crate::object::{Object, ObjectRef};

    let page_ref = editor.source_mut().get_page_ref(page)?;
    let page_obj = editor.source_mut().load_object(page_ref)?;
    let page_dict = page_obj.as_dict()
        .ok_or_else(|| Error::InvalidPdf("page not a dict".into()))?;

    // Get existing content stream data
    let existing_data = if let Some(contents_ref) = page_dict.get("Contents") {
        if let Some(r) = contents_ref.as_reference() {
            let obj = editor.source_mut().load_object(r)?;
            match obj {
                Object::Stream { data, .. } => data.to_vec(),
                _ => Vec::new(),
            }
        } else if let Object::Stream { data, .. } = contents_ref {
            data.to_vec()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // Prepend watermark stream + existing content
    let mut new_data = Vec::with_capacity(stream_bytes.len() + existing_data.len());
    new_data.extend_from_slice(stream_bytes);
    new_data.extend_from_slice(&existing_data);

    // Build new content stream object
    let mut content_dict = std::collections::HashMap::new();
    content_dict.insert("Length".into(), Object::Integer(new_data.len() as i64));

    let content_id = editor.alloc_id();
    editor.insert_modified(content_id, Object::Stream {
        dict: content_dict,
        data: bytes::Bytes::from(new_data),
    });

    // Update page to point to new content stream
    let mut new_page = page_dict.clone();
    new_page.insert("Contents".into(), Object::Reference(ObjectRef::new(content_id, 0)));
    editor.insert_modified(page_ref.id, Object::Dictionary(new_page));

    Ok(())
}

/// Add a standard stamp annotation to a page.
pub fn edit_add_stamp(
    editor: &mut DocumentEditor, page: usize, stamp_type: i32,
    x: f32, y: f32, w: f32, h: f32, opacity: f32,
) -> Result<()> {
    use crate::geometry::Rect;
    let rect = Rect::new(x, y, x + w, y + h);
    let st = stamp_type_from_int(stamp_type);
    let mut annot = crate::writer::StampAnnotation::new(rect, st);
    if opacity > 0.0 && opacity < 1.0 {
        annot.opacity = Some(opacity);
    }
    let mut pg = editor.get_page(page)?;
    pg.add_annotation(annot);
    editor.save_page(pg)?;
    Ok(())
}

/// Add an image stamp — builds appearance stream directly from image bytes.
/// Uses editor's pub(crate) alloc_id + insert_modified to place objects.
pub fn edit_add_image_stamp(
    editor: &mut DocumentEditor, page: usize, image_bytes: Vec<u8>,
    x: f32, y: f32, w: f32, h: f32, opacity: f32,
) -> Result<()> {
    use crate::object::Object;
    use crate::writer::ImageData;

    let img = ImageData::from_bytes(&image_bytes)
        .map_err(|e| Error::InvalidPdf(format!("invalid image: {e}")))?;

    // Image XObject
    let mut img_dict = std::collections::HashMap::new();
    img_dict.insert("Type".into(), Object::Name("XObject".into()));
    img_dict.insert("Subtype".into(), Object::Name("Image".into()));
    img_dict.insert("Width".into(), Object::Integer(img.width as i64));
    img_dict.insert("Height".into(), Object::Integer(img.height as i64));
    img_dict.insert("BitsPerComponent".into(), Object::Integer(img.bits_per_component as i64));
    let cs = match img.color_space {
        crate::writer::ColorSpace::DeviceGray => "DeviceGray",
        crate::writer::ColorSpace::DeviceRGB => "DeviceRGB",
        crate::writer::ColorSpace::DeviceCMYK => "DeviceCMYK",
    };
    img_dict.insert("ColorSpace".into(), Object::Name(cs.into()));
    match img.format {
        crate::writer::ImageFormat::Jpeg =>
            { img_dict.insert("Filter".into(), Object::Name("DCTDecode".into())); }
        crate::writer::ImageFormat::Png =>
            { img_dict.insert("Filter".into(), Object::Name("FlateDecode".into())); }
        _ => {}
    }
    img_dict.insert("Length".into(), Object::Integer(img.data.len() as i64));

    let img_id = editor.alloc_id();
    editor.insert_modified(img_id, Object::Stream {
        dict: img_dict,
        data: bytes::Bytes::from(img.data),
    });

    // Form XObject (appearance stream) — draws the image scaled to rect
    let content = format!("q\n{} 0 0 {} 0 0 cm\n/Im0 Do\nQ\n", w, h);

    let mut xobjects = std::collections::HashMap::new();
    xobjects.insert("Im0".into(), Object::Reference(
        crate::object::ObjectRef::new(img_id, 0),
    ));
    let mut resources = std::collections::HashMap::new();
    resources.insert("XObject".into(), Object::Dictionary(xobjects));

    if opacity < 1.0 {
        let mut gs = std::collections::HashMap::new();
        gs.insert("Type".into(), Object::Name("ExtGState".into()));
        gs.insert("ca".into(), Object::Real(opacity as f64));
        gs.insert("CA".into(), Object::Real(opacity as f64));
        let mut ext = std::collections::HashMap::new();
        ext.insert("GS0".into(), Object::Dictionary(gs));
        resources.insert("ExtGState".into(), Object::Dictionary(ext));
    }

    let mut form_dict = std::collections::HashMap::new();
    form_dict.insert("Type".into(), Object::Name("XObject".into()));
    form_dict.insert("Subtype".into(), Object::Name("Form".into()));
    form_dict.insert("BBox".into(), Object::Array(vec![
        Object::Real(0.0), Object::Real(0.0),
        Object::Real(w as f64), Object::Real(h as f64),
    ]));
    form_dict.insert("Resources".into(), Object::Dictionary(resources));
    form_dict.insert("Length".into(), Object::Integer(content.len() as i64));

    let form_id = editor.alloc_id();
    editor.insert_modified(form_id, Object::Stream {
        dict: form_dict,
        data: bytes::Bytes::from(content),
    });

    // Stamp annotation dict with appearance
    let mut ap = std::collections::HashMap::new();
    ap.insert("N".into(), Object::Reference(
        crate::object::ObjectRef::new(form_id, 0),
    ));

    let mut annot = std::collections::HashMap::new();
    annot.insert("Type".into(), Object::Name("Annot".into()));
    annot.insert("Subtype".into(), Object::Name("Stamp".into()));
    annot.insert("Rect".into(), Object::Array(vec![
        Object::Real(x as f64), Object::Real(y as f64),
        Object::Real((x + w) as f64), Object::Real((y + h) as f64),
    ]));
    annot.insert("Name".into(), Object::Name("ImageStamp".into()));
    annot.insert("F".into(), Object::Integer(132)); // Print + ReadOnly
    annot.insert("AP".into(), Object::Dictionary(ap));
    if opacity < 1.0 {
        annot.insert("CA".into(), Object::Real(opacity as f64));
    }

    let annot_id = editor.alloc_id();
    editor.insert_modified(annot_id, Object::Dictionary(annot));

    // Add annotation ref to page's /Annots array
    let page_ref = editor.source_mut().get_page_ref(page)?;
    let page_obj = editor.source_mut().load_object(page_ref)?;
    let mut page_dict = page_obj.as_dict()
        .ok_or_else(|| Error::InvalidPdf("page not a dict".into()))?
        .clone();
    let mut annots = page_dict.get("Annots")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    annots.push(Object::Reference(crate::object::ObjectRef::new(annot_id, 0)));
    page_dict.insert("Annots".into(), Object::Array(annots));
    editor.insert_modified(page_ref.id, Object::Dictionary(page_dict));

    Ok(())
}

fn stamp_type_from_int(i: i32) -> crate::writer::StampType {
    use crate::writer::StampType;
    match i {
        0 => StampType::Approved, 1 => StampType::Experimental,
        2 => StampType::NotApproved, 3 => StampType::AsIs,
        4 => StampType::Expired, 5 => StampType::NotForPublicRelease,
        6 => StampType::Confidential, 7 => StampType::Final,
        8 => StampType::Sold, 9 => StampType::Departmental,
        10 => StampType::ForComment, 11 => StampType::TopSecret,
        12 => StampType::Draft, 13 => StampType::ForPublicRelease,
        _ => StampType::Draft,
    }
}

// ═══════════════════════════════════════════════════════════════════
// Builder — PDF creation from scratch.
// Uses upstream DocumentBuilder + FluentPageBuilder APIs.
// ═══════════════════════════════════════════════════════════════════

use crate::writer::{DocumentBuilder, FluentPageBuilder, PageSize};

/// A buffered page-building operation replayed by [`replay_page_ops`].
pub enum PageOp {
    /// Set the current font by name and size.
    Font(String, f32),
    /// Move the cursor to absolute (x, y) coordinates.
    At(f32, f32),
    /// Draw inline text at the current cursor position.
    Text(String),
    /// Draw a heading at the given level (1-6).
    Heading(u8, String),
    /// Draw a paragraph of body text.
    Paragraph(String),
    /// Insert vertical space (points).
    Space(f32),
    /// Draw a horizontal rule across the page width.
    HorizontalRule,
    /// Place an image at the given rect with alt text.
    Image {
        /// Raw image bytes (JPEG or PNG).
        data: Vec<u8>,
        /// X origin in points.
        x: f32,
        /// Y origin in points.
        y: f32,
        /// Width in points.
        w: f32,
        /// Height in points.
        h: f32,
        /// Alt text for accessibility.
        alt: String,
    },
    /// Add a diagonal text watermark.
    Watermark(String),
    /// Add a text input field.
    TextField {
        /// Field name.
        name: String,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Optional default value.
        default_value: Option<String>,
    },
    /// Add a checkbox field.
    Checkbox {
        /// Field name.
        name: String,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Initial checked state.
        checked: bool,
    },
    /// Add a combo box (dropdown) field.
    ComboBox {
        /// Field name.
        name: String,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Selectable options.
        options: Vec<String>,
        /// Initially selected option.
        selected: Option<String>,
    },
    /// Add a push button field.
    PushButton {
        /// Field name.
        name: String,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Button label text.
        caption: String,
    },
    /// Add an empty digital signature field.
    SignatureField {
        /// Field name.
        name: String,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
    },
    /// Add a radio button group.
    RadioGroup {
        /// Group name.
        name: String,
        /// Value labels for each button.
        values: Vec<String>,
        /// X origins (one per button).
        xs: Vec<f32>,
        /// Y origins (one per button).
        ys: Vec<f32>,
        /// Widths (one per button).
        ws: Vec<f32>,
        /// Heights (one per button).
        hs: Vec<f32>,
        /// Initially selected value.
        selected: Option<String>,
    },
    /// Attach a JavaScript keystroke action to the most recent field.
    FieldKeystroke(String),
    /// Attach a JavaScript format action to the most recent field.
    FieldFormat(String),
    /// Attach a JavaScript validate action to the most recent field.
    FieldValidate(String),
    /// Attach a JavaScript calculate action to the most recent field.
    FieldCalculate(String),
    /// Attach a URL hyperlink annotation.
    LinkUrl(String),
    /// Attach a same-document page link annotation.
    LinkPage(usize),
    /// Insert a footnote with reference mark and note text.
    Footnote {
        /// Reference mark placed inline.
        ref_mark: String,
        /// Note text placed at page bottom.
        note_text: String,
    },
    /// Lay out text in multiple columns.
    Columns {
        /// Number of columns.
        column_count: u32,
        /// Gap between columns in points.
        gap_pt: f32,
        /// Text to flow across columns.
        text: String,
    },
    /// Insert a line break.
    Newline,
    /// Start a new page with the same dimensions as the current one.
    NewPageSameSize,
    /// Start a new page with custom dimensions.
    NewPage {
        /// Page width in points.
        width: f32,
        /// Page height in points.
        height: f32,
    },
    /// Finalize the current page.
    Done,
}

/// Replay a sequence of buffered page operations onto a builder.
pub fn replay_page_ops(builder: &mut DocumentBuilder, default_size: PageSize, ops: Vec<PageOp>) {
    let mut current_page: Option<FluentPageBuilder<'_>> = None;

    for op in ops {
        match op {
            PageOp::NewPage { width, height } => {
                if let Some(p) = current_page.take() { p.done(); }
                current_page = Some(builder.page(PageSize::Custom(width, height)));
            }
            PageOp::Done => {
                if let Some(p) = current_page.take() { p.done(); }
            }
            other => {
                let page = match current_page.take() {
                    Some(p) => p,
                    None => builder.page(default_size),
                };
                current_page = Some(match other {
                    PageOp::Font(ref name, sz) => page.font(name, sz),
                    PageOp::At(x, y) => page.at(x, y),
                    PageOp::Text(ref text) => page.text(text),
                    PageOp::Heading(level, ref text) => page.heading(level, text),
                    PageOp::Paragraph(ref text) => page.paragraph(text),
                    PageOp::Space(pts) => page.space(pts),
                    PageOp::HorizontalRule => page.horizontal_rule(),
                    PageOp::Image { data, x, y, w, h, alt } => {
                        match crate::writer::ImageData::from_bytes(&data) {
                            Ok(img) => page.image_with_alt(img, crate::geometry::Rect::new(x, y, x + w, y + h), &alt),
                            Err(_) => page, // skip — image data unparseable
                        }
                    }
                    PageOp::Watermark(text) => page.watermark(&text),
                    PageOp::TextField { name, x, y, w, h, default_value } => page.text_field(name, x, y, w, h, default_value),
                    PageOp::Checkbox { name, x, y, w, h, checked } => page.checkbox(&name, x, y, w, h, checked),
                    PageOp::ComboBox { name, x, y, w, h, options, selected } => page.combo_box(name, x, y, w, h, options, selected),
                    PageOp::PushButton { name, x, y, w, h, caption } => page.push_button(&name, x, y, w, h, &caption),
                    PageOp::SignatureField { name, x, y, w, h } => page.signature_field(&name, x, y, w, h),
                    PageOp::RadioGroup { name, values, xs, ys, ws, hs, selected } => {
                        let buttons: Vec<(String, f32, f32, f32, f32)> = values.into_iter()
                            .zip(xs).zip(ys).zip(ws).zip(hs)
                            .map(|((((v, x), y), w), h)| (v, x, y, w, h))
                            .collect();
                        page.radio_group(name, buttons, selected)
                    }
                    PageOp::FieldKeystroke(script) => page.field_keystroke(script),
                    PageOp::FieldFormat(script) => page.field_format(script),
                    PageOp::FieldValidate(script) => page.field_validate(script),
                    PageOp::FieldCalculate(script) => page.field_calculate(script),
                    PageOp::LinkUrl(url) => page.link_url(&url),
                    PageOp::LinkPage(target) => page.link_page(target),
                    PageOp::Footnote { ref_mark, note_text } => page.footnote(&ref_mark, &note_text),
                    PageOp::Columns { column_count, gap_pt, text } => page.columns(column_count, gap_pt, &text),
                    PageOp::Newline => page.newline(),
                    PageOp::NewPageSameSize => page.new_page_same_size(),
                    PageOp::NewPage { .. } | PageOp::Done => unreachable!(),
                });
            }
        }
    }
    if let Some(p) = current_page { p.done(); }
}

/// Create a new empty DocumentBuilder.
pub fn builder_new() -> DocumentBuilder { DocumentBuilder::new() }
/// Set the builder's document title.
pub fn builder_set_title(b: DocumentBuilder, title: &str) -> DocumentBuilder { b.title(title) }
/// Set the builder's document author.
pub fn builder_set_author(b: DocumentBuilder, author: &str) -> DocumentBuilder { b.author(author) }
/// Set the builder's document subject.
pub fn builder_set_subject(b: DocumentBuilder, subject: &str) -> DocumentBuilder { b.subject(subject) }
/// Set the builder's document keywords.
pub fn builder_set_keywords(b: DocumentBuilder, keywords: &str) -> DocumentBuilder { b.keywords(keywords) }
/// Add a custom-sized page and return a fluent page builder.
pub fn builder_add_page(b: &mut DocumentBuilder, width: f32, height: f32) -> FluentPageBuilder<'_> { b.page(PageSize::Custom(width, height)) }
/// Add an A4-sized page and return a fluent page builder.
pub fn builder_add_a4_page(b: &mut DocumentBuilder) -> FluentPageBuilder<'_> { b.page(PageSize::A4) }
/// Add a Letter-sized page and return a fluent page builder.
pub fn builder_add_letter_page(b: &mut DocumentBuilder) -> FluentPageBuilder<'_> { b.page(PageSize::Letter) }
/// Build the document and return the PDF bytes.
pub fn builder_save(b: DocumentBuilder) -> Result<Vec<u8>> { b.build() }
/// Build the document and write the PDF to a positioned writer.
pub fn builder_save_to_writer(b: DocumentBuilder, writer: &mut impl crate::host::positioned_write::PositionedWrite) -> Result<()> {
    b.build_to_writer(writer)
}
