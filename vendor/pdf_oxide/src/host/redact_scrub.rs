//! What a destructive redaction scrubs beyond the page's content.
//!
//! A removed glyph leaves two traces outside the content stream: the width of
//! its code in the font's `/Widths` and the text of its code in the font's
//! `/ToUnicode` CMap (and, in a Type 3 font, its glyph procedure), and the
//! `/ActualText`, `/Alt` and `/E` strings of the structure elements that own
//! the marked content that lost a mark. This module decides both for a save
//! and hands the writer the objects to write.
//!
//! It is a child module of the editor (see `document_editor.rs`), so it reads
//! the editor's private state: the redacted pages and the forms the pass opens.

use super::{AnnotationAppearance, DocForms, DocumentEditor, FlatPage, FlatSet, ScopeIds};
use crate::content::operators::Operator;
use crate::document::PdfDocument;
use crate::error::{Error, Result};
use crate::fonts::cmap::parse_tounicode_cmap;
use crate::host::content::{InlineImages, Operators};
use crate::host::page_prune::PagePrune;
use crate::object::{Object, ObjectRef};
use crate::redaction::font_scrub::{scrub_font, CodeSet, FontCodes, FontScrubInput};
use crate::redaction::text_engine::{FontMetrics, TextPass};
use crate::redaction::transform::Scope;
use crate::redaction::RegionSet;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;

/// How deep the walk that counts the codes other content shows follows forms.
const MAX_FORM_DEPTH: usize = 24;

/// The key of a content stream the pass decided: the redacted page, and the
/// forms between it and the stream (empty for the page's own content).
type ScopeKey = (usize, Vec<u32>);

/// A font as a resource dictionary names it.
#[derive(Clone)]
enum FontEntry {
    Reference(ObjectRef),
    Direct(HashMap<String, Object>),
}

/// What is known of the codes of one font object.
#[derive(Default)]
struct FontState {
    /// Codes the redaction removed, on every redacted page.
    removed: CodeSet,
    /// Codes the redacted pages still show.
    scoped: CodeSet,
    /// Codes the rest of the document shows.
    counted: CodeSet,
    /// Some user of the font shows codes that cannot be counted.
    uncountable: bool,
}

/// A font rewritten but for the ids of the objects it names.
struct Rewritten {
    dict: HashMap<String, Object>,
    /// The new `/ToUnicode` CMap, when the font had one.
    to_unicode: Option<Vec<u8>>,
    /// The entries of `/CharProcs` that become an empty procedure.
    procs: Vec<String>,
    /// The `/CharProcs` dictionary the entries above are set in.
    char_procs: Option<HashMap<String, Object>>,
    /// Source objects the font no longer names.
    orphans: Vec<u32>,
    /// The CID font a Type 0 font names, rewritten, when its object is
    /// indirect: the objects the writer places it with.
    descendant: Option<Descendant>,
}

/// A descendant CID font with the widths of the removed codes gone.
struct Descendant {
    /// The source object, which an in-place scrub overrides.
    reference: ObjectRef,
    dict: HashMap<String, Object>,
}

/// What a save's scrub decided, before any object has an id.
struct Computed {
    /// The font objects to rewrite, whether in place or as a copy.
    rewrites: Vec<(u32, bool, Rewritten)>,
    /// The direct font dictionaries to rewrite.
    direct_rewrites: Vec<(ScopeKey, String, Rewritten)>,
    /// Every font object the redacted scopes name, by scope and resource name.
    names: Vec<(ScopeKey, String, FontEntry)>,
    /// The structure elements that lose their strings.
    structure: HashMap<u32, Object>,
}

/// A new stream: a CMap or an empty glyph procedure.
pub(super) struct NewStream {
    pub id: u32,
    pub length_id: u32,
    pub dict: HashMap<String, Object>,
    pub bytes: Vec<u8>,
}

/// The objects a save writes for the scrub, and the entries the redacted
/// pages' resources take.
#[derive(Default)]
pub(super) struct Scrub {
    /// Objects written whole under a new id: the font copies.
    pub objects: Vec<(u32, Object)>,
    /// Streams written under a new id.
    pub streams: Vec<NewStream>,
    /// The Font entries of a scope's resources, by scope and name.
    fonts: HashMap<ScopeKey, HashMap<String, Object>>,
    /// How many font objects were scrubbed.
    pub fonts_scrubbed: usize,
}

impl Scrub {
    /// Gives the ids of `scope` (page `src`'s content, or a form copy below
    /// it) the Font entries its resources take.
    pub fn attach(&self, src: usize, scope: &Scope, ids: &mut ScopeIds) {
        self.attach_below(src, &mut Vec::new(), scope, ids);
    }

    fn attach_below(&self, src: usize, path: &mut Vec<u32>, scope: &Scope, ids: &mut ScopeIds) {
        if let Some(fonts) = self.fonts.get(&(src, path.clone())) {
            ids.fonts = fonts.clone();
        }
        for (form, form_ids) in scope.forms.iter().zip(ids.forms.iter_mut()) {
            path.push(form.source.id);
            self.attach_below(src, path, &form.scope, &mut form_ids.inner);
            path.pop();
        }
    }
}

/// Reads the objects of the source and those a flattened appearance carries
/// beside it (its fallback font's graph), which the source does not hold.
struct Reader<'a> {
    source: &'a PdfDocument,
    flats: Vec<&'a FlatPage>,
}

impl Reader<'_> {
    fn load_object(&self, reference: ObjectRef) -> Result<Object> {
        match self.flats.iter().find_map(|flat| flat.extra(reference.id)) {
            Some(extra) => Ok(extra.clone()),
            None => self.source.load_object(reference),
        }
    }

    fn resolve_object(&self, object: &Object) -> Result<Object> {
        match object {
            Object::Reference(reference) => self.load_object(*reference),
            other => self.source.resolve_object(other),
        }
    }
}

/// The fonts of `resources` by resource name.
fn font_entries(source: &PdfDocument, resources: &Object) -> Result<HashMap<String, FontEntry>> {
    let mut out = HashMap::new();
    let Some(fonts) = resources.as_dict().and_then(|d| d.get("Font")) else {
        return Ok(out);
    };
    let fonts = source.resolve_object(fonts)?;
    let Some(fonts) = fonts.as_dict() else {
        return Ok(out);
    };
    for (name, value) in fonts {
        match value {
            Object::Reference(r) => {
                out.insert(name.clone(), FontEntry::Reference(*r));
            },
            Object::Dictionary(d) => {
                out.insert(name.clone(), FontEntry::Direct(d.clone()));
            },
            _ => {},
        }
    }
    Ok(out)
}

/// The XObjects of `resources` by resource name.
fn xobject_refs(source: &PdfDocument, resources: &Object) -> Result<HashMap<String, ObjectRef>> {
    let mut out = HashMap::new();
    let Some(xobjects) = resources.as_dict().and_then(|d| d.get("XObject")) else {
        return Ok(out);
    };
    let xobjects = source.resolve_object(xobjects)?;
    if let Some(xobjects) = xobjects.as_dict() {
        for (name, value) in xobjects {
            if let Some(r) = value.as_reference() {
                out.insert(name.clone(), r);
            }
        }
    }
    Ok(out)
}

/// Counts the codes the document shows in the fonts the scrub considers.
struct Counter<'a> {
    source: &'a PdfDocument,
    states: &'a mut BTreeMap<u32, FontState>,
    visited: HashSet<u32>,
}

impl Counter<'_> {
    /// Marks the planned fonts `resources` names as showing codes that
    /// cannot be counted.
    fn give_up(&mut self, resources: &Object) -> Result<()> {
        for entry in font_entries(self.source, resources)?.values() {
            if let FontEntry::Reference(r) = entry {
                if let Some(state) = self.states.get_mut(&r.id) {
                    state.uncountable = true;
                }
            }
        }
        Ok(())
    }

    /// Counts the codes `ops` shows under `resources`, and those of the forms
    /// it draws.
    fn count_stream<R: Read>(
        &mut self,
        ops: Operators<R>,
        resources: &Object,
        depth: usize,
    ) -> Result<()> {
        let entries = font_entries(self.source, resources)?;
        let planned = entries.values().any(|entry| {
            matches!(entry, FontEntry::Reference(r) if self.states.contains_key(&r.id))
        });
        let xobjects = xobject_refs(self.source, resources)?;
        let metrics = DocumentEditor::font_metrics_of(self.source, resources, None)?;
        let none = RegionSet::new(0);
        let mut pass = TextPass::new(&none, 0.0, &metrics);
        let mut scratch = Vec::new();
        let mut ops = ops.with_inline_images(InlineImages::Skip);
        let mut intact = true;
        let mut stride = crate::cancel::Stride::job();
        for item in ops.by_ref() {
            stride.tick()?;
            let Ok(op) = item else {
                intact = false;
                break;
            };
            if let Operator::Do { name } = &op {
                if let Some(form) = xobjects.get(name.as_str()) {
                    self.count_form(*form, resources, depth + 1)?;
                }
            }
            if planned {
                scratch.clear();
                pass.apply(&op, &mut scratch);
            }
        }
        if !intact || !ops.damage().is_clean() {
            return self.give_up(resources);
        }
        for (name, codes) in &pass.result().fonts {
            crate::cancel::poll()?;
            let Some(FontEntry::Reference(font)) = entries.get(name) else {
                continue;
            };
            let Some(state) = self.states.get_mut(&font.id) else {
                continue;
            };
            if metrics.can_prune(name) {
                state.counted.union_with(&codes.surviving);
            } else {
                state.uncountable = true;
            }
        }
        Ok(())
    }

    /// Counts the form `form` once. A form with no `/Resources` of its own
    /// reads `fallback`.
    fn count_form(&mut self, form: ObjectRef, fallback: &Object, depth: usize) -> Result<()> {
        if !self.visited.insert(form.id) {
            return Ok(());
        }
        let Ok(object) = self.source.load_object(form) else {
            return self.give_up(fallback);
        };
        let Object::Stream { dict, .. } = &object else {
            return Ok(());
        };
        if dict.get("Subtype").and_then(Object::as_name) != Some("Form") {
            return Ok(());
        }
        let resources = match dict.get("Resources") {
            Some(resources) => self.source.resolve_object(resources)?,
            None => fallback.clone(),
        };
        if depth > MAX_FORM_DEPTH {
            return self.give_up(&resources);
        }
        match self.source.form_operators(form) {
            Ok(ops) => self.count_stream(ops, &resources, depth),
            Err(_) => self.give_up(&resources),
        }
    }

    /// Counts the appearance streams of the annotations of the page whose
    /// dictionary is `page` that the save keeps.
    fn count_annotations(&mut self, page: &Object, dropped: &HashSet<u32>) -> Result<()> {
        let Some(annots) = page.as_dict().and_then(|d| d.get("Annots")) else {
            return Ok(());
        };
        let annots = self.source.resolve_object(annots)?;
        for annot in annots.as_array().into_iter().flatten() {
            crate::cancel::poll()?;
            let Some(r) = annot.as_reference() else { continue };
            if dropped.contains(&r.id) {
                continue;
            }
            let Ok(annot) = self.source.load_object(r) else { continue };
            let Some(appearance) = annot.as_dict().and_then(|d| d.get("AP")) else { continue };
            let appearance = self.source.resolve_object(appearance)?;
            for key in ["N", "D", "R"] {
                let Some(entry) = appearance.as_dict().and_then(|d| d.get(key)) else { continue };
                self.count_appearance(entry)?;
            }
        }
        Ok(())
    }

    /// An appearance entry: a stream, or a dictionary of states.
    fn count_appearance(&mut self, entry: &Object) -> Result<()> {
        let Some(r) = entry.as_reference() else {
            if let Some(states) = entry.as_dict() {
                for state in states.values() {
                    if let Some(r) = state.as_reference() {
                        self.count_form(r, &Object::Null, 0)?;
                    }
                }
            }
            return Ok(());
        };
        match self.source.load_object(r) {
            Ok(Object::Stream { .. }) => self.count_form(r, &Object::Null, 0),
            Ok(Object::Dictionary(states)) => {
                for state in states.values() {
                    if let Some(r) = state.as_reference() {
                        self.count_form(r, &Object::Null, 0)?;
                    }
                }
                Ok(())
            },
            _ => Ok(()),
        }
    }

    /// Counts the forms the pass left in place: those a surviving `Do` of the
    /// scope names, and those of its form copies.
    fn count_kept(&mut self, scope: &Scope, resources: &Object, forms: &DocForms) -> Result<()> {
        let xobjects = xobject_refs(self.source, resources)?;
        for name in &scope.kept_xobject_names {
            if let Some(form) = xobjects.get(name.as_str()) {
                self.count_form(*form, resources, 0)?;
            }
        }
        for form in &scope.forms {
            let dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
            let inner = forms.resources_of(&dict)?;
            self.count_kept(&form.scope, &inner, forms)?;
        }
        Ok(())
    }
}

/// Reads the font `/Widths`, `/ToUnicode` and glyph procedures of `font` and
/// rewrites those of the codes in `removed` and not in `surviving`. `None`
/// when no code of the font needs it. A `/ToUnicode` that cannot be decoded
/// is dropped: the font keeps no mapping rather than a mapping that cannot be
/// scrubbed.
fn rewrite_font(
    source: &Reader,
    font: &HashMap<String, Object>,
    removed: &CodeSet,
    surviving: &CodeSet,
) -> Result<Option<Rewritten>> {
    let number = |o: &Object| o.as_real().or_else(|| o.as_integer().map(|i| i as f64));
    let mut dict = font.clone();
    let mut orphans = Vec::new();

    let first_char = match dict.get("FirstChar") {
        Some(value) => source
            .resolve_object(value)?
            .as_integer()
            .map_or(0, |n| n.clamp(0, i64::from(u16::MAX)) as u32),
        None => 0,
    };
    let mut widths: Vec<f32> = Vec::new();
    let mut widths_ref = None;
    if let Some(value) = dict.get("Widths") {
        widths_ref = value.as_reference();
        let value = source.resolve_object(value)?;
        for item in value.as_array().into_iter().flatten() {
            let item = source.resolve_object(item)?;
            widths.push(number(&item).unwrap_or(0.0) as f32);
        }
    }
    let missing_width = match dict.get("FontDescriptor") {
        Some(descriptor) => {
            let descriptor = source.resolve_object(descriptor)?;
            match descriptor.as_dict().and_then(|d| d.get("MissingWidth")) {
                Some(value) => number(&source.resolve_object(value)?).unwrap_or(0.0) as f32,
                None => 0.0,
            }
        },
        None => 0.0,
    };

    let mut to_unicode = BTreeMap::new();
    let mut cmap_shape = None;
    let mut cmap_ref = None;
    if let Some(r) = dict.get("ToUnicode").and_then(Object::as_reference) {
        cmap_ref = Some(r);
        let parsed = source
            .load_object(r)
            .and_then(|stream| {
                // A CMap is a table, read whole by nature.
                let mut data = Vec::new();
                crate::host::content::parts::decoded_reader(&stream)?.read_to_end(&mut data)?;
                Ok(data)
            })
            .and_then(|data| parse_tounicode_cmap(&data));
        match parsed {
            Ok(cmap) => {
                for (code, text) in cmap.mappings() {
                    to_unicode.insert(code, text);
                }
                cmap_shape = Some((cmap.code_width, cmap.wmode, cmap.notdef_mappings()));
            },
            Err(_) => {
                dict.remove("ToUnicode");
                orphans.push(r.id);
                cmap_ref = None;
            },
        }
    }
    let to_unicode_len = to_unicode.len();

    let subtype = dict.get("Subtype").and_then(Object::as_name).map(str::to_string);
    let mut procs = Vec::new();
    let mut char_procs = None;
    if subtype.as_deref() == Some("Type3") {
        let mut names: HashMap<u32, String> = HashMap::new();
        let mut base = None;
        if let Some(encoding) = dict.get("Encoding") {
            let encoding = source.resolve_object(encoding)?;
            if let Some(name) = encoding.as_name() {
                base = Some(name.to_string());
            }
            if let Some(name) = encoding.as_dict().and_then(|d| d.get("BaseEncoding")) {
                base = source.resolve_object(name)?.as_name().map(str::to_string);
            }
            if let Some(differences) = encoding.as_dict().and_then(|d| d.get("Differences")) {
                let differences = source.resolve_object(differences)?;
                let mut code = 0u32;
                for item in differences.as_array().into_iter().flatten() {
                    match item {
                        Object::Integer(n) => code = (*n).clamp(0, i64::from(u16::MAX)) as u32,
                        Object::Name(name) => {
                            names.insert(code, name.clone());
                            code += 1;
                        },
                        _ => {},
                    }
                }
            }
        }
        // A code no /Differences entry names reaches its procedure through the
        // base encoding, as a renderer reads it. A code that reaches no name
        // has a procedure that cannot be found, so it cannot be shown to be gone.
        for code in removed.iter().chain(surviving.iter()) {
            if names.contains_key(&code) {
                continue;
            }
            match type3_base_name(base.as_deref(), code) {
                Some(name) => {
                    names.insert(code, name.to_string());
                },
                None if removed.contains(code) && !surviving.contains(code) => {
                    return Err(Error::Unsupported(format!(
                        "the Type 3 font shows code {code}, which its /Encoding does not name; \
                         refusing to redact a glyph whose procedure cannot be found"
                    )));
                },
                None => {},
            }
        }
        let kept: HashSet<&String> = surviving.iter().filter_map(|c| names.get(&c)).collect();
        let doomed: BTreeSet<String> = removed
            .iter()
            .filter(|c| !surviving.contains(*c))
            .filter_map(|c| names.get(&c))
            .filter(|name| !kept.contains(name))
            .cloned()
            .collect();
        if !doomed.is_empty() {
            if let Some(value) = dict.get("CharProcs") {
                if let Some(r) = value.as_reference() {
                    orphans.push(r.id);
                }
                let resolved = source.resolve_object(value)?;
                let mut table = resolved.as_dict().cloned().unwrap_or_default();
                for name in &doomed {
                    if let Some(Object::Reference(r)) = table.get(name) {
                        orphans.push(r.id);
                    }
                    if table.contains_key(name) {
                        procs.push(name.clone());
                    }
                }
                char_procs = Some(std::mem::take(&mut table));
            }
        }
    }

    let mut descendant = None;
    let mut cid_widths_scrubbed = false;
    if let Some(value) = dict.get("DescendantFonts").cloned() {
        if let Some(found) = rewrite_descendant(source, &value, removed, surviving)? {
            cid_widths_scrubbed = true;
            orphans.extend(found.orphans);
            match found.reference {
                Some(reference) => descendant = Some(Descendant { reference, dict: found.dict }),
                None => {
                    if let Some(list) = value.as_reference() {
                        orphans.push(list.id);
                    }
                    dict.insert(
                        "DescendantFonts".to_string(),
                        Object::Array(vec![Object::Dictionary(found.dict)]),
                    );
                },
            }
        }
    }

    let result = scrub_font(&FontScrubInput {
        first_char,
        widths: widths.clone(),
        missing_width,
        to_unicode,
        removed_codes: removed.to_set(),
        surviving_codes: surviving.to_set(),
    });
    if result.codes_scrubbed == 0 && procs.is_empty() && !cid_widths_scrubbed {
        return Ok(None);
    }
    if result.widths != widths {
        if let Some(r) = widths_ref {
            orphans.push(r.id);
        }
        dict.insert(
            "Widths".to_string(),
            Object::Array(result.widths.iter().map(|w| Object::Real(f64::from(*w))).collect()),
        );
    }
    let mut cmap = None;
    if let (Some(r), Some((code_width, wmode, notdef))) = (cmap_ref, cmap_shape) {
        if result.to_unicode.len() != to_unicode_len {
            orphans.push(r.id);
            cmap = Some(cmap_bytes(&result.to_unicode, code_width, wmode, &notdef));
        }
    }
    Ok(Some(Rewritten { dict, to_unicode: cmap, procs, char_procs, orphans, descendant }))
}

/// The glyph name the base encoding `base` (a Type 3 font's `/Encoding` name
/// or `/BaseEncoding`; `StandardEncoding` when it names none) gives `code`.
fn type3_base_name(base: Option<&str>, code: u32) -> Option<&'static str> {
    use crate::fonts::cff_encoding::{mac_roman_byte_to_name, standard_encoding_byte_to_name};
    let byte = u8::try_from(code).ok()?;
    match base {
        Some("WinAnsiEncoding") => crate::fonts::FontInfo::gid_to_standard_glyph_name(u16::from(byte)),
        Some("MacRomanEncoding") => mac_roman_byte_to_name(byte),
        _ => standard_encoding_byte_to_name(byte),
    }
}

/// A descendant CID font whose `/W` lost the widths of the removed codes.
struct RewrittenDescendant {
    /// The font's object when `DescendantFonts` names it indirectly.
    reference: Option<ObjectRef>,
    dict: HashMap<String, Object>,
    /// Source objects the rewritten font no longer names.
    orphans: Vec<u32>,
}

/// The descendant font `fonts` (a Type 0 font's `/DescendantFonts`) names,
/// with the `/W` entries of the codes in `removed` and not in `surviving`
/// left out, which reads them as `/DW`. `None` when no width of such a code
/// is listed.
///
/// # Errors
/// [`Error::Unsupported`] when the `/W` array cannot be read: a width that
/// cannot be found cannot be shown to be gone.
fn rewrite_descendant(
    source: &Reader,
    fonts: &Object,
    removed: &CodeSet,
    surviving: &CodeSet,
) -> Result<Option<RewrittenDescendant>> {
    let list = source.resolve_object(fonts)?;
    let Some(first) = list.as_array().and_then(|items| items.first()) else {
        return Ok(None);
    };
    let reference = first.as_reference();
    let font = source.resolve_object(first)?;
    let Some(font) = font.as_dict() else {
        return Ok(None);
    };
    let Some(widths) = font.get("W") else {
        return Ok(None);
    };
    let unreadable = || {
        Error::Unsupported(
            "a CID font's /W array cannot be read; refusing to redact a page whose font keeps \
             widths that cannot be found"
                .to_string(),
        )
    };
    let widths_ref = widths.as_reference();
    let widths = source.resolve_object(widths)?;
    let items = widths.as_array().ok_or_else(unreadable)?;
    let doomed: BTreeSet<u32> = removed.iter().filter(|code| !surviving.contains(*code)).collect();
    if doomed.is_empty() {
        return Ok(None);
    }
    let cid = |item: &Object| -> Result<u32> {
        let value = source.resolve_object(item)?.as_integer().ok_or_else(unreadable)?;
        Ok(value.clamp(0, i64::from(u16::MAX)) as u32)
    };
    let mut out: Vec<Object> = Vec::new();
    let mut changed = false;
    let mut at = 0;
    while at < items.len() {
        let first = cid(&items[at])?;
        let entry = items.get(at + 1).ok_or_else(unreadable)?;
        let entry = source.resolve_object(entry)?;
        if let Some(run) = entry.as_array() {
            at += 2;
            let last = first + run.len() as u32;
            if doomed.range(first..last).next().is_none() {
                out.push(items[at - 2].clone());
                out.push(entry.clone());
                continue;
            }
            changed = true;
            let mut kept: Vec<Object> = Vec::new();
            let mut start = first;
            for (offset, width) in run.iter().enumerate() {
                if doomed.contains(&(first + offset as u32)) {
                    if !kept.is_empty() {
                        out.push(Object::Integer(i64::from(start)));
                        out.push(Object::Array(std::mem::take(&mut kept)));
                    }
                    start = first + offset as u32 + 1;
                } else {
                    kept.push(width.clone());
                }
            }
            if !kept.is_empty() {
                out.push(Object::Integer(i64::from(start)));
                out.push(Object::Array(kept));
            }
        } else {
            let last = cid(&entry)?;
            let width = items.get(at + 2).ok_or_else(unreadable)?;
            at += 3;
            if last < first || doomed.range(first..=last).next().is_none() {
                out.extend([Object::Integer(i64::from(first)), Object::Integer(i64::from(last)), width.clone()]);
                continue;
            }
            changed = true;
            let mut start = first;
            for hit in doomed.range(first..=last) {
                if *hit > start {
                    out.extend([
                        Object::Integer(i64::from(start)),
                        Object::Integer(i64::from(*hit - 1)),
                        width.clone(),
                    ]);
                }
                start = *hit + 1;
            }
            if start <= last {
                out.extend([Object::Integer(i64::from(start)), Object::Integer(i64::from(last)), width.clone()]);
            }
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut dict = font.clone();
    dict.insert("W".to_string(), Object::Array(out));
    Ok(Some(RewrittenDescendant {
        reference,
        dict,
        orphans: widths_ref.map(|r| r.id).into_iter().collect(),
    }))
}

/// A `/ToUnicode` CMap holding `entries`.
fn cmap_bytes(
    entries: &BTreeMap<u32, String>,
    code_width: u8,
    wmode: u8,
    notdef: &[(u32, u32, String)],
) -> Vec<u8> {
    let digits = if code_width >= 2 { 4 } else { 2 };
    let utf16 = |text: &str| -> String { text.encode_utf16().map(|u| format!("{u:04X}")).collect() };
    let mut out = String::new();
    out.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n");
    out.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    out.push_str("/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n");
    if wmode == 1 {
        out.push_str("/WMode 1 def\n");
    }
    let (low, high) = if digits == 4 { ("0000", "FFFF") } else { ("00", "FF") };
    out.push_str(&format!("1 begincodespacerange\n<{low}> <{high}>\nendcodespacerange\n"));
    let all: Vec<(&u32, &String)> = entries.iter().collect();
    for chunk in all.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (code, text) in chunk {
            out.push_str(&format!("<{code:0digits$X}> <{}>\n", utf16(text)));
        }
        out.push_str("endbfchar\n");
    }
    for chunk in notdef.chunks(100) {
        out.push_str(&format!("{} beginnotdefrange\n", chunk.len()));
        for (first, last, text) in chunk {
            out.push_str(&format!("<{first:0digits$X}> <{last:0digits$X}> <{}>\n", utf16(text)));
        }
        out.push_str("endnotdefrange\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out.into_bytes()
}

/// The value the number tree `node` holds for `key`: the tree is looked up
/// through its `/Limits`, never walked.
fn number_tree_get(
    source: &PdfDocument,
    node: &Object,
    key: i64,
    depth: usize,
) -> Result<Option<Object>> {
    if depth > 32 {
        return Ok(None);
    }
    let node = source.resolve_object(node)?;
    let Some(dict) = node.as_dict() else {
        return Ok(None);
    };
    if let Some(nums) = dict.get("Nums") {
        let nums = source.resolve_object(nums)?;
        for pair in nums.as_array().into_iter().flatten().collect::<Vec<_>>().chunks(2) {
            if let [at, value] = pair {
                if source.resolve_object(at)?.as_integer() == Some(key) {
                    return Ok(Some(source.resolve_object(value)?));
                }
            }
        }
        return Ok(None);
    }
    if let Some(kids) = dict.get("Kids") {
        let kids = source.resolve_object(kids)?;
        for kid in kids.as_array().into_iter().flatten() {
            let resolved = source.resolve_object(kid)?;
            let limits = resolved.as_dict().and_then(|d| d.get("Limits"));
            if let Some(limits) = limits {
                let limits = source.resolve_object(limits)?;
                if let Some([low, high]) = limits.as_array().map(Vec::as_slice) {
                    let low = source.resolve_object(low)?.as_integer();
                    let high = source.resolve_object(high)?.as_integer();
                    if low.is_some_and(|low| key < low) || high.is_some_and(|high| key > high) {
                        continue;
                    }
                }
            }
            if let Some(found) = number_tree_get(source, &resolved, key, depth + 1)? {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

impl DocumentEditor {
    /// The element dictionaries, copied without `/ActualText`, `/Alt` and
    /// `/E`, of the structure elements that own the marked content of
    /// `mcids` under the `/StructParents` key `dict` names.
    fn strip_structure_strings(
        &self,
        tree: &Object,
        dict: &HashMap<String, Object>,
        mcids: &BTreeSet<u32>,
        out: &mut HashMap<u32, Object>,
    ) -> Result<()> {
        if mcids.is_empty() {
            return Ok(());
        }
        let Some(key) = dict.get("StructParents") else {
            return Ok(());
        };
        let Some(key) = self.source.resolve_object(key)?.as_integer() else {
            return Ok(());
        };
        let Some(owners) = number_tree_get(&self.source, tree, key, 0)? else {
            return Ok(());
        };
        let Some(owners) = owners.as_array() else {
            return Ok(());
        };
        for mcid in mcids {
            let Some(Object::Reference(element)) = owners.get(*mcid as usize) else {
                continue;
            };
            if out.contains_key(&element.id) {
                continue;
            }
            let Ok(object) = self.source.load_object(*element) else { continue };
            let Some(fields) = object.as_dict() else { continue };
            let mut fields = fields.clone();
            let before = fields.len();
            for key in ["ActualText", "Alt", "E"] {
                fields.remove(key);
            }
            if fields.len() != before {
                out.insert(element.id, Object::Dictionary(fields));
            }
        }
        Ok(())
    }

    /// The structure elements that lose their strings, by object id, over
    /// every redacted page and the form copies below it.
    fn structure_overrides(&self, forms: &HashMap<usize, DocForms>) -> Result<HashMap<u32, Object>> {
        let mut out = HashMap::new();
        let catalog = self.source.catalog()?;
        let Some(root) = catalog.as_dict().and_then(|c| c.get("StructTreeRoot")) else {
            return Ok(out);
        };
        let root = self.source.resolve_object(root)?;
        let Some(tree) = root.as_dict().and_then(|r| r.get("ParentTree")) else {
            return Ok(out);
        };
        let tree = tree.clone();
        let mut pages: Vec<&usize> = self.redacted_pages.keys().collect();
        pages.sort();
        for src in pages {
            crate::cancel::poll()?;
            let page = &self.redacted_pages[src];
            let page_ref = self.source.get_page_ref(*src)?;
            let page_dict = self.source.load_object(page_ref)?.as_dict().cloned().unwrap_or_default();
            self.strip_structure_strings(&tree, &page_dict, &page.scope.mcids, &mut out)?;
            self.strip_form_strings(&tree, &forms[src], &page.scope, &mut out)?;
        }
        Ok(out)
    }

    fn strip_form_strings(
        &self,
        tree: &Object,
        forms: &DocForms,
        scope: &Scope,
        out: &mut HashMap<u32, Object>,
    ) -> Result<()> {
        for form in &scope.forms {
            let dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
            self.strip_structure_strings(tree, &dict, &form.scope.mcids, out)?;
            self.strip_form_strings(tree, forms, &form.scope, out)?;
        }
        Ok(())
    }

    /// Adds to `states` the codes of every redacted scope and the resource
    /// each font entry is named by, and the direct font dictionaries.
    #[allow(clippy::too_many_arguments)]
    fn collect_font_use(
        &self,
        forms: &DocForms,
        key: &mut ScopeKey,
        scope: &Scope,
        resources: &Object,
        states: &mut BTreeMap<u32, FontState>,
        directs: &mut Vec<(ScopeKey, String, HashMap<String, Object>, FontCodes)>,
        names: &mut Vec<(ScopeKey, String, FontEntry)>,
    ) -> Result<()> {
        let entries = font_entries(&self.source, resources)?;
        for (name, codes) in &scope.fonts {
            crate::cancel::poll()?;
            match entries.get(name) {
                Some(FontEntry::Reference(r)) => {
                    let state = states.entry(r.id).or_default();
                    state.removed.union_with(&codes.removed);
                    state.scoped.union_with(&codes.surviving);
                    names.push((key.clone(), name.clone(), FontEntry::Reference(*r)));
                },
                Some(FontEntry::Direct(dict)) => {
                    directs.push((key.clone(), name.clone(), dict.clone(), codes.clone()));
                },
                None => {},
            }
        }
        for form in &scope.forms {
            let dict = forms.load(form.source)?.as_dict().cloned().unwrap_or_default();
            let inner = forms.resources_of(&dict)?;
            key.1.push(form.source.id);
            self.collect_font_use(forms, key, &form.scope, &inner, states, directs, names)?;
            key.1.pop();
        }
        Ok(())
    }

    /// Gives each redacted page that is also flattened its flattened
    /// appearances and decides the page again with them drawn first, so the
    /// redaction removes what a flattened value shows under a box. The pages
    /// are decided again on every save: the flattening may be queued after
    /// the redaction was applied.
    pub(super) fn prepare_flat_redactions(&mut self) -> Result<()> {
        let mut pages: Vec<usize> = self.redacted_pages.keys().copied().collect();
        pages.sort();
        for src in pages {
            crate::cancel::poll()?;
            let annots = self.flatten_annotations_pages.contains(&src);
            let widgets = self.flatten_forms_pages.contains(&src);
            let had = self.redacted_pages[&src].flat.is_some();
            if !annots && !widgets && !had {
                continue;
            }
            let mut flat = FlatPage { sets: Vec::new(), overlay: Vec::new() };
            if annots {
                let appearances = self.get_annotation_appearances(src)?;
                self.push_flat_set(&mut flat, appearances, "FlatAnnot", false);
            }
            if widgets {
                let appearances = self.get_widget_appearances(src)?;
                self.push_flat_set(&mut flat, appearances, "FlatForm", true);
            }
            let flat = (!flat.sets.is_empty()).then_some(flat);
            let outcome = {
                let page = &self.redacted_pages[&src];
                let resources = Self::with_flat_xobjects(
                    &self.source,
                    Self::inherited_resources(&self.source, src)?,
                    flat.as_ref(),
                )?;
                let images = Self::image_resources_of(&self.source, &resources, flat.as_ref())?;
                let forms = DocForms {
                    source: &self.source,
                    page_resources: resources,
                    flat: flat.as_ref(),
                };
                Self::run_redaction_pass(
                    &page.regions,
                    &page.opts,
                    &page.fonts,
                    &images,
                    &forms,
                    self.source.page_operators(src)?,
                    flat.as_ref().map_or(&[][..], |flat| flat.overlay.as_slice()),
                    None,
                    &mut std::io::sink(),
                )?
            };
            self.redacted_image_originals.extend(outcome.originals.iter().copied());
            if let Some(page) = self.redacted_pages.get_mut(&src) {
                page.scope = outcome.scope;
                page.report = outcome.report;
                page.flat = flat;
            }
        }
        Ok(())
    }

    /// Numbers the appearances of one kind of flattening and appends their
    /// overlay to `flat`.
    fn push_flat_set(
        &mut self,
        flat: &mut FlatPage,
        #[allow(unused_mut)] mut appearances: Vec<AnnotationAppearance>,
        stem: &str,
        widgets: bool,
    ) {
        if appearances.is_empty() {
            return;
        }
        #[cfg(test)]
        if let Some(first) = appearances.first_mut() {
            let extras = crate::host::flat_extras_seam::TEST_FLAT_EXTRAS.with(|extras| extras.take());
            first.extra_objects.extend(extras);
        }
        let xobjects: Vec<(u32, String)> = (0..appearances.len())
            .map(|at| (self.allocate_object_id(), format!("{stem}{at}")))
            .collect();
        let names: Vec<String> = xobjects.iter().map(|(_, name)| name.clone()).collect();
        flat.overlay.extend(self.generate_flatten_overlay(&appearances, &names));
        flat.sets.push(FlatSet { appearances, xobjects, widgets });
    }

    /// The forms of redacted page `src`, as its pass opens them.
    pub(super) fn forms_of(&self, src: usize) -> Result<DocForms<'_>> {
        Ok(DocForms {
            source: &self.source,
            page_resources: Self::inherited_resources(&self.source, src)?,
            flat: self.redacted_pages[&src].flat.as_ref(),
        })
    }

    /// Decides the scrub of the redacted pages for one save, without writing
    /// or numbering anything.
    fn compute_scrub(&self) -> Result<Computed> {
        let mut sources: Vec<usize> = self.redacted_pages.keys().copied().collect();
        sources.sort();
        let mut forms: HashMap<usize, DocForms> = HashMap::new();
        for src in &sources {
            forms.insert(*src, self.forms_of(*src)?);
        }

        let mut states: BTreeMap<u32, FontState> = BTreeMap::new();
        let mut directs = Vec::new();
        let mut names = Vec::new();
        for src in &sources {
            let page = &self.redacted_pages[src];
            let resources = Self::inherited_resources(&self.source, *src)?;
            let mut key = (*src, Vec::new());
            self.collect_font_use(
                &forms[src],
                &mut key,
                &page.scope,
                &resources,
                &mut states,
                &mut directs,
                &mut names,
            )?;
        }
        states.retain(|_, state| {
            state
                .removed
                .iter()
                .any(|code| !state.scoped.contains(code))
        });
        directs.retain(|(_, _, _, codes)| codes.has_redaction_only_code());

        let reader = Reader {
            source: &self.source,
            flats: sources.iter().filter_map(|src| self.redacted_pages[src].flat.as_ref()).collect(),
        };
        let mut rewrites: Vec<(u32, bool, Rewritten)> = Vec::new();
        if !states.is_empty() {
            self.count_users(&sources, &forms, &mut states)?;
            for (id, state) in &states {
                let mut surviving = state.scoped.clone();
                if !state.uncountable {
                    surviving.union_with(&state.counted);
                }
                let font = reader.load_object(ObjectRef::new(*id, 0))?;
                let Some(dict) = font.as_dict() else { continue };
                if let Some(rewritten) = rewrite_font(&reader, dict, &state.removed, &surviving)? {
                    rewrites.push((*id, !state.uncountable, rewritten));
                }
            }
        }
        let mut direct_rewrites: Vec<(ScopeKey, String, Rewritten)> = Vec::new();
        for (key, name, dict, codes) in &directs {
            if let Some(rewritten) = rewrite_font(&reader, dict, &codes.removed, &codes.surviving)? {
                direct_rewrites.push((key.clone(), name.clone(), rewritten));
            }
        }
        let structure = self.structure_overrides(&forms)?;
        Ok(Computed { rewrites, direct_rewrites, names, structure })
    }

    /// Plans the scrub of the redacted pages for one save: the fonts whose
    /// removed codes no surviving text shows, and the structure strings. The
    /// fonts counted in every user are rewritten where they are; the others
    /// are copied for the redacted pages.
    pub(super) fn plan_scrub(&mut self, prune: &mut PagePrune) -> Result<Scrub> {
        let mut scrub = Scrub::default();
        if self.redacted_pages.is_empty() {
            return Ok(scrub);
        }
        let Computed { rewrites, direct_rewrites, names, structure } = self.compute_scrub()?;

        let mut copies: HashMap<u32, u32> = HashMap::new();
        for (id, in_place, mut rewritten) in rewrites {
            if self.is_flat_extra(id) {
                self.rewrite_flat_font(id, rewritten);
                scrub.fonts_scrubbed += 1;
                continue;
            }
            self.place_descendant(&mut rewritten, in_place, prune, &mut scrub);
            self.finish_font(&mut rewritten, &mut scrub);
            let font = Object::Dictionary(rewritten.dict);
            prune.redacted_image_originals.extend(rewritten.orphans);
            if in_place {
                prune.overrides.insert(id, font);
            } else {
                let copy = self.allocate_object_id();
                scrub.objects.push((copy, font));
                copies.insert(id, copy);
            }
            scrub.fonts_scrubbed += 1;
        }
        for (key, name, mut rewritten) in direct_rewrites {
            self.place_descendant(&mut rewritten, false, prune, &mut scrub);
            self.finish_font(&mut rewritten, &mut scrub);
            prune.redacted_image_originals.extend(rewritten.orphans);
            scrub.fonts.entry(key).or_default().insert(name, Object::Dictionary(rewritten.dict));
            scrub.fonts_scrubbed += 1;
        }
        for (key, name, entry) in names {
            if let FontEntry::Reference(r) = entry {
                if let Some(copy) = copies.get(&r.id) {
                    scrub
                        .fonts
                        .entry(key)
                        .or_default()
                        .insert(name, Object::Reference(ObjectRef::new(*copy, 0)));
                }
            }
        }
        for (id, element) in structure {
            prune.overrides.insert(id, element);
        }
        Ok(scrub)
    }

    /// Whether `id` is an object a flattened appearance of a redacted page
    /// carries beside it, which only the appearance holds.
    fn is_flat_extra(&self, id: u32) -> bool {
        self.redacted_pages
            .values()
            .any(|page| page.flat.as_ref().is_some_and(|flat| flat.extra(id).is_some()))
    }

    /// Writes the scrubbed font `rewritten` over the objects of the fallback
    /// font `id` that a flattened appearance carries: the appearance is the
    /// only user of its font, so the font is rewritten where it is.
    fn rewrite_flat_font(&mut self, id: u32, mut rewritten: Rewritten) {
        let old_cmap = rewritten.dict.get("ToUnicode").and_then(Object::as_reference);
        let mut replacements: Vec<(u32, Object)> = Vec::new();
        if let Some(descendant) = rewritten.descendant.take() {
            replacements.push((descendant.reference.id, Object::Dictionary(descendant.dict)));
        }
        if let (Some(bytes), Some(old)) = (rewritten.to_unicode.take(), old_cmap) {
            let mut dict = HashMap::new();
            dict.insert("Length".to_string(), Object::Integer(bytes.len() as i64));
            replacements.push((old.id, Object::stream(dict, bytes::Bytes::from(bytes))));
        }
        // A Type 3 font loses the entries of the glyphs only the removed text
        // showed, and the streams of those procedures (and any object the
        // rewritten font no longer names) are not written.
        if let Some(mut table) = rewritten.char_procs.take() {
            for name in &rewritten.procs {
                table.remove(name);
            }
            rewritten.dict.insert("CharProcs".to_string(), Object::Dictionary(table));
        }
        for orphan in std::mem::take(&mut rewritten.orphans) {
            if Some(orphan) == old_cmap.map(|r| r.id) {
                continue;
            }
            for page in self.redacted_pages.values_mut() {
                if let Some(flat) = page.flat.as_mut() {
                    flat.remove_extra(orphan);
                }
            }
        }
        replacements.push((id, Object::Dictionary(rewritten.dict)));
        for (target, object) in replacements {
            for page in self.redacted_pages.values_mut() {
                if let Some(flat) = page.flat.as_mut() {
                    if flat.extra(target).is_some() {
                        flat.replace_extra(target, object);
                        break;
                    }
                }
            }
        }
    }

    /// Places the descendant CID font `rewritten` carries: over its object
    /// when the font is rewritten where it is, else as a new object the
    /// font copy names.
    fn place_descendant(
        &mut self,
        rewritten: &mut Rewritten,
        in_place: bool,
        prune: &mut PagePrune,
        scrub: &mut Scrub,
    ) {
        let Some(descendant) = rewritten.descendant.take() else { return };
        let font = Object::Dictionary(descendant.dict);
        if in_place {
            prune.overrides.insert(descendant.reference.id, font);
            return;
        }
        let copy = self.allocate_object_id();
        scrub.objects.push((copy, font));
        rewritten.dict.insert(
            "DescendantFonts".to_string(),
            Object::Array(vec![Object::Reference(ObjectRef::new(copy, 0))]),
        );
    }

    /// Allocates the streams `rewritten` names and sets the entries that
    /// point at them.
    fn finish_font(&mut self, rewritten: &mut Rewritten, scrub: &mut Scrub) {
        if let Some(bytes) = rewritten.to_unicode.take() {
            let (id, length_id) = (self.allocate_object_id(), self.allocate_object_id());
            rewritten
                .dict
                .insert("ToUnicode".to_string(), Object::Reference(ObjectRef::new(id, 0)));
            scrub.streams.push(NewStream { id, length_id, dict: HashMap::new(), bytes });
        }
        if let Some(mut table) = rewritten.char_procs.take() {
            let (id, length_id) = (self.allocate_object_id(), self.allocate_object_id());
            for name in &rewritten.procs {
                table.insert(name.clone(), Object::Reference(ObjectRef::new(id, 0)));
            }
            rewritten.dict.insert("CharProcs".to_string(), Object::Dictionary(table));
            scrub.streams.push(NewStream { id, length_id, dict: HashMap::new(), bytes: Vec::new() });
        }
    }

    /// Counts, for every font of `states`, the codes the rest of the document
    /// shows: the pages the redaction did not touch, the forms and
    /// appearances they draw, and what the redacted pages keep. A font the
    /// AcroForm offers its fields is shown by values that do not exist yet:
    /// it cannot be counted.
    fn count_users(
        &self,
        sources: &[usize],
        forms: &HashMap<usize, DocForms>,
        states: &mut BTreeMap<u32, FontState>,
    ) -> Result<()> {
        let catalog = self.source.catalog()?;
        if let Some(acroform) = catalog.as_dict().and_then(|c| c.get("AcroForm")) {
            let acroform = self.source.resolve_object(acroform)?;
            if let Some(defaults) = acroform.as_dict().and_then(|a| a.get("DR")) {
                let defaults = self.source.resolve_object(defaults)?;
                for entry in font_entries(&self.source, &defaults)?.values() {
                    if let FontEntry::Reference(r) = entry {
                        if let Some(state) = states.get_mut(&r.id) {
                            state.uncountable = true;
                        }
                    }
                }
            }
        }
        let dropped = self.redacted_annotation_ids();
        let mut counter = Counter { source: &self.source, states, visited: HashSet::new() };
        for index in 0..self.source.page_count()? {
            crate::cancel::poll()?;
            let page_ref = self.source.get_page_ref(index)?;
            let page = self.source.load_object(page_ref)?;
            let resources = Self::inherited_resources(&self.source, index)?;
            if sources.contains(&index) {
                counter.count_kept(&self.redacted_pages[&index].scope, &resources, &forms[&index])?;
            } else {
                match self.source.page_operators(index) {
                    Ok(ops) => counter.count_stream(ops, &resources, 0)?,
                    Err(_) => counter.give_up(&resources)?,
                }
            }
            counter.count_annotations(&page, &dropped)?;
        }
        Ok(())
    }
}
