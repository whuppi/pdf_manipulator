//! Main document editing interface.
//!
//! Provides the DocumentEditor type for modifying PDF documents.

use crate::document::PdfDocument;
use crate::editor::form_fields::FormFieldWrapper;
use crate::editor::resource_manager::ResourceManager;
use crate::elements::{ContentElement, StructureElement};
use crate::error::{Error, Result};
use crate::extractors::HierarchicalExtractor;
use crate::geometry::Rect;
// ── pdf_manipulator patch: an image's redaction copy comes from the row writers ──
#[cfg(feature = "rendering")]
use crate::host::images::redact_copy;
// ── end pdf_manipulator patch ──
use crate::object::{Object, ObjectRef};
// ── pdf_manipulator patch: stream bodies are StreamBody ──
use crate::object::StreamBody;
// ── end pdf_manipulator patch ──
use crate::writer::{hoist_appearance_streams, ContentStreamBuilder, ObjectSerializer};
use std::collections::{HashMap, HashSet};
// ── pdf_manipulator patch: only the path saves open a file, and wasm has none ──
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
// ── end pdf_manipulator patch ──
#[cfg(not(target_arch = "wasm32"))]
use std::io::BufWriter;
use std::io::{Read, Write};
use std::path::Path;

// ── pdf_manipulator patch: the font and structure scrub of a redaction ──
// A child module, so it reads the redacted pages and the forms their passes open.
#[path = "../host/redact_scrub.rs"]
mod redact_scrub;
// ── end pdf_manipulator patch ──
// ── pdf_manipulator patch: image bounds are edited on the belt ──
#[path = "../host/image_mods.rs"]
mod image_mods;
// ── end pdf_manipulator patch ──

/// Document metadata (Info dictionary).
#[derive(Debug, Clone, Default)]
pub struct DocumentInfo {
    /// Document title
    pub title: Option<String>,
    /// Document author
    pub author: Option<String>,
    /// Document subject
    pub subject: Option<String>,
    /// Document keywords (comma-separated)
    pub keywords: Option<String>,
    /// Creator application
    pub creator: Option<String>,
    /// PDF producer
    pub producer: Option<String>,
    /// Creation date (PDF date format)
    pub creation_date: Option<String>,
    /// Modification date (PDF date format)
    pub mod_date: Option<String>,
}

impl DocumentInfo {
    /// Create a new empty DocumentInfo.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Set the subject.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Set the keywords.
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = Some(keywords.into());
        self
    }

    /// Set the creator.
    pub fn creator(mut self, creator: impl Into<String>) -> Self {
        self.creator = Some(creator.into());
        self
    }

    /// Set the producer.
    pub fn producer(mut self, producer: impl Into<String>) -> Self {
        self.producer = Some(producer.into());
        self
    }

    /// Convert to a PDF Info dictionary object.
    pub fn to_object(&self) -> Object {
        let mut dict = HashMap::new();

        // ── pdf_manipulator patch: Info values are text strings, not UTF-8 ──
        if let Some(ref title) = self.title {
            dict.insert("Title".to_string(), Object::text_string(title));
        }
        if let Some(ref author) = self.author {
            dict.insert("Author".to_string(), Object::text_string(author));
        }
        if let Some(ref subject) = self.subject {
            dict.insert("Subject".to_string(), Object::text_string(subject));
        }
        if let Some(ref keywords) = self.keywords {
            dict.insert("Keywords".to_string(), Object::text_string(keywords));
        }
        if let Some(ref creator) = self.creator {
            dict.insert("Creator".to_string(), Object::text_string(creator));
        }
        if let Some(ref producer) = self.producer {
            dict.insert("Producer".to_string(), Object::text_string(producer));
        }
        // ── end pdf_manipulator patch ──
        if let Some(ref creation_date) = self.creation_date {
            dict.insert(
                "CreationDate".to_string(),
                Object::String(creation_date.as_bytes().to_vec()),
            );
        }
        if let Some(ref mod_date) = self.mod_date {
            dict.insert("ModDate".to_string(), Object::String(mod_date.as_bytes().to_vec()));
        }

        Object::Dictionary(dict)
    }

    /// Parse from a PDF Info dictionary object.
    // ── pdf_manipulator patch: Info values decode as text strings (see
    // `decode_pdf_text_string`) — `from_utf8_lossy` turned PDFDocEncoding
    // and UTF-16BE metadata into U+FFFD mojibake. ──
    pub fn from_object(obj: &Object) -> Self {
        let mut info = Self::default();

        if let Some(dict) = obj.as_dict() {
            if let Some(Object::String(s)) = dict.get("Title") {
                info.title = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("Author") {
                info.author = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("Subject") {
                info.subject = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("Keywords") {
                info.keywords = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("Creator") {
                info.creator = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("Producer") {
                info.producer = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("CreationDate") {
                info.creation_date = crate::object::decode_pdf_text_string(s).into();
            }
            if let Some(Object::String(s)) = dict.get("ModDate") {
                info.mod_date = crate::object::decode_pdf_text_string(s).into();
            }
        }
        // ── end pdf_manipulator patch ──

        info
    }
}

/// Information about a page.
#[derive(Debug, Clone)]
pub struct PageInfo {
    /// Page index (0-based)
    pub index: usize,
    /// Page width in points
    pub width: f32,
    /// Page height in points
    pub height: f32,
    /// Page rotation (0, 90, 180, 270)
    pub rotation: i32,
    /// Object reference for this page
    pub object_ref: ObjectRef,
}

/// Options for saving the document.
// ── pdf_manipulator patch: Default is written out, object_streams defaults to true ──
#[derive(Debug, Clone)]
// ── end pdf_manipulator patch ──
pub struct SaveOptions {
    /// Use incremental update (append to original file)
    pub incremental: bool,
    /// Compress streams
    pub compress: bool,
    /// Remove unused objects
    pub garbage_collect: bool,
    /// Encryption configuration (None = no encryption)
    pub encryption: Option<EncryptionConfig>,
    // ── pdf_manipulator patch: keep the source's own encryption ──
    /// Write the output with the source's own encryption: its `/Encrypt`
    /// dictionary, `/ID` and file key unchanged, so the owner and user
    /// passwords and the permissions stay what the source set. Changes
    /// nothing when the source is not encrypted. Mutually exclusive with
    /// `encryption`.
    pub keep_source_encryption: bool,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: object streams and a cross-reference stream ──
    /// Pack the non-stream objects of a full rewrite into object streams
    /// and write a cross-reference stream (PDF 1.5). Off writes every
    /// object plain with a classic cross-reference table. An incremental
    /// save ignores it and keeps the source's own layout.
    pub object_streams: bool,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: the Flate level of a compressing save ──
    /// zlib level (0 to 9) of every stream a compressing full rewrite
    /// deflates or re-deflates. A stream whose re-encoding is not smaller
    /// keeps its source bytes. A level above 9 fails the save with
    /// `Error::InvalidOperation`.
    pub deflate_level: u32,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: one copy of identical images, ICC profiles and font programs ──
    /// Write one copy of each set of identical image XObjects, ICC profile
    /// streams and embedded font programs (`/FontFile`, `/FontFile2`,
    /// `/FontFile3`) of a full rewrite, across the document and every
    /// merged source, and point every reference at it. Two streams are one
    /// when their dictionaries (without `/Length`, every reference replaced
    /// by the object it names) and their encoded bytes are equal; a stream
    /// with `/OC`, `/StructParent` or `/StructParents` is never merged. A
    /// stream of an encrypted source is decrypted through a reader as it is
    /// hashed and compared, whatever its size. An incremental save ignores it.
    pub merge_duplicates: bool,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: unused resources dropped ──
    /// Drop from each resource dictionary of a full rewrite's kept pages
    /// (and of the pages merged in) the entries of `/Font`, `/XObject`,
    /// `/ColorSpace`, `/ExtGState`, `/Pattern`, `/Shading` and
    /// `/Properties` that no content using the dictionary names, and with
    /// garbage collection the objects only those entries reached. A
    /// dictionary whose content cannot be read, or that an object outside
    /// the pages' own resources also points at, stays whole, and so do the
    /// resources of a page the save edits. The AcroForm's `/DR` is never
    /// touched. The embedded TrueType and CID CFF font programs are cut down
    /// to the glyphs the kept pages show, every glyph id kept (see
    /// `host::font_subset`). An incremental save ignores it.
    pub prune_resources: bool,
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: Default is written out, object_streams defaults to true ──
impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            incremental: false,
            compress: false,
            garbage_collect: false,
            encryption: None,
            keep_source_encryption: false,
            object_streams: true,
            deflate_level: 9,
            merge_duplicates: false,
            prune_resources: false,
        }
    }
}
// ── end pdf_manipulator patch ──

impl SaveOptions {
    /// Create options for full rewrite (default).
    pub fn full_rewrite() -> Self {
        Self {
            incremental: false,
            compress: true,
            garbage_collect: true,
            ..Default::default()
        }
    }

    /// Create options for incremental update.
    pub fn incremental() -> Self {
        Self {
            incremental: true,
            compress: false,
            garbage_collect: false,
            ..Default::default()
        }
    }

    /// Create options with encryption enabled.
    ///
    /// Uses full rewrite mode since incremental updates don't support
    /// adding encryption to an existing PDF.
    pub fn with_encryption(config: EncryptionConfig) -> Self {
        Self {
            incremental: false,
            compress: true,
            garbage_collect: true,
            encryption: Some(config),
            ..Default::default()
        }
    }
}

/// Encryption algorithm for PDF security.
///
/// Per ISO 32000-1:2008 Section 7.6, PDF supports multiple encryption algorithms.
/// This enum represents the commonly used algorithms.
///
/// **Note**: This is a placeholder for v0.4.0 encryption support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EncryptionAlgorithm {
    /// RC4 with 40-bit key (PDF 1.1+, considered weak).
    Rc4_40,
    /// RC4 with 128-bit key (PDF 1.4+).
    Rc4_128,
    /// AES with 128-bit key (PDF 1.5+).
    Aes128,
    /// AES with 256-bit key (PDF 1.7 Extension Level 3+, recommended).
    #[default]
    Aes256,
}

/// Permission flags for encrypted PDFs.
///
/// Per ISO 32000-1:2008 Section 7.6.3.2, these flags control what operations
/// are permitted when the document is opened with the user password.
///
/// **Note**: This is a placeholder for v0.4.0 encryption support.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Allow printing the document.
    pub print: bool,
    /// Allow high-resolution printing.
    pub print_high_quality: bool,
    /// Allow modifying the document contents.
    pub modify: bool,
    /// Allow copying or extracting text and graphics.
    pub copy: bool,
    /// Allow adding annotations and form fields.
    pub annotate: bool,
    /// Allow filling in form fields.
    pub fill_forms: bool,
    /// Allow extracting content for accessibility.
    pub accessibility: bool,
    /// Allow document assembly (insert, rotate, delete pages).
    pub assemble: bool,
}

impl Permissions {
    /// Create with all permissions granted.
    pub fn all() -> Self {
        Self {
            print: true,
            print_high_quality: true,
            modify: true,
            copy: true,
            annotate: true,
            fill_forms: true,
            accessibility: true,
            assemble: true,
        }
    }

    /// Create with minimal permissions (view only).
    pub fn read_only() -> Self {
        Self {
            accessibility: true, // Always allow for compliance
            ..Default::default()
        }
    }

    /// Convert permissions to the 32-bit P value for the encryption dictionary.
    ///
    /// PDF Spec: Table 22 - User access permissions
    ///
    /// The returned value has reserved bits set appropriately:
    /// - Bits 7-8 must be 1
    /// - Bits 13-32 must be 1 (for compatibility)
    pub fn to_bits(&self) -> i32 {
        // Base value with required reserved bits set
        // Bits 7-8 (0-indexed: 6-7) and bits 13-32 (0-indexed: 12-31) must be 1
        let mut bits: i32 = 0xFFFFF0C0u32 as i32;

        // Bit 3 (0-indexed: 2): Print
        if self.print {
            bits |= 1 << 2;
        }

        // Bit 4 (0-indexed: 3): Modify contents
        if self.modify {
            bits |= 1 << 3;
        }

        // Bit 5 (0-indexed: 4): Copy or extract text and graphics
        if self.copy {
            bits |= 1 << 4;
        }

        // Bit 6 (0-indexed: 5): Add or modify annotations
        if self.annotate {
            bits |= 1 << 5;
        }

        // Bit 9 (0-indexed: 8): Fill in form fields (R>=3)
        if self.fill_forms {
            bits |= 1 << 8;
        }

        // Bit 10 (0-indexed: 9): Extract text for accessibility (R>=3)
        if self.accessibility {
            bits |= 1 << 9;
        }

        // Bit 11 (0-indexed: 10): Assemble document (R>=3)
        if self.assemble {
            bits |= 1 << 10;
        }

        // Bit 12 (0-indexed: 11): Print high quality (R>=3)
        if self.print_high_quality {
            bits |= 1 << 11;
        }

        bits
    }
}

/// Configuration for PDF encryption on save.
///
/// This struct configures how a PDF should be encrypted when saved.
/// Use with `SaveOptions::with_encryption()` to enable encryption.
///
/// # Example (Planned for v0.4.0)
///
/// ```ignore
/// use pdf_oxide::editor::{EncryptionConfig, EncryptionAlgorithm, Permissions};
///
/// let config = EncryptionConfig {
///     user_password: "user123".to_string(),
///     owner_password: "owner456".to_string(),
///     algorithm: EncryptionAlgorithm::Aes256,
///     permissions: Permissions::all(),
/// };
/// ```
///
/// **Note**: This is a placeholder for v0.4.0 encryption support.
/// Currently, PDFs are saved without encryption.
#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Password required to open the document (can be empty for no user password).
    pub user_password: String,
    /// Password for full access and changing security settings.
    pub owner_password: String,
    /// Encryption algorithm to use.
    pub algorithm: EncryptionAlgorithm,
    /// Permission flags when opened with user password.
    pub permissions: Permissions,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            user_password: String::new(),
            owner_password: String::new(),
            algorithm: EncryptionAlgorithm::default(),
            permissions: Permissions::all(),
        }
    }
}

impl EncryptionConfig {
    /// Create a new encryption config with the given passwords.
    pub fn new(user_password: impl Into<String>, owner_password: impl Into<String>) -> Self {
        Self {
            user_password: user_password.into(),
            owner_password: owner_password.into(),
            ..Default::default()
        }
    }

    /// Set the encryption algorithm.
    pub fn with_algorithm(mut self, algorithm: EncryptionAlgorithm) -> Self {
        self.algorithm = algorithm;
        self
    }

    /// Set the permissions.
    pub fn with_permissions(mut self, permissions: Permissions) -> Self {
        self.permissions = permissions;
        self
    }
}

/// Trait for editable document operations.
pub trait EditableDocument {
    /// Get document metadata.
    fn get_info(&mut self) -> Result<DocumentInfo>;

    /// Set document metadata.
    fn set_info(&mut self, info: DocumentInfo) -> Result<()>;

    /// Get the number of pages.
    fn page_count(&mut self) -> Result<usize>;

    /// Get information about a specific page.
    fn get_page_info(&mut self, index: usize) -> Result<PageInfo>;

    /// Remove a page by index.
    fn remove_page(&mut self, index: usize) -> Result<()>;

    /// Move a page from one index to another.
    fn move_page(&mut self, from: usize, to: usize) -> Result<()>;

    /// Duplicate a page.
    fn duplicate_page(&mut self, index: usize) -> Result<usize>;

    /// Save the document to a file.
    fn save(&mut self, path: impl AsRef<Path>) -> Result<()>;

    /// Save with specific options.
    fn save_with_options(&mut self, path: impl AsRef<Path>, options: SaveOptions) -> Result<()>;
}

// ── pdf_manipulator patch: a page a destructive redaction rewrites on save ──
/// A page a destructive redaction rewrites on save.
struct RedactedPage {
    regions: crate::redaction::RegionSet,
    opts: crate::redaction::RedactionOptions,
    fonts: crate::redaction::FontInfoMetrics,
    report: crate::redaction::RedactionReport,
    dropped_annots: std::collections::HashSet<u32>,
    /// What the pass queued: one image placement per overwritten copy (the
    /// save writes each as its own stream and its /SMask-or-/Mask copy,
    /// never the source's pixels redacted twice), the mesh, pattern and form
    /// copies, and the resource names the page's surviving marks still name.
    scope: crate::redaction::transform::Scope,
    /// The annotations and widgets the save flattens on this page, which the
    /// pass decided with the page's content (set when the save prepares it).
    flat: Option<FlatPage>,
}

/// The flattened appearances of a redacted page: each is a Form XObject the
/// page's overlay draws, so the pass sees it as any other form and the
/// redaction removes what lies under a box.
struct FlatPage {
    sets: Vec<FlatSet>,
    /// The overlay operators of every set, in order.
    overlay: Vec<u8>,
}

/// The appearances of one kind of flattening, with the id and resource name
/// each Form XObject is written under.
struct FlatSet {
    appearances: Vec<AnnotationAppearance>,
    xobjects: Vec<(u32, String)>,
    /// Whether the set flattens widgets: the page keeps no widget after it.
    widgets: bool,
}

impl FlatPage {
    /// The Form XObject written under `id`, when it is one of the page's.
    fn object(&self, id: u32) -> Option<Object> {
        self.sets.iter().find_map(|set| {
            let at = set.xobjects.iter().position(|(xid, _)| *xid == id)?;
            Some(set.appearances[at].form_object())
        })
    }

    /// The object written under `id` beside an appearance (its fallback
    /// font's graph), when it is one of the page's.
    fn extra(&self, id: u32) -> Option<&Object> {
        self.sets
            .iter()
            .flat_map(|set| &set.appearances)
            .flat_map(|appearance| &appearance.extra_objects)
            .find_map(|(extra_id, object)| (*extra_id == id).then_some(object))
    }

    /// `object` with every reference to an object of [`Self::extra`] replaced
    /// by that object, so a reader that knows only the source document can
    /// read the graph. A graph deeper than `depth`, or a cycle, keeps its
    /// reference.
    fn inline_extras(&self, object: &Object, depth: usize) -> Object {
        match object {
            Object::Reference(r) if depth > 0 => match self.extra(r.id) {
                Some(extra) => self.inline_extras(extra, depth - 1),
                None => object.clone(),
            },
            Object::Dictionary(dict) => Object::Dictionary(
                dict.iter().map(|(key, value)| (key.clone(), self.inline_extras(value, depth))).collect(),
            ),
            Object::Array(items) => {
                Object::Array(items.iter().map(|item| self.inline_extras(item, depth)).collect())
            },
            other => other.clone(),
        }
    }

    /// Drops the object written under `id` beside an appearance: it is not
    /// written.
    fn remove_extra(&mut self, id: u32) {
        for set in &mut self.sets {
            for appearance in &mut set.appearances {
                appearance.extra_objects.retain(|(extra_id, _)| *extra_id != id);
            }
        }
    }

    /// Replaces the object written under `id` beside an appearance.
    fn replace_extra(&mut self, id: u32, replacement: Object) {
        for set in &mut self.sets {
            for appearance in &mut set.appearances {
                for (extra_id, object) in &mut appearance.extra_objects {
                    if *extra_id == id {
                        *object = replacement;
                        return;
                    }
                }
            }
        }
    }
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: a page's destruction, planned before it is recorded ──
/// What destroying regions on one page records in the editor, worked out
/// without changing it.
struct DestructionPlan {
    originals: Vec<u32>,
    orphans: Vec<u32>,
    report: crate::redaction::RedactionReport,
    page: RedactedPage,
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: the ids one queued image copy is written under ──
/// The new object ids one image copy is written from: its stream and its
/// /Length, and the copies its `/SMask` or `/Mask` stencil stream takes
/// (written in step, after the page's content, the same u/v rect mapping).
#[derive(Default)]
struct RedactionCopyIds {
    image: (u32, u32),
    smask: Option<(u32, u32)>,
    mask: Option<(u32, u32)>,
}

/// The new object ids of every copy a scope queued, in the order of the
/// scope's lists: a mesh and a form take a stream and its /Length, a pattern
/// one dictionary, and a form carries the ids of the scope below it.
#[derive(Default)]
struct ScopeIds {
    images: Vec<RedactionCopyIds>,
    meshes: Vec<(u32, u32)>,
    patterns: Vec<u32>,
    forms: Vec<FormIds>,
    /// The /Font entries of the scope's resources that the scrub replaces:
    /// the copy of a font shared with content the redaction does not touch,
    /// or a direct font dictionary rewritten.
    fonts: HashMap<String, Object>,
}

/// The ids of one form copy and of the copies its own content queued.
struct FormIds {
    stream: (u32, u32),
    inner: ScopeIds,
}

/// Every copy of a page's scope and of the forms below it, flattened for the
/// writer, each with the context it is written in.
#[derive(Default)]
struct CopyWork<'a> {
    /// A placement, its ids and the /ColorSpace map of the resources it is
    /// drawn from.
    #[cfg(feature = "rendering")]
    images: Vec<(
        &'a crate::redaction::transform::ImagePlacement,
        &'a RedactionCopyIds,
        HashMap<String, Object>,
    )>,
    #[cfg(feature = "rendering")]
    meshes: Vec<(&'a crate::redaction::transform::MeshCopy, (u32, u32))>,
    /// A pattern copy, its id and the id of the mesh copy it names.
    #[cfg(feature = "rendering")]
    patterns: Vec<(&'a crate::redaction::transform::PatternCopy, u32, u32)>,
    forms: Vec<(&'a crate::redaction::transform::FormCopy, &'a FormIds)>,
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: the forms a redaction pass opens come from the source document ──
/// Opens the forms of a page for the redaction pass. A form without its own
/// /Resources reads the page's, as old writers expect.
struct DocForms<'d> {
    source: &'d PdfDocument,
    page_resources: Object,
    /// The page's flattened appearances, which no document holds.
    flat: Option<&'d FlatPage>,
}

impl DocForms<'_> {
    /// The object `form` names.
    fn load(&self, form: ObjectRef) -> Result<Object> {
        match self.flat.and_then(|flat| flat.object(form.id)) {
            Some(object) => Ok(object),
            None => self.source.load_object(form),
        }
    }

    /// The resources a form is drawn with, resolved.
    fn resources_of(&self, dict: &HashMap<String, Object>) -> Result<Object> {
        match dict.get("Resources") {
            Some(resources) => self.source.resolve_object(resources),
            None => Ok(self.page_resources.clone()),
        }
    }
}

impl crate::redaction::transform::FormLoader for DocForms<'_> {
    fn view(&self, form: ObjectRef) -> Result<crate::redaction::transform::FormView> {
        let object = self.load(form)?;
        let unreadable = |what: &str| {
            Error::Unsupported(format!(
                "the form {} {} has {what} that cannot be read; refusing to redact a page \
                 whose form cannot be placed",
                form.id, form.gen
            ))
        };
        let dict = object.as_dict().ok_or_else(|| unreadable("no dictionary"))?;
        let numbers = |key: &str, count: usize| -> Result<Option<Vec<f32>>> {
            let Some(value) = dict.get(key) else { return Ok(None) };
            let value = self.source.resolve_object(value)?;
            let items = value.as_array().ok_or_else(|| unreadable(key))?;
            if items.len() != count {
                return Err(unreadable(key));
            }
            items
                .iter()
                .map(|item| {
                    let item = self.source.resolve_object(item)?;
                    item.as_real()
                        .or_else(|| item.as_integer().map(|i| i as f64))
                        .map(|n| n as f32)
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| unreadable(key))
                })
                .collect::<Result<Vec<f32>>>()
                .map(Some)
        };
        let matrix = match numbers("Matrix", 6)? {
            Some(m) => crate::content::graphics_state::Matrix {
                a: m[0],
                b: m[1],
                c: m[2],
                d: m[3],
                e: m[4],
                f: m[5],
            },
            None => crate::content::graphics_state::Matrix::identity(),
        };
        let b = numbers("BBox", 4)?.ok_or_else(|| unreadable("no /BBox"))?;
        let resources = self.resources_of(dict)?;
        Ok(crate::redaction::transform::FormView {
            matrix,
            bbox: Rect::from_points(b[0].min(b[2]), b[1].min(b[3]), b[0].max(b[2]), b[1].max(b[3])),
            resources: DocumentEditor::image_resources_of(self.source, &resources, None)?,
            fonts: Box::new(DocumentEditor::font_metrics_of(self.source, &resources, self.flat)?),
        })
    }

    fn content(
        &self,
        form: ObjectRef,
    ) -> Result<crate::host::content::Operators<Box<dyn Read>>> {
        match self.flat.and_then(|flat| flat.object(form.id)) {
            Some(object) => Ok(crate::host::content::Operators::new(
                crate::host::content::parts::decoded_reader(&object)?,
                crate::host::content::Mode::Full,
            )),
            None => self.source.form_operators(form),
        }
    }
}
// ── end pdf_manipulator patch ──

/// PDF document editor.
///
/// Provides a high-level interface for modifying PDF documents.
/// Changes are tracked and can be saved either as incremental updates
/// or as a complete rewrite.
pub struct DocumentEditor {
    /// Source document (for reading)
    source: PdfDocument,
    /// Path to the source file
    source_path: String,
    /// Modified objects (object ID -> new object)
    modified_objects: HashMap<u32, Object>,
    /// New objects to add (will be assigned new IDs)
    new_objects: Vec<Object>,
    /// Next object ID to use for new objects
    next_object_id: u32,
    /// Modified metadata
    modified_info: Option<DocumentInfo>,
    // ── pdf_manipulator patch: one page order for source and merged pages ──
    /// The output pages in order: a source page by its index in the original
    /// document, a merged page by its position among all merged pages. A
    /// removed page is absent; a duplicated page appears twice.
    page_order: Vec<PageSlot>,
    // ── end pdf_manipulator patch ──
    /// Number of pages in original document
    original_page_count: usize,
    /// Track if document has been modified
    is_modified: bool,
    /// Modified page content (page_index → new structure, full replacement)
    modified_content: HashMap<usize, StructureElement>,
    /// Overlay additions for source-loaded pages (source_page_index → new elements).
    /// These are appended to the original content stream rather than replacing it.
    overlay_additions: HashMap<usize, Vec<ContentElement>>,
    /// Resource manager for fonts/images
    resource_manager: ResourceManager,
    /// Track if structure tree needs rebuilding
    structure_modified: bool,
    /// Modified page annotations (page_index → annotations)
    modified_annotations: HashMap<usize, Vec<crate::editor::dom::AnnotationWrapper>>,
    /// Modified page properties (rotation, boxes)
    modified_page_props: HashMap<usize, ModifiedPageProps>,
    /// Erase regions per page (whiteout overlays)
    erase_regions: HashMap<usize, Vec<[f32; 4]>>,
    /// Pages where annotations should be flattened
    flatten_annotations_pages: std::collections::HashSet<usize>,
    /// Pages where redactions should be applied
    apply_redactions_pages: std::collections::HashSet<usize>,
    /// Programmatic redaction regions queued per source page index
    /// (in addition to any `/Redact` annotations), used by destructive
    /// redaction (#231).
    redaction_regions: HashMap<usize, Vec<crate::redaction::RedactionRegion>>,
    // ── pdf_manipulator patch: the redacted pages, recorded at apply ──
    /// Destructively redacted pages per source page index, recorded by
    /// `apply_redactions_destructive` and written by the save path as the
    /// page's single new `/Contents` stream. Nothing of the content is held
    /// here: the save streams the rewrite from the source.
    redacted_pages: HashMap<usize, RedactedPage>,
    // ── end pdf_manipulator patch ──
    /// Object ids of the *original* `/Contents` streams of redacted
    /// pages. These hold the pre-redaction (secret) bytes and must NEVER
    /// be emitted, independent of GC reachability — the hard G6 guarantee
    /// (#231; reachability alone is insufficient because the page is only
    /// repointed transiently during the write).
    redacted_orphan_ids: std::collections::HashSet<u32>,
    // ── pdf_manipulator patch: the sources redaction overwrote or dropped ──
    /// The source image XObjects redaction overwrote or dropped: an original
    /// is written only when something else still references it (a page the
    /// redaction did not touch, resources another kept page shares), with
    /// garbage collection on and off.
    redacted_image_originals: std::collections::HashSet<u32>,
    // ── end pdf_manipulator patch ──
    /// Image modifications per page: page_index -> (image_name -> modification)
    image_modifications: HashMap<usize, HashMap<String, ImageModification>>,
    /// Pages where form fields should be flattened
    flatten_forms_pages: std::collections::HashSet<usize>,
    /// Flag to remove AcroForm from catalog after form flattening
    remove_acroform: bool,
    /// Warnings collected during form flattening (e.g. widgets with no /AP stream)
    flatten_warnings: Vec<String>,
    /// Embedded files to add to the document
    embedded_files: Vec<crate::writer::EmbeddedFile>,
    /// Modified or new form fields (field name → wrapper)
    modified_form_fields: HashMap<String, FormFieldWrapper>,
    /// Deleted form field names
    deleted_form_fields: HashSet<String>,
    /// Flag indicating AcroForm dictionary needs rebuilding on save
    acroform_modified: bool,
    // ── pdf_manipulator patch: a merge keeps its source open and records ids only ──
    /// One entry per merge: the merged document, kept open for the
    /// editor's lifetime, and the ids of what the save copies from it.
    merged_sources: Vec<MergedSource>,
    /// Rotation and boxes of merged pages, by merged page index (the
    /// position among all merged pages, in import order).
    merged_page_props: HashMap<usize, ModifiedPageProps>,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: a merge carries the source's AcroForm ──
    /// Root field references imported by a merge, already renumbered into
    /// this document. Appended to the saved catalog's `/AcroForm /Fields`.
    merged_form_fields: Vec<ObjectRef>,
    /// The merged document's `/DR` and `/DA`, imported alongside its
    /// fields; used only when this document's AcroForm has neither.
    /// Kept in the numbering of the merged document at that index of
    /// `merged_sources`; the save remaps it.
    merged_acroform_dr: Option<(usize, Object)>,
    merged_acroform_da: Option<Object>,
    /// Leaf page refs of `source`, in tree order, computed once per source.
    /// Page add, delete, move and merge change `page_order` and `merged_sources`
    /// only, never the source tree, so only a source replacement clears it.
    source_page_refs: Option<std::sync::Arc<[ObjectRef]>>,
    /// The way back of an [`DocumentEditor::undoable`] operation that is running.
    checkpoint: Option<Checkpoint>,
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: an operation that replaces the source can be undone ──
/// What an undoable operation restores: the staged state the editor had when
/// it began, and the source the operation replaced, which a commit or a
/// source replacement would otherwise drop.
struct Checkpoint {
    modified_objects: HashMap<u32, Object>,
    new_objects: Vec<Object>,
    next_object_id: u32,
    is_modified: bool,
    source_page_refs: Option<std::sync::Arc<[ObjectRef]>>,
    /// The first source the operation replaced; `None` until it replaces one.
    source: Option<PdfDocument>,
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: every editor field is classified for the incremental save ──
/// What an incremental save does with a [`DocumentEditor`] field.
#[cfg(test)]
#[derive(Clone, Copy)]
enum IncrementalClass {
    /// What was opened, not an edit.
    Source,
    /// Derived or transient state the append never writes.
    Cache,
    /// An edit the append writes.
    Carried,
    /// An edit `incremental_refusal` names.
    Refused,
}

#[cfg(test)]
impl IncrementalClass {
    fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Cache => "cache",
            Self::Carried => "carried",
            Self::Refused => "refused",
        }
    }
}

/// The class of every field of [`DocumentEditor`]. The pattern names every
/// field and has no `..`, so a field added without a row here does not
/// compile; give it one row in the list below the pattern.
#[cfg(test)]
impl DocumentEditor {
    /// `(field, class, why)` for every field of the editor; the class is
    /// `source`, `cache`, `carried` or `refused`.
    pub(crate) fn incremental_census(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        let DocumentEditor {
            source: _,
            source_path: _,
            modified_objects: _,
            new_objects: _,
            next_object_id: _,
            modified_info: _,
            page_order: _,
            original_page_count: _,
            is_modified: _,
            modified_content: _,
            overlay_additions: _,
            resource_manager: _,
            structure_modified: _,
            modified_annotations: _,
            modified_page_props: _,
            erase_regions: _,
            flatten_annotations_pages: _,
            apply_redactions_pages: _,
            redaction_regions: _,
            redacted_pages: _,
            redacted_orphan_ids: _,
            redacted_image_originals: _,
            image_modifications: _,
            flatten_forms_pages: _,
            remove_acroform: _,
            flatten_warnings: _,
            embedded_files: _,
            modified_form_fields: _,
            deleted_form_fields: _,
            acroform_modified: _,
            merged_sources: _,
            merged_page_props: _,
            merged_form_fields: _,
            merged_acroform_dr: _,
            merged_acroform_da: _,
            source_page_refs: _,
            checkpoint: _,
        } = self;
        vec![
            ("source", IncrementalClass::Source.name(), "the opened document; the append copies every original byte"),
            ("source_path", IncrementalClass::Source.name(), "where the document was opened from"),
            ("modified_objects", IncrementalClass::Carried.name(), "written as the append's replaced objects (incremental_objects)"),
            ("new_objects", IncrementalClass::Refused.name(), "'form fields or objects were added'; no editor call stages it today"),
            ("next_object_id", IncrementalClass::Cache.name(), "an id counter; the append reads it only for /Size and a new /Info id"),
            ("modified_info", IncrementalClass::Carried.name(), "written as the append's /Info object (build_info_object)"),
            ("page_order", IncrementalClass::Refused.name(), "'pages were removed or reordered' (the identity check)"),
            ("original_page_count", IncrementalClass::Source.name(), "the page count at open; the identity check compares against it"),
            ("is_modified", IncrementalClass::Cache.name(), "a dirty flag; the append writes what is staged, not this flag"),
            ("modified_content", IncrementalClass::Refused.name(), "'page content was drawn, erased or rebuilt'"),
            ("overlay_additions", IncrementalClass::Refused.name(), "'page content was drawn, erased or rebuilt'; no editor call stages it today"),
            ("resource_manager", IncrementalClass::Cache.name(), "names resources for hand-built content; the save never reads it, and that content is staged in modified_content"),
            ("structure_modified", IncrementalClass::Refused.name(), "'page content was drawn, erased or rebuilt'"),
            ("modified_annotations", IncrementalClass::Refused.name(), "'annotations were changed or flattened'"),
            ("modified_page_props", IncrementalClass::Carried.name(), "applied to the page object in the append (incremental_objects)"),
            ("erase_regions", IncrementalClass::Refused.name(), "'page content was drawn, erased or rebuilt'"),
            ("flatten_annotations_pages", IncrementalClass::Refused.name(), "'annotations were changed or flattened'"),
            ("apply_redactions_pages", IncrementalClass::Refused.name(), "'content was redacted or scrubbed'"),
            ("redaction_regions", IncrementalClass::Refused.name(), "'content was redacted or scrubbed'"),
            ("redacted_pages", IncrementalClass::Refused.name(), "'content was redacted or scrubbed'"),
            ("redacted_orphan_ids", IncrementalClass::Refused.name(), "'content was redacted or scrubbed'"),
            ("redacted_image_originals", IncrementalClass::Refused.name(), "not named by the refusal; set only with redacted_pages, which is"),
            ("image_modifications", IncrementalClass::Refused.name(), "'page content was drawn, erased or rebuilt'"),
            ("flatten_forms_pages", IncrementalClass::Refused.name(), "'annotations were changed or flattened'"),
            ("remove_acroform", IncrementalClass::Refused.name(), "'form fields were removed'"),
            ("flatten_warnings", IncrementalClass::Cache.name(), "diagnostics of a flatten, which is refused"),
            ("embedded_files", IncrementalClass::Refused.name(), "'files were embedded'"),
            ("modified_form_fields", IncrementalClass::Carried.name(), "an existing field's edits flush into the append's replaced objects (flush_form_fields_to_modified_objects); a new field is refused ('form fields or objects were added')"),
            ("deleted_form_fields", IncrementalClass::Refused.name(), "'form fields were removed'"),
            ("acroform_modified", IncrementalClass::Carried.name(), "the flag that runs the form flush in incremental_objects"),
            ("merged_sources", IncrementalClass::Refused.name(), "'pages were merged in'"),
            ("merged_page_props", IncrementalClass::Refused.name(), "keyed by merged page, which exists only after a merge: 'pages were merged in'"),
            ("merged_form_fields", IncrementalClass::Refused.name(), "'pages were merged in'"),
            ("merged_acroform_dr", IncrementalClass::Refused.name(), "not named by the refusal; set only by a merge, which fills merged_sources"),
            ("merged_acroform_da", IncrementalClass::Refused.name(), "not named by the refusal; set only by a merge, which fills merged_sources"),
            ("source_page_refs", IncrementalClass::Cache.name(), "the source's page refs, computed on demand"),
            ("checkpoint", IncrementalClass::Cache.name(), "the undo state of a running operation; empty between operations"),
        ]
    }
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: the written-id set follows every xref push ──
/// The output xref entries `(id, offset, gen, in_use)` of a full rewrite,
/// with the set of ids they hold. `push` is the only way to add an entry
/// (an inherent method shadows `Vec::push` reached through `DerefMut`), so
/// `contains_id` is exact at every point without rebuilding the set.
pub(crate) struct XrefEntries {
    entries: Vec<(u32, u64, u16, bool)>,
    ids: HashSet<u32>,
}

impl XrefEntries {
    fn new() -> Self {
        Self { entries: Vec::new(), ids: HashSet::new() }
    }

    pub(crate) fn push(&mut self, entry: (u32, u64, u16, bool)) {
        self.ids.insert(entry.0);
        self.entries.push(entry);
    }

    fn contains_id(&self, id: u32) -> bool {
        self.ids.contains(&id)
    }

    /// Counts `id` as written without an entry: a merged duplicate has no
    /// object of its own, and every write loop skips an id that is written.
    fn claim_id(&mut self, id: u32) {
        self.ids.insert(id);
    }

    /// Frees the id set once nothing asks `contains_id` any more.
    fn release_ids(&mut self) {
        self.ids = HashSet::new();
    }

    fn into_entries(self) -> Vec<(u32, u64, u16, bool)> {
        self.entries
    }
}

impl std::ops::Deref for XrefEntries {
    type Target = [(u32, u64, u16, bool)];

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl std::ops::DerefMut for XrefEntries {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: a merge keeps its source open and records ids only ──
/// A page imported from a merged document: nothing of it is copied until
/// the save streams it.
struct MergedPageImport {
    /// The page in the merged document.
    page_ref: ObjectRef,
    /// The page's id in this document.
    page_id: u32,
    /// The slice of `MergedSource::objects` this page reaches first.
    objects: std::ops::Range<usize>,
}

/// One merged document and the pages taken from it.
struct MergedSource {
    doc: PdfDocument,
    pages: Vec<MergedPageImport>,
    /// Every object the pages reach, grouped by the page that met it first:
    /// `(id in this document, id in the merged document)`. One flat list,
    /// since a vector per page costs more than its content on a long document.
    objects: Vec<(u32, u32)>,
    /// Merged-document id to id in this document, for the whole merge.
    id_map: HashMap<u32, u32>,
    /// Page-tree node (merged-document id) to the inheritable attributes
    /// (§7.7.3.4) a page below it gets, nearest value winning, in the merged
    /// document's numbering. The save writes them into pages that lack them.
    inherited: HashMap<u32, Vec<(String, Object)>>,
}

/// The page attributes a page inherits from its ancestors in the page tree.
const INHERITABLE: [&str; 4] = ["Resources", "MediaBox", "CropBox", "Rotate"];

/// The object cache of a merged document: it is read once per object, so
/// the cache only has to hold the walk's working set.
const MERGED_CACHE_BYTES: usize = 1024 * 1024;

/// One output page: a page of the original document by its index there, or
/// a merged page by its position among all merged pages, in import order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageKey {
    Source(usize),
    Merged(usize),
}

/// A `PageKey` in four bytes, so the page order of a 10,000-page document
/// merged with another costs 80 KB. The top bit marks a merged page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PageSlot(u32);

impl PageSlot {
    const MERGED: u32 = 1 << 31;

    pub(crate) fn source(index: usize) -> Self {
        Self::pack(index, 0)
    }

    pub(crate) fn merged(index: usize) -> Self {
        Self::pack(index, Self::MERGED)
    }

    fn pack(index: usize, tag: u32) -> Self {
        let index = u32::try_from(index)
            .ok()
            .filter(|&i| i & Self::MERGED == 0)
            .expect("a page index fits in 31 bits");
        Self(index | tag)
    }

    pub(crate) fn key(self) -> PageKey {
        let index = (self.0 & !Self::MERGED) as usize;
        if self.0 & Self::MERGED == 0 {
            PageKey::Source(index)
        } else {
            PageKey::Merged(index)
        }
    }
}
// ── end pdf_manipulator patch ──

/// Tracks modified page properties.
#[derive(Debug, Clone, Default)]
pub struct ModifiedPageProps {
    /// New rotation value (0, 90, 180, 270)
    pub rotation: Option<i32>,
    /// New MediaBox
    pub media_box: Option<[f32; 4]>,
    /// New CropBox
    pub crop_box: Option<[f32; 4]>,
    // ── pdf_manipulator patch: a page's own /CropBox can be removed ──
    /// The page's own `/CropBox` is dropped on save.
    pub crop_box_removed: bool,
    // ── end pdf_manipulator patch ──
}

/// Stores annotation appearance data for flattening.
#[derive(Debug, Clone)]
struct AnnotationAppearance {
    // ── pdf_manipulator patch: the appearance stream as stored, never a byte copy ──
    /// The appearance stream's dictionary (empty for a generated appearance)
    dict: HashMap<String, Object>,
    /// Its data: an extent of the document that holds the stream, or the
    /// bytes of an appearance generated here
    body: StreamBody,
    // ── end pdf_manipulator patch ──
    /// BBox of the appearance XObject
    bbox: [f32; 4],
    /// Rect of the annotation on the page
    annot_rect: [f32; 4],
    /// Optional transformation matrix from the appearance
    matrix: Option<[f32; 6]>,
    /// Resources used by the appearance
    resources: Option<Object>,
    /// Extra indirect objects this appearance depends on (e.g. an embedded
    /// fallback-font object graph), written verbatim alongside the appearance
    /// XObject. Empty for self-contained appearances.
    extra_objects: Vec<(u32, Object)>,
}

// ── pdf_manipulator patch: the appearance is flattened as the stream it is stored as ──
impl AnnotationAppearance {
    /// The data of the appearance object `object` and the entries of its
    /// dictionary that [`Self::form_object`] does not set (the filters and the
    /// like). A dictionary where a stream is expected is an empty stream.
    fn stream_of(object: &Object) -> (HashMap<String, Object>, StreamBody) {
        let none = HashMap::new();
        let (dict, body) = match object {
            Object::Stream { dict, body } => (dict, body.clone()),
            other => (other.as_dict().unwrap_or(&none), StreamBody::Inline(bytes::Bytes::new())),
        };
        // Rebuilt, not trimmed: a table emptied by `remove` keeps its slots.
        let rest = dict
            .iter()
            .filter(|(key, _)| {
                !["Length", "Type", "Subtype", "FormType", "BBox", "Matrix", "Resources"]
                    .contains(&key.as_str())
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        (rest, body)
    }

    /// The Form XObject this appearance is flattened as: its own stream with
    /// /BBox, /Matrix and /Resources set.
    fn form_object(&self) -> Object {
        let mut dict = self.dict.clone();
        dict.remove("Length");
        dict.insert("Type".to_string(), Object::Name("XObject".to_string()));
        dict.insert("Subtype".to_string(), Object::Name("Form".to_string()));
        dict.insert("FormType".to_string(), Object::Integer(1));
        dict.insert(
            "BBox".to_string(),
            Object::Array(self.bbox.iter().map(|v| Object::Real(*v as f64)).collect()),
        );
        if let Some(m) = self.matrix {
            dict.insert(
                "Matrix".to_string(),
                Object::Array(m.iter().map(|v| Object::Real(*v as f64)).collect()),
            );
        }
        if let Some(resources) = &self.resources {
            dict.insert("Resources".to_string(), resources.clone());
        }
        Object::Stream { dict, body: self.body.clone() }
    }
}
// ── end pdf_manipulator patch ──

/// Information about an image on a page.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageInfo {
    /// XObject name (e.g., "Im1")
    pub name: String,
    /// Position and size: x, y, width, height
    pub bounds: [f32; 4],
    /// Full transformation matrix [a, b, c, d, e, f]
    pub matrix: [f32; 6],
}

/// Modification to apply to an image.
#[derive(Debug, Clone)]
struct ImageModification {
    /// New x position (if Some, changes position)
    x: Option<f32>,
    /// New y position (if Some, changes position)
    y: Option<f32>,
    /// New width (if Some, changes width)
    width: Option<f32>,
    /// New height (if Some, changes height)
    height: Option<f32>,
}

impl DocumentEditor {
    /// Open a PDF document for editing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let editor = DocumentEditor::open("document.pdf")?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let mut source = PdfDocument::open(path.as_ref())?;

        // Get page count
        let page_count = source.page_count()?;

        // Find the highest object ID to know where to start for new objects
        let next_id = Self::find_max_object_id(&source) + 1;

        // Initialize page order as sequential
        // ── pdf_manipulator patch: one page order for source and merged pages ──
        let page_order: Vec<PageSlot> = (0..page_count).map(PageSlot::source).collect();
        // ── end pdf_manipulator patch ──

        Ok(Self {
            source,
            source_path: path_str,
            modified_objects: HashMap::new(),
            new_objects: Vec::new(),
            next_object_id: next_id,
            modified_info: None,
            page_order,
            original_page_count: page_count,
            is_modified: false,
            modified_content: HashMap::new(),
            overlay_additions: HashMap::new(),
            resource_manager: ResourceManager::new(),
            structure_modified: false,
            modified_annotations: HashMap::new(),
            modified_page_props: HashMap::new(),
            erase_regions: HashMap::new(),
            flatten_annotations_pages: std::collections::HashSet::new(),
            apply_redactions_pages: std::collections::HashSet::new(),
            redaction_regions: HashMap::new(),
            // ── pdf_manipulator patch: the redacted pages, recorded at apply ──
            redacted_pages: HashMap::new(),
            // ── end pdf_manipulator patch ──
            redacted_orphan_ids: std::collections::HashSet::new(),
            // ── pdf_manipulator patch: the sources redaction overwrote or dropped ──
            redacted_image_originals: std::collections::HashSet::new(),
            // ── end pdf_manipulator patch ──
            image_modifications: HashMap::new(),
            flatten_forms_pages: std::collections::HashSet::new(),
            remove_acroform: false,
            flatten_warnings: Vec::new(),
            embedded_files: Vec::new(),
            modified_form_fields: HashMap::new(),
            deleted_form_fields: HashSet::new(),
            acroform_modified: false,
            // ── pdf_manipulator patch: a merge keeps its source open and records ids only ──
            merged_sources: Vec::new(),
            merged_page_props: HashMap::new(),
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: a merge carries the source's AcroForm ──
            merged_form_fields: Vec::new(),
            merged_acroform_dr: None,
            merged_acroform_da: None,
            source_page_refs: None,
            checkpoint: None,
            // ── end pdf_manipulator patch ──
        })
    }

    /// Open a PDF document for editing from an existing PdfDocument object.
    pub fn from_document(mut source: PdfDocument) -> Result<Self> {
        let page_count = source.page_count()?;
        let next_id = Self::find_max_object_id(&source) + 1;
        // ── pdf_manipulator patch: one page order for source and merged pages ──
        let page_order: Vec<PageSlot> = (0..page_count).map(PageSlot::source).collect();
        // ── end pdf_manipulator patch ──

        Ok(Self {
            source,
            source_path: String::new(),
            modified_objects: HashMap::new(),
            new_objects: Vec::new(),
            next_object_id: next_id,
            modified_info: None,
            page_order,
            original_page_count: page_count,
            is_modified: false,
            modified_content: HashMap::new(),
            overlay_additions: HashMap::new(),
            resource_manager: ResourceManager::new(),
            structure_modified: false,
            modified_annotations: HashMap::new(),
            modified_page_props: HashMap::new(),
            erase_regions: HashMap::new(),
            flatten_annotations_pages: std::collections::HashSet::new(),
            apply_redactions_pages: std::collections::HashSet::new(),
            redaction_regions: HashMap::new(),
            // ── pdf_manipulator patch: the redacted pages, recorded at apply ──
            redacted_pages: HashMap::new(),
            // ── end pdf_manipulator patch ──
            redacted_orphan_ids: std::collections::HashSet::new(),
            // ── pdf_manipulator patch: the sources redaction overwrote or dropped ──
            redacted_image_originals: std::collections::HashSet::new(),
            // ── end pdf_manipulator patch ──
            image_modifications: HashMap::new(),
            flatten_forms_pages: std::collections::HashSet::new(),
            remove_acroform: false,
            flatten_warnings: Vec::new(),
            embedded_files: Vec::new(),
            modified_form_fields: HashMap::new(),
            deleted_form_fields: HashSet::new(),
            acroform_modified: false,
            // ── pdf_manipulator patch: a merge keeps its source open and records ids only ──
            merged_sources: Vec::new(),
            merged_page_props: HashMap::new(),
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: a merge carries the source's AcroForm ──
            merged_form_fields: Vec::new(),
            merged_acroform_dr: None,
            merged_acroform_da: None,
            source_page_refs: None,
            checkpoint: None,
            // ── end pdf_manipulator patch ──
        })
    }

    /// Open a PDF document for editing from in-memory bytes.
    ///
    /// This is the equivalent of `open()` but works with byte data instead of a file path,
    /// making it suitable for WASM environments where filesystem access is unavailable.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let mut source = PdfDocument::from_bytes(data)?;
        let page_count = source.page_count()?;
        let next_id = Self::find_max_object_id(&source) + 1;
        // ── pdf_manipulator patch: one page order for source and merged pages ──
        let page_order: Vec<PageSlot> = (0..page_count).map(PageSlot::source).collect();
        // ── end pdf_manipulator patch ──

        Ok(Self {
            source,
            source_path: String::new(),
            modified_objects: HashMap::new(),
            new_objects: Vec::new(),
            next_object_id: next_id,
            modified_info: None,
            page_order,
            original_page_count: page_count,
            is_modified: false,
            modified_content: HashMap::new(),
            overlay_additions: HashMap::new(),
            resource_manager: ResourceManager::new(),
            structure_modified: false,
            modified_annotations: HashMap::new(),
            modified_page_props: HashMap::new(),
            erase_regions: HashMap::new(),
            flatten_annotations_pages: std::collections::HashSet::new(),
            apply_redactions_pages: std::collections::HashSet::new(),
            redaction_regions: HashMap::new(),
            // ── pdf_manipulator patch: the redacted pages, recorded at apply ──
            redacted_pages: HashMap::new(),
            // ── end pdf_manipulator patch ──
            redacted_orphan_ids: std::collections::HashSet::new(),
            // ── pdf_manipulator patch: the sources redaction overwrote or dropped ──
            redacted_image_originals: std::collections::HashSet::new(),
            // ── end pdf_manipulator patch ──
            image_modifications: HashMap::new(),
            flatten_forms_pages: std::collections::HashSet::new(),
            remove_acroform: false,
            flatten_warnings: Vec::new(),
            embedded_files: Vec::new(),
            modified_form_fields: HashMap::new(),
            deleted_form_fields: HashSet::new(),
            acroform_modified: false,
            // ── pdf_manipulator patch: a merge keeps its source open and records ids only ──
            merged_sources: Vec::new(),
            merged_page_props: HashMap::new(),
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: a merge carries the source's AcroForm ──
            merged_form_fields: Vec::new(),
            merged_acroform_dr: None,
            merged_acroform_da: None,
            source_page_refs: None,
            checkpoint: None,
            // ── end pdf_manipulator patch ──
        })
    }

    /// Deprecated alias for `from_bytes`.
    #[deprecated(since = "0.3.15", note = "Use `from_bytes` instead")]
    pub fn open_from_bytes(data: Vec<u8>) -> Result<Self> {
        Self::from_bytes(data)
    }

    /// Save the document to an in-memory byte vector.
    ///
    /// This is the equivalent of `save()` but returns bytes instead of writing to a file,
    /// making it suitable for WASM environments where filesystem access is unavailable.
    pub fn save_to_bytes(&mut self) -> Result<Vec<u8>> {
        self.save_to_bytes_with_options(SaveOptions::full_rewrite())
    }

    /// Save the document to an in-memory byte vector with specific options.
    pub fn save_to_bytes_with_options(&mut self, options: SaveOptions) -> Result<Vec<u8>> {
        // ── pdf_manipulator patch: route through the streaming writer ──
        // A fresh Cursor starts at 0, so the byte count equals the stream
        // position every offset in the output assumes.
        use crate::host::positioned_write::CountingWriter;
        if options.incremental {
            return Err(Error::InvalidPdf(
                "Incremental saves are not supported for in-memory output".to_string(),
            ));
        }
        let mut writer = CountingWriter::new(std::io::Cursor::new(Vec::new()));
        self.write_full_to_writer(&mut writer, &options, &crate::cancel::CancelCheck::none())?;
        Ok(writer.into_inner().into_inner())
        // ── end pdf_manipulator patch ──
    }

    // ── pdf_manipulator patch: no /Size never collides new ids ──
    /// Find the maximum object ID in the document: the largest of
    /// `/Size - 1` (an object id is one below `/Size`, the entry count)
    /// and the largest object number in the xref. A source can claim a
    /// `/Size` smaller than its real content (or none at all), so only
    /// the xref's own numbers prove which ids are taken; callers add 1.
    fn find_max_object_id(doc: &PdfDocument) -> u32 {
        let size_max = doc
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Size"))
            .and_then(|s| s.as_integer())
            .and_then(|size| u32::try_from(size).ok())
            .and_then(|size| size.checked_sub(1));
        let xref_max = doc.all_object_ids().pop().unwrap_or(0);
        size_max.unwrap_or(0).max(xref_max)
    }
    // ── end pdf_manipulator patch ──

    /// Allocate a new object ID.
    fn allocate_object_id(&mut self) -> u32 {
        let id = self.next_object_id;
        self.next_object_id += 1;
        id
    }

    /// Allocate a new object ID (accessible to sibling crate modules).
    pub(crate) fn alloc_id(&mut self) -> u32 {
        self.allocate_object_id()
    }

    /// Stage a new or modified object (accessible to sibling crate modules).
    pub(crate) fn insert_modified(&mut self, id: u32, obj: Object) {
        self.modified_objects.insert(id, obj);
        self.is_modified = true;
    }

    /// Look up a staged object by ID (accessible to sibling crate modules).
    pub(crate) fn get_modified(&self, id: u32) -> Option<&Object> {
        self.modified_objects.get(&id)
    }

    /// Serialise all staged changes to bytes, re-parse, and reset staging state
    /// so that subsequent reads (e.g. from the validator) see the mutations.
    pub(crate) fn commit_in_place(&mut self) -> Result<()> {
        if !self.is_modified {
            return Ok(());
        }
        let new_bytes = self.save_to_bytes()?;
        // ── pdf_manipulator patch: the replaced source goes to the checkpoint ──
        self.replace_source(PdfDocument::from_bytes(new_bytes)?);
        // ── end pdf_manipulator patch ──
        self.source_page_refs = None;
        self.modified_objects.clear();
        self.new_objects.clear();
        self.next_object_id = Self::find_max_object_id(&self.source) + 1;
        self.is_modified = false;
        Ok(())
    }

    // ── pdf_manipulator patch: an operation that replaces the source can be undone ──
    /// Puts `source` in place of the editor's, handing the one it replaces to
    /// the running [`Self::undoable`] operation, if there is one.
    fn replace_source(&mut self, source: PdfDocument) {
        let replaced = std::mem::replace(&mut self.source, source);
        if let Some(checkpoint) = self.checkpoint.as_mut() {
            checkpoint.source.get_or_insert(replaced);
        }
    }

    /// Runs `operation` on the editor all or nothing: when it returns an
    /// error, whatever it staged and any source it committed or replaced are
    /// undone, so the editor is exactly as it was before the call. Operations
    /// do not nest.
    pub(crate) fn undoable<T>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        debug_assert!(self.checkpoint.is_none(), "undoable operations do not nest");
        self.checkpoint = Some(Checkpoint {
            modified_objects: self.modified_objects.clone(),
            new_objects: self.new_objects.clone(),
            next_object_id: self.next_object_id,
            is_modified: self.is_modified,
            source_page_refs: self.source_page_refs.clone(),
            source: None,
        });
        let outcome = operation(self);
        let checkpoint = self.checkpoint.take();
        if outcome.is_err() {
            if let Some(checkpoint) = checkpoint {
                self.modified_objects = checkpoint.modified_objects;
                self.new_objects = checkpoint.new_objects;
                self.next_object_id = checkpoint.next_object_id;
                self.is_modified = checkpoint.is_modified;
                self.source_page_refs = checkpoint.source_page_refs;
                if let Some(source) = checkpoint.source {
                    self.source = source;
                }
            }
        }
        outcome
    }
    // ── end pdf_manipulator patch ──

    /// Replace the source document wholesale from external bytes.
    ///
    /// Used when a conversion step produces a completely new PDF (e.g. transparency
    /// flattening via re-rendering) rather than mutating individual objects.
    /// Clears all staged modifications so subsequent fixes operate on the new document.
    pub(crate) fn replace_source_bytes(&mut self, bytes: Vec<u8>) -> Result<()> {
        // ── pdf_manipulator patch: the replaced source goes to the checkpoint ──
        self.replace_source(PdfDocument::from_bytes(bytes)?);
        // ── end pdf_manipulator patch ──
        self.source_page_refs = None;
        self.modified_objects.clear();
        self.new_objects.clear();
        self.next_object_id = Self::find_max_object_id(&self.source) + 1;
        // is_modified stays false: the source already holds the new bytes and
        // no objects are staged, so commit_in_place() would be a no-op anyway.
        // Subsequent insert_modified/alloc calls will flip it back to true.
        Ok(())
    }

    /// Consume the editor and return the underlying document.
    pub fn into_source(self) -> PdfDocument {
        self.source
    }

    /// Apply page property modifications to a page object.
    ///
    /// Returns a new page object with the modifications applied.
    fn apply_page_props_to_object(
        &self,
        page_obj: &Object,
        props: &ModifiedPageProps,
    ) -> Result<Object> {
        let page_dict = page_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Page is not a dictionary".to_string()))?;

        let mut new_dict = page_dict.clone();

        // Apply rotation if modified
        if let Some(rotation) = props.rotation {
            new_dict.insert("Rotate".to_string(), Object::Integer(rotation as i64));
        }

        // Apply MediaBox if modified
        if let Some(media_box) = props.media_box {
            let box_array = Object::Array(vec![
                Object::Real(media_box[0] as f64),
                Object::Real(media_box[1] as f64),
                Object::Real(media_box[2] as f64),
                Object::Real(media_box[3] as f64),
            ]);
            new_dict.insert("MediaBox".to_string(), box_array);
        }

        // Apply CropBox if modified
        if let Some(crop_box) = props.crop_box {
            let box_array = Object::Array(vec![
                Object::Real(crop_box[0] as f64),
                Object::Real(crop_box[1] as f64),
                Object::Real(crop_box[2] as f64),
                Object::Real(crop_box[3] as f64),
            ]);
            new_dict.insert("CropBox".to_string(), box_array);
        }
        // ── pdf_manipulator patch: a page's own /CropBox can be removed ──
        if props.crop_box_removed {
            new_dict.remove("CropBox");
        }
        // ── end pdf_manipulator patch ──

        Ok(Object::Dictionary(new_dict))
    }

    /// Check if the document has unsaved changes.
    pub fn is_modified(&self) -> bool {
        self.is_modified
    }

    // ── pdf_manipulator patch: internal state accessors for dispatch ──
    pub fn set_modified(&mut self, v: bool) { self.is_modified = v; }
    /// The source index of every source page in the output, in output order.
    pub fn page_order_visible(&self) -> Vec<i32> {
        self.page_order
            .iter()
            .filter_map(|slot| match slot.key() {
                PageKey::Source(index) => Some(index as i32),
                PageKey::Merged(_) => None,
            })
            .collect()
    }
    /// The source index of the page at output index `page`; a merged page
    /// has none and is refused.
    pub fn source_page_index(&self, page: usize) -> Result<usize> {
        self.source_page_for(page, "reading its source page")
    }
    /// Runs `f` with the document cut down to the pages at `pages` (output
    /// indices, in the order given), then puts the page list back, so the
    /// editor is observably unchanged afterwards even when `f` fails. The
    /// editor counts as modified while `f` runs, so a save rewrites in full.
    pub fn with_pages_selected<R>(
        &mut self,
        pages: &[usize],
        f: impl FnOnce(&mut Self) -> R,
    ) -> Result<R> {
        let mut selected = Vec::with_capacity(pages.len());
        for &page in pages {
            crate::cancel::poll()?;
            selected.push(*self.page_order.get(page).ok_or_else(|| {
                Error::InvalidPdf(format!(
                    "Page index {} out of range (document has {} pages)",
                    page,
                    self.page_order.len()
                ))
            })?);
        }
        let saved_order = std::mem::replace(&mut self.page_order, selected);
        let saved_is_modified = std::mem::replace(&mut self.is_modified, true);
        let result = f(self);
        self.page_order = saved_order;
        self.is_modified = saved_is_modified;
        Ok(result)
    }
    pub fn modified_objects_mut(&mut self) -> &mut HashMap<u32, Object> { &mut self.modified_objects }

    /// Makes the full rewrite skip source object `id`, as a destructive redaction does.
    #[cfg(test)]
    pub(crate) fn withhold_from_rewrite(&mut self, id: u32) {
        self.redacted_orphan_ids.insert(id);
    }
    // ── end pdf_manipulator patch ──

    /// Get the source file path.
    pub fn source_path(&self) -> &str {
        &self.source_path
    }

    /// Get immutable reference to the source document.
    pub fn source(&self) -> &PdfDocument {
        &self.source
    }

    /// Get mutable reference to the source document.
    ///
    /// This provides access to PdfDocument methods for extraction and conversion.
    pub fn source_mut(&mut self) -> &mut PdfDocument {
        self.source_page_refs = None;
        &mut self.source
    }

    /// Get the PDF version.
    pub fn version(&self) -> (u8, u8) {
        self.source.version()
    }

    // === Metadata operations ===

    /// Get the document title.
    pub fn title(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.title)
    }

    /// Set the document title.
    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.title = Some(title);
        }
        self.is_modified = true;
    }

    /// Get the document author.
    pub fn author(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.author)
    }

    /// Set the document author.
    pub fn set_author(&mut self, author: impl Into<String>) {
        let author = author.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.author = Some(author);
        }
        self.is_modified = true;
    }

    /// Get the document subject.
    pub fn subject(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.subject)
    }

    /// Set the document subject.
    pub fn set_subject(&mut self, subject: impl Into<String>) {
        let subject = subject.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.subject = Some(subject);
        }
        self.is_modified = true;
    }

    /// Get the document keywords.
    pub fn keywords(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.keywords)
    }

    /// Set the document keywords.
    pub fn set_keywords(&mut self, keywords: impl Into<String>) {
        let keywords = keywords.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.keywords = Some(keywords);
        }
        self.is_modified = true;
    }

    /// Get the document producer (tool that produced the PDF).
    pub fn producer(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.producer)
    }

    /// Set the document producer. Persists to `/Info.Producer` on save.
    pub fn set_producer(&mut self, producer: impl Into<String>) {
        let producer = producer.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.producer = Some(producer);
        }
        self.is_modified = true;
    }

    /// Get the raw PDF creation-date string (e.g. `D:20260421120000Z`).
    pub fn creation_date(&mut self) -> Result<Option<String>> {
        let info = self.get_info()?;
        Ok(info.creation_date)
    }

    /// Set the raw PDF creation-date string. Persists to
    /// `/Info.CreationDate` on save.
    pub fn set_creation_date(&mut self, date: impl Into<String>) {
        let date = date.into();
        if self.modified_info.is_none() {
            self.modified_info = Some(self.get_info().unwrap_or_default());
        }
        if let Some(ref mut info) = self.modified_info {
            info.creation_date = Some(date);
        }
        self.is_modified = true;
    }

    // === Page operations ===

    /// Get the current page count (after modifications).
    pub fn current_page_count(&self) -> usize {
        // ── pdf_manipulator patch: merged pages are in the page order ──
        self.page_order.len()
        // ── end pdf_manipulator patch ──
    }

    // ── pdf_manipulator patch: merged pages are addressed beside source pages ──
    /// Pages imported by merges, over all merged documents.
    fn merged_page_total(&self) -> usize {
        self.merged_sources.iter().map(|s| s.pages.len()).sum()
    }

    /// Where the page at user-visible index `output` lives. An index past
    /// the last page names the source page of that index.
    fn page_key(&self, output: usize) -> PageKey {
        self.page_order.get(output).map_or(PageKey::Source(output), |slot| slot.key())
    }

    /// The source page index of `output`, or the refusal for a merged
    /// page: its content is copied from the merged document at save and
    /// cannot be edited.
    fn source_page_for(&self, output: usize, op: &str) -> Result<usize> {
        match self.page_key(output) {
            PageKey::Source(index) => Ok(index),
            PageKey::Merged(_) => Err(Error::Unsupported(format!(
                "page {output} was merged in; {op} is not supported on merged pages"
            ))),
        }
    }

    /// The merged page at merged index `index` and the document it came from.
    fn merged_page(&self, index: usize) -> Option<(&MergedSource, &MergedPageImport)> {
        let mut rest = index;
        for source in &self.merged_sources {
            if rest < source.pages.len() {
                return Some((source, &source.pages[rest]));
            }
            rest -= source.pages.len();
        }
        None
    }

    /// The editable page properties of the page at `output`, by source
    /// index or by merged index.
    fn page_props_mut(&mut self, output: usize) -> &mut ModifiedPageProps {
        match self.page_key(output) {
            PageKey::Source(index) => self.modified_page_props.entry(index).or_default(),
            PageKey::Merged(index) => self.merged_page_props.entry(index).or_default(),
        }
    }

    /// The page dictionary of merged page `index`, as the merged document has it.
    fn merged_page_dict(&self, index: usize) -> Result<HashMap<String, Object>> {
        let (source, page) = self
            .merged_page(index)
            .ok_or_else(|| Error::InvalidPdf(format!("Page index {} out of range", index)))?;
        match Self::merged_page_object(source, page.page_ref)? {
            Object::Dictionary(dict) => Ok(dict),
            _ => Err(Error::InvalidPdf("Page is not a dictionary".to_string())),
        }
    }

    /// A page box (`MediaBox`, `CropBox`) of a page dictionary.
    fn box_of(dict: &HashMap<String, Object>, key: &str) -> Option<[f32; 4]> {
        let array = dict.get(key)?.as_array()?;
        if array.len() < 4 {
            return None;
        }
        let at = |i: usize, default: f64| {
            array[i]
                .as_real()
                .or_else(|| array[i].as_integer().map(|n| n as f64))
                .unwrap_or(default) as f32
        };
        Some([at(0, 0.0), at(1, 0.0), at(2, 612.0), at(3, 792.0)])
    }

    // ── end pdf_manipulator patch ──

    /// Translate a user-visible (output-order) page index to the internal source page index.
    ///
    /// Per-page HashMaps (erase_regions, overlay_additions, modified_page_props, …) are keyed
    /// by source index so that the serialization loop—which iterates `page_order` and already
    /// uses source indices—can find entries regardless of how `select_pages` / `remove_page`
    /// has reordered the output.
    ///
    /// When no reordering has been applied `page_order = [0, 1, 2, …]` so the mapping is the
    /// identity and there is no observable difference for callers.
    // ── pdf_manipulator patch: read by the engine proofs ──
    pub(crate) fn output_to_source_index(&self, output: usize) -> usize {
    // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: a page order lookup, not a scan per call ──
        match self.page_key(output) {
            PageKey::Source(index) => index,
            PageKey::Merged(_) => output,
        }
        // ── end pdf_manipulator patch ──
    }

    /// Get the list of page objects in current order.
    fn get_page_refs(&mut self) -> Result<std::sync::Arc<[ObjectRef]>> {
        if let Some(refs) = &self.source_page_refs {
            return Ok(std::sync::Arc::clone(refs));
        }
        // ── pdf_manipulator patch: one page walk in the crate (#261) ──
        let refs: std::sync::Arc<[ObjectRef]> = self.source.all_page_refs()?.into();
        // ── end pdf_manipulator patch ──
        self.source_page_refs = Some(std::sync::Arc::clone(&refs));
        Ok(refs)
    }

    /// Extract a subset of pages into a new PDF and write it to `output`.
    ///
    /// `pages` is a list of 0-based indices to keep. The current document is
    /// not modified. Works by cloning the document and removing all unwanted
    /// pages in reverse-index order (same approach used by the WASM binding).
    pub fn extract_pages(&mut self, pages: &[usize], output: impl AsRef<Path>) -> Result<()> {
        let bytes = self.extract_pages_to_bytes(pages)?;
        std::fs::write(output, bytes)?;
        Ok(())
    }

    /// Extract a subset of pages and return the result as PDF bytes.
    ///
    /// `pages` is a list of 0-based indices to keep. The current document is
    /// not modified.
    pub fn extract_pages_to_bytes(&mut self, pages: &[usize]) -> Result<Vec<u8>> {
        use crate::editor::EditableDocument;

        if pages.is_empty() {
            return Err(Error::InvalidPdf("pages list must not be empty".to_string()));
        }

        let page_count = self.page_count()?;
        for &page in pages {
            // ── pdf_manipulator patch: a cancel stops the page list within one page ──
            crate::cancel::poll()?;
            // ── end pdf_manipulator patch ──
            if page >= page_count {
                return Err(Error::InvalidPdf(format!(
                    "Page index {} out of range (document has {} pages)",
                    page, page_count
                )));
            }
        }

        // ── pdf_manipulator patch: no staged /Pages; the save prunes (#261) ──
        // Serialise once with a trimmed page order. The save's prune plan
        // drops everything only the other pages reach, merged pages included.
        self.with_pages_selected(pages, |editor| editor.save_to_bytes())?
        // ── end pdf_manipulator patch ──
    }

    /// Extract several non-overlapping page ranges in one call, returning the
    /// PDF bytes for each.
    ///
    /// `ranges` is a list of `(start, end)` tuples interpreted as `[start, end)`
    /// half-open ranges over current visible pages. `start` may equal `end`
    /// (empty range is rejected, matching `extract_pages_to_bytes`). Each output
    /// is independent; this call does not deduplicate work between ranges
    /// beyond the per-document caches that warm up after the first one.
    ///
    /// Mirrors the chunked workflow described in issue #474: a 12k-page
    /// document split into 3000-page chunks for downstream processing.
    pub fn extract_page_ranges_to_bytes(
        &mut self,
        ranges: &[(usize, usize)],
    ) -> Result<Vec<Vec<u8>>> {
        let mut out: Vec<Vec<u8>> = Vec::with_capacity(ranges.len());
        for &(start, end) in ranges {
            if end < start {
                return Err(Error::InvalidPdf(format!(
                    "Invalid page range: end ({}) < start ({})",
                    end, start
                )));
            }
            let pages: Vec<usize> = (start..end).collect();
            out.push(self.extract_pages_to_bytes(&pages)?);
        }
        Ok(out)
    }

    /// Restrict the document to the listed pages, in the order given. The
    /// pages not listed are dropped (in `page_order`); subsequent
    /// `save_to_bytes()` / `save()` will produce a PDF containing only the
    /// selected pages, with garbage-collected resources.
    ///
    /// This is the in-place analogue of `extract_pages_to_bytes`: it mutates
    /// the editor instead of returning bytes, so chained edits (e.g. add
    /// annotations, then save) work as expected. Equivalent to PyMuPDF's
    /// `doc.select(page_list)`.
    pub fn select_pages(&mut self, pages: &[usize]) -> Result<()> {
        use crate::editor::EditableDocument;

        if pages.is_empty() {
            return Err(Error::InvalidPdf("pages list must not be empty".to_string()));
        }
        let page_count = self.page_count()?;
        for &p in pages {
            // ── pdf_manipulator patch: a cancel stops the page list within one page ──
            crate::cancel::poll()?;
            // ── end pdf_manipulator patch ──
            if p >= page_count {
                return Err(Error::InvalidPdf(format!(
                    "Page index {} out of range (document has {} pages)",
                    p, page_count
                )));
            }
        }
        // ── pdf_manipulator patch: merged pages are selected like any other ──
        self.page_order = pages.iter().map(|&i| self.page_order[i]).collect();
        // ── end pdf_manipulator patch ──
        self.is_modified = true;
        Ok(())
    }

    /// Overlay a PNG image onto an existing page at the given position.
    ///
    /// `page_index` is 0-based. `x`, `y`, `width`, `height` are in PDF points
    /// (1/72 inch), measured from the bottom-left of the page.
    ///
    /// The PNG bytes are decoded, then appended to the page's content stream as
    /// an XObject.  The page is saved back after the operation.
    pub fn add_image_bytes_to_page(
        &mut self,
        page_index: usize,
        png_bytes: &[u8],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Result<()> {
        use crate::elements::ImageContent;
        use crate::geometry::Rect;

        let image = ImageContent::from_bytes(Rect::new(x, y, width, height), png_bytes.to_vec())
            .map_err(|e| crate::error::Error::Image(e.to_string()))?;
        self.edit_page(page_index, |page| {
            page.add_image(image);
            Ok(())
        })
    }

    /// Merge pages from another PDF into this document.
    ///
    /// This appends all pages from the source PDF to the end of this document.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("main.pdf")?;
    /// editor.merge_from("appendix.pdf")?;
    /// editor.save("combined.pdf")?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn merge_from(&mut self, source_path: impl AsRef<Path>) -> Result<usize> {
        let data = std::fs::read(source_path.as_ref())?;
        self.merge_from_bytes(&data)
    }

    /// Merge another PDF (from raw bytes) into this document.
    ///
    /// Works in all environments including WebAssembly.
    ///
    /// # Arguments
    ///
    /// * `data` - Raw bytes of the PDF to merge
    ///
    /// # Returns
    ///
    /// Number of pages merged from the source PDF.
    pub fn merge_from_bytes(&mut self, data: &[u8]) -> Result<usize> {
        // ── pdf_manipulator patch: shared core with merge_from_reader ──
        let source_doc = PdfDocument::from_bytes(data.to_vec())?;
        self.merge_from_document(source_doc)
        // ── end pdf_manipulator patch ──
    }

    // ── pdf_manipulator patch: O(1)-memory merge via reader ──
    pub(crate) fn merge_from_reader(&mut self, reader: Box<dyn crate::document::ReadSeek>) -> Result<usize> {
        let source_doc = PdfDocument::from_external_reader_bounded(reader, MERGED_CACHE_BYTES)?;
        self.merge_from_document(source_doc)
    }

    /// Selective twin of `merge_from_reader`: imports only `pages`, in the
    /// order given.
    pub(crate) fn merge_pages_from_reader(
        &mut self,
        reader: Box<dyn crate::document::ReadSeek>,
        pages: &[usize],
    ) -> Result<usize> {
        let source_doc = PdfDocument::from_external_reader_bounded(reader, MERGED_CACHE_BYTES)?;
        self.merge_pages_from_document(source_doc, pages)
    }

    /// Opens `reader` as a document to merge from, with the merge's small
    /// object cache. The caller authenticates it before [`Self::merge_document`].
    pub(crate) fn open_merge_source(reader: Box<dyn crate::document::ReadSeek>) -> Result<PdfDocument> {
        PdfDocument::from_external_reader_bounded(reader, MERGED_CACHE_BYTES)
    }

    /// Imports `pages` (every page when `None`) of a document the caller
    /// opened and authenticated, in the order given.
    pub(crate) fn merge_document(
        &mut self,
        source_doc: PdfDocument,
        pages: Option<&[usize]>,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<usize> {
        match pages {
            Some(pages) => self.merge_pages_from_document_cancellable(source_doc, pages, cancel),
            None => self.merge_from_document_cancellable(source_doc, cancel),
        }
    }

    fn merge_from_document(&mut self, source_doc: PdfDocument) -> Result<usize> {
        self.merge_from_document_cancellable(source_doc, &crate::cancel::CancelCheck::none())
    }

    fn merge_from_document_cancellable(
        &mut self,
        source_doc: PdfDocument,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<usize> {
        let source_page_count = source_doc.page_count()?;
        let pages: Vec<usize> = (0..source_page_count).collect();
        self.merge_pages_from_document_cancellable(source_doc, &pages, cancel)
    }

    /// The page-selection core every merge entry point shares.
    ///
    /// One `id_map` spans the whole merge, so an object two imported pages
    /// share keeps one identity — which is also what lets the source's
    /// `/AcroForm /Fields` entries be renumbered onto the imported field
    /// objects (`carry_merged_acroform`).
    ///
    /// The merge records ids only: the document stays open in the editor
    /// and the save streams each object from it.
    fn merge_pages_from_document(
        &mut self,
        source_doc: PdfDocument,
        pages: &[usize],
    ) -> Result<usize> {
        self.merge_pages_from_document_cancellable(source_doc, pages, &crate::cancel::CancelCheck::none())
    }

    /// [`merge_pages_from_document`](Self::merge_pages_from_document) that
    /// stops with the cancelled error once `cancel` reports cancelled. The
    /// only state the import touches before its commit is the next object
    /// id, which a cancel puts back.
    fn merge_pages_from_document_cancellable(
        &mut self,
        source_doc: PdfDocument,
        pages: &[usize],
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<usize> {
        let page_refs = source_doc.all_page_refs_cancellable(cancel)?;
        for &page in pages {
            cancel.check()?;
            if page >= page_refs.len() {
                return Err(Error::InvalidPdf(format!(
                    "Page index {} out of range (source has {} pages)",
                    page,
                    page_refs.len()
                )));
            }
        }
        if pages.is_empty() {
            return Ok(0);
        }

        let mut merged = MergedSource {
            doc: source_doc,
            pages: Vec::with_capacity(pages.len()),
            objects: Vec::new(),
            id_map: HashMap::new(),
            inherited: HashMap::new(),
        };
        let mut source_pages: Vec<u32> = page_refs.iter().map(|r| r.id).collect();
        source_pages.sort_unstable();
        // Every imported page gets its id first, so a destination or /P
        // naming an imported page lands on the merged copy.
        let first_free_id = self.next_object_id;
        let page_ids: Vec<u32> = pages.iter().map(|_| self.allocate_object_id()).collect();
        for (&page_idx, &page_id) in pages.iter().zip(&page_ids) {
            if let Err(e) = cancel.check() {
                self.next_object_id = first_free_id;
                return Err(e);
            }
            merged.id_map.entry(page_refs[page_idx].id).or_insert(page_id);
        }
        for (&page_idx, &page_id) in pages.iter().zip(&page_ids) {
            if let Err(e) = cancel.check() {
                self.next_object_id = first_free_id;
                return Err(e);
            }
            let page = self.import_page_from_document(
                &mut merged,
                page_refs[page_idx],
                page_id,
                &source_pages,
            )?;
            merged.pages.push(page);
        }
        self.carry_merged_acroform(&mut merged, &source_pages)?;
        merged.doc.clear_object_cache();
        let first_merged = self.merged_page_total();
        self.merged_sources.push(merged);
        self.page_order
            .extend((first_merged..first_merged + pages.len()).map(PageSlot::merged));
        self.is_modified = true;
        Ok(pages.len())
    }

    /// Record the merged document's root form fields under their new object
    /// ids, plus its `/DR` and `/DA`. Without this the imported widgets are
    /// on the page but in no `/AcroForm /Fields` array, so the saved file
    /// has fields no reader can find and no setter can reach.
    ///
    /// A field whose widgets all live on pages this merge did not import is
    /// skipped: its object was never renumbered, so it has no identity here.
    fn carry_merged_acroform(&mut self, merged: &mut MergedSource, source_pages: &[u32]) -> Result<()> {
        let source_doc = &merged.doc;
        let catalog = match source_doc.catalog() {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };
        let acroform_entry = catalog.as_dict().and_then(|d| d.get("AcroForm")).cloned();
        let acroform: HashMap<String, Object> = match acroform_entry {
            Some(Object::Dictionary(d)) => d,
            Some(Object::Reference(r)) => match source_doc.load_object(r) {
                Ok(Object::Dictionary(d)) => d,
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };

        let fields = match acroform.get("Fields") {
            Some(Object::Array(arr)) => arr.clone(),
            Some(Object::Reference(r)) => match source_doc.load_object(*r) {
                Ok(Object::Array(arr)) => arr,
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        // The /DR value stays in the merged document's numbering; the save
        // remaps it. Its resource objects ride with the last imported
        // page — the save writes every merged page's object list.
        let mut dr = None;
        if self.merged_acroform_dr.is_none() {
            if let Some(value) = acroform.get("DR").cloned() {
                self.walk_merge_ids(
                    source_doc,
                    &value,
                    &mut merged.id_map,
                    &mut merged.objects,
                    source_pages,
                )?;
                dr = Some(value);
            }
        }

        for entry in &fields {
            if let Some(field_ref) = entry.as_reference() {
                if let Some(&new_id) = merged.id_map.get(&field_ref.id) {
                    self.merged_form_fields.push(ObjectRef::new(new_id, 0));
                }
            }
        }
        if self.merged_acroform_da.is_none() {
            if let Some(da @ Object::String(_)) = acroform.get("DA") {
                self.merged_acroform_da = Some(da.clone());
            }
        }
        if let Some(value) = dr {
            let total = merged.objects.len();
            if let Some(last) = merged.pages.last_mut() {
                last.objects.end = total;
            }
            self.merged_acroform_dr = Some((self.merged_sources.len(), value));
        }
        Ok(())
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: the merge walk records ids, never object bytes ──
    /// Records the page at `page_ref` of a merged document: the new id of
    /// every object it reaches that the merge has not met yet. The page
    /// dictionary itself is not stored; the save reads it again.
    ///
    /// `merged.id_map` spans every page of one merge, so shared objects are
    /// imported once and the merge can look up a field's new id afterwards.
    /// `source_pages` are the merged document's page ids, sorted: the walk
    /// never follows a reference to one the merge did not import.
    fn import_page_from_document(
        &mut self,
        merged: &mut MergedSource,
        page_ref: ObjectRef,
        page_id: u32,
        source_pages: &[u32],
    ) -> Result<MergedPageImport> {
        let mut page = merged.doc.load_object(page_ref)?;
        let first = merged.objects.len();
        // The page tree is never followed: the save sets /Parent. What the
        // page inherits from it is imported with the page instead.
        if let Object::Dictionary(dict) = &mut page {
            if let Some(Object::Reference(parent)) = dict.remove("Parent") {
                self.resolve_inherited(merged, parent, source_pages)?;
            }
        }
        self.walk_merge_ids(
            &merged.doc,
            &page,
            &mut merged.id_map,
            &mut merged.objects,
            source_pages,
        )?;
        Ok(MergedPageImport { page_ref, page_id, objects: first..merged.objects.len() })
    }

    /// Fills `merged.inherited` for the page-tree node `node` and every
    /// ancestor not yet resolved: the inheritable attributes (§7.7.3.4) a
    /// page below the node gets from its ancestors, nearest value winning.
    /// The objects those values reference join the walk once, with the
    /// node that first holds them.
    fn resolve_inherited(
        &mut self,
        merged: &mut MergedSource,
        node: ObjectRef,
        source_pages: &[u32],
    ) -> Result<()> {
        let mut chain: Vec<(u32, HashMap<String, Object>)> = Vec::new();
        let mut at = Some(node);
        while let Some(r) = at {
            if merged.inherited.contains_key(&r.id) || chain.iter().any(|(id, _)| *id == r.id) {
                break;
            }
            let dict = match merged.doc.load_object(r)? {
                Object::Dictionary(d) => d,
                _ => HashMap::new(),
            };
            at = dict.get("Parent").and_then(|p| p.as_reference());
            chain.push((r.id, dict));
        }
        let mut inherited: Vec<(String, Object)> = at
            .and_then(|r| merged.inherited.get(&r.id).cloned())
            .unwrap_or_default();
        for (id, mut dict) in chain.into_iter().rev() {
            for key in INHERITABLE {
                if let Some(value) = dict.remove(key) {
                    self.walk_merge_ids(
                        &merged.doc,
                        &value,
                        &mut merged.id_map,
                        &mut merged.objects,
                        source_pages,
                    )?;
                    inherited.retain(|(k, _)| k != key);
                    inherited.push((key.to_string(), value));
                }
            }
            merged.inherited.insert(id, inherited.clone());
        }
        Ok(())
    }

    /// A merged page's dictionary as the save writes it before remapping:
    /// the page's own entries plus the inheritable ones it lacks, with no
    /// /Parent.
    fn merged_page_object(source: &MergedSource, page_ref: ObjectRef) -> Result<Object> {
        let mut page = source.doc.load_object(page_ref)?;
        if let Object::Dictionary(dict) = &mut page {
            if let Some(Object::Reference(parent)) = dict.remove("Parent") {
                for (key, value) in source.inherited.get(&parent.id).into_iter().flatten() {
                    dict.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
        }
        Ok(page)
    }

    /// Gives every object reachable from `root` a new id, once, pushing
    /// `(new, source)` for each into `out`. A reference already in `id_map`
    /// (met earlier, or an imported page) is not followed again, which also
    /// ends cycles. An annotation's /P is not followed: the save points it
    /// at the page the annotation lives on. Nor is a page of the merged
    /// document that the merge did not import (`source_pages`, sorted):
    /// it gets no id, so the save writes the reference as null.
    fn walk_merge_ids(
        &mut self,
        doc: &PdfDocument,
        root: &Object,
        id_map: &mut HashMap<u32, u32>,
        out: &mut Vec<(u32, u32)>,
        source_pages: &[u32],
    ) -> Result<()> {
        let mut pending: Vec<ObjectRef> = Vec::new();
        Self::push_walk_refs(root, &mut pending);
        while let Some(r) = pending.pop() {
            if id_map.contains_key(&r.id) || source_pages.binary_search(&r.id).is_ok() {
                continue;
            }
            let new_id = self.allocate_object_id();
            id_map.insert(r.id, new_id);
            out.push((new_id, r.id));
            Self::push_walk_refs(&doc.load_object(r)?, &mut pending);
        }
        Ok(())
    }

    /// A dictionary's entries in key order. A HashMap's own order differs
    /// per instance, so anything that allocates ids or writes bytes while it
    /// walks a dictionary walks this instead.
    fn sorted_entries(dict: &HashMap<String, Object>) -> Vec<(&String, &Object)> {
        let mut entries: Vec<_> = dict.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
    }

    /// The references the merge walk follows out of `obj`.
    fn push_walk_refs(obj: &Object, out: &mut Vec<ObjectRef>) {
        fn push_dict_refs(dict: &HashMap<String, Object>, out: &mut Vec<ObjectRef>) {
            let annotation = dict.contains_key("Subtype");
            // Key order, so the ids the walk allocates do not depend on a
            // HashMap's per-instance order.
            for (key, value) in DocumentEditor::sorted_entries(dict) {
                if annotation && key == "P" && matches!(value, Object::Reference(_)) {
                    continue;
                }
                DocumentEditor::push_walk_refs(value, out);
            }
        }
        match obj {
            Object::Reference(r) => out.push(*r),
            Object::Array(arr) => arr.iter().for_each(|o| Self::push_walk_refs(o, out)),
            Object::Dictionary(d) => push_dict_refs(d, out),
            Object::Stream { dict, .. } => push_dict_refs(dict, out),
            _ => {},
        }
    }

    /// Which merged pages the save writes, and which merged objects it
    /// leaves out. `kept[i]` says merged page `i` is in the page order. In
    /// a merged document that lost a page, an object is left out unless a
    /// kept page reaches it by the merge walk's rules; the document's
    /// `/AcroForm /DR`, which a form field of a kept page needs, counts as
    /// reached. An object two pages share is reached when either is kept.
    /// A merged document that lost no page leaves nothing out, and costs no
    /// walk. The walk applies `cuts` (the resource prune's, by output id) to
    /// the objects it reads, so what only a dropped resource entry reached
    /// is left out; a document with a cut is walked even when it lost no
    /// page.
    fn merged_prune(
        &self,
        cuts: &HashMap<u32, crate::host::resource_prune::Cuts>,
    ) -> Result<(Vec<bool>, HashSet<u32>)> {
        use crate::host::resource_prune::apply;

        let mut kept = vec![false; self.merged_page_total()];
        for slot in &self.page_order {
            crate::cancel::poll()?;
            if let PageKey::Merged(index) = slot.key() {
                kept[index] = true;
            }
        }
        let mut left_out = HashSet::new();
        let mut first = 0;
        for (source_index, source) in self.merged_sources.iter().enumerate() {
            let pages = first..first + source.pages.len();
            first = pages.end;
            let cut_here = source.pages.iter().any(|p| cuts.contains_key(&p.page_id))
                || source.objects.iter().any(|(out, _)| cuts.contains_key(out));
            if kept[pages.clone()].iter().all(|&k| k) && !cut_here {
                continue;
            }
            let imported: HashSet<u32> = source.pages.iter().map(|p| p.page_ref.id).collect();
            let mut reached: HashSet<u32> = HashSet::new();
            let mut pending: Vec<ObjectRef> = Vec::new();
            for (page, _) in source.pages.iter().zip(&kept[pages.clone()]).filter(|(_, &k)| k) {
                crate::cancel::poll()?;
                let mut page_object = Self::merged_page_object(source, page.page_ref)?;
                if let Some(cuts) = cuts.get(&page.page_id) {
                    page_object = apply(page_object, cuts);
                }
                Self::push_walk_refs(&page_object, &mut pending);
            }
            if let Some((owner, dr)) = &self.merged_acroform_dr {
                if *owner == source_index {
                    Self::push_walk_refs(dr, &mut pending);
                }
            }
            while let Some(r) = pending.pop() {
                let Some(&new_id) = source.id_map.get(&r.id) else { continue };
                if imported.contains(&r.id) || !reached.insert(new_id) {
                    continue;
                }
                let mut object = source.doc.load_object(r)?;
                if let Some(cuts) = cuts.get(&new_id) {
                    object = apply(object, cuts);
                }
                Self::push_walk_refs(&object, &mut pending);
            }
            source.doc.clear_object_cache();
            for page in &source.pages {
                crate::cancel::poll()?;
                for &(new_id, _) in &source.objects[page.objects.clone()] {
                    if !reached.contains(&new_id) {
                        left_out.insert(new_id);
                    }
                }
            }
        }
        Ok((kept, left_out))
    }

    /// The resource prune's cuts for the merged pages the save writes: by
    /// output id, applied to the object of the merged document before
    /// remapping.
    fn merged_resource_prune(
        &self,
        merged_kept: &[bool],
        merged_left_out: &HashSet<u32>,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<HashMap<u32, crate::host::resource_prune::Cuts>> {
        use crate::host::resource_prune::{plan, Scope, ScopePage};

        let unstaged = HashMap::new();
        let none = HashSet::new();
        let mut by_output = HashMap::new();
        let mut cut = HashMap::new();
        let mut first = 0;
        for source in &self.merged_sources {
            let range = first..first + source.pages.len();
            first = range.end;
            let mut pages = Vec::new();
            let mut writers: Vec<u32> = Vec::new();
            for (page, _) in source.pages.iter().zip(&merged_kept[range]).filter(|(_, &kept)| kept) {
                cancel.check()?;
                pages.push(ScopePage { id: page.page_ref.id, held: false });
                writers.push(page.page_ref.id);
            }
            if pages.is_empty() {
                continue;
            }
            writers.extend(
                source
                    .objects
                    .iter()
                    .filter(|(out, _)| !merged_left_out.contains(out))
                    .map(|&(_, src)| src),
            );
            writers.sort_unstable();
            writers.dedup();
            let page_object = |id: u32| Self::merged_page_object(source, ObjectRef::new(id, 0)).ok();
            plan(
                Scope {
                    doc: &source.doc,
                    staged: &unstaged,
                    overrides: &unstaged,
                    cuts: &mut cut,
                    excluded: &none,
                    pages,
                    page_object: Some(&page_object),
                    writers: Some(writers),
                    glyphs: None,
                },
                cancel,
            )?;
            for (id, cuts) in cut.drain() {
                if let Some(&out) = source.id_map.get(&id) {
                    by_output.insert(out, cuts);
                }
            }
            source.doc.clear_object_cache();
        }
        Ok(by_output)
    }
    // ── end pdf_manipulator patch ──

    /// Merge specific pages from another PDF into this document.
    ///
    /// # Arguments
    ///
    /// * `source_path` - Path to the PDF to merge from
    /// * `pages` - Indices of pages to merge (0-based)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("main.pdf")?;
    /// editor.merge_pages_from("source.pdf", &[0, 2, 4])?;  // Merge pages 1, 3, 5
    /// editor.save("combined.pdf")?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn merge_pages_from(
        &mut self,
        source_path: impl AsRef<Path>,
        pages: &[usize],
    ) -> Result<usize> {
        let source_doc = PdfDocument::open(source_path.as_ref())?;
        // ── pdf_manipulator patch: shared core with merge_pages_from_reader ──
        self.merge_pages_from_document(source_doc, pages)
        // ── end pdf_manipulator patch ──
    }

    // === Internal save helpers ===

    /// Build the Info dictionary object for the trailer.
    fn build_info_object(&self) -> Option<Object> {
        self.modified_info.as_ref().map(|info| info.to_object())
    }

    // ── pdf_manipulator patch: the public incremental save streams the source ──
    /// Write an incremental update to the PDF: the source copied through a
    /// read buffer, then the update section `host::incremental` builds. The
    /// source is never read whole, and the edits an append cannot carry are
    /// refused before the first byte is written.
    #[cfg(not(target_arch = "wasm32"))]
    fn write_incremental(&mut self, path: impl AsRef<Path>, options: &SaveOptions) -> Result<()> {
        use crate::host::positioned_write::CountingWriter;
        /// The file is created at the first byte, so a refusal, decided
        /// before any is written, leaves the path as it was.
        struct Lazy<'p> {
            path: &'p Path,
            file: Option<BufWriter<File>>,
        }
        impl std::io::Write for Lazy<'_> {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                if self.file.is_none() {
                    self.file = Some(BufWriter::new(File::create(self.path)?));
                }
                self.file.as_mut().map_or(Ok(0), |f| f.write(buf))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                self.file.as_mut().map_or(Ok(()), |f| f.flush())
            }
        }
        let mut writer = CountingWriter::new(Lazy { path: path.as_ref(), file: None });
        crate::host::incremental::write(self, &mut writer, options, &crate::cancel::CancelCheck::none())?;
        writer.flush()?;
        Ok(())
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: what an incremental save appends ──
    /// Why an incremental save cannot carry the staged edits, or `None`.
    ///
    /// An append keeps every original byte, so it can only carry an edit
    /// whose whole effect is a set of replaced or new objects. It refuses
    /// an edit that removes content (the original bytes would still hold
    /// it) and an edit the full rewrite materializes in its page loop:
    /// every editor field the page loop reads at write time must be
    /// listed here, or an incremental save silently drops that edit.
    pub(crate) fn incremental_refusal(&self) -> Option<&'static str> {
        // The source pages alone: a merge adds pages, which the next check names.
        let identity = self
            .page_order
            .iter()
            .map(|slot| slot.key())
            .filter(|key| matches!(key, PageKey::Source(_)))
            .eq((0..self.original_page_count).map(PageKey::Source));
        Some(if !identity {
            "pages were removed or reordered, and the original bytes would keep removed ones"
        } else if !self.merged_sources.is_empty() || !self.merged_form_fields.is_empty() {
            "pages were merged in"
        } else if !self.apply_redactions_pages.is_empty()
            || !self.redaction_regions.is_empty()
            || !self.redacted_pages.is_empty()
            || !self.redacted_orphan_ids.is_empty()
        {
            "content was redacted or scrubbed, and the original bytes would keep it"
        } else if !self.deleted_form_fields.is_empty() || self.remove_acroform {
            "form fields were removed, and the original bytes would keep them"
        } else if self.structure_modified
            || !self.modified_content.is_empty()
            || !self.overlay_additions.is_empty()
            || !self.erase_regions.is_empty()
            || !self.image_modifications.is_empty()
        {
            "page content was drawn, erased or rebuilt"
        } else if !self.modified_annotations.is_empty()
            || !self.flatten_annotations_pages.is_empty()
            || !self.flatten_forms_pages.is_empty()
        {
            "annotations were changed or flattened"
        } else if !self.embedded_files.is_empty() {
            "files were embedded"
        } else if !self.new_objects.is_empty()
            || self.modified_form_fields.values().any(|w| w.is_new())
        {
            "form fields or objects were added"
        } else if self.source.is_encrypted() {
            "the source is encrypted"
        } else if self.source.xref_repaired() {
            "the source's cross-reference table was broken and rebuilt on open"
        } else if self.source.header_offset() != 0 {
            "bytes precede the PDF header"
        } else {
            return None;
        })
    }

    /// Whether the staged edits only set form field values: what a
    /// certifying signature's DocMDP levels 2 and 3 allow. It fails
    /// closed: an object staged by an earlier save's form flush counts
    /// as another change.
    pub(crate) fn incremental_value_only(&self) -> bool {
        self.modified_objects.is_empty()
            && self.modified_info.is_none()
            && self.modified_page_props.is_empty()
            && self.modified_form_fields.values().filter(|w| w.is_modified()).all(|w| {
                w.modified_flags.is_none()
                    && w.modified_tooltip.is_none()
                    && w.modified_rect.is_none()
                    && w.modified_default_value.is_none()
                    && w.modified_max_length.is_none()
                    && w.modified_alignment.is_none()
                    && w.modified_default_appearance.is_none()
                    && w.modified_background_color.is_none()
                    && w.modified_border_color.is_none()
                    && w.modified_border_width.is_none()
                    && w.removed_properties.is_empty()
            })
    }

    /// The objects an incremental save appends. Call it only after
    /// [`Self::incremental_refusal`] returned `None`.
    pub(crate) fn incremental_objects(
        &mut self,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<crate::host::incremental::IncrementalChanges> {
        if self.acroform_modified {
            self.flush_form_fields_to_modified_objects()?;
        }
        let mut objects: std::collections::BTreeMap<u32, Object> =
            self.modified_objects.iter().map(|(&id, o)| (id, o.clone())).collect();
        if !self.modified_page_props.is_empty() {
            let refs = self.source.all_page_refs_cancellable(cancel)?;
            let mut props: Vec<_> = self.modified_page_props.iter().collect();
            props.sort_by_key(|(i, _)| **i);
            for (&index, p) in props {
                cancel.check()?;
                let r = *refs.get(index).ok_or_else(|| {
                    Error::InvalidPdf(format!("page {index} is not in the source"))
                })?;
                let base = match objects.get(&r.id) {
                    Some(o) => o.clone(),
                    None => self.source.load_object(r)?,
                };
                objects.insert(r.id, self.apply_page_props_to_object(&base, p)?);
            }
        }
        let mut info_id = None;
        // The id a new /Info takes belongs to this save: the editor's own
        // next id stays what it was.
        let mut next_id = self.next_object_id;
        if let Some(info) = self.build_info_object() {
            let id = match self
                .source
                .trailer()
                .as_dict()
                .and_then(|d| d.get("Info"))
                .and_then(|i| i.as_reference())
            {
                Some(r) => r.id,
                None => {
                    next_id += 1;
                    next_id - 1
                },
            };
            objects.insert(id, info);
            info_id = Some(id);
        }
        Ok(crate::host::incremental::IncrementalChanges {
            objects: objects.into_iter().collect(),
            info_id,
            next_id,
        })
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: button on-state resolution (#215) ──

    /// The on-state names a widget offers, read from its `/AP` `/N` state
    /// dictionary with `/Off` removed (ISO 32000-1 §12.7.4.2.3).
    ///
    /// Empty when `/AP` `/N` is a single stream rather than a state dictionary
    /// — the shape a writer leaves when it drew only the current state. The
    /// caller then falls back to regenerating an appearance.
    fn widget_on_states(&mut self, widget: &HashMap<String, Object>) -> Vec<String> {
        let ap = match widget.get("AP") {
            Some(Object::Dictionary(d)) => d.clone(),
            Some(Object::Reference(r)) => match self.source.load_object(*r) {
                Ok(Object::Dictionary(d)) => d,
                _ => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        let normal = match ap.get("N") {
            Some(Object::Dictionary(d)) => d.clone(),
            Some(Object::Reference(r)) => match self.source.load_object(*r) {
                Ok(Object::Dictionary(d)) => d,
                _ => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        // A Form XObject is the single-stream shape, not a state dictionary.
        if normal.get("Type").and_then(|t| t.as_name()) == Some("XObject") {
            return Vec::new();
        }
        let mut states: Vec<String> = normal.keys().filter(|k| *k != "Off").cloned().collect();
        states.sort();
        states
    }

    /// The appearance-state name a button field's `/V` should carry for
    /// `value`, or `None` for the off state.
    ///
    /// A button's value is a *name* — one of the states its `/AP` `/N` offers —
    /// never text to draw. Bindings that can only send a string (the Dart
    /// bridge before `setCheckboxFieldValue`, FDF imports) therefore arrive as
    /// `Text`, and are read here as that name.
    ///
    /// `available` is the widget's own on-states, and an exact match against it
    /// always wins: a Yes/No radio group really does have a state named `No`,
    /// so the word can only be read as "off" once the widget has said it offers
    /// no such state. After that, a word that plainly means on or off is taken
    /// at its word — `Boolean(true)` and `"yes"` alike take the widget's only
    /// on-state, which is why a box named `/On` no longer needs the caller to
    /// know that. Any other unrecognised name selects nothing rather than
    /// guessing which state was meant.
    fn button_on_state(
        value: &crate::editor::form_fields::FormFieldValue,
        available: &[String],
    ) -> Option<String> {
        use crate::editor::form_fields::FormFieldValue;
        // The single state a widget offers, when it offers exactly one — what
        // a value that means "on" without naming a state must resolve to.
        let sole_on_state = || match available {
            [one] => Some(one.clone()),
            _ => None,
        };
        match value {
            FormFieldValue::Boolean(true) => {
                sole_on_state().or_else(|| Some("Yes".to_string()))
            },
            FormFieldValue::Boolean(false) | FormFieldValue::None => None,
            FormFieldValue::Text(s) | FormFieldValue::Choice(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return None;
                }
                // State names are case-sensitive; an offered name is the answer.
                if available.iter().any(|a| a == s) {
                    return Some(s.to_string());
                }
                let word = s.to_ascii_lowercase();
                if matches!(word.as_str(), "off" | "no" | "false" | "0") {
                    return None;
                }
                if matches!(word.as_str(), "yes" | "on" | "true" | "1") {
                    return sole_on_state().or_else(|| {
                        // No state dictionary to resolve against — the caller's
                        // word is all there is.
                        available.is_empty().then(|| s.to_string())
                    });
                }
                // An unrecognised name: trust it only when there is no state
                // dictionary that could have contradicted it.
                available.is_empty().then(|| s.to_string())
            },
            FormFieldValue::MultiChoice(_) => None,
        }
    }

    /// Every on-state a *field* offers: the union across its `/Kids` for a radio
    /// group, or the merged field+widget's own states.
    ///
    /// Both the save path and the flattener resolve a value against this, never
    /// against one kid's states. Resolving per kid would answer a question the
    /// kid cannot see: `"on"` names no state in a `/Red` + `/Blue` group, but
    /// each kid alone has exactly one state, so every kid would light while the
    /// save path — which sees both — wrote `/Off`.
    pub(crate) fn field_on_states(&mut self, field: &HashMap<String, Object>) -> Vec<String> {
        let widget_refs = self.field_widget_refs(field);
        if widget_refs.is_empty() {
            return self.widget_on_states(field);
        }
        let mut all = Vec::new();
        for wref in widget_refs {
            if let Ok(w) = self.source.load_object(wref) {
                if let Some(wd) = w.as_dict() {
                    all.extend(self.widget_on_states(&wd.clone()));
                }
            }
        }
        all.sort();
        all.dedup();
        all
    }

    /// The widget dictionaries a field's `/AS` must be written to: its `/Kids`
    /// for a radio group, or the field dictionary itself for a merged
    /// field+widget (ISO 32000-1 §12.7.4.1).
    fn field_widget_refs(&mut self, field: &HashMap<String, Object>) -> Vec<ObjectRef> {
        let kids = match field.get("Kids") {
            Some(Object::Array(a)) => a.clone(),
            Some(Object::Reference(r)) => match self.source.load_object(*r) {
                Ok(Object::Array(a)) => a,
                _ => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        kids.iter().filter_map(|k| k.as_reference()).collect()
    }

    // ── end pdf_manipulator patch ──

    /// Sync modified form fields into `modified_objects` for incremental save.
    ///
    /// For each modified field wrapper, loads the original PDF annotation object,
    /// updates its `/V` entry (and `/AS` for checkboxes/radios), and inserts the
    /// updated object into `modified_objects`. Also sets `/NeedAppearances true`
    /// in the AcroForm dictionary so PDF readers regenerate appearance streams.
    fn flush_form_fields_to_modified_objects(&mut self) -> Result<()> {
        use crate::editor::form_fields::FormFieldValue;
        use crate::extractors::forms::FieldType;

        // ── pdf_manipulator patch: a property-only change (tooltip, rect, max
        // length, alignment, flags, colors, border, appearance) must reach the
        // saved object graph even when the field's /V was never touched — the
        // wrapper's `is_modified()` already covers that; only `modified_value`
        // gated the loop before, silently dropping every other setter on save
        // (they only ever took effect through `flattenForms`, which reads the
        // wrapper directly instead of going through this function). Carry every
        // `modified_*` field, not just the typed value. ──
        struct FieldFlush {
            obj_ref: ObjectRef,
            value: Option<FormFieldValue>,
            is_button: bool,
            is_choice: bool,
            tooltip: Option<String>,
            rect: Option<Rect>,
            max_length: Option<u32>,
            alignment: Option<u32>,
            flags: Option<u32>,
            background_color: Option<[f32; 3]>,
            border_color: Option<[f32; 3]>,
            border_width: Option<f32>,
            default_appearance: Option<String>,
            removed: Vec<&'static str>,
        }

        let fields_to_flush: Vec<FieldFlush> = {
            let mut result = Vec::new();
            // name order, two fields on one widget object resolve the same way every save
            let mut by_name: Vec<_> = self.modified_form_fields.iter().collect();
            by_name.sort_by(|a, b| a.0.cmp(b.0));
            for (_, wrapper) in by_name {
                if !wrapper.is_modified() || wrapper.is_new() {
                    continue;
                }
                let obj_ref = match wrapper.object_ref() {
                    Some(r) => r,
                    None => continue,
                };
                let is_button = wrapper
                    .field_type()
                    .map(|ft| *ft == FieldType::Button)
                    .unwrap_or(false);
                let is_choice = wrapper
                    .field_type()
                    .map(|ft| *ft == FieldType::Choice)
                    .unwrap_or(false);
                result.push(FieldFlush {
                    obj_ref,
                    value: wrapper.modified_value.clone(),
                    is_button,
                    is_choice,
                    tooltip: wrapper.modified_tooltip.clone(),
                    rect: wrapper.modified_rect,
                    max_length: wrapper.modified_max_length,
                    alignment: wrapper.modified_alignment,
                    flags: wrapper.modified_flags,
                    background_color: wrapper.modified_background_color,
                    border_color: wrapper.modified_border_color,
                    border_width: wrapper.modified_border_width,
                    default_appearance: wrapper.modified_default_appearance.clone(),
                    removed: wrapper.removed_properties.clone(),
                });
            }
            result
        };
        // ── end pdf_manipulator patch ──

        // Now load and update each field object
        for flush in &fields_to_flush {
            let obj_ref = flush.obj_ref;
            let original = self.source.load_object(obj_ref)?;

            let dict = match original.as_dict() {
                Some(d) => d.clone(),
                None => continue,
            };

            let mut new_dict = dict;

            // ── pdf_manipulator patch: button /V and /AS are names (#215) ──
            // A button field's /V and every widget's /AS are appearance-state
            // *names*, not text strings (ISO 32000-1 §12.7.4.2.3). This wrote
            // `Object::from(&FormFieldValue)` into both, which for the string
            // lane emits `/V (Yes)` — a text string no viewer matches against
            // /AP /N — and for the boolean lane emits a hardcoded `/Yes` that
            // misses a box whose on-state is named `/On`. Resolve the name from
            // the widget's own /AP /N instead, and put /AS on the widget that
            // owns it: a radio group's states live on its /Kids, not on the
            // parent field dictionary.
            if let Some(new_value) = &flush.value {
                if flush.is_button {
                    let widget_refs = self.field_widget_refs(&new_dict);
                    let available = self.field_on_states(&new_dict);
                    let target = Self::button_on_state(new_value, &available);
                    let v_name = target.clone().unwrap_or_else(|| "Off".to_string());
                    new_dict.insert("V".to_string(), Object::Name(v_name));

                    if widget_refs.is_empty() {
                        // Merged field + widget — /AS belongs on this dictionary.
                        new_dict.insert(
                            "AS".to_string(),
                            Object::Name(target.clone().unwrap_or_else(|| "Off".to_string())),
                        );
                    } else {
                        // Radio group — exactly the kid offering the chosen state
                        // turns on; every sibling goes to /Off.
                        for wref in widget_refs {
                            let kid = match self.source.load_object(wref) {
                                Ok(k) => k,
                                Err(_) => continue,
                            };
                            let kid_dict = match kid.as_dict() {
                                Some(d) => d.clone(),
                                None => continue,
                            };
                            let offers = self.widget_on_states(&kid_dict);
                            let kid_state = match &target {
                                Some(t) if offers.iter().any(|o| o == t) => t.clone(),
                                _ => "Off".to_string(),
                            };
                            let mut new_kid = kid_dict;
                            new_kid.insert("AS".to_string(), Object::Name(kid_state));
                            self.modified_objects
                                .insert(wref.id, Object::Dictionary(new_kid));
                        }
                    }
                } else {
                    // A value that is no value leaves no /V (and no /I) behind;
                    // a list box's /I names the selected options by index.
                    match new_value {
                        FormFieldValue::None => {
                            new_dict.remove("V");
                            new_dict.remove("I");
                        },
                        _ => {
                            new_dict.insert("V".to_string(), Object::from(new_value));
                            match crate::host::form_choice::selected_indexes(
                                &self.source,
                                &new_dict,
                                new_value,
                            ) {
                                Some(indexes) if flush.is_choice => {
                                    new_dict.insert("I".to_string(), indexes);
                                },
                                _ => {
                                    if flush.is_choice {
                                        new_dict.remove("I");
                                    }
                                },
                            }
                        },
                    }
                }
            }
            // ── end pdf_manipulator patch ──

            // ── pdf_manipulator patch: property-only mutations (tooltip, rect,
            // max length, alignment, flags, colors, border, appearance) written
            // into the same field object /V was written to above. The reader
            // (`FormExtractor::extract_fields`) resolves every one of these keys
            // from the field's own dict regardless of whether the field is
            // merged with its widget or has separate `/Kids`, so writing them
            // here is what `formField(name)` reads back. `/Rect` and `/MK` are
            // additionally mirrored onto every kid widget when the field has
            // one, so a viewer reading the widget annotation (not the field
            // dict) sees the same rect/appearance characteristics.
            let widget_refs = self.field_widget_refs(&new_dict);

            if let Some(ref tooltip) = flush.tooltip {
                new_dict.insert("TU".to_string(), Object::text_string(tooltip));
            }
            if let Some(rect) = flush.rect {
                let rect_obj = Object::Array(vec![
                    Object::Real(rect.x as f64),
                    Object::Real(rect.y as f64),
                    Object::Real((rect.x + rect.width) as f64),
                    Object::Real((rect.y + rect.height) as f64),
                ]);
                new_dict.insert("Rect".to_string(), rect_obj.clone());
                for &wref in &widget_refs {
                    if let Ok(kid) = self.source.load_object(wref) {
                        if let Some(kd) = kid.as_dict() {
                            let mut new_kid = kd.clone();
                            new_kid.insert("Rect".to_string(), rect_obj.clone());
                            self.modified_objects.insert(wref.id, Object::Dictionary(new_kid));
                        }
                    }
                }
            }
            if let Some(max_length) = flush.max_length {
                new_dict.insert("MaxLen".to_string(), Object::Integer(max_length as i64));
            }
            if let Some(alignment) = flush.alignment {
                new_dict.insert("Q".to_string(), Object::Integer(alignment as i64));
            }
            if let Some(flags) = flush.flags {
                new_dict.insert("Ff".to_string(), Object::Integer(flags as i64));
            }
            if flush.background_color.is_some() || flush.border_color.is_some() {
                let mut mk = new_dict
                    .get("MK")
                    .and_then(|o| self.source.resolve_object(o).ok())
                    .and_then(|o| o.as_dict().cloned())
                    .unwrap_or_default();
                if let Some(bg) = flush.background_color {
                    mk.insert(
                        "BG".to_string(),
                        Object::Array(bg.iter().map(|c| Object::Real(*c as f64)).collect()),
                    );
                }
                if let Some(bc) = flush.border_color {
                    mk.insert(
                        "BC".to_string(),
                        Object::Array(bc.iter().map(|c| Object::Real(*c as f64)).collect()),
                    );
                }
                let mk_obj = Object::Dictionary(mk);
                new_dict.insert("MK".to_string(), mk_obj.clone());
                for &wref in &widget_refs {
                    if let Ok(kid) = self.source.load_object(wref) {
                        if let Some(kd) = kid.as_dict() {
                            let mut new_kid = kd.clone();
                            new_kid.insert("MK".to_string(), mk_obj.clone());
                            self.modified_objects.insert(wref.id, Object::Dictionary(new_kid));
                        }
                    }
                }
            }
            if let Some(width) = flush.border_width {
                let mut bs = new_dict
                    .get("BS")
                    .and_then(|o| self.source.resolve_object(o).ok())
                    .and_then(|o| o.as_dict().cloned())
                    .unwrap_or_default();
                bs.insert("W".to_string(), Object::Real(width as f64));
                new_dict.insert("BS".to_string(), Object::Dictionary(bs));
            }
            if let Some(ref da) = flush.default_appearance {
                new_dict.insert("DA".to_string(), Object::text_string(da));
            }
            // Removed properties: the key is deleted, and an /MK left with no
            // entry is dropped, from the field and from every kid widget.
            if flush.removed.contains(&"TU") {
                new_dict.remove("TU");
            }
            if flush.removed.contains(&"MaxLen") {
                new_dict.remove("MaxLen");
            }
            let drop_colors: Vec<&str> = ["BG", "BC"]
                .into_iter()
                .filter(|key| flush.removed.contains(key))
                .collect();
            if !drop_colors.is_empty() {
                let without = |mk: Option<&Object>, source: &PdfDocument| -> Option<Object> {
                    let mut mk = mk
                        .and_then(|o| source.resolve_object(o).ok())
                        .and_then(|o| o.as_dict().cloned())?;
                    for key in &drop_colors {
                        mk.remove(*key);
                    }
                    (!mk.is_empty()).then_some(Object::Dictionary(mk))
                };
                match without(new_dict.get("MK"), &self.source) {
                    Some(mk) => {
                        new_dict.insert("MK".to_string(), mk);
                    },
                    None => {
                        new_dict.remove("MK");
                    },
                }
                for &wref in &widget_refs {
                    let kid = match self.modified_objects.get(&wref.id) {
                        Some(staged) => Some(staged.clone()),
                        None => self.source.load_object(wref).ok(),
                    };
                    if let Some(kd) = kid.as_ref().and_then(|k| k.as_dict()) {
                        let mut new_kid = kd.clone();
                        match without(kd.get("MK"), &self.source) {
                            Some(mk) => {
                                new_kid.insert("MK".to_string(), mk);
                            },
                            None => {
                                new_kid.remove("MK");
                            },
                        }
                        self.modified_objects.insert(wref.id, Object::Dictionary(new_kid));
                    }
                }
            }
            // ── end pdf_manipulator patch ──

            self.modified_objects
                .insert(obj_ref.id, Object::Dictionary(new_dict));
        }

        // Set /NeedAppearances true in the AcroForm dictionary
        if !fields_to_flush.is_empty() {
            let catalog = self.source.catalog()?;
            let catalog_dict = catalog
                .as_dict()
                .ok_or_else(|| Error::InvalidPdf("Catalog is not a dictionary".to_string()))?;

            if let Some(acroform_obj) = catalog_dict.get("AcroForm") {
                if let Some(acroform_ref) = acroform_obj.as_reference() {
                    // AcroForm is an indirect reference — load, modify, and write back
                    let acroform = self.source.load_object(acroform_ref)?;
                    if let Some(af_dict) = acroform.as_dict() {
                        let mut new_af = af_dict.clone();
                        new_af.insert("NeedAppearances".to_string(), Object::Boolean(true));
                        self.modified_objects
                            .insert(acroform_ref.id, Object::Dictionary(new_af));
                    }
                } else if let Some(af_dict) = acroform_obj.as_dict() {
                    // AcroForm is an inline dictionary in the catalog — modify catalog
                    let mut new_af = af_dict.clone();
                    new_af.insert("NeedAppearances".to_string(), Object::Boolean(true));
                    let mut new_catalog = catalog_dict.clone();
                    new_catalog.insert("AcroForm".to_string(), Object::Dictionary(new_af));

                    // Get catalog object ID from trailer /Root reference
                    if let Some(trailer_dict) = self.source.trailer().as_dict() {
                        if let Some(root_ref) =
                            trailer_dict.get("Root").and_then(|r| r.as_reference())
                        {
                            self.modified_objects
                                .insert(root_ref.id, Object::Dictionary(new_catalog));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ── pdf_manipulator patch: only its tests still call it; the host writer finds /Prev itself ──
    /// Find the offset of the previous xref table in the original PDF.
    #[cfg(test)]
    fn find_prev_xref_offset(&self, bytes: &[u8]) -> Result<u64> {
        // Search backwards from the end for "startxref"
        let search = b"startxref";
        let end = bytes.len();
        let mut pos = if end >= search.len() {
            end - search.len()
        } else {
            0
        };

        while pos > 0 {
            if bytes[pos..].starts_with(search) {
                // Found it - parse the offset that follows
                let after_keyword = pos + search.len();
                let remaining = &bytes[after_keyword..];

                // Skip whitespace and parse number
                let offset_str: String = remaining
                    .iter()
                    .skip_while(|&&b| b == b' ' || b == b'\n' || b == b'\r')
                    .take_while(|&&b| b.is_ascii_digit())
                    .map(|&b| b as char)
                    .collect();

                if let Ok(offset) = offset_str.parse::<u64>() {
                    return Ok(offset);
                }
            }
            pos = pos.saturating_sub(1);
        }

        Err(Error::InvalidPdf("Could not find startxref in original PDF".to_string()))
    }
    // ── end pdf_manipulator patch ──

    /// Write a full rewrite of the PDF.
    #[cfg(not(target_arch = "wasm32"))]
    fn write_full(&mut self, path: impl AsRef<Path>, options: &SaveOptions) -> Result<()> {
        // ── pdf_manipulator patch: route through the streaming writer ──
        use crate::host::positioned_write::CountingWriter;
        let file = File::create(path.as_ref())?;
        let mut writer = CountingWriter::new(BufWriter::new(file));
        self.write_full_to_writer(&mut writer, options, &crate::cancel::CancelCheck::none())
        // ── end pdf_manipulator patch ──
    }

    // ── pdf_manipulator patch: no decoded object stream outlives an operation ──
    /// Empties the source's object stream memo; the host calls it when an
    /// operation ends.
    pub(crate) fn release_objstm_memo(&self) {
        self.source.release_objstm_memo();
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: zero-clone BFS for GC ──
    /// Collect all object IDs reachable from the catalog root via BFS.
    ///
    /// Used by garbage collection: any source-document object not in this set is
    /// an orphan and can be omitted from the output.  The prune plan's
    /// overrides, then modified objects, are consulted first so that
    /// references introduced by edits are honoured, and the walk never
    /// enters an object the plan excludes (#261).
    ///
    /// Zero-clone: borrows cached objects via collect_refs_of() instead of
    /// cloning every Object. For modified objects, borrows from the HashMap
    /// directly. Uses a bit-vec for O(1) membership checks.
    ///
    /// With `written` (the save's cross-reference entries so far and the ids
    /// its written objects name), the walk starts from what the output
    /// already names and never reads the source version of an object the
    /// save has written: a catalog that drops /AcroForm or a page that drops
    /// its widgets does not keep their objects alive.
    ///
    /// A reference to a key of `aliases` (a merged duplicate) is followed
    /// as the reference to its value, which the save writes in its place.
    fn collect_reachable_ids(
        &self,
        prune: &crate::host::page_prune::PagePrune,
        written: Option<(&XrefEntries, &[(u32, u32)])>,
        aliases: &HashMap<u32, u32>,
        cancel: &crate::cancel::CancelCheck,
    ) -> std::collections::HashSet<u32> {
        fn push_refs(obj: &Object, out: &mut Vec<u32>) {
            match obj {
                Object::Reference(r) => out.push(r.id),
                Object::Array(arr) => arr.iter().for_each(|o| push_refs(o, out)),
                Object::Dictionary(d) => d.values().for_each(|o| push_refs(o, out)),
                // The save states an extent's /Length itself: the integer
                // object a source /Length names is not reached through it.
                Object::Stream { dict, body } => dict
                    .iter()
                    .filter(|(key, _)| {
                        !(key.as_str() == "Length" && matches!(body, StreamBody::Extent(_)))
                    })
                    .for_each(|(_, o)| push_refs(o, out)),
                _ => {}
            }
        }

        let max_id = self.next_object_id as usize + 1;
        let mut seen = vec![false; max_id];
        let mut queue: Vec<u32> = Vec::with_capacity(256);
        let mut refs_buf: Vec<u32> = Vec::with_capacity(32);
        // The page object of each redacted page, by id: the walk reads it as
        // the save writes it, not as the source has it.
        let redacted_page_at: HashMap<u32, usize> = self
            .redacted_pages
            .keys()
            .filter_map(|&src| self.source.get_page_ref(src).ok().map(|r| (r.id, src)))
            .collect();
        // an object stream is drained before the walk leaves it
        // Ids whose objects sit in an object stream wait here by container;
        // the walk reads the members of one stream together, so the stream
        // is decoded once instead of once per member.
        let mut by_stream: std::collections::BTreeMap<u32, Vec<u32>> =
            std::collections::BTreeMap::new();
        let mut stream_now: Option<u32> = None;
        let route = |ids: &[u32],
                     seen: &[bool],
                     queue: &mut Vec<u32>,
                     by_stream: &mut std::collections::BTreeMap<u32, Vec<u32>>| {
            for &id in ids {
                let id = aliases.get(&id).copied().unwrap_or(id);
                if seen.get(id as usize).copied() != Some(false) {
                    continue;
                }
                match self.source.container_stream_of(id) {
                    Some(stream) => by_stream.entry(stream).or_default().push(id),
                    None => queue.push(id),
                }
            }
        };

        if let Some((_, named)) = written {
            let named: Vec<u32> = named.iter().map(|&(target, _)| target).collect();
            route(&named, &seen, &mut queue, &mut by_stream);
        } else if let Some(r) = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Root"))
            .and_then(|v| v.as_reference())
        {
            queue.push(r.id);
        }
        if let Some(r) = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Info"))
            .and_then(|v| v.as_reference())
        {
            queue.push(r.id);
        }
        // The source root /Pages lists the nodes the save excludes; the
        // output's root lists the kept pages, so the walk enters them here.
        route(&prune.kept_pages, &seen, &mut queue, &mut by_stream);

        loop {
            if cancel.is_cancelled() {
                break;
            }
            // an object stream is drained before the walk leaves it
            let id = match queue.pop() {
                Some(id) => id,
                None => {
                    let stream = match stream_now.filter(|s| by_stream.contains_key(s)) {
                        Some(stream) => stream,
                        None => match by_stream.keys().next() {
                            Some(&stream) => stream,
                            None => break,
                        },
                    };
                    stream_now = Some(stream);
                    let pending = by_stream.get_mut(&stream).expect("stream was just found");
                    let id = pending.pop().expect("a pending list is never empty");
                    if pending.is_empty() {
                        by_stream.remove(&stream);
                    }
                    id
                },
            };
            let idx = id as usize;
            if idx >= max_id || seen[idx] || prune.excluded.contains(&id) {
                continue;
            }
            seen[idx] = true;

            // An object the save has written names what it wrote.
            if written.is_some_and(|(entries, _)| entries.contains_id(id)) {
                continue;
            }
            refs_buf.clear();
            // A redacted page writes its own /Resources: a source image is
            // reached only through a name a surviving `Do` still uses.
            if let Some(&src) = redacted_page_at.get(&id) {
                if let Some(page) = self.redacted_pages.get(&src) {
                    if let Some(dict) = self
                        .source
                        .load_object(ObjectRef::new(id, 0))
                        .ok()
                        .as_ref()
                        .and_then(|o| o.as_dict())
                    {
                        for (key, value) in dict {
                            if key.as_str() != "Resources" && key.as_str() != "Contents" {
                                push_refs(value, &mut refs_buf);
                            }
                        }
                    }
                    if let Ok(resources) = self.redacted_page_resources(src, page, &ScopeIds::default()) {
                        push_refs(&resources, &mut refs_buf);
                    }
                    // The copies the page queued name source objects too: the
                    // fonts and colour spaces of a form copy, the function of
                    // a mesh copy.
                    if let Ok(named) = self.copy_sources(src, page) {
                        for object in &named {
                            push_refs(object, &mut refs_buf);
                        }
                    }
                }
                route(&refs_buf, &seen, &mut queue, &mut by_stream);
                continue;
            }
            // The references of an object the resource prune cuts are those
            // that are left.
            if let Some(cuts) = prune.cuts.get(&id) {
                let base = prune
                    .overrides
                    .get(&id)
                    .or_else(|| self.modified_objects.get(&id))
                    .cloned()
                    .or_else(|| self.source.load_object(ObjectRef::new(id, 0)).ok());
                if let Some(base) = base {
                    push_refs(&crate::host::resource_prune::apply(base, cuts), &mut refs_buf);
                }
            } else if let Some(o) = prune.overrides.get(&id) {
                push_refs(o, &mut refs_buf);
            } else if let Some(m) = self.modified_objects.get(&id) {
                push_refs(m, &mut refs_buf);
            } else {
                self.source.collect_refs_of(id, &mut refs_buf);
            }
            route(&refs_buf, &seen, &mut queue, &mut by_stream);
        }

        seen.iter()
            .enumerate()
            .filter(|(_, &v)| v)
            .map(|(i, _)| i as u32)
            .collect()
    }

    // the written objects are closed under reference
    /// The first `(referenced id, referencing id)` of `references` that
    /// names an object the save has on hand and did not write: a staged
    /// object, a source object in use, or an id the save allocated. A
    /// reference the save nulls is never in `references`; one to an id the
    /// source itself lacks (no entry, or a free one) reads as null in the
    /// source too, so it is left alone.
    fn first_unwritten_reference(
        &self,
        references: &[(u32, u32)],
        written: &XrefEntries,
    ) -> Option<(u32, u32)> {
        let mut source_ids: Option<(Vec<u32>, u32)> = None;
        for &(target, writer_id) in references {
            if written.contains_id(target) {
                continue;
            }
            if self.modified_objects.contains_key(&target) {
                return Some((target, writer_id));
            }
            let (ids, bound) = source_ids.get_or_insert_with(|| {
                (self.source.all_object_ids(), Self::find_max_object_id(&self.source) + 1)
            });
            let on_hand = if target < *bound {
                ids.binary_search(&target).is_ok() && !self.source.is_free_entry(target)
            } else {
                target < self.next_object_id
            };
            if on_hand {
                return Some((target, writer_id));
            }
        }
        None
    }

    /// The object ids of the source pages whose content or resources the
    /// page loop of a full rewrite changes while writing: the page loop
    /// reads each of these fields, so the resource prune leaves the pages
    /// they name whole. Keep it in step with `incremental_refusal`.
    fn pages_edited_while_writing(&self, cancel: &crate::cancel::CancelCheck) -> HashSet<u32> {
        let leaves = self.source.all_page_refs_cancellable(cancel).unwrap_or_default();
        let mut indices: HashSet<usize> = HashSet::new();
        indices.extend(self.modified_content.keys());
        indices.extend(self.overlay_additions.keys());
        indices.extend(self.erase_regions.keys());
        indices.extend(self.image_modifications.keys());
        indices.extend(self.modified_annotations.keys());
        indices.extend(&self.flatten_annotations_pages);
        indices.extend(&self.flatten_forms_pages);
        indices.extend(&self.apply_redactions_pages);
        indices.extend(self.redaction_regions.keys());
        indices.extend(self.redacted_pages.keys());
        indices.into_iter().filter_map(|index| leaves.get(index).map(|r| r.id)).collect()
    }

    // one copy of identical images, ICC profiles and font programs
    /// The duplicate-to-kept map of this save's `merge_duplicates` option
    /// (empty with the option off): every object the save writes, from this
    /// document and from each merged source, in the numbering of the output.
    fn duplicate_aliases(
        &self,
        prune: &crate::host::page_prune::PagePrune,
        merged_left_out: &HashSet<u32>,
        options: &SaveOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<HashMap<u32, u32>> {
        use crate::host::duplicates::{find, Origin};

        if !options.merge_duplicates {
            return Ok(HashMap::new());
        }
        // The sweep's own rules decide which source objects are written,
        // except that garbage collection is not asked: a duplicate that
        // nothing reaches is never written, and one that something reaches
        // is written as its kept twin, which the reachability walk follows
        // through the alias map.
        let dropped_only = if options.garbage_collect {
            HashSet::new()
        } else {
            self.dropped_only_ids(prune)
        };
        let source_ids = self.source.all_object_ids();
        let mut objects: Vec<(u32, u32)> = source_ids
            .iter()
            .copied()
            .filter(|&id| {
                id != 0
                    && !self.source.is_free_entry(id)
                    && !self.redacted_orphan_ids.contains(&id)
                    && !prune.excluded.contains(&id)
                    && !dropped_only.contains(&id)
            })
            .map(|id| (id, id))
            .collect();
        objects.extend(
            self.modified_objects
                .keys()
                .copied()
                .filter(|&id| {
                    source_ids.binary_search(&id).is_err()
                        && !prune.excluded.contains(&id)
                        && !dropped_only.contains(&id)
                })
                .map(|id| (id, id)),
        );
        let mut origins =
            vec![Origin { doc: &self.source, objects, staged: Some(&self.modified_objects) }];
        for source in &self.merged_sources {
            let mut objects: Vec<(u32, u32)> = source
                .objects
                .iter()
                .copied()
                .filter(|(out, _)| !merged_left_out.contains(out))
                .collect();
            objects.sort_unstable();
            objects.dedup();
            origins.push(Origin { doc: &source.doc, objects, staged: None });
        }
        find(&origins, cancel)
    }

    // what only a dropped page reaches (#261)
    /// The objects reachable from the prune plan's excluded objects (dropped
    /// pages, inner /Pages nodes, dropped-page annotations) and from nothing
    /// the kept document reaches. A save without garbage collection writes
    /// every other source object, so it skips these by name.
    fn dropped_only_ids(
        &self,
        prune: &crate::host::page_prune::PagePrune,
    ) -> std::collections::HashSet<u32> {
        fn push_refs(obj: &Object, out: &mut Vec<u32>) {
            match obj {
                Object::Reference(r) => out.push(r.id),
                Object::Array(arr) => arr.iter().for_each(|o| push_refs(o, out)),
                Object::Dictionary(d) => d.values().for_each(|o| push_refs(o, out)),
                Object::Stream { dict, .. } => dict.values().for_each(|o| push_refs(o, out)),
                _ => {},
            }
        }

        let mut only = std::collections::HashSet::new();
        if prune.excluded.is_empty() {
            return only;
        }
        let kept = self.collect_reachable_ids(
            prune,
            None,
            &HashMap::new(),
            &crate::cancel::CancelCheck::none(),
        );
        let mut queue: Vec<u32> = prune.excluded.iter().copied().collect();
        let mut refs_buf: Vec<u32> = Vec::with_capacity(32);
        while let Some(id) = queue.pop() {
            if kept.contains(&id) || !only.insert(id) {
                continue;
            }
            refs_buf.clear();
            if let Some(m) = self.modified_objects.get(&id) {
                push_refs(m, &mut refs_buf);
            } else {
                self.source.collect_refs_of(id, &mut refs_buf);
            }
            queue.extend_from_slice(&refs_buf);
        }
        only
    }

    /// Load an object from `self.source` for copying into the output file.
    /// A stream's data is an extent that decrypts itself when it is read, so
    /// it survives into an output with no `/Encrypt` dictionary of its own
    /// (#1032). Every `self.source.load_object` call in
    /// `write_full_to_writer` whose result can reach the output writer goes
    /// through this instead of the raw accessor — the one exception is
    /// `collect_reachable_ids`'s graph walk just above, which only inspects
    /// references and never emits bytes.
    fn load_source_object(&self, r: ObjectRef) -> Result<Object> {
        self.source.load_object(r)
    }

    // the page walk writes staged objects
    /// Like `load_source_object`, but a staged replacement wins: the
    /// resource walk writes XObjects and ExtGStates before the sweep, and
    /// the sweep skips ids already written, so a lookup that ignores
    /// `modified_objects` here would drop every staged image on save.
    /// The prune plan's replacement wins over both, and its cuts apply to
    /// what is found: the walk writes the resource dictionaries the plan
    /// cut down.
    fn staged_or_source_object(
        &self,
        prune: &crate::host::page_prune::PagePrune,
        r: ObjectRef,
    ) -> Result<Object> {
        let base = match prune.overrides.get(&r.id).or_else(|| self.modified_objects.get(&r.id)) {
            Some(m) => m.clone(),
            None => self.load_source_object(r)?,
        };
        Ok(prune.cut(r.id, base))
    }

    /// Like `load_source_object`, but the prune plan's replacement wins, and
    /// its cuts apply to what is found.
    fn pruned_or_source_object(
        &self,
        prune: &crate::host::page_prune::PagePrune,
        r: ObjectRef,
    ) -> Result<Object> {
        let base = match prune.overrides.get(&r.id) {
            Some(pruned) => pruned.clone(),
            None => self.load_source_object(r)?,
        };
        Ok(prune.cut(r.id, base))
    }

    // an unchanged source stream is copied, not loaded
    /// Writes unstaged source object `r` as a chunked copy of its stream
    /// data and returns `true`, or returns `false` having written nothing
    /// when the object cannot be copied raw (not a plain stream, a
    /// container, a /Length that does not hold); the caller then loads and serializes it. The caller checks
    /// that no staged version of `r` exists; the prune plan's `cuts` of a
    /// stream apply to its dictionary and the data is still copied from the
    /// source. The dictionary goes through
    /// `serializer`, so nulling and generation 0 apply. With
    /// `deflate` (a level) a stream with no /Filter or a single FlateDecode
    /// is re-encoded at that level and written so only when strictly
    /// smaller than the source data; otherwise the source bytes are copied.
    /// A source data over `MEMORY_STREAM` is first re-encoded into a byte
    /// counter, and a smaller one is then deflated while streaming, with
    /// /Length a reference to a fresh object holding the compressed size,
    /// written right after.
    fn write_source_stream(
        source: &PdfDocument,
        next_object_id: &mut u32,
        r: ObjectRef,
        cuts: Option<&[crate::host::resource_prune::Cut]>,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<bool> {
        Self::write_stream_copy(
            source,
            next_object_id,
            r,
            r.id,
            None,
            cuts,
            writer,
            serializer,
            handler,
            deflate,
            xref_entries,
        )
    }

    /// The twin of `write_source_stream` for an object of a merged
    /// document: the object is written under `new_id` and its dictionary's
    /// references go through `id_map` (an annotation's /P becomes `page_id`).
    fn write_merged_stream(
        source: &PdfDocument,
        next_object_id: &mut u32,
        (new_id, src_id): (u32, u32),
        remap: (&HashMap<u32, u32>, u32),
        cuts: Option<&[crate::host::resource_prune::Cut]>,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<bool> {
        Self::write_stream_copy(
            source,
            next_object_id,
            ObjectRef::new(src_id, 0),
            new_id,
            Some(remap),
            cuts,
            writer,
            serializer,
            handler,
            deflate,
            xref_entries,
        )
    }

    /// Rewrites the references of `dict` through `id_map`. A reference the
    /// merge never met becomes null rather than naming an unrelated object
    /// of this document; an annotation's /P becomes `page_id`.
    fn remap_dict_refs(dict: &mut HashMap<String, Object>, id_map: &HashMap<u32, u32>, page_id: u32) {
        let annotation = dict.contains_key("Subtype");
        for (key, value) in dict.iter_mut() {
            if annotation && key == "P" && matches!(value, Object::Reference(_)) {
                *value = Object::Reference(ObjectRef::new(page_id, 0));
            } else {
                Self::remap_refs(value, id_map, page_id);
            }
        }
    }

    fn remap_refs(obj: &mut Object, id_map: &HashMap<u32, u32>, page_id: u32) {
        match obj {
            Object::Reference(r) => {
                *obj = match id_map.get(&r.id) {
                    Some(&new_id) => Object::Reference(ObjectRef::new(new_id, 0)),
                    None => Object::Null,
                };
            },
            Object::Array(arr) => arr.iter_mut().for_each(|o| Self::remap_refs(o, id_map, page_id)),
            Object::Dictionary(d) => Self::remap_dict_refs(d, id_map, page_id),
            Object::Stream { dict, .. } => Self::remap_dict_refs(dict, id_map, page_id),
            _ => {},
        }
    }

    fn write_stream_copy(
        source: &PdfDocument,
        next_object_id: &mut u32,
        r: ObjectRef,
        out_id: u32,
        remap: Option<(&HashMap<u32, u32>, u32)>,
        cuts: Option<&[crate::host::resource_prune::Cut]>,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<bool> {
        let Some(head) = source.stream_head(r)? else {
            return Ok(false);
        };
        if matches!(
            head.dict.get("Type").and_then(|t| t.as_name()),
            Some("ObjStm") | Some("XRef")
        ) {
            return Ok(false);
        }
        // The data as the output holds it is the decrypted data: a source
        // that is encrypted is read through its extent's decrypting reader.
        let stored = StreamBody::Extent(head.extent.clone());
        let mut dict = head.dict;
        if let Some(cuts) = cuts {
            crate::host::resource_prune::apply_to_dict(&mut dict, cuts);
        }
        if let Some((id_map, page_id)) = remap {
            Self::remap_dict_refs(&mut dict, id_map, page_id);
        }
        Self::write_stream_body(
            out_id,
            dict,
            &stored,
            deflate,
            next_object_id,
            writer,
            serializer,
            handler,
            xref_entries,
        )?;
        Ok(true)
    }

    /// Writes stream object `out_id`: `dict` as given but for /Filter and
    /// /Length, and the data of `stored` as the output holds it. With a
    /// `handler` the strings of `dict` and the data are enciphered as the
    /// serializer enciphers any stream, /Length the enciphered length.
    /// Without `deflate` the data is copied. With it, a stream
    /// with no /Filter, a single FlateDecode or a single LZWDecode is
    /// re-encoded at that level and written so only when strictly smaller
    /// than the source data. Data over `MEMORY_STREAM` is first re-encoded
    /// into a byte counter, and a smaller one is then deflated while
    /// streaming, /Length a reference to a fresh object holding the compressed
    /// (and enciphered) size, written right after.
    fn write_stream_body(
        out_id: u32,
        mut dict: HashMap<String, Object>,
        stored: &StreamBody,
        deflate: Option<u32>,
        next_object_id: &mut u32,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        use crate::encryption::stream::Cipher;
        use crate::host::deflate::{self, Encoding, MEMORY_STREAM};
        use crate::source::Counter;

        const CHUNK: usize = 64 * 1024;

        /// How the data is written: as the source holds it, as an in-memory
        /// re-encoding, or deflated while streaming (the level).
        enum Body {
            Copy,
            Memory(Vec<u8>),
            Streamed(u32),
        }

        let data_len = stored.decrypted_len()?;

        // A re-encoding is kept only when strictly smaller than the source
        // data; a stream that does not inflate keeps its source bytes.
        let encoding = deflate::encoding(&dict);
        let body = match deflate.filter(|_| encoding != Encoding::Other) {
            None => Body::Copy,
            Some(level) if data_len <= MEMORY_STREAM => {
                let mut raw = Vec::with_capacity(data_len as usize);
                stored
                    .raw_reader()?
                    .take(MEMORY_STREAM)
                    .read_to_end(&mut raw)
                    .map_err(crate::encryption::stream::from_io)?;
                match deflate::smaller(&raw, encoding, level) {
                    Some(encoded) => Body::Memory(encoded),
                    None => Body::Copy,
                }
            },
            Some(level) => {
                let mut counter = Counter::default();
                let counted = deflate::recode(
                    stored.raw_reader()?,
                    encoding,
                    level,
                    &mut counter,
                );
                if counted.is_ok() && counter.0 < data_len {
                    Body::Streamed(level)
                } else {
                    Body::Copy
                }
            },
        };

        if !matches!(body, Body::Copy) {
            match encoding {
                Encoding::Raw => {
                    dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
                },
                Encoding::Lzw { .. } => deflate::lzw_to_flate(&mut dict),
                _ => {},
            }
        }
        let cipher = match handler {
            Some(h) => h.stream_cipher(out_id, 0),
            None => Cipher::Identity,
        };
        let length_id = match &body {
            Body::Copy => {
                let len = cipher.encrypted_len(data_len);
                dict.insert("Length".to_string(), Object::Integer(len as i64));
                None
            },
            Body::Memory(encoded) => {
                let len = cipher.encrypted_len(encoded.len() as u64);
                dict.insert("Length".to_string(), Object::Integer(len as i64));
                None
            },
            Body::Streamed(_) => {
                let id = *next_object_id;
                *next_object_id += 1;
                dict.insert("Length".to_string(), Object::Reference(ObjectRef::new(id, 0)));
                Some(id)
            },
        };

        if let Some(h) = handler {
            fn encipher(object: &mut Object, h: &crate::encryption::EncryptionWriteHandler, id: u32) {
                match object {
                    Object::String(s) => *s = h.encrypt_string(s, id, 0),
                    Object::Array(items) => items.iter_mut().for_each(|o| encipher(o, h, id)),
                    Object::Dictionary(d) => d.values_mut().for_each(|o| encipher(o, h, id)),
                    _ => {},
                }
            }
            dict.values_mut().for_each(|o| encipher(o, h, out_id));
        }

        let offset = writer.position();
        serializer.write_stream_open(&mut *writer, out_id, 0, &dict)?;

        let iv = match handler {
            Some(_) => crate::encryption::EncryptionWriteHandler::generate_iv()?,
            None => [0u8; 16],
        };
        let body_start = writer.position();
        let mut sink = cipher.encrypting(&mut *writer, iv)?;
        match body {
            Body::Copy => {
                let mut data = stored.raw_reader()?;
                let mut buf = vec![0u8; CHUNK];
                loop {
                    let n = data
                        .read(&mut buf)
                        .map_err(crate::encryption::stream::from_io)?;
                    if n == 0 {
                        break;
                    }
                    sink.write_all(&buf[..n])?;
                }
            },
            Body::Memory(encoded) => sink.write_all(&encoded)?,
            Body::Streamed(level) => {
                deflate::recode(stored.raw_reader()?, encoding, level, &mut sink)?;
            },
        }
        sink.finish()?;
        let compressed_len = length_id.map(|_| writer.position() - body_start);
        writer.write_all(b"\nendstream\nendobj\n")?;
        xref_entries.push((out_id, offset, 0, true));

        if let (Some(id), Some(len)) = (length_id, compressed_len) {
            let offset = writer.position();
            writer.write_all(&serializer.serialize_indirect(id, 0, &Object::Integer(len as i64)))?;
            xref_entries.push((id, offset, 0, true));
        }
        Ok(())
    }

    // queued objects become object streams
    /// Writes the objects the packer holds as object streams: every full
    /// batch, and with `rest` the last partial one too. The stream is an
    /// object of its own, encrypted as a stream when the save encrypts.
    fn write_object_streams(
        packer: &crate::host::object_streams::ObjectPacker,
        next_object_id: &mut u32,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
        rest: bool,
    ) -> Result<()> {
        while let Some(batch) = packer.take_batch(rest) {
            let id = *next_object_id;
            *next_object_id += 1;
            let ids: Vec<u32> = batch.iter().map(|(member, _)| *member).collect();
            let stream = crate::host::object_streams::object_stream(batch, deflate);
            let offset = writer.position();
            let bytes = match handler {
                Some(h) => serializer.serialize_indirect_encrypted(id, 0, &stream, h),
                None => serializer.serialize_indirect(id, 0, &stream),
            };
            writer.write_all(&bytes)?;
            xref_entries.push((id, offset, 0, true));
            if !crate::host::object_streams::mark_packed(xref_entries, &ids, id) {
                return Err(Error::InvalidPdf(
                    "an object queued for an object stream has no cross-reference entry".to_string(),
                ));
            }
        }
        Ok(())
    }

    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: a redacted page's content, streamed from its source ──
    /// Writes a redacted page's content as stream `content_id`, its /Length the
    /// object `length_id` written after the body. The body is the redaction pass
    /// run over the source page, deflated at `deflate` and enciphered when the
    /// save encrypts. The pass must report what it reported when it was queued,
    /// or the save fails rather than write a different redaction.
    fn write_redacted_content(
        source: &PdfDocument,
        page_index: usize,
        page: &RedactedPage,
        mods: Option<&HashMap<String, ImageModification>>,
        content_id: u32,
        length_id: u32,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        let ops = source.page_operators(page_index)?;
        let flat = page.flat.as_ref();
        let page_resources = Self::with_flat_xobjects(
            source,
            Self::inherited_resources(source, page_index)?,
            flat,
        )?;
        let images = Self::image_resources_of(source, &page_resources, flat)?;
        let forms = DocForms { source, page_resources, flat };
        let overlay: &[u8] = flat.map_or(&[], |flat| flat.overlay.as_slice());
        let mods = mods.map(|mods| Self::mods_for_copies(mods, &images, &page.scope));
        let mods = mods.as_ref();
        let mut outcome = None;
        Self::write_pass_stream(
            (content_id, length_id),
            HashMap::new(),
            writer,
            serializer,
            handler,
            deflate,
            xref_entries,
            |out| {
                // The image bounds are edited after the redaction decided the
                // page, on the operators it wrote.
                outcome = Some(match mods {
                    Some(mods) => {
                        let mut stage = image_mods::ImageModStage::new(mods, &mut *out);
                        let outcome = Self::run_redaction_pass(
                            &page.regions,
                            &page.opts,
                            &page.fonts,
                            &images,
                            &forms,
                            ops,
                            overlay,
                            Some(&page.scope.tainted_marked),
                            &mut stage,
                        )?;
                        stage.finish()?;
                        outcome
                    },
                    None => Self::run_redaction_pass(
                        &page.regions,
                        &page.opts,
                        &page.fonts,
                        &images,
                        &forms,
                        ops,
                        overlay,
                        Some(&page.scope.tainted_marked),
                        out,
                    )?,
                });
                Ok(())
            },
        )?;
        let outcome = outcome.ok_or_else(|| {
            Error::InvalidPdf(format!("the redaction pass of page {page_index} did not run"))
        })?;
        if outcome.report != page.report || outcome.scope != page.scope {
            return Err(Error::InvalidPdf(format!(
                "the redaction of page {page_index} writes a different result on save than \
                 it recorded when it was queued"
            )));
        }
        Ok(())
    }

    /// `mods` with the modification of each image the redaction copied
    /// also under the copy's name: the belt sees the names the redacted
    /// content draws, and the caller named the source image.
    fn mods_for_copies(
        mods: &HashMap<String, ImageModification>,
        images: &crate::redaction::transform::ImageResources,
        scope: &crate::redaction::transform::Scope,
    ) -> HashMap<String, ImageModification> {
        let mut out = mods.clone();
        let mut names: Vec<&String> = mods.keys().collect();
        names.sort();
        for placement in &scope.placements {
            let named = names
                .iter()
                .find(|name| images.images.get(name.as_str()) == Some(&placement.source));
            if let Some(name) = named {
                out.insert(placement.name.clone(), mods[name.as_str()].clone());
            }
        }
        out
    }

    /// Writes stream `ids.0` with `dict` and its /Length as the object
    /// `ids.1` after the body. `body` writes the stream's bytes; they are
    /// deflated at `deflate` and enciphered when the save encrypts.
    #[allow(clippy::too_many_arguments)]
    fn write_pass_stream(
        ids: (u32, u32),
        mut dict: HashMap<String, Object>,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
        body: impl FnOnce(&mut dyn Write) -> Result<()>,
    ) -> Result<()> {
        let (stream_id, length_id) = ids;
        if deflate.is_some() {
            dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
        }
        dict.insert("Length".to_string(), Object::Reference(ObjectRef::new(length_id, 0)));
        let offset = writer.position();
        serializer.write_stream_open(&mut *writer, stream_id, 0, &dict)?;
        let body_start = writer.position();
        let deflated = |out: &mut dyn Write| -> Result<()> {
            match deflate {
                Some(level) => {
                    // A body writes one operator at a time, a few bytes each; the
                    // deflater takes them in pieces, which halves its work.
                    let zlib = flate2::write::ZlibEncoder::new(
                        out,
                        flate2::Compression::new(crate::host::deflate::zlib_level(level)),
                    );
                    let mut buffered = std::io::BufWriter::with_capacity(64 * 1024, zlib);
                    body(&mut buffered)?;
                    buffered.into_inner().map_err(|e| e.into_error())?.finish()?;
                    Ok(())
                },
                None => body(out),
            }
        };
        match handler {
            None => deflated(&mut *writer)?,
            Some(h) => {
                let iv = crate::encryption::EncryptionWriteHandler::generate_iv()?;
                let mut enc = h.stream_cipher(stream_id, 0).encrypting(&mut *writer, iv)?;
                deflated(&mut enc)?;
                enc.finish()?;
            },
        }
        let len = writer.position() - body_start;
        writer.write_all(b"\nendstream\nendobj\n")?;
        xref_entries.push((stream_id, offset, 0, true));
        let offset = writer.position();
        writer.write_all(&serializer.serialize_indirect(length_id, 0, &Object::Integer(len as i64)))?;
        xref_entries.push((length_id, offset, 0, true));
        Ok(())
    }

    /// One run of the redaction pass over a page's operators into `out`, the
    /// page's image XObjects by name and its forms opened by `forms`.
    fn run_redaction_pass<R: std::io::Read>(
        regions: &crate::redaction::RegionSet,
        opts: &crate::redaction::RedactionOptions,
        fonts: &crate::redaction::FontInfoMetrics,
        images: &crate::redaction::transform::ImageResources,
        forms: &dyn crate::redaction::transform::FormLoader,
        ops: crate::host::content::Operators<R>,
        appended: &[u8],
        marked_plan: Option<&std::collections::BTreeSet<u32>>,
        mut out: &mut dyn Write,
    ) -> Result<crate::redaction::transform::PassOutcome> {
        let mut pass =
            crate::redaction::transform::RedactPass::new(regions, opts, fonts, images).with_forms(forms);
        if let Some(plan) = marked_plan {
            pass = pass.with_marked_plan(plan);
        }
        pass.run_then(
            ops,
            crate::host::content::Operators::new(appended, crate::host::content::Mode::Full),
            &mut out,
        )
    }

    /// `resources` with the XObjects the page's flattened appearances are
    /// written as, under the names the page's overlay draws them by.
    fn with_flat_xobjects(
        source: &PdfDocument,
        resources: Object,
        flat: Option<&FlatPage>,
    ) -> Result<Object> {
        let Some(flat) = flat else { return Ok(resources) };
        let mut dict = resources.as_dict().cloned().unwrap_or_default();
        let mut xobjects = match dict.get("XObject") {
            Some(x) => source.resolve_object(x)?.as_dict().cloned().unwrap_or_default(),
            None => HashMap::new(),
        };
        for set in &flat.sets {
            for (id, name) in &set.xobjects {
                crate::cancel::poll()?;
                xobjects.insert(name.clone(), Object::Reference(ObjectRef::new(*id, 0)));
            }
        }
        dict.insert("XObject".to_string(), Object::Dictionary(xobjects));
        Ok(Object::Dictionary(dict))
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: the image XObjects a page's redaction decides from ──
    /// The /Resources `page` has, own or inherited (ISO 32000-1 §7.7.3.4),
    /// resolved; `Null` when none is found.
    fn inherited_resources(source: &PdfDocument, page: usize) -> Result<Object> {
        let page_ref = source.get_page_ref(page)?;
        let mut node = Some(page_ref);
        let mut guard = 0;
        while let Some(reference) = node {
            guard += 1;
            if guard > 64 {
                break; // a cyclic /Parent chain: bail (fail closed)
            }
            let Some(obj) = source.load_object(reference).ok() else { break };
            let Some(dict) = obj.as_dict() else { break };
            if let Some(res) = dict.get("Resources") {
                return source.resolve_object(res);
            }
            node = dict.get("Parent").and_then(|p| p.as_reference());
        }
        Ok(Object::Null)
    }

    /// The mesh shading (types 4 to 7) at `reference`, with the entries its
    /// decoder reads resolved to direct values; `None` for any other shading.
    fn mesh_source_of(
        source: &PdfDocument,
        reference: ObjectRef,
    ) -> Option<crate::redaction::transform::MeshSource> {
        let object = source.load_object(reference).ok()?;
        let Object::Stream { dict, body } = &object else { return None };
        let shading_type = source.resolve_object(dict.get("ShadingType")?).ok()?.as_integer()?;
        if !(4..=7).contains(&shading_type) {
            return None;
        }
        let mut dict = dict.clone();
        for key in ["BitsPerCoordinate", "BitsPerComponent", "BitsPerFlag", "Decode", "VerticesPerRow"] {
            if let Some(value) = dict.get(key) {
                let value = source.resolve_object(value).ok()?;
                dict.insert(key.to_string(), value);
            }
        }
        Some(crate::redaction::transform::MeshSource {
            reference,
            object: Object::Stream { dict, body: body.clone() },
            shading_type,
        })
    }

    /// The image and form XObjects, the mesh shadings, the shading patterns
    /// whose shading is a mesh and the line styles that `resources` name, by
    /// their resource name. Only /Subtype /Image and /ImageMask true streams
    /// are image entries.
    fn image_resources_of(
        source: &PdfDocument,
        resources: &Object,
        flat: Option<&FlatPage>,
    ) -> Result<crate::redaction::transform::ImageResources> {
        use crate::redaction::transform::{ImageResources, MeshPattern};
        let mut images = std::collections::HashMap::new();
        let mut forms = std::collections::HashMap::new();
        let mut meshes = std::collections::HashMap::new();
        let mut patterns = std::collections::HashMap::new();
        let entries = |key: &str| -> Result<HashMap<String, Object>> {
            let Some(res_dict) = resources.as_dict() else { return Ok(HashMap::new()) };
            match res_dict.get(key) {
                Some(x) => Ok(source.resolve_object(x)?.as_dict().cloned().unwrap_or_default()),
                None => Ok(HashMap::new()),
            }
        };
        for (name, value) in &entries("XObject")? {
            let Some(reference) = value.as_reference() else { continue };
            let Some(obj) = flat
                .and_then(|flat| flat.object(reference.id))
                .or_else(|| source.load_object(reference).ok())
            else {
                continue;
            };
            let Some(dict) = obj.as_dict() else { continue };
            let is_image = dict.get("ImageMask").and_then(|o| o.as_bool()) == Some(true)
                || dict.get("Subtype").and_then(|o| o.as_name()) == Some("Image");
            if is_image {
                images.insert(name.clone(), reference);
            } else if dict.get("Subtype").and_then(|o| o.as_name()) == Some("Form") {
                forms.insert(name.clone(), reference);
            }
        }
        for (name, value) in &entries("Shading")? {
            let Some(reference) = value.as_reference() else { continue };
            if let Some(mesh) = Self::mesh_source_of(source, reference) {
                meshes.insert(name.clone(), mesh);
            }
        }
        for (name, value) in &entries("Pattern")? {
            let Some(reference) = value.as_reference() else { continue };
            let Ok(obj) = source.load_object(reference) else { continue };
            let Some(dict) = obj.as_dict() else { continue };
            if dict.get("PatternType").and_then(|o| o.as_integer()) != Some(2) {
                continue;
            }
            let Some(shading) = dict.get("Shading").and_then(|o| o.as_reference()) else { continue };
            let Some(mesh) = Self::mesh_source_of(source, shading) else { continue };
            let matrix = match dict.get("Matrix") {
                None => crate::content::graphics_state::Matrix::identity(),
                Some(m) => {
                    let m = source.resolve_object(m)?;
                    let numbers: Option<Vec<f32>> = m.as_array().and_then(|items| {
                        items
                            .iter()
                            .map(|n| {
                                let n = source.resolve_object(n).ok()?;
                                n.as_real().or_else(|| n.as_integer().map(|i| i as f64))
                            })
                            .map(|n| n.filter(|n| n.is_finite()).map(|n| n as f32))
                            .collect()
                    });
                    match numbers.as_deref() {
                        Some(&[a, b, c, d, e, f]) => crate::content::graphics_state::Matrix { a, b, c, d, e, f },
                        _ => {
                            return Err(Error::Unsupported(format!(
                                "the pattern {} {} has a /Matrix that cannot be read; refusing to \
                                 redact a page whose mesh cannot be placed",
                                reference.id, reference.gen
                            )))
                        },
                    }
                },
            };
            patterns.insert(name.clone(), MeshPattern { reference, matrix, mesh });
        }
        let mut line_styles = std::collections::HashMap::new();
        for (name, value) in &entries("ExtGState")? {
            line_styles.insert(name.clone(), Self::line_style_of(source, value));
        }
        let mut mcids = std::collections::HashMap::new();
        let mut properties = std::collections::HashMap::new();
        for (name, value) in &entries("Properties")? {
            let Ok(list) = source.resolve_object(value) else { continue };
            let mcid = list
                .as_dict()
                .and_then(|d| d.get("MCID"))
                .and_then(|m| m.as_integer())
                .and_then(|m| u32::try_from(m).ok());
            if let Some(mcid) = mcid {
                mcids.insert(name.clone(), mcid);
            }
            if let Some(dict) = list.as_dict() {
                properties.insert(
                    name.clone(),
                    crate::redaction::transform::PropertyList {
                        reference: value.as_reference(),
                        dict: dict.clone(),
                    },
                );
            }
        }
        Ok(ImageResources {
            images,
            forms,
            meshes,
            patterns,
            line_styles,
            mcids,
            properties,
        })
    }

    /// The metrics of the fonts `resources` name, by resource name. A font
    /// that cannot be read is left out, so a show in it refuses.
    fn font_metrics_of(
        source: &PdfDocument,
        resources: &Object,
        flat: Option<&FlatPage>,
    ) -> Result<crate::redaction::FontInfoMetrics> {
        let mut map: HashMap<String, std::sync::Arc<crate::fonts::FontInfo>> = HashMap::new();
        if let Some(res_dict) = resources.as_dict() {
            if let Some(font_obj) = res_dict.get("Font") {
                let font_dict_obj = match font_obj.as_reference() {
                    Some(r) => source.load_object(r)?,
                    None => font_obj.clone(),
                };
                if let Some(font_dict) = font_dict_obj.as_dict() {
                    for (name, val) in font_dict {
                        let resolved = match val.as_reference() {
                            Some(r) => match flat.and_then(|flat| flat.extra(r.id)) {
                                Some(extra) => flat.map_or_else(
                                    || extra.clone(),
                                    |flat| flat.inline_extras(extra, 8),
                                ),
                                None => match source.load_object(r) {
                                    Ok(o) => o,
                                    Err(_) => continue,
                                },
                            },
                            None => val.clone(),
                        };
                        if let Ok(fi) = crate::fonts::FontInfo::from_dict(&resolved, source) {
                            map.insert(name.clone(), std::sync::Arc::new(fi));
                        }
                    }
                }
            }
        }
        Ok(crate::redaction::FontInfoMetrics::new(map))
    }

    /// The `/LW`, `/LJ`, `/ML` and `/LC` entries of an ExtGState; `None` when
    /// the dictionary does not resolve or an entry is not a number in range.
    fn line_style_of(
        source: &PdfDocument,
        value: &Object,
    ) -> Option<crate::redaction::transform::LineStyle> {
        let state = source.resolve_object(value).ok()?;
        let dict = state.as_dict()?;
        let entry = |key: &str| -> Option<Option<f64>> {
            match dict.get(key) {
                None => Some(None),
                Some(v) => {
                    let v = source.resolve_object(v).ok()?;
                    let n = v.as_real().or_else(|| v.as_integer().map(|i| i as f64))?;
                    Some(Some(n))
                },
            }
        };
        let cap = |key: &str| -> Option<Option<u8>> {
            match entry(key)? {
                None => Some(None),
                Some(n) if (0.0..=2.0).contains(&n) && n.fract() == 0.0 => Some(Some(n as u8)),
                Some(_) => None,
            }
        };
        Some(crate::redaction::transform::LineStyle {
            lw: entry("LW")?.map(|n| n as f32),
            lj: cap("LJ")?,
            ml: entry("ML")?.map(|n| n as f32),
            lc: cap("LC")?,
        })
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: one queued copy, streamed through the save's chain ──
    /// Writes one redacted copy of `source_ref` as stream `out_id`, its
    /// /Length the object `length_id`. Thin wrapper over
    /// [`crate::host::images::redact_copy::write_copy`]: the placement comes
    /// from the pass, the save's deflate level and encryption apply.
    #[cfg(feature = "rendering")]
    #[allow(clippy::too_many_arguments)]
    fn write_redacted_image_copy(
        source: &PdfDocument,
        source_ref: ObjectRef,
        (out_id, length_id): (u32, u32),
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        fills: &[([f32; 4], [f32; 3])],
        semantics: &redact_copy::CopySemantics,
        color_spaces: &HashMap<String, Object>,
        dict_edits: &HashMap<String, Object>,
        cancel: &crate::cancel::CancelCheck,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        redact_copy::write_copy(
            source,
            source_ref,
            out_id,
            length_id,
            u0,
            v0,
            u1,
            v1,
            fills,
            semantics,
            color_spaces,
            dict_edits,
            cancel,
            writer,
            serializer,
            handler,
            xref_entries,
        )
    }
    // ── end pdf_manipulator patch ──

    /// The /ColorSpace dictionary `resources` name, for the row geometry a
    /// copy's source is read through.
    fn colour_space_map_of(
        source: &PdfDocument,
        resources: &Object,
    ) -> Result<HashMap<String, Object>> {
        let Ok(Some(colour_dict)) = resources
            .as_dict()
            .and_then(|d| d.get("ColorSpace"))
            .map(|cs| source.resolve_object(cs))
            .transpose()
            .map(|o| o.map(|o| o.as_dict().cloned()))
        else {
            return Ok(HashMap::new());
        };
        Ok(colour_dict.unwrap_or_default())
    }

    // ── pdf_manipulator patch: the redacted page's own /Resources ──
    /// The /Resources a redacted page carries: a copy of the resolved,
    /// inherited dictionary where each copy name a placement queued points
    /// at its copy and a source name stays only when an unredacted `Do` on
    /// this page still names it. The source resources another page shares
    /// are never edited — the redacted page's own dictionary is the edit.
    fn redacted_page_resources(
        &self,
        page_index: usize,
        page: &RedactedPage,
        copies: &ScopeIds,
    ) -> Result<Object> {
        let resources = Self::inherited_resources(&self.source, page_index)?;
        let rewritten = Self::rewritten_resources(&self.source, &resources, &page.scope, copies)?;
        // The flattened appearances a surviving `Do` still draws.
        let Some(flat) = &page.flat else { return Ok(rewritten) };
        let mut dict = rewritten.as_dict().cloned().unwrap_or_default();
        let mut xobjects = dict.get("XObject").and_then(Object::as_dict).cloned().unwrap_or_default();
        for set in &flat.sets {
            for (id, name) in &set.xobjects {
                crate::cancel::poll()?;
                if page.scope.kept_xobject_names.contains(name) {
                    xobjects.insert(name.clone(), Object::Reference(ObjectRef::new(*id, 0)));
                }
            }
        }
        dict.insert("XObject".to_string(), Object::Dictionary(xobjects));
        Ok(Object::Dictionary(dict))
    }

    /// `resources` as a scope's content stream is written against: the
    /// copies the pass queued under the names it wrote, a source XObject only
    /// while a surviving `Do` names it, and a source shading or pattern
    /// unless a copy replaced it and no surviving mark names it. Copies take
    /// the ids in `ids` as far as they go.
    fn rewritten_resources(
        source: &PdfDocument,
        resources: &Object,
        scope: &crate::redaction::transform::Scope,
        ids: &ScopeIds,
    ) -> Result<Object> {
        let mut resources_dict = resources.as_dict().cloned().unwrap_or_default();
        let entries = |key: &str| -> Result<HashMap<String, Object>> {
            match resources_dict.get(key) {
                Some(x) => Ok(source.resolve_object(x)?.as_dict().cloned().unwrap_or_default()),
                None => Ok(HashMap::new()),
            }
        };
        let mut xobject_dict = entries("XObject")?;
        // A source name stays only when a surviving `Do` names it.
        xobject_dict.retain(|name, _| scope.kept_xobject_names.contains(name));
        // The copies, under the names the pass wrote.
        for (placement, ids) in scope.placements.iter().zip(&ids.images) {
            crate::cancel::poll()?;
            xobject_dict.insert(
                placement.name.clone(),
                Object::Reference(ObjectRef::new(ids.image.0, 0)),
            );
        }
        for (form, ids) in scope.forms.iter().zip(&ids.forms) {
            xobject_dict.insert(
                form.name.clone(),
                Object::Reference(ObjectRef::new(ids.stream.0, 0)),
            );
        }
        let mut shading_dict = entries("Shading")?;
        shading_dict.retain(|name, _| {
            scope.kept_shadings.contains(name) || !scope.replaced_shadings.contains(name)
        });
        for (mesh, (id, _)) in scope.meshes.iter().zip(&ids.meshes) {
            shading_dict.insert(mesh.name.clone(), Object::Reference(ObjectRef::new(*id, 0)));
        }
        let mut pattern_dict = entries("Pattern")?;
        pattern_dict.retain(|name, _| {
            scope.kept_patterns.contains(name) || !scope.replaced_patterns.contains(name)
        });
        for (pattern, id) in scope.patterns.iter().zip(&ids.patterns) {
            pattern_dict.insert(pattern.name.clone(), Object::Reference(ObjectRef::new(*id, 0)));
        }
        let mut property_dict = entries("Properties")?;
        property_dict.retain(|name, _| {
            scope.kept_properties.contains(name) || !scope.replaced_properties.contains(name)
        });
        for copy in &scope.properties {
            property_dict.insert(copy.name.clone(), Object::Dictionary(copy.dict.clone()));
        }
        if !ids.fonts.is_empty() {
            let mut font_dict = entries("Font")?;
            for (name, entry) in &ids.fonts {
                crate::cancel::poll()?;
                font_dict.insert(name.clone(), entry.clone());
            }
            resources_dict.insert("Font".to_string(), Object::Dictionary(font_dict));
        }
        resources_dict.insert("XObject".to_string(), Object::Dictionary(xobject_dict));
        for (key, dict) in [
            ("Shading", shading_dict),
            ("Pattern", pattern_dict),
            ("Properties", property_dict),
        ] {
            if !dict.is_empty() || resources_dict.contains_key(key) {
                resources_dict.insert(key.to_string(), Object::Dictionary(dict));
            }
        }
        Ok(Object::Dictionary(resources_dict))
    }

    /// Proves every image copy of `scope` and of the forms below it
    /// writable, and collects the /SMask and /Mask objects those copies
    /// replace.
    #[cfg(feature = "rendering")]
    fn validate_scope(
        &self,
        scope: &crate::redaction::transform::Scope,
        forms: &DocForms,
        resources: &Object,
        mask_originals: &mut Vec<u32>,
    ) -> Result<()> {
        let colour_spaces = Self::colour_space_map_of(&self.source, resources)?;
        for placement in &scope.placements {
            redact_copy::validate_copy_source(&self.source, placement.source, &colour_spaces)?;
            let dict = self.source.load_object(placement.source)?;
            let dict = dict.as_dict().cloned().unwrap_or_default();
            for key in ["SMask", "Mask"] {
                if let Some(mask) = dict.get(key).and_then(|o| o.as_reference()) {
                    redact_copy::validate_copy_source(&self.source, mask, &colour_spaces)?;
                    mask_originals.push(mask.id);
                }
            }
        }
        for form in &scope.forms {
            let dict = self.source.load_object(form.source)?.as_dict().cloned().unwrap_or_default();
            let inner = forms.resources_of(&dict)?;
            self.validate_scope(&form.scope, forms, &inner, mask_originals)?;
        }
        Ok(())
    }

    /// The objects the copies of `page` name sources from, for the
    /// reachability walk: the fonts and colour spaces of a form copy's
    /// resources, the function of a mesh copy, the rest of a pattern copy.
    fn copy_sources(&self, page_index: usize, page: &RedactedPage) -> Result<Vec<Object>> {
        let page_resources = Self::inherited_resources(&self.source, page_index)?;
        let forms = DocForms { source: &self.source, page_resources, flat: page.flat.as_ref() };
        let mut out = Vec::new();
        Self::push_copy_sources(&self.source, &forms, &page.scope, &mut out)?;
        Ok(out)
    }

    fn push_copy_sources(
        source: &PdfDocument,
        forms: &DocForms,
        scope: &crate::redaction::transform::Scope,
        out: &mut Vec<Object>,
    ) -> Result<()> {
        #[cfg(feature = "rendering")]
        for mesh in &scope.meshes {
            if let Some(mesh) = Self::mesh_source_of(source, mesh.source) {
                out.push(Object::Dictionary(crate::redaction::transform::filtered_dict(&mesh)?));
            }
        }
        for pattern in &scope.patterns {
            let mut dict = source.load_object(pattern.source)?.as_dict().cloned().unwrap_or_default();
            dict.remove("Shading");
            out.push(Object::Dictionary(dict));
        }
        for form in &scope.forms {
            let mut dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
            let resources = forms.resources_of(&dict)?;
            dict.remove("Resources");
            out.push(Object::Dictionary(dict));
            out.push(Self::rewritten_resources(source, &resources, &form.scope, &ScopeIds::default())?);
            Self::push_copy_sources(source, forms, &form.scope, out)?;
        }
        Ok(())
    }

    /// Allocates the ids of every copy `scope` queued, and of the copies of
    /// the forms below it.
    fn allocate_scope_ids(
        source: &PdfDocument,
        next_object_id: &mut u32,
        scope: &crate::redaction::transform::Scope,
    ) -> Result<ScopeIds> {
        fn next(counter: &mut u32) -> u32 {
            let id = *counter;
            *counter += 1;
            id
        }
        let mut ids = ScopeIds::default();
        for placement in &scope.placements {
            let source_dict =
                source.load_object(placement.source)?.as_dict().cloned().unwrap_or_default();
            let has_smask = source_dict.get("SMask").and_then(|o| o.as_reference()).is_some();
            let has_mask = source_dict.get("Mask").and_then(|o| o.as_reference()).is_some();
            let image = (next(next_object_id), next(next_object_id));
            let smask = has_smask.then(|| (next(next_object_id), next(next_object_id)));
            let mask = has_mask.then(|| (next(next_object_id), next(next_object_id)));
            ids.images.push(RedactionCopyIds { image, smask, mask });
        }
        for _ in &scope.meshes {
            ids.meshes.push((next(next_object_id), next(next_object_id)));
        }
        for _ in &scope.patterns {
            ids.patterns.push(next(next_object_id));
        }
        for form in &scope.forms {
            let stream = (next(next_object_id), next(next_object_id));
            let inner = Self::allocate_scope_ids(source, next_object_id, &form.scope)?;
            ids.forms.push(FormIds { stream, inner });
        }
        Ok(ids)
    }

    /// Flattens `scope` and the scopes of its forms into `work`, each copy
    /// with the resources it is drawn from.
    #[cfg(feature = "rendering")]
    fn collect_copy_work<'a>(
        source: &PdfDocument,
        forms: &DocForms,
        scope: &'a crate::redaction::transform::Scope,
        ids: &'a ScopeIds,
        resources: &Object,
        work: &mut CopyWork<'a>,
    ) -> Result<()> {
        let colour_spaces = Self::colour_space_map_of(source, resources)?;
        for (placement, copy) in scope.placements.iter().zip(&ids.images) {
            work.images.push((placement, copy, colour_spaces.clone()));
        }
        for (mesh, id) in scope.meshes.iter().zip(&ids.meshes) {
            work.meshes.push((mesh, *id));
        }
        for (pattern, id) in scope.patterns.iter().zip(&ids.patterns) {
            let mesh_id = ids.meshes.get(pattern.mesh).map(|m| m.0).ok_or_else(|| {
                Error::InvalidPdf("a pattern copy names a mesh copy that was not queued".to_string())
            })?;
            work.patterns.push((pattern, *id, mesh_id));
        }
        for (form, form_ids) in scope.forms.iter().zip(&ids.forms) {
            let dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
            let inner = forms.resources_of(&dict)?;
            Self::collect_copy_work(source, forms, &form.scope, &form_ids.inner, &inner, work)?;
            work.forms.push((form, form_ids));
        }
        Ok(())
    }

    /// As the build with rendering, for the form copies alone: a build
    /// without rendering queues no image, mesh or pattern copy.
    #[cfg(not(feature = "rendering"))]
    fn collect_copy_work<'a>(
        source: &PdfDocument,
        forms: &DocForms,
        scope: &'a crate::redaction::transform::Scope,
        ids: &'a ScopeIds,
        resources: &Object,
        work: &mut CopyWork<'a>,
    ) -> Result<()> {
        for (form, form_ids) in scope.forms.iter().zip(&ids.forms) {
            Self::collect_copy_work(source, forms, &form.scope, &form_ids.inner, resources, work)?;
            work.forms.push((form, form_ids));
        }
        Ok(())
    }

    /// Writes the mesh copy as stream `ids.0`: the source's primitives that
    /// meet no region, decided again as the dry run decided them.
    #[cfg(feature = "rendering")]
    #[allow(clippy::too_many_arguments)]
    fn write_mesh_copy(
        source: &PdfDocument,
        mesh: &crate::redaction::transform::MeshCopy,
        ids: (u32, u32),
        regions: &crate::redaction::RegionSet,
        padding: f32,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        let unreadable = || {
            Error::InvalidPdf(format!(
                "the mesh shading {} {} cannot be read when the redaction is saved",
                mesh.source.id, mesh.source.gen
            ))
        };
        let source_mesh = Self::mesh_source_of(source, mesh.source).ok_or_else(unreadable)?;
        let dict = crate::redaction::transform::filtered_dict(&source_mesh)?;
        let mut found = None;
        Self::write_pass_stream(ids, dict, writer, serializer, handler, deflate, xref_entries, |out| {
            let mut out = out;
            found = Some(crate::redaction::transform::write_filtered(
                &source_mesh,
                &mesh.to_page,
                regions,
                padding,
                &mut out,
            )?);
            Ok(())
        })?;
        match found {
            Some(found) if found.kept == mesh.kept && found.dropped == mesh.dropped => Ok(()),
            _ => Err(Error::InvalidPdf(format!(
                "the mesh shading {} {} keeps different primitives on save than it recorded \
                 when the redaction was queued",
                mesh.source.id, mesh.source.gen
            ))),
        }
    }

    /// Writes the pattern copy as object `id`: the source's dictionary, its
    /// shading the mesh copy `mesh_id`.
    #[cfg(feature = "rendering")]
    fn write_pattern_copy(
        source: &PdfDocument,
        pattern: &crate::redaction::transform::PatternCopy,
        (id, mesh_id): (u32, u32),
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        let mut dict = source.load_object(pattern.source)?.as_dict().cloned().unwrap_or_default();
        dict.insert("Shading".to_string(), Object::Reference(ObjectRef::new(mesh_id, 0)));
        let object = Object::Dictionary(dict);
        let offset = writer.position();
        match handler {
            Some(h) => serializer.write_indirect_encrypted(&mut *writer, id, 0, &object, h),
            None => serializer.write_indirect(&mut *writer, id, 0, &object),
        }
        .map_err(crate::encryption::stream::from_io)?;
        xref_entries.push((id, offset, 0, true));
        Ok(())
    }

    /// Writes the form copy as stream `ids.stream`: the source's dictionary
    /// with the rewritten resources, its content the pass run again under the
    /// matrix the dry run recorded.
    #[allow(clippy::too_many_arguments)]
    fn write_form_copy(
        source: &PdfDocument,
        forms: &DocForms,
        form: &crate::redaction::transform::FormCopy,
        ids: &FormIds,
        regions: &crate::redaction::RegionSet,
        opts: &crate::redaction::RedactionOptions,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        serializer: &ObjectSerializer,
        handler: &Option<crate::encryption::EncryptionWriteHandler>,
        deflate: Option<u32>,
        xref_entries: &mut XrefEntries,
    ) -> Result<()> {
        use crate::redaction::transform::FormLoader;
        let mut dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
        for key in ["Length", "Filter", "DecodeParms", "DL", "F", "FFilter", "FDecodeParms"] {
            dict.remove(key);
        }
        let resources = forms.resources_of(&dict)?;
        dict.insert(
            "Resources".to_string(),
            Self::rewritten_resources(source, &resources, &form.scope, &ids.inner)?,
        );
        let view = forms.view(form.source)?;
        let content = forms.content(form.source)?;
        let mut outcome = None;
        Self::write_pass_stream(ids.stream, dict, writer, serializer, handler, deflate, xref_entries, |out| {
            let mut out = out;
            outcome = Some(crate::redaction::transform::redact_form(
                regions, opts, &view, forms, form, content, &mut out,
            )?);
            Ok(())
        })?;
        match outcome {
            Some(outcome) if outcome.scope == form.scope => Ok(()),
            _ => Err(Error::InvalidPdf(format!(
                "the form {} {} is redacted differently on save than when the redaction was queued",
                form.source.id, form.source.gen
            ))),
        }
    }
    // ── end pdf_manipulator patch ──

    /// Write a full rewrite of the PDF to a generic writer.
    // ── pdf_manipulator patch: pub(crate) + PositionedWrite output ──
    // Visibility (pub(crate) for dispatch) AND the output type: the
    // writer is any PositionedWrite, and every byte offset below comes
    // from writer.position() — all such calls in this function are
    // part of this patch. `cancel` is polled once per page.
    // ── end pdf_manipulator patch ──
    pub(crate) fn write_full_to_writer(
        &mut self,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        options: &SaveOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<()> {
        // ── pdf_manipulator patch: the ids a save allocates are the save's own ──
        // A second save of the same editor must write the bytes the first
        // did, so the editor's next id is what it was before the save, whether
        // the save succeeded or failed.
        let next_object_id = self.next_object_id;
        let result = self.write_full_to_writer_allocating(writer, options, cancel);
        self.next_object_id = next_object_id;
        result
    }

    fn write_full_to_writer_allocating(
        &mut self,
        writer: &mut impl crate::host::positioned_write::PositionedWrite,
        options: &SaveOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<()> {
        // ── end pdf_manipulator patch ──
        use crate::encryption::{
            generate_file_id, Algorithm, EncryptDictBuilder, EncryptionWriteHandler,
        };

        // ── pdf_manipulator patch: the Flate level of this save ──
        if options.deflate_level > crate::host::deflate::MAX_LEVEL {
            return Err(Error::InvalidOperation(format!(
                "deflate_level {} is above {}",
                options.deflate_level,
                crate::host::deflate::MAX_LEVEL
            )));
        }
        let deflate = options.compress.then_some(options.deflate_level);
        // ── end pdf_manipulator patch ──

        // Fail closed rather than silently copying unreadable ciphertext
        // (#1032): every stream object read from `self.source` below goes
        // through `load_source_object` to decrypt it, but that requires an
        // authenticated encryption handler. An encrypted-but-unauthenticated
        // source has no key to decrypt with at all — refuse rather than
        // write out raw source ciphertext with no /Encrypt dict, which
        // silently produces a structurally valid but unreadable PDF.
        if self.source.is_encrypted() && !self.source.is_authenticated() {
            return Err(Error::EncryptedPdf);
        }

        // ── pdf_manipulator patch: a redacted page that is also flattened is decided with its appearances ──
        self.prepare_flat_redactions()?;
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: what the save drops with dropped pages (#261) ──
        let dropped_annots = self.redacted_annotation_ids();
        let mut prune = crate::host::page_prune::plan_cancellable(
            &self.source,
            &self.modified_objects,
            &self.page_order,
            &dropped_annots,
            // the image sources destructive redaction replaced
            &self.redacted_image_originals,
            cancel,
        )?;
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: the font and structure scrub of the redacted pages ──
        let scrub = self.plan_scrub(&mut prune)?;
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: the merged pages and objects the save drops ──
        let (merged_kept, mut merged_left_out) = self.merged_prune(&HashMap::new())?;
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: unused resources dropped, and the glyphs the font cut needs collected by the same walk ──
        let mut glyphs = options.prune_resources.then(|| {
            crate::host::font_subset::Collector::new(
                &self.source,
                &self.modified_objects,
                &prune.overrides,
                cancel,
            )
        });
        let merged_cuts = if options.prune_resources {
            use crate::host::resource_prune::{plan, Scope, ScopePage};

            let held = self.pages_edited_while_writing(cancel);
            plan(
                Scope {
                    doc: &self.source,
                    staged: &self.modified_objects,
                    overrides: &prune.overrides,
                    cuts: &mut prune.cuts,
                    excluded: &prune.excluded,
                    pages: prune
                        .kept_pages
                        .iter()
                        .map(|&id| ScopePage { id, held: held.contains(&id) })
                        .collect(),
                    page_object: None,
                    writers: None,
                    glyphs: glyphs.as_mut(),
                },
                cancel,
            )?;
            let cuts = self.merged_resource_prune(&merged_kept, &merged_left_out, cancel)?;
            if !cuts.is_empty() {
                merged_left_out = self.merged_prune(&cuts)?.1;
            }
            cuts
        } else {
            HashMap::new()
        };
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: one copy of identical images, ICC profiles and font programs ──
        let aliases = std::sync::Arc::new(self.duplicate_aliases(
            &prune,
            &merged_left_out,
            options,
            cancel,
        )?);
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: embedded font programs cut down to the glyphs shown, ids kept ──
        if let Some(glyphs) = glyphs {
            let fonts = glyphs.finish(&aliases, options.deflate_level)?;
            prune.overrides.extend(fonts);
        }
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: a loaded stream is deflated or re-deflated, kept only if smaller ──
        /// Deflate a stream object with no filter, or re-deflate one with a
        /// single FlateDecode (its /DecodeParms stay valid: only the zlib
        /// layer is redone), at `level`. The stream keeps its data when the
        /// result is not strictly smaller.
        fn deflate_stream(obj: Object, level: u32) -> Object {
            use crate::host::deflate::{encoding, smaller, Encoding, MEMORY_STREAM};
            match obj {
                Object::Stream { mut dict, body } => {
                    let kind = encoding(&dict);
                    // A stream over MEMORY_STREAM is written by `write_stream_body`;
                    // here it keeps its data. A read that fails keeps the stream;
                    // the write reports it.
                    if !matches!(body.decrypted_len(), Ok(len) if len <= MEMORY_STREAM) {
                        return Object::Stream { dict, body };
                    }
                    let mut raw = Vec::with_capacity(body.len() as usize);
                    let read = body.raw_reader().and_then(|reader| {
                        reader
                            .take(MEMORY_STREAM)
                            .read_to_end(&mut raw)
                            .map_err(crate::encryption::stream::from_io)
                    });
                    if read.is_err() {
                        return Object::Stream { dict, body };
                    }
                    let Some(compressed) = smaller(&raw, kind, level) else {
                        return Object::Stream { dict, body };
                    };
                    match kind {
                        Encoding::Raw => {
                            dict.insert(
                                "Filter".to_string(),
                                Object::Name("FlateDecode".to_string()),
                            );
                        },
                        Encoding::Lzw { .. } => crate::host::deflate::lzw_to_flate(&mut dict),
                        _ => {},
                    }
                    dict.insert(
                        "Length".to_string(),
                        Object::Integer(compressed.len() as i64),
                    );
                    Object::stream(dict, compressed)
                },
                other => other,
            }
        }
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: keep the source's own encryption ──
        // Read before the first byte is written, so an unsupported source
        // fails with nothing in the output.
        if options.keep_source_encryption && options.encryption.is_some() {
            return Err(Error::InvalidOperation(
                "keep_source_encryption and an explicit encryption config are exclusive"
                    .to_string(),
            ));
        }
        let kept = if options.keep_source_encryption {
            self.source.kept_encryption()?
        } else {
            None
        };
        // ── end pdf_manipulator patch ──

        // Write PDF header
        let (major, minor) = self.version();
        // ── pdf_manipulator patch: object streams and a cross-reference stream are PDF 1.5 ──
        let (major, minor) = if options.object_streams && (major, minor) < (1, 5) {
            (1, 5)
        } else {
            (major, minor)
        };
        // ── end pdf_manipulator patch ──
        write!(writer, "%PDF-{}.{}\n", major, minor)?;
        // Binary marker per spec (bytes > 127 to indicate binary content)
        writer.write_all(b"%\x80\x81\x82\x83\n")?;

        // ── pdf_manipulator patch: references to what the save leaves out are null (#261) ──
        let serializer = ObjectSerializer::compact()
            .nulling(prune.excluded.clone())
            .zero_generations()
            .recording_references()
            .aliasing(aliases.clone());
        // ── end pdf_manipulator patch ──

        // Set up encryption if configured
        let (file_id, encrypt_dict, encryption_handler) =
            if let Some(config) = options.encryption.as_ref() {
                let (id1, id2) = generate_file_id();

                // Convert EncryptionAlgorithm to encryption::Algorithm
                let algorithm = match config.algorithm {
                    EncryptionAlgorithm::Rc4_40 => Algorithm::RC4_40,
                    EncryptionAlgorithm::Rc4_128 => Algorithm::Rc4_128,
                    EncryptionAlgorithm::Aes128 => Algorithm::Aes128,
                    EncryptionAlgorithm::Aes256 => Algorithm::Aes256,
                };

                // Build the encryption dictionary, keeping the file
                // encryption key it wrapped into /UE and /OE.
                let (encrypt_dict, file_key) = EncryptDictBuilder::new(algorithm)
                    .user_password(config.user_password.as_bytes())
                    .owner_password(config.owner_password.as_bytes())
                    .permissions(config.permissions.to_bits())
                    .encrypt_metadata(true)
                    .build_with_key(&id1)?;

                // Create encryption handler.
                //
                // For AES-256 (R6) the key is the one just wrapped into /UE —
                // ISO 32000-2 Algorithm 8 generates exactly one, and there is
                // nothing to re-derive. Deriving separately here produced a
                // second random key, so the file authenticated and then
                // decrypted every stream to noise. For R<=4 the key *is* a
                // derivation from the password, owner hash, permissions and
                // file id, and the handler recomputes it.
                let handler = match file_key {
                    Some(key) => EncryptionWriteHandler::with_file_key(key, algorithm, true),
                    None => EncryptionWriteHandler::new(
                        config.user_password.as_bytes(),
                        &encrypt_dict.owner_password,
                        encrypt_dict.permissions,
                        &id1,
                        algorithm,
                        true,
                    )?,
                };

                (Some((id1, id2)), Some(encrypt_dict.to_object()), Some(handler))
            // ── pdf_manipulator patch: keep the source's own encryption ──
            } else if let Some(kept) = kept {
                let handler = EncryptionWriteHandler::with_file_key(
                    kept.file_key,
                    kept.algorithm,
                    kept.encrypt_metadata,
                );
                (Some(kept.file_id), Some(kept.encrypt_dict), Some(handler))
            // ── end pdf_manipulator patch ──
            } else {
                (None, None, None)
            };

        // ── pdf_manipulator patch: non-stream objects wait in the packer for an object stream ──
        // Returns no bytes for such an object: the caller's `write_all` of
        // them writes nothing and its xref push records an offset the xref
        // stream replaces with the object's place in its stream. Strings in
        // a packed object are not encrypted: the object stream is.
        let packer = crate::host::object_streams::ObjectPacker::new(options.object_streams);
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: an object is written to the output, its stream data in pieces; non-stream objects wait in the packer ──
        // Helper to write with or without encryption
        let write_obj = |out: &mut dyn Write,
                         s: &ObjectSerializer,
                         id: u32,
                         gen: u16,
                         obj: &Object,
                         handler: &Option<EncryptionWriteHandler>|
         -> Result<()> {
            if gen == 0 && packer.takes(obj) {
                packer.add(id, s.serialize_member(id, obj));
                return Ok(());
            }
            match handler {
                Some(h) => s.write_indirect_encrypted(out, id, gen, obj, h),
                None => s.write_indirect(out, id, gen, obj),
            }
            .map_err(crate::encryption::stream::from_io)
        };
        // ── end pdf_manipulator patch ──

        let mut xref_entries = XrefEntries::new();
        // ── pdf_manipulator patch: a merged duplicate is never written ──
        for &duplicate in aliases.keys() {
            xref_entries.claim_id(duplicate);
        }
        // ── end pdf_manipulator patch ──

        // Object 0 is always free
        xref_entries.push((0, 65535, 65535, false));

        // Get catalog and traverse to collect all referenced objects
        let catalog = self.source.catalog()?;
        let catalog_ref = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Root"))
            .and_then(|r| r.as_reference())
            .ok_or_else(|| Error::InvalidPdf("Missing catalog reference".to_string()))?;

        // For now, do a simple copy of essential objects
        // Full implementation would do complete object traversal

        // Write encryption dictionary if encrypting (must not be encrypted itself)
        let encrypt_obj_id = if let Some(ref enc_obj) = encrypt_dict {
            let enc_id = self.allocate_object_id();
            let offset = writer.position();
            // Encryption dict is NOT encrypted
            let bytes = serializer.serialize_indirect(enc_id, 0, &enc_obj);
            writer.write_all(&bytes)?;
            xref_entries.push((enc_id, offset, 0, true));
            Some(enc_id)
        } else {
            None
        };

        // ── pdf_manipulator patch: the objects the font scrub writes under new ids ──
        for (id, object) in &scrub.objects {
            let offset = writer.position();
            write_obj(&mut *writer, &serializer, *id, 0, object, &encryption_handler)?;
            xref_entries.push((*id, offset, 0, true));
        }
        for stream in &scrub.streams {
            Self::write_pass_stream(
                (stream.id, stream.length_id),
                stream.dict.clone(),
                &mut *writer,
                &serializer,
                &encryption_handler,
                deflate,
                &mut xref_entries,
                |out| {
                    out.write_all(&stream.bytes)?;
                    Ok(())
                },
            )?;
        }
        // ── end pdf_manipulator patch ──

        // Write catalog (possibly modified)
        let mut catalog_obj = self
            .modified_objects
            .get(&catalog_ref.id)
            .cloned()
            .unwrap_or(catalog);

        // Remove or rebuild AcroForm after form flattening
        if self.remove_acroform {
            // All pages flattened — drop the entire AcroForm
            if let Some(catalog_dict) = catalog_obj.as_dict() {
                let mut new_catalog = catalog_dict.clone();
                new_catalog.remove("AcroForm");
                catalog_obj = Object::Dictionary(new_catalog);
            }
        } else if !self.flatten_forms_pages.is_empty() {
            // Partial flatten — rebuild AcroForm keeping only fields whose widgets
            // remain on non-flattened pages (ISO 32000-1 §12.7.2).
            if let Some(catalog_dict) = catalog_obj.as_dict() {
                if let Some(rebuilt) = self.rebuild_partial_acroform(catalog_dict)? {
                    let mut new_catalog = catalog_dict.clone();
                    new_catalog.insert("AcroForm".to_string(), rebuilt);
                    catalog_obj = Object::Dictionary(new_catalog);
                }
            }
        }

        // Pre-allocate form field IDs and build AcroForm if we have form field changes
        // Stores: (page_index, object_id, wrapper, is_root_field)
        let mut all_form_field_data: Vec<(usize, u32, FormFieldWrapper, bool)> = Vec::new();
        // Map field name -> allocated ObjectRef (for parent/child linking)
        let mut field_name_to_ref: HashMap<String, ObjectRef> = HashMap::new();

        if self.acroform_modified && !self.remove_acroform {
            // Existing (already-in-document) modified fields are updated IN
            // PLACE: load the original field/widget object, override /V (and
            // /AS for buttons), and stage it in `modified_objects` so the graph
            // copy re-emits it with its full dictionary intact. This is
            // essential for a *merged* field+widget — where the field dict IS
            // the widget annotation (ISO 32000-1 §12.7.4.1): allocating a fresh
            // object for it instead would strand a bare `<< /FT /V >>` (no
            // /T, /Subtype, /Rect) and leave the real widget with an empty
            // value. Also sets /NeedAppearances on the existing AcroForm.
            self.flush_form_fields_to_modified_objects()?;

            // #647: for an *inline* AcroForm the flush above sets
            // /NeedAppearances true by re-writing the catalog into
            // `modified_objects`, but `catalog_obj` was snapshotted before the
            // flush — so without this the patch is silently dropped and viewers
            // honour the field's pre-existing empty /AP /N over /DA, rendering
            // the field blank (ISO 32000-1 §12.7.3.3, Table 226 — AP takes
            // precedence over DA). Re-adopt the patched catalog. Guarded to the
            // pure-fill case: the flatten branches above already rebuild the
            // catalog's AcroForm in-place (and must not be clobbered), and the
            // new-fields branch below emits an AcroForm with NeedAppearances via
            // AcroFormBuilder.
            if !self.remove_acroform && self.flatten_forms_pages.is_empty() {
                if let Some(patched) = self.modified_objects.get(&catalog_ref.id) {
                    catalog_obj = patched.clone();
                }
            }

            // ── pdf_manipulator patch: drop removed fields from the saved
            // AcroForm and their widgets from each page's /Annots (see
            // `remove_deleted_form_fields_from_acroform`). Same guard as the
            // #647 block above: the flatten branches rebuild the AcroForm
            // their own way and must not be clobbered here.
            if !self.remove_acroform && self.flatten_forms_pages.is_empty() {
                if let Some(catalog_dict) = catalog_obj.as_dict() {
                    if let Some(rebuilt) = self.remove_deleted_form_fields_from_acroform(catalog_dict)? {
                        let mut new_catalog = catalog_dict.clone();
                        new_catalog.insert("AcroForm".to_string(), rebuilt);
                        catalog_obj = Object::Dictionary(new_catalog);
                    }
                }
            }
            // ── end pdf_manipulator patch ──

            // Only genuinely NEW fields need freshly-allocated objects plus
            // /Fields and /Annots entries. Existing fields keep their object
            // ids and their place in /Fields/Annots (updated in place above).
            let mut all_wrappers: Vec<_> = self
                .modified_form_fields
                .values()
                .filter(|w| w.is_new())
                .cloned()
                .collect();

            // Sort: parent-only fields first, then terminal fields
            // This ensures parents get IDs before children that reference them
            all_wrappers.sort_by(|a, b| {
                let a_parent = a.is_parent_only();
                let b_parent = b.is_parent_only();
                // Parents first, then by name for deterministic ordering
                match (a_parent, b_parent) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name().cmp(b.name()),
                }
            });

            // First pass: allocate IDs for all fields
            for wrapper in &all_wrappers {
                let field_id = self.allocate_object_id();
                let field_ref = ObjectRef::new(field_id, 0);
                field_name_to_ref.insert(wrapper.name().to_string(), field_ref);
            }

            // Second pass: build field data with parent/child references resolved
            for mut wrapper in all_wrappers {
                let field_id = field_name_to_ref
                    .get(wrapper.name())
                    .map(|r| r.id)
                    .unwrap_or_else(|| self.allocate_object_id());

                // Set parent reference if this is a child field
                if let Some(parent_name) = wrapper.parent_name() {
                    if let Some(&parent_ref) = field_name_to_ref.get(parent_name) {
                        wrapper.set_parent_ref(parent_ref);
                    }
                }

                // Determine if this is a root field (no parent, goes in AcroForm /Fields)
                let is_root = wrapper.parent_name().is_none();

                all_form_field_data.push((wrapper.page_index(), field_id, wrapper, is_root));
            }

            // Update parent wrappers with child references
            // Build a map of parent -> children
            let mut parent_children: HashMap<String, Vec<ObjectRef>> = HashMap::new();
            for (_, field_id, wrapper, _) in &all_form_field_data {
                if let Some(parent_name) = wrapper.parent_name() {
                    parent_children
                        .entry(parent_name.to_string())
                        .or_default()
                        .push(ObjectRef::new(*field_id, 0));
                }
            }

            // Add child refs to parent wrappers
            for (_, _, wrapper, _) in &mut all_form_field_data {
                if let Some(children) = parent_children.get(wrapper.name()) {
                    for &child_ref in children {
                        wrapper.add_child_ref(child_ref);
                    }
                }
            }

            // Build AcroForm dictionary if we have fields
            if !all_form_field_data.is_empty() {
                use crate::writer::AcroFormBuilder;

                let mut acroform_builder = AcroFormBuilder::new();

                // Preserve the existing AcroForm /Fields entries — those fields
                // keep their object ids and were updated in place above, so they
                // must stay referenced (otherwise a fill that also adds a new
                // field would drop every pre-existing field from /Fields).
                if let Ok(cat) = self.source.catalog() {
                    if let Some(af_ref) = cat
                        .as_dict()
                        .and_then(|d| d.get("AcroForm"))
                        .and_then(|o| o.as_reference())
                    {
                        if let Ok(af) = self.load_source_object(af_ref) {
                            // ── pdf_manipulator patch: /Fields may be an indirect reference ──
                            // (ISO 32000-1 §12.7.3). as_array() alone returns None for
                            // the reference form, dropping every pre-existing field from
                            // the rebuilt /Fields. Resolve one level before reading it.
                            let orig_fields_obj = af
                                .as_dict()
                                .and_then(|d| d.get("Fields"))
                                .and_then(|o| self.source.resolve_object(o).ok());
                            if let Some(orig_fields) =
                                orig_fields_obj.as_ref().and_then(|o| o.as_array())
                            {
                                for r in orig_fields {
                                    if let Some(rf) = r.as_reference() {
                                        acroform_builder.add_field(rf);
                                    }
                                }
                            }
                            // ── end pdf_manipulator patch ──
                        }
                    }
                }

                // Add ROOT new fields (no parent) to AcroForm's /Fields array
                for (_, field_id, _, is_root) in &all_form_field_data {
                    if *is_root {
                        acroform_builder.add_field(ObjectRef::new(*field_id, 0));
                    }
                }

                // Build AcroForm dictionary with embedded resources
                let acroform_dict = acroform_builder.build_with_resources();

                // Update catalog to include AcroForm
                if let Some(catalog_dict) = catalog_obj.as_dict() {
                    let mut new_catalog = catalog_dict.clone();
                    new_catalog.insert("AcroForm".to_string(), Object::Dictionary(acroform_dict));
                    catalog_obj = Object::Dictionary(new_catalog);
                }
            }
        }

        // ── pdf_manipulator patch: merged pages keep their form fields ──
        // The merge imported each field object (`carry_merged_acroform`),
        // but only the target's own AcroForm reaches the saved catalog, so
        // without this the merged widgets belong to no field: they are in
        // no `/Fields` array, `FormExtractor` cannot see them and
        // `modify_form_field` cannot reach them.
        let merged_fields: Vec<ObjectRef> = self
            .merged_form_fields
            .iter()
            .filter(|field| !merged_left_out.contains(&field.id))
            .copied()
            .collect();
        if !merged_fields.is_empty() && !self.remove_acroform {
            let merged_dr = self.merged_acroform_dr.as_ref().map(|(source, dr)| {
                let mut dr = dr.clone();
                Self::remap_refs(&mut dr, &self.merged_sources[*source].id_map, 0);
                dr
            });
            let merged_da = self.merged_acroform_da.clone();
            let acroform_entry =
                catalog_obj.as_dict().and_then(|d| d.get("AcroForm")).cloned();
            let mut acroform: HashMap<String, Object> = match acroform_entry {
                Some(Object::Dictionary(d)) => d,
                Some(Object::Reference(r)) => match self.source.load_object(r) {
                    Ok(Object::Dictionary(d)) => d,
                    _ => HashMap::new(),
                },
                _ => HashMap::new(),
            };
            let mut fields = match acroform.get("Fields") {
                Some(Object::Array(arr)) => arr.clone(),
                Some(Object::Reference(r)) => match self.source.load_object(*r) {
                    Ok(Object::Array(arr)) => arr,
                    _ => Vec::new(),
                },
                _ => Vec::new(),
            };
            for field_ref in merged_fields {
                fields.push(Object::Reference(field_ref));
            }
            acroform.insert("Fields".to_string(), Object::Array(fields));
            if !acroform.contains_key("DR") {
                if let Some(dr) = merged_dr {
                    acroform.insert("DR".to_string(), dr);
                }
            }
            if !acroform.contains_key("DA") {
                if let Some(da) = merged_da {
                    acroform.insert("DA".to_string(), da);
                }
            }
            if let Some(catalog_dict) = catalog_obj.as_dict() {
                let mut new_catalog = catalog_dict.clone();
                new_catalog.insert("AcroForm".to_string(), Object::Dictionary(acroform));
                catalog_obj = Object::Dictionary(new_catalog);
            }
        }
        // ── end pdf_manipulator patch ──

        // Write embedded files and update catalog if any files are pending
        let mut embedded_file_refs: Vec<(String, ObjectRef)> = Vec::new();
        let embedded_files = std::mem::take(&mut self.embedded_files);
        if !embedded_files.is_empty() {
            for file in &embedded_files {
                // Allocate IDs for embedded file stream and filespec
                let stream_id = self.allocate_object_id();
                let filespec_id = self.allocate_object_id();

                // Build and write embedded file stream
                let stream_dict = file.build_stream_dict();
                // ── pdf_manipulator patch: stream bodies are StreamBody ──
                let stream_obj = match &file.staged {
                    Some(staged) => Object::Stream { dict: stream_dict, body: staged.body.clone() },
                    None => Object::stream(stream_dict, file.data.clone()),
                };
                // ── end pdf_manipulator patch ──
                let offset = writer.position();
                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                write_obj(
                    &mut *writer,
                    &serializer,
                    stream_id,
                    0,
                    &stream_obj,
                    &encryption_handler,
                )?;
                // ── end pdf_manipulator patch ──
                xref_entries.push((stream_id, offset, 0, true));

                // Build and write filespec dictionary
                let stream_ref = ObjectRef {
                    id: stream_id,
                    gen: 0,
                };
                let filespec_dict = file.build_filespec(stream_ref);
                let filespec_obj = Object::Dictionary(filespec_dict);
                let offset = writer.position();
                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                write_obj(
                    &mut *writer,
                    &serializer,
                    filespec_id,
                    0,
                    &filespec_obj,
                    &encryption_handler,
                )?;
                // ── end pdf_manipulator patch ──
                xref_entries.push((filespec_id, offset, 0, true));

                embedded_file_refs.push((
                    file.name.clone(),
                    ObjectRef {
                        id: filespec_id,
                        gen: 0,
                    },
                ));
            }

            // Update catalog with Names/EmbeddedFiles
            if let Some(catalog_dict) = catalog_obj.as_dict() {
                let mut new_catalog = catalog_dict.clone();

                // Build EmbeddedFiles name tree
                let mut names_array = Vec::new();
                // Sort by name for proper name tree ordering
                let mut sorted_refs = embedded_file_refs.clone();
                sorted_refs.sort_by(|a, b| a.0.cmp(&b.0));
                for (name, ref_) in sorted_refs {
                    names_array.push(Object::String(name.as_bytes().to_vec()));
                    names_array.push(Object::Reference(ref_));
                }

                let mut embedded_files_dict = HashMap::new();
                embedded_files_dict.insert("Names".to_string(), Object::Array(names_array));

                // Get or create Names dictionary
                let mut names_dict = match new_catalog.get("Names") {
                    Some(Object::Dictionary(d)) => d.clone(),
                    _ => HashMap::new(),
                };
                names_dict
                    .insert("EmbeddedFiles".to_string(), Object::Dictionary(embedded_files_dict));
                new_catalog.insert("Names".to_string(), Object::Dictionary(names_dict));

                catalog_obj = Object::Dictionary(new_catalog);
            }
        }

        // ── pdf_manipulator patch: catalog entries for dropped pages removed (#261) ──
        if let Object::Dictionary(ref mut d) = catalog_obj {
            prune.tidy_catalog(d);
        }
        // ── end pdf_manipulator patch ──

        let offset = writer.position();
        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
        write_obj(&mut *writer, &serializer, catalog_ref.id, 0, &catalog_obj, &encryption_handler)?;
        // ── end pdf_manipulator patch ──
        xref_entries.push((catalog_ref.id, offset, 0, true));

        // ── pdf_manipulator patch: merged page ids were allocated by the merge ──
        let merged_page_ids: Vec<u32> = self
            .merged_sources
            .iter()
            .flat_map(|s| s.pages.iter().map(|p| p.page_id))
            .collect();
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: a page object sits in the page tree once; a repeat is a new page object ──
        // `repeat_at` maps a page-order position that repeats an earlier page
        // to the id of its copy; `copies_of` maps a page object's id to the
        // ids of its copies. The copy is written from the page's final
        // dictionary once that is known (`page_copies`).
        let mut repeat_at: HashMap<usize, u32> = HashMap::new();
        let mut copies_of: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut page_copies: Vec<(Vec<u32>, Object)> = Vec::new();
        {
            let leaves = self.get_page_refs().unwrap_or_default();
            let mut seen_source = vec![false; leaves.len()];
            let mut seen_merged = vec![false; merged_page_ids.len()];
            for position in 0..self.page_order.len() {
                cancel.check()?;
                let (seen, original_id) = match self.page_order[position].key() {
                    PageKey::Source(index) if index < leaves.len() => {
                        (&mut seen_source[index], leaves[index].id)
                    },
                    PageKey::Source(_) => continue,
                    PageKey::Merged(index) => (&mut seen_merged[index], merged_page_ids[index]),
                };
                if !std::mem::replace(seen, true) {
                    continue;
                }
                let copy_id = self.allocate_object_id();
                repeat_at.insert(position, copy_id);
                copies_of.entry(original_id).or_default().push(copy_id);
            }
        }
        // ── end pdf_manipulator patch ──

        // Get and write pages tree
        if let Some(catalog_dict) = catalog_obj.as_dict() {
            if let Some(pages_ref) = catalog_dict.get("Pages").and_then(|p| p.as_reference()) {
                let pages_obj = self.load_source_object(pages_ref)?;

                // Rebuild Pages tree: filter by page_order, reorder, append merged pages
                // ── pdf_manipulator patch: the kid list is refs (8 bytes a page); the Kids array object lives only until the Pages object is written ──
                // ── end pdf_manipulator patch ──
                let final_pages_obj = if let Some(pages_dict) = pages_obj.as_dict() {
                    let mut new_pages_dict = pages_dict.clone();

                    // Collect flattened leaf page refs from the (possibly multi-level) page tree.
                    // page_order indices refer to leaf pages, not Kids array entries.
                    // Single tree walk — calling get_page_ref(i) in a loop is O(n²).
                    let original_page_refs: Vec<ObjectRef> =
                        self.source.all_page_refs_cancellable(cancel).unwrap_or_default();

                    // ── pdf_manipulator patch: merged pages sit in /Kids in page order; their own block writes them, not the page loop ──
                    let mut kids: Vec<Object> = Vec::with_capacity(self.page_order.len());
                    for (position, slot) in self.page_order.iter().enumerate() {
                        cancel.check()?;
                        if let Some(&copy_id) = repeat_at.get(&position) {
                            kids.push(Object::Reference(ObjectRef::new(copy_id, 0)));
                            continue;
                        }
                        match slot.key() {
                            PageKey::Source(idx) => {
                                if idx < original_page_refs.len() {
                                    kids.push(Object::Reference(original_page_refs[idx]));
                                }
                            },
                            PageKey::Merged(merged) => {
                                kids.push(Object::Reference(ObjectRef::new(
                                    merged_page_ids[merged],
                                    0,
                                )));
                            },
                        }
                    }
                    // ── end pdf_manipulator patch ──

                    new_pages_dict
                        .insert("Count".to_string(), Object::Integer(kids.len() as i64));
                    new_pages_dict.insert("Kids".to_string(), Object::Array(kids));

                    Object::Dictionary(new_pages_dict)
                } else {
                    pages_obj.clone()
                };

                let offset = writer.position();
                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                write_obj(
                    &mut *writer,
                    &serializer,
                    pages_ref.id,
                    0,
                    &final_pages_obj,
                    &encryption_handler,
                )?;
                // ── end pdf_manipulator patch ──
                xref_entries.push((pages_ref.id, offset, 0, true));
                drop(final_pages_obj);

                        // Build a mapping: output loop position → original source page index.
                        // After select_pages() the page_order may differ from 0..n, so all
                        // per-page HashMaps (modified_content, modified_annotations, …) must
                        // be keyed by the original source index, not the output position.
                        // ── pdf_manipulator patch: merged pages are not source pages ──
                        let source_indices: Vec<usize> = self
                            .page_order
                            .iter()
                            .enumerate()
                            .filter_map(|(position, slot)| match slot.key() {
                                PageKey::Source(i) if !repeat_at.contains_key(&position) => Some(i),
                                _ => None,
                            })
                            .collect();
                        // ── end pdf_manipulator patch ──
                        let source_page_count = source_indices.len();

                        let mut page_index = 0;
                        // ── pdf_manipulator patch: page refs fetched once per save; the page loop walks the source pages of the page order, merged pages are written below ──
                        let cached_page_refs = self.get_page_refs().unwrap_or_default();
                        let kids = source_indices
                            .iter()
                            .filter_map(|&i| cached_page_refs.get(i))
                            .map(|&r| Object::Reference(r));
                        // ── end pdf_manipulator patch ──
                        for kid in kids {
                            // ── pdf_manipulator patch: stop within one page of a cancel ──
                            cancel.check()?;
                            // ── end pdf_manipulator patch ──
                            // ── pdf_manipulator patch: full batches of queued objects become object streams ──
                            if packer.is_full() {
                                Self::write_object_streams(
                                    &packer,
                                    &mut self.next_object_id,
                                    writer,
                                    &serializer,
                                    &encryption_handler,
                                    deflate,
                                    &mut xref_entries,
                                    false,
                                )?;
                            }
                            // ── end pdf_manipulator patch ──
                            if let Some(page_ref) = kid.as_reference() {
                                // ── pdf_manipulator patch: staged page dict wins over source ──
                                // Edits that rewrite the page dictionary
                                // (under-content watermark, content replacement)
                                // stage it in modified_objects. Rebuilding from
                                // source here drops the staged /Contents pointer:
                                // the new content stream becomes an orphan and
                                // the edit vanishes from the saved document.
                                // The page's re-parented form (#261) wins over both.
                                // The resource prune's cuts apply to what is found.
                                let page_obj = prune.cut(
                                    page_ref.id,
                                    if let Some(pruned) = prune.overrides.get(&page_ref.id) {
                                        pruned.clone()
                                    } else if let Some(staged) =
                                        self.modified_objects.get(&page_ref.id)
                                    {
                                        staged.clone()
                                    } else {
                                        self.load_source_object(page_ref)?
                                    },
                                );
                                // ── end pdf_manipulator patch ──

                                // Resolve the original source page index for all HashMap lookups.
                                // Merged pages (appended after source pages) use the loop counter.
                                let source_page_index = if page_index < source_page_count {
                                    source_indices[page_index]
                                } else {
                                    page_index
                                };

                                // Check if we have erase overlays for this page
                                let has_erase_overlay =
                                    self.erase_regions.contains_key(&source_page_index);
                                let erase_overlay_id = if has_erase_overlay {
                                    Some(self.allocate_object_id())
                                } else {
                                    None
                                };

                                // Check if we have new annotations to add for this page
                                let new_annotation_count = self
                                    .modified_annotations
                                    .get(&source_page_index)
                                    .map(|anns| anns.iter().filter(|a| a.is_new()).count())
                                    .unwrap_or(0);
                                let new_annotation_ids: Vec<u32> = (0..new_annotation_count)
                                    .map(|_| self.allocate_object_id())
                                    .collect();

                                // Get pre-allocated form field data for this page
                                // Only include terminal fields (not parent-only) that have widgets
                                let page_form_fields: Vec<(u32, FormFieldWrapper)> =
                                    all_form_field_data
                                        .iter()
                                        .filter(|(pg_idx, _, wrapper, _)| {
                                            *pg_idx == source_page_index
                                                && !wrapper.is_parent_only()
                                        })
                                        .map(|(_, id, wrapper, _)| (*id, wrapper.clone()))
                                        .collect();
                                let new_form_field_ids: Vec<u32> =
                                    page_form_fields.iter().map(|(id, _)| *id).collect();
                                let new_form_field_wrappers: Vec<FormFieldWrapper> =
                                    page_form_fields.iter().map(|(_, w)| w.clone()).collect();

                                // ── pdf_manipulator patch: a redacted page's appearances are drawn by its redacted content ──
                                let flat_redacted = self
                                    .redacted_pages
                                    .get(&source_page_index)
                                    .is_some_and(|page| page.flat.is_some());
                                // ── end pdf_manipulator patch ──
                                // Check if we need to flatten annotations for this page
                                // ── pdf_manipulator patch: a redacted page's appearances are drawn by its redacted content ──
                                let should_flatten = !flat_redacted
                                    && self.flatten_annotations_pages.contains(&source_page_index);
                                // ── end pdf_manipulator patch ──
                                let flatten_data: Option<(
                                    Vec<AnnotationAppearance>,
                                    u32,
                                    Vec<(u32, String)>,
                                )> = if should_flatten {
                                    // Get annotation appearances
                                    let appearances =
                                        self.get_annotation_appearances(source_page_index)?;
                                    if !appearances.is_empty() {
                                        // Allocate object IDs for each XObject and one for the overlay
                                        let overlay_id = self.allocate_object_id();
                                        let xobj_ids: Vec<(u32, String)> = appearances
                                            .iter()
                                            .enumerate()
                                            .map(|(i, _)| {
                                                let id = self.allocate_object_id();
                                                let name = format!("FlatAnnot{}", i);
                                                (id, name)
                                            })
                                            .collect();
                                        Some((appearances, overlay_id, xobj_ids))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                // ── pdf_manipulator patch: the redacted content and the marks, per page ──
                                // Destructive redaction (#231): the page's content is
                                // rewritten from its source as the save writes it,
                                // so the secret is never held. The pair is the content
                                // stream's id and its /Length object's id.
                                let destructive_redaction: Option<(u32, u32)> =
                                    if self.redacted_pages.contains_key(&source_page_index) {
                                        Some((self.allocate_object_id(), self.allocate_object_id()))
                                    } else {
                                        None
                                    };
                                // ── end pdf_manipulator patch ──

                                // ── pdf_manipulator patch: the content stream of a page whose image bounds changed ──
                                let image_mods_content: Option<(u32, u32)> = if destructive_redaction.is_none()
                                    && self.image_modifications.contains_key(&source_page_index)
                                    && !(self.structure_modified
                                        && self.modified_content.contains_key(&source_page_index))
                                    && page_obj.as_dict().is_some_and(|d| d.contains_key("Contents"))
                                {
                                    Some((self.allocate_object_id(), self.allocate_object_id()))
                                } else {
                                    None
                                };
                                // ── end pdf_manipulator patch ──

                                // ── pdf_manipulator patch: the ids each queued copy is written under ──
                                // An image copy takes a pair for its stream and one for its
                                // /SMask (or /Mask stencil) stream; a mesh or form copy takes
                                // a stream and its /Length. All are allocated with the rest of
                                // the page's ids so the /Resources rebuild can name them.
                                let mut redaction_ids = match self.redacted_pages.get(&source_page_index) {
                                    Some(page) => Self::allocate_scope_ids(
                                        &self.source,
                                        &mut self.next_object_id,
                                        &page.scope,
                                    )?,
                                    None => ScopeIds::default(),
                                };
                                if let Some(page) = self.redacted_pages.get(&source_page_index) {
                                    scrub.attach(source_page_index, &page.scope, &mut redaction_ids);
                                }
                                // ── end pdf_manipulator patch ──
                                // ── pdf_manipulator patch: the /Redact annotations of marks not yet applied ──
                                // A queued mark that was not applied is saved as a /Redact
                                // annotation (ISO 32000-1 §12.5.6.23) and the content is left
                                // as it is. A redacted page writes no marks.
                                let mark_annots: Vec<(u32, HashMap<String, Object>)> =
                                    if destructive_redaction.is_none() {
                                        self.redaction_mark_annotations(source_page_index)
                                            .into_iter()
                                            .map(|mark| (self.allocate_object_id(), mark))
                                            .collect()
                                    } else {
                                        Vec::new()
                                    };
                                // ── end pdf_manipulator patch ──

                                // Check if we need to flatten form fields for this page
                                // ── pdf_manipulator patch: a redacted page's appearances are drawn by its redacted content ──
                                let should_flatten_forms = !flat_redacted
                                    && self.flatten_forms_pages.contains(&source_page_index);
                                // ── end pdf_manipulator patch ──
                                let form_flatten_data: Option<(
                                    Vec<AnnotationAppearance>,
                                    u32,
                                    Vec<(u32, String)>,
                                )> = if should_flatten_forms {
                                    let appearances =
                                        self.get_widget_appearances(source_page_index)?;
                                    if !appearances.is_empty() {
                                        let overlay_id = self.allocate_object_id();
                                        let xobj_ids: Vec<(u32, String)> = appearances
                                            .iter()
                                            .enumerate()
                                            .map(|(i, _)| {
                                                let id = self.allocate_object_id();
                                                let name = format!("FlatForm{}", i);
                                                (id, name)
                                            })
                                            .collect();
                                        Some((appearances, overlay_id, xobj_ids))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };

                                // Check if we have modified content for this page
                                let modified_content_id: Option<u32> = if self.structure_modified
                                    && self.modified_content.contains_key(&source_page_index)
                                {
                                    Some(self.allocate_object_id())
                                } else {
                                    None
                                };

                                // Check if we have overlay additions for this page
                                let overlay_additions_id: Option<u32> =
                                    if self.overlay_additions.contains_key(&source_page_index) {
                                        Some(self.allocate_object_id())
                                    } else {
                                        None
                                    };

                                // Every Standard-14 base-font name that the overlay content
                                // stream will actually emit via `Tf` needs a matching Font
                                // object registered in this page's `/Resources/Font`, else the
                                // resource name is dangling.
                                //
                                // The name to register is NOT the raw `font.name` the caller
                                // passed — it's the transformed name the writer emits, i.e.
                                // `map_base14_font_name(&font.name, is_bold)` (the same source
                                // of truth used by `ContentStreamBuilder::map_font_name`). Keying
                                // off the raw name diverges from the emitted `Tf` name for:
                                //   - bold/italic overlays (raw "Helvetica" + bold → emits
                                //     "/Helvetica-Bold");
                                //   - generic families ("Arial", "sans-serif" → "/Helvetica");
                                //   - Symbol/ZapfDingbats (routed to "/Helvetica").
                                // Registering the emitted name closes all three; only Standard-14
                                // names are ever emitted here (embedded fonts take the
                                // ShowEmbeddedText path and never reach this registration).
                                // ── pdf_manipulator patch: name order, so the font objects are written in a fixed order ──
                                let mut overlay_font_ids: std::collections::BTreeMap<String, u32> =
                                    std::collections::BTreeMap::new();
                                // ── end pdf_manipulator patch ──
                                if let Some(added) = self.overlay_additions.get(&source_page_index)
                                {
                                    let mut needed: Vec<String> = added
                                        .iter()
                                        .filter_map(|el| match el {
                                            ContentElement::Text(t) => {
                                                Some(crate::writer::map_base14_font_name(
                                                    &t.font.name,
                                                    t.is_bold(),
                                                ))
                                            },
                                            _ => None,
                                        })
                                        .collect();
                                    needed.sort();
                                    needed.dedup();
                                    for name in needed {
                                        overlay_font_ids.insert(name, self.allocate_object_id());
                                    }
                                }

                                // Apply page property modifications if any
                                let mut final_page_obj = if let Some(props) =
                                    self.modified_page_props.get(&source_page_index)
                                {
                                    self.apply_page_props_to_object(&page_obj, props)?
                                } else {
                                    page_obj.clone()
                                };

                                // ── pdf_manipulator patch: the page draws the content written on the belt ──
                                // The streams it replaces are written only where something else
                                // still names them.
                                if let (Some((content_id, _)), Some(page_dict)) =
                                    (image_mods_content, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();
                                    if let Some(contents) = new_dict.get("Contents").cloned() {
                                        if let Some(r) = contents.as_reference() {
                                            prune.redacted_image_originals.insert(r.id);
                                        }
                                        let resolved = self.source.resolve_object(&contents)?;
                                        for part in resolved.as_array().into_iter().flatten() {
                                            if let Some(r) = part.as_reference() {
                                                prune.redacted_image_originals.insert(r.id);
                                            }
                                        }
                                    }
                                    new_dict.insert(
                                        "Contents".to_string(),
                                        Object::Reference(ObjectRef::new(content_id, 0)),
                                    );
                                    final_page_obj = Object::Dictionary(new_dict);
                                }
                                // ── end pdf_manipulator patch ──

                                // If we have an erase overlay, update Contents to include it
                                if let (Some(overlay_obj_id), Some(page_dict)) =
                                    (erase_overlay_id, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();
                                    // Get existing Contents reference
                                    if let Some(contents) = new_dict.get("Contents").cloned() {
                                        // Create an array with original content + overlay
                                        let overlay_ref =
                                            Object::Reference(ObjectRef::new(overlay_obj_id, 0));
                                        let contents_array = match contents {
                                            Object::Reference(_) => {
                                                Object::Array(vec![contents, overlay_ref])
                                            },
                                            Object::Array(mut arr) => {
                                                arr.push(overlay_ref);
                                                Object::Array(arr)
                                            },
                                            _ => Object::Array(vec![contents, overlay_ref]),
                                        };
                                        new_dict.insert("Contents".to_string(), contents_array);
                                    } else {
                                        // ── pdf_manipulator patch ──
                                        // A page with no /Contents (legal —
                                        // blank page) still gets the overlay:
                                        // it becomes the page's content.
                                        new_dict.insert(
                                            "Contents".to_string(),
                                            Object::Reference(ObjectRef::new(overlay_obj_id, 0)),
                                        );
                                        // ── end pdf_manipulator patch ──
                                    }
                                    final_page_obj = Object::Dictionary(new_dict);
                                }

                                // If we have overlay additions (add_text on existing page), append
                                // a new stream after the original content rather than replacing it.
                                if let (Some(additions_id), Some(page_dict)) =
                                    (overlay_additions_id, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();
                                    if let Some(contents) = new_dict.get("Contents").cloned() {
                                        let additions_ref =
                                            Object::Reference(ObjectRef::new(additions_id, 0));
                                        let contents_array = match contents {
                                            Object::Reference(_) => {
                                                Object::Array(vec![contents, additions_ref])
                                            },
                                            Object::Array(mut arr) => {
                                                arr.push(additions_ref);
                                                Object::Array(arr)
                                            },
                                            _ => Object::Array(vec![contents, additions_ref]),
                                        };
                                        new_dict.insert("Contents".to_string(), contents_array);
                                    } else {
                                        // ── pdf_manipulator patch ──
                                        // A page with no /Contents (legal —
                                        // blank page) still gets the overlay:
                                        // it becomes the page's content.
                                        new_dict.insert(
                                            "Contents".to_string(),
                                            Object::Reference(ObjectRef::new(additions_id, 0)),
                                        );
                                        // ── end pdf_manipulator patch ──
                                    }
                                    final_page_obj = Object::Dictionary(new_dict);
                                }

                                // Register any Base-14 fonts used by overlay_additions text
                                // into this page's /Resources/Font, so the `Tf` operator in the
                                // overlay stream resolves to a real font object instead of an
                                // undeclared resource name. /Resources (and /Resources/Font)
                                // are very commonly *indirect references* on a page loaded from
                                // an existing document, not inline dictionaries — both levels
                                // must be resolved via `self.source.load_object()` or the
                                // existing Font entries (e.g. /F1, /F3 used by the page's
                                // original content) are lost when /Resources is rebuilt below.
                                if !overlay_font_ids.is_empty() {
                                    if let Some(page_dict) = final_page_obj.as_dict() {
                                        let mut new_dict = page_dict.clone();

                                        let resolve_dict = |obj: &Object,
                                                             src: &PdfDocument|
                                         -> HashMap<String, Object> {
                                            match obj {
                                                Object::Dictionary(d) => d.clone(),
                                                Object::Reference(r) => src
                                                    .load_object(*r)
                                                    .ok()
                                                    .and_then(|o| o.as_dict().cloned())
                                                    .unwrap_or_default(),
                                                _ => HashMap::new(),
                                            }
                                        };

                                        // /Resources is an inheritable attribute (ISO 32000-1
                                        // §7.7.3.4): a page may legally omit its own /Resources
                                        // and inherit it from an ancestor in the /Pages chain.
                                        // Writing a fresh font-only /Resources here would shadow
                                        // that inherited dict and drop the page's real fonts, so
                                        // when the page has no own /Resources we seed from the
                                        // inherited one.
                                        //
                                        // SECURITY (destructive redaction): we seed the page's
                                        // materialised /Resources with ONLY the inherited /Font
                                        // sub-dictionary, never the whole inherited dict. An
                                        // ancestor /Resources can carry /XObject, /Pattern, etc.
                                        // that reference content-bearing streams unrelated to
                                        // this page; copying them wholesale onto the page would
                                        // make them freshly reachable and thus survive the
                                        // orphan-drop backstop — re-introducing exactly the kind
                                        // of stray reference destructive redaction must not
                                        // resurrect. /Font is the only sub-dict the overlay `Tf`
                                        // resolution needs, and font programs are not a redaction
                                        // leak surface here (the secret lives in content streams,
                                        // which are rewritten). A page that genuinely renders an
                                        // inherited XObject still inherits it at render time via
                                        // the /Pages chain — we are not removing that path, only
                                        // declining to inline-copy it onto the page.
                                        let mut resources_dict: HashMap<String, Object> =
                                            match new_dict.get("Resources") {
                                                Some(obj) => resolve_dict(obj, &self.source),
                                                None => {
                                                    let mut node = new_dict
                                                        .get("Parent")
                                                        .and_then(|p| p.as_reference())
                                                        .and_then(|r| {
                                                            self.load_source_object(r).ok()
                                                        });
                                                    let mut inherited_font: Option<Object> = None;
                                                    let mut guard = 0;
                                                    while let Some(obj) = node {
                                                        guard += 1;
                                                        if guard > 64 {
                                                            break; // cyclic /Parent — bail
                                                        }
                                                        let Some(dict) = obj.as_dict() else {
                                                            break;
                                                        };
                                                        if let Some(res) = dict.get("Resources") {
                                                            // Found the inherited /Resources:
                                                            // take ONLY its /Font sub-dict.
                                                            let res_dict =
                                                                resolve_dict(res, &self.source);
                                                            inherited_font =
                                                                res_dict.get("Font").cloned();
                                                            break;
                                                        }
                                                        node = dict
                                                            .get("Parent")
                                                            .and_then(|p| p.as_reference())
                                                            .and_then(|r| {
                                                                self.load_source_object(r).ok()
                                                            });
                                                    }
                                                    let mut seeded: HashMap<String, Object> =
                                                        HashMap::new();
                                                    if let Some(font) = inherited_font {
                                                        seeded.insert("Font".to_string(), font);
                                                    }
                                                    seeded
                                                },
                                            };
                                        let mut font_dict: HashMap<String, Object> =
                                            match resources_dict.get("Font") {
                                                Some(obj) => resolve_dict(obj, &self.source),
                                                None => HashMap::new(),
                                            };
                                        for (name, id) in &overlay_font_ids {
                                            font_dict.insert(
                                                name.clone(),
                                                Object::Reference(ObjectRef::new(*id, 0)),
                                            );
                                        }
                                        resources_dict.insert(
                                            "Font".to_string(),
                                            Object::Dictionary(font_dict),
                                        );
                                        new_dict.insert(
                                            "Resources".to_string(),
                                            Object::Dictionary(resources_dict),
                                        );
                                        final_page_obj = Object::Dictionary(new_dict);
                                    }
                                }

                                // If we're flattening annotations, update page dictionary
                                if let (
                                    Some((ref appearances, flatten_overlay_id, ref xobj_ids)),
                                    Some(page_dict),
                                ) = (&flatten_data, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();

                                    // Add flatten overlay to Contents
                                    if let Some(contents) = new_dict.get("Contents").cloned() {
                                        let overlay_ref = Object::Reference(ObjectRef::new(
                                            *flatten_overlay_id,
                                            0,
                                        ));
                                        let contents_array = match contents {
                                            Object::Reference(_) => {
                                                Object::Array(vec![contents, overlay_ref])
                                            },
                                            Object::Array(mut arr) => {
                                                arr.push(overlay_ref);
                                                Object::Array(arr)
                                            },
                                            _ => Object::Array(vec![contents, overlay_ref]),
                                        };
                                        new_dict.insert("Contents".to_string(), contents_array);
                                    } else {
                                        // ── pdf_manipulator patch ──
                                        // A page with no /Contents (legal —
                                        // blank page) still gets the overlay:
                                        // it becomes the page's content.
                                        new_dict.insert(
                                            "Contents".to_string(),
                                            Object::Reference(ObjectRef::new(*flatten_overlay_id, 0)),
                                        );
                                        // ── end pdf_manipulator patch ──
                                    }

                                    // Add XObjects to Resources
                                    let resources = new_dict.get("Resources").cloned();
                                    let mut resources_dict = match resources {
                                        Some(Object::Dictionary(d)) => d,
                                        Some(Object::Reference(res_ref)) => {
                                            match self.load_source_object(res_ref) {
                                                Ok(Object::Dictionary(d)) => d,
                                                _ => HashMap::new(),
                                            }
                                        },
                                        _ => HashMap::new(),
                                    };

                                    // Get or create XObject subdictionary
                                    let mut xobject_dict = match resources_dict.get("XObject") {
                                        Some(Object::Dictionary(d)) => d.clone(),
                                        Some(Object::Reference(xobj_ref)) => {
                                            match self.load_source_object(*xobj_ref) {
                                                Ok(Object::Dictionary(d)) => d,
                                                _ => HashMap::new(),
                                            }
                                        },
                                        _ => HashMap::new(),
                                    };

                                    // Add our flattened annotation XObjects
                                    for (obj_id, name) in xobj_ids {
                                        xobject_dict.insert(
                                            name.clone(),
                                            Object::Reference(ObjectRef::new(*obj_id, 0)),
                                        );
                                    }

                                    resources_dict.insert(
                                        "XObject".to_string(),
                                        Object::Dictionary(xobject_dict),
                                    );
                                    new_dict.insert(
                                        "Resources".to_string(),
                                        Object::Dictionary(resources_dict),
                                    );

                                    // Remove /Annots array
                                    new_dict.remove("Annots");

                                    final_page_obj = Object::Dictionary(new_dict);
                                }

                                // ── pdf_manipulator patch: queued marks not yet applied join the page's /Annots ──
                                if !mark_annots.is_empty() {
                                    if let Some(page_dict) = final_page_obj.as_dict() {
                                        let mut new_dict = page_dict.clone();
                                        let mut annots = match self
                                            .annots_without(&new_dict, &std::collections::HashSet::new())
                                        {
                                            Some(Object::Array(list)) => list,
                                            _ => Vec::new(),
                                        };
                                        annots.extend(
                                            mark_annots
                                                .iter()
                                                .map(|(id, _)| Object::Reference(ObjectRef::new(*id, 0))),
                                        );
                                        new_dict.insert("Annots".to_string(), Object::Array(annots));
                                        final_page_obj = Object::Dictionary(new_dict);
                                    }
                                }
                                // ── end pdf_manipulator patch ──

                                // Destructive redaction (#231): REPLACE the
                                // page /Contents with the single rewritten
                                // stream (secret physically removed + opaque
                                // overlay). The old content object becomes
                                // unreferenced and is dropped by the
                                // garbage-collected full rewrite (no residual
                                // recoverable bytes, G6). /Redact annotations
                                // are removed (ISO 32000-1 §12.5.6.23).
                                if let (Some((redacted_id, _)), Some(page_dict)) =
                                    (&destructive_redaction, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();
                                    let redacted_ref =
                                        Object::Reference(ObjectRef::new(*redacted_id, 0));
                                    // If overlay-additions already appended a second stream to
                                    // Contents earlier in this save (DOM edits to a source page
                                    // via `save_page`, #940), keep that addition but replace
                                    // *all* original content references with the single redacted
                                    // stream — not just entry 0. `redacted_ref` already carries
                                    // the fully redacted content of every original stream, so any other
                                    // original entries left in the array would be pure
                                    // duplicates, and every one of them is dropped from the
                                    // output as an orphan (`redacted_orphan_ids`) regardless —
                                    // leaving them referenced here produces a dangling
                                    // `/Contents` entry that strict readers reject (#799, #941).
                                    // We know the addition's object id directly, so there's no
                                    // need to infer it positionally from the array.
                                    let contents_value = match overlay_additions_id {
                                        Some(additions_id) => Object::Array(vec![
                                            redacted_ref,
                                            Object::Reference(ObjectRef::new(additions_id, 0)),
                                        ]),
                                        None => redacted_ref,
                                    };
                                    new_dict.insert("Contents".to_string(), contents_value);
                                    // ── pdf_manipulator patch: the redacted page's own /Resources ──
                                    // The copy names the pass queued point at their copies; a
                                    // source name stays only when a surviving `Do` names it.
                                    let redacted = self
                                        .redacted_pages
                                        .get(&source_page_index)
                                        .ok_or_else(|| {
                                            Error::InvalidPdf(
                                                "a redacted page's recorded pass is missing".to_string(),
                                            )
                                        })?;
                                    let resources = self.redacted_page_resources(
                                        source_page_index,
                                        redacted,
                                        &redaction_ids,
                                    )?;
                                    new_dict.insert("Resources".to_string(), resources);
                                    // ── end pdf_manipulator patch ──
                                    // ── pdf_manipulator patch: the redacted page's dropped annotations leave /Annots ──
                                    let dropped = self.redacted_pages[&source_page_index]
                                        .dropped_annots
                                        .clone();
                                    match self.annots_without(&new_dict, &dropped) {
                                        Some(annots) => new_dict.insert("Annots".to_string(), annots),
                                        None => new_dict.remove("Annots"),
                                    };
                                    // The flattened annotations and widgets left the page.
                                    for set in redacted.flat.iter().flat_map(|flat| &flat.sets) {
                                        let kept: Vec<Object> = match new_dict.get("Annots") {
                                            Some(Object::Array(list)) if set.widgets => list
                                                .iter()
                                                .filter(|annot| {
                                                    !annot
                                                        .as_reference()
                                                        .and_then(|r| self.load_source_object(r).ok())
                                                        .and_then(|a| {
                                                            a.as_dict()
                                                                .and_then(|d| d.get("Subtype"))
                                                                .and_then(|s| s.as_name().map(str::to_string))
                                                        })
                                                        .is_some_and(|subtype| subtype == "Widget")
                                                })
                                                .cloned()
                                                .collect(),
                                            _ => Vec::new(),
                                        };
                                        if kept.is_empty() {
                                            new_dict.remove("Annots");
                                        } else {
                                            new_dict.insert("Annots".to_string(), Object::Array(kept));
                                        }
                                    }
                                    // ── end pdf_manipulator patch ──
                                    final_page_obj = Object::Dictionary(new_dict);
                                }

                                // If we're flattening form fields, update page dictionary
                                if let (
                                    Some((
                                        ref form_appearances,
                                        form_overlay_id,
                                        ref form_xobj_ids,
                                    )),
                                    Some(page_dict),
                                ) = (&form_flatten_data, final_page_obj.as_dict())
                                {
                                    let mut new_dict = page_dict.clone();

                                    // Add form flatten overlay to Contents
                                    if let Some(contents) = new_dict.get("Contents").cloned() {
                                        let overlay_ref =
                                            Object::Reference(ObjectRef::new(*form_overlay_id, 0));
                                        let contents_array = match contents {
                                            Object::Reference(_) => {
                                                Object::Array(vec![contents, overlay_ref])
                                            },
                                            Object::Array(mut arr) => {
                                                arr.push(overlay_ref);
                                                Object::Array(arr)
                                            },
                                            _ => Object::Array(vec![contents, overlay_ref]),
                                        };
                                        new_dict.insert("Contents".to_string(), contents_array);
                                    } else {
                                        // ── pdf_manipulator patch ──
                                        // A page with no /Contents (legal —
                                        // blank page) still gets the overlay:
                                        // it becomes the page's content.
                                        new_dict.insert(
                                            "Contents".to_string(),
                                            Object::Reference(ObjectRef::new(*form_overlay_id, 0)),
                                        );
                                        // ── end pdf_manipulator patch ──
                                    }

                                    // Add XObjects to Resources
                                    let resources = new_dict.get("Resources").cloned();
                                    let mut resources_dict = match resources {
                                        Some(Object::Dictionary(d)) => d,
                                        Some(Object::Reference(res_ref)) => {
                                            match self.load_source_object(res_ref) {
                                                Ok(Object::Dictionary(d)) => d,
                                                _ => HashMap::new(),
                                            }
                                        },
                                        _ => HashMap::new(),
                                    };

                                    // Get or create XObject subdictionary
                                    let mut xobject_dict = match resources_dict.get("XObject") {
                                        Some(Object::Dictionary(d)) => d.clone(),
                                        Some(Object::Reference(xobj_ref)) => {
                                            match self.load_source_object(*xobj_ref) {
                                                Ok(Object::Dictionary(d)) => d,
                                                _ => HashMap::new(),
                                            }
                                        },
                                        _ => HashMap::new(),
                                    };

                                    // Add flattened form XObjects
                                    for (obj_id, name) in form_xobj_ids {
                                        xobject_dict.insert(
                                            name.clone(),
                                            Object::Reference(ObjectRef::new(*obj_id, 0)),
                                        );
                                    }

                                    resources_dict.insert(
                                        "XObject".to_string(),
                                        Object::Dictionary(xobject_dict),
                                    );
                                    new_dict.insert(
                                        "Resources".to_string(),
                                        Object::Dictionary(resources_dict),
                                    );

                                    // Remove Widget annotations from /Annots array, preserving others
                                    if let Some(annots) = new_dict.get("Annots").cloned() {
                                        let annots_array = match annots {
                                            Object::Array(arr) => arr,
                                            Object::Reference(annots_ref) => {
                                                match self.load_source_object(annots_ref) {
                                                    Ok(Object::Array(arr)) => arr,
                                                    _ => vec![],
                                                }
                                            },
                                            _ => vec![],
                                        };

                                        // Filter out Widget annotations
                                        let mut filtered_annots = Vec::new();
                                        for (at, annot_ref) in annots_array.into_iter().enumerate() {
                                            // ── pdf_manipulator patch: each annotation is read once ──
                                            if at % 256 == 255 {
                                                self.source.clear_object_cache();
                                            }
                                            // ── end pdf_manipulator patch ──
                                            if let Some(ref_obj) = annot_ref.as_reference() {
                                                if let Ok(annot_obj) =
                                                    self.load_source_object(ref_obj)
                                                {
                                                    if let Some(annot_dict) = annot_obj.as_dict() {
                                                        let subtype = annot_dict
                                                            .get("Subtype")
                                                            .and_then(|s| s.as_name());
                                                        if subtype != Some("Widget") {
                                                            // Keep non-Widget annotations
                                                            filtered_annots.push(annot_ref);
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        if filtered_annots.is_empty() {
                                            // All annotations were widgets, remove Annots entirely
                                            new_dict.remove("Annots");
                                        } else {
                                            // Keep remaining annotations
                                            new_dict.insert(
                                                "Annots".to_string(),
                                                Object::Array(filtered_annots),
                                            );
                                        }
                                    }

                                    final_page_obj = Object::Dictionary(new_dict);
                                }

                                // Add new annotations and form fields to the page's /Annots array
                                if !new_annotation_ids.is_empty() || !new_form_field_ids.is_empty()
                                {
                                    if let Some(page_dict) = final_page_obj.as_dict() {
                                        let mut new_dict = page_dict.clone();

                                        // Get existing Annots array or create new one
                                        let mut annots_array = match new_dict.get("Annots").cloned()
                                        {
                                            Some(Object::Array(arr)) => arr,
                                            Some(Object::Reference(annots_ref)) => {
                                                match self.load_source_object(annots_ref) {
                                                    Ok(Object::Array(arr)) => arr,
                                                    _ => vec![],
                                                }
                                            },
                                            _ => vec![],
                                        };

                                        // Add references to new annotations
                                        for annot_id in &new_annotation_ids {
                                            annots_array.push(Object::Reference(ObjectRef::new(
                                                *annot_id, 0,
                                            )));
                                        }

                                        // Add references to new form fields (widget annotations)
                                        for field_id in &new_form_field_ids {
                                            annots_array.push(Object::Reference(ObjectRef::new(
                                                *field_id, 0,
                                            )));
                                        }

                                        new_dict.insert(
                                            "Annots".to_string(),
                                            Object::Array(annots_array),
                                        );
                                        final_page_obj = Object::Dictionary(new_dict);
                                    }
                                }

                                // Update page's /Contents reference if we have modified content
                                if let Some(new_content_id) = modified_content_id {
                                    if let Some(page_dict) = final_page_obj.as_dict() {
                                        let mut new_dict = page_dict.clone();
                                        // Replace the Contents reference with the new content stream
                                        new_dict.insert(
                                            "Contents".to_string(),
                                            Object::Reference(ObjectRef::new(new_content_id, 0)),
                                        );
                                        final_page_obj = Object::Dictionary(new_dict);
                                    }
                                }

                                let offset = writer.position();
                                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                write_obj(
                                    &mut *writer,
                                    &serializer,
                                    page_ref.id,
                                    0,
                                    &final_page_obj,
                                    &encryption_handler,
                                )?;
                                // ── end pdf_manipulator patch ──
                                xref_entries.push((page_ref.id, offset, 0, true));
                                // ── pdf_manipulator patch: a repeated page is copied from this final dictionary ──
                                if let Some(copy_ids) = copies_of.get(&page_ref.id) {
                                    page_copies.push((copy_ids.clone(), final_page_obj.clone()));
                                }
                                // ── end pdf_manipulator patch ──

                                // Collect new XObject refs from content generation (for Resources update)
                                let mut new_xobject_refs: Vec<(String, ObjectRef)> = Vec::new();

                                // Write page contents if present
                                if let Some(page_dict) = page_obj.as_dict() {
                                    // Check if this page has modified content (structure rebuild)
                                    if self.structure_modified
                                        && self.modified_content.contains_key(&source_page_index)
                                    {
                                        // Generate new content stream from modified StructureElement
                                        if let Some(structure) =
                                            self.modified_content.get(&source_page_index)
                                        {
                                            let (content_bytes, pending_images) =
                                                self.generate_content_stream(structure)?;

                                            // Create XObject entries for pending images
                                            let mut xobject_refs: Vec<(String, ObjectRef)> =
                                                Vec::new();
                                            for pending_image in pending_images {
                                                let xobj_id = self.allocate_object_id();

                                                // Build XObject stream for the image
                                                let xobj_stream =
                                                    Self::build_image_xobject(&pending_image.image);
                                                let offset = writer.position();
                                                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                                write_obj(
                                                    &mut *writer,
                                                    &serializer,
                                                    xobj_id,
                                                    0,
                                                    &xobj_stream,
                                                    &encryption_handler,
                                                )?;
                                                // ── end pdf_manipulator patch ──
                                                xref_entries.push((xobj_id, offset, 0, true));

                                                xobject_refs.push((
                                                    pending_image.resource_id,
                                                    ObjectRef::new(xobj_id, 0),
                                                ));
                                            }

                                            // Create stream object for the content
                                            // ── pdf_manipulator patch: stream bodies are StreamBody ──
                                            let content_stream_obj = Object::stream(
                                                HashMap::new(),
                                                content_bytes,
                                            );
                                            // ── end pdf_manipulator patch ──

                                            // Use the pre-allocated content ID (page /Contents already updated)
                                            if let Some(content_id) = modified_content_id {
                                                let offset = writer.position();
                                                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                                write_obj(
                                                    &mut *writer,
                                                    &serializer,
                                                    content_id,
                                                    0,
                                                    &content_stream_obj,
                                                    &encryption_handler,
                                                )?;
                                                // ── end pdf_manipulator patch ──
                                                xref_entries.push((content_id, offset, 0, true));
                                            }

                                            // Collect xobject refs for Resources/XObject update
                                            new_xobject_refs.extend(xobject_refs);
                                        }
                                    } else {
                                        // ── pdf_manipulator patch: a page's image bounds are written on the belt ──
                                        // One new content stream, read through the page's
                                        // operators; a stream that cannot be read fails the
                                        // save, and the original content is never written
                                        // in its place.
                                        if let Some(ids) = image_mods_content {
                                            let mods = &self.image_modifications[&source_page_index];
                                            Self::write_pass_stream(
                                                ids,
                                                HashMap::new(),
                                                &mut *writer,
                                                &serializer,
                                                &encryption_handler,
                                                deflate,
                                                &mut xref_entries,
                                                |out| {
                                                    Self::write_image_mods_content(
                                                        &self.source,
                                                        source_page_index,
                                                        mods,
                                                        out,
                                                    )
                                                },
                                            )?;
                                        // ── end pdf_manipulator patch ──
                                        } else if destructive_redaction.is_none() {
                                            // Use original contents — but NEVER when this
                                            // page was destructively redacted (#231 G6):
                                            // the redacted replacement is written below
                                            // and the page /Contents now points to it;
                                            // emitting the original here would re-introduce
                                            // the secret bytes.
                                            if let Some(contents_ref) = page_dict
                                                .get("Contents")
                                                .and_then(|c| c.as_reference())
                                            {
                                                // ── pdf_manipulator patch: staged stream wins over source; an unchanged one is copied ──
                                                // Prefer a staged stream: an
                                                // under-content edit re-points
                                                // /Contents at a NEW object that
                                                // exists only in modified_objects.
                                                // Loading it from source yields
                                                // null, poisons the xref, and the
                                                // real stream is then skipped as
                                                // "already written".
                                                let staged = self.modified_objects.get(&contents_ref.id);
                                                if staged.is_some()
                                                    || !Self::write_source_stream(
                                                        &self.source,
                                                        &mut self.next_object_id,
                                                        contents_ref,
                                                        None,
                                                        writer,
                                                        &serializer,
                                                        &encryption_handler,
                                                        deflate,
                                                        &mut xref_entries,
                                                    )?
                                                {
                                                    let contents_obj = if let Some(staged) = staged {
                                                        staged.clone()
                                                    } else {
                                                        self.load_source_object(contents_ref)?
                                                    };
                                                    let offset = writer.position();
                                                    write_obj(
                                                        &mut *writer,
                                                        &serializer,
                                                        contents_ref.id,
                                                        0,
                                                        &contents_obj,
                                                        &encryption_handler,
                                                    )?;
                                                    xref_entries.push((
                                                        contents_ref.id,
                                                        offset,
                                                        0,
                                                        true,
                                                    ));
                                                }
                                                // ── end pdf_manipulator patch ──
                                            }
                                        }
                                    }

                                    // Write resources if present (as reference)
                                    if let Some(resources_ref) =
                                        page_dict.get("Resources").and_then(|r| r.as_reference())
                                    {
                                        // ── pdf_manipulator patch: the prune plan's resource dictionary wins ──
                                        let mut resources_obj =
                                            self.pruned_or_source_object(&prune, resources_ref)?;
                                        // ── end pdf_manipulator patch ──

                                        // Inject new XObject refs into Resources dict
                                        if !new_xobject_refs.is_empty() {
                                            if let Some(res_dict) = resources_obj.as_dict() {
                                                let mut new_res = res_dict.clone();
                                                // Resolve existing XObject dict (may be inline or indirect ref)
                                                let mut xobj_entries = match new_res.get("XObject")
                                                {
                                                    Some(Object::Dictionary(d)) => d.clone(),
                                                    Some(Object::Reference(r)) => self
                                                        .source
                                                        .load_object(*r)
                                                        .ok()
                                                        .and_then(|o| o.as_dict().cloned())
                                                        .unwrap_or_default(),
                                                    _ => HashMap::new(),
                                                };
                                                for (name, obj_ref) in &new_xobject_refs {
                                                    xobj_entries.insert(
                                                        name.clone(),
                                                        Object::Reference(*obj_ref),
                                                    );
                                                }
                                                new_res.insert(
                                                    "XObject".to_string(),
                                                    Object::Dictionary(xobj_entries),
                                                );
                                                resources_obj = Object::Dictionary(new_res);
                                            }
                                        }

                                        let offset = writer.position();
                                        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            resources_ref.id,
                                            0,
                                            &resources_obj,
                                            &encryption_handler,
                                        )?;
                                        // ── end pdf_manipulator patch ──
                                        xref_entries.push((resources_ref.id, offset, 0, true));
                                    } else if !new_xobject_refs.is_empty() {
                                        // Resources is inline (not a reference) — new XObject refs
                                        // cannot be injected because the page dict was already written.
                                        log::warn!(
                                            "Page {} has inline Resources dict; {} new image XObject(s) \
                                             could not be added to Resources/XObject",
                                            page_index,
                                            new_xobject_refs.len(),
                                        );
                                    }

                                    // Write font objects referenced in Resources (handles inline Resources dict)
                                    if let Some(resources) = page_dict.get("Resources") {
                                        // ── pdf_manipulator patch: the prune plan's resource dictionaries win ──
                                        let resources_dict = match resources {
                                            Object::Dictionary(d) => Some(d.clone()),
                                            Object::Reference(r) => self
                                                .pruned_or_source_object(&prune, *r)
                                                .ok()
                                                .and_then(|o| o.as_dict().cloned()),
                                            _ => None,
                                        };
                                        // ── end pdf_manipulator patch ──
                                        if let Some(res_dict) = resources_dict {
                                            // Copy Font dictionary entries
                                            if let Some(fonts) = res_dict.get("Font") {
                                                // ── pdf_manipulator patch: the prune plan's font dictionary wins ──
                                                let font_dict = match fonts {
                                                    Object::Dictionary(d) => Some(d.clone()),
                                                    Object::Reference(r) => self
                                                        .pruned_or_source_object(&prune, *r)
                                                        .ok()
                                                        .and_then(|o| o.as_dict().cloned()),
                                                    _ => None,
                                                };
                                                // ── end pdf_manipulator patch ──
                                                if let Some(fdict) = font_dict {
                                                    // ── pdf_manipulator patch: key order, so the objects are written in a fixed order ──
                                                    for (_, font_ref) in DocumentEditor::sorted_entries(&fdict) {
                                                    // ── end pdf_manipulator patch ──
                                                        if let Some(ref_obj) =
                                                            font_ref.as_reference()
                                                        {
                                                            // Check if we've already written this object
                                                            if !xref_entries.contains_id(ref_obj.id) {
                                                                // Prefer a staged modification over the original source.
                                                                // ── pdf_manipulator patch: the prune plan's replacement and cuts win ──
                                                                let font_obj = self.staged_or_source_object(&prune, ref_obj);
                                                                // ── end pdf_manipulator patch ──
                                                                if let Ok(font_obj) = font_obj {
                                                                    let offset =
                                                                        writer.position();
                                                                    // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                                                    write_obj(
                                                                        &mut *writer,
                                                                        &serializer,
                                                                        ref_obj.id,
                                                                        0,
                                                                        &font_obj,
                                                                        &encryption_handler,
                                                                    )?;
                                                                    // ── end pdf_manipulator patch ──
                                                                    xref_entries.push((
                                                                        ref_obj.id, offset, 0, true,
                                                                    ));
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Copy XObject dictionary entries (images, forms, etc.)
                                            if let Some(xobjects) = res_dict.get("XObject") {
                                                let xobject_dict = match xobjects {
                                                    Object::Dictionary(d) => Some(d.clone()),
                                                    Object::Reference(r) => {
                                                        // ── pdf_manipulator patch: a save never drops an unreadable object ──
                                                        // A resource object the page references
                                                        // that cannot be read would dangle in the
                                                        // output xref; fail the save instead.
                                                        let obj = self.staged_or_source_object(&prune, *r)?;
                                                        if !xref_entries.contains_id(r.id) {
                                                            let offset = writer.position();
                                                            write_obj(
                                                                &mut *writer,
                                                                &serializer,
                                                                r.id,
                                                                0,
                                                                &obj,
                                                                &encryption_handler,
                                                            )?;
                                                            xref_entries.push((r.id, offset, 0, true));
                                                        }
                                                        obj.as_dict().cloned()
                                                        // ── end pdf_manipulator patch ──
                                                    },
                                                    _ => None,
                                                };
                                                if let Some(xobj_dict) = xobject_dict {
                                                    // ── pdf_manipulator patch: key order, so the objects are written in a fixed order ──
                                                    // A redacted page names only the source images its surviving `Do`s
                                                    // still use: its other source names left with the copies.
                                                    let kept_names = destructive_redaction
                                                        .and(self.redacted_pages.get(&source_page_index))
                                                        .map(|page| &page.scope.kept_xobject_names);
                                                    for (name, xobj_ref) in DocumentEditor::sorted_entries(&xobj_dict) {
                                                        if kept_names.is_some_and(|kept| !kept.contains(name)) {
                                                            continue;
                                                        }
                                                    // ── end pdf_manipulator patch ──
                                                        if let Some(ref_obj) =
                                                            xobj_ref.as_reference()
                                                        {
                                                            if !xref_entries.contains_id(ref_obj.id) {
                                                                // ── pdf_manipulator patch: unchanged streams copied; an unreadable object fails the save ──
                                                                if self.modified_objects.contains_key(&ref_obj.id)
                                                                    || !Self::write_source_stream(
                                                                        &self.source,
                                                                        &mut self.next_object_id,
                                                                        ref_obj,
                                                                        prune.cuts.get(&ref_obj.id).map(|cuts| cuts.as_slice()),
                                                                        writer,
                                                                        &serializer,
                                                                        &encryption_handler,
                                                                        deflate,
                                                                        &mut xref_entries,
                                                                    )?
                                                                {
                                                                    let xobj_obj =
                                                                        self.staged_or_source_object(&prune, ref_obj)?;
                                                                    let offset =
                                                                        writer.position();
                                                                    write_obj(
                                                                        &mut *writer,
                                                                        &serializer,
                                                                        ref_obj.id,
                                                                        0,
                                                                        &xobj_obj,
                                                                        &encryption_handler,
                                                                    )?;
                                                                    xref_entries.push((
                                                                        ref_obj.id, offset, 0, true,
                                                                    ));
                                                                }
                                                                // ── end pdf_manipulator patch ──
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Copy ExtGState dictionary entries
                                            if let Some(gs_obj) = res_dict.get("ExtGState") {
                                                let gs_dict = match gs_obj {
                                                    Object::Dictionary(d) => Some(d.clone()),
                                                    Object::Reference(r) => {
                                                        // ── pdf_manipulator patch: a save never drops an unreadable object ──
                                                        // A resource object the page references
                                                        // that cannot be read would dangle in the
                                                        // output xref; fail the save instead.
                                                        let obj = self.staged_or_source_object(&prune, *r)?;
                                                        if !xref_entries.contains_id(r.id) {
                                                            let offset = writer.position();
                                                            write_obj(
                                                                &mut *writer,
                                                                &serializer,
                                                                r.id,
                                                                0,
                                                                &obj,
                                                                &encryption_handler,
                                                            )?;
                                                            xref_entries.push((r.id, offset, 0, true));
                                                        }
                                                        obj.as_dict().cloned()
                                                        // ── end pdf_manipulator patch ──
                                                    },
                                                    _ => None,
                                                };
                                                if let Some(gsd) = gs_dict {
                                                    // ── pdf_manipulator patch: key order, so the objects are written in a fixed order ──
                                                    for (_, gs_ref) in DocumentEditor::sorted_entries(&gsd) {
                                                    // ── end pdf_manipulator patch ──
                                                        if let Some(ref_obj) = gs_ref.as_reference()
                                                        {
                                                            if !xref_entries.contains_id(ref_obj.id) {
                                                                // ── pdf_manipulator patch: unchanged streams copied; an unreadable object fails the save ──
                                                                if self.modified_objects.contains_key(&ref_obj.id)
                                                                    || !Self::write_source_stream(
                                                                        &self.source,
                                                                        &mut self.next_object_id,
                                                                        ref_obj,
                                                                        prune.cuts.get(&ref_obj.id).map(|cuts| cuts.as_slice()),
                                                                        writer,
                                                                        &serializer,
                                                                        &encryption_handler,
                                                                        deflate,
                                                                        &mut xref_entries,
                                                                    )?
                                                                {
                                                                    let obj =
                                                                        self.staged_or_source_object(&prune, ref_obj)?;
                                                                    let offset =
                                                                        writer.position();
                                                                    write_obj(
                                                                        &mut *writer,
                                                                        &serializer,
                                                                        ref_obj.id,
                                                                        0,
                                                                        &obj,
                                                                        &encryption_handler,
                                                                    )?;
                                                                    xref_entries.push((
                                                                        ref_obj.id, offset, 0, true,
                                                                    ));
                                                                }
                                                                // ── end pdf_manipulator patch ──
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Write erase overlay content stream if present
                                if let Some(overlay_obj_id) = erase_overlay_id {
                                    if let Some(overlay_content) =
                                        self.generate_erase_overlay(source_page_index)
                                    {
                                        // Create stream object for the overlay
                                        // ── pdf_manipulator patch: stream bodies are StreamBody ──
                                        let overlay_stream = Object::stream(
                                            HashMap::new(),
                                            overlay_content,
                                        );
                                        // ── end pdf_manipulator patch ──
                                        let offset = writer.position();
                                        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            overlay_obj_id,
                                            0,
                                            &overlay_stream,
                                            &encryption_handler,
                                        )?;
                                        // ── end pdf_manipulator patch ──
                                        xref_entries.push((overlay_obj_id, offset, 0, true));
                                    }
                                }

                                // Write overlay additions stream (add_text on existing page)
                                if let Some(additions_id) = overlay_additions_id {
                                    if let Some(added) =
                                        self.overlay_additions.get(&source_page_index)
                                    {
                                        let wrapper = StructureElement {
                                            structure_type: "Document".to_string(),
                                            bbox: crate::geometry::Rect::new(0.0, 0.0, 0.0, 0.0),
                                            children: added.clone(),
                                            reading_order: None,
                                            alt_text: None,
                                            language: None,
                                        };
                                        if let Ok((content_bytes, _pending)) =
                                            self.generate_content_stream(&wrapper)
                                        {
                                            // ── pdf_manipulator patch: stream bodies are StreamBody ──
                                            let additions_stream = Object::stream(
                                                HashMap::new(),
                                                content_bytes,
                                            );
                                            // ── end pdf_manipulator patch ──
                                            let offset = writer.position();
                                            // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                            write_obj(
                                                &mut *writer,
                                                &serializer,
                                                additions_id,
                                                0,
                                                &additions_stream,
                                                &encryption_handler,
                                            )?;
                                            // ── end pdf_manipulator patch ──
                                            xref_entries.push((additions_id, offset, 0, true));
                                        }
                                    }
                                }

                                // Write a minimal Type1 Font object for each Base-14 font
                                // referenced by overlay_additions text that wasn't already
                                // declared on this page.
                                for (name, font_id) in &overlay_font_ids {
                                    let mut font_obj_dict: HashMap<String, Object> = HashMap::new();
                                    font_obj_dict.insert(
                                        "Type".to_string(),
                                        Object::Name("Font".to_string()),
                                    );
                                    font_obj_dict.insert(
                                        "Subtype".to_string(),
                                        Object::Name("Type1".to_string()),
                                    );
                                    font_obj_dict
                                        .insert("BaseFont".to_string(), Object::Name(name.clone()));
                                    font_obj_dict.insert(
                                        "Encoding".to_string(),
                                        Object::Name("WinAnsiEncoding".to_string()),
                                    );
                                    let font_obj = Object::Dictionary(font_obj_dict);
                                    let offset = writer.position();
                                    // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                    write_obj(
                                        &mut *writer,
                                        &serializer,
                                        *font_id,
                                        0,
                                        &font_obj,
                                        &encryption_handler,
                                    )?;
                                    // ── end pdf_manipulator patch ──
                                    xref_entries.push((*font_id, offset, 0, true));
                                }

                                // Write new annotation objects
                                if !new_annotation_ids.is_empty() {
                                    // ── pdf_manipulator patch: reuse the per-save page refs ──
                                    let page_refs = &cached_page_refs;
                                    // ── end pdf_manipulator patch ──

                                    // Build the annotation dictionaries first, in
                                    // a scope that releases the `modified_annotations`
                                    // borrow before we allocate ids / mutate `self`.
                                    let built_annots: Vec<(u32, HashMap<String, Object>)> =
                                        if let Some(annotations) =
                                            self.modified_annotations.get(&source_page_index)
                                        {
                                            new_annotation_ids
                                                .iter()
                                                .zip(annotations.iter().filter(|a| a.is_new()))
                                                .filter_map(|(annot_id, annot_wrapper)| {
                                                    annot_wrapper.writer_annotation().map(|wa| {
                                                        let mut dict = wa.build(&page_refs);
                                                        // ── pdf_manipulator patch: appearance for AP-less types ──
                                                        // Types whose build() emits no /AP (stamp) get one
                                                        // from the appearance generator, as an INLINE
                                                        // stream — the hoist below lifts it to an indirect
                                                        // object like any builder-inlined appearance.
                                                        // Without /AP the annotation is viewer-synthesized
                                                        // at best and flattening finds nothing to inline.
                                                        if !dict.contains_key("AP") {
                                                            if let Some((ap_dict, ap_data)) = wa.appearance() {
                                                                let mut ap = HashMap::new();
                                                                ap.insert(
                                                                    "N".to_string(),
                                                                    Object::stream(
                                                                        ap_dict,
                                                                        bytes::Bytes::from(ap_data),
                                                                    ),
                                                                );
                                                                dict.insert(
                                                                    "AP".to_string(),
                                                                    Object::Dictionary(ap),
                                                                );
                                                            }
                                                        }
                                                        // ── end pdf_manipulator patch ──
                                                        (*annot_id, dict)
                                                    })
                                                })
                                                .collect()
                                        } else {
                                            Vec::new()
                                        };

                                    for (annot_id, mut annot_dict) in built_annots {
                                        // Hoist any inline appearance stream (e.g. a
                                        // watermark's `/AP /N`) into its own indirect
                                        // object; a stream may not be a direct dict value.
                                        let hoisted = hoist_appearance_streams(
                                            &mut annot_dict,
                                            &mut self.next_object_id,
                                        );
                                        for (stream_id, stream_obj) in hoisted {
                                            let offset = writer.position();
                                            // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                            write_obj(
                                                &mut *writer,
                                                &serializer,
                                                stream_id,
                                                0,
                                                &stream_obj,
                                                &encryption_handler,
                                            )?;
                                            xref_entries.push((stream_id, offset, 0, true));
                                        }

                                        // Write the annotation object
                                        let offset = writer.position();
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            annot_id,
                                            0,
                                            &Object::Dictionary(annot_dict),
                                            &encryption_handler,
                                        )?;
                                            // ── end pdf_manipulator patch ──
                                        xref_entries.push((annot_id, offset, 0, true));
                                    }
                                }

                                // Write new form field objects
                                if !new_form_field_ids.is_empty() {
                                    let page_ref_for_fields = ObjectRef::new(page_ref.id, 0);

                                    for (field_id, wrapper) in new_form_field_ids
                                        .iter()
                                        .zip(new_form_field_wrappers.iter())
                                    {
                                        // Build the form field dictionary
                                        let field_dict =
                                            wrapper.build_field_dict(page_ref_for_fields);

                                        // Write the form field object
                                        let offset = writer.position();
                                        // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            *field_id,
                                            0,
                                            &Object::Dictionary(field_dict),
                                            &encryption_handler,
                                        )?;
                                        // ── end pdf_manipulator patch ──
                                        xref_entries.push((*field_id, offset, 0, true));
                                    }
                                }

                                // Write flatten annotation XObjects and overlay
                                if let Some((ref appearances, overlay_id, ref xobj_ids)) =
                                    flatten_data
                                {
                                    // Write each appearance as a Form XObject
                                    for ((obj_id, _name), appearance) in
                                        xobj_ids.iter().zip(appearances.iter())
                                    {
                                        // ── pdf_manipulator patch: the appearance is written as the stream it is stored as ──
                                        let form_stream = appearance.form_object();

                                        let offset = writer.position();
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            *obj_id,
                                            0,
                                            &form_stream,
                                            &encryption_handler,
                                        )?;
                                        xref_entries.push((*obj_id, offset, 0, true));

                                        // Write any indirect objects this appearance depends
                                        // on (e.g. an embedded fallback-font graph).
                                        for (extra_id, extra_obj) in &appearance.extra_objects {
                                            let off = writer.position();
                                            write_obj(
                                                &mut *writer,
                                                &serializer,
                                                *extra_id,
                                                0,
                                                extra_obj,
                                                &encryption_handler,
                                            )?;
                                            xref_entries.push((*extra_id, off, 0, true));
                                        }
                                        // ── end pdf_manipulator patch ──
                                    }

                                    // Write the overlay content stream that invokes the XObjects
                                    let xobj_names: Vec<String> =
                                        xobj_ids.iter().map(|(_, name)| name.clone()).collect();
                                    let overlay_content =
                                        self.generate_flatten_overlay(appearances, &xobj_names);

                                    // ── pdf_manipulator patch: stream bodies are StreamBody ──
                                    let overlay_stream = Object::stream(
                                        HashMap::new(),
                                        overlay_content,
                                    );
                                    // ── end pdf_manipulator patch ──

                                    let offset = writer.position();
                                    // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                    write_obj(
                                        &mut *writer,
                                        &serializer,
                                        overlay_id,
                                        0,
                                        &overlay_stream,
                                        &encryption_handler,
                                    )?;
                                    // ── end pdf_manipulator patch ──
                                    xref_entries.push((overlay_id, offset, 0, true));
                                }

                                // ── pdf_manipulator patch: the /Redact annotations of marks not yet applied ──
                                for (annot_id, mark) in mark_annots {
                                    let offset = writer.position();
                                    write_obj(
                                        &mut *writer,
                                        &serializer,
                                        annot_id,
                                        0,
                                        &Object::Dictionary(mark),
                                        &encryption_handler,
                                    )?;
                                    xref_entries.push((annot_id, offset, 0, true));
                                }
                                // ── end pdf_manipulator patch ──

                                // ── pdf_manipulator patch: the redacted content streams from its source as the save writes it ──
                                // The page /Contents was repointed to this stream
                                // above, orphaning the original.
                                if let (Some((content_id, length_id)), Some(page)) =
                                    (destructive_redaction, self.redacted_pages.get(&source_page_index))
                                {
                                    Self::write_redacted_content(
                                        &self.source,
                                        source_page_index,
                                        page,
                                        self.image_modifications.get(&source_page_index),
                                        content_id,
                                        length_id,
                                        &mut *writer,
                                        &serializer,
                                        &encryption_handler,
                                        deflate,
                                        &mut xref_entries,
                                    )?;
                                    // ── end pdf_manipulator patch ──
                                    // ── pdf_manipulator patch: the flattened appearances a surviving `Do` still draws ──
                                    // One the redaction dropped is not written; one it replaced by a
                                    // copy is not either, but its extra objects are, since the copy
                                    // draws the fonts they hold.
                                    for set in page.flat.iter().flat_map(|flat| &flat.sets) {
                                        for ((id, name), appearance) in
                                            set.xobjects.iter().zip(&set.appearances)
                                        {
                                            let kept = page.scope.kept_xobject_names.contains(name);
                                            let copied =
                                                page.scope.forms.iter().any(|form| form.source.id == *id);
                                            if !kept && !copied {
                                                continue;
                                            }
                                            if kept {
                                                let offset = writer.position();
                                                write_obj(
                                                    &mut *writer,
                                                    &serializer,
                                                    *id,
                                                    0,
                                                    &appearance.form_object(),
                                                    &encryption_handler,
                                                )?;
                                                xref_entries.push((*id, offset, 0, true));
                                            }
                                            for (extra_id, extra_obj) in &appearance.extra_objects {
                                                let off = writer.position();
                                                write_obj(
                                                    &mut *writer,
                                                    &serializer,
                                                    *extra_id,
                                                    0,
                                                    extra_obj,
                                                    &encryption_handler,
                                                )?;
                                                xref_entries.push((*extra_id, off, 0, true));
                                            }
                                        }
                                    }
                                    // ── end pdf_manipulator patch ──
                                    // ── pdf_manipulator patch: the queued image copies, written after the page's content ──
                                    // Each copy (and, in step, the copies its /SMask or /Mask
                                    // stencil stream take) streams from the source's rows through
                                    // the same deflate → encryption → output chain the content
                                    // uses; nothing holds an encoded copy.
                                    let page_resources =
                                        Self::inherited_resources(&self.source, source_page_index)?;
                                    let doc_forms = DocForms {
                                        source: &self.source,
                                        page_resources: page_resources.clone(),
                                        flat: page.flat.as_ref(),
                                    };
                                    let mut work = CopyWork::default();
                                    Self::collect_copy_work(
                                        &self.source,
                                        &doc_forms,
                                        &page.scope,
                                        &redaction_ids,
                                        &page_resources,
                                        &mut work,
                                    )?;
                                    #[cfg(feature = "rendering")]
                                    for (placement, ids, colour_spaces) in &work.images {
                                        let source = &self.source;
                                        let source_dict = source
                                            .load_object(placement.source)?
                                            .as_dict()
                                            .cloned()
                                            .unwrap_or_default();
                                        // The mask copies' fill is the covered rect; the
                                        // colour is unused, the semantics pick the sample.
                                        let base_fills = [(placement.covered_rect(), [1.0, 1.0, 1.0])];
                                        if let (Some((sid, slid)), Some(mask_ref)) =
                                            (ids.smask, source_dict.get("SMask").and_then(|o| o.as_reference()))
                                        {
                                            // The /SMask's own copy: opaque in the covered rect,
                                            // so the fill shows.
                                            Self::write_redacted_image_copy(
                                                source,
                                                mask_ref,
                                                (sid, slid),
                                                placement.u0,
                                                placement.v0,
                                                placement.u1,
                                                placement.v1,
                                                &base_fills,
                                                &redact_copy::CopySemantics::Opaque,
                                                &colour_spaces,
                                                &HashMap::new(),
                                                cancel,
                                                &mut *writer,
                                                &serializer,
                                                &encryption_handler,
                                                &mut xref_entries,
                                            )?;
                                        }
                                        if let (Some((mid, mlid)), Some(stream_ref)) =
                                            (ids.mask, source_dict.get("Mask").and_then(|o| o.as_reference()))
                                        {
                                            // The /Mask stencil's copy: the covered rect maps
                                            // to the samples the base image shows through.
                                            Self::write_redacted_image_copy(
                                                source,
                                                stream_ref,
                                                (mid, mlid),
                                                placement.u0,
                                                placement.v0,
                                                placement.u1,
                                                placement.v1,
                                                &base_fills,
                                                &redact_copy::CopySemantics::Base,
                                                &colour_spaces,
                                                &HashMap::new(),
                                                cancel,
                                                &mut *writer,
                                                &serializer,
                                                &encryption_handler,
                                                &mut xref_entries,
                                            )?;
                                        }
                                        // The image copy itself: the covered pixels take the
                                        // regions' fill; the mask references, if the source
                                        // carried any, name the new copies.
                                        let dict_edits = {
                                            let mut edits = HashMap::new();
                                            if let Some((sid, _)) = ids.smask {
                                                edits.insert(
                                                    "SMask".to_string(),
                                                    Object::Reference(ObjectRef::new(sid, 0)),
                                                );
                                            }
                                            if let Some((mid, _)) = ids.mask {
                                                edits.insert(
                                                    "Mask".to_string(),
                                                    Object::Reference(ObjectRef::new(mid, 0)),
                                                );
                                            }
                                            edits
                                        };
                                        Self::write_redacted_image_copy(
                                            source,
                                            placement.source,
                                            ids.image,
                                            placement.u0,
                                            placement.v0,
                                            placement.u1,
                                            placement.v1,
                                            &placement.fills,
                                            &redact_copy::CopySemantics::Overlay,
                                            &colour_spaces,
                                            &dict_edits,
                                            cancel,
                                            &mut *writer,
                                            &serializer,
                                            &encryption_handler,
                                            &mut xref_entries,
                                        )?;
                                    }
                                    // The mesh, pattern and form copies, through the same chain.
                                    #[cfg(feature = "rendering")]
                                    for (mesh, ids) in &work.meshes {
                                        Self::write_mesh_copy(
                                            &self.source,
                                            mesh,
                                            *ids,
                                            &page.regions,
                                            page.opts.edge_padding,
                                            &mut *writer,
                                            &serializer,
                                            &encryption_handler,
                                            deflate,
                                            &mut xref_entries,
                                        )?;
                                    }
                                    #[cfg(feature = "rendering")]
                                    for (pattern, id, mesh_id) in &work.patterns {
                                        Self::write_pattern_copy(
                                            &self.source,
                                            pattern,
                                            (*id, *mesh_id),
                                            &mut *writer,
                                            &serializer,
                                            &encryption_handler,
                                            &mut xref_entries,
                                        )?;
                                    }
                                    for (form, ids) in &work.forms {
                                        Self::write_form_copy(
                                            &self.source,
                                            &doc_forms,
                                            form,
                                            ids,
                                            &page.regions,
                                            &page.opts,
                                            &mut *writer,
                                            &serializer,
                                            &encryption_handler,
                                            deflate,
                                            &mut xref_entries,
                                        )?;
                                    }
                                    // ── end pdf_manipulator patch ──
                                }

                                // Write form flatten XObjects and overlay if present
                                if let Some((
                                    ref form_appearances,
                                    form_overlay_id,
                                    ref form_xobj_ids,
                                )) = form_flatten_data
                                {
                                    // Write each form appearance as an XObject
                                    for ((obj_id, _), appearance) in
                                        form_xobj_ids.iter().zip(form_appearances.iter())
                                    {
                                        // ── pdf_manipulator patch: the appearance is written as the stream it is stored as ──
                                        let form_stream = appearance.form_object();

                                        let offset = writer.position();
                                        write_obj(
                                            &mut *writer,
                                            &serializer,
                                            *obj_id,
                                            0,
                                            &form_stream,
                                            &encryption_handler,
                                        )?;
                                        xref_entries.push((*obj_id, offset, 0, true));

                                        // Write any indirect objects this appearance depends
                                        // on (e.g. an embedded fallback-font graph).
                                        for (extra_id, extra_obj) in &appearance.extra_objects {
                                            let off = writer.position();
                                            write_obj(
                                                &mut *writer,
                                                &serializer,
                                                *extra_id,
                                                0,
                                                extra_obj,
                                                &encryption_handler,
                                            )?;
                                            xref_entries.push((*extra_id, off, 0, true));
                                        }
                                        // ── end pdf_manipulator patch ──
                                    }

                                    // Write the overlay content stream that invokes the XObjects
                                    let xobj_names: Vec<String> = form_xobj_ids
                                        .iter()
                                        .map(|(_, name)| name.clone())
                                        .collect();
                                    let overlay_content = self
                                        .generate_flatten_overlay(form_appearances, &xobj_names);

                                    // ── pdf_manipulator patch: stream bodies are StreamBody ──
                                    let overlay_stream = Object::stream(
                                        HashMap::new(),
                                        overlay_content,
                                    );
                                    // ── end pdf_manipulator patch ──

                                    let offset = writer.position();
                                    // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                                    write_obj(
                                        &mut *writer,
                                        &serializer,
                                        form_overlay_id,
                                        0,
                                        &overlay_stream,
                                        &encryption_handler,
                                    )?;
                                    // ── end pdf_manipulator patch ──
                                    xref_entries.push((form_overlay_id, offset, 0, true));
                                }

                                page_index += 1;
                            }
                        }
            }
        }

        // ── pdf_manipulator patch: merged objects stream from the merged documents, one at a time ──
        // Write merged pages and their dependent objects
        let output_pages_ref = catalog_obj
            .as_dict()
            .and_then(|d| d.get("Pages"))
            .and_then(|p| p.as_reference());
        let mut merged_index = 0;
        for source in &self.merged_sources {
            for page in &source.pages {
                // A page that left the page order is not written; its
                // objects are, when a kept page reaches them.
                if merged_kept[merged_index] {
                    // The page dictionary is read again here, so the merge kept
                    // no copy of it. Its /Parent was never imported.
                    // The resource prune's cuts apply to it.
                    let mut page_obj = Self::merged_page_object(source, page.page_ref)?;
                    if let Some(cuts) = merged_cuts.get(&page.page_id) {
                        page_obj = crate::host::resource_prune::apply(page_obj, cuts);
                    }
                    if let Object::Dictionary(dict) = &mut page_obj {
                        Self::remap_dict_refs(dict, &source.id_map, page.page_id);
                    }
                    if let Some(props) = self.merged_page_props.get(&merged_index) {
                        page_obj = self.apply_page_props_to_object(&page_obj, props)?;
                    }
                    if let (Object::Dictionary(dict), Some(pages_ref)) = (&mut page_obj, output_pages_ref) {
                        dict.insert("Parent".to_string(), Object::Reference(pages_ref));
                    }

                    let offset = writer.position();
                    write_obj(
                        &mut *writer,
                        &serializer,
                        page.page_id,
                        0,
                        &page_obj,
                        &encryption_handler,
                    )?;
                    xref_entries.push((page.page_id, offset, 0, true));
                    if let Some(copy_ids) = copies_of.get(&page.page_id) {
                        page_copies.push((copy_ids.clone(), page_obj));
                    }
                }
                merged_index += 1;

                for &(new_id, src_id) in &source.objects[page.objects.clone()] {
                    cancel.check()?;
                    if xref_entries.contains_id(new_id) || merged_left_out.contains(&new_id) {
                        continue;
                    }
                    if packer.is_full() {
                        Self::write_object_streams(
                            &packer,
                            &mut self.next_object_id,
                            writer,
                            &serializer,
                            &encryption_handler,
                            deflate,
                            &mut xref_entries,
                            false,
                        )?;
                    }
                    // The resource prune's cuts apply to the object.
                    let cuts = merged_cuts.get(&new_id).map(|cuts| cuts.as_slice());
                    if Self::write_merged_stream(
                        &source.doc,
                        &mut self.next_object_id,
                        (new_id, src_id),
                        (&source.id_map, page.page_id),
                        cuts,
                        writer,
                        &serializer,
                        &encryption_handler,
                        deflate,
                        &mut xref_entries,
                    )? {
                        continue;
                    }
                    let src_ref = ObjectRef::new(src_id, 0);
                    let mut obj = source.doc.load_object(src_ref)?;
                    if let Some(cuts) = cuts {
                        obj = crate::host::resource_prune::apply(obj, cuts);
                    }
                    Self::remap_refs(&mut obj, &source.id_map, page.page_id);
                    let offset = writer.position();
                    write_obj(&mut *writer, &serializer, new_id, 0, &obj, &encryption_handler)?;
                    xref_entries.push((new_id, offset, 0, true));
                }
                // Each object was read once; the cache would only sit full.
                source.doc.clear_object_cache();
            }
        }
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: a repeated page is a new page object sharing the page's content ──
        // /Contents and /Resources stay shared by reference. /Annots is
        // dropped: an annotation names its page in /P and a widget belongs to
        // one form field, so a second page cannot list the same ones.
        for (copy_ids, page_obj) in page_copies {
            cancel.check()?;
            let Object::Dictionary(mut dict) = page_obj else { continue };
            dict.remove("Annots");
            if let Some(pages_ref) = output_pages_ref {
                dict.insert("Parent".to_string(), Object::Reference(pages_ref));
            }
            let copy = Object::Dictionary(dict);
            for copy_id in copy_ids {
                let offset = writer.position();
                write_obj(&mut *writer, &serializer, copy_id, 0, &copy, &encryption_handler)?;
                xref_entries.push((copy_id, offset, 0, true));
            }
        }
        // ── end pdf_manipulator patch ──

        // Write parent-only form fields (non-terminal fields with no widget)
        // These don't belong to any specific page, so write them after page processing
        for (_, field_id, wrapper, _) in &all_form_field_data {
            if wrapper.is_parent_only() {
                // Build parent field dictionary (no widget entries)
                let field_dict = wrapper.build_parent_dict();

                // Write the parent field object
                let offset = writer.position();
                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                write_obj(
                    &mut *writer,
                    &serializer,
                    *field_id,
                    0,
                    &Object::Dictionary(field_dict),
                    &encryption_handler,
                )?;
                // ── end pdf_manipulator patch ──
                xref_entries.push((*field_id, offset, 0, true));
            }
        }

        // Write info dictionary if modified
        // ── pdf_manipulator patch: the new /Info takes the source /Info's id ──
        // Unmodified, the source /Info is written by the sweep under its own
        // id and the trailer names it below; modified, the new dictionary
        // replaces it at that id, so no old copy is left beside it.
        let source_info_id = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Info"))
            .and_then(|r| r.as_reference())
            .map(|r| r.id);
        // ── end pdf_manipulator patch ──
        let info_ref = if self.modified_info.is_some() {
            let info = self.modified_info.clone().unwrap();
            // ── pdf_manipulator patch: the new /Info takes the source /Info's id ──
            let info_id = match source_info_id.filter(|id| !xref_entries.contains_id(*id)) {
                Some(id) => id,
                None => self.allocate_object_id(),
            };
            // ── end pdf_manipulator patch ──
            let info_obj = info.to_object();
            let offset = writer.position();
            // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
            write_obj(&mut *writer, &serializer, info_id, 0, &info_obj, &encryption_handler)?;
            // ── end pdf_manipulator patch ──
            xref_entries.push((info_id, offset, 0, true));
            Some(ObjectRef::new(info_id, 0))
        } else {
            None
        };

        // ── Remaining-objects sweep ────────────────────────────────────────────
        //
        // The page-tree traversal above is deliberately shallow: it handles the
        // catalog, pages tree, page dicts, content streams, and the first level
        // of font/XObject resource references.  For documents built by
        // `DocumentBuilder` with embedded TrueType fonts, the full object graph
        // is deeper:
        //
        //   Type0 font dict  →  DescendantFonts  →  CIDFontType2 dict
        //                                         →  FontDescriptor dict
        //                                            → FontFile2 stream
        //                    →  ToUnicode CMap stream
        //
        // Any of these that were not reached above would produce dangling
        // cross-reference entries in the output, making the PDF unrenderable
        // even though it is structurally valid.  (Issue #401.)
        //
        // Solution: after the main traversal, enumerate every object ID in the
        // source document's xref table and write any that were not yet written.
        // This is safe — `xref_entries` dedups in O(1), so already-written
        // objects are skipped; orphaned objects are harmless in a PDF reader.
        //
        // When garbage_collect=true, only reachable objects are written.
        // When compress=true, raw (unfiltered) streams are FlateDecode-compressed.

        // ── pdf_manipulator patch: GC walks the pruned page tree (#261) ──
        // The walk never enters a dropped page, an inner /Pages node or a
        // dropped page's own annotation, and reads the plan's overrides,
        // so nothing that only a dropped page reaches is written.
        let reachable_ids = if options.garbage_collect {
            Some(self.collect_reachable_ids(
                &prune,
                Some((&xref_entries, &serializer.written_references())),
                &aliases,
                cancel,
            ))
        } else {
            None
        };
        // Without GC the sweep writes every source object, so the objects
        // only a dropped page reaches are named and skipped instead.
        let dropped_only = if options.garbage_collect {
            std::collections::HashSet::new()
        } else {
            self.dropped_only_ids(&prune)
        };
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: the reachability an original image a redaction replaced is judged by ──
        // With garbage collection on the sweep's own reachable set answers;
        // with it off the same walk runs once, over what the output already
        // names, so a copy's original appears exactly when a kept page still
        // quotes it.
        let redact_reach = if !prune.redacted_image_originals.is_empty() && !options.garbage_collect {
            Some(self.collect_reachable_ids(
                &prune,
                Some((&xref_entries, &serializer.written_references())),
                &aliases,
                cancel,
            ))
        } else {
            None
        };
        // ── end pdf_manipulator patch ──

        let all_source_ids = self.source.all_object_ids();
        // The walk above ends early on a cancel; what it found is not a set to write by.
        cancel.check()?;
        for obj_id in all_source_ids.iter().copied() {
            // ── pdf_manipulator patch: a cancel stops the write before the next object ──
            cancel.check()?;
            // ── end pdf_manipulator patch ──
            if obj_id == 0 || xref_entries.contains_id(obj_id) {
                continue;
            }
            // ── pdf_manipulator patch: full batches of queued objects become object streams ──
            if packer.is_full() {
                Self::write_object_streams(
                    &packer,
                    &mut self.next_object_id,
                    writer,
                    &serializer,
                    &encryption_handler,
                    deflate,
                    &mut xref_entries,
                    false,
                )?;
            }
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: free entries skipped, unreadable objects refused ──
            // Free xref entries hold no object; `all_object_ids` unions the
            // whole table, free slots included, so skip them before loading
            // instead of treating their load failure as a damaged file.
            if self.source.is_free_entry(obj_id) {
                continue;
            }
            // ── end pdf_manipulator patch ──
            // #231 G6: never emit the original content stream of a
            // destructively-redacted page (it holds the secret bytes),
            // regardless of GC reachability.
            if self.redacted_orphan_ids.contains(&obj_id) {
                continue;
            }
            // ── pdf_manipulator patch: dropped pages never written (#261) ──
            if prune.excluded.contains(&obj_id) || dropped_only.contains(&obj_id) {
                continue;
            }
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: an original image a redaction replaced is written only when something else still references it ──
            // Without garbage collection the walk's answer is taken by name.
            // With it there is no walk to take, so the reachable-set check
            // below decides alone.
            if prune.redacted_image_originals.contains(&obj_id) {
                let referenced_elsewhere = redact_reach
                    .as_ref()
                    .is_none_or(|interesting| interesting.contains(&obj_id));
                if !referenced_elsewhere {
                    continue;
                }
            }
            // ── end pdf_manipulator patch ──
            if let Some(ref reachable) = reachable_ids {
                if !reachable.contains(&obj_id) {
                    continue;
                }
            }
            // ── pdf_manipulator patch: an unchanged source stream the plan does not replace is copied, not loaded ──
            if !self.modified_objects.contains_key(&obj_id)
                && !prune.overrides.contains_key(&obj_id)
                && Self::write_source_stream(
                    &self.source,
                    &mut self.next_object_id,
                    ObjectRef { id: obj_id, gen: 0 },
                    prune.cuts.get(&obj_id).map(|cuts| cuts.as_slice()),
                    writer,
                    &serializer,
                    &encryption_handler,
                    deflate,
                    &mut xref_entries,
                )?
            {
                continue;
            }
            // ── end pdf_manipulator patch ──
            // Prefer a staged modification over the original source object.
            // ── pdf_manipulator patch: the prune plan's override wins (#261) ──
            let loaded = if let Some(o) = prune.overrides.get(&obj_id) {
                Ok(o.clone())
            } else if let Some(m) = self.modified_objects.get(&obj_id) {
            // ── end pdf_manipulator patch ──
                Ok(m.clone())
            } else {
                self.load_source_object(ObjectRef { id: obj_id, gen: 0 })
            };
            // The resource prune's cuts apply to what is found.
            let loaded = loaded.map(|object| prune.cut(obj_id, object));
            match loaded {
                // ── pdf_manipulator patch: source containers are never copied (#261) ──
                // The rewrite writes each object on its own and a fresh xref
                // table, so the source's object streams and xref streams are
                // dead weight, and an object stream still holds every object
                // packed in it: a dropped page's dictionary, a field's value.
                Ok(Object::Stream { ref dict, .. })
                    if matches!(
                        dict.get("Type").and_then(|t| t.as_name()),
                        Some("ObjStm") | Some("XRef")
                    ) => {},
                // ── end pdf_manipulator patch ──
                Ok(obj) => {
                    // ── pdf_manipulator patch: a staged stream over MEMORY_STREAM is counted, then recoded while streaming ──
                    if let (Some(level), Object::Stream { dict, body }) = (deflate, &obj) {
                        if body.decrypted_len()? > crate::host::deflate::MEMORY_STREAM {
                            Self::write_stream_body(
                                obj_id,
                                dict.clone(),
                                body,
                                Some(level),
                                &mut self.next_object_id,
                                writer,
                                &serializer,
                                &encryption_handler,
                                &mut xref_entries,
                            )?;
                            continue;
                        }
                    }
                    // ── end pdf_manipulator patch ──
                    let obj = match deflate {
                        Some(level) => deflate_stream(obj, level),
                        None => obj,
                    };
                    let offset = writer.position();
                    // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                    write_obj(&mut *writer, &serializer, obj_id, 0, &obj, &encryption_handler)?;
                    // ── end pdf_manipulator patch ──
                    xref_entries.push((obj_id, offset, 0, true));
                },
                // ── pdf_manipulator patch: an unreadable in-use object fails the save ──
                Err(e) => return Err(e),
                // ── end pdf_manipulator patch ──
            }
        }




        // Write any new objects from modified_objects whose IDs are not in the
        // original source xref (e.g. XMP streams allocated by the PDF/A converter).
        {
            let mut new_objs: Vec<(u32, Object)> = self
                .modified_objects
                .iter()
                .filter(|(&id, _)| !xref_entries.contains_id(id))
                // ── pdf_manipulator patch: a staged source object the sweep found unreachable stays out ──
                // A flattened widget is staged by the form flush (its new /V) yet
                // no page or field names it any more; writing it here would name
                // its /AP stream, which the sweep above did not write.
                .filter(|(&id, _)| {
                    reachable_ids.as_ref().is_none_or(|reachable| {
                        reachable.contains(&id) || all_source_ids.binary_search(&id).is_err()
                    })
                })
                // ── end pdf_manipulator patch ──
                // ── pdf_manipulator patch: dropped pages never written (#261) ──
                .filter(|(&id, _)| !prune.excluded.contains(&id) && !dropped_only.contains(&id))
                // ── end pdf_manipulator patch ──
                .map(|(&id, obj)| (id, obj.clone()))
                .collect();
            // ── pdf_manipulator patch: id order, a HashMap's order differs per instance ──
            new_objs.sort_by_key(|(id, _)| *id);
            // ── end pdf_manipulator patch ──
            for (obj_id, obj) in new_objs {
                // ── pdf_manipulator patch: a cancel stops the write before the next object ──
                cancel.check()?;
                // ── end pdf_manipulator patch ──
                // ── pdf_manipulator patch: full batches of queued objects become object streams ──
                if packer.is_full() {
                    Self::write_object_streams(
                        &packer,
                        &mut self.next_object_id,
                        writer,
                        &serializer,
                        &encryption_handler,
                        deflate,
                        &mut xref_entries,
                        false,
                    )?;
                }
                // ── end pdf_manipulator patch ──
                let offset = writer.position();
                // ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
                write_obj(&mut *writer, &serializer, obj_id, 0, &obj, &encryption_handler)?;
                // ── end pdf_manipulator patch ──
                xref_entries.push((obj_id, offset, 0, true));
            }
        }

        // ── pdf_manipulator patch: the last queued objects become an object stream ──
        // Every object counts as written once it is in `xref_entries`, queued
        // or not, so the closure check below sees them either way.
        Self::write_object_streams(
            &packer,
            &mut self.next_object_id,
            writer,
            &serializer,
            &encryption_handler,
            deflate,
            &mut xref_entries,
            true,
        )?;
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: the written objects are closed under reference ──
        if let Some((target, writer_id)) =
            self.first_unwritten_reference(&serializer.written_references(), &xref_entries)
        {
            return Err(Error::InvalidPdf(format!(
                "the rewrite would leave object {} referenced by {} unwritten",
                target, writer_id
            )));
        }
        // ── end pdf_manipulator patch ──

        // ── pdf_manipulator patch: the trailer keeps the source's /Info and /ID ──
        // The unmodified /Info is named only when the save wrote it (a
        // sanitize or garbage collection can leave it out). /ID is carried
        // unchanged unless the save sets its own with encryption.
        let info_ref = info_ref.or_else(|| {
            source_info_id
                .filter(|id| xref_entries.contains_id(*id))
                .map(|id| ObjectRef::new(id, 0))
        });
        let source_file_id: Option<Object> = if file_id.is_none() {
            self.source
                .trailer()
                .as_dict()
                .and_then(|d| d.get("ID"))
                .filter(|id| matches!(id, Object::Array(_)))
                .cloned()
        } else {
            None
        };
        // ── end pdf_manipulator patch ──

        // Sort xref entries by object ID
        xref_entries.sort_by_key(|(id, _, _, _)| *id);

        // ── pdf_manipulator patch: the cross-reference section is a stream (ISO 32000-1 §7.5.8) ──
        // Objects in an object stream are type 2 rows; the stream itself is
        // the last object and is not encrypted (§7.6.1).
        if options.object_streams {
            use crate::host::object_streams::{field_width, xref_stream, Row, XrefRows};

            xref_entries.release_ids();
            let max_id = xref_entries.iter().map(|(id, _, _, _)| *id).max().unwrap_or(0);
            let stream_id = max_id + 1;
            let xref_offset = writer.position();
            let rows = XrefRows::new(&xref_entries, stream_id, xref_offset);
            let widest = rows
                .clone()
                .map(|row| match row {
                    Row::Offset { offset, .. } => offset,
                    Row::Packed { stream, .. } => stream as u64,
                    Row::Free { .. } => 0,
                })
                .max()
                .unwrap_or(0);

            let mut trailer: HashMap<String, Object> = HashMap::new();
            trailer.insert("Size".to_string(), Object::Integer(stream_id as i64 + 1));
            trailer.insert(
                "Root".to_string(),
                Object::Reference(ObjectRef::new(catalog_ref.id, 0)),
            );
            if let Some(info_ref) = info_ref {
                trailer.insert("Info".to_string(), Object::Reference(info_ref));
            }
            if let Some(enc_id) = encrypt_obj_id {
                trailer.insert("Encrypt".to_string(), Object::Reference(ObjectRef::new(enc_id, 0)));
            }
            if let Some((id1, id2)) = file_id {
                trailer.insert(
                    "ID".to_string(),
                    Object::Array(vec![Object::String(id1), Object::String(id2)]),
                );
            } else if let Some(id) = source_file_id {
                trailer.insert("ID".to_string(), id);
            }
            let stream = xref_stream(rows, field_width(widest), deflate, trailer, cancel)?;
            writer.write_all(&serializer.serialize_indirect(stream_id, 0, &stream))?;
            write!(writer, "startxref\n{}\n%%EOF\n", xref_offset)?;

            writer.flush()?;
            self.is_modified = false;
            return Ok(());
        }
        // ── end pdf_manipulator patch ──

        // Write xref table
        let xref_offset = writer.position();
        write!(writer, "xref\n")?;

        // Find max object ID
        let max_id = xref_entries
            .iter()
            .map(|(id, _, _, _)| *id)
            .max()
            .unwrap_or(0);
        write!(writer, "0 {}\n", max_id + 1)?;

        // Write entries (fill gaps with free entries)
        let mut entry_map: HashMap<u32, (u64, u16, bool)> = xref_entries
            .into_entries()
            .into_iter()
            .map(|(id, off, gen, used)| (id, (off, gen, used)))
            .collect();

        for id in 0..=max_id {
            if let Some((offset, gen, in_use)) = entry_map.get(&id) {
                if *in_use {
                    write!(writer, "{:010} {:05} n \n", offset, gen)?;
                } else {
                    write!(writer, "{:010} {:05} f \n", offset, gen)?;
                }
            } else {
                // Free entry pointing to object 0
                write!(writer, "0000000000 65535 f \n")?;
            }
        }

        // Write trailer
        write!(writer, "trailer\n")?;
        write!(writer, "<<\n")?;
        write!(writer, "  /Size {}\n", max_id + 1)?;
        write!(writer, "  /Root {} 0 R\n", catalog_ref.id)?;

        if let Some(info_ref) = info_ref {
            write!(writer, "  /Info {} {} R\n", info_ref.id, info_ref.gen)?;
        }

        // Write encryption entries if encrypting
        if let Some(enc_id) = encrypt_obj_id {
            write!(writer, "  /Encrypt {} 0 R\n", enc_id)?;
        }

        // Write file ID if encryption is enabled
        if let Some((id1, id2)) = file_id {
            let id1_hex: String = id1.iter().map(|b| format!("{:02X}", b)).collect();
            let id2_hex: String = id2.iter().map(|b| format!("{:02X}", b)).collect();
            write!(writer, "  /ID [<{}> <{}>]\n", id1_hex, id2_hex)?;
        // ── pdf_manipulator patch: the source's /ID is carried unchanged ──
        } else if let Some(id) = source_file_id {
            write!(writer, "  /ID ")?;
            writer.write_all(&serializer.serialize(&id))?;
            write!(writer, "\n")?;
        // ── end pdf_manipulator patch ──
        }

        write!(writer, ">>\n")?;
        write!(writer, "startxref\n")?;
        write!(writer, "{}\n", xref_offset)?;
        write!(writer, "%%EOF\n")?;

        writer.flush()?;
        self.is_modified = false;


        Ok(())
    }

    // === Content modification operations ===

    /// Extract hierarchical content from a page.
    ///
    /// Returns the page's hierarchical content structure with all children populated.
    /// For untagged PDFs, returns a synthetic hierarchy based on geometric analysis.
    ///
    /// # Arguments
    ///
    /// * `page_index` - The page to extract from (0-indexed)
    ///
    /// # Returns
    ///
    /// `Ok(Some(structure))` if structure is found or generated,
    /// `Ok(None)` if no structure is available,
    /// `Err` if an error occurs during extraction
    pub fn get_page_content(&mut self, page_index: usize) -> Result<Option<StructureElement>> {
        HierarchicalExtractor::extract_page(&self.source, page_index)
    }

    /// Replace the content of a page with a new structure.
    ///
    /// Marks the document as modified and sets the structure_modified flag
    /// so the structure tree will be rebuilt on save.
    ///
    /// # Arguments
    ///
    /// * `page_index` - The page to modify (0-indexed)
    /// * `content` - The new hierarchical structure for the page
    ///
    /// # Returns
    ///
    /// `Err` if the page index is out of range
    pub fn set_page_content(&mut self, page_index: usize, content: StructureElement) -> Result<()> {
        let page_count = self.current_page_count();
        if page_index >= page_count {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of range (document has {} pages)",
                page_index, page_count
            )));
        }

        let source_index = self.source_page_for(page_index, "setting page content")?;
        self.modified_content.insert(source_index, content);
        self.structure_modified = true;
        self.is_modified = true;
        Ok(())
    }

    /// Modify a page's structure in-place using a closure.
    ///
    /// Extracts the current content, passes it to the closure for modification,
    /// then saves it back.
    ///
    /// # Arguments
    ///
    /// * `page_index` - The page to modify
    /// * `f` - Closure that modifies the structure
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.modify_structure(0, |structure| {
    ///     // Modify structure in place
    ///     structure.alt_text = Some("Modified alt text".to_string());
    ///     Ok(())
    /// })?;
    /// ```
    pub fn modify_structure<F>(&mut self, page_index: usize, f: F) -> Result<()>
    where
        F: FnOnce(&mut StructureElement) -> Result<()>,
    {
        let mut content = self
            .get_page_content(page_index)?
            .ok_or_else(|| Error::InvalidPdf("No structure available for page".to_string()))?;

        f(&mut content)?;
        self.set_page_content(page_index, content)
    }

    /// Get the resource manager for allocating fonts, images, etc.
    ///
    /// Use this when manually constructing content elements that need resources.
    pub fn resource_manager_mut(&mut self) -> &mut ResourceManager {
        &mut self.resource_manager
    }

    /// Get a reference to the resource manager.
    pub fn resource_manager(&self) -> &ResourceManager {
        &self.resource_manager
    }

    /// Get a page for DOM-like editing.
    ///
    /// Returns a PdfPage that allows hierarchical navigation and querying
    /// of page content with a DOM-like API.
    pub fn get_page(&mut self, page_index: usize) -> Result<crate::editor::dom::PdfPage> {
        // ── pdf_manipulator patch: a merged page is refused before anything reads it as a source page ──
        self.source_page_for(page_index, "editing page content")?;
        // ── end pdf_manipulator patch ──
        // Get the page info first
        let page_info = self.get_page_info(page_index)?;

        // Get or extract the page content
        let content = if let Some(structure) = self.get_page_content(page_index)? {
            structure
        } else {
            // If no modified content, try to extract from original
            match HierarchicalExtractor::extract_page(&self.source, page_index)? {
                Some(structure) => structure,
                None => {
                    // Create empty structure if extraction fails
                    StructureElement {
                        structure_type: "Document".to_string(),
                        bbox: crate::geometry::Rect::new(
                            0.0,
                            0.0,
                            page_info.width,
                            page_info.height,
                        ),
                        children: Vec::new(),
                        reading_order: Some(0),
                        alt_text: None,
                        language: None,
                    }
                },
            }
        };

        // ── pdf_manipulator patch: pending annotations are the truth ──
        // Load annotations: pending edits first. Once any annotation
        // edit has touched this page, `modified_annotations` IS the
        // page's current truth (it was seeded from source at that
        // moment) — re-seeding from source here would hand the caller
        // a stale set, and the eventual save_page would REPLACE the
        // pending entry with it, silently erasing earlier edits
        // (e.g. a stamp wiping out a watermark added before it).
        let source_page = self.source_page_for(page_index, "editing page content")?;
        let annotations: Vec<crate::editor::dom::AnnotationWrapper> =
            if let Some(pending) = self.modified_annotations.get(&source_page) {
                pending.clone()
            } else {
                self.source
                    .get_annotations(page_index)
                    .unwrap_or_default()
                    .into_iter()
                    .map(crate::editor::dom::AnnotationWrapper::from_read)
                    .collect()
            };
        // ── end pdf_manipulator patch ──

        Ok(crate::editor::dom::PdfPage::from_structure_with_annotations(
            page_index,
            content,
            page_info.width,
            page_info.height,
            annotations,
        ))
    }

    /// Save a modified page back to the document.
    ///
    /// This saves both the page content and any modified annotations.
    pub fn save_page(&mut self, page: crate::editor::dom::PdfPage) -> Result<()> {
        let page_index = self.source_page_for(page.page_index, "saving page content")?;
        let annotations_modified = page.has_annotations_modified();

        // Extract annotations before moving root
        let annotations: Vec<crate::editor::dom::AnnotationWrapper> = if annotations_modified {
            page.annotations().to_vec()
        } else {
            Vec::new()
        };

        if page.is_loaded_from_source() {
            // Page was loaded from an existing PDF.  Use overlay approach: preserve the
            // original content stream and append only the newly added elements as a second
            // stream.  This avoids losing graphics/form-structure that the text-only
            // HierarchicalExtractor cannot round-trip.
            //
            // Always mark as modified so callers that check `is_modified()` after
            // `save_page()` continue to see the expected true value.
            self.is_modified = true;
            let added: Vec<ContentElement> = page.added_children().to_vec();
            // `save_page()` can legitimately be called more than once for the same
            // page (e.g. one add_text() + save_page_from_editor() per inserted
            // element). `.insert()` here replaced any elements staged by a prior
            // call instead of accumulating them, silently dropping all but the
            // last call's additions. Extend the existing Vec instead.
            if !added.is_empty() {
                self.overlay_additions
                    .entry(page_index)
                    .or_default()
                    .extend(added);
            }

            // Pre-existing content that was edited/removed via the DOM can't be
            // captured by the overlay above (it was never "added" — see
            // `PdfPage::pending_erasures`), so destructively redact it from the
            // real content stream instead of silently dropping the edit.
            let erasures = page.pending_erasures().to_vec();
            if !erasures.is_empty() {
                // ── pdf_manipulator patch: a DOM erasure is applied directly, not queued as a mark ──
                let regions: Vec<crate::redaction::RedactionRegion> = erasures
                    .iter()
                    .map(|bbox| {
                        crate::redaction::RedactionRegion::from_rect(
                            bbox.x,
                            bbox.y,
                            bbox.x + bbox.width,
                            bbox.y + bbox.height,
                            None,
                        )
                    })
                    .collect();
                self.apply_redactions_destructive_for_page(
                    page_index,
                    &crate::redaction::RedactionOptions::default(),
                    &regions,
                )?;
                // ── end pdf_manipulator patch ──
            }
        } else {
            // Freshly created page — replace the entire content stream.
            self.set_page_content(page_index, page.root)?;
        }

        // Save annotations if they were modified
        if annotations_modified {
            self.modified_annotations.insert(page_index, annotations);
            self.is_modified = true;
        }

        Ok(())
    }

    /// Get the modified annotations for a page (if any).
    pub fn get_page_annotations(
        &self,
        page_index: usize,
    ) -> Option<&Vec<crate::editor::dom::AnnotationWrapper>> {
        self.modified_annotations.get(&page_index)
    }

    /// Check if a page has modified annotations.
    pub fn has_modified_annotations(&self, page_index: usize) -> bool {
        self.modified_annotations.contains_key(&page_index)
    }

    /// Append an annotation to a page without parsing page content.
    // ── pdf_manipulator patch: fast annotation append + bulk media boxes ──
    /// Avoids the HierarchicalExtractor cost of get_page().
    pub fn add_page_annotation<A: Into<crate::writer::Annotation>>(
        &mut self,
        page_index: usize,
        annotation: A,
    ) -> Result<()> {
        let source_page = self.source_page_for(page_index, "adding an annotation")?;
        let wrapper = crate::editor::dom::AnnotationWrapper::from_write(annotation);
        self.modified_annotations
            .entry(source_page)
            .or_insert_with(|| {
                self.source.get_annotations(source_page)
                    .unwrap_or_default()
                    .into_iter()
                    .map(crate::editor::dom::AnnotationWrapper::from_read)
                    .collect()
            })
            .push(wrapper);
        self.is_modified = true;
        Ok(())
    }

    /// Edit a page with a closure, automatically saving changes.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.edit_page(0, |page| {
    ///     let text_elements = page.find_text_containing("Hello");
    ///     for text in text_elements {
    ///         page.set_text(text.id(), "Hi")?;
    ///     }
    ///     Ok(())
    /// })?;
    /// ```
    pub fn edit_page<F>(&mut self, page_index: usize, f: F) -> Result<()>
    where
        F: FnOnce(&mut crate::editor::dom::PdfPage) -> Result<()>,
    {
        let mut page = self.get_page(page_index)?;
        f(&mut page)?;
        self.save_page(page)
    }

    /// Get a page editor for fluent/XMLDocument-style editing.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.page_editor(0)?
    ///    .find_text_containing("Hello")?
    ///    .for_each(|mut text| {
    ///        text.set_text("Hi");
    ///        Ok(())
    ///    })?
    ///    .done()?;
    /// editor.save_page_editor_modified()?;
    /// ```
    pub fn page_editor(&mut self, page_index: usize) -> Result<crate::editor::dom::PageEditor> {
        let page = self.get_page(page_index)?;
        Ok(crate::editor::dom::PageEditor { page })
    }

    /// Save a page from the fluent editor back to the document.
    pub fn save_page_from_editor(&mut self, page: crate::editor::dom::PdfPage) -> Result<()> {
        self.save_page(page)
    }

    // =========================================================================
    // Page Properties: Rotation, Cropping
    // =========================================================================

    /// Get the rotation of a page in degrees (0, 90, 180, 270).
    ///
    /// Returns the effective rotation, considering any modifications.
    pub fn get_page_rotation(&mut self, index: usize) -> Result<i32> {
        // merged pages read their own document
        let source_index = match self.page_key(index) {
            PageKey::Source(source_index) => source_index,
            PageKey::Merged(merged) => {
                if let Some(rotation) = self.merged_page_props.get(&merged).and_then(|p| p.rotation) {
                    return Ok(rotation);
                }
                let dict = self.merged_page_dict(merged)?;
                return Ok(dict
                    .get("Rotate")
                    .and_then(|r| r.as_integer())
                    .map(|r| (r % 360 + 360) as i32 % 360)
                    .unwrap_or(0));
            },
        };
        // Check if we have a modified rotation
        if let Some(props) = self.modified_page_props.get(&source_index) {
            if let Some(rotation) = props.rotation {
                return Ok(rotation);
            }
        }

        // Otherwise get from original document
        let info = self.get_page_info(index)?;
        Ok(info.rotation)
    }

    /// Set the rotation of a page.
    ///
    /// Rotation must be 0, 90, 180, or 270 degrees.
    pub fn set_page_rotation(&mut self, index: usize, degrees: i32) -> Result<()> {
        // Validate rotation
        if ![0, 90, 180, 270].contains(&degrees) {
            return Err(Error::InvalidPdf(
                "Rotation must be 0, 90, 180, or 270 degrees".to_string(),
            ));
        }

        // Validate page index
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of range (document has {} pages)",
                index,
                self.current_page_count()
            )));
        }

        // Store the modified rotation keyed by source index so serialization can find it.
        // merged pages keep theirs by merged index
        let props = self.page_props_mut(index);
        props.rotation = Some(degrees);

        self.is_modified = true;
        Ok(())
    }

    /// Rotate a page by the given degrees (adds to current rotation).
    ///
    /// The result is normalized to 0, 90, 180, or 270.
    pub fn rotate_page_by(&mut self, index: usize, degrees: i32) -> Result<()> {
        let current = self.get_page_rotation(index)?;
        let new_rotation = ((current + degrees) % 360 + 360) % 360;

        // Normalize to valid PDF rotation
        let normalized = match new_rotation {
            0..=44 => 0,
            45..=134 => 90,
            135..=224 => 180,
            225..=314 => 270,
            _ => 0,
        };

        self.set_page_rotation(index, normalized)
    }

    /// Rotate all pages by the given degrees.
    pub fn rotate_all_pages(&mut self, degrees: i32) -> Result<()> {
        let count = self.current_page_count();
        for i in 0..count {
            self.rotate_page_by(i, degrees)?;
        }
        Ok(())
    }

    /// Get the MediaBox of a page (physical page size).
    ///
    /// Returns [llx, lly, urx, ury] (lower-left x, lower-left y, upper-right x, upper-right y).
    pub fn get_page_media_box(&mut self, index: usize) -> Result<[f32; 4]> {
        // merged pages read their own document
        let source_index = match self.page_key(index) {
            PageKey::Source(source_index) => source_index,
            PageKey::Merged(merged) => {
                if let Some(media_box) = self.merged_page_props.get(&merged).and_then(|p| p.media_box) {
                    return Ok(media_box);
                }
                let dict = self.merged_page_dict(merged)?;
                return Ok(Self::box_of(&dict, "MediaBox").unwrap_or([0.0, 0.0, 612.0, 792.0]));
            },
        };
        // Check if we have a modified MediaBox
        if let Some(props) = self.modified_page_props.get(&source_index) {
            if let Some(media_box) = props.media_box {
                return Ok(media_box);
            }
        }

        // Get from original document
        let page_refs = self.get_page_refs()?;
        if source_index >= page_refs.len() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", index)));
        }

        let page_ref = page_refs[source_index];
        let page_obj = self.source.load_object(page_ref)?;
        let page_dict = page_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Page is not a dictionary".to_string()))?;

        if let Some(media_box) = page_dict.get("MediaBox").and_then(|m| m.as_array()) {
            if media_box.len() >= 4 {
                let llx = media_box[0]
                    .as_real()
                    .or_else(|| media_box[0].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0) as f32;
                let lly = media_box[1]
                    .as_real()
                    .or_else(|| media_box[1].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0) as f32;
                let urx = media_box[2]
                    .as_real()
                    .or_else(|| media_box[2].as_integer().map(|i| i as f64))
                    .unwrap_or(612.0) as f32;
                let ury = media_box[3]
                    .as_real()
                    .or_else(|| media_box[3].as_integer().map(|i| i as f64))
                    .unwrap_or(792.0) as f32;
                return Ok([llx, lly, urx, ury]);
            }
        }

        // Default to Letter size
        Ok([0.0, 0.0, 612.0, 792.0])
    }

    /// Fetch all page media boxes in one pass. Calls get_page_refs() once
    /// instead of once per page, avoiding O(N²) for bulk operations.
    pub fn all_media_boxes(&mut self) -> Vec<[f32; 4]> {
        let count = self.current_page_count();
        let page_refs = self.get_page_refs().unwrap_or_default();
        let mut result = Vec::with_capacity(count);
        for i in 0..count {
            // merged pages read their own document
            let source_index = match self.page_key(i) {
                PageKey::Source(source_index) => source_index,
                PageKey::Merged(_) => {
                    result.push(self.get_page_media_box(i).unwrap_or([0.0, 0.0, 612.0, 792.0]));
                    continue;
                },
            };
            if let Some(props) = self.modified_page_props.get(&source_index) {
                if let Some(media_box) = props.media_box {
                    result.push(media_box);
                    continue;
                }
            }
            if source_index < page_refs.len() {
                if let Ok(page_obj) = self.source.load_object(page_refs[source_index]) {
                    if let Some(dict) = page_obj.as_dict() {
                        if let Some(mb) = dict.get("MediaBox").and_then(|m| m.as_array()) {
                            if mb.len() >= 4 {
                                let llx = mb[0].as_real().or_else(|| mb[0].as_integer().map(|i| i as f64)).unwrap_or(0.0) as f32;
                                let lly = mb[1].as_real().or_else(|| mb[1].as_integer().map(|i| i as f64)).unwrap_or(0.0) as f32;
                                let urx = mb[2].as_real().or_else(|| mb[2].as_integer().map(|i| i as f64)).unwrap_or(612.0) as f32;
                                let ury = mb[3].as_real().or_else(|| mb[3].as_integer().map(|i| i as f64)).unwrap_or(792.0) as f32;
                                result.push([llx, lly, urx, ury]);
                                continue;
                            }
                        }
                    }
                }
            }
            result.push([0.0, 0.0, 612.0, 792.0]);
        }
        result
    }
    // ── end pdf_manipulator patch ──

    /// Set the MediaBox of a page.
    pub fn set_page_media_box(&mut self, index: usize, box_: [f32; 4]) -> Result<()> {
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", index)));
        }

        let props = self.page_props_mut(index);
        props.media_box = Some(box_);

        self.is_modified = true;
        Ok(())
    }

    /// Get the CropBox of a page (visible/printable area).
    ///
    /// Returns None if no CropBox is set (defaults to MediaBox).
    pub fn get_page_crop_box(&mut self, index: usize) -> Result<Option<[f32; 4]>> {
        // ── pdf_manipulator patch: merged pages read their own document ──
        let source_index = match self.page_key(index) {
            PageKey::Source(source_index) => source_index,
            PageKey::Merged(merged) => {
                if let Some(props) = self.merged_page_props.get(&merged) {
                    if let Some(crop_box) = props.crop_box {
                        return Ok(Some(crop_box));
                    }
                    if props.crop_box_removed {
                        return Ok(None);
                    }
                }
                return Ok(Self::box_of(&self.merged_page_dict(merged)?, "CropBox"));
            },
        };
        // ── end pdf_manipulator patch ──
        // Check if we have a modified CropBox
        if let Some(props) = self.modified_page_props.get(&source_index) {
            if let Some(crop_box) = props.crop_box {
                return Ok(Some(crop_box));
            }
            // ── pdf_manipulator patch: a page's own /CropBox can be removed ──
            if props.crop_box_removed {
                return Ok(None);
            }
            // ── end pdf_manipulator patch ──
        }

        // Get from original document
        let page_refs = self.get_page_refs()?;
        if source_index >= page_refs.len() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", index)));
        }

        let page_ref = page_refs[source_index];
        let page_obj = self.source.load_object(page_ref)?;
        let page_dict = page_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Page is not a dictionary".to_string()))?;

        if let Some(crop_box) = page_dict.get("CropBox").and_then(|c| c.as_array()) {
            if crop_box.len() >= 4 {
                let llx = crop_box[0]
                    .as_real()
                    .or_else(|| crop_box[0].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0) as f32;
                let lly = crop_box[1]
                    .as_real()
                    .or_else(|| crop_box[1].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0) as f32;
                let urx = crop_box[2]
                    .as_real()
                    .or_else(|| crop_box[2].as_integer().map(|i| i as f64))
                    .unwrap_or(612.0) as f32;
                let ury = crop_box[3]
                    .as_real()
                    .or_else(|| crop_box[3].as_integer().map(|i| i as f64))
                    .unwrap_or(792.0) as f32;
                return Ok(Some([llx, lly, urx, ury]));
            }
        }

        Ok(None)
    }

    /// Set the CropBox of a page.
    pub fn set_page_crop_box(&mut self, index: usize, box_: [f32; 4]) -> Result<()> {
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", index)));
        }

        let props = self.page_props_mut(index);
        props.crop_box = Some(box_);
        // ── pdf_manipulator patch: a page's own /CropBox can be removed ──
        props.crop_box_removed = false;
        // ── end pdf_manipulator patch ──

        self.is_modified = true;
        Ok(())
    }

    // ── pdf_manipulator patch: a page's own /CropBox can be removed ──
    /// Drops the page's own `/CropBox`, so the page falls back to its
    /// media box.
    pub fn remove_page_crop_box(&mut self, index: usize) -> Result<()> {
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", index)));
        }

        let props = self.page_props_mut(index);
        props.crop_box = None;
        props.crop_box_removed = true;

        self.is_modified = true;
        Ok(())
    }
    // ── end pdf_manipulator patch ──

    /// Crop margins from all pages.
    ///
    /// This sets the CropBox to be smaller than the MediaBox by the specified margins.
    pub fn crop_margins(&mut self, left: f32, right: f32, top: f32, bottom: f32) -> Result<()> {
        let count = self.current_page_count();
        for i in 0..count {
            let media_box = self.get_page_media_box(i)?;
            let crop_box = [
                media_box[0] + left,
                media_box[1] + bottom,
                media_box[2] - right,
                media_box[3] - top,
            ];
            self.set_page_crop_box(i, crop_box)?;
        }
        Ok(())
    }

    // =========================================================================
    // Content Erasing (Whiteout)
    // =========================================================================

    /// Erase a rectangular region on a page by covering it with white.
    ///
    /// This adds a white rectangle overlay that covers the specified region.
    /// The original content is not removed but hidden beneath the white overlay.
    ///
    /// # Arguments
    ///
    /// * `page` - Page index (0-based)
    /// * `rect` - Rectangle to erase [llx, lly, urx, ury]
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Erase a region in the upper-left corner
    /// editor.erase_region(0, [72.0, 700.0, 200.0, 792.0])?;
    /// editor.save("output.pdf")?;
    /// ```
    pub fn erase_region(&mut self, page: usize, rect: [f32; 4]) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "erasing a region")?;
        let regions = self.erase_regions.entry(source_page).or_default();
        regions.push(rect);

        self.is_modified = true;
        Ok(())
    }

    /// Erase multiple rectangular regions on a page.
    pub fn erase_regions(&mut self, page: usize, rects: &[[f32; 4]]) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "erasing regions")?;
        let regions = self.erase_regions.entry(source_page).or_default();
        regions.extend_from_slice(rects);

        self.is_modified = true;
        Ok(())
    }

    /// Clear all pending erase operations for a page.
    pub fn clear_erase_regions(&mut self, page: usize) {
        if let PageKey::Source(source_page) = self.page_key(page) {
            self.erase_regions.remove(&source_page);
        }
    }

    /// Generate the content stream for erase overlays.
    ///
    /// Returns PDF operators that draw white rectangles over the specified regions.
    fn generate_erase_overlay(&self, page: usize) -> Option<Vec<u8>> {
        let regions = self.erase_regions.get(&page)?;
        if regions.is_empty() {
            return None;
        }

        let mut content = Vec::new();

        // Save graphics state
        content.extend_from_slice(b"q\n");

        // Set fill color to white (RGB 1 1 1)
        content.extend_from_slice(b"1 1 1 rg\n");

        // Draw each rectangle
        for rect in regions {
            let x = rect[0];
            let y = rect[1];
            let width = rect[2] - rect[0];
            let height = rect[3] - rect[1];

            // Rectangle path and fill
            content.extend_from_slice(
                format!("{:.2} {:.2} {:.2} {:.2} re f\n", x, y, width, height).as_bytes(),
            );
        }

        // Restore graphics state
        content.extend_from_slice(b"Q\n");

        Some(content)
    }

    // ========================================================================
    // Annotation Flattening
    // ========================================================================

    /// Mark annotations on a page for flattening.
    ///
    /// When the document is saved, annotations on this page will be rendered
    /// into the page content and removed from the annotations array.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Flatten annotations on page 0
    /// editor.flatten_page_annotations(0)?;
    /// editor.save("output.pdf")?;
    /// ```
    pub fn flatten_page_annotations(&mut self, page: usize) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "flattening annotations")?;
        self.flatten_annotations_pages.insert(source_page);
        self.is_modified = true;
        Ok(())
    }

    /// Mark all pages for annotation flattening.
    ///
    /// When the document is saved, all annotations will be rendered
    /// into the page content and removed.
    pub fn flatten_all_annotations(&mut self) -> Result<()> {
        let page_count = self.current_page_count();
        // ── pdf_manipulator patch: every page, named by the source page it shows ──
        // The set holds source page indices, so a document with pages removed
        // or reordered must not insert the output indices.
        let mut keys = Vec::with_capacity(page_count);
        for page in 0..page_count {
            crate::cancel::poll()?;
            keys.push(match self.page_key(page) {
                PageKey::Source(source_page) => source_page,
                PageKey::Merged(_) => page,
            });
        }
        self.flatten_annotations_pages.extend(keys);
        // ── end pdf_manipulator patch ──
        self.is_modified = true;
        Ok(())
    }

    /// Check if a page has annotations marked for flattening.
    pub fn is_page_marked_for_flatten(&self, page: usize) -> bool {
        matches!(self.page_key(page), PageKey::Source(source_page)
            if self.flatten_annotations_pages.contains(&source_page))
    }

    /// Clear the flatten annotation flag for a page.
    pub fn unmark_page_for_flatten(&mut self, page: usize) {
        if let PageKey::Source(source_page) = self.page_key(page) {
            self.flatten_annotations_pages.remove(&source_page);
        }
    }

    // ========================================================================
    // Form Flattening
    // ========================================================================

    /// Mark form fields on a specific page for flattening.
    ///
    /// When the document is saved, form fields (Widget annotations) on this page
    /// will be rendered into the page content. Only Widget annotations are flattened,
    /// other annotation types are preserved.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Flatten forms on page 0
    /// editor.flatten_forms_on_page(0)?;
    /// editor.save("flattened.pdf")?;
    /// ```
    pub fn flatten_forms_on_page(&mut self, page: usize) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "flattening forms")?;
        self.flatten_forms_pages.insert(source_page);
        self.is_modified = true;
        Ok(())
    }

    /// Mark all pages for form field flattening.
    ///
    /// When the document is saved, all form fields will be rendered into the page
    /// content and the AcroForm dictionary will be removed from the catalog.
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.flatten_forms()?;
    /// editor.save("flattened.pdf")?;
    /// ```
    pub fn flatten_forms(&mut self) -> Result<()> {
        let page_count = self.current_page_count();
        // ── pdf_manipulator patch: every page, named by the source page it shows ──
        let mut keys = Vec::with_capacity(page_count);
        for page in 0..page_count {
            crate::cancel::poll()?;
            keys.push(match self.page_key(page) {
                PageKey::Source(source_page) => source_page,
                PageKey::Merged(_) => page,
            });
        }
        self.flatten_forms_pages.extend(keys);
        // ── end pdf_manipulator patch ──
        self.remove_acroform = true;
        self.is_modified = true;
        Ok(())
    }

    /// Check if a page has form fields marked for flattening.
    pub fn is_page_marked_for_form_flatten(&self, page: usize) -> bool {
        matches!(self.page_key(page), PageKey::Source(source_page)
            if self.flatten_forms_pages.contains(&source_page))
    }

    /// Check if AcroForm will be removed on save.
    pub fn will_remove_acroform(&self) -> bool {
        self.remove_acroform
    }

    /// Warnings collected during the last form-flattening save.
    ///
    /// Each entry names a widget field that had no `/AP` appearance stream and
    /// could not have one generated — flattening it produces a blank rectangle.
    pub fn flatten_warnings(&self) -> &[String] {
        &self.flatten_warnings
    }

    /// Rebuild an AcroForm dict containing only root fields that still have at
    /// least one widget on a non-flattened page.
    ///
    /// Returns `None` if the original catalog has no AcroForm, or if the AcroForm
    /// contains `/XFA` (in which case we leave it untouched and emit a warning).
    fn rebuild_partial_acroform(
        &mut self,
        catalog_dict: &HashMap<String, Object>,
    ) -> Result<Option<Object>> {
        let acroform_obj = match catalog_dict.get("AcroForm") {
            Some(o) => o.clone(),
            None => return Ok(None),
        };

        let acroform_dict: HashMap<String, Object> = match acroform_obj {
            Object::Dictionary(d) => d,
            Object::Reference(r) => match self.source.load_object(r)? {
                Object::Dictionary(d) => d,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        // XFA forms use a completely different field-addressing model; rebuilding
        // the /Fields array would break SOM expressions inside the XFA stream.
        // Leave the AcroForm as-is and warn the caller (ISO 32000-1 §12.7.8).
        if acroform_dict.contains_key("XFA") {
            self.flatten_warnings.push(
                "XFA form detected — AcroForm not rebuilt after partial flatten; \
                 XFA SOM expressions may reference flattened widgets"
                    .to_string(),
            );
            return Ok(None);
        }

        let fields_array = match acroform_dict.get("Fields") {
            Some(Object::Array(arr)) => arr.clone(),
            // ── pdf_manipulator patch: /Fields may be an indirect reference ──
            // ISO 32000-1 §12.7.3 — the AcroForm field array can be reached
            // through an indirect reference. The array-only match treated that
            // as "no fields" and skipped the rebuild, dropping every field after
            // a partial flatten. Resolve the reference form too.
            Some(Object::Reference(r)) => match self.source.load_object(*r) {
                Ok(Object::Array(arr)) => arr,
                _ => return Ok(None),
            },
            // ── end pdf_manipulator patch ──
            _ => return Ok(None),
        };

        // Build page-ref → page-index map in one tree walk
        // (per-index get_page_ref is O(n), looping it is O(n²)).
        let page_ref_to_index: HashMap<u32, usize> = self
            .source
            .all_page_refs()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.id, i))
            .collect();

        // Collect root field refs that survive the partial flatten
        let flattened = self.flatten_forms_pages.clone();
        let mut surviving: Vec<Object> = Vec::new();
        for field_entry in &fields_array {
            if let Object::Reference(field_ref) = field_entry {
                if self.field_has_surviving_widgets(field_ref, &flattened, &page_ref_to_index)? {
                    surviving.push(Object::Reference(*field_ref));
                }
            }
        }

        // Preserve all original top-level AcroForm keys; replace only /Fields.
        let mut new_acroform = acroform_dict.clone();
        new_acroform.insert("Fields".to_string(), Object::Array(surviving));
        Ok(Some(Object::Dictionary(new_acroform)))
    }

    // ── pdf_manipulator patch: `remove_form_field` must reach the saved file ──
    // `deleted_form_fields` was consulted only by the in-memory query helpers
    // (`has_form_field`, `get_form_fields`, `modify_form_field`); the save
    // writer never dropped the field from `/AcroForm /Fields` nor its widget
    // from the owning page's `/Annots`, so a "removed" field reappeared on
    // reopen. Filters both, modelled on `rebuild_partial_acroform` above.
    /// Rebuilds the AcroForm's `/Fields` array to drop every field named in
    /// `self.deleted_form_fields`, and stages each owning page's `/Annots`
    /// with that field's widget removed. Returns `None` when there is
    /// nothing to remove (unchanged catalog).
    fn remove_deleted_form_fields_from_acroform(
        &mut self,
        catalog_dict: &HashMap<String, Object>,
    ) -> Result<Option<Object>> {
        use crate::extractors::forms::FormExtractor;

        if self.deleted_form_fields.is_empty() {
            return Ok(None);
        }

        let acroform_obj = match catalog_dict.get("AcroForm") {
            Some(o) => o.clone(),
            None => return Ok(None),
        };
        let acroform_dict: HashMap<String, Object> = match acroform_obj {
            Object::Dictionary(d) => d,
            Object::Reference(r) => match self.source.load_object(r)? {
                Object::Dictionary(d) => d,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let fields_array = match acroform_dict.get("Fields") {
            Some(Object::Array(arr)) => arr.clone(),
            Some(Object::Reference(r)) => match self.source.load_object(*r) {
                Ok(Object::Array(arr)) => arr,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        // The object id of every deleted field — `remove_form_field` names the
        // field the caller asked for, which is the terminal/root field itself.
        let deleted_ids: HashSet<u32> = FormExtractor::extract_fields(&self.source)?
            .into_iter()
            .filter(|f| self.deleted_form_fields.contains(&f.full_name))
            .filter_map(|f| f.object_ref.map(|r| r.id))
            .collect();
        if deleted_ids.is_empty() {
            return Ok(None);
        }

        let surviving: Vec<Object> = fields_array
            .into_iter()
            .filter(|entry| entry.as_reference().map(|r| !deleted_ids.contains(&r.id)).unwrap_or(true))
            .collect();
        let mut new_acroform = acroform_dict.clone();
        new_acroform.insert("Fields".to_string(), Object::Array(surviving));

        // Strip the deleted widgets from every page's /Annots (a merged
        // field/widget appears there directly; the id set covers that case).
        for page_ref in self.source.all_page_refs().unwrap_or_default() {
            let page_obj = match self.source.load_object(page_ref) {
                Ok(o) => o,
                Err(_) => continue,
            };
            let page_dict = match page_obj.as_dict() {
                Some(d) => d.clone(),
                None => continue,
            };
            let annots = match page_dict.get("Annots") {
                Some(Object::Array(arr)) => arr.clone(),
                Some(Object::Reference(r)) => match self.source.load_object(*r) {
                    Ok(Object::Array(arr)) => arr,
                    _ => continue,
                },
                _ => continue,
            };
            let filtered: Vec<Object> = annots
                .iter()
                .filter(|a| a.as_reference().map(|r| !deleted_ids.contains(&r.id)).unwrap_or(true))
                .cloned()
                .collect();
            if filtered.len() != annots.len() {
                let mut new_page = page_dict;
                new_page.insert("Annots".to_string(), Object::Array(filtered));
                self.modified_objects.insert(page_ref.id, Object::Dictionary(new_page));
            }
        }

        Ok(Some(Object::Dictionary(new_acroform)))
    }
    // ── end pdf_manipulator patch ──

    /// Returns true if `field_ref` or any of its descendants has a widget whose
    /// `/P` page reference maps to a page that was NOT flattened.
    fn field_has_surviving_widgets(
        &mut self,
        field_ref: &ObjectRef,
        flattened_pages: &HashSet<usize>,
        page_ref_to_index: &HashMap<u32, usize>,
    ) -> Result<bool> {
        let field_obj = match self.source.load_object(*field_ref) {
            Ok(o) => o,
            Err(_) => return Ok(true), // can't load → keep to be safe
        };
        let dict = match field_obj.as_dict() {
            Some(d) => d.clone(),
            None => return Ok(true),
        };

        if let Some(Object::Array(kids)) = dict.get("Kids").cloned() {
            // Non-terminal field — recurse into kids
            for kid in &kids {
                if let Object::Reference(kid_ref) = kid {
                    if self.field_has_surviving_widgets(
                        kid_ref,
                        flattened_pages,
                        page_ref_to_index,
                    )? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        } else {
            // Terminal field / merged field+widget — check /P (page ref)
            match dict.get("P") {
                Some(Object::Reference(page_ref)) => {
                    match page_ref_to_index.get(&page_ref.id) {
                        Some(idx) => Ok(!flattened_pages.contains(idx)),
                        // Page ref not in map — keep the field (unknown page)
                        None => Ok(true),
                    }
                },
                // No /P — keep the field
                _ => Ok(true),
            }
        }
    }

    // =========================================================================
    // File Attachments (Embedded Files)
    // =========================================================================

    /// Embed a file in the document.
    ///
    /// The file will be added to the document's EmbeddedFiles name tree
    /// when the document is saved.
    ///
    /// # Arguments
    ///
    /// * `name` - The file name (used as identifier and display name)
    /// * `data` - The file contents
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("input.pdf")?;
    /// editor.embed_file("data.csv", csv_bytes)?;
    /// editor.save("output.pdf")?;
    /// ```
    pub fn embed_file(&mut self, name: &str, data: Vec<u8>) -> Result<()> {
        let file = crate::writer::EmbeddedFile::new(name, data);
        self.embedded_files.push(file);
        self.is_modified = true;
        Ok(())
    }

    /// Embed a file with additional metadata.
    ///
    /// # Arguments
    ///
    /// * `file` - The embedded file configuration
    pub fn embed_file_with_options(&mut self, file: crate::writer::EmbeddedFile) -> Result<()> {
        self.embedded_files.push(file);
        self.is_modified = true;
        Ok(())
    }

    /// Get the list of files that will be embedded on save.
    pub fn pending_embedded_files(&self) -> &[crate::writer::EmbeddedFile] {
        &self.embedded_files
    }

    /// Clear all pending embedded files.
    pub fn clear_embedded_files(&mut self) {
        self.embedded_files.clear();
    }

    // =========================================================================
    // XFA Forms Support
    // =========================================================================

    /// Check if this document contains XFA forms.
    ///
    /// XFA (XML Forms Architecture) is an XML-based form specification used
    /// in some PDFs, particularly government and financial forms.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    /// if editor.has_xfa()? {
    ///     println!("Document contains XFA forms");
    /// }
    /// ```
    pub fn has_xfa(&mut self) -> Result<bool> {
        crate::xfa::XfaExtractor::has_xfa(&mut self.source)
    }

    /// Analyze XFA forms in this document without converting.
    ///
    /// Returns information about the XFA form structure including
    /// field count, page count, and field types.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    /// let analysis = editor.analyze_xfa()?;
    ///
    /// if analysis.has_xfa {
    ///     println!("Found {} fields across {} pages",
    ///         analysis.field_count.unwrap_or(0),
    ///         analysis.page_count.unwrap_or(0));
    /// }
    /// ```
    pub fn analyze_xfa(&mut self) -> Result<crate::xfa::XfaAnalysis> {
        crate::xfa::analyze_xfa_document(&mut self.source)
    }

    /// Convert XFA forms to AcroForm and return new PDF bytes.
    ///
    /// This creates a new PDF document with the XFA forms converted to
    /// standard AcroForm fields. The original document is not modified.
    ///
    /// # Limitations
    ///
    /// This implementation supports **static conversion only**:
    /// - Extracts field definitions and current values
    /// - Converts fields to equivalent AcroForm types
    /// - Uses simple vertical stacking layout
    ///
    /// **NOT supported:**
    /// - Dynamic XFA features (scripts, calculations, conditional logic)
    /// - Complex layouts (tables, grids, repeating sections)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("xfa_form.pdf")?;
    /// if editor.has_xfa()? {
    ///     let acroform_bytes = editor.convert_xfa_to_acroform(None)?;
    ///     std::fs::write("converted.pdf", acroform_bytes)?;
    /// }
    /// ```
    pub fn convert_xfa_to_acroform(
        &mut self,
        options: Option<crate::xfa::XfaConversionOptions>,
    ) -> Result<Vec<u8>> {
        crate::xfa::convert_xfa_document(&mut self.source, options)
    }

    // =========================================================================
    // Form Field Editing
    // =========================================================================

    /// Get all form fields from the document.
    ///
    /// Returns form fields from the document's AcroForm, including any modifications
    /// made during this editing session. Deleted fields are not included.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    /// let fields = editor.get_form_fields()?;
    ///
    /// for field in &fields {
    ///     println!("{}: {:?}", field.name(), field.value());
    /// }
    /// ```
    pub fn get_form_fields(&mut self) -> Result<Vec<FormFieldWrapper>> {
        use crate::extractors::forms::FormExtractor;

        // Extract fields from source document
        let source_fields = FormExtractor::extract_fields(&self.source)?;

        // Build page ref -> index map for resolving field page indices in
        // one tree walk (per-index get_page_ref is O(n), looping is O(n²)).
        let page_ref_map: HashMap<u32, usize> = self
            .source
            .all_page_refs()
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.id, i))
            .collect();

        let mut result = Vec::new();

        // Add original fields (wrapped), excluding deleted ones
        for field in source_fields {
            let full_name = field.full_name.clone();

            // Skip if deleted
            if self.deleted_form_fields.contains(&full_name) {
                continue;
            }

            // Check if we have a modified version
            if let Some(wrapper) = self.modified_form_fields.get(&full_name) {
                result.push(wrapper.clone());
            } else {
                // Determine page index from widget annotation's /P entry
                let page_index = field
                    .object_ref
                    .and_then(|obj_ref| self.source.load_object(obj_ref).ok())
                    .and_then(|obj| obj.as_dict().cloned())
                    .and_then(|dict| {
                        // Check /P on the field dict first (merged field+widget)
                        if let Some(page_ref) = dict.get("P").and_then(|p| p.as_reference()) {
                            return Some(page_ref);
                        }
                        // If no /P, follow /Kids to the first widget annotation
                        dict.get("Kids")
                            .and_then(|k| match k {
                                Object::Array(arr) => Some(arr.clone()),
                                Object::Reference(r) => self
                                    .source
                                    .load_object(*r)
                                    .ok()
                                    .and_then(|o| o.as_array().cloned()),
                                _ => None,
                            })
                            .and_then(|kids| kids.first().cloned())
                            .and_then(|kid_ref| {
                                kid_ref
                                    .as_reference()
                                    .and_then(|r| self.source.load_object(r).ok())
                            })
                            .and_then(|kid_obj| kid_obj.as_dict().cloned())
                            .and_then(|kid_dict| kid_dict.get("P").and_then(|p| p.as_reference()))
                    })
                    .and_then(|page_ref| page_ref_map.get(&page_ref.id).copied())
                    .unwrap_or(0);
                result.push(FormFieldWrapper::from_read(field, page_index, None));
            }
        }

        // Add new fields (not from original document)
        for (name, wrapper) in &self.modified_form_fields {
            if wrapper.is_new() && !self.deleted_form_fields.contains(name) {
                result.push(wrapper.clone());
            }
        }

        Ok(result)
    }

    /// Get the value of a specific form field by name.
    ///
    /// Returns the current value of the field, which may be the original value
    /// or a modified value if `set_form_field_value()` was called.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field (e.g., "form.section.field")
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    ///
    /// if let Some(value) = editor.get_form_field_value("email")? {
    ///     println!("Email: {:?}", value);
    /// }
    /// ```
    pub fn get_form_field_value(
        &mut self,
        name: &str,
    ) -> Result<Option<crate::editor::form_fields::FormFieldValue>> {
        use crate::editor::form_fields::FormFieldValue;
        use crate::extractors::forms::FormExtractor;

        // Check if deleted
        if self.deleted_form_fields.contains(name) {
            return Ok(None);
        }

        // Check modified fields first
        if let Some(wrapper) = self.modified_form_fields.get(name) {
            return Ok(Some(wrapper.value()));
        }

        // Look up in original document
        let source_fields = FormExtractor::extract_fields(&self.source)?;

        for field in source_fields {
            if field.full_name == name {
                return Ok(Some(FormFieldValue::from(&field.value)));
            }
        }

        Ok(None)
    }

    /// Check if a form field with the given name exists.
    ///
    /// Returns true if the field exists in the original document or was added
    /// during this editing session, and has not been deleted.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    ///
    /// if editor.has_form_field("email")? {
    ///     println!("Email field exists");
    /// }
    /// ```
    pub fn has_form_field(&mut self, name: &str) -> Result<bool> {
        use crate::extractors::forms::FormExtractor;

        // Check if deleted
        if self.deleted_form_fields.contains(name) {
            return Ok(false);
        }

        // Check modified fields (includes new fields)
        if self.modified_form_fields.contains_key(name) {
            return Ok(true);
        }

        // Look up in original document
        let source_fields = FormExtractor::extract_fields(&self.source)?;

        for field in source_fields {
            if field.full_name == name {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Add a new form field to a page.
    ///
    /// Creates a new form field and widget annotation on the specified page.
    /// The field will be added to the document's AcroForm on save.
    ///
    /// # Arguments
    ///
    /// * `page` - The page index (0-based) where the field should appear
    /// * `widget` - A form field widget implementing `FormFieldWidget`
    ///
    /// # Returns
    ///
    /// The full qualified name of the added field, which may be modified if
    /// a field with the same name already exists.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    /// use pdf_oxide::writer::form_fields::TextFieldWidget;
    /// use pdf_oxide::geometry::Rect;
    ///
    /// let mut editor = DocumentEditor::open("document.pdf")?;
    ///
    /// // Add a text field to page 0
    /// let name = editor.add_form_field(0,
    ///     TextFieldWidget::new("email", Rect::new(100.0, 700.0, 200.0, 20.0))
    ///         .with_value("user@example.com")
    /// )?;
    ///
    /// println!("Added field: {}", name);
    /// editor.save("output.pdf")?;
    /// ```
    pub fn add_form_field<W: crate::writer::form_fields::FormFieldWidget>(
        &mut self,
        page: usize,
        widget: W,
    ) -> Result<String> {
        // Validate page index
        let page_count = self.page_count()?;
        if page >= page_count {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of bounds (document has {} pages)",
                page, page_count
            )));
        }

        // Make name unique if it already exists
        let mut name = widget.field_name().to_string();
        let mut counter = 1;
        while self.has_form_field(&name)? {
            name = format!("{}_{}", widget.field_name(), counter);
            counter += 1;
        }

        // Create wrapper from widget
        let mut wrapper = FormFieldWrapper::from_widget(&widget, page);

        // Override name if it was modified for uniqueness
        if name != widget.field_name() {
            wrapper.name = name.clone();
        }

        // Mark document as modified
        self.is_modified = true;
        self.acroform_modified = true;

        // Store in modified fields
        self.modified_form_fields.insert(name.clone(), wrapper);

        Ok(name)
    }

    /// Add a parent container field for hierarchical form fields.
    ///
    /// Parent fields are non-terminal fields that don't have a widget annotation
    /// but contain child fields via the `/Kids` array. They can be used to:
    /// - Group related fields (e.g., `address.street`, `address.city`)
    /// - Inherit properties to children (flags, field type, default appearance)
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the parent field
    ///
    /// # Returns
    ///
    /// The full qualified name of the parent field.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::{DocumentEditor, ParentFieldConfig};
    ///
    /// let mut editor = DocumentEditor::open("document.pdf")?;
    ///
    /// // Create a parent field
    /// editor.add_parent_field(ParentFieldConfig::new("address"))?;
    ///
    /// // Add children under the parent
    /// editor.add_child_field("address", 0, TextFieldWidget::new("street", rect))?;
    /// editor.add_child_field("address", 0, TextFieldWidget::new("city", rect2))?;
    ///
    /// editor.save("output.pdf")?;
    /// ```
    pub fn add_parent_field(
        &mut self,
        config: crate::editor::form_fields::ParentFieldConfig,
    ) -> Result<String> {
        let name = config.full_name();

        // Check if parent already exists
        if self.has_form_field(&name)? {
            return Err(Error::InvalidPdf(format!("Parent field already exists: {}", name)));
        }

        // If this parent has a parent, verify it exists
        if let Some(ref parent_name) = config.parent_name {
            if !self.has_form_field(parent_name)? {
                return Err(Error::InvalidPdf(format!("Parent field not found: {}", parent_name)));
            }
        }

        // Create wrapper from config
        let wrapper = FormFieldWrapper::from_parent_config(&config);

        // Mark document as modified
        self.is_modified = true;
        self.acroform_modified = true;

        // Store in modified fields
        self.modified_form_fields.insert(name.clone(), wrapper);

        Ok(name)
    }

    /// Add a form field as a child of an existing parent field.
    ///
    /// Creates a hierarchical relationship where the child field's partial name
    /// becomes the full name: `parent_name.widget_name`.
    ///
    /// # Arguments
    ///
    /// * `parent_name` - Name of the existing parent field
    /// * `page` - Page index where the widget appears (0-based)
    /// * `widget` - The form field widget to add
    ///
    /// # Returns
    ///
    /// The full qualified name of the child field.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::{DocumentEditor, ParentFieldConfig};
    /// use pdf_oxide::writer::form_fields::TextFieldWidget;
    /// use pdf_oxide::geometry::Rect;
    ///
    /// let mut editor = DocumentEditor::open("document.pdf")?;
    ///
    /// // Create parent first
    /// editor.add_parent_field(ParentFieldConfig::new("contact"))?;
    ///
    /// // Add children
    /// let name = editor.add_child_field("contact", 0,
    ///     TextFieldWidget::new("email", Rect::new(100.0, 700.0, 200.0, 20.0))
    /// )?;
    /// assert_eq!(name, "contact.email");
    ///
    /// editor.save("output.pdf")?;
    /// ```
    pub fn add_child_field<W: crate::writer::form_fields::FormFieldWidget>(
        &mut self,
        parent_name: &str,
        page: usize,
        widget: W,
    ) -> Result<String> {
        // Validate page index
        let page_count = self.page_count()?;
        if page >= page_count {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of bounds (document has {} pages)",
                page, page_count
            )));
        }

        // Verify parent exists
        if !self.has_form_field(parent_name)? {
            return Err(Error::InvalidPdf(format!("Parent field not found: {}", parent_name)));
        }

        // Create wrapper with parent reference
        let wrapper = FormFieldWrapper::from_widget_with_parent(&widget, page, parent_name);
        let name = wrapper.name.clone();

        // Check for duplicate name
        if self.has_form_field(&name)? {
            return Err(Error::InvalidPdf(format!("Child field already exists: {}", name)));
        }

        // Mark document as modified
        self.is_modified = true;
        self.acroform_modified = true;

        // Store in modified fields
        self.modified_form_fields.insert(name.clone(), wrapper);

        Ok(name)
    }

    /// Add a form field with automatic hierarchical parent creation.
    ///
    /// If the widget name contains dots (e.g., "address.street"), this method
    /// automatically creates any missing parent fields. This provides a convenient
    /// way to create hierarchical forms without manually managing parents.
    ///
    /// # Arguments
    ///
    /// * `page` - Page index where the widget appears (0-based)
    /// * `widget` - The form field widget to add
    ///
    /// # Returns
    ///
    /// The full qualified name of the added field.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    /// use pdf_oxide::writer::form_fields::TextFieldWidget;
    /// use pdf_oxide::geometry::Rect;
    ///
    /// let mut editor = DocumentEditor::open("document.pdf")?;
    ///
    /// // Automatically creates "address" parent if needed
    /// editor.add_form_field_hierarchical(0,
    ///     TextFieldWidget::new("address.street", Rect::new(100.0, 700.0, 200.0, 20.0))
    /// )?;
    ///
    /// // Reuses existing "address" parent
    /// editor.add_form_field_hierarchical(0,
    ///     TextFieldWidget::new("address.city", Rect::new(100.0, 670.0, 200.0, 20.0))
    /// )?;
    ///
    /// // Creates nested hierarchy: "contact" -> "address" -> "zip"
    /// editor.add_form_field_hierarchical(0,
    ///     TextFieldWidget::new("contact.address.zip", Rect::new(100.0, 640.0, 100.0, 20.0))
    /// )?;
    ///
    /// editor.save("output.pdf")?;
    /// ```
    pub fn add_form_field_hierarchical<W: crate::writer::form_fields::FormFieldWidget>(
        &mut self,
        page: usize,
        widget: W,
    ) -> Result<String> {
        use crate::editor::form_fields::ParentFieldConfig;

        let full_name = widget.field_name().to_string();

        // If no dots, delegate to regular add_form_field
        if !full_name.contains('.') {
            return self.add_form_field(page, widget);
        }

        // Parse the hierarchy path
        let parts: Vec<&str> = full_name.split('.').collect();

        // Create parent fields as needed
        let mut current_parent = String::new();
        for i in 0..(parts.len() - 1) {
            let part = parts[i];
            let parent_name = if current_parent.is_empty() {
                part.to_string()
            } else {
                format!("{}.{}", current_parent, part)
            };

            // Create parent if it doesn't exist
            if !self.has_form_field(&parent_name)? {
                let mut config = ParentFieldConfig::new(part);
                if !current_parent.is_empty() {
                    config = config.with_parent(&current_parent);
                }
                self.add_parent_field(config)?;
            }

            current_parent = parent_name;
        }

        // Add the terminal field as a child
        self.add_child_field(&current_parent, page, widget)
    }

    /// Set the value of an existing form field.
    ///
    /// Modifies the value of a form field. The field must exist in the document
    /// (either from the original PDF or added via `add_form_field`).
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `value` - The new value for the field
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::{DocumentEditor, FormFieldValue};
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    ///
    /// editor.set_form_field_value("name", FormFieldValue::Text("John Doe".into()))?;
    /// editor.set_form_field_value("subscribe", FormFieldValue::Boolean(true))?;
    ///
    /// editor.save("updated.pdf")?;
    /// ```
    pub fn set_form_field_value(
        &mut self,
        name: &str,
        value: crate::editor::form_fields::FormFieldValue,
    ) -> Result<()> {
        use crate::extractors::forms::FormExtractor;

        // Check if deleted
        if self.deleted_form_fields.contains(name) {
            return Err(Error::InvalidPdf(format!("Cannot set value on deleted field: {}", name)));
        }

        // Check if we already have a wrapper for this field
        if let Some(wrapper) = self.modified_form_fields.get_mut(name) {
            wrapper.set_value(value);
            self.is_modified = true;
            self.acroform_modified = true;
            return Ok(());
        }

        // Look up in original document and create wrapper
        let source_fields = FormExtractor::extract_fields(&self.source)?;

        for field in source_fields {
            if field.full_name == name {
                // Create wrapper and set value
                let obj_ref = field.object_ref;
                let mut wrapper = FormFieldWrapper::from_read(field, 0, obj_ref);
                wrapper.set_value(value);

                self.modified_form_fields.insert(name.to_string(), wrapper);
                self.is_modified = true;
                self.acroform_modified = true;
                return Ok(());
            }
        }

        Err(Error::InvalidPdf(format!("Form field not found: {}", name)))
    }

    /// Remove a form field from the document.
    ///
    /// Marks a form field for removal. The field will be removed from the
    /// document's AcroForm and its widget annotation will be removed from
    /// the page when the document is saved.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field to remove
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    ///
    /// editor.remove_form_field("obsolete_field")?;
    ///
    /// editor.save("cleaned.pdf")?;
    /// ```
    pub fn remove_form_field(&mut self, name: &str) -> Result<()> {
        // Check if field exists
        if !self.has_form_field(name)? {
            return Err(Error::InvalidPdf(format!("Form field not found: {}", name)));
        }

        // Remove from modified fields if present
        self.modified_form_fields.remove(name);

        // Add to deleted set
        self.deleted_form_fields.insert(name.to_string());

        self.is_modified = true;
        self.acroform_modified = true;

        Ok(())
    }

    // ========== Form Field Property Modification APIs ==========

    /// Set a form field to read-only.
    ///
    /// A read-only field cannot be edited by the user in a PDF viewer.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `readonly` - Whether the field should be read-only
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("form.pdf")?;
    /// editor.set_form_field_readonly("signature_field", true)?;
    /// editor.save("readonly.pdf")?;
    /// ```
    pub fn set_form_field_readonly(&mut self, name: &str, readonly: bool) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_readonly(readonly);
        })
    }

    /// Set a form field as required.
    ///
    /// A required field must have a value when the form is submitted/exported.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `required` - Whether the field should be required
    pub fn set_form_field_required(&mut self, name: &str, required: bool) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_required(required);
        })
    }

    /// Set a form field's tooltip/description.
    ///
    /// The tooltip is displayed when the user hovers over the field.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `tooltip` - The tooltip text
    pub fn set_form_field_tooltip(&mut self, name: &str, tooltip: impl Into<String>) -> Result<()> {
        let tooltip_str = tooltip.into();
        self.modify_form_field(name, |wrapper| {
            wrapper.set_tooltip(tooltip_str);
        })
    }

    /// Set a form field's bounding rectangle.
    ///
    /// This changes the position and size of the field on the page.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `rect` - The new bounding rectangle
    pub fn set_form_field_rect(&mut self, name: &str, rect: Rect) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_rect(rect);
        })
    }

    /// Set a form field's maximum text length.
    ///
    /// Only applicable to text fields.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `max_len` - The maximum number of characters
    pub fn set_form_field_max_length(&mut self, name: &str, max_len: u32) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_max_length(max_len);
        })
    }

    /// Set a form field's text alignment.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `alignment` - 0 = left, 1 = center, 2 = right
    pub fn set_form_field_alignment(&mut self, name: &str, alignment: u32) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_alignment(alignment);
        })
    }

    /// Set a form field's background color.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `color` - RGB color values (0.0 to 1.0)
    pub fn set_form_field_background_color(&mut self, name: &str, color: [f32; 3]) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_background_color(color);
        })
    }

    /// Set a form field's border color.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `color` - RGB color values (0.0 to 1.0)
    pub fn set_form_field_border_color(&mut self, name: &str, color: [f32; 3]) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_border_color(color);
        })
    }

    // ── pdf_manipulator patch: properties removed, not changed ──
    /// Remove a form field's tooltip (`/TU`).
    pub fn remove_form_field_tooltip(&mut self, name: &str) -> Result<()> {
        self.modify_form_field(name, |wrapper| wrapper.remove_tooltip())
    }

    /// Remove a form field's maximum text length (`/MaxLen`).
    pub fn remove_form_field_max_length(&mut self, name: &str) -> Result<()> {
        self.modify_form_field(name, |wrapper| wrapper.remove_max_length())
    }

    /// Remove a form field's background color (`/MK /BG`).
    pub fn remove_form_field_background_color(&mut self, name: &str) -> Result<()> {
        self.modify_form_field(name, |wrapper| wrapper.remove_background_color())
    }

    /// Remove a form field's border color (`/MK /BC`).
    pub fn remove_form_field_border_color(&mut self, name: &str) -> Result<()> {
        self.modify_form_field(name, |wrapper| wrapper.remove_border_color())
    }
    // ── end pdf_manipulator patch ──

    /// Set a form field's border width.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `width` - Border width in points
    pub fn set_form_field_border_width(&mut self, name: &str, width: f32) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_border_width(width);
        })
    }

    /// Set a form field's default appearance string.
    ///
    /// The DA string specifies font, size, and color for field content.
    /// Example: "/Helv 12 Tf 0 g" for 12pt Helvetica in black.
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `da` - The default appearance string
    pub fn set_form_field_default_appearance(
        &mut self,
        name: &str,
        da: impl Into<String>,
    ) -> Result<()> {
        let da_str = da.into();
        self.modify_form_field(name, |wrapper| {
            wrapper.set_default_appearance(da_str);
        })
    }

    /// Set form field flags directly.
    ///
    /// Use this for setting custom flag combinations. Common flags:
    /// - Bit 1 (0x01): ReadOnly
    /// - Bit 2 (0x02): Required
    /// - Bit 3 (0x04): NoExport
    ///
    /// # Arguments
    ///
    /// * `name` - The full qualified name of the field
    /// * `flags` - The field flag bits
    pub fn set_form_field_flags(&mut self, name: &str, flags: u32) -> Result<()> {
        self.modify_form_field(name, |wrapper| {
            wrapper.set_flags(flags);
        })
    }

    /// Internal helper to modify a form field.
    ///
    /// Gets or creates a wrapper for the field and applies the modification.
    fn modify_form_field<F>(&mut self, name: &str, modify_fn: F) -> Result<()>
    where
        F: FnOnce(&mut FormFieldWrapper),
    {
        use crate::extractors::forms::FormExtractor;

        // Check if deleted
        if self.deleted_form_fields.contains(name) {
            return Err(Error::InvalidPdf(format!("Cannot modify deleted field: {}", name)));
        }

        // Check if we already have a wrapper for this field
        if let Some(wrapper) = self.modified_form_fields.get_mut(name) {
            modify_fn(wrapper);
            self.is_modified = true;
            self.acroform_modified = true;
            return Ok(());
        }

        // Look up in original document and create wrapper
        let source_fields = FormExtractor::extract_fields(&self.source)?;

        for field in source_fields {
            if field.full_name == name {
                // Get object ref from the field
                let object_ref = field.object_ref;

                // Create wrapper
                let mut wrapper = FormFieldWrapper::from_read(field, 0, object_ref);
                modify_fn(&mut wrapper);

                self.modified_form_fields.insert(name.to_string(), wrapper);
                self.is_modified = true;
                self.acroform_modified = true;
                return Ok(());
            }
        }

        Err(Error::InvalidPdf(format!("Form field not found: {}", name)))
    }

    // ========== Form Data Export APIs ==========

    /// Export form field data to FDF format.
    ///
    /// Writes all form field data (original and modified) to an FDF file.
    /// This is useful for data extraction, backup, or batch processing.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path to write the FDF file
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("filled_form.pdf")?;
    /// editor.export_form_data_fdf("form_data.fdf")?;
    /// ```
    pub fn export_form_data_fdf(&mut self, output_path: impl AsRef<std::path::Path>) -> Result<()> {
        use crate::extractors::forms::FormExtractor;
        FormExtractor::export_fdf(&self.source, output_path)
    }

    /// Export form field data to XFDF format.
    ///
    /// Writes all form field data (original and modified) to an XFDF (XML) file.
    /// XFDF is useful for web integration and human-readable data exchange.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path to write the XFDF file
    ///
    /// # Example
    ///
    /// ```ignore
    /// use pdf_oxide::editor::DocumentEditor;
    ///
    /// let mut editor = DocumentEditor::open("filled_form.pdf")?;
    /// editor.export_form_data_xfdf("form_data.xfdf")?;
    /// ```
    pub fn export_form_data_xfdf(
        &mut self,
        output_path: impl AsRef<std::path::Path>,
    ) -> Result<()> {
        use crate::extractors::forms::FormExtractor;
        FormExtractor::export_xfdf(&self.source, output_path)
    }

    /// Get widget annotation appearances for form flattening.
    ///
    /// Returns appearance data for Widget annotations only.
    /// Generates appearance streams for widgets that don't have them.
    fn get_widget_appearances(&mut self, page: usize) -> Result<Vec<AnnotationAppearance>> {
        use crate::annotation_types::AnnotationSubtype;

        // ── pdf_manipulator patch: one annotation is held at a time ──
        let annotation_refs = self.source.annotation_refs(page)?;
        let mut appearances = Vec::new();

        for (at, annotation_ref) in annotation_refs.into_iter().enumerate() {
            // Each annotation and appearance is read once: the cache would
            // only fill beside the appearances.
            if at % 256 == 255 {
                self.source.clear_object_cache();
            }
            let Ok(annotation) = self.source.parse_annotation(annotation_ref) else {
                continue;
            };
            // ── end pdf_manipulator patch ──
            // Only process Widget annotations (form fields)
            if annotation.subtype_enum != AnnotationSubtype::Widget {
                continue;
            }

            // Skip annotations without a raw dictionary
            let raw_dict = match &annotation.raw_dict {
                Some(dict) => dict,
                None => continue,
            };

            // ── pdf_manipulator patch: modified buttons flatten in their new
            // state (#215) ──
            // A button's new value is a state name, and the paths below cannot
            // see it: the text branch only understands Text/Choice values and
            // would leave field_type's `checked` flag alone, while the /AP
            // branch reads the document's stale /AS. Both bake the unchecked
            // appearance over a box the caller had checked. Pick the state
            // first, then prefer the widget's own /AP stream for it — that is
            // what a viewer would draw — and regenerate only when the widget
            // carries no state dictionary to pick from.
            if let Some(state) = self.modified_button_state(&annotation)? {
                let mut updated = annotation.clone();
                updated.appearance_state =
                    Some(state.clone().unwrap_or_else(|| "Off".to_string()));
                updated.field_type = match updated.field_type {
                    Some(crate::annotation_types::WidgetFieldType::Radio { .. }) => {
                        Some(crate::annotation_types::WidgetFieldType::Radio {
                            selected: state.clone(),
                        })
                    },
                    _ => Some(crate::annotation_types::WidgetFieldType::Checkbox {
                        checked: state.is_some(),
                    }),
                };
                if let Ok(Some(app)) = self.extract_widget_appearance(&updated, &raw_dict.clone()) {
                    appearances.push(app);
                    continue;
                }
                if let Some(generated) = self.generate_widget_appearance(&updated)? {
                    appearances.push(generated);
                }
                // Off with no /Off stream draws nothing — correct, not a warning.
                continue;
            }
            // ── end pdf_manipulator patch ──

            // ── pdf_manipulator patch: a choice widget whose value is a list ──
            // A multi-select list box's value is an array, which the text
            // branch below cannot carry, and a cleared one has no value to
            // read. Both regenerate from the dictionary the appearance reads,
            // with this session's value written into it; a document saved
            // earlier with /NeedAppearances regenerates from its stored array.
            if matches!(
                annotation.field_type,
                Some(crate::annotation_types::WidgetFieldType::Choice { .. })
            ) {
                use crate::editor::form_fields::FormFieldValue;
                let modified = self.modified_choice_value(&annotation);
                let stored_list = self.source.acroform_needs_appearances()
                    && matches!(
                        crate::host::form_choice::inherited(&self.source, &raw_dict.clone(), "V"),
                        Some(Object::Array(_))
                    );
                if matches!(modified, Some(FormFieldValue::MultiChoice(_) | FormFieldValue::None))
                    || (modified.is_none() && stored_list)
                {
                    let mut updated = annotation.clone();
                    if let (Some(value), Some(dict)) = (&modified, updated.raw_dict.as_mut()) {
                        match value {
                            FormFieldValue::None => {
                                dict.remove("V");
                            },
                            other => {
                                dict.insert("V".to_string(), Object::from(other));
                            },
                        }
                    }
                    if let Some(crate::annotation_types::WidgetFieldType::Choice {
                        options, ..
                    }) = &updated.field_type
                    {
                        updated.field_type =
                            Some(crate::annotation_types::WidgetFieldType::Choice {
                                options: options.clone(),
                                selected: None,
                            });
                    }
                    if let Some(generated) = self.generate_widget_appearance(&updated)? {
                        appearances.push(generated);
                        continue;
                    }
                }
            }
            // ── end pdf_manipulator patch ──

            // If this widget's value was changed via set_form_field_value, the
            // document's stored /AP is stale — it still renders the original
            // value or the empty placeholder the form writer never refreshed.
            // Regenerate the appearance from the new value so the flattened page
            // shows what was set, instead of baking the blank placeholder.
            // ── pdf_manipulator patch: also regenerate from persisted /V on reopen ──
            // modified_widget_text() is the in-session value and is empty after a
            // save+reopen (it lives in the session-local modified_form_fields map).
            // A field filled in an earlier session therefore falls through to the
            // stale /AP below and flattens blank. When the AcroForm carries
            // /NeedAppearances true — which our save path sets whenever a value is
            // written, deliberately leaving the /AP a placeholder — take the parsed
            // /V instead and regenerate. Same regeneration block, two text sources.
            // buttons are not text (#215)
            // A button's /V is an appearance-state name, so the /NeedAppearances
            // fallback below would feed "Yes" in as a value to draw. It reaches
            // generate_widget_appearance, which ignores the text and emits its
            // own synthetic checkmark — discarding the /Yes artwork the document
            // actually ships. Buttons fall through to the /AP branch instead,
            // which picks the stream their /AS names, and only regenerate when
            // the widget carries no state dictionary at all.
            let is_button_widget = matches!(
                annotation.field_type,
                Some(crate::annotation_types::WidgetFieldType::Checkbox { .. })
                    | Some(crate::annotation_types::WidgetFieldType::Radio { .. })
                    | Some(crate::annotation_types::WidgetFieldType::Button)
            );
            let regen_text = if is_button_widget {
                None
            } else {
                self.modified_widget_text(&annotation).or_else(|| {
                    if self.source.acroform_needs_appearances() {
                        annotation.field_value.clone().filter(|s| !s.is_empty())
                    } else {
                        None
                    }
                })
            };
            if let Some(text) = regen_text {
                // ── end pdf_manipulator patch ──
                // Values with CJK/emoji glyphs the /DA font cannot render are
                // regenerated with an embedded fallback font; pure-Latin values
                // return None here and fall through to the plain /DA path.
                if let Some(app) = self.regenerate_text_appearance_embedded(&annotation, &text)? {
                    appearances.push(app);
                    continue;
                }
                let mut updated = annotation.clone();
                match updated.field_type {
                    Some(crate::annotation_types::WidgetFieldType::Choice {
                        ref options, ..
                    }) => {
                        let options = options.clone();
                        // ── pdf_manipulator patch: the stored value is replaced too ──
                        // The appearance reads a list-valued /V from the dictionary
                        // first, so a stale array there would win over `text`.
                        if let Some(dict) = updated.raw_dict.as_mut() {
                            dict.insert("V".to_string(), Object::text_string(&text));
                        }
                        // ── end pdf_manipulator patch ──
                        updated.field_type =
                            Some(crate::annotation_types::WidgetFieldType::Choice {
                                options,
                                selected: Some(text),
                            });
                    },
                    _ => {
                        updated.field_value = Some(text);
                    },
                }
                if let Some(generated) = self.generate_widget_appearance(&updated)? {
                    appearances.push(generated);
                    continue;
                }
            }

            // Try to get appearance from AP dictionary
            let appearance_result = self.extract_widget_appearance(&annotation, raw_dict);

            match appearance_result {
                Ok(Some(appearance)) => appearances.push(appearance),
                Ok(None) => {
                    // No appearance stream - try to generate one
                    if let Some(generated) = self.generate_widget_appearance(&annotation)? {
                        appearances.push(generated);
                    } else {
                        // Widget has neither /AP nor a generatable appearance — flattening
                        // it produces a blank rectangle. Record field name as a warning.
                        let field_name = annotation
                            .raw_dict
                            .as_ref()
                            .and_then(|d| d.get("T"))
                            .and_then(|t| match t {
                                // ── pdf_manipulator patch: /T is a text string ──
                                Object::String(s) => {
                                    Some(crate::object::decode_pdf_text_string(s))
                                },
                                // ── end pdf_manipulator patch ──
                                _ => None,
                            })
                            .unwrap_or_else(|| format!("widget@page{}", page));
                        self.flatten_warnings.push(format!(
                            "field '{}' has no /AP appearance stream — flattening produces blank rectangle",
                            field_name
                        ));
                    }
                },
                Err(_) => continue,
            }
        }

        Ok(appearances)
    }

    /// Look up the value set via `set_form_field_value` for the field that owns
    /// `annotation`, if it was modified in this editing session. Used by the
    /// flattener to regenerate a stale `/AP` from the new value.
    fn modified_widget_text(&self, annotation: &crate::annotations::Annotation) -> Option<String> {
        use crate::editor::form_fields::FormFieldValue;
        let field_name = annotation.field_name.as_deref()?;
        // The widget's /T is the partial name; match the modified field by exact
        // name first, then by trailing segment for hierarchical fields.
        let wrapper = self.modified_form_fields.get(field_name).or_else(|| {
            self.modified_form_fields
                .iter()
                .find(|(full, _)| full.rsplit('.').next() == Some(field_name))
                .map(|(_, w)| w)
        })?;
        match wrapper.value() {
            FormFieldValue::Text(s) | FormFieldValue::Choice(s) => Some(s),
            _ => None,
        }
    }

    // ── pdf_manipulator patch: the value set on a choice widget's field this session ──
    /// The value this session set on the field that owns `annotation`, when it
    /// set one; unlike [`Self::modified_widget_text`] it keeps every shape.
    fn modified_choice_value(
        &self,
        annotation: &crate::annotations::Annotation,
    ) -> Option<crate::editor::form_fields::FormFieldValue> {
        let field_name = annotation.field_name.as_deref()?;
        let wrapper = self.modified_form_fields.get(field_name).or_else(|| {
            self.modified_form_fields
                .iter()
                .find(|(full, _)| full.rsplit('.').next() == Some(field_name))
                .map(|(_, w)| w)
        })?;
        wrapper.modified_value.clone()
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: modified-button state for flattening (#215) ──

    /// The appearance state `annotation`'s widget must flatten in, when its
    /// field's value was changed this session.
    ///
    /// `Some(Some(name))` turns the widget on in that state, `Some(None)` turns
    /// it off, `None` means this widget's field was not modified as a button
    /// and the caller should fall through to its other paths.
    ///
    /// The flattener cannot read the widget's own `/AS` here: the document on
    /// disk still says `/Off`, because the new value lives only in
    /// `modified_form_fields` until save. Reading it anyway is what baked the
    /// unchecked appearance over a box the caller had checked.
    fn modified_button_state(
        &mut self,
        annotation: &crate::annotations::Annotation,
    ) -> Result<Option<Option<String>>> {
        use crate::annotation_types::WidgetFieldType;
        use crate::extractors::forms::FieldType;

        // Only button widgets have appearance states to choose between.
        match annotation.field_type {
            Some(WidgetFieldType::Checkbox { .. }) | Some(WidgetFieldType::Radio { .. }) => {},
            _ => return Ok(None),
        }
        let raw_dict = match &annotation.raw_dict {
            Some(d) => d.clone(),
            None => return Ok(None),
        };

        // Locate the modified field that owns this widget. A merged
        // field+widget carries its own /T; a radio kid carries none, and is
        // owned by whichever ancestor the /Parent chain reaches.
        let mut owner: Option<&FormFieldWrapper> = None;
        if let Some(name) = annotation.field_name.as_deref() {
            owner = self.modified_form_fields.get(name).or_else(|| {
                self.modified_form_fields
                    .iter()
                    .find(|(full, _)| full.rsplit('.').next() == Some(name))
                    .map(|(_, w)| w)
            });
        }
        if owner.is_none() {
            let mut parent = raw_dict.get("Parent").and_then(|p| p.as_reference());
            let mut depth = 0;
            while let (Some(pref), true) = (parent, depth < 10) {
                depth += 1;
                if let Some(w) = self
                    .modified_form_fields
                    .values()
                    .find(|w| w.object_ref().map(|r| r.id) == Some(pref.id))
                {
                    owner = Some(w);
                    break;
                }
                parent = match self.source.load_object(pref) {
                    Ok(obj) => obj
                        .as_dict()
                        .and_then(|d| d.get("Parent"))
                        .and_then(|p| p.as_reference()),
                    Err(_) => None,
                };
            }
        }
        // Take what we need and drop the borrow before touching self again.
        let (value, owner_ref) = {
            let owner = match owner {
                Some(w) => w,
                None => return Ok(None),
            };
            if owner.field_type().map(|ft| *ft != FieldType::Button).unwrap_or(false) {
                return Ok(None);
            }
            (owner.value(), owner.object_ref())
        };

        // Resolve against the states the whole FIELD offers — the same set the
        // save path sees — so a group answers once. Resolving against this one
        // widget's states would let an on-word light every kid of a group while
        // the save path wrote /Off.
        let available = match owner_ref.and_then(|r| self.source.load_object(r).ok()) {
            Some(field) => match field.as_dict() {
                Some(d) => self.field_on_states(&d.clone()),
                None => self.widget_on_states(&raw_dict),
            },
            None => self.widget_on_states(&raw_dict),
        };
        let target = Self::button_on_state(&value, &available);

        // The field chose one state; this widget lights only if it offers it.
        if let Some(name) = &target {
            let mine = self.widget_on_states(&raw_dict);
            if !mine.is_empty() && !mine.iter().any(|a| a == name) {
                // A sibling's state — this widget stays off.
                return Ok(Some(None));
            }
        }
        Ok(Some(target))
    }

    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: widget-appearance generation moved to
    // form_regen.rs (`impl PdfDocument`) so the page renderer can share it —
    // resolve_obj, acroform_needs_appearances, acroform_default_resources,
    // effective_da, parse_da, helvetica_font_dict, build_text_appearance_resources,
    // and is_self_contained_simple_font now live there. This editor reaches them
    // via `self.source.*` (resolve_obj was reinventing PdfDocument::resolve_object
    // and is gone). The flatten path keeps the CJK/emoji embedded-font fallback
    // below; the shared Latin/`/DA` path is `self.source.regenerate_widget_appearance`. ──
    // ── end pdf_manipulator patch ──

    /// Extract appearance stream from a widget annotation.
    fn extract_widget_appearance(
        &mut self,
        annotation: &crate::annotations::Annotation,
        raw_dict: &HashMap<String, Object>,
    ) -> Result<Option<AnnotationAppearance>> {
        // Get the /AP (appearance) dictionary
        let ap_dict = match raw_dict.get("AP") {
            Some(Object::Dictionary(d)) => d.clone(),
            Some(Object::Reference(ap_ref)) => match self.source.load_object(*ap_ref)? {
                Object::Dictionary(d) => d,
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };

        // Get the /N (normal appearance) entry
        let normal_appearance = match ap_dict.get("N") {
            Some(obj) => obj.clone(),
            None => return Ok(None),
        };

        // Handle appearance states (e.g., /Yes and /Off for checkboxes)
        let (appearance_obj, appearance_ref) = match normal_appearance {
            Object::Reference(ref_obj) => {
                let obj = self.source.load_object(ref_obj)?;
                (obj, Some(ref_obj))
            },
            Object::Dictionary(ref dict) => {
                // Check if this is a Form XObject or a state dictionary
                if dict.get("Type").and_then(|t| t.as_name()) == Some("XObject") {
                    (Object::Dictionary(dict.clone()), None)
                } else {
                    // This is a state dictionary - get the current appearance state
                    let state = annotation.appearance_state.as_deref().unwrap_or("Off");
                    match dict.get(state) {
                        Some(Object::Reference(ref_obj)) => {
                            let obj = self.source.load_object(*ref_obj)?;
                            (obj, Some(*ref_obj))
                        },
                        Some(obj) => (obj.clone(), None),
                        None => {
                            // Try "Yes" as fallback for checkboxes
                            if state == "Off" {
                                return Ok(None); // Off state - skip
                            }
                            match dict.get("Yes") {
                                Some(Object::Reference(ref_obj)) => {
                                    let obj = self.source.load_object(*ref_obj)?;
                                    (obj, Some(*ref_obj))
                                },
                                Some(obj) => (obj.clone(), None),
                                None => return Ok(None),
                            }
                        },
                    }
                }
            },
            _ => return Ok(None),
        };

        // Extract Form XObject properties
        let form_dict = match appearance_obj.as_dict() {
            Some(d) => d,
            None => return Ok(None),
        };

        // Get BBox
        let bbox = match form_dict.get("BBox") {
            Some(Object::Array(arr)) if arr.len() >= 4 => {
                let values: Vec<f64> = arr
                    .iter()
                    .filter_map(|o| o.as_real().or_else(|| o.as_integer().map(|i| i as f64)))
                    .collect();
                if values.len() >= 4 {
                    [
                        values[0] as f32,
                        values[1] as f32,
                        values[2] as f32,
                        values[3] as f32,
                    ]
                } else {
                    return Ok(None);
                }
            },
            _ => return Ok(None),
        };

        // Get Matrix (optional)
        let matrix = match form_dict.get("Matrix") {
            Some(Object::Array(arr)) if arr.len() >= 6 => {
                let values: Vec<f64> = arr
                    .iter()
                    .filter_map(|o| o.as_real().or_else(|| o.as_integer().map(|i| i as f64)))
                    .collect();
                if values.len() >= 6 {
                    Some([
                        values[0] as f32,
                        values[1] as f32,
                        values[2] as f32,
                        values[3] as f32,
                        values[4] as f32,
                        values[5] as f32,
                    ])
                } else {
                    None
                }
            },
            _ => None,
        };

        // Get Resources
        let resources = form_dict.get("Resources").cloned();

        // Get the annotation's Rect
        let annot_rect = annotation.rect.unwrap_or([0.0, 0.0, 0.0, 0.0]);
        let annot_rect = [
            annot_rect[0] as f32,
            annot_rect[1] as f32,
            annot_rect[2] as f32,
            annot_rect[3] as f32,
        ];

        // ── pdf_manipulator patch: the appearance stream is carried as it is stored ──
        let stream_obj = match appearance_ref {
            Some(ref_obj) => self.source.load_object(ref_obj)?,
            None => appearance_obj,
        };
        let (dict, body) = AnnotationAppearance::stream_of(&stream_obj);
        // ── end pdf_manipulator patch ──

        Ok(Some(AnnotationAppearance {
            dict,
            body,
            bbox,
            annot_rect,
            matrix,
            resources,
            extra_objects: Vec::new(),
        }))
    }

    /// Escape a string for a PDF literal `(...)` text operand.
    fn escape_pdf_literal(s: &str) -> String {
        // ── pdf_manipulator patch: single-byte encode, not UTF-8 passthrough —
        // these literals draw under single-byte /DA fonts. ──
        crate::writer::form_fields::encode_win_ansi_literal(s)
        // ── end pdf_manipulator patch ──
    }

    /// Regenerate a *text* widget appearance whose value contains non-Latin or
    /// emoji characters by embedding bundled fallback fonts and emitting
    /// CID-keyed text (ISO 32000-1 §12.7.3.3 + §9.7 composite fonts).
    ///
    /// Returns `Ok(None)` when no fallback is needed (pure Latin-1) or available
    /// (no fallback font is registered or compiled in, or a font fails to
    /// parse/embed), so the caller falls back to the plain `/DA` appearance path.
    fn regenerate_text_appearance_embedded(
        &mut self,
        annotation: &crate::annotations::Annotation,
        text: &str,
    ) -> Result<Option<AnnotationAppearance>> {
        use crate::annotation_types::WidgetFieldType;
        use crate::fonts::form_fallback;

        match annotation.field_type {
            Some(WidgetFieldType::Text)
            | Some(WidgetFieldType::Choice { .. })
            | Some(WidgetFieldType::Button) => {},
            _ => return Ok(None),
        }
        if text.is_empty() || !text.chars().any(|c| form_fallback::classify(c).is_some()) {
            return Ok(None);
        }

        // ── pdf_manipulator patch: fallback availability is a runtime question
        // (registered font first, compiled-in asset second), not a compile-time
        // cfg — the missing-font warning moved into
        // embed_fallback_text_appearance where the per-kind lookup happens. ──
        self.embed_fallback_text_appearance(annotation, text)
        // ── end pdf_manipulator patch ──
    }

    /// The fallback-embedding body of [`Self::regenerate_text_appearance_embedded`].
    fn embed_fallback_text_appearance(
        &mut self,
        annotation: &crate::annotations::Annotation,
        text: &str,
    ) -> Result<Option<AnnotationAppearance>> {
        use crate::fonts::form_fallback::{self, Fallback};
        use crate::writer::{build_embedded_font_objects, EmbeddedFont};
        use std::collections::BTreeMap;

        let rect = match annotation.rect {
            Some(r) => r,
            None => return Ok(None),
        };
        let width = (rect[2] - rect[0]) as f32;
        let height = (rect[3] - rect[1]) as f32;

        // ── pdf_manipulator patch: parse_da/effective_da moved to form_regen.rs ──
        let (da_font, da_size, da_color) =
            PdfDocument::parse_da(&self.source.effective_da(annotation).unwrap_or_default());
        // ── end pdf_manipulator patch ──
        let font_size = if da_size > 0.0 {
            da_size
        } else {
            (height * 0.7).clamp(6.0, 12.0)
        };

        // Resource dict seeded with the /DA font (used for any Latin runs).
        let mut font_res: HashMap<String, Object> = HashMap::new();
        // ── pdf_manipulator patch: build_text_appearance_resources moved to form_regen.rs ──
        if let Object::Dictionary(res) = self.source.build_text_appearance_resources(&da_font) {
            // ── end pdf_manipulator patch ──
            if let Some(Object::Dictionary(f)) = res.get("Font") {
                font_res = f.clone();
            }
        }

        // Parse the value into runs, and build one EmbeddedFont per fallback
        // kind present, recording the glyphs each run uses.
        let runs = form_fallback::split_runs(text);
        let mut fonts: BTreeMap<Fallback, EmbeddedFont> = BTreeMap::new();
        for run in &runs {
            if let Some(kind) = run.fallback {
                if !fonts.contains_key(&kind) {
                    // ── pdf_manipulator patch: registered-or-embedded lookup;
                    // a missing font degrades to the /DA path with a warning
                    // (the pre-registry builds said "rebuild with the
                    // `cjk-form-fonts` feature" here). ──
                    let bytes = match form_fallback::resolve_font_bytes(kind) {
                        Some(b) => b,
                        None => {
                            self.flatten_warnings.push(format!(
                                "field value contains {:?} characters the /DA font cannot \
                                 render; register a fallback font (registerFallbackFont) or \
                                 build with the `cjk-form-fonts` feature",
                                kind
                            ));
                            return Ok(None);
                        },
                    };
                    match EmbeddedFont::from_data(None, bytes.to_vec()) {
                        // ── end pdf_manipulator patch ──
                        Ok(f) => {
                            fonts.insert(kind, f);
                        },
                        // A font that won't parse → give up, use the /DA path.
                        Err(_) => return Ok(None),
                    }
                }
            }
        }
        // Per run, the original-face glyph IDs (for fallback runs).
        let mut run_gids: Vec<Vec<u16>> = Vec::with_capacity(runs.len());
        for run in &runs {
            match run.fallback {
                Some(kind) => {
                    let gids = fonts
                        .get_mut(&kind)
                        .expect("font present")
                        .encode_string(&run.text);
                    run_gids.push(gids);
                },
                None => run_gids.push(Vec::new()),
            }
        }

        // Build the embedded-font object graph for each kind and remember the
        // subset remapper + the resource name we registered it under.
        let mut extra_objects: Vec<(u32, Object)> = Vec::new();
        let mut placed: BTreeMap<Fallback, (String, crate::fonts::GlyphRemapper)> = BTreeMap::new();
        for (kind, font) in fonts.iter_mut() {
            let (ids, objs, remapper) =
                build_embedded_font_objects(font, || self.allocate_object_id())?;
            let res_name = match kind {
                Fallback::Cjk => "FbCJK",
                Fallback::Emoji => "FbEmoji",
            };
            font_res.insert(res_name.to_string(), Object::Reference(ObjectRef::new(ids.type0, 0)));
            extra_objects.extend(objs);
            placed.insert(*kind, (res_name.to_string(), remapper));
        }

        // Emit the appearance content per ISO 32000-1 §12.7.3.3.
        let mut content = String::new();
        content.push_str("/Tx BMC\nq\nBT\n");
        let (r, g, b) = da_color;
        content.push_str(&format!("{} {} {} rg\n", r, g, b));
        let y = ((height - font_size) / 2.0).max(2.0);
        content.push_str(&format!("2 {} Td\n", y));

        for (run, gids) in runs.iter().zip(run_gids.iter()) {
            match run.fallback {
                None => {
                    content.push_str(&format!("{} {} Tf\n", da_font, font_size));
                    content.push_str(&format!("({}) Tj\n", Self::escape_pdf_literal(&run.text)));
                },
                Some(kind) => {
                    let (res_name, remapper) = placed.get(&kind).expect("placed font");
                    content.push_str(&format!("/{} {} Tf\n", res_name, font_size));
                    content.push('<');
                    for &orig in gids {
                        let sub = remapper.get(orig).unwrap_or(orig);
                        content.push_str(&format!("{:04X}", sub));
                    }
                    content.push_str("> Tj\n");
                },
            }
        }
        content.push_str("ET\nQ\nEMC\n");

        let mut res = HashMap::new();
        res.insert("Font".to_string(), Object::Dictionary(font_res));

        Ok(Some(AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from(content.into_bytes())),
            bbox: [0.0, 0.0, width, height],
            annot_rect: [
                rect[0] as f32,
                rect[1] as f32,
                rect[2] as f32,
                rect[3] as f32,
            ],
            matrix: None,
            resources: Some(Object::Dictionary(res)),
            extra_objects,
        }))
    }

    /// Generate appearance stream for a widget without one.
    // ── pdf_manipulator patch: delegate to the shared generator ──
    /// Regenerate a widget's appearance for the flatten overlay by delegating to
    /// the shared `PdfDocument::regenerate_widget_appearance` (form_regen.rs,
    /// also used by the page renderer) and wrapping the result into an
    /// `AnnotationAppearance`.
    fn generate_widget_appearance(
        &self,
        annotation: &crate::annotations::Annotation,
    ) -> Result<Option<AnnotationAppearance>> {
        let generated = match self.source.regenerate_widget_appearance(annotation)? {
            Some(g) => g,
            None => return Ok(None),
        };
        let annot_rect = annotation
            .rect
            .map(|r| [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32])
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);
        Ok(Some(AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from(generated.content)),
            bbox: generated.bbox,
            annot_rect,
            matrix: None,
            resources: generated.resources,
            extra_objects: Vec::new(),
        }))
    }
    // ── end pdf_manipulator patch ──

    /// Get annotation appearance stream data for flattening.
    ///
    /// Returns a list of (content_bytes, bbox, resources) for each annotation
    /// that has an appearance stream.
    fn get_annotation_appearances(&mut self, page: usize) -> Result<Vec<AnnotationAppearance>> {
        // ── pdf_manipulator patch: one annotation is held at a time ──
        let annotation_refs = self.source.annotation_refs(page)?;
        let mut appearances = Vec::new();

        for annotation_ref in annotation_refs {
            let Ok(annotation) = self.source.parse_annotation(annotation_ref) else {
                continue;
            };
            // ── end pdf_manipulator patch ──
            // Skip annotations without a raw dictionary
            let raw_dict = match &annotation.raw_dict {
                Some(dict) => dict,
                None => continue,
            };

            // Get the /AP (appearance) dictionary
            let ap_dict = match raw_dict.get("AP") {
                Some(Object::Dictionary(d)) => d.clone(),
                Some(Object::Reference(ap_ref)) => match self.source.load_object(*ap_ref)? {
                    Object::Dictionary(d) => d,
                    _ => continue,
                },
                _ => continue,
            };

            // Get the /N (normal appearance) entry
            let normal_appearance = match ap_dict.get("N") {
                Some(obj) => obj.clone(),
                None => continue,
            };

            // The normal appearance can be:
            // 1. A reference to a Form XObject
            // 2. A dictionary of appearance states (e.g., for checkboxes: /Yes, /Off)
            let (appearance_obj, appearance_ref) = match normal_appearance {
                Object::Reference(ref_obj) => {
                    let obj = self.source.load_object(ref_obj)?;
                    (obj, Some(ref_obj))
                },
                Object::Dictionary(ref dict) => {
                    // Check if this is a Form XObject or a state dictionary
                    if dict.get("Type").and_then(|t| t.as_name()) == Some("XObject") {
                        (Object::Dictionary(dict.clone()), None)
                    } else {
                        // This is a state dictionary - get the current appearance state
                        let state = annotation.appearance_state.as_deref().unwrap_or("Off");
                        match dict.get(state) {
                            Some(Object::Reference(ref_obj)) => {
                                let obj = self.source.load_object(*ref_obj)?;
                                (obj, Some(*ref_obj))
                            },
                            Some(obj) => (obj.clone(), None),
                            None => continue,
                        }
                    }
                },
                _ => continue,
            };

            // Extract the Form XObject properties
            let form_dict = match appearance_obj.as_dict() {
                Some(d) => d,
                None => continue,
            };

            // Get BBox
            let bbox = match form_dict.get("BBox") {
                Some(Object::Array(arr)) if arr.len() >= 4 => {
                    let values: Vec<f64> = arr
                        .iter()
                        .filter_map(|o| o.as_real().or_else(|| o.as_integer().map(|i| i as f64)))
                        .collect();
                    if values.len() >= 4 {
                        [
                            values[0] as f32,
                            values[1] as f32,
                            values[2] as f32,
                            values[3] as f32,
                        ]
                    } else {
                        continue;
                    }
                },
                _ => continue,
            };

            // Get Matrix (optional, defaults to identity)
            let matrix = match form_dict.get("Matrix") {
                Some(Object::Array(arr)) if arr.len() >= 6 => {
                    let values: Vec<f64> = arr
                        .iter()
                        .filter_map(|o| o.as_real().or_else(|| o.as_integer().map(|i| i as f64)))
                        .collect();
                    if values.len() >= 6 {
                        Some([
                            values[0] as f32,
                            values[1] as f32,
                            values[2] as f32,
                            values[3] as f32,
                            values[4] as f32,
                            values[5] as f32,
                        ])
                    } else {
                        None
                    }
                },
                _ => None,
            };

            // Get Resources (optional)
            let resources = form_dict.get("Resources").cloned();

            // Get the annotation's Rect (position on page)
            let annot_rect = annotation.rect.unwrap_or([0.0, 0.0, 0.0, 0.0]);
            let annot_rect = [
                annot_rect[0] as f32,
                annot_rect[1] as f32,
                annot_rect[2] as f32,
                annot_rect[3] as f32,
            ];

            // ── pdf_manipulator patch: the appearance stream is carried as it is stored ──
            let stream_obj = match appearance_ref {
                Some(ref_obj) => match self.source.load_object(ref_obj) {
                    Ok(obj) => obj,
                    Err(_) => continue,
                },
                None => appearance_obj,
            };
            let (dict, body) = AnnotationAppearance::stream_of(&stream_obj);
            // ── end pdf_manipulator patch ──

            appearances.push(AnnotationAppearance {
                dict,
                body,
                bbox,
                annot_rect,
                matrix,
                resources,
                extra_objects: Vec::new(),
            });
        }

        Ok(appearances)
    }

    /// Generate content stream to render flattened annotations.
    ///
    /// This creates PDF operators that invoke each annotation's appearance
    /// as a Form XObject at the correct position.
    fn generate_flatten_overlay(
        &self,
        appearances: &[AnnotationAppearance],
        xobject_names: &[String],
    ) -> Vec<u8> {
        let mut content = Vec::new();

        for (appearance, xobj_name) in appearances.iter().zip(xobject_names.iter()) {
            // Save graphics state
            content.extend_from_slice(b"q\n");

            // ── pdf_manipulator patch: §12.5.5 appearance placement ──
            // Position the appearance per ISO 32000-1 §12.5.5: the
            // form's /Matrix is applied by the XObject itself (it is
            // written into the FlatForm dict), so the placement matrix
            // A here must map the MATRIX-TRANSFORMED BBox onto the
            // annotation Rect. Mapping the raw BBox — or re-applying
            // /Matrix in this overlay — double-shifts any appearance
            // whose Matrix carries a translation (producers commonly
            // bake the widget's absolute page Y into it), landing the
            // content off-page.
            let bbox = appearance.bbox;
            let rect = appearance.annot_rect;
            let m = appearance
                .matrix
                .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

            // Transform the four BBox corners through /Matrix and take
            // the bounding box of the result.
            let corners = [
                (bbox[0], bbox[1]),
                (bbox[2], bbox[1]),
                (bbox[0], bbox[3]),
                (bbox[2], bbox[3]),
            ];
            let (mut tb_x0, mut tb_y0) = (f32::INFINITY, f32::INFINITY);
            let (mut tb_x1, mut tb_y1) =
                (f32::NEG_INFINITY, f32::NEG_INFINITY);
            for (x, y) in corners {
                let cx = m[0] * x + m[2] * y + m[4];
                let cy = m[1] * x + m[3] * y + m[5];
                tb_x0 = tb_x0.min(cx);
                tb_y0 = tb_y0.min(cy);
                tb_x1 = tb_x1.max(cx);
                tb_y1 = tb_y1.max(cy);
            }

            let tb_width = tb_x1 - tb_x0;
            let tb_height = tb_y1 - tb_y0;
            let rect_width = rect[2] - rect[0];
            let rect_height = rect[3] - rect[1];

            let sx = if tb_width != 0.0 { rect_width / tb_width } else { 1.0 };
            let sy = if tb_height != 0.0 {
                rect_height / tb_height
            } else {
                1.0
            };
            let tx = rect[0] - tb_x0 * sx;
            let ty = rect[1] - tb_y0 * sy;

            content.extend_from_slice(
                format!("{:.6} 0 0 {:.6} {:.6} {:.6} cm\n", sx, sy, tx, ty).as_bytes(),
            );
            // ── end pdf_manipulator patch ──

            // Invoke the XObject
            content.extend_from_slice(format!("/{} Do\n", xobj_name).as_bytes());

            // Restore graphics state
            content.extend_from_slice(b"Q\n");
        }

        content
    }

    // ========================================================================
    // Redaction Application
    // ========================================================================

    /// Mark a page for redaction application.
    ///
    /// When the document is saved, redaction annotations on this page will be
    /// applied: content will be visually obscured and the redaction annotations
    /// removed.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Apply redactions on page 0
    /// editor.apply_page_redactions(0)?;
    /// editor.save("output.pdf")?;
    /// ```
    /// Queue an explicit rectangular redaction on `page` (zero-based,
    /// output order) in page user-space points `[x0, y0, x1, y1]`.
    ///
    /// The region is applied by [`apply_redactions_destructive`], which
    /// physically removes the underlying text — not a cosmetic overlay
    /// (#231, ISO 32000-1:2008 §12.5.6.23). `fill` is the overlay colour
    /// (DeviceRGB); `None` uses the configured default.
    ///
    /// [`apply_redactions_destructive`]: DocumentEditor::apply_redactions_destructive
    ///
    /// # Errors
    /// [`Error::InvalidPdf`] if `page` is out of range.
    pub fn add_redaction(
        &mut self,
        page: usize,
        rect: [f32; 4],
        fill: Option<[f32; 3]>,
    ) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }
        let source_page = self.source_page_for(page, "adding a redaction")?;
        self.redaction_regions.entry(source_page).or_default().push(
            crate::redaction::RedactionRegion::from_rect(rect[0], rect[1], rect[2], rect[3], fill),
        );
        self.is_modified = true;
        Ok(())
    }

    /// Number of redaction regions queued for `page` (zero-based, output
    /// order): `/Redact` annotations in the source plus programmatic
    /// [`add_redaction`] rectangles.
    ///
    /// [`add_redaction`]: DocumentEditor::add_redaction
    ///
    /// # Errors
    /// [`Error::InvalidPdf`] if `page` is out of range.
    pub fn redaction_count(&mut self, page: usize) -> Result<usize> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }
        let source_page = self.source_page_for(page, "counting redactions")?;
        let annots = self.get_redaction_data(source_page)?.len();
        let prog = self
            .redaction_regions
            .get(&source_page)
            .map_or(0, |v| v.len());
        Ok(annots + prog)
    }

    // ── pdf_manipulator patch: the annotations a redaction drops ──
    /// Every annotation id the queued redactions drop, across the redacted pages.
    fn redacted_annotation_ids(&self) -> std::collections::HashSet<u32> {
        self.redacted_pages
            .values()
            .flat_map(|page| page.dropped_annots.iter().copied())
            .collect()
    }

    /// The annotations of source page `src` that a redaction over `rs` drops:
    /// each whose /Rect meets a padded region, and the /Popup of each such
    /// annotation. An annotation whose /Rect cannot be read is dropped too:
    /// it may hold the secret, and a redaction never keeps what it cannot place.
    fn redacted_annotations(
        &self,
        src: usize,
        rs: &crate::redaction::RegionSet,
        opts: &crate::redaction::RedactionOptions,
    ) -> Result<std::collections::HashSet<u32>> {
        let mut dropped = std::collections::HashSet::new();
        let page_ref = self.source.get_page_ref(src)?;
        let page = self.source.load_object(page_ref)?;
        let Some(annots) = page.as_dict().and_then(|d| d.get("Annots")) else {
            return Ok(dropped);
        };
        let annots = match annots {
            Object::Reference(r) => self.source.load_object(*r)?,
            other => other.clone(),
        };
        let Some(list) = annots.as_array() else {
            return Ok(dropped);
        };
        for entry in list {
            let Some(r) = entry.as_reference() else {
                continue;
            };
            let annot = self.source.load_object(r).ok();
            let dict = annot.as_ref().and_then(|a| a.as_dict());
            let rect = dict.and_then(|d| d.get("Rect")).and_then(Self::rect_of);
            let meets = match rect {
                Some(rect) => rs.regions.iter().any(|reg| reg.intersects_rect(&rect, opts.edge_padding)),
                None => true,
            };
            if !meets {
                continue;
            }
            dropped.insert(r.id);
            if let Some(popup) = dict.and_then(|d| d.get("Popup")).and_then(|p| p.as_reference()) {
                dropped.insert(popup.id);
            }
        }
        Ok(dropped)
    }

    /// A PDF rectangle `[x0 y0 x1 y1]` in page space, or `None` when it is not
    /// four numbers.
    fn rect_of(obj: &Object) -> Option<crate::geometry::Rect> {
        let [x0, y0, x1, y1] = obj.as_array()?.as_slice() else {
            return None;
        };
        let num = |o: &Object| o.as_real().or_else(|| o.as_integer().map(|i| i as f64));
        Some(crate::geometry::Rect::from_points(
            num(x0)? as f32,
            num(y0)? as f32,
            num(x1)? as f32,
            num(y1)? as f32,
        ))
    }

    /// The page's `/Annots` with the annotations in `dropped` removed, or `None`
    /// when none is left. An indirect array is read through the source.
    fn annots_without(
        &self,
        page: &HashMap<String, Object>,
        dropped: &std::collections::HashSet<u32>,
    ) -> Option<Object> {
        let annots = match page.get("Annots")? {
            Object::Reference(r) => self.source.load_object(*r).ok()?,
            other => other.clone(),
        };
        let kept: Vec<Object> = annots
            .as_array()?
            .iter()
            .filter(|a| !a.as_reference().is_some_and(|r| dropped.contains(&r.id)))
            .cloned()
            .collect();
        (!kept.is_empty()).then_some(Object::Array(kept))
    }
    // ── end pdf_manipulator patch ──

    /// Build the [`crate::redaction::FontInfoMetrics`] for a source page
    /// from its (possibly inherited) `/Resources/Font`. A font that fails
    /// to parse is simply omitted — the engine then reports it
    /// non-simple and *refuses* rather than under-redacting.
    fn build_page_font_metrics(
        &mut self,
        source_page: usize,
    ) -> Result<crate::redaction::FontInfoMetrics> {
        // No resources ⇒ empty map ⇒ the engine reports unknown fonts
        // non-simple and *refuses* (fail closed, never under-redact).
        let resources = Self::inherited_resources(&self.source, source_page)?;
        Self::font_metrics_of(&self.source, &resources, None)
    }

    /// Destructively apply every queued redaction (programmatic
    /// rectangles + source `/Redact` annotations): the underlying text
    /// is physically removed from the content stream and an opaque
    /// overlay drawn over the area (ISO 32000-1:2008 §12.5.6.23 — "remove
    /// all traces"). The redacted pages' content is rewritten so that a
    /// subsequent garbage-collected full-rewrite save (the default
    /// [`save_to_bytes`]/[`save`]) leaves no residual recoverable bytes
    /// (G6); the `/Redact` annotations are removed.
    ///
    /// Returns an aggregate [`crate::redaction::RedactionReport`].
    ///
    /// [`save_to_bytes`]: DocumentEditor::save_to_bytes
    /// [`save`]: DocumentEditor::save
    ///
    /// # Errors
    /// - [`Error::Unsupported`] if a redacted page shows text in a
    ///   composite/Type0/unknown font (refused rather than risk a silent
    ///   under-redaction — fail closed).
    /// - [`Error::InvalidPdf`]/[`Error::ParseError`] on an unreadable
    ///   page or content stream.
    pub fn apply_redactions_destructive(
        &mut self,
        opts: crate::redaction::RedactionOptions,
    ) -> Result<crate::redaction::RedactionReport> {
        // ── pdf_manipulator patch: the cancellable twin does the work ──
        self.apply_redactions_destructive_cancellable(opts, &crate::cancel::CancelCheck::none())
    }

    /// [`apply_redactions_destructive`](Self::apply_redactions_destructive)
    /// that stops with the cancelled error once `cancel` reports cancelled.
    /// Every poll comes while pages are planned, before any is recorded, so a
    /// cancel leaves the editor as it was.
    pub fn apply_redactions_destructive_cancellable(
        &mut self,
        opts: crate::redaction::RedactionOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<crate::redaction::RedactionReport> {
        // ── end pdf_manipulator patch ──
        // ── pdf_manipulator patch: every page with a queued mark or a /Redact annotation ──
        // Every page with a queued mark or a /Redact annotation is applied.
        let mut pages: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
        for src in 0..self.source.page_count()? {
            cancel.check()?;
            if self.redaction_regions.contains_key(&src) || !self.get_redaction_data(src)?.is_empty()
            {
                pages.insert(src);
            }
        }

        // Every page is planned before any is recorded, so an error on page N
        // leaves the pages before it as they were.
        let mut planned = Vec::with_capacity(pages.len());
        for src in pages {
            cancel.check()?;
            let rs = self.page_redaction_regions(src, &[])?;
            let plan = self.plan_destruction(src, &rs, &opts, cancel)?;
            planned.push((src, plan));
        }

        let mut total = crate::redaction::RedactionReport::default();
        for (src, plan) in planned {
            let rep = self.record_destruction(src, plan);
            // The queued marks are applied now: they are removed with their content.
            self.redaction_regions.remove(&src);
            total.regions += rep.regions;
            total.glyphs_removed += rep.glyphs_removed;
            total.bytes_removed += rep.bytes_removed;
            total.annotations_removed += rep.annotations_removed;
            total.fonts_scrubbed += rep.fonts_scrubbed;
            total.images_modified += rep.images_modified;
            total.images_removed += rep.images_removed;
            total.paths_pruned += rep.paths_pruned;
            total.xobjects_specialized += rep.xobjects_specialized;
        }
        // ── end pdf_manipulator patch ──
        self.is_modified = true;
        Ok(total)
    }

    /// Destructively apply queued redactions (programmatic rectangles +
    /// source `/Redact` annotations) for a single source page. Shared by
    /// [`apply_redactions_destructive`] (all queued pages) and `save_page`
    /// (DOM edits to pre-existing content on one page).
    ///
    /// [`apply_redactions_destructive`]: DocumentEditor::apply_redactions_destructive
    fn apply_redactions_destructive_for_page(
        &mut self,
        src: usize,
        opts: &crate::redaction::RedactionOptions,
        // ── pdf_manipulator patch: the extra regions a caller brings ──
        extra: &[crate::redaction::RedactionRegion],
        // ── end pdf_manipulator patch ──
    ) -> Result<crate::redaction::RedactionReport> {
        // ── pdf_manipulator patch: delegate to the shared per-page core ──
        let rs = self.page_redaction_regions(src, extra)?;
        let rep = self.destroy_regions_on_page(src, &rs, opts)?;
        // The queued marks are applied now: they are removed with their content.
        self.redaction_regions.remove(&src);
        Ok(rep)
        // ── end pdf_manipulator patch ──
    }

    // ── pdf_manipulator patch: the regions of a page's redaction, read without changing the editor ──
    /// The regions a destructive redaction of source page `src` destroys: its
    /// `/Redact` annotations, the marks queued on it and the `extra` regions
    /// a caller brings (the erasures of a DOM edit).
    fn page_redaction_regions(
        &mut self,
        src: usize,
        extra: &[crate::redaction::RedactionRegion],
    ) -> Result<crate::redaction::RegionSet> {
        use crate::redaction::{RedactionRegion, RegionSet};
        let mut rs = RegionSet::new(src);
        for rd in self.get_redaction_data(src)? {
            rs.push(RedactionRegion::from_rect(
                rd.rect[0],
                rd.rect[1],
                rd.rect[2],
                rd.rect[3],
                Some(rd.color),
            ));
        }
        if let Some(progs) = self.redaction_regions.get(&src) {
            for r in progs {
                rs.push(*r);
            }
        }
        for r in extra {
            rs.push(*r);
        }
        Ok(rs)
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: scoped destructive removal ──

    /// Destroy the given regions on one page: rewrite its content
    /// stream without the covered glyphs and queue the original
    /// /Contents objects for hard-dropping at save. The per-page core
    /// of [`Self::apply_redactions_destructive`], callable with an
    /// explicit region set so an operation can destroy ONLY its own
    /// regions without firing the document's pending redaction marks.
    fn destroy_regions_on_page(
        &mut self,
        src: usize,
        rs: &crate::redaction::RegionSet,
        opts: &crate::redaction::RedactionOptions,
    ) -> Result<crate::redaction::RedactionReport> {
        let plan = self.plan_destruction(src, rs, opts, &crate::cancel::CancelCheck::none())?;
        Ok(self.record_destruction(src, plan))
    }

    /// Works out what destroying `rs` on source page `src` records, without
    /// changing the editor: every refusal of the redaction is raised here, so
    /// [`Self::record_destruction`] cannot fail.
    fn plan_destruction(
        &mut self,
        src: usize,
        rs: &crate::redaction::RegionSet,
        opts: &crate::redaction::RedactionOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<Option<DestructionPlan>> {
        if rs.is_empty() {
            return Ok(None);
        }
        let page_ref = self.source.get_page_ref(src)?;
        let page_obj = self.source.load_object(page_ref)?;
        let has_contents = page_obj.as_dict().and_then(|d| d.get("Contents")).is_some();
        if !has_contents {
            return Ok(None);
        }
        // The pass reads the source's content, so a page redacted before must
        // be redacted again over its earlier regions too, or the new record
        // replaces the old one and the earlier removals come back. The report
        // counts only what this call adds.
        let earlier = self
            .redacted_pages
            .get(&src)
            .map(|p| (p.regions.clone(), p.report, p.dropped_annots.len()));
        let combined;
        let rs = match &earlier {
            Some((regions, _, _)) => {
                let mut all = regions.clone();
                all.regions.extend(rs.regions.iter().copied());
                combined = all;
                &combined
            },
            None => rs,
        };
        // The pass runs over the whole content now, so a refusal (a damaged
        // stream, a font whose glyphs cannot be pruned) is raised here, before
        // anything is recorded. The save runs the same pass and must agree.
        let fonts = self.build_page_font_metrics(src)?;
        let page_resources = Self::inherited_resources(&self.source, src)?;
        let images = Self::image_resources_of(&self.source, &page_resources, None)?;
        let forms = DocForms {
            source: &self.source,
            page_resources: page_resources.clone(),
            flat: None,
        };
        let outcome = Self::run_redaction_pass(
            rs,
            opts,
            &fonts,
            &images,
            &forms,
            self.source.page_operators(src)?.with_cancel_every(cancel.clone(), 256),
            &[],
            None,
            &mut std::io::sink(),
        )?;
        // Each queued copy's source is proved writable now: a source the row writers cannot read (a filter the engine does not
        // decode, a colour space the copy cannot carry) refuses the redaction
        // here, at the dry run: nothing is recorded and the save is never
        // asked to fail halfway with a rejected copy.
        // A placement's /SMask and /Mask stencil are copied too, so they are
        // proved and their originals kept out of the output like the image's.
        // A build without rendering queues no image copy, so there is
        // nothing to prove.
        let mut originals = outcome.originals.clone();
        #[cfg(feature = "rendering")]
        {
            let mut mask_originals = Vec::new();
            self.validate_scope(&outcome.scope, &forms, &page_resources, &mut mask_originals)?;
            originals.extend(mask_originals);
        }
        let dropped_annots = self.redacted_annotations(src, rs, opts)?;
        let mut report = crate::redaction::RedactionReport {
            annotations_removed: dropped_annots.len(),
            ..outcome.report.clone()
        };
        if let Some((_, before, annotations_before)) = &earlier {
            report.regions = report.regions.saturating_sub(before.regions);
            report.glyphs_removed = report.glyphs_removed.saturating_sub(before.glyphs_removed);
            report.images_modified = report.images_modified.saturating_sub(before.images_modified);
            report.images_removed = report.images_removed.saturating_sub(before.images_removed);
            report.paths_pruned = report.paths_pruned.saturating_sub(before.paths_pruned);
            report.xobjects_specialized =
                report.xobjects_specialized.saturating_sub(before.xobjects_specialized);
            report.fonts_scrubbed = report.fonts_scrubbed.saturating_sub(before.fonts_scrubbed);
            report.bytes_removed = report.bytes_removed.saturating_sub(before.bytes_removed);
            report.annotations_removed =
                report.annotations_removed.saturating_sub(*annotations_before);
        }
        // The original /Contents object ids, which the save path hard-drops:
        // the removed content must not survive even as an orphaned, GC-missed
        // object.
        let mut orphans = Vec::new();
        if let Some(d) = page_obj.as_dict() {
            match d.get("Contents") {
                Some(Object::Reference(r)) => orphans.push(r.id),
                Some(Object::Array(arr)) => {
                    for it in arr {
                        if let Object::Reference(r) = it {
                            orphans.push(r.id);
                        }
                    }
                },
                _ => {},
            }
        }
        Ok(Some(DestructionPlan {
            originals,
            orphans,
            report,
            page: RedactedPage {
                regions: rs.clone(),
                opts: *opts,
                fonts,
                report: outcome.report,
                dropped_annots,
                scope: outcome.scope,
                flat: None,
            },
        }))
    }

    /// Records a plan from [`Self::plan_destruction`] and returns its report.
    /// It reads nothing and cannot fail.
    fn record_destruction(
        &mut self,
        src: usize,
        plan: Option<DestructionPlan>,
    ) -> crate::redaction::RedactionReport {
        let Some(plan) = plan else {
            return crate::redaction::RedactionReport::default();
        };
        self.redacted_image_originals.extend(plan.originals);
        self.redacted_orphan_ids.extend(plan.orphans);
        self.redacted_pages.insert(src, plan.page);
        self.apply_redactions_pages.insert(src);
        self.is_modified = true;
        plan.report
    }

    /// Destructively redacts the rectangles `rects` on `page` and only them:
    /// covered glyphs are rewritten out of the content stream and the original
    /// stream objects are hard-dropped at save, with the opaque overlay in
    /// `fill` drawn over each (no overlay at all when `fill` is `None` and
    /// `opts` does not draw one). Unlike [`Self::apply_redactions_destructive`]
    /// this fires only the given rectangles: the marks queued with
    /// [`Self::add_redaction`] and the document's `/Redact` annotations stay
    /// pending, and a later apply of them keeps what this call removed. The
    /// call plans before it records, so an error leaves the editor as it was.
    ///
    /// # Errors
    /// [`Error::InvalidPdf`] if `page` is out of range, [`Error::Unsupported`]
    /// for a merged page or a font whose glyphs cannot be pruned.
    pub fn redact_regions(
        &mut self,
        page: usize,
        rects: &[[f32; 4]],
        fill: Option<[f32; 3]>,
        opts: &crate::redaction::RedactionOptions,
        cancel: &crate::cancel::CancelCheck,
    ) -> Result<crate::redaction::RedactionReport> {
        use crate::redaction::{RedactionRegion, RegionSet};

        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }
        let src = self.source_page_for(page, "redacting regions")?;
        let mut rs = RegionSet::new(src);
        for r in rects {
            rs.push(RedactionRegion::from_rect(r[0], r[1], r[2], r[3], fill));
        }
        let plan = self.plan_destruction(src, &rs, opts, cancel)?;
        let report = self.record_destruction(src, plan);
        self.is_modified = true;
        Ok(report)
    }

    /// Drops every mark queued with [`Self::add_redaction`], on every page. A
    /// `/Redact` annotation the document carries is not a queued mark and
    /// stays.
    pub fn clear_redactions(&mut self) {
        self.redaction_regions.clear();
    }
    // ── end pdf_manipulator patch ──

    /// Standalone document sanitization without geometric redaction
    /// (#231 T10, feature plan §4.6 / §5.1).
    ///
    /// Strips document-level secrets that cosmetic redaction left behind:
    /// the `/Info` dictionary, the catalog XMP `/Metadata` stream,
    /// document JavaScript (`/OpenAction`, `/AA`, `/Names/JavaScript`)
    /// and `/Names/EmbeddedFiles`, subject to the
    /// [`crate::redaction::RedactionOptions`] toggles. The removed object
    /// subtrees are hard-excluded from the
    /// output (G6) so a secret cannot survive even as a GC-missed
    /// orphan, and `/Info` is replaced with an empty dictionary.
    ///
    /// Returns a [`crate::redaction::RedactionReport`]; `bytes_removed`
    /// is a best-effort total of the dropped objects' sizes so callers
    /// can assert the scrub did real work.
    ///
    /// # Errors
    /// [`Error::InvalidPdf`] if the document has no resolvable catalog.
    pub fn sanitize_document(
        &mut self,
        opts: crate::redaction::RedactionOptions,
    ) -> Result<crate::redaction::RedactionReport> {
        use std::collections::{HashSet, VecDeque};

        let catalog_ref = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Root"))
            .and_then(|r| r.as_reference())
            .ok_or_else(|| Error::InvalidPdf("Missing catalog reference".to_string()))?;
        let info_root = self
            .source
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Info"))
            .and_then(|r| r.as_reference())
            .map(|r| r.id);

        let catalog_dict = match self.modified_objects.get(&catalog_ref.id) {
            Some(obj) => obj.clone(),
            None => self.source.catalog()?,
        };
        let catalog_dict = catalog_dict
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Catalog is not a dictionary".to_string()))?
            .clone();

        let scrub = crate::redaction::sanitize_catalog(&catalog_dict, &opts, |id| {
            self.source.load_object(ObjectRef { id, gen: 0 }).ok()
        });
        let counts = scrub.counts;
        let removed_roots = scrub.removed_roots.clone();

        // Stage the scrubbed catalog and the cleared /Info first so the
        // post-scrub reachability set reflects them.
        self.modified_objects
            .insert(catalog_ref.id, Object::Dictionary(scrub.catalog));
        if opts.scrub_metadata {
            self.modified_info = Some(DocumentInfo::default());
        }

        // ── pdf_manipulator patch: the same prune plan the save uses (#261) ──
        let dropped_annots = self.redacted_annotation_ids();
        let reachable = self.collect_reachable_ids(
            &crate::host::page_prune::plan(
                &self.source,
                &self.modified_objects,
                &self.page_order,
                &dropped_annots,
                // the image sources destructive redaction replaced
                &self.redacted_image_originals,
            ),
            None,
            &HashMap::new(),
            &crate::cancel::CancelCheck::none(),
        );
        // ── end pdf_manipulator patch ──

        // Expand the catalog-derived roots → full subtree, excluding
        // anything still reachable after the scrub (a genuinely shared
        // object — e.g. a filespec also referenced by a file-attachment
        // annotation — must not be dropped).
        let mut to_drop: HashSet<u32> = HashSet::new();
        let mut queue: VecDeque<u32> = removed_roots.into_iter().collect();
        let mut bytes_removed: u64 = 0;
        let account = |obj: &Object, b: &mut u64| {
            *b += match obj {
                // ── pdf_manipulator patch: stream bodies are StreamBody ──
                Object::Stream { body, .. } => body.len() + 64,
                // ── end pdf_manipulator patch ──
                _ => 48,
            };
        };
        while let Some(id) = queue.pop_front() {
            if id == 0 || reachable.contains(&id) || !to_drop.insert(id) {
                continue;
            }
            if let Ok(obj) = self.source.load_object(ObjectRef { id, gen: 0 }) {
                account(&obj, &mut bytes_removed);
                fn walk(o: &Object, q: &mut VecDeque<u32>) {
                    match o {
                        Object::Reference(r) => q.push_back(r.id),
                        Object::Array(a) => a.iter().for_each(|x| walk(x, q)),
                        Object::Dictionary(d) => d.values().for_each(|x| walk(x, q)),
                        Object::Stream { dict, .. } => dict.values().for_each(|x| walk(x, q)),
                        _ => {},
                    }
                }
                walk(&obj, &mut queue);
            }
        }

        // The original /Info object is *replaced* by an empty dict, so it
        // must be hard-excluded unconditionally — `collect_reachable_ids`
        // always seeds the source trailer /Info, so the reachable-subtract
        // guard above would wrongly protect the secret-bearing original
        // (same defense-in-depth as redacted content streams, G6).
        if opts.scrub_metadata {
            if let Some(id) = info_root {
                if to_drop.insert(id) {
                    if let Ok(obj) = self.source.load_object(ObjectRef { id, gen: 0 }) {
                        account(&obj, &mut bytes_removed);
                    }
                }
            }
        }

        self.redacted_orphan_ids.extend(to_drop.iter().copied());
        self.is_modified = true;

        Ok(crate::redaction::RedactionReport {
            annotations_removed: counts.total(),
            bytes_removed,
            ..crate::redaction::RedactionReport::default()
        })
    }

    // ── pdf_manipulator patch: the queued marks are the page's redaction state ──
    /// Whether `page` has queued redaction marks (from [`add_redaction`]) that
    /// no [`apply_redactions_destructive`] has removed yet.
    ///
    /// [`add_redaction`]: DocumentEditor::add_redaction
    /// [`apply_redactions_destructive`]: DocumentEditor::apply_redactions_destructive
    pub fn is_page_marked_for_redaction(&self, page: usize) -> bool {
        matches!(self.page_key(page), PageKey::Source(source_page)
            if self.redaction_regions.get(&source_page).is_some_and(|marks| !marks.is_empty()))
    }

    /// Drops the queued redaction marks of `page`. Its /Redact annotations in
    /// the source stay as they are.
    pub fn unmark_page_for_redaction(&mut self, page: usize) {
        if let PageKey::Source(source_page) = self.page_key(page) {
            self.redaction_regions.remove(&source_page);
        }
    }
    // ── end pdf_manipulator patch ──

    // ── pdf_manipulator patch: a queued mark saved as a /Redact annotation ──
    /// The /Redact annotation dictionaries of the marks queued on source page
    /// `src` that no apply has removed: each mark's /Rect, and its /IC when it
    /// has a fill colour.
    fn redaction_mark_annotations(&self, src: usize) -> Vec<HashMap<String, Object>> {
        let Some(marks) = self.redaction_regions.get(&src) else {
            return Vec::new();
        };
        let reals = |values: &[f32]| -> Object {
            Object::Array(values.iter().map(|v| Object::Real(f64::from(*v))).collect())
        };
        marks
            .iter()
            .map(|mark| {
                let mut dict = HashMap::new();
                dict.insert("Type".to_string(), Object::Name("Annot".to_string()));
                dict.insert("Subtype".to_string(), Object::Name("Redact".to_string()));
                dict.insert("Rect".to_string(), reals(&mark.bbox));
                if let Some(fill) = mark.fill {
                    dict.insert("IC".to_string(), reals(&fill));
                }
                dict
            })
            .collect()
    }
    // ── end pdf_manipulator patch ──

    /// Get redaction annotation data for a page.
    ///
    /// Returns a list of redaction areas with their fill colors.
    fn get_redaction_data(&mut self, page: usize) -> Result<Vec<RedactionData>> {
        use crate::annotation_types::AnnotationSubtype;

        let annotations = self.source.get_annotations(page)?;
        let mut redactions = Vec::new();

        for annotation in annotations {
            // ── pdf_manipulator patch: a cancel stops the annotations within one annotation ──
            crate::cancel::poll()?;
            // ── end pdf_manipulator patch ──
            // Only process Redact annotations
            if annotation.subtype_enum != AnnotationSubtype::Redact {
                continue;
            }

            // Get the redaction rectangle
            let rect = match annotation.rect {
                Some(r) => [r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32],
                None => continue,
            };

            // Get interior color (IC entry) - the fill color for the redaction
            // Default to black if not specified
            let color = match &annotation.interior_color {
                Some(color) if color.len() >= 3 => {
                    [color[0] as f32, color[1] as f32, color[2] as f32]
                },
                _ => [0.0, 0.0, 0.0], // Default to black
            };

            // Also handle QuadPoints if present (multiple redaction areas)
            if let Some(ref quad_points) = annotation.quad_points {
                for quad in quad_points {
                    // QuadPoints are 8 values: x1,y1,x2,y2,x3,y3,x4,y4
                    // representing corners in a specific order
                    // Convert to bounding box
                    let xs = [quad[0], quad[2], quad[4], quad[6]];
                    let ys = [quad[1], quad[3], quad[5], quad[7]];

                    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min) as f32;
                    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max) as f32;
                    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min) as f32;
                    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max) as f32;

                    redactions.push(RedactionData {
                        rect: [min_x, min_y, max_x, max_y],
                        color,
                    });
                }
            } else {
                // Just use the main Rect
                redactions.push(RedactionData { rect, color });
            }
        }

        Ok(redactions)
    }

    // ========================================================================
    // Image Repositioning & Resizing
    // ========================================================================

    /// Get information about images on a page.
    ///
    /// Returns a list of images with their names, positions, and sizes.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    ///
    /// # Example
    ///
    /// ```ignore
    /// let images = editor.get_page_images(0)?;
    /// for img in images {
    ///     println!("Image {} at ({}, {}) size {}x{}",
    ///         img.name, img.bounds[0], img.bounds[1],
    ///         img.bounds[2], img.bounds[3]);
    /// }
    /// ```
    pub fn get_page_images(&mut self, page: usize) -> Result<Vec<ImageInfo>> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        // ── pdf_manipulator patch: a merged page has no source page to read ──
        let original_page_idx = self.source_page_for(page, "reading images")?;

        // Get page reference
        let page_ref = self.source.get_page_ref(original_page_idx)?;
        // ── end pdf_manipulator patch ──
        let page_obj = self.source.load_object(page_ref)?;
        let page_dict = page_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Page is not a dictionary".to_string()))?;

        // Get Contents
        let contents = match page_dict.get("Contents") {
            Some(c) => c.clone(),
            None => return Ok(Vec::new()),
        };

        // ── pdf_manipulator patch: the page's content is read as operators, never whole ──
        // The page has contents: its parts are walked in turn, as every page walk does.
        if !matches!(contents, Object::Reference(_) | Object::Array(_)) {
            return Ok(Vec::new());
        }
        let operators = self.source.page_operators(original_page_idx)?;
        // ── end pdf_manipulator patch ──

        // Track CTM through the operators to find images
        let mut images = Vec::new();
        let mut ctm_stack: Vec<[f32; 6]> = vec![[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]]; // Identity
        let mut current_ctm = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];

        // ── pdf_manipulator patch: a cancel stops the page's operators within a stride ──
        let mut stride = crate::cancel::Stride::job();
        // ── end pdf_manipulator patch ──
        for op in operators {
            // ── pdf_manipulator patch: a cancel stops the page's operators within a stride ──
            stride.tick()?;
            let op = op?;
            // ── end pdf_manipulator patch ──
            match op {
                crate::content::operators::Operator::SaveState => {
                    ctm_stack.push(current_ctm);
                },
                crate::content::operators::Operator::RestoreState => {
                    if let Some(saved) = ctm_stack.pop() {
                        current_ctm = saved;
                    }
                },
                crate::content::operators::Operator::Cm { a, b, c, d, e, f } => {
                    // Concatenate transformation matrix
                    // New CTM = [a,b,c,d,e,f] * current_ctm
                    let new_a = a * current_ctm[0] + b * current_ctm[2];
                    let new_b = a * current_ctm[1] + b * current_ctm[3];
                    let new_c = c * current_ctm[0] + d * current_ctm[2];
                    let new_d = c * current_ctm[1] + d * current_ctm[3];
                    let new_e = e * current_ctm[0] + f * current_ctm[2] + current_ctm[4];
                    let new_f = e * current_ctm[1] + f * current_ctm[3] + current_ctm[5];
                    current_ctm = [new_a, new_b, new_c, new_d, new_e, new_f];
                },
                crate::content::operators::Operator::Do { ref name } => {
                    // Check if this is an image XObject (vs Form XObject)
                    // For now, include all XObjects; a more refined implementation
                    // would check the XObject's Subtype
                    let matrix = current_ctm;

                    // Extract position and size from matrix
                    // Standard image matrix: [width, 0, 0, height, x, y]
                    let x = matrix[4];
                    let y = matrix[5];
                    // Width and height from scaling components
                    let width = (matrix[0] * matrix[0] + matrix[1] * matrix[1]).sqrt();
                    let height = (matrix[2] * matrix[2] + matrix[3] * matrix[3]).sqrt();

                    images.push(ImageInfo {
                        name: name.clone(),
                        bounds: [x, y, width, height],
                        matrix,
                    });
                },
                _ => {},
            }
        }

        Ok(images)
    }

    /// Reposition an image on a page.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    /// * `image_name` - The XObject name (e.g., "Im1")
    /// * `x` - New x position
    /// * `y` - New y position
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.reposition_image(0, "Im1", 100.0, 200.0)?;
    /// editor.save("output.pdf")?;
    /// ```
    pub fn reposition_image(
        &mut self,
        page: usize,
        image_name: &str,
        x: f32,
        y: f32,
    ) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "editing an image")?;
        let page_mods = self.image_modifications.entry(source_page).or_default();
        let modification = page_mods
            .entry(image_name.to_string())
            .or_insert(ImageModification {
                x: None,
                y: None,
                width: None,
                height: None,
            });
        modification.x = Some(x);
        modification.y = Some(y);

        self.is_modified = true;
        Ok(())
    }

    /// Resize an image on a page.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    /// * `image_name` - The XObject name (e.g., "Im1")
    /// * `width` - New width
    /// * `height` - New height
    ///
    /// # Example
    ///
    /// ```ignore
    /// editor.resize_image(0, "Im1", 200.0, 150.0)?;
    /// editor.save("output.pdf")?;
    /// ```
    pub fn resize_image(
        &mut self,
        page: usize,
        image_name: &str,
        width: f32,
        height: f32,
    ) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "editing an image")?;
        let page_mods = self.image_modifications.entry(source_page).or_default();
        let modification = page_mods
            .entry(image_name.to_string())
            .or_insert(ImageModification {
                x: None,
                y: None,
                width: None,
                height: None,
            });
        modification.width = Some(width);
        modification.height = Some(height);

        self.is_modified = true;
        Ok(())
    }

    /// Reposition and resize an image on a page.
    ///
    /// # Arguments
    /// * `page` - The zero-based page index
    /// * `image_name` - The XObject name (e.g., "Im1")
    /// * `x` - New x position
    /// * `y` - New y position
    /// * `width` - New width
    /// * `height` - New height
    pub fn set_image_bounds(
        &mut self,
        page: usize,
        image_name: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> Result<()> {
        if page >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!("Page index {} out of range", page)));
        }

        let source_page = self.source_page_for(page, "editing an image")?;
        let page_mods = self.image_modifications.entry(source_page).or_default();
        page_mods.insert(
            image_name.to_string(),
            ImageModification {
                x: Some(x),
                y: Some(y),
                width: Some(width),
                height: Some(height),
            },
        );

        self.is_modified = true;
        Ok(())
    }

    /// Clear image modifications for a page.
    pub fn clear_image_modifications(&mut self, page: usize) {
        self.image_modifications.remove(&page);
    }

    /// Check if a page has image modifications.
    pub fn has_image_modifications(&self, page: usize) -> bool {
        self.image_modifications
            .get(&page)
            .map(|m| !m.is_empty())
            .unwrap_or(false)
    }

    // ── pdf_manipulator patch: image bounds are edited on the belt ──
    /// The page `page_index`'s content with `mods` applied, written to `out`
    /// one operator at a time. The modifications are in page space: they are
    /// made on the composed matrix and mapped back through the enclosing
    /// transform, so an image drawn under `q 1 0 0 1 56 700 cm` moves to the
    /// position asked for and not 56 past it. Any error reaches the caller.
    fn write_image_mods_content(
        source: &PdfDocument,
        page_index: usize,
        mods: &HashMap<String, ImageModification>,
        out: &mut dyn Write,
    ) -> Result<()> {
        let mut ops = source
            .page_operators(page_index)?
            .with_inline_images(crate::host::content::InlineImages::Collect);
        let mut stage = image_mods::ImageModStage::new(mods, out);
        let mut stride = crate::cancel::Stride::job();
        for item in ops.by_ref() {
            stride.tick()?;
            stage.push(&item?)?;
        }
        if !ops.damage().is_clean() {
            return Err(Error::Unsupported(
                "the content stream is damaged; refusing to move an image in a stream whose \
                 operators cannot be read in full"
                    .to_string(),
            ));
        }
        stage.finish()?;
        Ok(())
    }

    /// `content_data` with `modifications` applied: the belt over one buffer,
    /// for the tests that read what it writes.
    #[cfg(test)]
    fn rewrite_content_stream_with_image_mods(
        &self,
        content_data: &[u8],
        modifications: &HashMap<String, ImageModification>,
    ) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        let mut stage = image_mods::ImageModStage::new(modifications, &mut output);
        for item in crate::host::content::Operators::new(content_data, crate::host::content::Mode::Full) {
            stage.push(&item?)?;
        }
        stage.finish()?;
        drop(stage);
        Ok(output)
    }
    // ── end pdf_manipulator patch ──

    /// Serialize an operator to content-stream bytes.
    ///
    /// Delegates to [`crate::redaction::serialize`] — the single source
    /// of truth (DRY, #231 T1). Binary strings are emitted as
    /// hexadecimal per ISO 32000-1 §7.3.4.3 (binary-safe).
    fn serialize_operator(&self, output: &mut Vec<u8>, op: &crate::content::operators::Operator) {
        crate::redaction::serialize::serialize_operator(output, op);
    }

    /// Serialize a PDF object to bytes.
    ///
    /// Delegates to [`crate::redaction::serialize`] (single source of
    /// truth, #231 T1).
    fn serialize_object(&self, output: &mut Vec<u8>, obj: &crate::object::Object) {
        crate::redaction::serialize::serialize_object(output, obj);
    }
}

/// Data for a redaction area.
#[derive(Debug, Clone)]
struct RedactionData {
    /// Redaction rectangle [llx, lly, urx, ury]
    rect: [f32; 4],
    /// Fill color [r, g, b]
    color: [f32; 3],
}

impl EditableDocument for DocumentEditor {
    fn get_info(&mut self) -> Result<DocumentInfo> {
        // Return modified info if available
        if let Some(ref info) = self.modified_info {
            return Ok(info.clone());
        }

        // Otherwise, load from source document
        let trailer = self.source.trailer();
        if let Some(trailer_dict) = trailer.as_dict() {
            if let Some(info_ref) = trailer_dict.get("Info").and_then(|i| i.as_reference()) {
                let info_obj = self.source.load_object(info_ref)?;
                return Ok(DocumentInfo::from_object(&info_obj));
            }
        }

        // No Info dictionary
        Ok(DocumentInfo::default())
    }

    fn set_info(&mut self, info: DocumentInfo) -> Result<()> {
        self.modified_info = Some(info);
        self.is_modified = true;
        Ok(())
    }

    fn page_count(&mut self) -> Result<usize> {
        Ok(self.current_page_count())
    }

    fn get_page_info(&mut self, index: usize) -> Result<PageInfo> {
        // ── pdf_manipulator patch: a merged page reports its merged document's facts ──
        if let PageKey::Merged(merged) = self.page_key(index) {
            let [llx, lly, urx, ury] = self.get_page_media_box(index)?;
            let rotation = self.get_page_rotation(index)?;
            let page_id = self
                .merged_page(merged)
                .map(|(_, page)| page.page_id)
                .ok_or_else(|| Error::InvalidPdf(format!("Page index {} out of range", index)))?;
            return Ok(PageInfo {
                index,
                width: urx - llx,
                height: ury - lly,
                rotation,
                object_ref: ObjectRef::new(page_id, 0),
            });
        }
        // ── end pdf_manipulator patch ──
        let page_refs = self.get_page_refs()?;

        if index >= page_refs.len() {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of range (document has {} pages)",
                index,
                page_refs.len()
            )));
        }

        let page_ref = page_refs[index];
        let page_obj = self.source.load_object(page_ref)?;
        let page_dict = page_obj
            .as_dict()
            .ok_or_else(|| Error::InvalidPdf("Page is not a dictionary".to_string()))?;

        // Get MediaBox for dimensions
        let (width, height) = if let Some(media_box) = page_dict.get("MediaBox") {
            self.parse_media_box(media_box)?
        } else {
            // Try to inherit from parent
            (612.0, 792.0) // Default to Letter size
        };

        let rotation = page_dict
            .get("Rotate")
            .and_then(|r| r.as_integer())
            .unwrap_or(0) as i32;

        Ok(PageInfo {
            index,
            width,
            height,
            rotation,
            object_ref: page_ref,
        })
    }

    fn remove_page(&mut self, index: usize) -> Result<()> {
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of range (document has {} pages)",
                index,
                self.current_page_count()
            )));
        }

        // ── pdf_manipulator patch: a removed page, merged or source, leaves the page order ──
        self.page_order.remove(index);
        // ── end pdf_manipulator patch ──

        self.is_modified = true;
        Ok(())
    }

    fn move_page(&mut self, from: usize, to: usize) -> Result<()> {
        let count = self.current_page_count();
        if from >= count || to >= count {
            return Err(Error::InvalidPdf(format!(
                "Page index out of range (document has {} pages)",
                count
            )));
        }

        // ── pdf_manipulator patch: merged pages move like source pages ──
        let moved = self.page_order.remove(from);
        self.page_order.insert(to, moved);
        // ── end pdf_manipulator patch ──
        self.is_modified = true;
        Ok(())
    }

    fn duplicate_page(&mut self, index: usize) -> Result<usize> {
        if index >= self.current_page_count() {
            return Err(Error::InvalidPdf(format!(
                "Page index {} out of range (document has {} pages)",
                index,
                self.current_page_count()
            )));
        }

        // ── pdf_manipulator patch: a merged page is listed twice like a source page ──
        self.page_order.push(self.page_order[index]);
        // ── end pdf_manipulator patch ──
        self.is_modified = true;

        Ok(self.current_page_count() - 1)
    }

    fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.save_with_options(path, SaveOptions::full_rewrite())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_with_options(&mut self, path: impl AsRef<Path>, options: SaveOptions) -> Result<()> {
        if options.incremental {
            // ── pdf_manipulator patch: the incremental save takes the options the host writer reads ──
            self.write_incremental(path, &options)
            // ── end pdf_manipulator patch ──
        } else {
            self.write_full(path, &options)
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn save_with_options(&mut self, _path: impl AsRef<Path>, _options: SaveOptions) -> Result<()> {
        Err(Error::InvalidPdf(
            "Filesystem save is not available in WASM. Use save_to_bytes() instead.".to_string(),
        ))
    }
}

impl DocumentEditor {
    /// Parse a MediaBox array into (width, height).
    fn parse_media_box(&self, media_box: &Object) -> Result<(f32, f32)> {
        if let Some(arr) = media_box.as_array() {
            if arr.len() >= 4 {
                let llx = arr[0]
                    .as_real()
                    .or_else(|| arr[0].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0);
                let lly = arr[1]
                    .as_real()
                    .or_else(|| arr[1].as_integer().map(|i| i as f64))
                    .unwrap_or(0.0);
                let urx = arr[2]
                    .as_real()
                    .or_else(|| arr[2].as_integer().map(|i| i as f64))
                    .unwrap_or(612.0);
                let ury = arr[3]
                    .as_real()
                    .or_else(|| arr[3].as_integer().map(|i| i as f64))
                    .unwrap_or(792.0);

                return Ok(((urx - llx) as f32, (ury - lly) as f32));
            }
        }

        // Default to Letter size
        Ok((612.0, 792.0))
    }

    /// Generate a content stream from a StructureElement with marked content wrapping.
    ///
    /// This is used when writing modified structure elements back to a PDF.
    /// Wraps each element in BDC/EMC (Begin/End Marked Content) operators for tagged PDF support.
    ///
    /// Returns the content stream bytes and any pending images that need XObject registration.
    ///
    /// # PDF Spec Compliance
    ///
    /// - ISO 32000-1:2008, Section 14.7.4 - Marked Content Sequences
    fn generate_content_stream(
        &self,
        elem: &StructureElement,
    ) -> Result<(Vec<u8>, Vec<crate::writer::PendingImage>)> {
        let mut builder = ContentStreamBuilder::new();
        builder.add_structure_element(elem);
        let bytes = builder.build()?;
        let pending_images = builder.take_pending_images();
        Ok((bytes, pending_images))
    }

    /// Build an XObject stream from ImageContent.
    ///
    /// Creates a PDF Image XObject suitable for embedding in a PDF.
    /// Per PDF spec Section 8.9, images are represented as XObject streams.
    fn build_image_xobject(image: &crate::elements::ImageContent) -> Object {
        use crate::elements::{ColorSpace as ElemColorSpace, ImageFormat as ElemImageFormat};

        let mut dict = HashMap::new();

        dict.insert("Type".to_string(), Object::Name("XObject".to_string()));
        dict.insert("Subtype".to_string(), Object::Name("Image".to_string()));
        dict.insert("Width".to_string(), Object::Integer(image.width as i64));
        dict.insert("Height".to_string(), Object::Integer(image.height as i64));
        dict.insert(
            "BitsPerComponent".to_string(),
            Object::Integer(image.bits_per_component as i64),
        );

        // Map color space
        let color_space_name = match image.color_space {
            ElemColorSpace::Gray => "DeviceGray",
            ElemColorSpace::RGB => "DeviceRGB",
            ElemColorSpace::CMYK => "DeviceCMYK",
            ElemColorSpace::Indexed => "Indexed",
            ElemColorSpace::Lab => "Lab",
        };
        dict.insert("ColorSpace".to_string(), Object::Name(color_space_name.to_string()));

        // Set filter based on image format
        match image.format {
            ElemImageFormat::Jpeg => {
                dict.insert("Filter".to_string(), Object::Name("DCTDecode".to_string()));
            },
            ElemImageFormat::Png | ElemImageFormat::Raw => {
                dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
            },
            ElemImageFormat::Jpeg2000 => {
                dict.insert("Filter".to_string(), Object::Name("JPXDecode".to_string()));
            },
            ElemImageFormat::Jbig2 => {
                dict.insert("Filter".to_string(), Object::Name("JBIG2Decode".to_string()));
            },
            ElemImageFormat::Unknown => {
                // No filter for unknown format (raw data)
            },
        }

        dict.insert("Length".to_string(), Object::Integer(image.data.len() as i64));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        Object::stream(dict, image.data.clone())
        // ── end pdf_manipulator patch ──
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_info_builder() {
        let info = DocumentInfo::new()
            .title("Test Document")
            .author("Test Author")
            .subject("Test Subject")
            .keywords("test, rust, pdf");

        assert_eq!(info.title, Some("Test Document".to_string()));
        assert_eq!(info.author, Some("Test Author".to_string()));
        assert_eq!(info.subject, Some("Test Subject".to_string()));
        assert_eq!(info.keywords, Some("test, rust, pdf".to_string()));
    }

    #[test]
    fn test_document_info_to_object() {
        let info = DocumentInfo::new().title("My PDF").author("John Doe");

        let obj = info.to_object();
        let dict = obj.as_dict().unwrap();

        assert!(dict.contains_key("Title"));
        assert!(dict.contains_key("Author"));
        assert!(!dict.contains_key("Subject"));
    }

    #[test]
    fn test_document_info_from_object() {
        let mut dict = HashMap::new();
        dict.insert("Title".to_string(), Object::String(b"Test Title".to_vec()));
        dict.insert("Author".to_string(), Object::String(b"Test Author".to_vec()));

        let obj = Object::Dictionary(dict);
        let info = DocumentInfo::from_object(&obj);

        assert_eq!(info.title, Some("Test Title".to_string()));
        assert_eq!(info.author, Some("Test Author".to_string()));
        assert_eq!(info.subject, None);
    }

    #[test]
    fn test_save_options() {
        let full = SaveOptions::full_rewrite();
        assert!(!full.incremental);
        assert!(full.compress);
        assert!(full.garbage_collect);

        let inc = SaveOptions::incremental();
        assert!(inc.incremental);
        assert!(!inc.compress);
        assert!(!inc.garbage_collect);
    }

    // =========================================================================
    // DocumentInfo: additional coverage
    // =========================================================================

    #[test]
    fn test_document_info_default_is_all_none() {
        let info = DocumentInfo::default();
        assert_eq!(info.title, None);
        assert_eq!(info.author, None);
        assert_eq!(info.subject, None);
        assert_eq!(info.keywords, None);
        assert_eq!(info.creator, None);
        assert_eq!(info.producer, None);
        assert_eq!(info.creation_date, None);
        assert_eq!(info.mod_date, None);
    }

    #[test]
    fn test_document_info_creator_producer() {
        let info = DocumentInfo::new()
            .creator("pdf_oxide")
            .producer("Rust PDF Library");
        assert_eq!(info.creator, Some("pdf_oxide".to_string()));
        assert_eq!(info.producer, Some("Rust PDF Library".to_string()));
    }

    #[test]
    fn test_document_info_to_object_all_fields() {
        let info = DocumentInfo {
            title: Some("Title".into()),
            author: Some("Author".into()),
            subject: Some("Subject".into()),
            keywords: Some("Keywords".into()),
            creator: Some("Creator".into()),
            producer: Some("Producer".into()),
            creation_date: Some("D:20260101000000".into()),
            mod_date: Some("D:20260226000000".into()),
        };

        let obj = info.to_object();
        let dict = obj.as_dict().unwrap();

        assert_eq!(dict.len(), 8);
        assert!(dict.contains_key("Title"));
        assert!(dict.contains_key("Author"));
        assert!(dict.contains_key("Subject"));
        assert!(dict.contains_key("Keywords"));
        assert!(dict.contains_key("Creator"));
        assert!(dict.contains_key("Producer"));
        assert!(dict.contains_key("CreationDate"));
        assert!(dict.contains_key("ModDate"));
    }

    #[test]
    fn test_document_info_to_object_empty() {
        let info = DocumentInfo::default();
        let obj = info.to_object();
        let dict = obj.as_dict().unwrap();
        assert!(dict.is_empty());
    }

    #[test]
    fn test_document_info_roundtrip() {
        let original = DocumentInfo {
            title: Some("My Title".into()),
            author: Some("My Author".into()),
            subject: Some("My Subject".into()),
            keywords: Some("a, b, c".into()),
            creator: Some("TestCreator".into()),
            producer: Some("TestProducer".into()),
            creation_date: Some("D:20260101".into()),
            mod_date: Some("D:20260226".into()),
        };

        let obj = original.to_object();
        let reconstructed = DocumentInfo::from_object(&obj);

        assert_eq!(original.title, reconstructed.title);
        assert_eq!(original.author, reconstructed.author);
        assert_eq!(original.subject, reconstructed.subject);
        assert_eq!(original.keywords, reconstructed.keywords);
        assert_eq!(original.creator, reconstructed.creator);
        assert_eq!(original.producer, reconstructed.producer);
        assert_eq!(original.creation_date, reconstructed.creation_date);
        assert_eq!(original.mod_date, reconstructed.mod_date);
    }

    #[test]
    fn test_document_info_from_non_dict() {
        // from_object on a non-dict should return all None
        let obj = Object::Integer(42);
        let info = DocumentInfo::from_object(&obj);
        assert_eq!(info.title, None);
        assert_eq!(info.author, None);
    }

    #[test]
    fn test_document_info_from_dict_with_wrong_types() {
        // Values that are not Object::String should be ignored
        let mut dict = HashMap::new();
        dict.insert("Title".to_string(), Object::Integer(123));
        dict.insert("Author".to_string(), Object::Boolean(true));

        let obj = Object::Dictionary(dict);
        let info = DocumentInfo::from_object(&obj);
        assert_eq!(info.title, None);
        assert_eq!(info.author, None);
    }

    // =========================================================================
    // SaveOptions: additional coverage
    // =========================================================================

    #[test]
    fn test_save_options_default() {
        let opts = SaveOptions::default();
        assert!(!opts.incremental);
        assert!(!opts.compress);
        // ── pdf_manipulator patch: the level defaults to 9 ──
        assert_eq!(opts.deflate_level, 9);
        // ── end pdf_manipulator patch ──
        assert!(!opts.garbage_collect);
        assert!(opts.encryption.is_none());
    }

    #[test]
    fn test_save_options_with_encryption() {
        let config = EncryptionConfig::new("user", "owner");
        let opts = SaveOptions::with_encryption(config);
        assert!(!opts.incremental);
        assert!(opts.compress);
        assert!(opts.garbage_collect);
        assert!(opts.encryption.is_some());
    }

    // =========================================================================
    // EncryptionAlgorithm
    // =========================================================================

    #[test]
    fn test_encryption_algorithm_default() {
        let algo = EncryptionAlgorithm::default();
        assert_eq!(algo, EncryptionAlgorithm::Aes256);
    }

    #[test]
    fn test_encryption_algorithm_variants() {
        let _ = EncryptionAlgorithm::Rc4_40;
        let _ = EncryptionAlgorithm::Rc4_128;
        let _ = EncryptionAlgorithm::Aes128;
        let _ = EncryptionAlgorithm::Aes256;
    }

    // =========================================================================
    // Permissions
    // =========================================================================

    #[test]
    fn test_permissions_all() {
        let perms = Permissions::all();
        assert!(perms.print);
        assert!(perms.print_high_quality);
        assert!(perms.modify);
        assert!(perms.copy);
        assert!(perms.annotate);
        assert!(perms.fill_forms);
        assert!(perms.accessibility);
        assert!(perms.assemble);
    }

    #[test]
    fn test_permissions_read_only() {
        let perms = Permissions::read_only();
        assert!(!perms.print);
        assert!(!perms.print_high_quality);
        assert!(!perms.modify);
        assert!(!perms.copy);
        assert!(!perms.annotate);
        assert!(!perms.fill_forms);
        assert!(perms.accessibility); // Always true for compliance
        assert!(!perms.assemble);
    }

    #[test]
    fn test_permissions_to_bits_all() {
        let perms = Permissions::all();
        let bits = perms.to_bits();

        // All permission bits should be set
        assert!(bits & (1 << 2) != 0); // print
        assert!(bits & (1 << 3) != 0); // modify
        assert!(bits & (1 << 4) != 0); // copy
        assert!(bits & (1 << 5) != 0); // annotate
        assert!(bits & (1 << 8) != 0); // fill_forms
        assert!(bits & (1 << 9) != 0); // accessibility
        assert!(bits & (1 << 10) != 0); // assemble
        assert!(bits & (1 << 11) != 0); // print_high_quality
    }

    #[test]
    fn test_permissions_to_bits_read_only() {
        let perms = Permissions::read_only();
        let bits = perms.to_bits();

        // Only accessibility should be set (plus reserved bits)
        assert!(bits & (1 << 2) == 0); // print not set
        assert!(bits & (1 << 3) == 0); // modify not set
        assert!(bits & (1 << 4) == 0); // copy not set
        assert!(bits & (1 << 5) == 0); // annotate not set
        assert!(bits & (1 << 8) == 0); // fill_forms not set
        assert!(bits & (1 << 9) != 0); // accessibility is set
        assert!(bits & (1 << 10) == 0); // assemble not set
        assert!(bits & (1 << 11) == 0); // print_high_quality not set
    }

    #[test]
    fn test_permissions_to_bits_reserved_bits() {
        // Reserved bits 7-8 (0-indexed: 6-7) and 13-32 (0-indexed: 12-31) must be 1
        let perms = Permissions::default();
        let bits = perms.to_bits();

        // Bits 6 and 7 must be set
        assert!(bits & (1 << 6) != 0);
        assert!(bits & (1 << 7) != 0);

        // Bits 12-31 must be set
        for bit in 12..32 {
            assert!(bits & (1 << bit) != 0, "bit {} should be set", bit);
        }
    }

    #[test]
    fn test_permissions_to_bits_individual_flags() {
        // Test each flag individually
        let mut perms = Permissions::default();
        let base_bits = perms.to_bits();

        perms.print = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 2));

        perms = Permissions::default();
        perms.modify = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 3));

        perms = Permissions::default();
        perms.copy = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 4));

        perms = Permissions::default();
        perms.annotate = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 5));

        perms = Permissions::default();
        perms.fill_forms = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 8));

        perms = Permissions::default();
        perms.assemble = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 10));

        perms = Permissions::default();
        perms.print_high_quality = true;
        assert_eq!(perms.to_bits(), base_bits | (1 << 11));
    }

    // =========================================================================
    // EncryptionConfig
    // =========================================================================

    #[test]
    fn test_encryption_config_new() {
        let config = EncryptionConfig::new("user_pw", "owner_pw");
        assert_eq!(config.user_password, "user_pw");
        assert_eq!(config.owner_password, "owner_pw");
        assert_eq!(config.algorithm, EncryptionAlgorithm::Aes256);
        // Permissions should be all by default
        assert!(config.permissions.print);
        assert!(config.permissions.copy);
    }

    #[test]
    fn test_encryption_config_default() {
        let config = EncryptionConfig::default();
        assert!(config.user_password.is_empty());
        assert!(config.owner_password.is_empty());
        assert_eq!(config.algorithm, EncryptionAlgorithm::Aes256);
    }

    #[test]
    fn test_encryption_config_with_algorithm() {
        let config = EncryptionConfig::new("u", "o").with_algorithm(EncryptionAlgorithm::Aes128);
        assert_eq!(config.algorithm, EncryptionAlgorithm::Aes128);
    }

    #[test]
    fn test_encryption_config_with_permissions() {
        let config = EncryptionConfig::new("u", "o").with_permissions(Permissions::read_only());
        assert!(!config.permissions.print);
        assert!(config.permissions.accessibility);
    }

    // =========================================================================
    // ModifiedPageProps
    // =========================================================================

    #[test]
    fn test_modified_page_props_default() {
        let props = ModifiedPageProps::default();
        assert!(props.rotation.is_none());
        assert!(props.media_box.is_none());
        assert!(props.crop_box.is_none());
    }

    #[test]
    fn test_modified_page_props_with_values() {
        let props = ModifiedPageProps {
            rotation: Some(90),
            media_box: Some([0.0, 0.0, 612.0, 792.0]),
            crop_box: Some([10.0, 10.0, 602.0, 782.0]),
            ..Default::default()
        };
        assert_eq!(props.rotation, Some(90));
        assert_eq!(props.media_box, Some([0.0, 0.0, 612.0, 792.0]));
        assert_eq!(props.crop_box, Some([10.0, 10.0, 602.0, 782.0]));
    }

    // =========================================================================
    // apply_page_props_to_object (private, tested from within module)
    // =========================================================================

    /// Helper to create a minimal DocumentEditor for internal method testing.
    /// Returns a DocumentEditor from a minimal valid PDF.
    fn create_test_editor() -> DocumentEditor {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let pdf_bytes = minimal_pdf_bytes();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp =
            std::env::temp_dir().join(format!("pdf_oxide_test_{}_{id}.pdf", std::process::id()));
        std::fs::write(&tmp, &pdf_bytes).unwrap();
        let editor = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        editor
    }

    /// Generates bytes for a minimal valid PDF with one page.
    fn minimal_pdf_bytes() -> Vec<u8> {
        // ── pdf_manipulator patch: startxref recomputed to the true xref
        // offset — the upstream constant pointed a byte-count past the
        // xref keyword, so every open went through reconstruction ──
        let pdf = b"%PDF-1.4\n\
            1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
            2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
            3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
            xref\n\
            0 4\n\
            0000000000 65535 f \n\
            0000000009 00000 n \n\
            0000000058 00000 n \n\
            0000000115 00000 n \n\
            trailer\n<< /Size 4 /Root 1 0 R >>\n\
            startxref\n186\n%%EOF\n";
        // ── end pdf_manipulator patch ──
        pdf.to_vec()
    }

    #[test]
    fn test_apply_page_props_rotation() {
        let editor = create_test_editor();

        let mut page_dict = HashMap::new();
        page_dict.insert("Type".to_string(), Object::Name("Page".to_string()));
        let page_obj = Object::Dictionary(page_dict);

        let props = ModifiedPageProps {
            rotation: Some(90),
            media_box: None,
            crop_box: None,
            ..Default::default()
        };

        let result = editor
            .apply_page_props_to_object(&page_obj, &props)
            .unwrap();
        let dict = result.as_dict().unwrap();
        assert_eq!(dict.get("Rotate").unwrap().as_integer().unwrap(), 90);
    }

    #[test]
    fn test_apply_page_props_media_box() {
        let editor = create_test_editor();

        let mut page_dict = HashMap::new();
        page_dict.insert("Type".to_string(), Object::Name("Page".to_string()));
        let page_obj = Object::Dictionary(page_dict);

        let props = ModifiedPageProps {
            rotation: None,
            media_box: Some([0.0, 0.0, 300.0, 400.0]),
            crop_box: None,
            ..Default::default()
        };

        let result = editor
            .apply_page_props_to_object(&page_obj, &props)
            .unwrap();
        let dict = result.as_dict().unwrap();
        let mb = dict.get("MediaBox").unwrap().as_array().unwrap();
        assert_eq!(mb.len(), 4);
    }

    #[test]
    fn test_apply_page_props_crop_box() {
        let editor = create_test_editor();

        let mut page_dict = HashMap::new();
        page_dict.insert("Type".to_string(), Object::Name("Page".to_string()));
        let page_obj = Object::Dictionary(page_dict);

        let props = ModifiedPageProps {
            rotation: None,
            media_box: None,
            crop_box: Some([10.0, 10.0, 602.0, 782.0]),
            ..Default::default()
        };

        let result = editor
            .apply_page_props_to_object(&page_obj, &props)
            .unwrap();
        let dict = result.as_dict().unwrap();
        assert!(dict.contains_key("CropBox"));
    }

    #[test]
    fn test_apply_page_props_all_at_once() {
        let editor = create_test_editor();

        let mut page_dict = HashMap::new();
        page_dict.insert("Type".to_string(), Object::Name("Page".to_string()));
        let page_obj = Object::Dictionary(page_dict);

        let props = ModifiedPageProps {
            rotation: Some(180),
            media_box: Some([0.0, 0.0, 500.0, 700.0]),
            crop_box: Some([20.0, 20.0, 480.0, 680.0]),
            ..Default::default()
        };

        let result = editor
            .apply_page_props_to_object(&page_obj, &props)
            .unwrap();
        let dict = result.as_dict().unwrap();
        assert!(dict.contains_key("Rotate"));
        assert!(dict.contains_key("MediaBox"));
        assert!(dict.contains_key("CropBox"));
    }

    #[test]
    fn test_apply_page_props_non_dict_error() {
        let editor = create_test_editor();
        let page_obj = Object::Integer(42);
        let props = ModifiedPageProps::default();

        let result = editor.apply_page_props_to_object(&page_obj, &props);
        assert!(result.is_err());
    }

    // =========================================================================
    // generate_erase_overlay
    // =========================================================================

    #[test]
    fn test_generate_erase_overlay_no_regions() {
        let editor = create_test_editor();
        // No regions added for page 0
        let result = editor.generate_erase_overlay(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_erase_overlay_empty_regions() {
        let mut editor = create_test_editor();
        // Insert an empty vec for page 5
        editor.erase_regions.insert(5, vec![]);
        let result = editor.generate_erase_overlay(5);
        assert!(result.is_none());
    }

    #[test]
    fn test_generate_erase_overlay_single_region() {
        let mut editor = create_test_editor();
        editor
            .erase_regions
            .insert(0, vec![[10.0, 20.0, 100.0, 200.0]]);

        let content = editor.generate_erase_overlay(0).unwrap();
        let content_str = String::from_utf8(content).unwrap();

        assert!(content_str.contains("q\n"));
        assert!(content_str.contains("1 1 1 rg\n"));
        assert!(content_str.contains("re f\n"));
        assert!(content_str.contains("Q\n"));
    }

    #[test]
    fn test_generate_erase_overlay_multiple_regions() {
        let mut editor = create_test_editor();
        editor
            .erase_regions
            .insert(0, vec![[10.0, 20.0, 100.0, 200.0], [300.0, 400.0, 500.0, 600.0]]);

        let content = editor.generate_erase_overlay(0).unwrap();
        let content_str = String::from_utf8(content).unwrap();

        // Should contain two "re f" operations
        assert_eq!(content_str.matches("re f\n").count(), 2);
    }

    // =========================================================================
    // generate_flatten_overlay
    // =========================================================================

    #[test]
    fn test_generate_flatten_overlay_empty() {
        let editor = create_test_editor();
        let content = editor.generate_flatten_overlay(&[], &[]);
        assert!(content.is_empty());
    }

    #[test]
    fn test_generate_flatten_overlay_single_appearance() {
        let editor = create_test_editor();

        let appearance = AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from_static(b"test content")),
            bbox: [0.0, 0.0, 100.0, 50.0],
            annot_rect: [10.0, 20.0, 210.0, 70.0],
            matrix: None,
            resources: None,
            extra_objects: Vec::new(),
        };

        let names = vec!["FlatAnnot0".to_string()];
        let content = editor.generate_flatten_overlay(&[appearance], &names);
        let content_str = String::from_utf8(content).unwrap();

        assert!(content_str.contains("q\n"));
        assert!(content_str.contains("cm\n"));
        assert!(content_str.contains("/FlatAnnot0 Do\n"));
        assert!(content_str.contains("Q\n"));
    }

    #[test]
    fn test_generate_flatten_overlay_with_matrix() {
        let editor = create_test_editor();

        let appearance = AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from_static(b"test")),
            bbox: [0.0, 0.0, 100.0, 100.0],
            annot_rect: [0.0, 0.0, 100.0, 100.0],
            matrix: Some([1.0, 0.0, 0.0, 1.0, 5.0, 5.0]),
            resources: None,
            extra_objects: Vec::new(),
        };

        let names = vec!["Xobj0".to_string()];
        let content = editor.generate_flatten_overlay(&[appearance], &names);
        let content_str = String::from_utf8(content).unwrap();

        // ── pdf_manipulator patch: §12.5.5 placement expectations ──
        // Exactly ONE cm: the placement matrix A. The form's /Matrix
        // is applied by the XObject itself — emitting it here too
        // double-shifts translated appearances off the page.
        assert_eq!(content_str.matches("cm\n").count(), 1);
        // BBox [0,0,100,100] through translate(5,5) → [5,5,105,105];
        // mapping that onto Rect [0,0,100,100] compensates with -5.
        assert!(
            content_str.contains("1.000000 0 0 1.000000 -5.000000 -5.000000 cm"),
            "placement must map the MATRIX-TRANSFORMED BBox to Rect: {content_str}"
        );
        // ── end pdf_manipulator patch ──
    }

    #[test]
    fn test_generate_flatten_overlay_zero_size_bbox() {
        let editor = create_test_editor();

        let appearance = AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from_static(b"empty")),
            bbox: [0.0, 0.0, 0.0, 0.0], // zero-size
            annot_rect: [10.0, 20.0, 110.0, 120.0],
            matrix: None,
            resources: None,
            extra_objects: Vec::new(),
        };

        let names = vec!["Xobj0".to_string()];
        let content = editor.generate_flatten_overlay(&[appearance], &names);
        let content_str = String::from_utf8(content).unwrap();

        // Should still generate content even with zero bbox (uses 1.0 fallback for scale)
        assert!(content_str.contains("/Xobj0 Do\n"));
    }

    // =========================================================================
    // serialize_operator
    // =========================================================================

    #[test]
    fn test_serialize_operator_save_restore_state() {
        let editor = create_test_editor();
        let mut output = Vec::new();

        editor.serialize_operator(&mut output, &crate::content::operators::Operator::SaveState);
        assert_eq!(&output, b"q\n");

        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::RestoreState);
        assert_eq!(&output, b"Q\n");
    }

    #[test]
    fn test_serialize_operator_cm() {
        let editor = create_test_editor();
        let mut output = Vec::new();

        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Cm {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 10.0,
                f: 20.0,
            },
        );

        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("1.0"));
        assert!(s.contains("10.0"));
        assert!(s.contains("20.0"));
        assert!(s.ends_with("cm\n"));
    }

    #[test]
    fn test_serialize_operator_text_operations() {
        let editor = create_test_editor();

        // BT
        let mut output = Vec::new();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::BeginText);
        assert_eq!(&output, b"BT\n");

        // ET
        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::EndText);
        assert_eq!(&output, b"ET\n");

        // Tf
        output.clear();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Tf {
                font: "Helv".to_string(),
                size: 12.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("/Helv"));
        assert!(s.contains("Tf\n"));
    }

    #[test]
    fn test_serialize_operator_td() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Td { tx: 5.0, ty: 10.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("Td\n"));
    }

    #[test]
    fn test_serialize_operator_tj() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Tj {
                text: b"Hello World".to_vec(),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("Hello World"));
        assert!(s.ends_with("Tj\n"));
    }

    #[test]
    fn test_serialize_operator_tj_with_escapes() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Tj {
                text: b"Hello (World) \\ test".to_vec(),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("\\("));
        assert!(s.contains("\\)"));
        assert!(s.contains("\\\\"));
    }

    #[test]
    fn test_serialize_operator_tj_array() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::TJ {
                array: vec![
                    crate::content::operators::TextElement::String(b"AB".to_vec()),
                    crate::content::operators::TextElement::Offset(-120.0),
                    crate::content::operators::TextElement::String(b"CD".to_vec()),
                ],
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.starts_with("["));
        assert!(s.contains("(AB)"));
        assert!(s.contains("(CD)"));
        assert!(s.ends_with("TJ\n"));
    }

    #[test]
    fn test_serialize_operator_do() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Do {
                name: "Im1".to_string(),
            },
        );
        assert_eq!(&output, b"/Im1 Do\n");
    }

    #[test]
    fn test_serialize_operator_color_ops() {
        let editor = create_test_editor();

        // SetFillRgb
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillRgb {
                r: 1.0,
                g: 0.0,
                b: 0.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("rg\n"));

        // SetStrokeGray
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeGray { gray: 0.5 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("G\n"));

        // SetFillCmyk
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillCmyk {
                c: 0.1,
                m: 0.2,
                y: 0.3,
                k: 0.4,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("k\n"));
    }

    #[test]
    fn test_serialize_operator_path_ops() {
        let editor = create_test_editor();

        // Stroke
        let mut output = Vec::new();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::Stroke);
        assert_eq!(&output, b"S\n");

        // Fill
        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::Fill);
        assert_eq!(&output, b"f\n");

        // FillEvenOdd
        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::FillEvenOdd);
        assert_eq!(&output, b"f*\n");

        // EndPath
        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::EndPath);
        assert_eq!(&output, b"n\n");

        // ClosePath
        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::ClosePath);
        assert_eq!(&output, b"h\n");

        // CloseFillStroke
        output.clear();
        editor
            .serialize_operator(&mut output, &crate::content::operators::Operator::CloseFillStroke);
        assert_eq!(&output, b"b\n");
    }

    #[test]
    fn test_serialize_operator_clipping() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::ClipNonZero);
        assert_eq!(&output, b"W\n");

        output.clear();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::ClipEvenOdd);
        assert_eq!(&output, b"W*\n");
    }

    #[test]
    fn test_serialize_operator_rectangle() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Rectangle {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("re\n"));
    }

    #[test]
    fn test_serialize_operator_marked_content() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::BeginMarkedContent {
                tag: "P".to_string(),
            },
        );
        assert_eq!(&output, b"/P BMC\n");

        output.clear();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::EndMarkedContent,
        );
        assert_eq!(&output, b"EMC\n");
    }

    #[test]
    fn test_serialize_operator_set_line_width() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetLineWidth { width: 2.5 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("w\n"));
    }

    #[test]
    fn test_serialize_operator_set_dash() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetDash {
                array: vec![3.0, 2.0],
                phase: 0.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.starts_with("["));
        assert!(s.ends_with("d\n"));
    }

    #[test]
    fn test_serialize_operator_tstar() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(&mut output, &crate::content::operators::Operator::TStar);
        assert_eq!(&output, b"T*\n");
    }

    #[test]
    fn test_serialize_operator_quote() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Quote {
                text: b"Hello".to_vec(),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("Hello"));
        assert!(s.ends_with("'\n"));
    }

    #[test]
    fn test_serialize_operator_double_quote() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::DoubleQuote {
                word_space: 1.0,
                char_space: 0.5,
                text: b"Hi".to_vec(),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("Hi"));
        assert!(s.ends_with("\"\n"));
    }

    #[test]
    fn test_serialize_operator_ext_gstate() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetExtGState {
                dict_name: "GS1".to_string(),
            },
        );
        assert_eq!(&output, b"/GS1 gs\n");
    }

    #[test]
    fn test_serialize_operator_paint_shading() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::PaintShading {
                name: "Sh1".to_string(),
            },
        );
        assert_eq!(&output, b"/Sh1 sh\n");
    }

    #[test]
    fn test_serialize_operator_set_rendering_intent() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetRenderingIntent {
                intent: "RelativeColorimetric".to_string(),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("/RelativeColorimetric"));
        assert!(s.ends_with("ri\n"));
    }

    #[test]
    fn test_serialize_operator_color_space() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillColorSpace {
                name: "DeviceRGB".to_string(),
            },
        );
        assert_eq!(&output, b"/DeviceRGB cs\n");

        output.clear();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeColorSpace {
                name: "DeviceCMYK".to_string(),
            },
        );
        assert_eq!(&output, b"/DeviceCMYK CS\n");
    }

    #[test]
    fn test_serialize_operator_set_fill_stroke_color() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillColor {
                components: vec![0.5, 0.6, 0.7],
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("sc\n"));

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeColor {
                components: vec![0.1, 0.2],
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("SC\n"));
    }

    #[test]
    fn test_serialize_operator_color_n_with_pattern() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillColorN {
                components: vec![0.5],
                name: Some(Box::new("P1".to_string())),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("/P1"));
        assert!(s.ends_with("scn\n"));

        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeColorN {
                components: vec![],
                name: None,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("SCN\n"));
    }

    #[test]
    fn test_serialize_operator_other() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Other {
                name: "DP".to_string(),
                operands: Box::new(vec![Object::Name("MC0".to_string())]),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("/MC0"));
        assert!(s.contains("DP"));
    }

    #[test]
    fn test_serialize_operator_inline_image() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        let mut dict = std::collections::HashMap::new();
        dict.insert("W".to_string(), Object::Integer(10));
        dict.insert("H".to_string(), Object::Integer(10));
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::InlineImage {
                dict: Box::new(dict),
                data: vec![0xFF; 10],
            },
        );
        let s = String::from_utf8_lossy(&output);
        assert!(s.contains("BI\n"));
        assert!(s.contains("ID "));
        assert!(s.contains("EI\n"));
    }

    #[test]
    fn test_serialize_operator_curves() {
        let editor = create_test_editor();

        // CurveTo
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::CurveTo {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                x3: 5.0,
                y3: 6.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("c\n"));

        // CurveToV
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::CurveToV {
                x2: 1.0,
                y2: 2.0,
                x3: 3.0,
                y3: 4.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("v\n"));

        // CurveToY
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::CurveToY {
                x1: 1.0,
                y1: 2.0,
                x3: 3.0,
                y3: 4.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("y\n"));
    }

    #[test]
    fn test_serialize_operator_text_state() {
        let editor = create_test_editor();

        let ops: Vec<(crate::content::operators::Operator, &str)> = vec![
            (crate::content::operators::Operator::Tc { char_space: 1.0 }, "Tc\n"),
            (crate::content::operators::Operator::Tw { word_space: 2.0 }, "Tw\n"),
            (crate::content::operators::Operator::Tz { scale: 100.0 }, "Tz\n"),
            (crate::content::operators::Operator::TL { leading: 14.0 }, "TL\n"),
            (crate::content::operators::Operator::Tr { render: 0 }, "Tr\n"),
            (crate::content::operators::Operator::Ts { rise: 3.0 }, "Ts\n"),
        ];

        for (op, suffix) in ops {
            let mut output = Vec::new();
            editor.serialize_operator(&mut output, &op);
            let s = String::from_utf8(output).unwrap();
            assert!(s.ends_with(suffix), "Expected suffix '{}', got '{}'", suffix, s);
        }
    }

    // =========================================================================
    // serialize_object
    // =========================================================================

    #[test]
    fn test_serialize_object_null() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Null);
        assert_eq!(&output, b"null");
    }

    #[test]
    fn test_serialize_object_boolean() {
        let editor = create_test_editor();

        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Boolean(true));
        assert_eq!(&output, b"true");

        output.clear();
        editor.serialize_object(&mut output, &Object::Boolean(false));
        assert_eq!(&output, b"false");
    }

    #[test]
    fn test_serialize_object_integer() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Integer(42));
        assert_eq!(&output, b"42");
    }

    #[test]
    fn test_serialize_object_real() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Real(std::f64::consts::PI));
        let s = String::from_utf8(output).unwrap();
        assert!(s.starts_with("3.14"));
    }

    #[test]
    fn test_serialize_object_name() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Name("Type".to_string()));
        assert_eq!(&output, b"/Type");
    }

    #[test]
    fn test_serialize_object_string() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::String(b"hello".to_vec()));
        assert_eq!(&output, b"(hello)");
    }

    #[test]
    fn test_serialize_object_string_with_escapes() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::String(b"a(b)c\\d".to_vec()));
        assert_eq!(&output, b"(a\\(b\\)c\\\\d)");
    }

    #[test]
    fn test_serialize_object_array() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(
            &mut output,
            &Object::Array(vec![Object::Integer(1), Object::Integer(2), Object::Integer(3)]),
        );
        assert_eq!(&output, b"[1 2 3]");
    }

    #[test]
    fn test_serialize_object_empty_array() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Array(vec![]));
        assert_eq!(&output, b"[]");
    }

    #[test]
    fn test_serialize_object_dictionary() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        let mut dict = HashMap::new();
        dict.insert("Key".to_string(), Object::Integer(42));
        editor.serialize_object(&mut output, &Object::Dictionary(dict));
        let s = String::from_utf8(output).unwrap();
        assert!(s.starts_with("<<"));
        assert!(s.ends_with(">>"));
        assert!(s.contains("/Key"));
        assert!(s.contains("42"));
    }

    #[test]
    fn test_serialize_object_reference() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(&mut output, &Object::Reference(ObjectRef::new(5, 0)));
        assert_eq!(&output, b"5 0 R");
    }

    #[test]
    fn test_serialize_object_stream_placeholder() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_object(
            &mut output,
            // ── pdf_manipulator patch: stream bodies are StreamBody ──
            &Object::stream(HashMap::new(), bytes::Bytes::from_static(b"data")),
            // ── end pdf_manipulator patch ──
        );
        assert_eq!(&output, b"(stream)");
    }

    // =========================================================================
    // build_image_xobject
    // =========================================================================

    #[test]
    fn test_build_image_xobject_jpeg() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent::new(
            crate::geometry::Rect::new(0.0, 0.0, 100.0, 50.0),
            ImageFormat::Jpeg,
            vec![0xFF, 0xD8, 0xFF, 0xE0], // Fake JPEG header
            100,
            50,
        );

        let obj = DocumentEditor::build_image_xobject(&image);
        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        if let Object::Stream { dict, body: _ } = obj {
            // ── end pdf_manipulator patch ──
            assert_eq!(dict.get("Type").unwrap(), &Object::Name("XObject".to_string()));
            assert_eq!(dict.get("Subtype").unwrap(), &Object::Name("Image".to_string()));
            assert_eq!(dict.get("Width").unwrap().as_integer().unwrap(), 100);
            assert_eq!(dict.get("Height").unwrap().as_integer().unwrap(), 50);
            assert_eq!(dict.get("Filter").unwrap(), &Object::Name("DCTDecode".to_string()));
            assert_eq!(dict.get("ColorSpace").unwrap(), &Object::Name("DeviceRGB".to_string()));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_png() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 200.0, 100.0),
            format: ImageFormat::Png,
            data: vec![0x89, 0x50, 0x4E, 0x47], // Fake PNG header
            width: 200,
            height: 100,
            bits_per_component: 8,
            color_space: crate::elements::ColorSpace::Gray,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            assert_eq!(dict.get("Filter").unwrap(), &Object::Name("FlateDecode".to_string()));
            assert_eq!(dict.get("ColorSpace").unwrap(), &Object::Name("DeviceGray".to_string()));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_jpeg2000() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            format: ImageFormat::Jpeg2000,
            data: vec![0; 8],
            width: 10,
            height: 10,
            bits_per_component: 8,
            color_space: crate::elements::ColorSpace::CMYK,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            assert_eq!(dict.get("Filter").unwrap(), &Object::Name("JPXDecode".to_string()));
            assert_eq!(dict.get("ColorSpace").unwrap(), &Object::Name("DeviceCMYK".to_string()));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_jbig2() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            format: ImageFormat::Jbig2,
            data: vec![0; 4],
            width: 10,
            height: 10,
            bits_per_component: 1,
            color_space: crate::elements::ColorSpace::Gray,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            assert_eq!(dict.get("Filter").unwrap(), &Object::Name("JBIG2Decode".to_string()));
            assert_eq!(dict.get("BitsPerComponent").unwrap().as_integer().unwrap(), 1);
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_unknown_format() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            format: ImageFormat::Unknown,
            data: vec![0; 4],
            width: 10,
            height: 10,
            bits_per_component: 8,
            color_space: crate::elements::ColorSpace::RGB,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            // Unknown format should not have a Filter
            assert!(!dict.contains_key("Filter"));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_indexed_colorspace() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            format: ImageFormat::Raw,
            data: vec![0; 10],
            width: 10,
            height: 10,
            bits_per_component: 8,
            color_space: crate::elements::ColorSpace::Indexed,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            assert_eq!(dict.get("ColorSpace").unwrap(), &Object::Name("Indexed".to_string()));
            assert_eq!(dict.get("Filter").unwrap(), &Object::Name("FlateDecode".to_string()));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_lab_colorspace() {
        use crate::elements::{ImageContent, ImageFormat};

        let image = ImageContent {
            bbox: crate::geometry::Rect::new(0.0, 0.0, 10.0, 10.0),
            format: ImageFormat::Raw,
            data: vec![0; 10],
            width: 10,
            height: 10,
            bits_per_component: 8,
            color_space: crate::elements::ColorSpace::Lab,
            reading_order: None,
            alt_text: None,
            horizontal_dpi: None,
            vertical_dpi: None,
            soft_mask: None,
            matrix: None,
            is_artifact: false,
            // ── pdf_manipulator patch: the image streams from its source ──
            streamed: None,
            // ── end pdf_manipulator patch ──
        };

        let obj = DocumentEditor::build_image_xobject(&image);
        if let Object::Stream { dict, .. } = obj {
            assert_eq!(dict.get("ColorSpace").unwrap(), &Object::Name("Lab".to_string()));
        } else {
            panic!("Expected Stream object");
        }
    }

    #[test]
    fn test_build_image_xobject_length() {
        use crate::elements::{ImageContent, ImageFormat};

        let data = vec![1, 2, 3, 4, 5];
        let image = ImageContent::new(
            crate::geometry::Rect::new(0.0, 0.0, 5.0, 1.0),
            ImageFormat::Raw,
            data.clone(),
            5,
            1,
        );

        let obj = DocumentEditor::build_image_xobject(&image);
        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        if let Object::Stream { dict, body } = obj {
            // ── end pdf_manipulator patch ──
            assert_eq!(dict.get("Length").unwrap().as_integer().unwrap(), 5);
            // ── pdf_manipulator patch: stream bodies are StreamBody ──
            assert_eq!(body.inline().expect("an inline body").as_ref(), &data[..]);
            // ── end pdf_manipulator patch ──
        } else {
            panic!("Expected Stream object");
        }
    }

    // =========================================================================
    // parse_media_box
    // =========================================================================

    #[test]
    fn test_parse_media_box_real_values() {
        let editor = create_test_editor();
        let media_box = Object::Array(vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(595.28),
            Object::Real(841.89),
        ]);

        let (w, h) = editor.parse_media_box(&media_box).unwrap();
        assert!((w - 595.28).abs() < 0.01);
        assert!((h - 841.89).abs() < 0.01);
    }

    #[test]
    fn test_parse_media_box_integer_values() {
        let editor = create_test_editor();
        let media_box = Object::Array(vec![
            Object::Integer(0),
            Object::Integer(0),
            Object::Integer(612),
            Object::Integer(792),
        ]);

        let (w, h) = editor.parse_media_box(&media_box).unwrap();
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    #[test]
    fn test_parse_media_box_non_zero_origin() {
        let editor = create_test_editor();
        let media_box = Object::Array(vec![
            Object::Real(50.0),
            Object::Real(50.0),
            Object::Real(562.0),
            Object::Real(742.0),
        ]);

        let (w, h) = editor.parse_media_box(&media_box).unwrap();
        assert!((w - 512.0).abs() < 0.01);
        assert!((h - 692.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_media_box_non_array() {
        let editor = create_test_editor();
        let media_box = Object::Integer(42);

        let (w, h) = editor.parse_media_box(&media_box).unwrap();
        // Should return default Letter size
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    #[test]
    fn test_parse_media_box_short_array() {
        let editor = create_test_editor();
        let media_box = Object::Array(vec![Object::Integer(0), Object::Integer(0)]);

        let (w, h) = editor.parse_media_box(&media_box).unwrap();
        // Should return default Letter size
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    // =========================================================================
    // find_prev_xref_offset
    // =========================================================================

    #[test]
    fn test_find_prev_xref_offset_valid() {
        let editor = create_test_editor();

        // The function searches backwards from pos = len - 100 down to 1.
        // At each pos it checks if bytes[pos..] starts with "startxref".
        // So "startxref" must be at a position p where 1 <= p <= len - 100.
        // We put startxref at position 10, then pad after it to make total > 110.
        let mut pdf_data = Vec::new();
        pdf_data.extend_from_slice(b"%PDF-1.4\n"); // 9 bytes
        pdf_data.extend_from_slice(b"startxref\n12345\n%%EOF\n"); // startxref at byte 9
                                                                  // Pad to ensure len - 100 >= 9 (i.e., len >= 109)
        while pdf_data.len() < 120 {
            pdf_data.push(b'\n');
        }
        let result = editor.find_prev_xref_offset(&pdf_data);
        assert_eq!(result.unwrap(), 12345);
    }

    #[test]
    fn test_find_prev_xref_offset_with_whitespace() {
        let editor = create_test_editor();

        let mut pdf_data = Vec::new();
        pdf_data.extend_from_slice(b"%PDF-1.4\n"); // 9 bytes
        pdf_data.extend_from_slice(b"startxref\n  \r\n67890\n%%EOF\n"); // startxref at byte 9
        while pdf_data.len() < 120 {
            pdf_data.push(b'\n');
        }
        let result = editor.find_prev_xref_offset(&pdf_data);
        assert_eq!(result.unwrap(), 67890);
    }

    #[test]
    fn test_find_prev_xref_offset_not_found() {
        let editor = create_test_editor();

        // No "startxref" anywhere, and data is long enough
        let mut pdf_data = vec![b'X'; 200];
        pdf_data.extend_from_slice(b"\n%%EOF\n");
        let result = editor.find_prev_xref_offset(&pdf_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_prev_xref_offset_short_data() {
        let editor = create_test_editor();

        // Data shorter than 100 bytes -- pos starts at 0, loop doesn't execute
        let pdf_data = b"startxref\n999\n%%EOF\n";
        let result = editor.find_prev_xref_offset(pdf_data);
        // Short data means pos starts at 0 and while pos > 0 is false, so it should err
        assert!(result.is_err());
    }

    // =========================================================================
    // allocate_object_id
    // =========================================================================

    #[test]
    fn test_allocate_object_id_sequential() {
        let mut editor = create_test_editor();
        let first = editor.allocate_object_id();
        let second = editor.allocate_object_id();
        let third = editor.allocate_object_id();

        assert_eq!(second, first + 1);
        assert_eq!(third, first + 2);
    }

    // =========================================================================
    // current_page_count and page manipulation
    // =========================================================================

    #[test]
    fn test_current_page_count_initial() {
        let editor = create_test_editor();
        assert_eq!(editor.current_page_count(), 1);
    }

    #[test]
    fn test_current_page_count_after_removal() {
        let mut editor = create_test_editor();
        // Remove page 0
        editor.page_order.remove(0);
        assert_eq!(editor.current_page_count(), 0);
    }

    #[test]
    fn test_current_page_count_mixed() {
        let mut editor = create_test_editor();
        // Simulate 3 pages: [0, 1, 2], then remove page at index 1
        editor.page_order = vec![PageSlot::source(0), PageSlot::source(2)];
        assert_eq!(editor.current_page_count(), 2);
    }

    // =========================================================================
    // is_modified
    // =========================================================================

    #[test]
    fn test_is_modified_initial() {
        let editor = create_test_editor();
        assert!(!editor.is_modified());
    }

    // =========================================================================
    // source_path
    // =========================================================================

    #[test]
    fn test_source_path() {
        let pdf_bytes = minimal_pdf_bytes();
        let tmp = std::env::temp_dir().join("pdf_oxide_test_path.pdf");
        std::fs::write(&tmp, &pdf_bytes).unwrap();
        let editor = DocumentEditor::open(&tmp).unwrap();
        assert!(editor.source_path().contains("pdf_oxide_test_path.pdf"));
        let _ = std::fs::remove_file(&tmp);
    }

    // =========================================================================
    // version
    // =========================================================================

    #[test]
    fn test_version() {
        let editor = create_test_editor();
        let (major, minor) = editor.version();
        assert_eq!(major, 1);
        assert_eq!(minor, 4);
    }

    // =========================================================================
    // build_info_object
    // =========================================================================

    #[test]
    fn test_build_info_object_none() {
        let editor = create_test_editor();
        assert!(editor.build_info_object().is_none());
    }

    #[test]
    fn test_build_info_object_some() {
        let mut editor = create_test_editor();
        editor.modified_info = Some(DocumentInfo::new().title("Test"));
        let obj = editor.build_info_object();
        assert!(obj.is_some());
        let dict = obj.unwrap().as_dict().unwrap().clone();
        assert!(dict.contains_key("Title"));
    }

    // =========================================================================
    // RedactionData
    // =========================================================================

    #[test]
    fn test_redaction_data_construction() {
        let rd = RedactionData {
            rect: [10.0, 20.0, 100.0, 200.0],
            color: [1.0, 0.0, 0.0],
        };
        assert_eq!(rd.rect[0], 10.0);
        assert_eq!(rd.color[0], 1.0);
    }

    // =========================================================================
    // AnnotationAppearance
    // =========================================================================

    #[test]
    fn test_annotation_appearance_construction() {
        let ap = AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::from_static(b"q 1 0 0 1 0 0 cm Q")),
            bbox: [0.0, 0.0, 100.0, 50.0],
            annot_rect: [10.0, 20.0, 110.0, 70.0],
            matrix: Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            resources: None,
            extra_objects: Vec::new(),
        };
        assert_eq!(ap.bbox[2], 100.0);
        assert_eq!(ap.annot_rect[0], 10.0);
        assert!(ap.matrix.is_some());
        assert!(ap.resources.is_none());
    }

    #[test]
    fn test_annotation_appearance_no_matrix_no_resources() {
        let ap = AnnotationAppearance {
            dict: HashMap::new(),
            body: StreamBody::Inline(bytes::Bytes::new()),
            bbox: [0.0, 0.0, 0.0, 0.0],
            annot_rect: [0.0, 0.0, 0.0, 0.0],
            matrix: None,
            resources: None,
            extra_objects: Vec::new(),
        };
        assert!(ap.matrix.is_none());
        assert!(ap.body.is_empty());
    }

    // =========================================================================
    // ImageInfo
    // =========================================================================

    #[test]
    fn test_image_info_construction() {
        let info = ImageInfo {
            name: "Im1".to_string(),
            bounds: [10.0, 20.0, 100.0, 50.0],
            matrix: [100.0, 0.0, 0.0, 50.0, 10.0, 20.0],
        };
        assert_eq!(info.name, "Im1");
        assert_eq!(info.bounds[2], 100.0);
    }

    // =========================================================================
    // ImageModification
    // =========================================================================

    #[test]
    fn test_image_modification_construction() {
        let m = ImageModification {
            x: Some(10.0),
            y: None,
            width: Some(200.0),
            height: None,
        };
        assert_eq!(m.x, Some(10.0));
        assert_eq!(m.y, None);
        assert_eq!(m.width, Some(200.0));
        assert_eq!(m.height, None);
    }

    // =========================================================================
    // PageInfo
    // =========================================================================

    #[test]
    fn test_page_info_construction() {
        let info = PageInfo {
            index: 0,
            width: 612.0,
            height: 792.0,
            rotation: 90,
            object_ref: ObjectRef::new(5, 0),
        };
        assert_eq!(info.index, 0);
        assert_eq!(info.width, 612.0);
        assert_eq!(info.rotation, 90);
        assert_eq!(info.object_ref.id, 5);
    }

    // =========================================================================
    // Metadata setters (set_title, set_author, etc.)
    // =========================================================================

    #[test]
    fn test_set_title_marks_modified() {
        let mut editor = create_test_editor();
        assert!(!editor.is_modified());
        editor.set_title("New Title");
        assert!(editor.is_modified());
        assert_eq!(editor.modified_info.as_ref().unwrap().title, Some("New Title".to_string()));
    }

    #[test]
    fn test_set_author_marks_modified() {
        let mut editor = create_test_editor();
        editor.set_author("New Author");
        assert!(editor.is_modified());
        assert_eq!(editor.modified_info.as_ref().unwrap().author, Some("New Author".to_string()));
    }

    #[test]
    fn test_set_subject_marks_modified() {
        let mut editor = create_test_editor();
        editor.set_subject("New Subject");
        assert!(editor.is_modified());
        assert_eq!(editor.modified_info.as_ref().unwrap().subject, Some("New Subject".to_string()));
    }

    #[test]
    fn test_set_keywords_marks_modified() {
        let mut editor = create_test_editor();
        editor.set_keywords("kw1, kw2");
        assert!(editor.is_modified());
        assert_eq!(editor.modified_info.as_ref().unwrap().keywords, Some("kw1, kw2".to_string()));
    }

    #[test]
    fn test_set_multiple_metadata_fields() {
        let mut editor = create_test_editor();
        editor.set_title("T1");
        editor.set_author("A1");
        editor.set_subject("S1");
        editor.set_keywords("K1");

        let info = editor.modified_info.as_ref().unwrap();
        assert_eq!(info.title, Some("T1".to_string()));
        assert_eq!(info.author, Some("A1".to_string()));
        assert_eq!(info.subject, Some("S1".to_string()));
        assert_eq!(info.keywords, Some("K1".to_string()));
    }

    // =========================================================================
    // Erase region API methods
    // =========================================================================

    #[test]
    fn test_erase_region_valid() {
        let mut editor = create_test_editor();
        let result = editor.erase_region(0, [10.0, 20.0, 100.0, 200.0]);
        assert!(result.is_ok());
        assert!(editor.is_modified());
        assert_eq!(editor.erase_regions.get(&0).unwrap().len(), 1);
    }

    #[test]
    fn test_erase_region_out_of_range() {
        let mut editor = create_test_editor();
        let result = editor.erase_region(99, [10.0, 20.0, 100.0, 200.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_erase_regions_multiple() {
        let mut editor = create_test_editor();
        let rects = &[[10.0, 20.0, 100.0, 200.0], [200.0, 300.0, 400.0, 500.0]];
        let result = editor.erase_regions(0, rects);
        assert!(result.is_ok());
        assert_eq!(editor.erase_regions.get(&0).unwrap().len(), 2);
    }

    #[test]
    fn test_erase_regions_out_of_range() {
        let mut editor = create_test_editor();
        let result = editor.erase_regions(99, &[[10.0, 20.0, 100.0, 200.0]]);
        assert!(result.is_err());
    }

    #[test]
    fn test_clear_erase_regions() {
        let mut editor = create_test_editor();
        editor.erase_region(0, [10.0, 20.0, 100.0, 200.0]).unwrap();
        assert!(editor.erase_regions.contains_key(&0));
        editor.clear_erase_regions(0);
        assert!(!editor.erase_regions.contains_key(&0));
    }

    // =========================================================================
    // Annotation flatten methods
    // =========================================================================

    #[test]
    fn test_flatten_page_annotations_valid() {
        let mut editor = create_test_editor();
        assert!(!editor.is_page_marked_for_flatten(0));
        editor.flatten_page_annotations(0).unwrap();
        assert!(editor.is_page_marked_for_flatten(0));
        assert!(editor.is_modified());
    }

    #[test]
    fn test_flatten_page_annotations_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.flatten_page_annotations(99).is_err());
    }

    #[test]
    fn test_flatten_all_annotations() {
        let mut editor = create_test_editor();
        editor.flatten_all_annotations().unwrap();
        assert!(editor.is_page_marked_for_flatten(0));
    }

    #[test]
    fn test_unmark_page_for_flatten() {
        let mut editor = create_test_editor();
        editor.flatten_page_annotations(0).unwrap();
        assert!(editor.is_page_marked_for_flatten(0));
        editor.unmark_page_for_flatten(0);
        assert!(!editor.is_page_marked_for_flatten(0));
    }

    // =========================================================================
    // Form flatten methods
    // =========================================================================

    #[test]
    fn test_flatten_forms_on_page_valid() {
        let mut editor = create_test_editor();
        assert!(!editor.is_page_marked_for_form_flatten(0));
        editor.flatten_forms_on_page(0).unwrap();
        assert!(editor.is_page_marked_for_form_flatten(0));
    }

    #[test]
    fn test_flatten_forms_on_page_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.flatten_forms_on_page(99).is_err());
    }

    #[test]
    fn test_flatten_forms() {
        let mut editor = create_test_editor();
        assert!(!editor.will_remove_acroform());
        editor.flatten_forms().unwrap();
        assert!(editor.is_page_marked_for_form_flatten(0));
        assert!(editor.will_remove_acroform());
    }

    // =========================================================================
    // Redaction methods
    // =========================================================================

    // ── pdf_manipulator patch: a redaction is a queued mark until apply ──
    #[test]
    fn test_add_redaction_marks_page() {
        let mut editor = create_test_editor();
        assert!(!editor.is_page_marked_for_redaction(0));
        editor.add_redaction(0, [10.0, 10.0, 50.0, 50.0], None).unwrap();
        assert!(editor.is_page_marked_for_redaction(0));
        assert!(editor.is_modified());
    }

    #[test]
    fn test_add_redaction_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.add_redaction(99, [10.0, 10.0, 50.0, 50.0], None).is_err());
    }

    #[test]
    fn test_apply_without_marks_changes_nothing() {
        let mut editor = create_test_editor();
        let report = editor
            .apply_redactions_destructive(crate::redaction::RedactionOptions::default())
            .unwrap();
        assert_eq!(report.regions, 0);
        assert!(!editor.is_page_marked_for_redaction(0));
    }

    #[test]
    fn test_unmark_page_for_redaction() {
        let mut editor = create_test_editor();
        editor.add_redaction(0, [10.0, 10.0, 50.0, 50.0], None).unwrap();
        editor.unmark_page_for_redaction(0);
        assert!(!editor.is_page_marked_for_redaction(0));
    }

    // ── end pdf_manipulator patch ──

    // =========================================================================
    // Image modification methods
    // =========================================================================

    #[test]
    fn test_reposition_image_valid() {
        let mut editor = create_test_editor();
        let result = editor.reposition_image(0, "Im1", 100.0, 200.0);
        assert!(result.is_ok());
        assert!(editor.is_modified());
        assert!(editor.has_image_modifications(0));
    }

    #[test]
    fn test_reposition_image_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.reposition_image(99, "Im1", 100.0, 200.0).is_err());
    }

    #[test]
    fn test_resize_image_valid() {
        let mut editor = create_test_editor();
        let result = editor.resize_image(0, "Im1", 300.0, 200.0);
        assert!(result.is_ok());
        assert!(editor.has_image_modifications(0));
    }

    #[test]
    fn test_resize_image_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.resize_image(99, "Im1", 300.0, 200.0).is_err());
    }

    #[test]
    fn test_set_image_bounds_valid() {
        let mut editor = create_test_editor();
        let result = editor.set_image_bounds(0, "Im1", 10.0, 20.0, 300.0, 200.0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_set_image_bounds_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor
            .set_image_bounds(99, "Im1", 10.0, 20.0, 300.0, 200.0)
            .is_err());
    }

    #[test]
    fn test_clear_image_modifications() {
        let mut editor = create_test_editor();
        editor.reposition_image(0, "Im1", 10.0, 20.0).unwrap();
        assert!(editor.has_image_modifications(0));
        editor.clear_image_modifications(0);
        assert!(!editor.has_image_modifications(0));
    }

    #[test]
    fn test_has_image_modifications_empty() {
        let editor = create_test_editor();
        assert!(!editor.has_image_modifications(0));
        assert!(!editor.has_image_modifications(99));
    }

    // =========================================================================
    // Embedded files
    // =========================================================================

    #[test]
    fn test_embed_file() {
        let mut editor = create_test_editor();
        let result = editor.embed_file("test.txt", b"Hello World".to_vec());
        assert!(result.is_ok());
        assert!(editor.is_modified());
        assert_eq!(editor.pending_embedded_files().len(), 1);
    }

    #[test]
    fn test_embed_file_multiple() {
        let mut editor = create_test_editor();
        editor.embed_file("a.txt", b"AAA".to_vec()).unwrap();
        editor.embed_file("b.txt", b"BBB".to_vec()).unwrap();
        assert_eq!(editor.pending_embedded_files().len(), 2);
    }

    #[test]
    fn test_clear_embedded_files() {
        let mut editor = create_test_editor();
        editor.embed_file("a.txt", b"AAA".to_vec()).unwrap();
        assert_eq!(editor.pending_embedded_files().len(), 1);
        editor.clear_embedded_files();
        assert!(editor.pending_embedded_files().is_empty());
    }

    // =========================================================================
    // set_page_rotation and rotate_page_by
    // =========================================================================

    #[test]
    fn test_set_page_rotation_valid_values() {
        let mut editor = create_test_editor();
        for &deg in &[0, 90, 180, 270] {
            assert!(editor.set_page_rotation(0, deg).is_ok());
        }
    }

    #[test]
    fn test_set_page_rotation_invalid_values() {
        let mut editor = create_test_editor();
        assert!(editor.set_page_rotation(0, 45).is_err());
        assert!(editor.set_page_rotation(0, 360).is_err());
        assert!(editor.set_page_rotation(0, -90).is_err());
    }

    #[test]
    fn test_set_page_rotation_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.set_page_rotation(99, 90).is_err());
    }

    #[test]
    fn test_set_page_media_box_valid() {
        let mut editor = create_test_editor();
        let result = editor.set_page_media_box(0, [0.0, 0.0, 500.0, 700.0]);
        assert!(result.is_ok());
        assert!(editor.is_modified());
    }

    #[test]
    fn test_set_page_media_box_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor
            .set_page_media_box(99, [0.0, 0.0, 500.0, 700.0])
            .is_err());
    }

    #[test]
    fn test_set_page_crop_box_valid() {
        let mut editor = create_test_editor();
        let result = editor.set_page_crop_box(0, [10.0, 10.0, 602.0, 782.0]);
        assert!(result.is_ok());
        assert!(editor.is_modified());
    }

    #[test]
    fn test_set_page_crop_box_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor
            .set_page_crop_box(99, [10.0, 10.0, 602.0, 782.0])
            .is_err());
    }

    // =========================================================================
    // EditableDocument trait: page_count, set_info, get_info
    // =========================================================================

    #[test]
    fn test_editable_page_count() {
        let mut editor = create_test_editor();
        assert_eq!(EditableDocument::page_count(&mut editor).unwrap(), 1);
    }

    #[test]
    fn test_editable_set_info() {
        let mut editor = create_test_editor();
        let info = DocumentInfo::new().title("SetInfo Test");
        EditableDocument::set_info(&mut editor, info).unwrap();
        assert!(editor.is_modified());

        let retrieved = EditableDocument::get_info(&mut editor).unwrap();
        assert_eq!(retrieved.title, Some("SetInfo Test".to_string()));
    }

    #[test]
    fn test_editable_get_info_returns_modified() {
        let mut editor = create_test_editor();

        // Set info via the trait
        let info = DocumentInfo::new()
            .title("Modified Title")
            .author("Modified Author");
        EditableDocument::set_info(&mut editor, info).unwrap();

        // Get should return the modified version
        let result = EditableDocument::get_info(&mut editor).unwrap();
        assert_eq!(result.title, Some("Modified Title".to_string()));
        assert_eq!(result.author, Some("Modified Author".to_string()));
    }

    // =========================================================================
    // set_page_content - error path
    // =========================================================================

    #[test]
    fn test_set_page_content_out_of_range() {
        let mut editor = create_test_editor();
        let content = StructureElement::default();
        let result = editor.set_page_content(99, content);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_page_content_valid() {
        let mut editor = create_test_editor();
        let content = StructureElement {
            structure_type: "Document".to_string(),
            bbox: crate::geometry::Rect::new(0.0, 0.0, 612.0, 792.0),
            children: Vec::new(),
            reading_order: Some(0),
            alt_text: None,
            language: None,
        };
        let result = editor.set_page_content(0, content);
        assert!(result.is_ok());
        assert!(editor.is_modified());
        assert!(editor.structure_modified);
        assert!(editor.modified_content.contains_key(&0));
    }

    // =========================================================================
    // Annotations query methods
    // =========================================================================

    #[test]
    fn test_has_modified_annotations_false() {
        let editor = create_test_editor();
        assert!(!editor.has_modified_annotations(0));
    }

    #[test]
    fn test_get_page_annotations_none() {
        let editor = create_test_editor();
        assert!(editor.get_page_annotations(0).is_none());
    }

    // =========================================================================
    // rewrite_content_stream_with_image_mods
    // =========================================================================

    #[test]
    fn test_rewrite_content_stream_no_mods() {
        let editor = create_test_editor();
        let content = b"q\n100 0 0 50 10 20 cm\n/Im1 Do\nQ\n";
        let mods = HashMap::new(); // empty modifications
        let result = editor
            .rewrite_content_stream_with_image_mods(content, &mods)
            .unwrap();
        let s = String::from_utf8(result).unwrap();
        // Should preserve the cm operator as-is
        assert!(s.contains("cm\n"));
        assert!(s.contains("/Im1 Do\n"));
    }

    #[test]
    fn test_rewrite_content_stream_with_position_mod() {
        let editor = create_test_editor();
        let content = b"q\n100 0 0 50 10 20 cm\n/Im1 Do\nQ\n";
        let mut mods = HashMap::new();
        mods.insert(
            "Im1".to_string(),
            ImageModification {
                x: Some(200.0),
                y: Some(300.0),
                width: None,
                height: None,
            },
        );
        let result = editor
            .rewrite_content_stream_with_image_mods(content, &mods)
            .unwrap();
        let s = String::from_utf8(result).unwrap();
        // The e and f values (position) should be modified
        assert!(s.contains("200.0"));
        assert!(s.contains("300.0"));
    }

    #[test]
    fn test_rewrite_content_stream_with_size_mod() {
        let editor = create_test_editor();
        let content = b"q\n100 0 0 50 10 20 cm\n/Im1 Do\nQ\n";
        let mut mods = HashMap::new();
        mods.insert(
            "Im1".to_string(),
            ImageModification {
                x: None,
                y: None,
                width: Some(400.0),
                height: Some(300.0),
            },
        );
        let result = editor
            .rewrite_content_stream_with_image_mods(content, &mods)
            .unwrap();
        let s = String::from_utf8(result).unwrap();
        // The a and d values (size) should be modified
        assert!(s.contains("400.0"));
        assert!(s.contains("300.0"));
    }

    // =========================================================================
    // Open with non-existent file
    // =========================================================================

    #[test]
    fn test_open_nonexistent_file() {
        let result = DocumentEditor::open("/tmp/nonexistent_pdf_oxide_test.pdf");
        assert!(result.is_err());
    }

    // =========================================================================
    // EditableDocument trait: remove_page, move_page, duplicate_page
    // =========================================================================

    /// Helper that creates a test editor with N pages (using a multi-page minimal PDF).
    fn create_multi_page_editor(n: usize) -> DocumentEditor {
        // Build a minimal PDF with n pages
        let mut pdf = Vec::new();
        pdf.extend_from_slice(b"%PDF-1.4\n");

        // Object 1: Catalog
        let catalog_offset = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

        // Build Kids array
        let mut kids = String::from("[");
        for i in 0..n {
            if i > 0 {
                kids.push(' ');
            }
            kids.push_str(&format!("{} 0 R", i + 3));
        }
        kids.push(']');

        // Object 2: Pages
        let pages_offset = pdf.len();
        let pages_str =
            format!("2 0 obj\n<< /Type /Pages /Kids {} /Count {} >>\nendobj\n", kids, n);
        pdf.extend_from_slice(pages_str.as_bytes());

        // Page objects
        let mut page_offsets = Vec::new();
        for i in 0..n {
            page_offsets.push(pdf.len());
            let page_str = format!(
                "{} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
                i + 3
            );
            pdf.extend_from_slice(page_str.as_bytes());
        }

        // xref
        let xref_offset = pdf.len();
        let total_objects = n + 3;
        pdf.extend_from_slice(format!("xref\n0 {}\n", total_objects).as_bytes());
        pdf.extend_from_slice(b"0000000000 65535 f \n");
        pdf.extend_from_slice(format!("{:010} 00000 n \n", catalog_offset).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \n", pages_offset).as_bytes());
        for offset in &page_offsets {
            pdf.extend_from_slice(format!("{:010} 00000 n \n", offset).as_bytes());
        }

        // trailer
        pdf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
                total_objects, xref_offset
            )
            .as_bytes(),
        );

        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = std::env::temp_dir()
            .join(format!("pdf_oxide_multipage_test_{}_{id}.pdf", std::process::id()));
        std::fs::write(&tmp, &pdf).unwrap();
        let editor = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        editor
    }

    #[test]
    fn test_remove_page_valid() {
        let mut editor = create_multi_page_editor(3);
        assert_eq!(editor.current_page_count(), 3);
        editor.remove_page(1).unwrap();
        assert_eq!(editor.current_page_count(), 2);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_remove_page_out_of_range() {
        let mut editor = create_multi_page_editor(2);
        assert!(editor.remove_page(5).is_err());
    }

    #[test]
    fn test_move_page_valid() {
        let mut editor = create_multi_page_editor(3);
        editor.move_page(0, 2).unwrap();
        assert!(editor.is_modified());
        // Page order should be rearranged
        assert_eq!(
            editor.page_order,
            vec![PageSlot::source(1), PageSlot::source(2), PageSlot::source(0)]
        );
    }

    #[test]
    fn test_move_page_out_of_range() {
        let mut editor = create_multi_page_editor(2);
        assert!(editor.move_page(0, 5).is_err());
        assert!(editor.move_page(5, 0).is_err());
    }

    #[test]
    fn test_duplicate_page_valid() {
        let mut editor = create_multi_page_editor(2);
        let new_idx = editor.duplicate_page(0).unwrap();
        assert_eq!(new_idx, 2); // Was 2 pages, new one is at index 2
        assert_eq!(editor.current_page_count(), 3);
        assert!(editor.is_modified());
    }

    #[test]
    fn test_duplicate_page_out_of_range() {
        let mut editor = create_multi_page_editor(2);
        assert!(editor.duplicate_page(5).is_err());
    }

    // =========================================================================
    // get_page_info
    // =========================================================================

    #[test]
    fn test_get_page_info_valid() {
        let mut editor = create_test_editor();
        let info = editor.get_page_info(0).unwrap();
        assert_eq!(info.index, 0);
        assert_eq!(info.width, 612.0);
        assert_eq!(info.height, 792.0);
        assert_eq!(info.rotation, 0);
    }

    #[test]
    fn test_get_page_info_out_of_range() {
        let mut editor = create_test_editor();
        assert!(editor.get_page_info(99).is_err());
    }

    // =========================================================================
    // extract_pages error
    // =========================================================================

    #[test]
    fn test_extract_pages_works() {
        let mut editor = create_test_editor();
        let out = std::env::temp_dir().join("pdf_oxide_extract_test.pdf");
        let result = editor.extract_pages(&[0], &out);
        let _ = std::fs::remove_file(&out);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_pages_out_of_range() {
        let mut editor = create_test_editor();
        let out = std::env::temp_dir().join("pdf_oxide_extract_oob.pdf");
        let result = editor.extract_pages(&[99], &out);
        assert!(result.is_err());
    }

    /// Sequential chunked extraction — mirrors issue #474's S3-Lambda
    /// workflow: open once, extract non-overlapping page ranges in turn,
    /// each call must (a) succeed, (b) leave the source observably
    /// unchanged, (c) produce a PDF whose page count matches the slice.
    #[test]
    fn test_extract_pages_chunked_sequential() {
        let mut editor = create_multi_page_editor(10);
        let original_count = editor.current_page_count();

        for (start, end) in [(0, 3), (3, 6), (6, 10)] {
            let pages: Vec<usize> = (start..end).collect();
            let bytes = editor.extract_pages_to_bytes(&pages).unwrap();
            assert!(!bytes.is_empty());

            // Source document must be unchanged after each call.
            assert_eq!(
                editor.current_page_count(),
                original_count,
                "source page count changed after chunk {start}..{end}"
            );

            // Round-trip the chunk and verify its page count.
            let mut chunk = DocumentEditor::from_bytes(bytes).unwrap();
            assert_eq!(chunk.page_count().unwrap(), end - start);
        }
    }

    /// Out-of-order page selection (non-sequential indices).
    #[test]
    fn test_extract_pages_non_sequential() {
        let mut editor = create_multi_page_editor(5);
        let bytes = editor.extract_pages_to_bytes(&[3, 0, 4]).unwrap();
        let mut chunk = DocumentEditor::from_bytes(bytes).unwrap();
        assert_eq!(chunk.page_count().unwrap(), 3);
    }

    /// Batch extraction (issue #474 Option A). Each output independently
    /// reopens with the right page count.
    #[test]
    fn test_extract_page_ranges_to_bytes_batch() {
        let mut editor = create_multi_page_editor(10);
        let chunks = editor
            .extract_page_ranges_to_bytes(&[(0, 3), (3, 6), (6, 10)])
            .unwrap();
        assert_eq!(chunks.len(), 3);
        for (i, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = DocumentEditor::from_bytes(bytes).unwrap();
            let expected = match i {
                0 => 3,
                1 => 3,
                _ => 4,
            };
            assert_eq!(chunk.page_count().unwrap(), expected);
        }
    }

    /// In-place selection (issue #474 Option B). After `select_pages`, the
    /// document has only the listed pages; subsequent `save_to_bytes` round-
    /// trips with the same count.
    #[test]
    fn test_select_pages_in_place() {
        let mut editor = create_multi_page_editor(5);
        editor.select_pages(&[1, 3, 4]).unwrap();
        assert_eq!(editor.current_page_count(), 3);
        let bytes = editor.save_to_bytes().unwrap();
        let mut reopened = DocumentEditor::from_bytes(bytes).unwrap();
        assert_eq!(reopened.page_count().unwrap(), 3);
    }

    /// `select_pages` validates indices.
    #[test]
    fn test_select_pages_out_of_range() {
        let mut editor = create_multi_page_editor(3);
        let result = editor.select_pages(&[5]);
        assert!(result.is_err());
    }

    // =========================================================================
    // rotate_page_by normalization
    // =========================================================================

    #[test]
    fn test_rotate_page_by_normalization() {
        let mut editor = create_test_editor();

        // Start at 0, rotate by 90
        editor.rotate_page_by(0, 90).unwrap();
        assert_eq!(editor.get_page_rotation(0).unwrap(), 90);

        // Rotate by another 90 -> 180
        editor.rotate_page_by(0, 90).unwrap();
        assert_eq!(editor.get_page_rotation(0).unwrap(), 180);

        // Rotate by another 90 -> 270
        editor.rotate_page_by(0, 90).unwrap();
        assert_eq!(editor.get_page_rotation(0).unwrap(), 270);

        // Rotate by another 90 -> 360 -> normalized to 0
        editor.rotate_page_by(0, 90).unwrap();
        assert_eq!(editor.get_page_rotation(0).unwrap(), 0);
    }

    #[test]
    fn test_rotate_all_pages() {
        let mut editor = create_multi_page_editor(3);
        editor.rotate_all_pages(90).unwrap();

        for i in 0..3 {
            assert_eq!(editor.get_page_rotation(i).unwrap(), 90);
        }
    }

    // =========================================================================
    // crop_margins
    // =========================================================================

    #[test]
    fn test_crop_margins() {
        let mut editor = create_test_editor();
        editor.crop_margins(72.0, 72.0, 72.0, 72.0).unwrap();

        // Should have set crop box for page 0
        let props = editor.modified_page_props.get(&0).unwrap();
        let crop = props.crop_box.unwrap();
        assert!((crop[0] - 72.0).abs() < 0.01); // left margin
        assert!((crop[1] - 72.0).abs() < 0.01); // bottom margin
        assert!((crop[2] - 540.0).abs() < 0.01); // 612 - 72
        assert!((crop[3] - 720.0).abs() < 0.01); // 792 - 72
    }

    // =========================================================================
    // find_max_object_id
    // =========================================================================

    #[test]
    fn test_find_max_object_id() {
        let editor = create_test_editor();
        // Our minimal PDF has /Size 4 and objects 1..3 in its xref, so the
        // maximum object id is 3 (/Size is the entry count, one above it).
        let max = DocumentEditor::find_max_object_id(&editor.source);
        assert_eq!(max, 3);
    }

    // =========================================================================
    // Operator: MoveTo, LineTo, Tm, TD
    // =========================================================================

    #[test]
    fn test_serialize_operator_move_to() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::MoveTo { x: 10.0, y: 20.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("m\n"));
    }

    #[test]
    fn test_serialize_operator_line_to() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::LineTo { x: 100.0, y: 200.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("l\n"));
    }

    #[test]
    fn test_serialize_operator_tm() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::Tm {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                e: 72.0,
                f: 700.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("Tm\n"));
    }

    #[test]
    fn test_serialize_operator_big_td() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::TD { tx: 0.0, ty: -14.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("TD\n"));
    }

    #[test]
    fn test_serialize_operator_set_line_cap() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetLineCap { cap_style: 1 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("J\n"));
    }

    #[test]
    fn test_serialize_operator_set_line_join() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetLineJoin { join_style: 2 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("j\n"));
    }

    #[test]
    fn test_serialize_operator_set_miter_limit() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetMiterLimit { limit: 10.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("M\n"));
    }

    #[test]
    fn test_serialize_operator_set_flatness() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFlatness { tolerance: 50.0 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("i\n"));
    }

    #[test]
    fn test_serialize_operator_set_stroke_cmyk() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeCmyk {
                c: 0.0,
                m: 1.0,
                y: 0.5,
                k: 0.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("K\n"));
    }

    #[test]
    fn test_serialize_operator_set_stroke_rgb() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetStrokeRgb {
                r: 0.0,
                g: 0.5,
                b: 1.0,
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("RG\n"));
    }

    #[test]
    fn test_serialize_operator_set_fill_gray() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::SetFillGray { gray: 0.75 },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.ends_with("g\n"));
    }

    #[test]
    fn test_serialize_operator_begin_marked_content_dict() {
        let editor = create_test_editor();
        let mut output = Vec::new();
        editor.serialize_operator(
            &mut output,
            &crate::content::operators::Operator::BeginMarkedContentDict {
                tag: "Span".to_string(),
                properties: Box::new(Object::Name("MC0".to_string())),
            },
        );
        let s = String::from_utf8(output).unwrap();
        assert!(s.contains("/Span"));
        assert!(s.contains("/MC0"));
        assert!(s.ends_with("BDC\n"));
    }

    // =========================================================================
    // Issue #262: CLI merge creates blank documents
    // =========================================================================

    #[test]
    fn test_merge_from_bytes_preserves_content() {
        // Create two PDFs with distinct text content
        let pdf1_bytes = crate::api::Pdf::from_text("Hello from document one")
            .unwrap()
            .into_bytes();
        let pdf2_bytes = crate::api::Pdf::from_text("Goodbye from document two")
            .unwrap()
            .into_bytes();

        // Merge pdf2 into pdf1
        let mut editor = DocumentEditor::from_bytes(pdf1_bytes).unwrap();
        let merged_count = editor.merge_from_bytes(&pdf2_bytes).unwrap();
        assert_eq!(merged_count, 1, "should report 1 page merged");

        let result_bytes = editor.save_to_bytes().unwrap();

        // Open the merged result and verify both pages have content
        let mut merged_doc = crate::document::PdfDocument::from_bytes(result_bytes).unwrap();
        assert_eq!(merged_doc.page_count().unwrap(), 2, "merged PDF should have 2 pages");

        // Extract text from page 1 (original)
        let text1 = merged_doc.extract_text(0).unwrap();
        assert!(
            text1.contains("Hello from document one"),
            "Page 1 should contain original text, got: {:?}",
            text1
        );

        // Extract text from page 2 (merged)
        let text2 = merged_doc.extract_text(1).unwrap();
        assert!(
            text2.contains("Goodbye from document two"),
            "Page 2 should contain merged text, got: {:?}",
            text2
        );
    }

    #[test]
    fn test_merge_multiple_pages() {
        // Create PDFs with different content
        let pdf1_bytes = crate::api::Pdf::from_text("First").unwrap().into_bytes();
        let pdf2_bytes = crate::api::Pdf::from_text("Second").unwrap().into_bytes();
        let pdf3_bytes = crate::api::Pdf::from_text("Third").unwrap().into_bytes();

        let mut editor = DocumentEditor::from_bytes(pdf1_bytes).unwrap();
        editor.merge_from_bytes(&pdf2_bytes).unwrap();
        editor.merge_from_bytes(&pdf3_bytes).unwrap();

        let result_bytes = editor.save_to_bytes().unwrap();

        let mut merged_doc = crate::document::PdfDocument::from_bytes(result_bytes).unwrap();
        assert_eq!(merged_doc.page_count().unwrap(), 3, "merged PDF should have 3 pages");

        let text1 = merged_doc.extract_text(0).unwrap();
        let text2 = merged_doc.extract_text(1).unwrap();
        let text3 = merged_doc.extract_text(2).unwrap();

        assert!(text1.contains("First"), "Page 1 text: {:?}", text1);
        assert!(text2.contains("Second"), "Page 2 text: {:?}", text2);
        assert!(text3.contains("Third"), "Page 3 text: {:?}", text3);
    }

    /// Regression test: merged PDFs must have extractable text on every page.
    ///
    /// Creates two single-page PDFs with known text content, merges them,
    /// then verifies that extract_text() returns the correct content for
    /// each page of the merged document.
    #[test]
    fn test_merge_text_extractable() {
        let pdf_a = crate::api::Pdf::from_text("Hello from A")
            .unwrap()
            .into_bytes();
        let pdf_b = crate::api::Pdf::from_text("Hello from B")
            .unwrap()
            .into_bytes();

        let mut editor = DocumentEditor::from_bytes(pdf_a).unwrap();
        let pages_merged = editor.merge_from_bytes(&pdf_b).unwrap();
        assert_eq!(pages_merged, 1, "should merge exactly 1 page from B");

        let result = editor.save_to_bytes().unwrap();

        let mut doc = crate::document::PdfDocument::from_bytes(result).unwrap();
        assert_eq!(doc.page_count().unwrap(), 2, "merged PDF must have 2 pages");

        let text_page0 = doc.extract_text(0).unwrap();
        let text_page1 = doc.extract_text(1).unwrap();

        assert!(
            text_page0.contains("Hello from A"),
            "Page 0 should contain 'Hello from A', got: {:?}",
            text_page0
        );
        assert!(
            text_page1.contains("Hello from B"),
            "Page 1 should contain 'Hello from B', got: {:?}",
            text_page1
        );

        // Verify no cross-contamination
        assert!(!text_page0.contains("Hello from B"), "Page 0 should NOT contain text from B");
        assert!(!text_page1.contains("Hello from A"), "Page 1 should NOT contain text from A");
    }

    // =========================================================================
    // Regression: destructive redaction + add_text() interaction on the same
    // page (three related bugs found together — see PR description).
    // =========================================================================

    /// A minimal single-page PDF with two separate text-showing (`Tj`) runs
    /// on one Type1/Helvetica font — unlike `minimal_pdf_bytes()` above,
    /// this page has real extractable text, split into two runs so that
    /// redacting one leaves the other's extraction untouched.
    fn minimal_pdf_with_text_bytes() -> Vec<u8> {
        let content_stream: &[u8] = b"BT\n/F1 12 Tf\n1 0 0 1 100 700 Tm\n(SECRETVALUE) Tj\n\
             1 0 0 1 100 680 Tm\n(public text) Tj\nET\n";
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec());
        objs.push(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec());
        objs.push(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>\nendobj\n"
                .to_vec(),
        );
        objs.push(
            [
                format!("4 0 obj\n<< /Length {} >>\nstream\n", content_stream.len()).into_bytes(),
                content_stream.to_vec(),
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );
        objs.push(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        );

        let header = b"%PDF-1.4\n".to_vec();
        let mut body = header.clone();
        let mut offsets = vec![0u64];
        let mut pos = header.len() as u64;
        for o in &objs {
            offsets.push(pos);
            body.extend_from_slice(o);
            pos += o.len() as u64;
        }
        let xref_offset = pos;
        let mut xref = format!("xref\n0 {}\n", objs.len() + 1).into_bytes();
        xref.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets[1..] {
            xref.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        let trailer = format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref_offset
        )
        .into_bytes();

        [body, xref, trailer].concat()
    }

    /// Writes `minimal_pdf_with_text_bytes()` to a uniquely-named temp file
    /// and opens it as a `DocumentEditor`, mirroring `create_test_editor()`
    /// above but with real extractable text on the page.
    fn create_test_editor_with_text() -> DocumentEditor {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let pdf_bytes = minimal_pdf_with_text_bytes();
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = std::env::temp_dir()
            .join(format!("pdf_oxide_redact_addtext_test_{}_{id}.pdf", std::process::id()));
        std::fs::write(&tmp, &pdf_bytes).unwrap();
        let editor = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        editor
    }

    /// Bug 1 + bug 2 regression: destructive redaction on a page that also
    /// has `add_text()` overlay additions must not silently drop the added
    /// text (bug 1: `/Contents` was overwritten wholesale instead of
    /// preserving the overlay-additions array entry), and the replacement
    /// text's Base-14 font must be usable without breaking extraction of
    /// unrelated text on the same page (bug 2: the font resource was never
    /// registered in `/Resources/Font`).
    #[test]
    fn destructive_redaction_preserves_add_text_overlay() {
        let mut editor = create_test_editor_with_text();

        // Locate "SECRETVALUE" and redact it destructively.
        let bbox = {
            let pe = editor.page_editor(0).unwrap();
            let matches = pe.page.find_text_containing("SECRETVALUE");
            assert_eq!(matches.len(), 1, "expected exactly one match for SECRETVALUE");
            matches[0].bbox()
        };
        let rect = [bbox.x, bbox.y, bbox.x + bbox.width, bbox.y + bbox.height];
        editor
            .add_redaction(0, rect, Some([0.0, 0.0, 0.0]))
            .unwrap();
        let report = editor
            .apply_redactions_destructive(crate::redaction::RedactionOptions::default())
            .unwrap();
        assert!(report.glyphs_removed > 0, "expected some glyphs to be removed");

        // Add replacement text at the same position (Base-14 font, no
        // dependency on the original page's font encoding).
        let mut pe = editor.page_editor(0).unwrap();
        let font = crate::elements::FontSpec {
            name: "Helvetica".to_string(),
            size: 12.0,
        };
        pe.page.add_text(crate::elements::TextContent::new(
            "{{REDACTED}}",
            bbox,
            font,
            crate::elements::TextStyle::default(),
        ));
        let page = pe.done().unwrap();
        editor.save_page_from_editor(page).unwrap();

        let bytes = editor.save_to_bytes().unwrap();

        // Re-open the saved bytes independently.
        let tmp = std::env::temp_dir()
            .join(format!("pdf_oxide_redact_addtext_verify_{}.pdf", std::process::id()));
        std::fs::write(&tmp, &bytes).unwrap();
        let mut verify = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);

        let pe = verify.page_editor(0).unwrap();
        assert!(
            pe.page.find_text_containing("SECRETVALUE").is_empty(),
            "redacted text must not survive"
        );
        assert!(
            !pe.page.find_text_containing("{{REDACTED}}").is_empty(),
            "added replacement text must be present after save (was silently dropped \
             before the /Contents-array-preservation fix)"
        );
        assert!(
            !pe.page.find_text_containing("public text").is_empty(),
            "unrelated text on the same page must survive untouched — if the added \
             text's Base-14 font resource were left undeclared in /Resources/Font, a \
             strict content-stream parser would choke on the added text's `Tf` \
             operator and fail to extract this text too"
        );
    }

    /// Bug 3 regression: two separate `add_text()` + `save_page_from_editor()`
    /// round-trips on the same page must both survive to the saved output.
    /// Before the fix, `save_page()` used `HashMap::insert` on
    /// `overlay_additions`, so the second call silently discarded the first
    /// call's added element instead of accumulating both.
    #[test]
    fn multiple_add_text_calls_on_same_page_all_survive() {
        let mut editor = create_test_editor_with_text();
        let font = || crate::elements::FontSpec {
            name: "Helvetica".to_string(),
            size: 10.0,
        };
        let bbox_a = crate::geometry::Rect {
            x: 50.0,
            y: 50.0,
            width: 100.0,
            height: 12.0,
        };
        let bbox_b = crate::geometry::Rect {
            x: 50.0,
            y: 80.0,
            width: 100.0,
            height: 12.0,
        };

        for (bbox, label) in [(bbox_a, "TOKEN_ONE"), (bbox_b, "TOKEN_TWO")] {
            let mut pe = editor.page_editor(0).unwrap();
            pe.page.add_text(crate::elements::TextContent::new(
                label,
                bbox,
                font(),
                crate::elements::TextStyle::default(),
            ));
            let page = pe.done().unwrap();
            editor.save_page_from_editor(page).unwrap();
        }

        let bytes = editor.save_to_bytes().unwrap();
        let tmp = std::env::temp_dir()
            .join(format!("pdf_oxide_multi_addtext_verify_{}.pdf", std::process::id()));
        std::fs::write(&tmp, &bytes).unwrap();
        let mut verify = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);

        let pe = verify.page_editor(0).unwrap();
        assert!(
            !pe.page.find_text_containing("TOKEN_ONE").is_empty(),
            "first add_text() call must survive a second, separate save_page() call"
        );
        assert!(
            !pe.page.find_text_containing("TOKEN_TWO").is_empty(),
            "second add_text() call must also be present"
        );
    }

    /// A single-page PDF whose *original* `/Contents` is already a
    /// two-element array `[c1 c2]` (ISO 32000-1 §7.7.3.3), with the
    /// page's `/Resources` **and** `/Resources/Font` reached through
    /// *indirect* references (the common shape for a page loaded from a
    /// real document). `c1` holds the secret, `c2` holds unrelated text.
    fn multi_stream_indirect_resources_pdf_bytes() -> Vec<u8> {
        let c1: &[u8] = b"BT\n/F1 12 Tf\n1 0 0 1 100 700 Tm\n(SECRETVALUE) Tj\nET\n";
        let c2: &[u8] = b"BT\n/F1 12 Tf\n1 0 0 1 100 680 Tm\n(public text) Tj\nET\n";
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec());
        objs.push(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec());
        // Page: /Contents is an array [4 0 R 5 0 R]; /Resources is an
        // indirect reference to obj 6; obj 6's /Font is an indirect
        // reference to obj 7.
        objs.push(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Contents [4 0 R 5 0 R] /Resources 6 0 R >>\nendobj\n"
                .to_vec(),
        );
        objs.push(
            [
                format!("4 0 obj\n<< /Length {} >>\nstream\n", c1.len()).into_bytes(),
                c1.to_vec(),
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );
        objs.push(
            [
                format!("5 0 obj\n<< /Length {} >>\nstream\n", c2.len()).into_bytes(),
                c2.to_vec(),
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );
        objs.push(b"6 0 obj\n<< /Font 7 0 R >>\nendobj\n".to_vec());
        objs.push(b"7 0 obj\n<< /F1 8 0 R >>\nendobj\n".to_vec());
        objs.push(
            b"8 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        );

        let header = b"%PDF-1.4\n".to_vec();
        let mut body = header.clone();
        let mut offsets = vec![0u64];
        let mut pos = header.len() as u64;
        for o in &objs {
            offsets.push(pos);
            body.extend_from_slice(o);
            pos += o.len() as u64;
        }
        let xref_offset = pos;
        let mut xref = format!("xref\n0 {}\n", objs.len() + 1).into_bytes();
        xref.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets[1..] {
            xref.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        let trailer = format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref_offset
        )
        .into_bytes();

        [body, xref, trailer].concat()
    }

    fn open_editor_from_bytes(bytes: &[u8], tag: &str) -> DocumentEditor {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp =
            std::env::temp_dir().join(format!("pdf_oxide_{tag}_{}_{id}.pdf", std::process::id()));
        std::fs::write(&tmp, bytes).unwrap();
        let editor = DocumentEditor::open(&tmp).unwrap();
        let _ = std::fs::remove_file(&tmp);
        editor
    }

    /// Assert that every reference in the page's `/Contents` array points
    /// to an object that is **physically defined** in the saved bytes —
    /// i.e. no *dangling* reference. This is the property strict validators
    /// (veraPDF / Poppler strict mode) reject and the one the multi-stream
    /// `/Contents` must-fix is about.
    ///
    /// This deliberately works on the raw saved bytes rather than through
    /// `DocumentEditor::load_object`: a missing object is silently
    /// recovered by `load_object` (file-scan fallback, or `Object::Null`
    /// per §7.3.10), which would *mask* a dangling reference. A strict
    /// validator has no such tolerance, so the test must not either.
    fn assert_no_dangling_contents_in_bytes(bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        // Find the page object's /Contents array. Our fixtures have a
        // single /Type /Page; locate its dict and the [...] after
        // /Contents.
        let page_pos = text
            .find("/Type /Page")
            .or_else(|| text.find("/Type/Page"))
            .expect("no /Type /Page object found in saved bytes");
        let contents_pos = text[page_pos..]
            .find("/Contents")
            .map(|p| page_pos + p)
            .expect("page has no /Contents");
        let after = &text[contents_pos + "/Contents".len()..];
        let after = after.trim_start();
        assert!(
            after.starts_with('['),
            "expected an array /Contents in this multi-stream fixture, got: {}",
            &after[..after.len().min(40)]
        );
        let close = after.find(']').expect("unterminated /Contents array");
        let inner = &after[1..close];

        // Parse "N G R" triples out of the array.
        let toks: Vec<&str> = inner.split_whitespace().collect();
        let mut ids: Vec<u32> = Vec::new();
        let mut i = 0;
        while i + 2 < toks.len() + 1 && i + 2 <= toks.len() {
            if toks.get(i + 2) == Some(&"R") {
                if let Ok(id) = toks[i].parse::<u32>() {
                    ids.push(id);
                }
                i += 3;
            } else {
                i += 1;
            }
        }
        assert!(!ids.is_empty(), "no object references parsed from /Contents array");

        for id in ids {
            // A defined object appears as "\n<id> <gen> obj". Accept any
            // generation number.
            let defined =
                text.contains(&format!("\n{id} 0 obj")) || text.starts_with(&format!("{id} 0 obj"));
            assert!(
                defined,
                "dangling /Contents reference to object {id}: it appears in the \
                 array but no `{id} 0 obj` is defined in the saved file — strict \
                 validators (veraPDF / Poppler strict mode) reject this"
            );
        }
    }

    /// Must-fix 1 regression: a page whose *original* `/Contents` is
    /// already a `[c1 c2]` array, redacted destructively and then given an
    /// `add_text()` overlay, must (a) drop the secret, (b) keep the
    /// unrelated stream's text, (c) keep the overlay, and (d) leave **no
    /// dangling `/Contents` reference** — the entry-0-only replacement in
    /// the original PR left `c2` dangling. Also exercises indirect
    /// `/Resources` and `/Resources/Font` (the headline complexity), which
    /// the earlier tests did not cover.
    #[test]
    fn destructive_redaction_multi_stream_contents_no_dangling_ref() {
        let mut editor =
            open_editor_from_bytes(&multi_stream_indirect_resources_pdf_bytes(), "multistream");

        let bbox = {
            let pe = editor.page_editor(0).unwrap();
            let matches = pe.page.find_text_containing("SECRETVALUE");
            assert_eq!(matches.len(), 1, "expected exactly one match for SECRETVALUE");
            matches[0].bbox()
        };
        let rect = [bbox.x, bbox.y, bbox.x + bbox.width, bbox.y + bbox.height];
        editor
            .add_redaction(0, rect, Some([0.0, 0.0, 0.0]))
            .unwrap();
        let report = editor
            .apply_redactions_destructive(crate::redaction::RedactionOptions::default())
            .unwrap();
        assert!(report.glyphs_removed > 0, "expected some glyphs to be removed");

        let mut pe = editor.page_editor(0).unwrap();
        let font = crate::elements::FontSpec {
            name: "Helvetica".to_string(),
            size: 12.0,
        };
        pe.page.add_text(crate::elements::TextContent::new(
            "{{REDACTED}}",
            bbox,
            font,
            crate::elements::TextStyle::default(),
        ));
        let page = pe.done().unwrap();
        editor.save_page_from_editor(page).unwrap();

        // ── pdf_manipulator patch: the assertions read plain bytes, so no object streams ──
        let bytes = editor.save_to_bytes_with_options(SaveOptions {
            object_streams: false,
            ..SaveOptions::full_rewrite()
        }).unwrap();
        // ── end pdf_manipulator patch ──

        // (d) No dangling /Contents reference after the multi-stream
        // rewrite — this is the core of must-fix 1. Checked on the raw
        // bytes, since load_object() would silently mask a missing object.
        assert_no_dangling_contents_in_bytes(&bytes);

        let mut verify = open_editor_from_bytes(&bytes, "multistream_verify");

        let pe = verify.page_editor(0).unwrap();
        // (a) secret gone.
        assert!(
            pe.page.find_text_containing("SECRETVALUE").is_empty(),
            "redacted text must not survive"
        );
        // (b) unrelated original stream (c2) text survives.
        assert!(
            !pe.page.find_text_containing("public text").is_empty(),
            "text from the *second* original content stream must survive the redaction"
        );
        // (c) overlay survives.
        assert!(
            !pe.page.find_text_containing("{{REDACTED}}").is_empty(),
            "add_text() overlay must survive destructive redaction of a multi-stream page"
        );
    }

    /// Must-fix 2 regression: a **bold** overlay makes the writer emit
    /// `/Helvetica-Bold` via `map_font_name`, so the registration must key
    /// off that transformed name — not the raw `"Helvetica"` — or the
    /// `Tf` operator references an undeclared font and a strict parser
    /// chokes on the overlay (and, via the shared stream, unrelated text).
    #[test]
    fn styled_overlay_font_registered_under_emitted_name() {
        let mut editor = create_test_editor_with_text();

        let bbox = crate::geometry::Rect {
            x: 100.0,
            y: 600.0,
            width: 120.0,
            height: 14.0,
        };
        let mut pe = editor.page_editor(0).unwrap();
        let font = crate::elements::FontSpec {
            name: "Helvetica".to_string(),
            size: 12.0,
        };
        let style = crate::elements::TextStyle {
            weight: crate::layout::FontWeight::Bold,
            ..crate::elements::TextStyle::default()
        };
        pe.page
            .add_text(crate::elements::TextContent::new("BOLDTOKEN", bbox, font, style));
        let page = pe.done().unwrap();
        editor.save_page_from_editor(page).unwrap();

        // ── pdf_manipulator patch: the assertions read plain bytes, so no object streams ──
        let bytes = editor.save_to_bytes_with_options(SaveOptions {
            object_streams: false,
            ..SaveOptions::full_rewrite()
        }).unwrap();
        // ── end pdf_manipulator patch ──
        let mut verify = open_editor_from_bytes(&bytes, "boldfont_verify");

        // The bold overlay text and the original page text must both
        // extract cleanly: if `/Helvetica-Bold` were unregistered while
        // the stream emits `/Helvetica-Bold Tf`, a strict parser would
        // fail on the overlay's `Tf`.
        let pe = verify.page_editor(0).unwrap();
        assert!(
            !pe.page.find_text_containing("BOLDTOKEN").is_empty(),
            "bold overlay text must survive — the emitted /Helvetica-Bold must be registered"
        );
        assert!(
            !pe.page.find_text_containing("public text").is_empty(),
            "unrelated original text must still extract"
        );

        // The emitted font name (/Helvetica-Bold) must be present in the
        // saved bytes as a registered **/BaseFont** — not merely as the
        // `/Helvetica-Bold Tf` operand inside the content stream (which is
        // there regardless of registration). Checking for the `/BaseFont`
        // key proves registration keyed off the transformed name rather
        // than the raw "Helvetica".
        let haystack = String::from_utf8_lossy(&bytes);
        assert!(
            haystack.contains("/BaseFont /Helvetica-Bold")
                || haystack.contains("/BaseFont/Helvetica-Bold"),
            "a Font object with /BaseFont /Helvetica-Bold must be written — the \
             overlay stream emits `/Helvetica-Bold Tf`, so registering the raw \
             \"Helvetica\" leaves that Tf dangling"
        );
    }

    /// A single-page PDF where the page has NO own `/Resources` and inherits
    /// it from the `/Pages` node. The inherited `/Resources` carries both a
    /// `/Font` sub-dict (needed by the page's text) and an `/XObject`
    /// sub-dict referencing a Form XObject (`/SecretForm`) whose stream
    /// contains sensitive text. This models the shape behind the
    /// inherited-/Resources leak concern.
    fn inherited_resources_with_xobject_pdf_bytes() -> Vec<u8> {
        let content: &[u8] = b"BT\n/F1 12 Tf\n1 0 0 1 100 700 Tm\n(visible text) Tj\nET\n";
        let secret: &[u8] = b"BT\n/F1 12 Tf\n1 0 0 1 10 10 Tm\n(XOBJECTSECRET) Tj\nET\n";
        let mut objs: Vec<Vec<u8>> = Vec::new();
        objs.push(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec());
        // /Pages node holds the inheritable /Resources (Font + XObject).
        objs.push(
            b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 \
              /Resources << /Font << /F1 5 0 R >> /XObject << /SecretForm 6 0 R >> >> >>\nendobj\n"
                .to_vec(),
        );
        // Page: NO own /Resources — must inherit from /Pages (§7.7.3.4).
        objs.push(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
              /Contents 4 0 R >>\nendobj\n"
                .to_vec(),
        );
        objs.push(
            [
                format!("4 0 obj\n<< /Length {} >>\nstream\n", content.len()).into_bytes(),
                content.to_vec(),
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );
        objs.push(
            b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        );
        objs.push(
            [
                format!(
                    "6 0 obj\n<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Length {} >>\nstream\n",
                    secret.len()
                )
                .into_bytes(),
                secret.to_vec(),
                b"\nendstream\nendobj\n".to_vec(),
            ]
            .concat(),
        );

        let header = b"%PDF-1.4\n".to_vec();
        let mut body = header.clone();
        let mut offsets = vec![0u64];
        let mut pos = header.len() as u64;
        for o in &objs {
            offsets.push(pos);
            body.extend_from_slice(o);
            pos += o.len() as u64;
        }
        let xref_offset = pos;
        let mut xref = format!("xref\n0 {}\n", objs.len() + 1).into_bytes();
        xref.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets[1..] {
            xref.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
        }
        let trailer = format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref_offset
        )
        .into_bytes();

        [body, xref, trailer].concat()
    }

    /// Adversarial-review regression: when overlay text forces the page's
    /// `/Resources` to be materialised and the page inherits `/Resources`
    /// from an ancestor, the materialised dict must seed ONLY the inherited
    /// `/Font` — never the inherited `/XObject` (or other content-bearing
    /// resources). Inlining an inherited Form XObject reference onto the
    /// page would make that stream freshly reachable and let it survive a
    /// later destructive-redaction orphan-drop, re-introducing a stray
    /// reference. The overlay font must still be registered so the added
    /// text's `Tf` does not dangle.
    #[test]
    fn overlay_does_not_inline_inherited_xobject_into_page_resources() {
        let mut editor =
            open_editor_from_bytes(&inherited_resources_with_xobject_pdf_bytes(), "inherit_xobj");

        let bbox = crate::geometry::Rect {
            x: 100.0,
            y: 500.0,
            width: 120.0,
            height: 14.0,
        };
        let mut pe = editor.page_editor(0).unwrap();
        let font = crate::elements::FontSpec {
            name: "Helvetica".to_string(),
            size: 12.0,
        };
        pe.page.add_text(crate::elements::TextContent::new(
            "OVERLAYTOKEN",
            bbox,
            font,
            crate::elements::TextStyle::default(),
        ));
        let page = pe.done().unwrap();
        editor.save_page_from_editor(page).unwrap();

        let bytes = editor.save_to_bytes().unwrap();
        let mut verify = open_editor_from_bytes(&bytes, "inherit_xobj_verify");

        // Inspect the saved page's OWN /Resources (now materialised).
        let page_ref = verify.source.get_page_ref(0).unwrap();
        let page_obj = verify.source.load_object(page_ref).unwrap();
        let page_dict = page_obj.as_dict().unwrap();
        let res_obj = page_dict
            .get("Resources")
            .cloned()
            .expect("page should have a materialised /Resources after overlay");
        let res_dict = match res_obj {
            Object::Dictionary(d) => d,
            Object::Reference(r) => verify
                .source
                .load_object(r)
                .unwrap()
                .as_dict()
                .cloned()
                .unwrap(),
            other => panic!("unexpected /Resources object: {other:?}"),
        };

        // The overlay font must be registered (Tf must resolve)...
        assert!(
            res_dict.contains_key("Font"),
            "materialised /Resources must carry a /Font sub-dict for the overlay"
        );
        // ...but the inherited /XObject must NOT have been inlined onto the page.
        assert!(
            !res_dict.contains_key("XObject"),
            "inherited /XObject must not be copied into the page's /Resources — \
             doing so re-references the Form XObject and would defeat a later \
             destructive-redaction orphan-drop"
        );

        // The overlay text still extracts.
        let pe = verify.page_editor(0).unwrap();
        assert!(
            !pe.page.find_text_containing("OVERLAYTOKEN").is_empty(),
            "overlay text must survive with its font registered"
        );
    }
}
