//! Parser for PDF structure trees.
//!
//! Parses StructTreeRoot and StructElem dictionaries according to PDF spec Section 14.7.

use super::types::{StructChild, StructElem, StructTreeRoot, StructType};
use crate::document::PdfDocument;
use crate::error::Error;
use crate::object::Object;
use std::collections::{HashMap, HashSet};

use std::time::Duration;

// ── pdf_manipulator patch: the budget is a wall-clock bound only; the element cap is gone ──
/// An optional bound on structure-tree parsing.
///
/// A parsing library returns the *complete* tree by default, so the unbounded
/// guard (`ParseBudget::unbounded`) applies no limit. Callers that deliberately
/// want to bound the work — e.g. the extraction reading-order strategy, which
/// can fall back to content-stream order — build a bounded guard from a
/// wall-clock `Duration`. On `wasm32-unknown-unknown` `std::time::Instant`
/// panics, so the limit is a no-op there.
#[derive(Clone, Copy, Default)]
pub(crate) struct ParseBudget {
    #[cfg(not(target_arch = "wasm32"))]
    deadline: Option<std::time::Instant>,
}

impl ParseBudget {
    /// No limit — parse the entire tree (the default for a general-purpose API).
    pub(crate) fn unbounded() -> Self {
        Self::default()
    }

    /// Bound parsing by `budget` wall-clock time.
    fn from_option(budget: Option<Duration>) -> Self {
        match budget {
            None => Self::unbounded(),
            Some(_budget) => Self {
                #[cfg(not(target_arch = "wasm32"))]
                deadline: Some(std::time::Instant::now() + _budget),
            },
        }
    }

    /// Whether the deadline has passed. The element count is not a bound; the
    /// parameter keeps upstream's signature, so its callers and tests merge
    /// unchanged.
    #[inline]
    fn exceeded(&self, _element_count: usize) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(deadline) = self.deadline {
            return std::time::Instant::now() > deadline;
        }
        false
    }
}
// ── end pdf_manipulator patch ──

/// A timer for measuring elapsed time, WASM-safe.
#[derive(Clone, Copy)]
struct Timer {
    #[cfg(not(target_arch = "wasm32"))]
    start: std::time::Instant,
}

impl Timer {
    fn now() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                start: std::time::Instant::now(),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {}
        }
    }

    fn elapsed_debug(&self) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        {
            format!("{:?}", self.start.elapsed())
        }
        #[cfg(target_arch = "wasm32")]
        {
            "(time unavailable on wasm)".to_string()
        }
    }
}

// ── pdf_manipulator patch: MAX_STRUCT_ELEMENTS is removed; the tree is walked, not capped ──
// ── end pdf_manipulator patch ──

/// Decode a PDF text string (UTF-16BE/LE with BOM, or PDFDocEncoding).
fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        // UTF-16BE with BOM
        let utf16_pairs: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&utf16_pairs)
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string())
    } else if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        // UTF-16LE with BOM
        let utf16_pairs: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&utf16_pairs)
            .unwrap_or_else(|_| String::from_utf8_lossy(bytes).to_string())
    } else {
        // PDFDocEncoding
        bytes
            .iter()
            .filter_map(|&b| crate::fonts::font_dict::pdfdoc_encoding_lookup(b))
            .collect()
    }
}

/// Helper function to resolve an object (handles both direct objects and references).
fn resolve_object(document: &PdfDocument, obj: &Object) -> Result<Object, Error> {
    match obj {
        Object::Reference(obj_ref) => document.load_object(*obj_ref),
        _ => Ok(obj.clone()),
    }
}

/// Build a mapping from page object IDs to page indices.
/// This allows resolving /Pg references in marked content references.
///
/// Uses a single-pass traversal of the page tree (O(n)) instead of
/// calling get_page_ref per page (which is O(n) per call → O(n²) total).
fn build_page_map(document: &PdfDocument) -> HashMap<u32, u32> {
    let mut page_map = HashMap::new();

    // Get the root Pages node from the catalog
    let pages_ref = match document.catalog().ok().and_then(|cat| {
        cat.as_dict()
            .and_then(|d| d.get("Pages"))
            .and_then(|p| p.as_reference())
    }) {
        Some(r) => r,
        None => return page_map,
    };

    let mut index: u32 = 0;
    build_page_map_recursive(document, pages_ref, &mut page_map, &mut index);
    page_map
}

/// Recursively walk the page tree once, collecting page object IDs.
fn build_page_map_recursive(
    document: &PdfDocument,
    node_ref: crate::object::ObjectRef,
    page_map: &mut HashMap<u32, u32>,
    index: &mut u32,
) {
    let node = match document.load_object(node_ref) {
        Ok(n) => n,
        Err(_) => return,
    };
    let dict = match node.as_dict() {
        Some(d) => d,
        None => return,
    };
    let node_type = dict.get("Type").and_then(|t| t.as_name()).unwrap_or("");

    match node_type {
        "Page" => {
            page_map.insert(node_ref.id, *index);
            *index += 1;
        },
        "Pages" => {
            if let Some(kids) = dict.get("Kids").and_then(|k| k.as_array()) {
                let kid_refs: Vec<_> = kids.iter().filter_map(|k| k.as_reference()).collect();
                for kid_ref in kid_refs {
                    build_page_map_recursive(document, kid_ref, page_map, index);
                }
            }
        },
        _ => {},
    }
}

/// Parse the complete structure tree from a PDF document.
///
/// Reads the `/StructTreeRoot` from the catalog and recursively parses every
/// structure element. This is the general-purpose entry point: it applies **no**
/// time budget and **no** element cap, so it returns the whole tree however large
/// it is. Callers that need to bound the work (e.g. to keep an interactive
/// pipeline responsive and fall back to another strategy) should use
/// [`parse_structure_tree_with_budget`].
///
/// # Returns
/// * `Ok(Some(StructTreeRoot))` - the document's structure tree
/// * `Ok(None)` - the document is not tagged (no `/StructTreeRoot`)
/// * `Err(Error)` - parsing failed
pub fn parse_structure_tree(document: &PdfDocument) -> Result<Option<StructTreeRoot>, Error> {
    parse_structure_tree_inner(document, ParseBudget::unbounded())
}

/// Parse the structure tree with an optional bound on the work done.
///
/// `budget` is a wall-clock limit: `None` parses the complete tree (identical to
/// [`parse_structure_tree`]); `Some(duration)` stops once the deadline is
/// exceeded (or a companion element cap is hit) and returns `Ok(None)` so the
/// caller can fall back to another strategy. Prefer `None` unless you have a
/// concrete responsiveness requirement — a bound can drop the tree on a slow
/// machine even when the document is perfectly valid.
pub fn parse_structure_tree_with_budget(
    document: &PdfDocument,
    budget: Option<Duration>,
) -> Result<Option<StructTreeRoot>, Error> {
    parse_structure_tree_inner(document, ParseBudget::from_option(budget))
}

// ── pdf_manipulator patch: the structure tree is walked with an explicit stack, not parsed recursively and held whole ──
/// What the walk needs before it visits the first element: the root's
/// `/RoleMap`, its `/K` and the page-object → page-index map.
pub(crate) struct StructHeader {
    pub(crate) role_map: HashMap<String, String>,
    k: Option<Object>,
    page_map: HashMap<u32, u32>,
}

/// An element as the walk meets it, before its children.
pub(crate) struct ElemHead {
    pub(crate) struct_type: StructType,
    pub(crate) source_role: Option<String>,
    pub(crate) page: Option<u32>,
    pub(crate) actual_text: Option<String>,
}

/// What a walk reports: an element opens, marked content it names, the element
/// closes. Nothing of an element outlives its `leave` unless the visitor keeps it.
pub(crate) trait StructVisitor {
    fn enter(&mut self, head: ElemHead);
    fn marked_content(&mut self, mcid: u32, page: u32, scope: crate::structure::McidScope);
    fn leave(&mut self);
}

/// Builds elements from the events of a walk: `leave` returns an element
/// once the stack is empty again, that is, when it was a top-level one.
#[derive(Default)]
pub(crate) struct Builder {
    stack: Vec<StructElem>,
}

impl Builder {
    pub(crate) fn enter(&mut self, head: ElemHead) {
        let mut elem = StructElem::new(head.struct_type);
        elem.source_role = head.source_role;
        elem.page = head.page;
        elem.actual_text = head.actual_text;
        self.stack.push(elem);
    }

    pub(crate) fn marked_content(
        &mut self,
        mcid: u32,
        page: u32,
        scope: crate::structure::McidScope,
    ) {
        if let Some(top) = self.stack.last_mut() {
            top.add_child(StructChild::MarkedContentRef { mcid, page, scope });
        }
    }

    /// Closes the open element; returns it when it was a top-level one.
    pub(crate) fn leave(&mut self) -> Option<StructElem> {
        let done = self.stack.pop()?;
        match self.stack.last_mut() {
            Some(parent) => {
                parent.add_child(StructChild::StructElem(Box::new(done)));
                None
            },
            None => Some(done),
        }
    }
}

/// Reads the head of the structure tree: `Ok(None)` when the document is not
/// tagged (no `/StructTreeRoot`, or one that is not a dictionary).
pub(crate) fn open_structure_tree(document: &PdfDocument) -> Result<Option<StructHeader>, Error> {
    let catalog = document.catalog()?;
    let catalog_dict = catalog
        .as_dict()
        .ok_or_else(|| Error::InvalidPdf("Catalog is not a dictionary".into()))?;
    let struct_tree_root_ref = match catalog_dict.get("StructTreeRoot") {
        Some(obj) => obj,
        None => return Ok(None),
    };
    let page_map = build_page_map(document);
    let struct_tree_root_obj = resolve_object(document, struct_tree_root_ref)?;
    let struct_tree_dict = match struct_tree_root_obj.as_dict() {
        Some(d) => d,
        None => {
            log::warn!(
                "StructTreeRoot resolved to {} (expected dictionary), treating as no structure tree",
                struct_tree_root_obj.type_name()
            );
            return Ok(None);
        },
    };
    let mut role_map = HashMap::new();
    if let Some(role_map_obj) = struct_tree_dict.get("RoleMap") {
        let role_map_obj = resolve_object(document, role_map_obj)?;
        if let Some(role_map_dict) = role_map_obj.as_dict() {
            for (key, value) in role_map_dict.iter() {
                if let Some(name) = value.as_name() {
                    role_map.insert(key.clone(), name.to_string());
                }
            }
        }
    }
    let k = match struct_tree_dict.get("K") {
        Some(k_obj) => Some(resolve_object(document, k_obj)?),
        None => None,
    };
    Ok(Some(StructHeader {
        role_map,
        k,
        page_map,
    }))
}

/// The children of one open element still to visit. `single` is a `/K` that
/// was not an array; `root` the children of the `/StructTreeRoot`.
struct Frame {
    kids: Vec<Object>,
    next: usize,
    single: bool,
    root: bool,
    page: Option<u32>,
}

struct Opened {
    head: ElemHead,
    kids: Option<(Vec<Object>, bool)>,
}

struct Walk<'a> {
    document: &'a PdfDocument,
    header: &'a StructHeader,
    /// Object ids already met, so a cycle ends. Two integers per element met.
    visited: HashSet<(u32, u16)>,
}

impl Walk<'_> {
    fn first_visit(&mut self, r: &crate::object::ObjectRef) -> bool {
        self.visited.insert((r.id, r.gen))
    }

    /// Reads the element `obj` names, or `None` when it is not one (the caller
    /// may still read it as a marked-content reference).
    fn open_element(&mut self, obj: &Object) -> Result<Option<Opened>, Error> {
        let mut obj = resolve_object(self.document, obj)?;
        loop {
            // An OBJR dictionary is followed to the object it names (§14.7.4).
            let followed: Option<Object> = {
                let Some(dict) = obj.as_dict() else {
                    return Ok(None);
                };
                match dict.get("Type").and_then(|t| t.as_name()) {
                    Some("OBJR") => {
                        let Some(obj_ref) = dict.get("Obj").and_then(|o| o.as_reference()) else {
                            return Ok(None);
                        };
                        if !self.first_visit(&obj_ref) {
                            log::debug!(
                                "OBJR: cycle detected — object {} already visited, skipping",
                                obj_ref.id
                            );
                            return Ok(None);
                        }
                        match self.document.load_object(obj_ref) {
                            Ok(resolved) => Some(resolve_object(self.document, &resolved)?),
                            Err(e) => {
                                log::warn!(
                                    "OBJR: failed to load /Obj {} {}: {}",
                                    obj_ref.id,
                                    obj_ref.gen,
                                    e
                                );
                                return Ok(None);
                            },
                        }
                    },
                    Some(name) if name != "StructElem" => return Ok(None),
                    _ => None,
                }
            };
            match followed {
                Some(next) => obj = next,
                None => break,
            }
        }
        let dict = obj.as_dict().expect("checked above");

        let Some(s_name) = dict.get("S").and_then(|s| s.as_name()) else {
            return Ok(None);
        };
        let mapped = self.header.role_map.get(s_name);
        let struct_type = StructType::from_str(mapped.map(|s| s.as_str()).unwrap_or(s_name));
        let source_role = mapped.is_some().then(|| s_name.to_string());
        let page = match dict.get("Pg") {
            Some(Object::Reference(pg_ref)) => self.header.page_map.get(&pg_ref.id).copied(),
            _ => None,
        };
        let mut actual_text = None;
        if let Some(at_obj) = dict.get("ActualText") {
            let at_obj = resolve_object(self.document, at_obj)?;
            if let Some(at_bytes) = at_obj.as_string() {
                let text = decode_pdf_text_string(at_bytes);
                if !text.is_empty() {
                    actual_text = Some(text);
                }
            }
        }

        let kids = match dict.get("K") {
            None => None,
            // A /K that is an indirect dictionary keeps its id for the cycle check.
            Some(Object::Reference(r)) => match self.document.load_object(*r) {
                Err(e) => {
                    log::warn!("Failed to load /K reference {}: {}", r.id, e);
                    None
                },
                Ok(k) => {
                    if k.as_dict().is_some() && !self.first_visit(r) {
                        log::warn!(
                            "Cycle in structure tree: /K object {} already visited, skipping children",
                            r.id
                        );
                        None
                    } else {
                        kids_of(k)
                    }
                },
            },
            Some(raw) => kids_of(resolve_object(self.document, raw)?),
        };
        Ok(Some(Opened {
            head: ElemHead {
                struct_type,
                source_role,
                page,
                actual_text,
            },
            kids,
        }))
    }
}

/// The children a resolved `/K` holds: an array's items, or the one object.
fn kids_of(k: Object) -> Option<(Vec<Object>, bool)> {
    match k {
        Object::Array(arr) => Some((arr, false)),
        Object::Integer(_) | Object::Dictionary(_) | Object::Reference(_) => Some((vec![k], true)),
        _ => None,
    }
}

/// Visits the structure tree depth-first, in document order. The stack holds
/// the children still to visit of each open element, never the elements.
/// Returns `false` when the budget ran out before the walk finished.
pub(crate) fn walk_structure_tree(
    document: &PdfDocument,
    header: &StructHeader,
    deadline: ParseBudget,
    visitor: &mut dyn StructVisitor,
) -> Result<bool, Error> {
    let mut walk = Walk {
        document,
        header,
        visited: HashSet::new(),
    };
    let mut stack: Vec<Frame> = Vec::new();
    if let Some(k) = &header.k {
        if let Some((kids, single)) = kids_of(k.clone()) {
            stack.push(Frame {
                kids,
                next: 0,
                single,
                root: true,
                page: None,
            });
        }
    }
    while let Some(frame) = stack.last_mut() {
        if deadline.exceeded(0) {
            log::debug!("Structure tree parse budget exceeded, falling back to content order");
            return Ok(false);
        }
        if frame.next >= frame.kids.len() {
            let done = stack.pop().expect("a frame was open");
            if !done.root {
                visitor.leave();
            }
            continue;
        }
        let item = std::mem::replace(&mut frame.kids[frame.next], Object::Null);
        frame.next += 1;
        let (single, root, parent_page) = (frame.single, frame.root, frame.page);

        // What the item is, once the cycle guard and the indirection are past.
        let candidate: Option<Object> = if root {
            if !single {
                if let Object::Reference(r) = &item {
                    if !walk.first_visit(r) {
                        log::warn!(
                            "Cycle in structure tree: root object {} already visited, skipping",
                            r.id
                        );
                        continue;
                    }
                }
            }
            Some(item)
        } else {
            match item {
                Object::Integer(mcid) => {
                    let page = parent_page.unwrap_or(0);
                    visitor.marked_content(
                        mcid as u32,
                        page,
                        crate::structure::McidScope::Page(page),
                    );
                    None
                },
                Object::Dictionary(_) => Some(item),
                Object::Reference(r) => {
                    if !walk.first_visit(&r) {
                        log::warn!(
                            "Cycle in structure tree: object {} already visited, skipping",
                            r.id
                        );
                        continue;
                    }
                    if single {
                        match document.load_object(r) {
                            Ok(resolved) => Some(resolved),
                            Err(e) => {
                                log::warn!("Failed to resolve ObjectRef {} {}: {}", r.id, r.gen, e);
                                None
                            },
                        }
                    } else {
                        match resolve_object(document, &Object::Reference(r))? {
                            Object::Integer(mcid) => {
                                let page = parent_page.unwrap_or(0);
                                visitor.marked_content(
                                    mcid as u32,
                                    page,
                                    crate::structure::McidScope::Page(page),
                                );
                                None
                            },
                            Object::Dictionary(d) => Some(Object::Dictionary(d)),
                            Object::Reference(r2) => {
                                if !walk.first_visit(&r2) {
                                    log::warn!(
                                        "Cycle in structure tree: object {} already visited, skipping",
                                        r2.id
                                    );
                                    continue;
                                }
                                match document.load_object(r2) {
                                    Ok(resolved) => Some(resolved),
                                    Err(e) => {
                                        log::warn!(
                                            "Failed to resolve ObjectRef {} {}: {}",
                                            r2.id,
                                            r2.gen,
                                            e
                                        );
                                        None
                                    },
                                }
                            },
                            _ => None,
                        }
                    }
                },
                _ => None,
            }
        };
        let Some(candidate) = candidate else { continue };

        match walk.open_element(&candidate)? {
            Some(opened) => {
                let page = opened.head.page;
                visitor.enter(opened.head);
                let (kids, single) = opened.kids.unwrap_or_default();
                stack.push(Frame {
                    kids,
                    next: 0,
                    single,
                    root: false,
                    page,
                });
            },
            // A root item that is not an element is dropped; elsewhere it may be an MCR.
            None if root => {},
            None => {
                if let Some(StructChild::MarkedContentRef { mcid, page, scope }) =
                    parse_marked_content_ref(document, &candidate, &header.page_map)?
                {
                    visitor.marked_content(mcid, page, scope);
                }
            },
        }
    }
    Ok(true)
}

struct TreeBuilder {
    builder: Builder,
    roots: Vec<StructElem>,
}

impl StructVisitor for TreeBuilder {
    fn enter(&mut self, head: ElemHead) {
        self.builder.enter(head);
    }
    fn marked_content(&mut self, mcid: u32, page: u32, scope: crate::structure::McidScope) {
        self.builder.marked_content(mcid, page, scope);
    }
    fn leave(&mut self) {
        if let Some(done) = self.builder.leave() {
            self.roots.push(done);
        }
    }
}

fn parse_structure_tree_inner(
    document: &PdfDocument,
    deadline: ParseBudget,
) -> Result<Option<StructTreeRoot>, Error> {
    let parse_start = Timer::now();
    let Some(header) = open_structure_tree(document)? else {
        return Ok(None);
    };
    let mut tree = TreeBuilder {
        builder: Builder::default(),
        roots: Vec::new(),
    };
    if !walk_structure_tree(document, &header, deadline, &mut tree)? {
        return Ok(None);
    }
    let mut struct_tree = StructTreeRoot::new();
    struct_tree.role_map = header.role_map;
    for root in tree.roots {
        struct_tree.add_root_element(root);
    }
    log::debug!(
        "Structure tree parsed: {} root elements in {}",
        struct_tree.root_elements.len(),
        parse_start.elapsed_debug()
    );
    Ok(Some(struct_tree))
}
// ── end pdf_manipulator patch ──

/// Parse a marked content reference dictionary.
///
/// According to ISO 32000-1:2008 §14.7.5.4.2, a marked content reference
/// (MCR) dictionary has:
/// - `/Type /MCR` (optional but, when present, fixes the dict's identity)
/// - `/Pg` — page reference (optional; inherits enclosing StructElem
///   `/Pg` when absent)
/// - `/MCID` — required marked-content identifier
/// - `/Stm` — optional reference to the content stream that holds the
///   MCID. When absent, the MCID lives in the page's own content
///   stream. When present, the stream's dictionary determines the
///   scope:
///     - `/Subtype /Form` → Form XObject scope
///     - `/PatternType 1` → Tiling Pattern scope
///     - anything else → falls back to Page scope with a debug log,
///       since we can't classify the stream.
fn parse_marked_content_ref(
    document: &PdfDocument,
    obj: &Object,
    page_map: &HashMap<u32, u32>,
) -> Result<Option<StructChild>, Error> {
    let dict = match obj.as_dict() {
        Some(d) => d,
        None => return Ok(None),
    };

    // Check for /Type /MCR
    if let Some(type_obj) = dict.get("Type") {
        if let Some(type_name) = type_obj.as_name() {
            if type_name != "MCR" {
                return Ok(None);
            }
        }
    }

    // Get /MCID
    let mcid = match dict.get("MCID").and_then(|obj| obj.as_integer()) {
        Some(mcid) => mcid,
        None => return Ok(None), // Missing /MCID, skip gracefully
    };

    // Get /Pg (page reference) and resolve to page number.
    let page = dict
        .get("Pg")
        .and_then(|pg_obj| {
            if let Object::Reference(pg_ref) = pg_obj {
                page_map.get(&pg_ref.id).copied()
            } else {
                None
            }
        })
        .unwrap_or(0); // Default to page 0 if no /Pg

    // Resolve /Stm to determine the MCID's content-stream scope.
    // `/StmOwn` is the "inline image" form (§14.7.5.4.2) — out of
    // scope here since inline images do not carry their own MCID
    // namespace.
    let scope = resolve_mcr_scope(document, dict, page);

    Ok(Some(StructChild::MarkedContentRef {
        mcid: mcid as u32,
        page,
        scope,
    }))
}

/// Determine the `McidScope` for a marked-content reference dict.
///
/// When `/Stm` is absent the MCID belongs to the enclosing page's
/// content stream; the scope is `Page(page)`. When `/Stm` is present
/// the referenced stream's dictionary classifies the scope: Form
/// XObjects produce `Form(ref)`, Tiling Patterns produce
/// `Pattern(ref)`. An unclassifiable `/Stm` (unresolved, missing
/// subtype, or unrecognised type) falls back to `Page(page)` with a
/// debug log so the assembler still has *some* lookup key — at worst
/// the lookup misses and the raw glyphs flow through unchanged, which
/// is strictly safer than a collision.
fn resolve_mcr_scope(
    document: &PdfDocument,
    mcr_dict: &HashMap<String, Object>,
    page: u32,
) -> crate::structure::McidScope {
    use crate::structure::McidScope;

    let stm = match mcr_dict.get("Stm") {
        Some(s) => s,
        None => return McidScope::Page(page),
    };

    let stm_ref = match stm {
        Object::Reference(r) => *r,
        _ => {
            log::debug!("MCR /Stm is not an indirect reference; defaulting to page scope");
            return McidScope::Page(page);
        },
    };

    let stm_obj = match document.load_object(stm_ref) {
        Ok(o) => o,
        Err(e) => {
            log::debug!(
                "MCR /Stm {} {} could not be resolved ({}); defaulting to page scope",
                stm_ref.id,
                stm_ref.gen,
                e
            );
            return McidScope::Page(page);
        },
    };

    // For both streams and dictionaries, locate the dict-half.
    let stream_dict: &HashMap<String, Object> = match &stm_obj {
        Object::Stream { dict, .. } => dict,
        Object::Dictionary(d) => d,
        _ => {
            log::debug!(
                "MCR /Stm {} {} resolves to non-stream/non-dict; defaulting to page scope",
                stm_ref.id,
                stm_ref.gen
            );
            return McidScope::Page(page);
        },
    };

    // Form XObject: /Type /XObject /Subtype /Form (or /Subtype /Form
    // alone — producers sometimes omit /Type because it is optional on
    // an XObject stream per §8.8).
    if let Some(subtype) = stream_dict.get("Subtype").and_then(|o| o.as_name()) {
        if subtype == "Form" {
            return McidScope::Form(stm_ref);
        }
    }

    // Tiling Pattern: /PatternType 1 (per §8.7.3.3). Shading patterns
    // (PatternType 2) do not have a content stream of their own and
    // cannot host MCIDs.
    if let Some(pt) = stream_dict.get("PatternType").and_then(|o| o.as_integer()) {
        if pt == 1 {
            return McidScope::Pattern(stm_ref);
        }
    }

    log::debug!(
        "MCR /Stm {} {} has unknown subtype/pattern type; defaulting to page scope",
        stm_ref.id,
        stm_ref.gen
    );
    McidScope::Page(page)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_budget_unbounded_never_exceeds() {
        // The default (general-purpose) guard imposes no cap: a huge element
        // count must not trip it, so `parse_structure_tree` returns the whole tree.
        let guard = ParseBudget::unbounded();
        assert!(!guard.exceeded(0));
        // ── pdf_manipulator patch: MAX_STRUCT_ELEMENTS is removed; 10_000 was its value ──
        assert!(!guard.exceeded(10_000 + 1));
        // ── end pdf_manipulator patch ──
        assert!(!guard.exceeded(usize::MAX));
        // `None` budget is the unbounded guard.
        assert!(!ParseBudget::from_option(None).exceeded(usize::MAX));
    }

    // ── pdf_manipulator patch: the element cap is gone, and its test with it; host/engine_proofs/mem_structure.rs parses past the old cap ──
    // ── end pdf_manipulator patch ──

    #[test]
    fn test_struct_type_mapping() {
        let role_map = {
            let mut map = HashMap::new();
            map.insert("Heading1".to_string(), "H1".to_string());
            map
        };

        let mapped = role_map
            .get("Heading1")
            .map(|s| s.as_str())
            .unwrap_or("Heading1");
        assert_eq!(mapped, "H1");
    }

    #[test]
    fn test_decode_pdf_text_string_utf8() {
        let text = b"Hello World";
        assert_eq!(decode_pdf_text_string(text), "Hello World");
    }

    #[test]
    fn test_decode_pdf_text_string_utf16be() {
        // UTF-16BE BOM + "AB"
        let bytes = vec![0xFE, 0xFF, 0x00, 0x41, 0x00, 0x42];
        assert_eq!(decode_pdf_text_string(&bytes), "AB");
    }

    #[test]
    fn test_decode_pdf_text_string_utf16le() {
        // UTF-16LE BOM + "AB"
        let bytes = vec![0xFF, 0xFE, 0x41, 0x00, 0x42, 0x00];
        assert_eq!(decode_pdf_text_string(&bytes), "AB");
    }

    #[test]
    fn test_decode_pdf_text_string_pdfdoc_encoding() {
        // ASCII subset works as PDFDocEncoding
        let bytes = vec![0x48, 0x65, 0x6C, 0x6C, 0x6F]; // "Hello"
        let result = decode_pdf_text_string(&bytes);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_resolve_object_direct() {
        // Direct object should be returned as-is
        let obj = Object::Integer(42);
        let doc = {
            let pdf = build_test_pdf();
            PdfDocument::from_bytes(pdf).unwrap()
        };
        let result = resolve_object(&doc, &obj).unwrap();
        assert_eq!(result, Object::Integer(42));
    }

    fn minimal_test_doc() -> PdfDocument {
        let pdf = build_test_pdf();
        PdfDocument::from_bytes(pdf).unwrap()
    }

    #[test]
    fn test_parse_marked_content_ref_not_dict() {
        let doc = minimal_test_doc();
        let obj = Object::Integer(5);
        let page_map = HashMap::new();
        let result = parse_marked_content_ref(&doc, &obj, &page_map).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_marked_content_ref_wrong_type() {
        let doc = minimal_test_doc();
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("NotMCR".to_string()));
        dict.insert("MCID".to_string(), Object::Integer(5));
        let obj = Object::Dictionary(dict);
        let page_map = HashMap::new();
        let result = parse_marked_content_ref(&doc, &obj, &page_map).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_marked_content_ref_missing_mcid() {
        let doc = minimal_test_doc();
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("MCR".to_string()));
        let obj = Object::Dictionary(dict);
        let page_map = HashMap::new();
        let result = parse_marked_content_ref(&doc, &obj, &page_map).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_marked_content_ref_valid() {
        let doc = minimal_test_doc();
        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("MCR".to_string()));
        dict.insert("MCID".to_string(), Object::Integer(7));
        let obj = Object::Dictionary(dict);
        let page_map = HashMap::new();
        let result = parse_marked_content_ref(&doc, &obj, &page_map).unwrap();
        assert!(result.is_some());
        if let Some(StructChild::MarkedContentRef { mcid, page, scope }) = result {
            assert_eq!(mcid, 7);
            assert_eq!(page, 0); // default
            assert_eq!(scope, crate::structure::McidScope::Page(0));
        }
    }

    #[test]
    fn test_parse_marked_content_ref_with_page() {
        let doc = minimal_test_doc();
        let mut page_map = HashMap::new();
        page_map.insert(10, 2u32); // object 10 -> page 2

        let mut dict = HashMap::new();
        dict.insert("Type".to_string(), Object::Name("MCR".to_string()));
        dict.insert("MCID".to_string(), Object::Integer(3));
        dict.insert(
            "Pg".to_string(),
            Object::Reference(crate::object::ObjectRef { id: 10, gen: 0 }),
        );
        let obj = Object::Dictionary(dict);
        let result = parse_marked_content_ref(&doc, &obj, &page_map).unwrap();
        if let Some(StructChild::MarkedContentRef { mcid, page, scope }) = result {
            assert_eq!(mcid, 3);
            assert_eq!(page, 2);
            assert_eq!(scope, crate::structure::McidScope::Page(2));
        } else {
            panic!("Expected MarkedContentRef");
        }
    }

    #[test]
    fn test_parse_structure_tree_untagged_pdf() {
        let pdf = build_test_pdf();
        let doc = PdfDocument::from_bytes(pdf).unwrap();
        let result = parse_structure_tree(&doc).unwrap();
        assert!(result.is_none()); // No StructTreeRoot in minimal PDF
    }

    /// Build a minimal PDF for testing
    fn build_test_pdf() -> Vec<u8> {
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let off1 = pdf.len();
        pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
        let off2 = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
        let off3 = pdf.len();
        pdf.extend_from_slice(
            b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
        );

        let xref_offset = pdf.len();
        pdf.extend_from_slice(b"xref\n0 4\n");
        pdf.extend_from_slice(b"0000000000 65535 f \r\n");
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off1).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off2).as_bytes());
        pdf.extend_from_slice(format!("{:010} 00000 n \r\n", off3).as_bytes());
        pdf.extend_from_slice(
            format!("trailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", xref_offset)
                .as_bytes(),
        );
        pdf
    }
}
