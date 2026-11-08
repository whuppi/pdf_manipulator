//! Shared dispatch — the single brain for both native and WASM.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! RULE: Every operation the Flutter bridge can perform is a function here.
//! The op units under `ops/` are the only callers. No other file calls the engine directly.
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
// The op units under `ops/` encode these to binary via ResponseWriter.
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
    /// The /P word as the file stores it, ISO 32000-1 Table 22 bit positions
    /// (-1, every bit set, when the file is not encrypted).
    pub permission_bits: i32,
    /// Document info Title field.
    pub title: String,
    /// Document info Author field.
    pub author: String,
    /// Document info Subject field.
    pub subject: String,
    /// Document info Keywords field.
    pub keywords: String,
    /// Document info Producer field.
    pub producer: String,
    /// Document info Creator field.
    pub creator: String,
    /// Document info CreationDate field (raw PDF date string).
    pub creation_date: String,
    /// Document info ModDate field (raw PDF date string).
    pub mod_date: String,
}

/// Dimensions, boxes and rotation of a single page.
pub struct PageInfo {
    /// The media box's left edge in points.
    pub x: f64,
    /// The media box's bottom edge in points.
    pub y: f64,
    /// Page width in points.
    pub width: f64,
    /// Page height in points.
    pub height: f64,
    /// The page's `/CropBox` as `[x, y, width, height]` in points, when it
    /// has one.
    pub crop_box: Option<[f64; 4]>,
    /// Page rotation in degrees (0, 90, 180, 270).
    pub rotation: i32,
    /// The page's label from the catalog's `/PageLabels`, when it has one.
    pub label: Option<String>,
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
    /// Whether the digest of the covered bytes matches and the signer's
    /// key verifies it.
    pub is_valid: bool,
    /// Whether the signed byte ranges reach the last byte of the file.
    pub covers_whole_document: bool,
    /// Signing time as printed in the signature, in seconds since the
    /// epoch; `None` when absent or unparsable.
    pub signing_time: Option<i64>,
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
    /// Bookmark title, absent when the bookmark has none.
    pub title: Option<String>,
    /// First page index (inclusive).
    pub start_page: usize,
    /// Exclusive end page (0-based); the range is `start_page..end_page`.
    pub end_page: usize,
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
    /// Document creator application.
    pub creator: String,
    /// Document producer.
    pub producer: String,
    /// Document creation date (raw PDF date string).
    pub creation_date: String,
    /// Document modification date (raw PDF date string).
    pub mod_date: String,
}

// ═══════════════════════════════════════════════════════════════════
// Read operations — take &mut PdfDocument, return typed results.
// ═══════════════════════════════════════════════════════════════════

/// Open a document and return its metadata and the strings of its info
/// dictionary. The per-page facts are a read of their own
/// ([`page_info`]): opening a document never pays for them.
pub fn open_document(doc: &mut PdfDocument) -> Result<OpenResult> {
    let page_count = doc.page_count()?;
    let (major, minor) = doc.version();
    let is_encrypted = doc.is_encrypted();
    let requires_password = is_encrypted && !doc.is_authenticated();
    let is_tagged = doc.structure_tree().ok().flatten().is_some();

    // Encrypt-dict accessors return None on unencrypted documents —
    // 0 / 0xFF are the wire's "not encrypted / all permitted" values.
    let enc_algo: u8 = doc.encryption_algorithm().unwrap_or(0);
    let perms: i32 = doc.permission_bits().unwrap_or(-1);

    let title = doc.document_info_string("Title").unwrap_or_default();
    let author = doc.document_info_string("Author").unwrap_or_default();
    let subject = doc.document_info_string("Subject").unwrap_or_default();
    let keywords = doc.document_info_string("Keywords").unwrap_or_default();
    let producer = doc.document_info_string("Producer").unwrap_or_default();
    let creator = doc.document_info_string("Creator").unwrap_or_default();
    let creation_date = doc.document_info_string("CreationDate").unwrap_or_default();
    let mod_date = doc.document_info_string("ModDate").unwrap_or_default();

    Ok(OpenResult {
        page_count, version_major: major, version_minor: minor,
        is_encrypted, requires_password, is_tagged,
        encryption_algorithm: enc_algo, permission_bits: perms,
        title, author, subject, keywords,
        producer, creator, creation_date, mod_date,
    })
}

/// The facts of the pages named: their size, their rotation and their label.
/// The pages are read one at a time, through the bounded object cache, never
/// through `get_page`, whose cache would hold every page dictionary.
pub fn page_info(
    doc: &mut PdfDocument,
    pages: &[usize],
    cancel: &crate::cancel::CancelCheck,
) -> Result<Vec<PageInfo>> {
    use crate::extractors::page_labels::PageLabelExtractor;
    // One read of the /PageLabels number tree; a document without one
    // has no labels, not labels that repeat the page numbers. A label is
    // computed for the pages asked for only, never for the whole document.
    let ranges = PageLabelExtractor::extract(doc).unwrap_or_default();
    let page_refs = doc.all_page_refs()?;
    let mut out = Vec::with_capacity(pages.len());
    for &i in pages {
        cancel.check()?;
        let page_ref = *page_refs
            .get(i)
            .ok_or_else(|| Error::InvalidPdf(format!("Page {i} not found in tree")))?;
        let facts = doc.page_facts(page_ref)?;
        let [x0, y0, x1, y1] = facts.media_box.unwrap_or([0.0, 0.0, 612.0, 792.0]);
        out.push(PageInfo {
            x: x0 as f64,
            y: y0 as f64,
            width: (x1 - x0) as f64,
            height: (y1 - y0) as f64,
            crop_box: facts.crop_box.map(|[cx0, cy0, cx1, cy1]| {
                [cx0 as f64, cy0 as f64, (cx1 - cx0) as f64, (cy1 - cy0) as f64]
            }),
            rotation: facts.rotation(),
            label: (!ranges.is_empty()).then(|| PageLabelExtractor::get_label(&ranges, i)),
        });
    }
    Ok(out)
}

/// Extract text from one page or all pages in the given format.
pub fn extract_text(
    doc: &mut PdfDocument,
    page: Option<usize>,
    format: &str,
    cancel: &crate::cancel::CancelCheck,
) -> Result<ExtractTextResult> {
    #[cfg(not(feature = "extract"))]
    {
        let _ = (doc, page, format, cancel);
        return Err(Error::Unsupported("extract support not enabled in this build".into()));
    }
    #[cfg(feature = "extract")]
    {
        let text = match format {
            "markdown" => {
                let opts = crate::converters::ConversionOptions::default();
                match page {
                    None => doc.to_markdown_all_cancellable(&opts, cancel)?,
                    Some(i) => doc.to_markdown_cancellable(i, &opts, cancel)?,
                }
            }
            "html" => {
                let opts = crate::converters::ConversionOptions::default();
                match page {
                    None => {
                        let count = doc.page_count()?;
                        let mut all = String::new();
                        cancel.each(0..count, |i| {
                            if i > 0 { all.push('\n'); }
                            all.push_str(&doc.to_html_cancellable(i, &opts, cancel)?);
                            Ok(())
                        })?;
                        all
                    }
                    Some(i) => doc.to_html_cancellable(i, &opts, cancel)?,
                }
            }
            _ => {
                match page {
                    None => {
                        let count = doc.page_count()?;
                        let mut all = String::new();
                        cancel.each(0..count, |i| {
                            if i > 0 { all.push('\n'); }
                            all.push_str(&doc.extract_text_cancellable(i, cancel)?);
                            Ok(())
                        })?;
                        all
                    }
                    Some(i) => doc.extract_text_cancellable(i, cancel)?,
                }
            }
        };
        Ok(ExtractTextResult { text })
    }
}

/// Extract text from all pages, one frame per page.
///
/// Each frame body is exactly the bytes the page adds to the output of
/// [`extract_text`] for all pages: page 0 its text, later pages the page
/// separator followed by their text. Joining the frame bodies gives the
/// same string. At most one page of text is in memory at a time.
///
/// Frame format: [len: u32 LE] [binary-encoded response with `text`]
/// End marker:   [0x00000000]
pub fn extract_text_framed<W: std::io::Write>(
    doc: &mut PdfDocument,
    format: &str,
    writer: &mut W,
    cancel: &crate::cancel::CancelCheck,
) -> Result<usize> {
    #[cfg(not(feature = "extract"))]
    {
        let _ = (doc, format, writer, cancel);
        return Err(Error::Unsupported("extract support not enabled in this build".into()));
    }
    #[cfg(feature = "extract")]
    {
        use crate::host::binary_codec::ResponseWriter;
        let separator = match format {
            "markdown" => "\n---\n\n",
            _ => "\n",
        };
        // A locked document yields empty markdown, so it yields no frames.
        let count = if format == "markdown" && doc.text_is_withheld() {
            0
        } else {
            doc.page_count()?
        };
        let opts = crate::converters::ConversionOptions::default();
        cancel.each(0..count, |i| {
            let page_text = match format {
                "markdown" => doc.to_markdown_cancellable(i, &opts, cancel)?,
                "html" => doc.to_html_cancellable(i, &opts, cancel)?,
                _ => doc.extract_text_cancellable(i, cancel)?,
            };
            let mut body = String::with_capacity(separator.len() + page_text.len());
            if i > 0 {
                body.push_str(separator);
            }
            body.push_str(&page_text);
            drop(page_text);
            let mut w = ResponseWriter::ok();
            w.put_str("text", &body);
            let frame = w.finish();
            let len_bytes = (frame.len() as u32).to_le_bytes();
            writer.write_all(&len_bytes).map_err(|e| Error::InvalidPdf(e.to_string()))?;
            writer.write_all(&frame).map_err(|e| Error::InvalidPdf(e.to_string()))?;
            Ok(())
        })?;
        // A zero-length frame terminates the stream.
        writer.write_all(&0u32.to_le_bytes()).map_err(|e| Error::InvalidPdf(e.to_string()))?;
        Ok(count)
    }
}

/// Search for text across the document, optionally filtered to one page.
pub fn search_text(
    doc: &mut PdfDocument,
    query: &str,
    page: Option<usize>,
    cancel: &crate::cancel::CancelCheck,
) -> Result<SearchResult> {
    #[cfg(not(feature = "extract"))]
    {
        let _ = (doc, query, page, cancel);
        return Err(Error::Unsupported("extract support not enabled in this build".into()));
    }
    #[cfg(feature = "extract")]
    {
        use crate::search::{SearchOptions, TextSearcher};
        let opts = SearchOptions::default();
        let all_hits = TextSearcher::search_cancellable(doc, query, &opts, cancel)?;
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
}

/// Enumerate digital signatures in the document, each one's digest and
/// signer key checked against the bytes its `/ByteRange` covers.
pub fn get_signatures(_doc: &mut PdfDocument) -> Result<SignaturesResult> {
    #[cfg(feature = "signatures")]
    {
        let sigs = crate::signatures::enumerate_signatures(_doc)?;
        let statuses = super::verify::verify_all(_doc, &sigs)?;
        let signatures = sigs
            .iter()
            .zip(statuses)
            .map(|(s, status)| SignatureInfo {
                signer_name: s.signer_name.clone().unwrap_or_default(),
                reason: s.reason.clone().unwrap_or_default(),
                location: s.location.clone().unwrap_or_default(),
                is_valid: matches!(status.check, super::verify::SigCheck::Valid),
                covers_whole_document: status.covers_whole_document,
                signing_time: s.signing_time.as_deref().and_then(crate::signatures::parse_pdf_date_to_epoch),
            })
            .collect();
        Ok(SignaturesResult { signatures })
    }
    #[cfg(not(feature = "signatures"))]
    { Ok(SignaturesResult { signatures: vec![] }) }
}

/// Whether the document has at least one signature and every signature
/// verifies: each digest matches the bytes it covers and the signer's
/// key verifies it. Certificate trust and revocation are not checked.
pub fn verify_signatures(doc: &mut PdfDocument) -> Result<bool> {
    #[cfg(feature = "signatures")]
    {
        let sigs = crate::signatures::enumerate_signatures(doc)?;
        if sigs.is_empty() {
            return Ok(false);
        }
        let statuses = super::verify::verify_all(doc, &sigs)?;
        Ok(statuses.iter().all(|s| matches!(s.check, super::verify::SigCheck::Valid)))
    }
    #[cfg(not(feature = "signatures"))]
    {
        let _ = doc;
        Err(Error::Unsupported("signatures support not enabled in this build".into()))
    }
}

/// Validate PDF/A compliance at the given level (1=A1b, 2=A2b, 3=A3b).
pub fn validate_pdf_a(
    doc: &mut PdfDocument,
    level: i32,
    cancel: &crate::cancel::CancelCheck,
) -> Result<ValidationResult> {
    #[cfg(not(feature = "pdfa"))]
    {
        let _ = (doc, level, cancel);
        return Err(Error::Unsupported("PDF/A support not enabled in this build".into()));
    }
    #[cfg(feature = "pdfa")]
    {
    use crate::compliance::{PdfALevel, PdfAValidator};
    let pdf_level = match level {
        1 => PdfALevel::A1b,
        3 => PdfALevel::A3b,
        _ => PdfALevel::A2b,
    };
    let v = PdfAValidator::new().validate_cancellable(doc, pdf_level, cancel)?;
    Ok(ValidationResult {
        compliant: v.is_compliant,
        errors: v.errors.len() as i32,
        warnings: v.warnings.len() as i32,
    })
    }
}

/// Validate PDF/UA compliance at the given level (1=UA1, 2=UA2).
pub fn validate_pdf_ua(
    doc: &mut PdfDocument,
    level: i32,
    cancel: &crate::cancel::CancelCheck,
) -> Result<ValidationResult> {
    #[cfg(not(feature = "pdfa"))]
    {
        let _ = (doc, level, cancel);
        return Err(Error::Unsupported("PDF/A support not enabled in this build".into()));
    }
    #[cfg(feature = "pdfa")]
    {
        use crate::compliance::pdf_ua::{PdfUaLevel, PdfUaValidator};
        let ua_level = match level {
            2 => PdfUaLevel::Ua2,
            _ => PdfUaLevel::Ua1,
        };
        let v = PdfUaValidator::new().validate_cancellable(doc, ua_level, cancel)?;
        Ok(ValidationResult {
            compliant: v.is_compliant,
            errors: v.errors.len() as i32,
            warnings: v.warnings.len() as i32,
        })
    }
}

/// Classify a single page's content type.
pub fn classify_page(doc: &mut PdfDocument, page: usize) -> Result<ClassificationResult> {
    #[cfg(not(feature = "extract"))]
    {
        let _ = (doc, page);
        return Err(Error::Unsupported("extract support not enabled in this build".into()));
    }
    #[cfg(feature = "extract")]
    {
        let classification = doc.classify_page(page)?;
        Ok(ClassificationResult { type_name: format!("{:?}", classification.kind) })
    }
}

/// Classify the overall document type.
pub fn classify_document(
    doc: &mut PdfDocument,
    cancel: &crate::cancel::CancelCheck,
) -> Result<ClassificationResult> {
    #[cfg(not(feature = "extract"))]
    {
        let _ = (doc, cancel);
        return Err(Error::Unsupported("extract support not enabled in this build".into()));
    }
    #[cfg(feature = "extract")]
    {
        let classification = doc.classify_document_cancellable(cancel)?;
        Ok(ClassificationResult { type_name: format!("{:?}", classification.summary) })
    }
}

/// Plan bookmark-based page ranges for splitting the document.
pub fn plan_split_by_bookmarks(
    doc: &mut PdfDocument,
    cancel: &crate::cancel::CancelCheck,
) -> Result<Vec<BookmarkSplit>> {
    use crate::split_bookmarks::{
        plan_split_by_bookmarks_cancellable as do_plan, SplitByBookmarksOptions,
    };
    let opts = SplitByBookmarksOptions::default();
    let splits = do_plan(doc, &opts, cancel)?;
    Ok(splits.iter().map(|s| BookmarkSplit {
        title: s.title.clone(),
        start_page: s.start_page,
        end_page: s.end_page,
    }).collect())
}

// ── XFA ──

/// What a document's XFA packet declares. `has_xfa false` means the
/// document has no XFA at all; the counts are −1 then.
pub struct XfaInfoRow {
    /// Whether the document carries an XFA packet.
    pub has_xfa: bool,
    /// Number of XFA fields, or −1 when there is no XFA.
    pub field_count: i32,
    /// Number of XFA pages, or −1 when there is no XFA.
    pub page_count: i32,
    /// The distinct XFA field types, sorted.
    pub field_types: Vec<String>,
}

/// Read the document's XFA structure without converting it.
pub fn doc_xfa(doc: &mut PdfDocument) -> Result<XfaInfoRow> {
    let analysis = crate::xfa::analyze_xfa_document(doc)?;
    Ok(XfaInfoRow {
        has_xfa: analysis.has_xfa,
        field_count: analysis.field_count.map_or(-1, |n| n as i32),
        page_count: analysis.page_count.map_or(-1, |n| n as i32),
        field_types: analysis.field_types,
    })
}

/// Convert an XFA document into a plain AcroForm document, written to the
/// sink in bounded chunks. Errors when the document has no XFA.
pub fn convert_xfa_to_acroform<W: std::io::Write>(
    doc: &mut PdfDocument,
    writer: &mut W,
) -> Result<()> {
    let bytes = crate::xfa::convert_xfa_document(doc, None)?;
    for chunk in bytes.chunks(256 * 1024) {
        writer.write_all(chunk).map_err(|e| Error::InvalidPdf(e.to_string()))?;
    }
    Ok(())
}

// ── Form fields ──

/// One AcroForm field in wire vocabulary; the field meanings are documented
/// on `crate::extractors::forms::FormField`.
pub struct FormFieldRow {
    /// Fully qualified (dotted) field name.
    pub name: String,
    /// `text|checkbox|radioGroup|comboBox|listBox|button|signature|unknown`.
    pub field_type: &'static str,
    /// `text|checked|choice|multiChoice|none`.
    pub value_kind: &'static str,
    /// Populated when `value_kind == "text"` or `"choice"`.
    pub text: String,
    /// Populated when `value_kind == "checked"`.
    pub checked: bool,
    /// Populated when `value_kind == "multiChoice"`.
    pub choices: Vec<String>,
    /// `/TU`, or `""` for none.
    pub tooltip: String,
    /// Whether the first widget carries a `/Rect`.
    pub has_bounds: bool,
    /// Bounds x, valid only when `has_bounds`.
    pub x: f64,
    /// Bounds y, valid only when `has_bounds`.
    pub y: f64,
    /// Bounds width, valid only when `has_bounds`.
    pub width: f64,
    /// Bounds height, valid only when `has_bounds`.
    pub height: f64,
    /// `/MaxLen`, or −1 for none.
    pub max_length: i32,
    /// `/Q`, or −1 for none.
    pub alignment: i32,
    /// `/Ff` bit 1.
    pub read_only: bool,
    /// `/Ff` bit 2.
    pub required: bool,
}

/// Read every AcroForm field of a document: name, type, value, tooltip,
/// bounds, max length, alignment and the readOnly/required flags.
pub fn doc_form_fields(doc: &mut PdfDocument) -> Result<Vec<FormFieldRow>> {
    use crate::extractors::forms::{FieldType, FieldValue, FormExtractor};

    let fields = FormExtractor::extract_fields(doc)?;
    Ok(fields
        .into_iter()
        .map(|f| {
            let flags = f.flags.unwrap_or(0);
            let field_type: &'static str = match &f.field_type {
                FieldType::Button => {
                    if flags & (1 << 15) != 0 {
                        "radioGroup"
                    } else if flags & (1 << 16) != 0 {
                        "button"
                    } else {
                        "checkbox"
                    }
                }
                FieldType::Text => "text",
                FieldType::Choice => {
                    if flags & (1 << 17) != 0 {
                        "comboBox"
                    } else {
                        "listBox"
                    }
                }
                FieldType::Signature => "signature",
                FieldType::Unknown(_) => "unknown",
            };
            let (value_kind, text, checked, choices): (&'static str, String, bool, Vec<String>) =
                match (&f.field_type, &f.value) {
                    // A choice field's /V is the selected option's export
                    // value, a text string on disk; it is a choice, not text.
                    (FieldType::Choice, FieldValue::Text(s)) => {
                        ("choice", s.clone(), false, Vec::new())
                    }
                    (_, FieldValue::Text(s)) => ("text", s.clone(), false, Vec::new()),
                    // A radio group's /V is the selected option's export
                    // name; "Off" means none, like an unset choice field.
                    (FieldType::Button, FieldValue::Name(n)) if field_type == "radioGroup" => {
                        if n == "Off" {
                            ("none", String::new(), false, Vec::new())
                        } else {
                            ("choice", n.clone(), false, Vec::new())
                        }
                    }
                    (_, FieldValue::Boolean(b)) => ("checked", String::new(), *b, Vec::new()),
                    (FieldType::Button, FieldValue::Name(n)) => {
                        ("checked", String::new(), n != "Off", Vec::new())
                    }
                    (_, FieldValue::Name(n)) => ("choice", n.clone(), false, Vec::new()),
                    (_, FieldValue::Array(arr)) => ("multiChoice", String::new(), false, arr.clone()),
                    (_, FieldValue::None) => ("none", String::new(), false, Vec::new()),
                };
            let (has_bounds, x, y, width, height) = match f.bounds {
                Some([x0, y0, x1, y1]) => (true, x0, y0, x1 - x0, y1 - y0),
                None => (false, 0.0, 0.0, 0.0, 0.0),
            };
            FormFieldRow {
                name: f.full_name,
                field_type,
                value_kind,
                text,
                checked,
                choices,
                tooltip: f.tooltip.unwrap_or_default(),
                has_bounds,
                x,
                y,
                width,
                height,
                max_length: f.max_length.map_or(-1, |n| n as i32),
                alignment: f.alignment.map_or(-1, |n| n as i32),
                read_only: flags & 1 != 0,
                required: flags & 2 != 0,
            }
        })
        .collect())
}

/// Export every AcroForm field's current value as FDF or XFDF, written to
/// the sink in bounded chunks — the document is never held as one blob.
pub fn doc_export_form_data<W: std::io::Write>(
    doc: &mut PdfDocument,
    format: &str,
    writer: &mut W,
) -> Result<()> {
    use crate::extractors::forms::FormExtractor;
    use crate::fdf::{FdfWriter, XfdfWriter};

    let fields = FormExtractor::extract_fields(doc)?;
    let bytes = if format == "fdf" {
        FdfWriter::from_fields(fields).to_bytes()?
    } else {
        XfdfWriter::from_fields(fields).to_bytes()
    };
    for chunk in bytes.chunks(256 * 1024) {
        writer.write_all(chunk).map_err(|e| Error::InvalidPdf(e.to_string()))?;
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Editor operations — take &mut DocumentEditor, return typed results.
// Uses upstream DocumentEditor public + EditableDocument trait methods.
// ═══════════════════════════════════════════════════════════════════

/// Read current metadata from the editor.
pub fn edit_get_metadata(editor: &mut DocumentEditor) -> EditorMetadataResult {
    use crate::editor::EditableDocument;
    let version = editor.version();
    let info = editor.get_info().unwrap_or_default();
    EditorMetadataResult {
        page_count: editor.current_page_count(),
        version_major: version.0,
        version_minor: version.1,
        title: info.title.unwrap_or_default(),
        author: info.author.unwrap_or_default(),
        subject: info.subject.unwrap_or_default(),
        keywords: info.keywords.unwrap_or_default(),
        creator: info.creator.unwrap_or_default(),
        producer: info.producer.unwrap_or_default(),
        creation_date: info.creation_date.unwrap_or_default(),
        mod_date: info.mod_date.unwrap_or_default(),
    }
}

/// Check whether the editor has unsaved modifications.
pub fn edit_is_modified(editor: &DocumentEditor) -> bool {
    editor.is_modified()
}

/// The facts of the editor's pages named, as the edited document has them
/// now: media box, the page's own crop box (absent when it has none),
/// rotation and the label the catalog's `/PageLabels` gives that position.
/// The boxes are converted from corners to an origin and a size so the wire
/// never carries a box a caller could add the origin to twice.
pub fn edit_page_info(
    editor: &mut DocumentEditor,
    pages: &[usize],
    cancel: &crate::cancel::CancelCheck,
) -> Result<Vec<PageInfo>> {
    use crate::extractors::page_labels::PageLabelExtractor;
    let ranges = PageLabelExtractor::extract(editor.source_mut()).unwrap_or_default();
    let mut out = Vec::with_capacity(pages.len());
    for &i in pages {
        cancel.check()?;
        let [x0, y0, x1, y1] = editor.get_page_media_box(i)?;
        let crop_box = editor
            .get_page_crop_box(i)?
            .map(|[cx0, cy0, cx1, cy1]| {
                [cx0 as f64, cy0 as f64, (cx1 - cx0) as f64, (cy1 - cy0) as f64]
            });
        out.push(PageInfo {
            x: x0 as f64,
            y: y0 as f64,
            width: (x1 - x0) as f64,
            height: (y1 - y0) as f64,
            crop_box,
            rotation: editor.get_page_rotation(i)?,
            label: (!ranges.is_empty()).then(|| PageLabelExtractor::get_label(&ranges, i)),
        });
    }
    Ok(out)
}

/// Set the MediaBox of every page named, from an `[x0, y0, x1, y1]`
/// rectangle. The pages are checked before the first is changed.
pub fn edit_set_media_box(
    editor: &mut DocumentEditor,
    pages: &[usize],
    box_: [f32; 4],
) -> Result<()> {
    check_pages(editor, pages)?;
    for &page in pages {
        editor.set_page_media_box(page, box_)?;
    }
    Ok(())
}

/// Set the CropBox of every page named, or remove the page's own crop box
/// when `box_` is `None`. The pages are checked before the first is changed.
pub fn edit_set_crop_box(
    editor: &mut DocumentEditor,
    pages: &[usize],
    box_: Option<[f32; 4]>,
) -> Result<()> {
    check_pages(editor, pages)?;
    for &page in pages {
        match box_ {
            Some(box_) => editor.set_page_crop_box(page, box_)?,
            None => editor.remove_page_crop_box(page)?,
        }
    }
    Ok(())
}

/// Set the absolute rotation (0/90/180/270) of every page named. Unlike
/// `edit_rotate_pages` (relative, additive), this replaces the stored
/// rotation outright. The pages are checked before the first is changed.
pub fn edit_set_rotation(
    editor: &mut DocumentEditor,
    pages: &[usize],
    degrees: i32,
) -> Result<()> {
    check_pages(editor, pages)?;
    for &page in pages {
        editor.set_page_rotation(page, degrees)?;
    }
    Ok(())
}

/// Refuses a page list that is empty, names a page twice or names a page
/// the editor does not have, before any of its pages is changed.
fn check_pages(editor: &DocumentEditor, pages: &[usize]) -> Result<()> {
    if pages.is_empty() {
        return Err(Error::InvalidOperation("pages must not be empty".into()));
    }
    refuse_repeated_pages(pages.iter().copied())?;
    let count = editor.current_page_count();
    match pages.iter().find(|&&p| p >= count) {
        Some(p) => Err(Error::InvalidOperation(format!(
            "page {p} out of range (document has {count} pages)"
        ))),
        None => Ok(()),
    }
}

/// List the image XObjects placed on a page: resource name, placement
/// bounds and the full transform. The name is what `edit_set_image_bounds`
/// takes; an out-of-range page is an error, a page without images an
/// empty list.
pub fn edit_page_images(
    editor: &mut DocumentEditor,
    page: usize,
) -> Result<Vec<crate::editor::ImageInfo>> {
    // Upstream records every `Do` (forms included); the contract is images.
    let placed = editor.get_page_images(page)?;
    let source_page = editor.source_page_index(page)?;
    let page_ref = editor.source().get_page_ref(source_page)?;
    let image_names =
        crate::host::images::inventory::page_image_xobject_names(editor.source(), page_ref)?;
    Ok(placed
        .into_iter()
        .filter(|i| image_names.contains(&i.name))
        .collect())
}

// ── Metadata ──

/// Replace the document's standard info fields with `info`: a field it
/// leaves `None` is removed from the saved document. Dates are raw PDF date
/// strings, e.g. "D:20240101120000Z".
pub fn edit_set_metadata(editor: &mut DocumentEditor, info: crate::editor::DocumentInfo) -> Result<()> {
    use crate::editor::EditableDocument;
    editor.set_info(info)
}

// ── Page manipulation ──

/// Keep only the specified pages (by index), in the order given, removing
/// all others. Keeping no page is refused: a document has at least one.
pub fn edit_select_pages(editor: &mut DocumentEditor, pages: &[usize]) -> Result<()> {
    if pages.is_empty() {
        return Err(Error::InvalidOperation("a document keeps at least one page".into()));
    }
    refuse_repeated_pages(pages.iter().copied())?;
    editor.select_pages(pages)
}

/// Refuses a page list that names a page twice. The save writes a page
/// under its own object id, so a repeat would write one object twice and
/// give it two places in the page tree, which a page cannot have.
pub(crate) fn refuse_repeated_pages(pages: impl IntoIterator<Item = usize>) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    for p in pages {
        crate::cancel::poll()?;
        if !seen.insert(p) {
            return Err(Error::InvalidOperation(format!("page {p} is listed twice")));
        }
    }
    Ok(())
}

/// Delete pages by index. Uses select_pages with the inverse set —
/// no upstream patch needed. Deleting every page is refused.
pub fn edit_delete_pages(editor: &mut DocumentEditor, pages: &[usize]) -> Result<()> {
    check_pages(editor, pages)?;
    let total = editor.current_page_count();
    let keep: Vec<usize> = (0..total).filter(|i| !pages.contains(i)).collect();
    edit_select_pages(editor, &keep)
}

/// Turn every page named by `degrees` on top of its current rotation. The
/// pages are checked before the first is changed.
pub fn edit_rotate_pages(editor: &mut DocumentEditor, pages: &[usize], degrees: i32) -> Result<()> {
    check_pages(editor, pages)?;
    for &page in pages {
        editor.rotate_page_by(page, degrees)?;
    }
    Ok(())
}

/// The degrees a wire rotation name stands for; any other name is `None`.
pub fn rotation_degrees_from_name(name: &str) -> Option<i32> {
    match name {
        "r0" => Some(0),
        "r90" => Some(90),
        "r180" => Some(180),
        "r270" => Some(270),
        _ => None,
    }
}

/// The wire name of a stored page rotation; `None` for a value that is not
/// 0, 90, 180 or 270.
pub fn rotation_name_from_degrees(degrees: i32) -> Option<&'static str> {
    match degrees {
        0 => Some("r0"),
        90 => Some("r90"),
        180 => Some("r180"),
        270 => Some("r270"),
        _ => None,
    }
}

/// The PDF/A level (1, 2 or 3) a wire name stands for; any other name is
/// `None`.
pub fn pdfa_level_from_name(name: &str) -> Option<i32> {
    match name {
        "a1" => Some(1),
        "a2" => Some(2),
        "a3" => Some(3),
        _ => None,
    }
}

/// The PDF/UA level (1 or 2) a wire name stands for; any other name is
/// `None`.
pub fn pdfua_level_from_name(name: &str) -> Option<i32> {
    match name {
        "ua1" => Some(1),
        "ua2" => Some(2),
        _ => None,
    }
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

/// Refuses a page set that names a page the editor cannot edit, before any
/// page is changed: an empty or repeating list, a page the document does not
/// have, and a page merged in, whose content is copied at save.
fn check_editable_pages(editor: &DocumentEditor, pages: &[usize]) -> Result<()> {
    check_pages(editor, pages)?;
    for &page in pages {
        crate::cancel::poll()?;
        editor.source_page_index(page)?;
    }
    Ok(())
}

/// Flatten interactive form fields into page content. `pages == None`
/// flattens every page; `Some(pages)` flattens only those pages' widgets.
/// A page set is checked before the first page is marked.
pub fn edit_flatten_forms(editor: &mut DocumentEditor, pages: Option<&[usize]>) -> Result<()> {
    match pages {
        None => editor.flatten_forms(),
        Some(pages) => {
            check_editable_pages(editor, pages)?;
            pages.iter().try_for_each(|&p| editor.flatten_forms_on_page(p))
        },
    }
}

/// Flatten annotations into page content. `pages == None` flattens every
/// page; `Some(pages)` flattens only those pages' annotations. A page set is
/// checked before the first page is marked.
pub fn edit_flatten_annotations(editor: &mut DocumentEditor, pages: Option<&[usize]>) -> Result<()> {
    match pages {
        None => editor.flatten_all_annotations(),
        Some(pages) => {
            check_editable_pages(editor, pages)?;
            pages.iter().try_for_each(|&p| editor.flatten_page_annotations(p))
        },
    }
}

/// The `reduceImages` knobs as they arrive on the wire (`PdfImagePolicy`);
/// the field meanings are documented on `host::images::policy::Policy`.
pub struct ImagePolicyArgs {
    /// Target ppi for RGB and CMYK images; `None` never downsamples them.
    pub color_ppi: Option<f64>,
    /// Target ppi for gray images.
    pub gray_ppi: Option<f64>,
    /// Target ppi for bilevel images.
    pub mono_ppi: Option<f64>,
    /// Downsample only above target × threshold.
    pub downsample_threshold: f64,
    /// JPEG quality 1–100.
    pub jpeg_quality: u8,
    /// Lossless sources may become JPEG.
    pub allow_lossy: bool,
    /// CMYK output becomes RGB.
    pub convert_cmyk_to_rgb: bool,
    /// Images narrower or shorter than this are kept.
    pub min_pixels: u32,
    /// Minimum saving as a fraction of the stored bytes; 0 = any reduction.
    pub min_savings: f64,
    /// `auto` | `full` | `half` chroma subsampling for written JPEGs.
    pub chroma: String,
    /// Re-encode stored JPEGs at the policy quality even without a downsample.
    pub recompress_jpeg: bool,
}

/// One `reduceImages` report row in wire vocabulary; the field meanings
/// are documented on `host::images::report::Outcome`.
pub struct ImageOutcomeRow {
    /// Object number.
    pub object_id: u32,
    /// `raw|jpeg|jpx|ccitt|jbig2|unknown`.
    pub encoding: &'static str,
    /// `bilevel|gray|rgb|cmyk|other`.
    pub color: &'static str,
    /// Palette indices as stored.
    pub indexed: bool,
    /// Bits per component as stored.
    pub bits: u8,
    /// Pixel width before.
    pub width: u32,
    /// Pixel height before.
    pub height: u32,
    /// An `/SMask` is attached.
    pub soft_mask: bool,
    /// Placements across the visible pages.
    pub uses: u32,
    /// Effective ppi of the most demanding placement.
    pub ppi_min: Option<f64>,
    /// `kept|recompressed|downsampled`.
    pub action: &'static str,
    /// Keep reason, or `""`.
    pub keep_reason: &'static str,
    /// Stored bytes before, parent plus soft mask.
    pub bytes_before: u64,
    /// Stored bytes after.
    pub bytes_after: u64,
    /// Pixel width after.
    pub width_after: u32,
    /// Pixel height after.
    pub height_after: u32,
}

/// Re-encode and downsample the document's images under a policy; one row per image.
pub fn edit_reduce_images(
    editor: &mut DocumentEditor,
    args: ImagePolicyArgs,
    cancel: &crate::cancel::CancelCheck,
) -> Result<Vec<ImageOutcomeRow>> {
    #[cfg(feature = "rendering")]
    {
        use crate::host::images::policy::Policy;
        let policy = Policy {
            color_ppi: args.color_ppi,
            gray_ppi: args.gray_ppi,
            mono_ppi: args.mono_ppi,
            downsample_threshold: args.downsample_threshold,
            jpeg_quality: args.jpeg_quality,
            allow_lossy: args.allow_lossy,
            convert_cmyk_to_rgb: args.convert_cmyk_to_rgb,
            min_pixels: args.min_pixels,
            min_savings: args.min_savings,
            chroma: match args.chroma.as_str() {
                "full" => crate::host::images::policy::Chroma::Full,
                "half" => crate::host::images::policy::Chroma::Half,
                _ => crate::host::images::policy::Chroma::Auto,
            },
            recompress_jpeg: args.recompress_jpeg,
        };
        let report = crate::host::images::execute::reduce_images(editor, &policy, cancel)?;
        Ok(report
            .images
            .into_iter()
            .map(|o| ImageOutcomeRow {
                object_id: o.object_id,
                encoding: o.kind.encoding.wire_name(),
                color: o.kind.color.wire_name(),
                indexed: o.kind.indexed,
                bits: o.kind.bits,
                width: o.kind.width,
                height: o.kind.height,
                soft_mask: o.kind.masks.soft_mask.is_some(),
                uses: o.uses,
                ppi_min: o.ppi_min,
                action: o.action.wire_name(),
                keep_reason: o.keep_reason.map_or("", |r| r.wire_name()),
                bytes_before: o.bytes_before,
                bytes_after: o.bytes_after,
                width_after: o.width_after,
                height_after: o.height_after,
            })
            .collect())
    }
    #[cfg(not(feature = "rendering"))]
    {
        let _ = (editor, args, cancel);
        Ok(Vec::new())
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

/// The data of a file to embed.
pub(crate) enum FileData {
    /// Held in memory.
    Whole(Vec<u8>),
    /// Left in the caller's source: the save reads it from there in pieces.
    Source(crate::host::bridge_api::BoxedReader),
}

/// Embed a file attachment into the document. `description`, `mime_type`
/// and `relationship` are omitted from the file spec when empty.
pub(crate) fn edit_embed_file(
    editor: &mut DocumentEditor,
    name: &str,
    data: FileData,
    description: &str,
    mime_type: &str,
    relationship: &str,
) -> Result<()> {
    use crate::writer::{AFRelationship, EmbeddedFile};

    let mut file = match data {
        FileData::Whole(bytes) => EmbeddedFile::new(name, bytes),
        FileData::Source(reader) => {
            EmbeddedFile::staged(name, crate::host::staged::body_over(reader.0)?)?
        },
    };
    if !description.is_empty() {
        file.description = Some(description.to_string());
    }
    if !mime_type.is_empty() {
        file.mime_type = Some(mime_type.to_string());
    }
    file.af_relationship = match relationship {
        "source" => Some(AFRelationship::Source),
        "data" => Some(AFRelationship::Data),
        "alternative" => Some(AFRelationship::Alternative),
        "supplement" => Some(AFRelationship::Supplement),
        "unspecified" => Some(AFRelationship::Unspecified),
        _ => None,
    };
    editor.embed_file_with_options(file)
}

// ── Attachments ──

/// One attachment of a document in wire vocabulary.
pub struct AttachmentRow {
    /// The file spec's `/UF`, or `/F` when there is no Unicode name.
    pub name: String,
    /// Declared byte size, or −1 when the file spec declares none.
    pub size: i64,
    /// `/Desc`, or `""` for none.
    pub description: String,
    /// MIME type from the stream's `/Subtype`, or `""` for none.
    pub mime_type: String,
}

/// List every attachment in the document's embedded-files name tree.
/// Metadata only — no attachment's bytes are decoded.
pub fn doc_attachments(doc: &mut PdfDocument) -> Result<Vec<AttachmentRow>> {
    Ok(crate::host::attachments::list_attachments(doc)?
        .into_iter()
        .map(|a| AttachmentRow {
            name: a.name,
            size: a.size,
            description: a.description,
            mime_type: a.mime_type,
        })
        .collect())
}

// ── Annotations ──

/// One page annotation in wire vocabulary.
pub struct AnnotationRow {
    /// The `/Subtype` name (e.g. "Text", "Widget"), or `""` when absent.
    pub subtype: String,
    /// `/Rect` as `[x1, y1, x2, y2]` with `x1 <= x2` and `y1 <= y2`,
    /// or `None` when the annotation declares no rectangle.
    pub rect: Option<[f64; 4]>,
    /// `/Contents`, or `""` when absent.
    pub contents: String,
    /// `/T` (author/title), or `""` when absent.
    pub author: String,
    /// `/T` on a widget (form field name), or `""` when absent.
    pub field_name: String,
}

/// List one page's annotations in `/Annots` order.
pub fn doc_annotations(doc: &mut PdfDocument, page: usize) -> Result<Vec<AnnotationRow>> {
    Ok(doc.get_annotations(page)?
        .into_iter()
        .map(|a| AnnotationRow {
            subtype: a.subtype.unwrap_or_default(),
            rect: a.rect.map(|r| [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])]),
            contents: a.contents.unwrap_or_default(),
            // A widget's /T is its field name, never an author.
            author: if a.field_name.is_some() { String::new() } else { a.author.unwrap_or_default() },
            field_name: a.field_name.unwrap_or_default(),
        })
        .collect())
}

/// Write the attachment named `name` to the sink in pieces of 64 KiB.
pub fn doc_extract_attachment<W: std::io::Write>(
    doc: &mut PdfDocument,
    name: &str,
    writer: &mut W,
) -> Result<()> {
    crate::host::attachments::copy_attachment(doc, name, writer)
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

/// Check or clear a checkbox field.
///
/// The string lane above reads a button value as an appearance-state name, so
/// it can already turn a checkbox on when the caller knows the name. This lane
/// exists for the caller who does not: `true` selects whichever on-state the
/// widget offers.
pub fn edit_set_checkbox_field_value(editor: &mut DocumentEditor, name: &str, checked: bool) -> Result<()> {
    use crate::editor::form_fields::FormFieldValue;
    editor.set_form_field_value(name, FormFieldValue::Boolean(checked))
}

/// The form field `name`, as the form reader finds it in the source, with
/// the field's own dictionary.
fn form_field_of(
    editor: &DocumentEditor,
    name: &str,
) -> Result<(
    crate::extractors::forms::FormField,
    std::collections::HashMap<String, crate::object::Object>,
)> {
    use crate::extractors::forms::FormExtractor;

    let field = FormExtractor::extract_fields(editor.source())?
        .into_iter()
        .find(|f| f.full_name == name)
        .ok_or_else(|| Error::InvalidOperation(format!("form field not found: {name}")))?;
    let object_ref = field
        .object_ref
        .ok_or_else(|| Error::InvalidOperation(format!("form field has no object: {name}")))?;
    let dict = editor
        .source()
        .load_object(object_ref)?
        .as_dict()
        .cloned()
        .ok_or_else(|| Error::InvalidPdf(format!("form field {name} is not a dictionary")))?;
    Ok((field, dict))
}

/// Set a choice field's value to one option, named by its export value.
///
/// A combo box takes any of its options; one with the edit flag takes any
/// text. A list box takes one of its options, and a multi-select list box
/// selects just that one. A radio group or checkbox takes one of its states.
/// A value the field does not offer, and a field that is no choice, radio or
/// checkbox, are refused before anything changes.
pub fn edit_set_form_field_choice(editor: &mut DocumentEditor, name: &str, value: &str) -> Result<()> {
    use crate::editor::form_fields::FormFieldValue;
    use crate::extractors::forms::{field_flags, FieldType};
    use crate::host::form_choice::{flags_of, options_of};

    let (field, dict) = form_field_of(editor, name)?;
    match field.field_type {
        FieldType::Button => {
            if field.flags.unwrap_or(0) & field_flags::PUSH_BUTTON != 0 {
                return Err(Error::InvalidOperation(format!(
                    "form field {name} is a push button and has no value"
                )));
            }
            let states = editor.field_on_states(&dict);
            if value != "Off" && !states.iter().any(|s| s == value) {
                return Err(Error::InvalidOperation(format!(
                    "{value:?} is not a state of form field {name}"
                )));
            }
            editor.set_form_field_value(name, FormFieldValue::Choice(value.to_string()))
        },
        FieldType::Choice => {
            let flags = flags_of(editor.source(), &dict);
            let listed = options_of(editor.source(), &dict).iter().any(|o| o.export == value);
            let editable_combo = flags & field_flags::COMBO != 0 && flags & field_flags::EDIT != 0;
            if !listed && !editable_combo {
                return Err(Error::InvalidOperation(format!(
                    "{value:?} is not an option of form field {name}"
                )));
            }
            let multiple = flags & field_flags::COMBO == 0 && flags & field_flags::MULTI_SELECT != 0;
            let new_value = if multiple {
                FormFieldValue::MultiChoice(vec![value.to_string()])
            } else {
                FormFieldValue::Choice(value.to_string())
            };
            editor.set_form_field_value(name, new_value)
        },
        _ => Err(Error::InvalidOperation(format!(
            "form field {name} is not a choice, radio or checkbox field"
        ))),
    }
}

/// Set a multi-select list box's value to the options named by `values`, by
/// export value. No value clears the selection. A field that is no
/// multi-select list box, a value the field does not offer and a value listed
/// twice are refused before anything changes.
pub fn edit_set_form_field_choices(editor: &mut DocumentEditor, name: &str, values: &[String]) -> Result<()> {
    use crate::editor::form_fields::FormFieldValue;
    use crate::extractors::forms::{field_flags, FieldType};
    use crate::host::form_choice::{flags_of, options_of};

    let (field, dict) = form_field_of(editor, name)?;
    let flags = flags_of(editor.source(), &dict);
    if field.field_type != FieldType::Choice
        || flags & field_flags::COMBO != 0
        || flags & field_flags::MULTI_SELECT == 0
    {
        return Err(Error::InvalidOperation(format!(
            "form field {name} is not a multi-select list box"
        )));
    }
    let options = options_of(editor.source(), &dict);
    for (at, value) in values.iter().enumerate() {
        if !options.iter().any(|o| &o.export == value) {
            return Err(Error::InvalidOperation(format!(
                "{value:?} is not an option of form field {name}"
            )));
        }
        if values[..at].contains(value) {
            return Err(Error::InvalidOperation(format!(
                "{value:?} is listed twice for form field {name}"
            )));
        }
    }
    let new_value = if values.is_empty() {
        FormFieldValue::None
    } else {
        FormFieldValue::MultiChoice(values.to_vec())
    };
    editor.set_form_field_value(name, new_value)
}

/// Clear a form field's value: a text or choice field loses its `/V`, a
/// checkbox turns off, a radio group selects nothing.
pub fn edit_clear_form_field_value(editor: &mut DocumentEditor, name: &str) -> Result<()> {
    use crate::editor::form_fields::FormFieldValue;

    form_field_of(editor, name)?;
    editor.set_form_field_value(name, FormFieldValue::None)
}

// ── Form field property mutations ──

/// Remove a form field (and its widget) from the document.
pub fn edit_remove_form_field(editor: &mut DocumentEditor, name: &str) -> Result<()> {
    editor.remove_form_field(name)
}

/// Set a form field's read-only flag.
pub fn edit_set_form_field_readonly(editor: &mut DocumentEditor, name: &str, readonly: bool) -> Result<()> {
    editor.set_form_field_readonly(name, readonly)
}

/// Set a form field's required flag.
pub fn edit_set_form_field_required(editor: &mut DocumentEditor, name: &str, required: bool) -> Result<()> {
    editor.set_form_field_required(name, required)
}

/// Set a form field's tooltip (`/TU`); `None` removes it.
pub fn edit_set_form_field_tooltip(editor: &mut DocumentEditor, name: &str, tooltip: Option<&str>) -> Result<()> {
    match tooltip {
        Some(tooltip) => editor.set_form_field_tooltip(name, tooltip),
        None => editor.remove_form_field_tooltip(name),
    }
}

/// Set a form field's bounding rectangle (`/Rect` of its first widget).
pub fn edit_set_form_field_bounds(
    editor: &mut DocumentEditor,
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<()> {
    editor.set_form_field_rect(name, crate::geometry::Rect::new(x, y, width, height))
}

/// Set a form field's maximum text length (`/MaxLen`); `None` removes it.
pub fn edit_set_form_field_max_length(
    editor: &mut DocumentEditor,
    name: &str,
    max_length: Option<u32>,
) -> Result<()> {
    match max_length {
        Some(max_length) => editor.set_form_field_max_length(name, max_length),
        None => editor.remove_form_field_max_length(name),
    }
}

/// Set a form field's text alignment (`/Q`) from its wire name: `left`,
/// `center` or `right`.
pub fn edit_set_form_field_alignment(editor: &mut DocumentEditor, name: &str, alignment: &str) -> Result<()> {
    let q = alignment_q_from_name(alignment)
        .ok_or_else(|| Error::InvalidOperation(format!("unknown alignment: {alignment}")))?;
    editor.set_form_field_alignment(name, q)
}

/// The `/Q` value (0 left, 1 center, 2 right) of a wire alignment name.
fn alignment_q_from_name(name: &str) -> Option<u32> {
    match name {
        "left" => Some(0),
        "center" => Some(1),
        "right" => Some(2),
        _ => None,
    }
}

/// The wire name of a `/Q` value; `None` for a value outside 0 to 2.
pub fn alignment_name_from_q(q: i32) -> Option<&'static str> {
    match q {
        0 => Some("left"),
        1 => Some("center"),
        2 => Some("right"),
        _ => None,
    }
}

/// Set a form field's background color (`/MK /BG`); `None` removes it.
pub fn edit_set_form_field_background_color(
    editor: &mut DocumentEditor,
    name: &str,
    color: Option<[f32; 3]>,
) -> Result<()> {
    match color {
        Some(color) => editor.set_form_field_background_color(name, color),
        None => editor.remove_form_field_background_color(name),
    }
}

/// Set a form field's border color (`/MK /BC`); `None` removes it.
pub fn edit_set_form_field_border_color(
    editor: &mut DocumentEditor,
    name: &str,
    color: Option<[f32; 3]>,
) -> Result<()> {
    match color {
        Some(color) => editor.set_form_field_border_color(name, color),
        None => editor.remove_form_field_border_color(name),
    }
}

/// Set a form field's border width (`/BS /W`).
pub fn edit_set_form_field_border_width(editor: &mut DocumentEditor, name: &str, width: f32) -> Result<()> {
    editor.set_form_field_border_width(name, width)
}

/// Set a form field's default appearance (`/DA`): font resource name, size
/// and text color. Builds the raw DA string the engine stores — the only
/// place raw DA syntax is written.
pub fn edit_set_form_field_appearance(
    editor: &mut DocumentEditor,
    name: &str,
    font: &str,
    font_size: f64,
    r: f64,
    g: f64,
    b: f64,
) -> Result<()> {
    let da = format!("/{font} {font_size} Tf {r} {g} {b} rg");
    editor.set_form_field_default_appearance(name, da)
}

/// Set a form field's flag bits directly (`/Ff`, OR of `PdfFormFieldFlag`
/// bit values).
pub fn edit_set_form_field_flags(editor: &mut DocumentEditor, name: &str, flags: u32) -> Result<()> {
    editor.set_form_field_flags(name, flags)
}

/// Move and resize a named image XObject on a page in one edit.
pub fn edit_set_image_bounds(
    editor: &mut DocumentEditor,
    page: usize,
    name: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Result<()> {
    editor.set_image_bounds(page, name, x, y, width, height)
}

/// Convert the document to PDF/A at the given level (1=A1b, 2=A2b, 3=A3b).
/// Register a runtime fallback font for form-value baking.
///
/// `kind` is `"cjk"` or `"emoji"`. A later registration replaces the earlier
/// one on the lane that receives it.
pub fn register_fallback_font(kind: &str, bytes: Vec<u8>) -> Result<()> {
    use crate::fonts::form_fallback::Fallback;
    let k = match kind {
        "cjk" => Fallback::Cjk,
        "emoji" => Fallback::Emoji,
        other => {
            return Err(Error::InvalidPdf(format!(
                "unknown fallback font kind '{other}' (expected 'cjk' or 'emoji')"
            )))
        },
    };
    if bytes.is_empty() {
        return Err(Error::InvalidPdf("fallback font bytes are empty".into()));
    }
    crate::host::caches::with(|e| e.register(k, bytes));
    Ok(())
}

/// Convert the editor's document to PDF/A at the given conformance level.
pub fn edit_convert_to_pdf_a(
    editor: &mut DocumentEditor,
    level: i32,
    cancel: &crate::cancel::CancelCheck,
) -> Result<()> {
    #[cfg(not(feature = "pdfa"))]
    {
        let _ = (editor, level, cancel);
        return Err(Error::Unsupported("PDF/A support not enabled in this build".into()));
    }
    #[cfg(feature = "pdfa")]
    {
    use crate::compliance::PdfALevel;
    let pdf_level = match level {
        1 => PdfALevel::A1b,
        3 => PdfALevel::A3b,
        _ => PdfALevel::A2b,
    };
    let converter = crate::compliance::PdfAConverter::new(pdf_level).with_cancel(cancel.clone());
    // The edits are staged and the save writes them. All or nothing: a cancel
    // or an error at any point of the conversion puts the editor back as it was.
    editor.undoable(|editor| converter.convert_with_editor(editor).map(|_| ()))
    }
}

// ── Redaction ──

/// Mark a rectangular region on a page for redaction. `fill` is the overlay
/// colour drawn over it when it is applied; `None` draws no overlay.
pub fn edit_add_redaction(
    editor: &mut DocumentEditor,
    page: usize,
    rect: [f32; 4],
    fill: Option<[f32; 3]>,
) -> Result<()> {
    editor.add_redaction(page, rect, fill)
}

/// Drop every mark queued by `edit_add_redaction`.
pub fn edit_clear_redactions(editor: &mut DocumentEditor) {
    editor.clear_redactions();
}

/// The options every host redaction runs under: a region draws an overlay
/// only when it carries a fill colour, so a mark with none draws no box.
fn redaction_options() -> crate::redaction::RedactionOptions {
    crate::redaction::RedactionOptions {
        draw_overlay_when_no_ic: false,
        ..Default::default()
    }
}

/// Mark `rects` on `page` and apply them, and only them, in one step: the
/// marks queued earlier stay queued and are not applied. A failure leaves
/// nothing marked and nothing applied.
pub fn edit_redact(
    editor: &mut DocumentEditor,
    page: usize,
    rects: &[[f32; 4]],
    fill: Option<[f32; 3]>,
    cancel: &crate::cancel::CancelCheck,
) -> Result<RedactionReportRow> {
    let report = editor.redact_regions(page, rects, fill, &redaction_options(), cancel)?;
    Ok(report.into())
}

/// Count pending redaction annotations on a page.
pub fn edit_redaction_count(editor: &mut DocumentEditor, page: usize) -> Result<usize> {
    editor.redaction_count(page)
}

/// One `applyRedactions` report in wire vocabulary; the field meanings are
/// documented on `crate::redaction::RedactionReport`.
#[derive(Debug)]
pub struct RedactionReportRow {
    /// Number of regions applied.
    pub regions: usize,
    /// Glyphs physically removed from content streams.
    pub glyphs_removed: usize,
    /// Images whose covered pixels were overwritten and re-encoded.
    pub images_modified: usize,
    /// Images deleted entirely (fully covered).
    pub images_removed: usize,
    /// Path subpaths dropped or geometry-clipped.
    pub paths_pruned: usize,
    /// Shared XObjects/patterns/Type3 fonts cloned-and-specialized.
    pub xobjects_specialized: usize,
}

impl From<crate::redaction::RedactionReport> for RedactionReportRow {
    fn from(r: crate::redaction::RedactionReport) -> Self {
        Self {
            regions: r.regions,
            glyphs_removed: r.glyphs_removed,
            images_modified: r.images_modified,
            images_removed: r.images_removed,
            paths_pruned: r.paths_pruned,
            xobjects_specialized: r.xobjects_specialized,
        }
    }
}

/// Apply all pending redactions, permanently removing redacted content.
pub fn edit_apply_redactions(
    editor: &mut DocumentEditor,
    cancel: &crate::cancel::CancelCheck,
) -> Result<RedactionReportRow> {
    let report = editor.apply_redactions_destructive_cancellable(redaction_options(), cancel)?;
    Ok(report.into())
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

/// Sanitize the document per `PdfSanitizeOptions`: scrub metadata, strip
/// JavaScript and/or drop embedded files.
pub fn edit_sanitize(
    editor: &mut DocumentEditor,
    metadata: bool,
    javascript: bool,
    embedded_files: bool,
) -> Result<()> {
    let opts = crate::redaction::RedactionOptions {
        scrub_metadata: metadata,
        remove_javascript: javascript,
        remove_embedded_files: embedded_files,
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
    cancel: &crate::cancel::CancelCheck,
) -> Result<()> {
    if options.incremental {
        return Ok(crate::host::incremental::write(editor, writer, options, cancel)?);
    }
    editor.write_full_to_writer(writer, options, cancel)
}

/// Save the editor's document with compression, GC, and save-mode flags.
pub fn edit_save(
    editor: &mut DocumentEditor,
    writer: &mut impl crate::host::positioned_write::PositionedWrite,
    compress: bool,
    garbage_collect: bool,
    save_mode: i32,
    cancel: &crate::cancel::CancelCheck,
) -> Result<()> {
    use crate::editor::SaveOptions;
    let options = SaveOptions {
        incremental: save_mode == 1,
        compress,
        garbage_collect,
        ..Default::default()
    };
    edit_save_with_options(editor, writer, &options, cancel)
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
    #[cfg(not(feature = "office"))]
    {
        let _ = (doc, format, writer);
        return Err(Error::Unsupported("office support not enabled in this build".into()));
    }
    #[cfg(feature = "office")]
    {
        match format {
            "docx" => doc.to_docx_writer_flow(writer),
            "pptx" => doc.to_pptx_writer_flow(writer),
            "xlsx" => doc.to_xlsx_writer_flow(writer),
            _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
        }
    }
}

/// Convert a PDF to an office format, returning the bytes.
pub fn convert_to_format(doc: &PdfDocument, format: &str) -> Result<Vec<u8>> {
    #[cfg(not(feature = "office"))]
    {
        let _ = (doc, format);
        return Err(Error::Unsupported("office support not enabled in this build".into()));
    }
    #[cfg(feature = "office")]
    {
        match format {
            "docx" => doc.to_docx_bytes(),
            "pptx" => doc.to_pptx_bytes(),
            "xlsx" => doc.to_xlsx_bytes(),
            _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
        }
    }
}

/// Convert an office document to PDF, streaming output to a writer.
pub fn convert_from_format_writer<R: std::io::Read + std::io::Seek + Send + 'static, W: std::io::Write>(
    reader: R, format: &str, writer: &mut W,
) -> Result<()> {
    #[cfg(not(feature = "office"))]
    {
        let _ = (reader, format, writer);
        return Err(Error::Unsupported("office support not enabled in this build".into()));
    }
    #[cfg(feature = "office")]
    {
        let converter = crate::converters::office::OfficeConverter::new();
        match format {
            "docx" => converter.convert_docx_reader_to_writer(reader, writer),
            "pptx" => converter.convert_pptx_reader_to_writer(reader, writer),
            "xlsx" => converter.convert_xlsx_reader_to_writer(reader, writer),
            _ => Err(Error::InvalidPdf(format!("Unknown conversion format: {}", format))),
        }
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

/// The corner a corner-positioned watermark sits in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatermarkCorner {
    /// The page's top-left corner.
    TopLeft,
    /// The page's top-right corner.
    TopRight,
    /// The page's bottom-left corner.
    BottomLeft,
    /// The page's bottom-right corner.
    BottomRight,
}

impl WatermarkCorner {
    /// The corner the wire name (`topLeft`, `topRight`, `bottomLeft`,
    /// `bottomRight`) names.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "topLeft" => Ok(Self::TopLeft),
            "topRight" => Ok(Self::TopRight),
            "bottomLeft" => Ok(Self::BottomLeft),
            "bottomRight" => Ok(Self::BottomRight),
            other => Err(Error::InvalidOperation(format!("unknown watermark corner: {other}"))),
        }
    }
}

/// Where a watermark goes on a page, in the units of the wire: points from
/// the page's lower-left origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WatermarkPosition {
    /// Centred on the page.
    Centre,
    /// In one corner, inset from its two edges.
    Corner {
        /// The corner it sits in.
        corner: WatermarkCorner,
        /// The inset from the left or right edge, in points.
        margin_x: f32,
        /// The inset from the top or bottom edge, in points.
        margin_y: f32,
    },
    /// Repeated in a grid that covers the page.
    Tiled {
        /// The number of copies across.
        columns: u32,
        /// The number of copies down.
        rows: u32,
    },
    /// In one rectangle the caller gives.
    Exact {
        /// The rectangle's left edge, in points.
        x: f32,
        /// The rectangle's bottom edge, in points.
        y: f32,
        /// The rectangle's width, in points.
        width: f32,
        /// The rectangle's height, in points.
        height: f32,
    },
}

impl WatermarkPosition {
    /// The rectangles of a `page_w` by `page_h` page the watermark is drawn in.
    fn rects(&self, page_w: f32, page_h: f32) -> Vec<crate::geometry::Rect> {
        use crate::geometry::Rect;
        match *self {
            Self::Centre => vec![Rect::new(page_w * 0.1, page_h * 0.3, page_w * 0.8, page_h * 0.4)],
            Self::Corner { corner, margin_x, margin_y } => {
                let (w, h) = (page_w * 0.3, page_h * 0.08);
                vec![match corner {
                    WatermarkCorner::TopLeft => Rect::new(margin_x, page_h - margin_y - h, w, h),
                    WatermarkCorner::TopRight => {
                        Rect::new(page_w - margin_x - w, page_h - margin_y - h, w, h)
                    },
                    WatermarkCorner::BottomLeft => Rect::new(margin_x, margin_y, w, h),
                    WatermarkCorner::BottomRight => {
                        Rect::new(page_w - margin_x - w, margin_y, w, h)
                    },
                }]
            },
            Self::Tiled { columns, rows } => {
                let (columns, rows) = (columns.max(1), rows.max(1));
                let (cell_w, cell_h) = (page_w / columns as f32, page_h / rows as f32);
                let mut rects = Vec::with_capacity((columns * rows) as usize);
                for row in 0..rows {
                    for col in 0..columns {
                        rects.push(Rect::new(col as f32 * cell_w, row as f32 * cell_h, cell_w, cell_h));
                    }
                }
                rects
            },
            Self::Exact { x, y, width, height } => vec![Rect::new(x, y, width, height)],
        }
    }
}

/// Add a text watermark to the pages named, or to every page when `pages`
/// is `None`. `layer` is the wire name `foreground` or `background`.
pub fn edit_watermark(
    editor: &mut DocumentEditor, pages: Option<&[usize]>, text: &str,
    font_size: f32, rotation: f32, opacity: f32,
    r: f32, g: f32, b: f32,
    layer: &str, position: WatermarkPosition,
) -> Result<()> {
    use crate::writer::WatermarkAnnotation;

    let background = match layer {
        "foreground" => false,
        "background" => true,
        other => {
            return Err(Error::InvalidOperation(format!("unknown watermark layer: {other}")))
        }
    };

    let add_to_page = |ed: &mut DocumentEditor, p: usize, mb: [f32; 4]| -> Result<()> {
        let rects = position.rects(mb[2], mb[3]);
        if background {
            // Under-content: prepend watermark to page content stream.
            // Renders BEHIND existing page content.
            let font_res = ensure_page_font(ed, p, "Helvetica")?;
            for rect in &rects {
                let stream = generate_watermark_stream(
                    text, *rect, font_size, rotation, opacity, r, g, b,
                    &font_res,
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
                ed.add_page_annotation(p, wm)?;
            }
        }
        Ok(())
    };

    // Everything that can fail on a page is read before the first page is
    // changed, so an error on any page leaves the editor as it was.
    let check_page = |ed: &mut DocumentEditor, p: usize| -> Result<()> {
        if background {
            plan_page_font(ed, p, "Helvetica").map(|_| ())
        } else {
            ed.source_page_index(p).map(|_| ())
        }
    };

    let targets: Vec<(usize, [f32; 4])> = match pages {
        Some(pages) => {
            check_pages(editor, pages)?;
            let mut targets = Vec::with_capacity(pages.len());
            for &page in pages {
                crate::cancel::poll()?;
                let mb = editor.get_page_media_box(page)?;
                check_page(editor, page)?;
                targets.push((page, mb));
            }
            targets
        },
        None => {
            let count = editor.current_page_count();
            for page in 0..count {
                check_page(editor, page)?;
            }
            editor.all_media_boxes().into_iter().enumerate().collect()
        },
    };
    editor.undoable(|editor| {
        for (page, mb) in targets {
            add_to_page(editor, page, mb)?;
        }
        Ok(())
    })
}

/// Generate PDF content stream operators for a text watermark.
fn generate_watermark_stream(
    text: &str,
    rect: crate::geometry::Rect,
    font_size: f32,
    rotation: f32,
    opacity: f32,
    r: f32, g: f32, b: f32,
    font_res: &str,
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
         BT\n/{} {:.1} Tf\n{:.2} {:.2} Td\n({}) Tj\nET\nQ\n",
        ar, ag, ab,
        cos_r, sin_r, -sin_r, cos_r, cx, cy,
        font_res,
        font_size,
        -approx_width / 2.0, -font_size / 3.0,
        escaped,
    ).into_bytes()
}

/// Ensure the page's `/Resources/Font` maps a resource name to
/// `base_font` (a standard Type1 font), staging the updated page dict.
/// Returns the resource name the content stream must use in `Tf`.
///
/// A `Tf` operand is a RESOURCE name resolved through the page's
/// `/Resources/Font` — not a typeface name. Emitting a stream without
/// registering its font produces spec-invalid output that extractors
/// and strict viewers cannot decode.
fn ensure_page_font(
    editor: &mut DocumentEditor,
    page: usize,
    base_font: &str,
) -> Result<String> {
    let plan = plan_page_font(editor, page, base_font)?;
    Ok(apply_page_font(editor, plan, base_font))
}

/// What [`ensure_page_font`] found on a page: everything it reads, so the
/// part that can fail runs without staging anything.
struct PageFontPlan {
    page_ref: crate::object::ObjectRef,
    page_dict: std::collections::HashMap<String, crate::object::Object>,
    resources: std::collections::HashMap<String, crate::object::Object>,
    fonts: std::collections::HashMap<String, crate::object::Object>,
    /// The resource name of a registration of the font the page already has.
    found: Option<String>,
}

/// Reads the page, its resources and its fonts for [`ensure_page_font`],
/// staging nothing; every failure of the font step is raised here.
fn plan_page_font(
    editor: &mut DocumentEditor,
    page: usize,
    base_font: &str,
) -> Result<PageFontPlan> {
    use crate::object::Object;

    let page_ref = editor.source_mut().get_page_ref(page)?;
    // Staged-preferred: an earlier edit on this editor may already have
    // replaced the page dict; loading from source would drop that work.
    let page_obj = if let Some(staged) =
        editor.modified_objects_mut().get(&page_ref.id)
    {
        staged.clone()
    } else {
        editor.source_mut().load_object(page_ref)?
    };
    let page_dict = match page_obj.as_dict() {
        Some(d) => d.clone(),
        None => return Err(Error::InvalidPdf("page not a dict".into())),
    };

    // Resolve a possibly-referenced dict value to an owned dict,
    // preferring staged objects over source (same staleness rule).
    fn deref_dict(
        editor: &mut DocumentEditor,
        value: Option<&Object>,
    ) -> Result<std::collections::HashMap<String, Object>> {
        match value {
            Some(Object::Dictionary(d)) => Ok(d.clone()),
            Some(Object::Reference(r)) => {
                let r = *r;
                let obj = if let Some(staged) =
                    editor.modified_objects_mut().get(&r.id)
                {
                    staged.clone()
                } else {
                    editor.source_mut().load_object(r)?
                };
                Ok(match obj {
                    Object::Dictionary(d) => d,
                    _ => std::collections::HashMap::new(),
                })
            }
            _ => Ok(std::collections::HashMap::new()),
        }
    }

    let resources = deref_dict(editor, page_dict.get("Resources"))?;
    let fonts = deref_dict(editor, resources.get("Font"))?;

    // Reuse an existing registration of the same standard font.
    for (name, value) in fonts.clone() {
        crate::cancel::poll()?;
        let font_obj = match value {
            Object::Dictionary(d) => Object::Dictionary(d),
            Object::Reference(r) => {
                if let Some(staged) = editor.modified_objects_mut().get(&r.id)
                {
                    staged.clone()
                } else {
                    match editor.source_mut().load_object(r) {
                        Ok(o) => o,
                        Err(_) => continue, // unreadable entry: skip, register fresh
                    }
                }
            }
            _ => continue,
        };
        if let Some(d) = font_obj.as_dict() {
            if d.get("BaseFont").and_then(|o| o.as_name()) == Some(base_font)
                && d.get("Subtype").and_then(|o| o.as_name()) == Some("Type1")
            {
                return Ok(PageFontPlan {
                    page_ref,
                    page_dict,
                    resources,
                    fonts,
                    found: Some(name),
                });
            }
        }
    }

    Ok(PageFontPlan { page_ref, page_dict, resources, fonts, found: None })
}

/// Stages what [`plan_page_font`] found: nothing when the page already has
/// the font, else a new font object and the page dict that names it. Cannot
/// fail.
fn apply_page_font(editor: &mut DocumentEditor, plan: PageFontPlan, base_font: &str) -> String {
    use crate::object::{Object, ObjectRef};
    let PageFontPlan { page_ref, page_dict, mut resources, mut fonts, found } = plan;
    if let Some(name) = found {
        return name;
    }

    // Register fresh under a collision-free name.
    let mut n = 0usize;
    let name = loop {
        let candidate = format!("WMF{n}");
        if !fonts.contains_key(&candidate) {
            break candidate;
        }
        n += 1;
    };

    let mut font = std::collections::HashMap::new();
    font.insert("Type".into(), Object::Name("Font".into()));
    font.insert("Subtype".into(), Object::Name("Type1".into()));
    font.insert("BaseFont".into(), Object::Name(base_font.into()));
    font.insert("Encoding".into(), Object::Name("WinAnsiEncoding".into()));
    let font_id = editor.alloc_id();
    editor.insert_modified(font_id, Object::Dictionary(font));

    fonts.insert(name.clone(), Object::Reference(ObjectRef::new(font_id, 0)));
    // Inline the merged dicts directly on the page. If Resources was a
    // reference it may be SHARED across pages — staging the referenced
    // object would leak this font into every sharing page; an inline
    // copy scopes the change to this page only.
    resources.insert("Font".into(), Object::Dictionary(fonts));
    let mut new_page = page_dict;
    new_page.insert("Resources".into(), Object::Dictionary(resources));
    editor.insert_modified(page_ref.id, Object::Dictionary(new_page));
    name
}

/// Prepend a content stream to a page so it renders BEHIND the
/// existing content (under-content).
///
/// The new stream is staged as its OWN object and /Contents becomes an
/// array `[new, ...existing]` — never a byte-concatenation. The
/// existing stream is routinely compressed (its dict carries /Filter);
/// splicing plaintext operators onto raw compressed bytes produces a
/// stream that decodes to garbage past the prepended part, silently
/// destroying the page's original content. The array form is the
/// spec's mechanism for exactly this: elements render as one stream
/// with whitespace between, and the original bytes are never touched.
fn prepend_to_page_content(
    editor: &mut DocumentEditor,
    page: usize,
    stream_bytes: &[u8],
) -> Result<()> {
    use crate::object::{Object, ObjectRef};

    let page_ref = editor.source_mut().get_page_ref(page)?;
    // Staged-preferred: ensure_page_font (and any earlier prepend) stages
    // an updated page dict; loading from source would drop that work and
    // re-resolve a stale Contents value.
    let page_obj = if let Some(staged) =
        editor.modified_objects_mut().get(&page_ref.id)
    {
        staged.clone()
    } else {
        editor.source_mut().load_object(page_ref)?
    };
    let page_dict = page_obj.as_dict()
        .ok_or_else(|| Error::InvalidPdf("page not a dict".into()))?;

    // Stage the new under-content stream as its own object.
    let mut content_dict = std::collections::HashMap::new();
    content_dict.insert("Length".into(), Object::Integer(stream_bytes.len() as i64));
    let content_id = editor.alloc_id();
    editor.insert_modified(content_id, Object::stream(
        content_dict,
        bytes::Bytes::from(stream_bytes.to_vec()),
    ));
    let new_ref = Object::Reference(ObjectRef::new(content_id, 0));

    // /Contents = [new, ...existing]. A direct (non-reference) existing
    // stream is hoisted into its own object first — array elements must
    // be references.
    let contents = match page_dict.get("Contents") {
        None => new_ref,
        Some(Object::Array(existing)) => {
            let mut arr = Vec::with_capacity(existing.len() + 1);
            arr.push(new_ref);
            arr.extend(existing.iter().cloned());
            Object::Array(arr)
        }
        Some(r @ Object::Reference(_)) => {
            Object::Array(vec![new_ref, r.clone()])
        }
        Some(direct @ Object::Stream { .. }) => {
            let hoisted_id = editor.alloc_id();
            editor.insert_modified(hoisted_id, direct.clone());
            Object::Array(vec![
                new_ref,
                Object::Reference(ObjectRef::new(hoisted_id, 0)),
            ])
        }
        Some(_) => new_ref, // malformed /Contents: the overlay becomes the content
    };

    let mut new_page = page_dict.clone();
    new_page.insert("Contents".into(), contents);
    editor.insert_modified(page_ref.id, Object::Dictionary(new_page));

    Ok(())
}

/// Add a standard stamp annotation to every page of `pages`. `stamp_type` is
/// the wire name (`Approved`, `Draft`, ...). The stamp type and every page are
/// checked, and every page's stamp is built, before the first page is saved,
/// so an error leaves no page stamped.
pub fn edit_add_stamp(
    editor: &mut DocumentEditor, pages: &[usize], stamp_type: &str,
    x: f32, y: f32, w: f32, h: f32, opacity: f32,
) -> Result<()> {
    use crate::geometry::Rect;
    // `Rect::new` takes (x, y, width, height) — passing the far corner
    // makes the stamp's /Rect and its appearance BBox grow with the
    // placement, so the label lands off-page for any y above the middle.
    let rect = Rect::new(x, y, w, h);
    let st = stamp_type_from_name(stamp_type)
        .ok_or_else(|| Error::InvalidOperation(format!("unknown stamp type: {stamp_type}")))?;
    check_editable_pages(editor, pages)?;
    let mut built = Vec::with_capacity(pages.len());
    for &page in pages {
        crate::cancel::poll()?;
        let mut annot = crate::writer::StampAnnotation::new(rect, st.clone());
        if opacity > 0.0 && opacity < 1.0 {
            annot.opacity = Some(opacity);
        }
        let mut pg = editor.get_page(page)?;
        pg.add_annotation(annot);
        built.push(pg);
    }
    for pg in built {
        editor.save_page(pg)?;
    }
    Ok(())
}

/// Add an image stamp to every page of `pages` — builds the appearance stream
/// directly from the image, once: the pages share one image and one
/// appearance, each with an annotation of its own. All or nothing: the image is
/// read and every page checked before anything is staged, and an error
/// after that puts the editor back as it was. An image left in its source
/// stays there until the save.
pub fn edit_add_image_stamp(
    editor: &mut DocumentEditor, pages: &[usize], image: impl Into<ImageBytes>,
    x: f32, y: f32, w: f32, h: f32, opacity: f32,
) -> Result<()> {
    use crate::writer::ImageData;

    let staged = match image.into() {
        ImageBytes::Whole(bytes) => {
            let img = ImageData::from_bytes(&bytes)
                .map_err(|e| Error::InvalidPdf(format!("invalid image: {e}")))?;
            crate::host::image_embed::Staged {
                image: (img.build_xobject_dict(), img.data.clone().into()),
                mask: img.build_soft_mask_dict().zip(img.soft_mask.clone()).map(|(dict, data)| (dict, data.into())),
            }
        },
        ImageBytes::Streamed(image) => image.stage()?,
    };
    check_editable_pages(editor, pages)?;
    editor.undoable(|editor| stage_image_stamps(editor, pages, staged, x, y, w, h, opacity))
}

/// The staging of [`edit_add_image_stamp`].
fn stage_image_stamps(
    editor: &mut DocumentEditor, pages: &[usize], staged: crate::host::image_embed::Staged,
    x: f32, y: f32, w: f32, h: f32, opacity: f32,
) -> Result<()> {
    use crate::object::Object;

    // The dictionary carries the PNG DecodeParms/Predictor=15 that the
    // per-row filter bytes require; a hand-rolled FlateDecode dict without
    // it misreads every scanline.
    let (mut img_dict, img_body) = staged.image;

    // A PNG alpha channel must travel as a separate grayscale /SMask, or
    // transparent pixels paint solid black over the page.
    if let Some((smask_dict, smask_body)) = staged.mask {
        let smask_id = editor.alloc_id();
        editor.insert_modified(smask_id, Object::Stream { dict: smask_dict, body: smask_body });
        img_dict.insert(
            "SMask".into(),
            Object::Reference(crate::object::ObjectRef::new(smask_id, 0)),
        );
    }

    let img_id = editor.alloc_id();
    editor.insert_modified(img_id, Object::Stream { dict: img_dict, body: img_body });

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
    editor.insert_modified(form_id, Object::stream(form_dict, bytes::Bytes::from(content)));

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

    for &page in pages {
        crate::cancel::poll()?;
        let annot_id = editor.alloc_id();
        editor.insert_modified(annot_id, Object::Dictionary(annot.clone()));

        // Add annotation ref to the page's /Annots array. The page is the
        // source page the output index names, and its dictionary is the one
        // an earlier edit staged, if any.
        let source_page = editor.source_page_index(page)?;
        let page_ref = editor.source_mut().get_page_ref(source_page)?;
        let page_obj = match editor.modified_objects_mut().get(&page_ref.id) {
            Some(staged) => staged.clone(),
            None => editor.source_mut().load_object(page_ref)?,
        };
        let mut page_dict = page_obj.as_dict()
            .ok_or_else(|| Error::InvalidPdf("page not a dict".into()))?
            .clone();
        // /Annots may be a direct array OR an indirect reference to one (ISO
        // 32000-1 §7.3.10 — any object may be indirect). Resolve both: `as_array`
        // alone returns None for the indirect form, so a naive read drops every
        // existing widget/link on the page and leaves only the new stamp.
        // Detach the object from page_dict first so load_object's &mut borrow of
        // the editor doesn't overlap the page_dict borrow.
        let annots_obj = page_dict.get("Annots").cloned();
        let mut annots = match annots_obj {
            Some(Object::Array(items)) => items,
            Some(Object::Reference(reference)) => editor
                .source_mut()
                .load_object(reference)
                .ok()
                .and_then(|o| o.as_array().cloned())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        annots.push(Object::Reference(crate::object::ObjectRef::new(annot_id, 0)));
        // Write the resolved list back inline; the old indirect array object is
        // orphaned harmlessly (the page no longer references it).
        page_dict.insert("Annots".into(), Object::Array(annots));
        editor.insert_modified(page_ref.id, Object::Dictionary(page_dict));
    }

    Ok(())
}

/// The standard stamp a wire name stands for; any other name is `None`.
fn stamp_type_from_name(name: &str) -> Option<crate::writer::StampType> {
    use crate::writer::StampType;
    Some(match name {
        "Approved" => StampType::Approved,
        "Experimental" => StampType::Experimental,
        "NotApproved" => StampType::NotApproved,
        "AsIs" => StampType::AsIs,
        "Expired" => StampType::Expired,
        "NotForPublicRelease" => StampType::NotForPublicRelease,
        "Confidential" => StampType::Confidential,
        "Final" => StampType::Final,
        "Sold" => StampType::Sold,
        "Departmental" => StampType::Departmental,
        "ForComment" => StampType::ForComment,
        "TopSecret" => StampType::TopSecret,
        "Draft" => StampType::Draft,
        "ForPublicRelease" => StampType::ForPublicRelease,
        _ => return None,
    })
}

// ═══════════════════════════════════════════════════════════════════
// Builder — PDF creation from scratch.
// Uses upstream DocumentBuilder + FluentPageBuilder APIs.
// ═══════════════════════════════════════════════════════════════════

use crate::host::image_embed::ImageBytes;
use crate::writer::{DocumentBuilder, FluentPageBuilder, PageSize};

/// A parsed page-building operation, applied by [`apply_page_op`].
#[derive(Clone)]
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
        /// The image, whole or left in its source.
        data: ImageBytes,
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
    /// Draw inline text at the current position without advancing the line.
    Inline(String),
    /// Write a styled text or inline run: the font/size/bold/italic/colour
    /// overrides apply to this run only — the page's font and size are
    /// restored afterwards. `inline = true` writes at the cursor without
    /// stepping a line; false follows `Text`'s line semantics.
    StyledText {
        /// The run's text.
        text: String,
        /// Font name override for this run.
        font: Option<String>,
        /// Font size override in points.
        size: Option<f32>,
        /// Draw the run in the bold face of its font.
        bold: bool,
        /// Draw the run in the italic face of its font.
        italic: bool,
        /// RGB colour override.
        color: Option<(f32, f32, f32)>,
        /// Write at the cursor without stepping a line.
        inline: bool,
    },
    /// Draw wrapped, horizontally-aligned text inside an exact rectangle.
    TextInRect {
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Text to wrap and place.
        text: String,
        /// Horizontal alignment of each line.
        align: crate::writer::TextAlign,
    },
    /// Draw a filled rectangle primitive.
    FilledRect(f32, f32, f32, f32, f32, f32, f32),
    /// Draw a stroked rectangle.
    StrokeRect {
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Stroke width in points.
        width: f32,
        /// Red component (0.0-1.0).
        r: f32,
        /// Green component (0.0-1.0).
        g: f32,
        /// Blue component (0.0-1.0).
        b: f32,
    },
    /// Draw a stroked rectangle with a dash pattern.
    StrokeRectDashed {
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Stroke width in points.
        width: f32,
        /// Red component (0.0-1.0).
        r: f32,
        /// Green component (0.0-1.0).
        g: f32,
        /// Blue component (0.0-1.0).
        b: f32,
        /// On/off dash lengths in points.
        dash: Vec<f32>,
        /// Dash phase in points.
        phase: f32,
    },
    /// Draw a stroked line.
    StrokeLine {
        /// Start x.
        x1: f32,
        /// Start y.
        y1: f32,
        /// End x.
        x2: f32,
        /// End y.
        y2: f32,
        /// Stroke width in points.
        width: f32,
        /// Red component (0.0-1.0).
        r: f32,
        /// Green component (0.0-1.0).
        g: f32,
        /// Blue component (0.0-1.0).
        b: f32,
    },
    /// Draw a stroked line with a dash pattern.
    StrokeLineDashed {
        /// Start x.
        x1: f32,
        /// Start y.
        y1: f32,
        /// End x.
        x2: f32,
        /// End y.
        y2: f32,
        /// Stroke width in points.
        width: f32,
        /// Red component (0.0-1.0).
        r: f32,
        /// Green component (0.0-1.0).
        g: f32,
        /// Blue component (0.0-1.0).
        b: f32,
        /// On/off dash lengths in points.
        dash: Vec<f32>,
        /// Dash phase in points.
        phase: f32,
    },
    /// Add a highlight annotation to the last text element.
    Highlight(f32, f32, f32),
    /// Add an underline annotation to the last text element.
    Underline(f32, f32, f32),
    /// Add a strikeout annotation to the last text element.
    Strikeout(f32, f32, f32),
    /// Add a squiggly annotation to the last text element.
    Squiggly(f32, f32, f32),
    /// Add a sticky note at the current cursor position.
    StickyNote(String),
    /// Add a sticky note at explicit coordinates.
    StickyNoteAt(f32, f32, String),
    /// Add a stamp annotation at the current cursor position.
    Stamp(crate::writer::StampType),
    /// Add a FreeText annotation inside an exact rectangle.
    FreeText {
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
        /// Note text shown on the page.
        text: String,
    },
    /// Add a named-destination link to the last text element.
    LinkNamed(String),
    /// Add a JavaScript link to the last text element.
    LinkJavaScript(String),
    /// Set the page-open JavaScript action.
    OnOpen(String),
    /// Set the page-close JavaScript action.
    OnClose(String),
    /// Place an image as an artifact (marked, excluded from reading
    /// order), at the given rect.
    ImageArtifact {
        /// The image, whole or left in its source.
        data: ImageBytes,
        /// X origin.
        x: f32,
        /// Y origin.
        y: f32,
        /// Width.
        w: f32,
        /// Height.
        h: f32,
    },
    /// Replay a streaming table block with per-column widths and
    /// extended sizing modes.
    StreamingTableBlock {
        /// Header, width (`None` = the writer's default, measured over by
        /// the sample mode) and alignment per column.
        config_columns: Vec<(String, Option<f32>, crate::writer::CellAlign)>,
        /// Repeat the header row on each page.
        repeat_header: bool,
        /// Draw the header row at all.
        has_header: bool,
        /// Row-major `(text, rowspan)` cells.
        rows: Vec<Vec<(String, usize)>>,
        /// One of `fixed`, `sample`.
        mode: String,
        /// Rows sampled for column sizing in `sample` mode.
        sample_rows: usize,
        /// Lower clamp on measured column widths (points).
        min_col_width_pt: f32,
        /// Upper clamp on measured column widths (points).
        max_col_width_pt: f32,
        /// Maximum rowspan.
        max_rowspan: usize,
    },
    /// Start a new page with the same dimensions as the current one.
    NewPageSameSize,
}

/// Apply a page operation to the builder's current page at once. With no
/// current page, a default A4 page is opened first. An engine error (bad
/// image bytes, a streaming-table row the engine refuses) is returned; ops
/// applied before it stay applied.
pub fn apply_page_op(builder: &mut DocumentBuilder, op: PageOp) -> Result<()> {
    let index = match builder.current_page_index() {
        Some(index) => index,
        None => builder.begin_page(PageSize::A4),
    };
    let mut page = builder.page_mut(index);
    match op {
        PageOp::Font(ref name, sz) => page.font(name, sz),
        PageOp::At(x, y) => page.at(x, y),
        PageOp::Text(ref text) => page.text(text),
        PageOp::Heading(level, ref text) => page.heading(level, text),
        PageOp::Paragraph(ref text) => page.paragraph(text),
        PageOp::Space(pts) => page.space(pts),
        PageOp::HorizontalRule => page.horizontal_rule(),
        PageOp::Image { data, x, y, w, h, alt } => {
            // `Rect::new` takes (x, y, width, height): the far
            // corner here scaled every image by its own origin.
            let rect = crate::geometry::Rect::new(x, y, w, h);
            match data {
                ImageBytes::Whole(bytes) => page.image_with_alt(page_image(&bytes)?, rect, &alt),
                ImageBytes::Streamed(image) => page.image_streamed(image, rect, Some(alt), false),
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
        PageOp::Inline(ref text) => page.inline(text),
        PageOp::StyledText {
            ref text,
            ref font,
            size,
            bold,
            italic,
            color,
            inline,
        } => {
            if inline {
                page.inline_styled(text, font.as_deref(), size, bold, italic, color)
            } else {
                page.text_styled(text, font.as_deref(), size, bold, italic, color)
            }
        }
        PageOp::TextInRect { x, y, w, h, ref text, align } => page.text_in_rect(
            crate::geometry::Rect::new(x, y, w, h),
            text,
            align,
        ),
        PageOp::FilledRect(x, y, w, h, r, g, b) => page.filled_rect(x, y, w, h, r, g, b),
        PageOp::StrokeRect { x, y, w, h, width, r, g, b } => {
            page.stroke_rect(x, y, w, h, crate::writer::LineStyle::new(width, r, g, b))
        }
        PageOp::StrokeRectDashed { x, y, w, h, width, r, g, b, dash, phase } => page
            .stroke_rect(
                x,
                y,
                w,
                h,
                crate::writer::LineStyle::new(width, r, g, b).with_dash(&dash, phase),
            ),
        PageOp::StrokeLine { x1, y1, x2, y2, width, r, g, b } => page.stroke_line(
            x1,
            y1,
            x2,
            y2,
            crate::writer::LineStyle::new(width, r, g, b),
        ),
        PageOp::StrokeLineDashed { x1, y1, x2, y2, width, r, g, b, dash, phase } => page
            .stroke_line(
                x1,
                y1,
                x2,
                y2,
                crate::writer::LineStyle::new(width, r, g, b).with_dash(&dash, phase),
            ),
        PageOp::Highlight(r, g, b) => page.highlight((r, g, b)),
        PageOp::Underline(r, g, b) => page.underline((r, g, b)),
        PageOp::Strikeout(r, g, b) => page.strikeout((r, g, b)),
        PageOp::Squiggly(r, g, b) => page.squiggly((r, g, b)),
        PageOp::StickyNote(ref text) => page.sticky_note(text),
        PageOp::StickyNoteAt(x, y, ref text) => page.sticky_note_at(x, y, text),
        PageOp::Stamp(stamp_type) => page.stamp(stamp_type),
        PageOp::FreeText { x, y, w, h, ref text } => page.freetext(
            crate::geometry::Rect::new(x, y, w, h),
            text,
        ),
        PageOp::LinkNamed(ref dest) => page.link_named(dest),
        PageOp::LinkJavaScript(ref script) => page.link_javascript(script),
        PageOp::OnOpen(ref script) => page.on_open(script),
        PageOp::OnClose(ref script) => page.on_close(script),
        PageOp::ImageArtifact { data, x, y, w, h } => {
            let rect = crate::geometry::Rect::new(x, y, w, h);
            match data {
                ImageBytes::Whole(bytes) => page.image_with_artifact(page_image(&bytes)?, rect),
                ImageBytes::Streamed(image) => page.image_streamed(image, rect, None, true),
            }
        }
        PageOp::StreamingTableBlock {
            ref config_columns,
            repeat_header,
            has_header,
            ref rows,
            ref mode,
            sample_rows,
            min_col_width_pt,
            max_col_width_pt,
            max_rowspan,
        } => {
            let mut cfg = crate::writer::StreamingTableConfig::new()
                .repeat_header(repeat_header)
                .header_row(has_header)
                .max_rowspan(max_rowspan);
            cfg = match mode.as_str() {
                "sample" => {
                    cfg.mode_sample(sample_rows, min_col_width_pt, max_col_width_pt)
                },
                _ => cfg.mode_fixed(),
            };
            for (header, width, align) in config_columns {
                let mut column = crate::writer::StreamingColumn::new(header.clone()).align(*align);
                if let Some(width) = width {
                    column = column.width_pt(*width);
                }
                cfg = cfg.column(column);
            }
            let mut t = page.streaming_table(cfg);
            for row in rows {
                t.push_row(|r| {
                    for (text, span) in row {
                        if *span > 1 {
                            r.span_cell(text.as_str(), *span);
                        } else {
                            r.cell(text.as_str());
                        }
                    }
                })?;
            }
            // The page's state lives in the builder: the returned
            // page handle carries nothing to keep.
            let _ = t.finish();
            return Ok(());
        }
        PageOp::Newline => page.newline(),
        PageOp::NewPageSameSize => page.new_page_same_size(),
    };
    Ok(())
}

/// Decode the image bytes of a page op. The caller passed the bytes, so a
/// failure is an invalid argument, not an engine fault.
fn page_image(data: &[u8]) -> Result<crate::writer::ImageData> {
    crate::writer::ImageData::from_bytes(data)
        .map_err(|e| Error::InvalidOperation(format!("image data is not a usable image: {e}")))
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
/// Set the builder's creator application.
pub fn builder_set_creator(b: DocumentBuilder, creator: &str) -> DocumentBuilder { b.creator(creator) }
/// Set the builder's producing application.
pub fn builder_set_producer(b: DocumentBuilder, producer: &str) -> DocumentBuilder { b.producer(producer) }
/// Set the builder's creation date (already formatted PDF date string).
pub fn builder_set_creation_date(b: DocumentBuilder, date: &str) -> DocumentBuilder { b.creation_date(date) }
/// Set the builder's last-modification date.
pub fn builder_set_mod_date(b: DocumentBuilder, date: &str) -> DocumentBuilder { b.mod_date(date) }
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

#[cfg(all(test, not(feature = "extract")))]
mod extract_trim_probe_tests {
    use super::*;

    #[test]
    fn extract_text_reports_not_enabled_when_trimmed() {
        let mut doc = match PdfDocument::from_bytes(
            crate::host::dispatch::trim_probe_tests_support::minimal_pdf(),
        ) {
            Ok(doc) => doc,
            Err(e) => panic!("minimal doc must parse: {e}"),
        };
        let err = match extract_text(&mut doc, None, "plainText", &crate::cancel::CancelCheck::none()) {
            Ok(_) => panic!("expected not-enabled error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("not enabled in this build"));
    }

    // A trimmed build must answer an excluded extract-family op with the
    // typed not-enabled error, never a crash.
    #[test]
    fn search_reports_not_enabled_when_trimmed() {
        let mut doc = match PdfDocument::from_bytes(
            crate::host::dispatch::trim_probe_tests_support::minimal_pdf(),
        ) {
            Ok(doc) => doc,
            Err(e) => panic!("minimal doc must parse: {e}"),
        };
        let err = match search_text(&mut doc, "x", None, &crate::cancel::CancelCheck::none()) {
            Ok(_) => panic!("expected not-enabled error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("not enabled in this build"));
    }
}

#[cfg(all(test, not(feature = "office")))]
mod office_trim_probe_tests {
    use super::*;

    // A trimmed build must answer an excluded office op with the typed
    // not-enabled error, never a crash.
    #[test]
    fn office_ops_report_not_enabled_when_trimmed() {
        let err = match convert_from_format_to_bytes(b"PK\x03\x04", "docx") {
            Ok(_) => panic!("expected not-enabled error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("not enabled in this build"));
    }
}

#[cfg(test)]
pub(crate) mod trim_probe_tests_support {
    /// The smallest parseable PDF (catalog + one empty page + xref).
    pub(crate) fn minimal_pdf() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (id, body) in [
            (1, "<< /Type /Catalog /Pages 2 0 R >>"),
            (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (3, "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>"),
        ] {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{id} 0 obj\n{body}\nendobj\n").as_bytes());
        }
        let xref_off = buf.len();
        buf.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
        for off in &offsets {
            buf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        buf.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{xref_off}\n%%EOF\n")
                .as_bytes(),
        );
        buf
    }
}

#[cfg(all(test, not(feature = "pdfa")))]
mod trim_probe_tests {
    use super::*;

    // The shake-audit runtime probe: a trimmed build must answer an excluded
    // op with the typed not-enabled error, never a crash.
    #[test]
    fn pdfa_ops_report_not_enabled_when_trimmed() {
        let mut doc = match PdfDocument::from_bytes(
            trim_probe_tests_support::minimal_pdf(),
        ) {
            Ok(doc) => doc,
            Err(e) => panic!("minimal doc must parse: {e}"),
        };
        let err = match validate_pdf_a(&mut doc, 2, &crate::cancel::CancelCheck::none()) {
            Ok(_) => panic!("expected not-enabled error"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("not enabled in this build"));
    }
}

#[cfg(test)]
mod wire_name_tests {
    use super::*;

    #[test]
    fn a_stamp_name_maps_and_an_index_or_unknown_name_does_not() {
        assert!(matches!(
            stamp_type_from_name("NotForPublicRelease"),
            Some(crate::writer::StampType::NotForPublicRelease)
        ));
        assert!(stamp_type_from_name("12").is_none());
        assert!(stamp_type_from_name("draft").is_none());
    }

    #[test]
    fn an_alignment_name_maps_to_its_q_value_and_back() {
        for (name, q) in [("left", 0), ("center", 1), ("right", 2)] {
            assert_eq!(alignment_q_from_name(name), Some(q));
            assert_eq!(alignment_name_from_q(q as i32), Some(name));
        }
        assert_eq!(alignment_q_from_name("1"), None);
        assert_eq!(alignment_name_from_q(-1), None);
        assert_eq!(alignment_name_from_q(3), None);
    }

    #[test]
    fn a_rotation_name_maps_to_its_degrees_and_back() {
        for (name, degrees) in [("r0", 0), ("r90", 90), ("r180", 180), ("r270", 270)] {
            assert_eq!(rotation_degrees_from_name(name), Some(degrees));
            assert_eq!(rotation_name_from_degrees(degrees), Some(name));
        }
        assert_eq!(rotation_degrees_from_name("90"), None);
        assert_eq!(rotation_degrees_from_name("R90"), None);
        assert_eq!(rotation_name_from_degrees(45), None);
        assert_eq!(rotation_name_from_degrees(360), None);
    }

    #[test]
    fn a_level_name_maps_and_a_number_or_unknown_name_does_not() {
        for (name, level) in [("a1", 1), ("a2", 2), ("a3", 3)] {
            assert_eq!(pdfa_level_from_name(name), Some(level));
        }
        for (name, level) in [("ua1", 1), ("ua2", 2)] {
            assert_eq!(pdfua_level_from_name(name), Some(level));
        }
        assert_eq!(pdfa_level_from_name("2"), None);
        assert_eq!(pdfa_level_from_name("ua1"), None);
        assert_eq!(pdfua_level_from_name("1"), None);
        assert_eq!(pdfua_level_from_name("a1"), None);
    }
}
