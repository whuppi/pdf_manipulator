//! Unused entries of resource dictionaries, dropped by a full rewrite.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A full rewrite that prunes resources asks [`plan`] first. It reads what
//! the kept pages paint and answers with [`Cut`]s: for the subdictionaries
//! `/Font`, `/XObject`, `/ColorSpace`, `/ExtGState`, `/Pattern`, `/Shading`
//! and `/Properties` of each resource dictionary, the names some content
//! resolves against it. A cut holds no object: the save applies it to the
//! object it writes, when it loads it. What only a dropped entry reached
//! falls out of the output through garbage collection. The AcroForm's
//! `/DR` is never touched.
//!
//! A subdictionary is identified by its object id when it is indirect and
//! by its owner's id and the keys down to it when it is direct, never by a
//! document-wide set of names. The names of a subdictionary are the union
//! of what every content that resolves against it names:
//! - a kept page's content streams against the page's `/Resources`;
//! - a Form XObject, a soft-mask group, a tiling pattern or a Type 3
//!   glyph procedure against its own `/Resources`, or against the
//!   resources of the context that paints it when it has none;
//! - an appearance stream against its own `/Resources`, or against the
//!   page's when it has none.
//!
//! Each content is read once, as operators, in the memory of one token
//! (`Operators` with inline image data skipped); no content is held and no
//! content has a size limit. The names come from the operator arms of
//! [`consumed_names`]. What the walk knows of a resource dictionary held
//! directly in an object is dropped as soon as the walk of its content ends,
//! and a form's own dictionary is only met when its work item is read, so
//! the memory of a prune does not grow with the pages or the forms.
//!
//! The rewrite is conservative. A context stays whole (every
//! subdictionary of its resource dictionary keeps every entry) when:
//! - a part of its content cannot be opened, or the reader fails
//!   mid-stream;
//! - the operator source reports any damage;
//! - a name its operators held but no arm consumed is a key of one of its
//!   subdictionaries;
//! - the walk reaches a form that is already being walked (a cycle) or
//!   one nested deeper than [`MAX_NESTING`];
//! - a form, pattern, glyph procedure or soft-mask group it paints has
//!   `/Resources` that cannot be read;
//! - another object than the ones walked points at the dictionary;
//! - the page is edited while the save writes it.
//!
//! The same walk feeds the font cut (`host::font_subset`) when the scope
//! holds a [`Collector`]: every operator it reads is also offered to the
//! collector, so no content is opened for the cut a second time. The cut
//! needs the glyphs of every content a viewer may paint, which is more than
//! the names the prune follows: an entry no name reaches, in a dictionary
//! the prune leaves whole, still stays in the file. So the walk also reads,
//! once, each content that its resource dictionaries hold and no name
//! reached, after the names of the dictionary are final. Those reads feed
//! the collector and nothing else: they record no name, pin nothing, and a
//! resource dictionary only they reach is never cut. A content that some
//! name reaches is read once for both.

use crate::cancel::CancelCheck;
use crate::content::operators::Operator;
use crate::document::PdfDocument;
use crate::error::Result;
use crate::host::content::inline::components;
use crate::host::font_subset::{Collector, Feed};
use crate::host::content::parts::{decoded_reader, PartsReader};
use crate::host::content::{consumed_names, InlineImages, Mode, Operators, Resolver};
use crate::object::{Object, ObjectRef};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::rc::Rc;
use smallvec::SmallVec;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

type Dict = HashMap<String, Object>;

/// The resource subdictionaries a rewrite prunes, in the order of the
/// constants below.
const KINDS: [&str; 7] =
    ["Font", "XObject", "ColorSpace", "ExtGState", "Pattern", "Shading", "Properties"];
const FONT: usize = 0;
const XOBJECT: usize = 1;
const COLOR_SPACE: usize = 2;
const EXT_G_STATE: usize = 3;
const PATTERN: usize = 4;
const SHADING: usize = 5;
const PROPERTIES: usize = 6;

/// Forms painted from forms deeper than this are not walked: the context
/// that paints the first of them stays whole. A viewer cuts nesting off
/// at a depth of its own, so what lies beyond is not known to be painted.
const MAX_NESTING: usize = 64;

/// The keys down from an object to a dictionary in it: `/Resources` and the
/// names of the resource subdictionaries, so at most two, none allocated.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Path {
    /// Starts with `/Resources`.
    resources: bool,
    /// Ends with the subdictionary `KINDS[kind]`; `NONE` for no such key.
    kind: u8,
}

impl Path {
    const NONE: u8 = u8::MAX;
    const EMPTY: Path = Path { resources: false, kind: Path::NONE };

    /// The path of the `/Resources` entry.
    fn resources() -> Path {
        Path { resources: true, kind: Path::NONE }
    }

    /// This path and then the subdictionary `kind`.
    fn then(self, kind: usize) -> Path {
        Path { kind: kind as u8, ..self }
    }

    fn is_empty(&self) -> bool {
        !self.resources && self.kind == Path::NONE
    }

    /// The keys, in order, and how many there are.
    fn keys(&self) -> ([&'static str; 2], usize) {
        let mut keys = [""; 2];
        let mut len = 0;
        if self.resources {
            keys[len] = "Resources";
            len += 1;
        }
        if self.kind != Path::NONE {
            keys[len] = KINDS[usize::from(self.kind)];
            len += 1;
        }
        (keys, len)
    }
}

/// A dictionary, named by the object that holds it and the keys down to it.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Slot {
    owner: u32,
    path: Path,
}

/// What a work item reads: one form, or the parts of a page's `/Contents`.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Visit {
    Form(u32),
    Page(Box<[u32]>),
}

/// One resource subdictionary and the resource dictionaries that hold it.
struct Sub {
    kind: usize,
    dict: Dict,
    holders: Vec<Slot>,
    /// For `/ColorSpace`: the component count of each entry that has one,
    /// which an unfiltered inline image's size depends on.
    components: Option<Arc<HashMap<String, usize>>>,
}

/// A resource dictionary with the slots of its subdictionaries.
struct Ctx {
    slot: Slot,
    subs: [Option<Slot>; 7],
}

/// What a walk reads against a resource dictionary.
enum Content {
    /// The `/Contents` of a kept page: the part ids, and whether it is one
    /// stream rather than an array of them.
    Page { parts: Vec<u32>, single: bool },
    /// A form, a soft-mask group, a tiling pattern, a glyph procedure.
    Form(u32),
}

/// The forms being walked, innermost first.
struct Link {
    id: u32,
    depth: usize,
    parent: Option<Rc<Link>>,
}

fn in_chain(chain: &Option<Rc<Link>>, id: u32) -> bool {
    let mut at = chain.as_deref();
    while let Some(link) = at {
        if link.id == id {
            return true;
        }
        at = link.parent.as_deref();
    }
    false
}

/// The resource dictionary a content is read against.
enum Target {
    Ready(Rc<Ctx>),
    /// The `/Resources` entry `value` of object `owner`, met when the work
    /// item is read, so a form's context lives only while the form is walked.
    Own { owner: u32, value: Object },
}

/// What a work item is for, as bits. A content read for several purposes is
/// read once.
///
/// A prune that feeds no collector only ever asks for [`NAMES`].
type Need = u8;
/// Record the names the content resolves and queue what they paint.
const NAMES: Need = 1;
/// Offer the content's operators to the collector.
const FEED: Need = 2;
/// The fonts of the dictionary the content is read against stay whole: a
/// form field can use them, or the save edits the page. Reached contents
/// are read for the collector as well.
const POISON: Need = 4;

/// Content to read against a resource dictionary.
struct Work {
    content: Content,
    target: Target,
    /// The context that paints the content, kept whole when the target
    /// cannot be read.
    painter: Option<Rc<Ctx>>,
    chain: Option<Rc<Link>>,
    /// The purposes of this read that no earlier read of the same content
    /// against the same dictionary served.
    need: Need,
}

/// What the prune cuts out of one object, by the names of dictionaries
/// down from the object's own.
#[derive(Debug, Clone)]
pub(crate) enum Cut {
    /// The dictionary at `path` keeps the entries named in `keep` (sorted).
    Keep { path: Path, keep: Box<[Box<str>]> },
    /// The entry at `path` is removed.
    Remove { path: Path },
}

/// The cuts of one object; most have one.
pub(crate) type Cuts = SmallVec<[Cut; 1]>;

/// `object` with `cuts` applied to its dictionary (a stream's own).
pub(crate) fn apply(mut object: Object, cuts: &[Cut]) -> Object {
    match &mut object {
        Object::Dictionary(dict) => apply_to_dict(dict, cuts),
        Object::Stream { dict, .. } => apply_to_dict(dict, cuts),
        _ => {},
    }
    object
}

/// `dict` with `cuts` applied.
pub(crate) fn apply_to_dict(dict: &mut Dict, cuts: &[Cut]) {
    fn descend<'d>(dict: &'d mut Dict, path: &[&'static str]) -> Option<&'d mut Dict> {
        let mut dict = dict;
        for key in path {
            dict = match dict.get_mut(*key)? {
                Object::Dictionary(inner) => inner,
                _ => return None,
            };
        }
        Some(dict)
    }
    for cut in cuts {
        match cut {
            Cut::Keep { path, keep } => {
                let (keys, len) = path.keys();
                if let Some(target) = descend(dict, &keys[..len]) {
                    target.retain(|name, _| keep.binary_search_by(|k| (**k).cmp(name.as_str())).is_ok());
                }
            },
            Cut::Remove { path } => {
                let (keys, len) = path.keys();
                if let Some((last, parent)) = keys[..len].split_last() {
                    if let Some(target) = descend(dict, parent) {
                        target.remove(*last);
                    }
                }
            },
        }
    }
}

/// The names one content resolves, by kind.
#[derive(Default)]
struct Names {
    sets: [HashSet<String>; 7],
}

/// A kept page of the document the save writes from.
pub(crate) struct ScopePage {
    pub id: u32,
    /// The save edits this page while writing: its resources stay whole.
    pub held: bool,
}

/// One document a save writes objects from, in its own numbering.
pub(crate) struct Scope<'a, 'c> {
    pub doc: &'a PdfDocument,
    /// Objects the editor stages over the document's own.
    pub staged: &'a HashMap<u32, Object>,
    /// Objects the save replaces.
    pub overrides: &'a HashMap<u32, Object>,
    /// Where [`plan`] puts the cuts, by id of the object they apply to.
    pub cuts: &'a mut HashMap<u32, Cuts>,
    /// Ids the save never writes.
    pub excluded: &'a HashSet<u32>,
    pub pages: Vec<ScopePage>,
    /// A page dictionary as the save writes it, when that is not the
    /// document's own (a merged page carries the attributes it inherited).
    pub page_object: Option<&'a dyn Fn(u32) -> Option<Object>>,
    /// The ids of the objects the save writes from this document, or
    /// `None` for every object of the document that is not excluded.
    pub writers: Option<Vec<u32>>,
    /// The font cut that reads the contents along with the prune.
    pub glyphs: Option<&'a mut Collector<'c>>,
}

/// Cuts the resource dictionaries of the pages of `scope` down: the cuts
/// go into `scope.cuts`, keyed by id in the numbering of the scope's
/// document.
pub(crate) fn plan(scope: Scope<'_, '_>, cancel: &CancelCheck) -> Result<()> {
    let Scope { doc, staged, overrides, cuts, excluded, pages, page_object, writers, glyphs } =
        scope;
    let mut pruner = Pruner {
        doc,
        staged,
        overrides,
        cuts,
        excluded,
        writers,
        page_object,
        glyphs,
        cache: HashMap::new(),
        ctxs: HashMap::new(),
        subs: HashMap::new(),
        used: HashMap::new(),
        pinned: HashSet::new(),
        seen: HashMap::new(),
        visited: HashMap::new(),
        work: Vec::new(),
        created: Vec::new(),
        local_visited: HashMap::new(),
        finished: HashSet::new(),
        unnamed: HashSet::new(),
        poisoned: HashSet::new(),
        unioned: HashSet::new(),
    };
    let mut done = HashSet::new();
    for ScopePage { id: page, held } in pages {
        cancel.check()?;
        if done.insert(page) {
            pruner.page(page, held);
            pruner.drain(cancel)?;
        }
    }
    pruner.finish_unnamed(cancel)?;
    pruner.close_color_spaces();
    pruner.pin_shared(cancel)?;
    cancel.check()?;
    pruner.cut_shared();
    Ok(())
}

struct Pruner<'a, 'c> {
    doc: &'a PdfDocument,
    staged: &'a HashMap<u32, Object>,
    overrides: &'a HashMap<u32, Object>,
    cuts: &'a mut HashMap<u32, Cuts>,
    excluded: &'a HashSet<u32>,
    writers: Option<Vec<u32>>,
    page_object: Option<&'a dyn Fn(u32) -> Option<Object>>,
    glyphs: Option<&'a mut Collector<'c>>,
    /// Objects as the walk reads them; a stream is its dictionary and its
    /// extent, never its data.
    cache: HashMap<u32, Option<Rc<Object>>>,
    ctxs: HashMap<Slot, Rc<Ctx>>,
    subs: HashMap<Slot, Sub>,
    /// The names of each subdictionary some content resolves against it.
    used: HashMap<Slot, HashSet<String>>,
    /// Subdictionaries that stay whole.
    pinned: HashSet<Slot>,
    /// Indirect dictionaries the walk reached, with the ids of the objects
    /// it found the reference in.
    seen: HashMap<u32, HashSet<u32>>,
    /// What each content has been read for against each dictionary.
    visited: HashMap<(Visit, Slot), Need>,
    work: Vec<Work>,
    /// The resource dictionaries held directly in an object that are being
    /// walked: a direct subdictionary has one owner, so what is known about
    /// it is final when the last work item reading against its dictionary
    /// is done, and is not kept after it.
    created: Vec<Slot>,
    /// The `visited` keys of those dictionaries, by dictionary.
    local_visited: HashMap<Slot, Vec<Visit>>,
    /// The objects whose own direct resources are done.
    finished: HashSet<u32>,
    /// The dictionaries only a read for the collector reached: no name is
    /// recorded against them, so they are never cut.
    unnamed: HashSet<Slot>,
    /// The dictionaries whose fonts stay whole, and so the contents they
    /// hold: those are read as poisoned.
    poisoned: HashSet<Slot>,
    /// The dictionaries whose entries no name reached have been queued for
    /// the collector, and whether as poisoned.
    unioned: HashSet<(Slot, bool)>,
}

impl Pruner<'_, '_> {
    /// The staged or replacing object for `id`, if the save has one.
    fn replaced(&self, id: u32) -> Option<&Object> {
        self.overrides.get(&id).or_else(|| self.staged.get(&id))
    }

    /// The object the save would write for `id`, before the cuts: a stream
    /// carries its extent, not its data.
    fn read(&self, id: u32) -> Option<Object> {
        match self.replaced(id) {
            Some(obj) => Some(obj.clone()),
            None => self
                .page_object
                .and_then(|page| page(id))
                .or_else(|| self.doc.load_object(ObjectRef::new(id, 0)).ok()),
        }
    }

    /// The ids `id` references in the object the save writes for it.
    fn refs_of(&self, id: u32, out: &mut Vec<u32>) {
        match self.replaced(id) {
            Some(obj) => push_refs(obj, out),
            None => match self.page_object.and_then(|page| page(id)) {
                Some(page) => push_refs(&page, out),
                None => self.doc.collect_refs_of(id, out),
            },
        }
    }

    fn object(&mut self, id: u32) -> Option<Rc<Object>> {
        if let Some(hit) = self.cache.get(&id) {
            return hit.clone();
        }
        let read = self.read(id).map(Rc::new);
        self.cache.insert(id, read.clone());
        read
    }

    /// `value` itself, or the object it references.
    fn resolved(&mut self, value: &Object) -> Option<Rc<Object>> {
        match value {
            Object::Reference(r) => self.object(r.id),
            other => Some(Rc::new(other.clone())),
        }
    }

    /// The resource dictionary `value` (the `/Resources` entry of the
    /// object `holder`), with the slots of its subdictionaries. `named` is
    /// whether names are recorded against it: a dictionary met only for the
    /// collector is not, and the prune leaves it and the objects it points
    /// at as the walk of names would have found them.
    fn context(&mut self, holder: u32, value: &Object, named: bool) -> Option<Rc<Ctx>> {
        let (slot, dict) = match value {
            Object::Reference(r) => {
                if named {
                    self.seen.entry(r.id).or_default().insert(holder);
                }
                let obj = self.object(r.id)?;
                let Object::Dictionary(dict) = &*obj else { return None };
                (Slot { owner: r.id, path: Path::EMPTY }, dict.clone())
            },
            Object::Dictionary(dict) => {
                (Slot { owner: holder, path: Path::resources() }, dict.clone())
            },
            _ => return None,
        };
        if let Some(ctx) = self.ctxs.get(&slot).cloned() {
            if named && self.unnamed.remove(&slot) {
                for sub in ctx.subs.iter().flatten().filter(|sub| sub.path.is_empty()) {
                    self.seen.entry(sub.owner).or_default().insert(slot.owner);
                }
            }
            return Some(ctx);
        }
        let mut font_holder = None;
        let mut subs: [Option<Slot>; 7] = Default::default();
        for (kind, key) in KINDS.iter().enumerate() {
            let (sub_slot, sub_dict) = match dict.get(*key) {
                Some(Object::Reference(r)) => {
                    if named {
                        self.seen.entry(r.id).or_default().insert(slot.owner);
                    }
                    let Some(obj) = self.object(r.id) else { continue };
                    let Object::Dictionary(sub_dict) = &*obj else { continue };
                    (Slot { owner: r.id, path: Path::EMPTY }, sub_dict.clone())
                },
                Some(Object::Dictionary(sub_dict)) => {
                    (Slot { owner: slot.owner, path: slot.path.then(kind) }, sub_dict.clone())
                },
                _ => continue,
            };
            if !self.subs.contains_key(&sub_slot) {
                let components = (kind == COLOR_SPACE).then(|| self.space_components(&sub_dict));
                self.subs.insert(
                    sub_slot.clone(),
                    Sub { kind, dict: sub_dict, holders: Vec::new(), components },
                );
            }
            let sub = self.subs.get_mut(&sub_slot).expect("the subdictionary was just inserted");
            if sub.kind != kind {
                if named {
                    self.pinned.insert(sub_slot);
                }
                continue;
            }
            if !sub.holders.contains(&slot) {
                sub.holders.push(slot.clone());
            }
            self.used.entry(sub_slot.clone()).or_default();
            if kind == FONT {
                font_holder = Some(sub_slot.owner);
            }
            subs[kind] = Some(sub_slot);
        }
        if let (Some(glyphs), Some(holder)) = (self.glyphs.as_mut(), font_holder) {
            glyphs.reached(holder);
        }
        if !named {
            self.unnamed.insert(slot.clone());
        }
        if !slot.path.is_empty() {
            self.created.push(slot.clone());
        }
        let ctx = Rc::new(Ctx { slot: slot.clone(), subs });
        self.ctxs.insert(slot, ctx.clone());
        Some(ctx)
    }

    /// Whether a resource dictionary that holds `sub` has had names recorded
    /// against it: a subdictionary only unnamed dictionaries hold is not cut.
    fn is_named_sub(&self, sub: &Sub) -> bool {
        sub.holders.iter().any(|holder| !self.unnamed.contains(holder))
    }

    /// The component count of each entry of a `/ColorSpace` dictionary.
    fn space_components(&mut self, dict: &Dict) -> Arc<HashMap<String, usize>> {
        let mut counts = HashMap::new();
        for (name, value) in dict {
            let Some(space) = self.resolved(value) else { continue };
            if let Some(n) = components(&space, None) {
                counts.insert(name.clone(), n);
            }
        }
        Arc::new(counts)
    }

    /// The resolver of an inline image's named colour space against `ctx`.
    fn resolver(&self, ctx: &Ctx) -> Resolver {
        let counts = ctx.subs[COLOR_SPACE]
            .as_ref()
            .and_then(|slot| self.subs.get(slot))
            .and_then(|sub| sub.components.clone());
        Box::new(move |name| counts.as_ref().and_then(|counts| counts.get(name).copied()))
    }

    /// Keeps every subdictionary of `ctx` whole.
    fn pin(&mut self, ctx: &Ctx) {
        self.pinned.extend(ctx.subs.iter().flatten().cloned());
    }

    fn entry(&self, ctx: &Ctx, kind: usize, name: &str) -> Option<Object> {
        let slot = ctx.subs[kind].as_ref()?;
        self.subs.get(slot)?.dict.get(name).cloned()
    }

    fn is_key(&self, ctx: &Ctx, name: &str) -> bool {
        ctx.subs
            .iter()
            .flatten()
            .any(|slot| self.subs.get(slot).is_some_and(|sub| sub.dict.contains_key(name)))
    }

    /// What a read of a content is for: `names` to record the names it
    /// resolves, and, when a collector is fed, to offer it the operators
    /// (`poison`: and to keep the fonts of the dictionary whole).
    fn need(&self, names: bool, poison: bool) -> Need {
        let mut need = 0;
        if names {
            need |= NAMES;
        }
        if self.glyphs.is_some() {
            need |= FEED;
            if poison {
                need |= POISON;
            }
        }
        need
    }

    /// The plan of the font cut cannot be trusted: a content could not be
    /// read to its end.
    fn glyphs_damaged(&mut self) {
        if let Some(glyphs) = self.glyphs.as_mut() {
            glyphs.disable();
        }
    }

    /// Leaves whole the programs of the fonts `ctx` holds, and of everything
    /// the dictionaries reached from it hold.
    fn poison_ctx(&mut self, ctx: &Ctx) {
        self.poisoned.insert(ctx.slot);
        let Some(fonts) = ctx.subs[FONT].as_ref().and_then(|slot| self.subs.get(slot)) else {
            return;
        };
        if let Some(glyphs) = self.glyphs.as_mut() {
            glyphs.poison_map(&fonts.dict);
        }
    }

    /// Queues `content` against `target`, once per dictionary and purpose.
    fn push(
        &mut self,
        content: Content,
        target: Target,
        painter: Option<Rc<Ctx>>,
        chain: Option<Rc<Link>>,
        need: Need,
    ) {
        let slot = match &target {
            Target::Ready(ctx) => ctx.slot.clone(),
            Target::Own { owner, value } => match slot_of(*owner, value) {
                Some(slot) => slot,
                None => {
                    if need & NAMES != 0 {
                        if let Some(painter) = &painter {
                            self.pin(painter);
                        }
                    }
                    return;
                },
            },
        };
        let visit = match &content {
            Content::Page { parts, .. } => Visit::Page(parts.as_slice().into()),
            Content::Form(id) => Visit::Form(*id),
        };
        let done = self.visited.get(&(visit.clone(), slot)).copied().unwrap_or(0);
        let remaining = need & !done;
        if remaining == 0 {
            return;
        }
        // A read only for the collector is moot once the plan is dropped.
        if remaining & NAMES == 0 && self.glyphs.as_ref().map_or(true, |g| g.is_disabled()) {
            return;
        }
        self.visited.insert((visit.clone(), slot), done | need);
        if done == 0 && !slot.path.is_empty() {
            self.local_visited.entry(slot).or_default().push(visit);
        }
        self.work.push(Work { content, target, painter, chain, need: remaining });
    }

    /// Queues what a kept page paints: its contents against its resources,
    /// and each appearance stream with resources of its own.
    fn page(&mut self, id: u32, held: bool) {
        let Some(obj) = self.object(id) else { return };
        let Some(dict) = dict_of(&obj) else { return };
        let own = dict.get("Resources").and_then(|value| self.context(id, value, true));
        let glyphs = self.glyphs.is_some();
        if held {
            if let Some(ctx) = &own {
                self.pin(ctx);
            }
            if !glyphs {
                return;
            }
        }
        // The collector reads a page against the nearest resources it
        // inherits, which the prune never cuts.
        let ctx = match (&own, glyphs) {
            (None, true) => self.inherited_resources(id),
            _ => own.clone(),
        };
        if held {
            if let Some(ctx) = &ctx {
                self.poison_ctx(ctx);
            }
        }
        let names = !held && own.is_some();
        let need = self.need(names, held);
        match (&ctx, dict.get("Contents")) {
            (Some(ctx), Some(contents)) => match self.content_ids(contents) {
                Some((parts, _)) if parts.is_empty() => {},
                Some((parts, single)) => self.push(
                    Content::Page { parts, single },
                    Target::Ready(ctx.clone()),
                    Some(ctx.clone()),
                    None,
                    need,
                ),
                None => {
                    if names {
                        self.pin(ctx);
                    }
                    self.glyphs_damaged();
                },
            },
            // A page with no resources shows no font; one with text is damage.
            (None, Some(_)) => self.glyphs_damaged(),
            _ => {},
        }
        for (form, names, poison) in self.appearance_forms(dict, held) {
            let Some(obj) = self.object(form) else { continue };
            let Some(form_dict) = dict_of(&obj) else { continue };
            // Some viewers read an appearance stream without resources of
            // its own against the page's.
            let (target, names) = match form_dict.get("Resources") {
                Some(value) if self.is_done(form, value, poison) => continue,
                Some(value) => (Target::Own { owner: form, value: value.clone() }, names),
                None => match &ctx {
                    Some(ctx) => (Target::Ready(ctx.clone()), names && own.is_some()),
                    None => continue,
                },
            };
            let need = self.need(names, poison);
            self.enter(form, target, ctx.clone(), &None, need);
        }
    }

    /// The resources the nearest ancestor of `page` holds, when the page has
    /// none of its own.
    fn inherited_resources(&mut self, page: u32) -> Option<Rc<Ctx>> {
        let mut at = page;
        for _ in 0..MAX_NESTING {
            let obj = self.object(at)?;
            let dict = dict_of(&obj)?;
            if let Some(value) = dict.get("Resources") {
                return self.context(at, value, false);
            }
            at = dict.get("Parent")?.as_reference()?.id;
        }
        None
    }

    /// The content streams `value` (a page's `/Contents`) names, and
    /// whether it is a single stream.
    fn content_ids(&mut self, value: &Object) -> Option<(Vec<u32>, bool)> {
        let items = match value {
            Object::Reference(r) => match &*self.object(r.id)? {
                Object::Stream { .. } => return Some((vec![r.id], true)),
                Object::Array(items) => items.clone(),
                _ => return None,
            },
            Object::Array(items) => items.clone(),
            _ => return None,
        };
        let mut parts = Vec::with_capacity(items.len());
        for item in &items {
            match item {
                Object::Null => {},
                other => parts.push(other.as_reference()?.id),
            }
        }
        Some((parts, false))
    }

    /// The appearance streams of the annotations a page lists, each with
    /// whether its names count (the prune walks the annotations the save
    /// writes and holds by reference) and whether its fonts stay whole (a
    /// form field, or a page the save edits). The collector is also given
    /// the annotations the prune does not walk, since they stay in the file
    /// when the save writes them.
    fn appearance_forms(&mut self, page: &Dict, held: bool) -> Vec<(u32, bool, bool)> {
        let glyphs = self.glyphs.is_some();
        let Some(annots) = page.get("Annots").and_then(|v| self.resolved(v)) else {
            return Vec::new();
        };
        let Object::Array(annots) = &*annots else { return Vec::new() };
        let mut forms = Vec::new();
        for annot in annots {
            if crate::cancel::poll().is_err() {
                break;
            }
            let (names, annot) = match annot {
                Object::Reference(r) => {
                    let names = !self.excluded.contains(&r.id);
                    if !names && !glyphs {
                        continue;
                    }
                    let Some(annot) = self.object(r.id) else { continue };
                    (names, annot)
                },
                direct if glyphs => (false, Rc::new(direct.clone())),
                _ => continue,
            };
            let Some(annot) = dict_of(&annot) else { continue };
            let poison = held
                || annot.get("Subtype").and_then(|s| s.as_name()) == Some("Widget")
                || annot.contains_key("FT");
            let Some(ap) = annot.get("AP") else { continue };
            let Some(ap) = self.resolved(ap) else { continue };
            let Object::Dictionary(ap) = &*ap else { continue };
            let mut found = Vec::new();
            for key in ["N", "R", "D"] {
                if let Some(value) = ap.get(key) {
                    self.streams_of(value, true, &mut found);
                }
            }
            forms.extend(found.into_iter().map(|form| (form, names, poison)));
        }
        forms
    }

    /// The stream `value` references, or the streams of the state
    /// dictionary it is (or references).
    fn streams_of(&mut self, value: &Object, states: bool, out: &mut Vec<u32>) {
        let Some(obj) = self.resolved(value) else { return };
        match (&*obj, value) {
            (Object::Stream { .. }, Object::Reference(r)) => out.push(r.id),
            (Object::Dictionary(entries), _) if states => {
                for entry in entries.values() {
                    self.streams_of(entry, false, out);
                }
            },
            _ => {},
        }
    }

    /// The operators of `content` read against `ctx`, or `None` when it
    /// cannot be opened.
    fn open(
        &self,
        content: &Content,
        ctx: &Ctx,
        cancel: &CancelCheck,
    ) -> Option<Operators<Box<dyn Read>>> {
        let ops = match content {
            Content::Form(id) => match self.overrides.get(id).or_else(|| self.staged.get(id)) {
                Some(obj) => Operators::new(decoded_reader(obj).ok()?, Mode::Full),
                None => self.doc.form_operators(ObjectRef::new(*id, 0)).ok()?,
            },
            Content::Page { parts, single } => {
                let unreadable = Arc::new(AtomicU64::new(0));
                let mut objects = Vec::with_capacity(parts.len());
                for &id in parts {
                    objects.push(self.read(id)?);
                }
                let source = if *single {
                    PartsReader::of_one(
                        decoded_reader(objects.first()?).ok()?,
                        Arc::clone(&unreadable),
                    )
                } else {
                    PartsReader::of_array(objects, Arc::clone(&unreadable))
                };
                Operators::new(Box::new(source) as Box<dyn Read>, Mode::Full)
                    .with_parts_counter(unreadable)
            },
        };
        Some(
            ops.with_cancel(cancel.clone())
                .with_inline_images(InlineImages::Skip)
                .with_resolver(self.resolver(ctx)),
        )
    }

    /// Reads one content: records what it names against its context and
    /// queues what it paints, and offers its operators to the collector.
    /// A work item for a content that an earlier read already took the
    /// operators of reads it only for the names, and one that asks only to
    /// keep fonts whole reads nothing.
    fn process(&mut self, work: Work, cancel: &CancelCheck) -> Result<()> {
        let Work { content, target, painter, chain, need } = work;
        let wants_names = need & NAMES != 0;
        let feeds = need & FEED != 0;
        let poison = need & POISON != 0;
        let ctx = match target {
            Target::Ready(ctx) => ctx,
            Target::Own { owner, value } => match self.context(owner, &value, wants_names) {
                Some(ctx) => ctx,
                None => {
                    if wants_names {
                        if let Some(painter) = &painter {
                            self.pin(painter);
                        }
                    }
                    // A viewer may fall back to the painter's resources.
                    if let Some(painter) = painter.filter(|_| feeds) {
                        let target = Target::Ready(painter.clone());
                        self.push(content, target, Some(painter), chain, need & !NAMES);
                    }
                    return Ok(());
                },
            },
        };
        if poison {
            self.poison_ctx(&ctx);
        }
        if need & (NAMES | FEED) == 0 {
            return Ok(());
        }
        let Some(mut ops) = self.open(&content, &ctx, cancel) else {
            if wants_names {
                self.pin(&ctx);
            }
            if feeds {
                self.glyphs_damaged();
            }
            return Ok(());
        };
        let mut names = Names::default();
        let mut unexplained = false;
        let mut feed = Feed::new();
        let no_fonts = Dict::new();
        while let Some(op) = ops.next() {
            let op = op?;
            if wants_names && !unexplained {
                let consumed = consumed_names(&op);
                // A name the operator held that no arm of it uses may be a
                // resource the source misread.
                if ops
                    .pending_names()
                    .iter()
                    .any(|name| !consumed.contains(&name.as_str()) && self.is_key(&ctx, name))
                {
                    unexplained = true;
                    if !feeds {
                        break;
                    }
                } else {
                    record(&op, &consumed, &mut names);
                }
            }
            if feeds {
                let fonts = ctx.subs[FONT]
                    .as_ref()
                    .and_then(|slot| self.subs.get(slot))
                    .map_or(&no_fonts, |sub| &sub.dict);
                if let Some(glyphs) = self.glyphs.as_mut() {
                    glyphs.op(&mut feed, &op, fonts, poison);
                }
            }
        }
        let damaged = !ops.damage().is_clean();
        if damaged && feeds {
            self.glyphs_damaged();
        }
        if !wants_names {
            return Ok(());
        }
        if unexplained || damaged {
            self.pin(&ctx);
            return Ok(());
        }
        drop(ops);
        for (kind, slot) in ctx.subs.iter().enumerate() {
            if let Some(slot) = slot {
                self.used.entry(slot.clone()).or_default().extend(names.sets[kind].iter().cloned());
            }
        }
        if !self.follow(&names, &ctx, &chain, poison) {
            self.pin(&ctx);
        }
        Ok(())
    }

    /// Queues what `names` paint against `ctx`. `false` when one of them
    /// cannot be read. `poison` is whether the painting content's fonts stay
    /// whole, and so what it paints.
    fn follow(
        &mut self,
        names: &Names,
        ctx: &Rc<Ctx>,
        chain: &Option<Rc<Link>>,
        poison: bool,
    ) -> bool {
        let need = self.need(true, poison);
        for name in &names.sets[XOBJECT] {
            let Some(Object::Reference(r)) = self.entry(ctx, XOBJECT, name) else { continue };
            // Not cached: a page that paints thousands of forms would hold
            // them all.
            let Some(obj) = self.read(r.id) else { return false };
            let Object::Stream { dict, .. } = &obj else { continue };
            match dict.get("Subtype").and_then(|s| s.as_name()) {
                Some("Image") => self.dictionary_spaces(dict, ctx),
                Some("PS") => {},
                _ => self.paint(r.id, dict, ctx, chain, need),
            }
        }
        for name in &names.sets[FONT] {
            let Some(entry) = self.entry(ctx, FONT, name) else { continue };
            let (holder, font) = match &entry {
                Object::Reference(r) => {
                    let Some(font) = self.object(r.id) else { return false };
                    (Some(r.id), font)
                },
                other => (None, Rc::new(other.clone())),
            };
            let Some(dict) = dict_of(&font) else { continue };
            if dict.get("Subtype").and_then(|s| s.as_name()) != Some("Type3") {
                continue;
            }
            let child = match (holder, dict.get("Resources")) {
                (Some(holder), Some(value)) if self.is_done(holder, value, poison) => continue,
                (Some(holder), Some(value)) => match self.context(holder, value, true) {
                    Some(own) => own,
                    None => {
                        self.pin(ctx);
                        continue;
                    },
                },
                // A font held directly in a resource dictionary has no id to
                // name its own resources by.
                (None, Some(_)) => {
                    self.pin(ctx);
                    continue;
                },
                (_, None) => ctx.clone(),
            };
            let Some(procs) = dict.get("CharProcs").and_then(|v| self.resolved(v)) else {
                continue;
            };
            let Object::Dictionary(procs) = &*procs else { continue };
            let mut ids: Vec<u32> =
                procs.values().filter_map(|v| v.as_reference()).map(|r| r.id).collect();
            ids.sort_unstable();
            for id in ids {
                self.enter(id, Target::Ready(child.clone()), Some(ctx.clone()), chain, need);
            }
        }
        for name in &names.sets[EXT_G_STATE] {
            let Some(entry) = self.entry(ctx, EXT_G_STATE, name) else { continue };
            let Some(state) = self.resolved(&entry) else { return false };
            let Some(mask) = dict_of(&state).and_then(|d| d.get("SMask")) else { continue };
            let Some(mask) = self.resolved(mask) else { continue };
            let Some(Object::Reference(group)) = dict_of(&mask).and_then(|d| d.get("G")) else {
                continue;
            };
            let Some(form) = self.object(group.id) else { return false };
            if let Object::Stream { dict, .. } = &*form {
                self.paint(group.id, dict, ctx, chain, need);
            }
        }
        for name in &names.sets[PATTERN] {
            let Some(entry) = self.entry(ctx, PATTERN, name) else { continue };
            let Some(pattern) = self.resolved(&entry) else { return false };
            match (&*pattern, &entry) {
                (Object::Stream { dict, .. }, Object::Reference(r)) => {
                    self.paint(r.id, dict, ctx, chain, need)
                },
                (Object::Dictionary(dict), _) => {
                    if let Some(shading) = dict.get("Shading").and_then(|v| self.resolved(v)) {
                        if let Some(shading) = dict_of(&shading) {
                            self.dictionary_spaces(shading, ctx);
                        }
                    }
                },
                _ => {},
            }
        }
        for name in &names.sets[SHADING] {
            let Some(entry) = self.entry(ctx, SHADING, name) else { continue };
            let Some(shading) = self.resolved(&entry) else { return false };
            if let Some(shading) = dict_of(&shading) {
                self.dictionary_spaces(shading, ctx);
            }
        }
        true
    }

    /// Queues the content of stream `id` (a form, a soft-mask group, a
    /// tiling pattern) against its own resources, or the painter's when it
    /// has none. Resources it has that cannot be read keep the painter
    /// whole and the stream unwalked: a viewer may fall back to the
    /// painter's for any name.
    fn paint(
        &mut self,
        id: u32,
        dict: &Dict,
        painter: &Rc<Ctx>,
        chain: &Option<Rc<Link>>,
        need: Need,
    ) {
        let target = match dict.get("Resources") {
            Some(value) if self.is_done(id, value, need & POISON != 0) => return,
            Some(value) => Target::Own { owner: id, value: value.clone() },
            None => Target::Ready(painter.clone()),
        };
        self.enter(id, target, Some(painter.clone()), chain, need);
    }

    /// Whether the resources `value` that object `owner` holds directly were
    /// already cut. They are not met again: a walk would find them whole
    /// and cut them as it did, and the collector has had their contents. A
    /// request to keep their fonts whole is met again.
    fn is_done(&self, owner: u32, value: &Object, poison: bool) -> bool {
        !poison && matches!(value, Object::Dictionary(_)) && self.finished.contains(&owner)
    }

    /// Queues stream `id` against `child`. A form already being walked, or
    /// nested deeper than [`MAX_NESTING`], keeps both contexts whole; a
    /// read for the collector only never keeps anything whole, and drops
    /// the plan of the font cut when it is nested too deep.
    fn enter(
        &mut self,
        id: u32,
        target: Target,
        painter: Option<Rc<Ctx>>,
        chain: &Option<Rc<Link>>,
        need: Need,
    ) {
        let depth = chain.as_ref().map_or(1, |link| link.depth + 1);
        if depth > MAX_NESTING || in_chain(chain, id) {
            if depth > MAX_NESTING {
                self.glyphs_damaged();
            }
            if need & NAMES == 0 {
                return;
            }
            if let Some(painter) = &painter {
                self.pin(painter);
            }
            match target {
                Target::Ready(ctx) => self.pin(&ctx),
                // Met now, while the walk that is cut off still holds it.
                Target::Own { owner, value } => {
                    if let Some(ctx) = self.context(owner, &value, true) {
                        self.pin(&ctx);
                    }
                },
            }
            return;
        }
        let link = Rc::new(Link { id, depth, parent: chain.clone() });
        self.push(Content::Form(id), target, painter, Some(link), need);
    }

    /// Marks the color spaces `dict` (an image, a shading or a pattern)
    /// names as used.
    fn dictionary_spaces(&mut self, dict: &Dict, ctx: &Ctx) {
        let Some(slot) = ctx.subs[COLOR_SPACE].clone() else { return };
        let mut found = HashSet::new();
        let masks = ["SMask", "Mask"].into_iter().filter_map(|key| dict.get(key));
        let masks: Vec<Object> = masks.cloned().collect();
        self.space_names(dict, &mut found);
        for mask in masks {
            if let Some(mask) = self.resolved(&mask) {
                if let Some(mask) = dict_of(&mask) {
                    self.space_names(mask, &mut found);
                }
            }
        }
        self.used.entry(slot).or_default().extend(found);
    }

    fn space_names(&mut self, dict: &Dict, found: &mut HashSet<String>) {
        match dict.get("ColorSpace") {
            Some(Object::Reference(r)) => {
                if let Some(obj) = self.object(r.id) {
                    push_names(&obj, found);
                }
            },
            Some(other) => push_names(other, found),
            None => {},
        }
    }

    /// A color space entry that is used may name another by its array
    /// (an Indexed base): the closure of the used names.
    fn close_color_spaces(&mut self) {
        let slots: Vec<Slot> = self
            .subs
            .iter()
            .filter(|(slot, sub)| sub.kind == COLOR_SPACE && !self.pinned.contains(*slot))
            .map(|(slot, _)| slot.clone())
            .collect();
        for slot in slots {
            self.close_color_space(&slot);
        }
    }

    fn close_color_space(&mut self, slot: &Slot) {
        let dict = self.subs[slot].dict.clone();
        loop {
            let used = self.used.get(slot).cloned().unwrap_or_default();
            let mut found = HashSet::new();
            for name in &used {
                match dict.get(name) {
                    Some(Object::Reference(r)) => {
                        if let Some(obj) = self.object(r.id) {
                            push_names(&obj, &mut found);
                        }
                    },
                    Some(other) => push_names(other, &mut found),
                    None => {},
                }
            }
            let added: Vec<String> =
                found.into_iter().filter(|n| dict.contains_key(n) && !used.contains(n)).collect();
            if added.is_empty() {
                break;
            }
            self.used.entry(slot.clone()).or_default().extend(added);
        }
    }

    /// Ends the walk of each resource dictionary held directly in an object
    /// that no work item reads against any more: its subdictionaries are
    /// final, so their cuts are made and what the walk knew of them is
    /// dropped. What a walk reaches through an indirect dictionary stays
    /// until the end, since other pages name against it too.
    ///
    /// With a collector, a dictionary that no work item reads against any
    /// more first has the contents its entries hold that no name reached
    /// queued for the collector, and ends once those are read.
    fn settle(&mut self) {
        self.cache.clear();
        let mut ended: Vec<Slot> = Vec::new();
        let mut unreached: Vec<Rc<Ctx>> = Vec::new();
        for slot in &self.created {
            match self.ctxs.get(slot) {
                None => ended.push(*slot),
                Some(ctx) if Rc::strong_count(ctx) == 1 => {
                    if self.glyphs.is_some()
                        && !self.unioned.contains(&(*slot, self.poisoned.contains(slot)))
                    {
                        unreached.push(ctx.clone());
                    } else {
                        ended.push(*slot);
                    }
                },
                Some(_) => {},
            }
        }
        for ctx in unreached {
            let poison = self.poisoned.contains(&ctx.slot);
            self.unioned.insert((ctx.slot, poison));
            self.union_follow(&ctx, poison);
        }
        if ended.is_empty() {
            return;
        }
        self.created.retain(|slot| !ended.contains(slot));
        for slot in ended {
            let Some(ctx) = self.ctxs.remove(&slot) else { continue };
            for sub_slot in ctx.subs.iter().flatten().filter(|sub| !sub.path.is_empty()) {
                if !self.pinned.contains(sub_slot) && self.subs[sub_slot].kind == COLOR_SPACE {
                    self.close_color_space(sub_slot);
                }
                let sub = self.subs.remove(sub_slot).expect("a context's subdictionary is known");
                let used = self.used.remove(sub_slot).unwrap_or_default();
                if !self.pinned.remove(sub_slot) {
                    self.replace(sub_slot, &sub, &used);
                }
            }
            for visit in self.local_visited.remove(&slot).unwrap_or_default() {
                self.visited.remove(&(visit, slot));
            }
            if !self.unnamed.remove(&slot) {
                self.finished.insert(slot.owner);
            }
            self.poisoned.remove(&slot);
            self.unioned.remove(&(slot, false));
            self.unioned.remove(&(slot, true));
        }
    }

    /// Reads every queued content until none is left.
    fn drain(&mut self, cancel: &CancelCheck) -> Result<()> {
        loop {
            while let Some(work) = self.work.pop() {
                cancel.check()?;
                self.process(work, cancel)?;
                self.settle();
            }
            self.settle();
            if self.work.is_empty() {
                return Ok(());
            }
        }
    }

    /// With a collector, queues for it, in every dictionary that has not
    /// been, the contents its entries hold that no name reached, and reads
    /// them: the dictionaries held by reference live until here.
    fn finish_unnamed(&mut self, cancel: &CancelCheck) -> Result<()> {
        if self.glyphs.is_none() {
            return Ok(());
        }
        loop {
            let pending: Vec<Rc<Ctx>> = self
                .ctxs
                .values()
                .filter(|ctx| !self.unioned.contains(&(ctx.slot, self.poisoned.contains(&ctx.slot))))
                .cloned()
                .collect();
            if pending.is_empty() {
                return Ok(());
            }
            for ctx in pending {
                cancel.check()?;
                let poison = self.poisoned.contains(&ctx.slot);
                self.unioned.insert((ctx.slot, poison));
                self.union_follow(&ctx, poison);
            }
            self.drain(cancel)?;
        }
    }

    /// The entries of subdictionary `kind` of `ctx`, in the order of their names.
    fn sub_values(&self, ctx: &Ctx, kind: usize) -> Vec<Object> {
        let Some(sub) = ctx.subs[kind].as_ref().and_then(|slot| self.subs.get(slot)) else {
            return Vec::new();
        };
        let mut names: Vec<&String> = sub.dict.keys().collect();
        names.sort();
        names.into_iter().filter_map(|name| sub.dict.get(name).cloned()).collect()
    }

    /// Queues for the collector, and only for it, the contents `ctx` holds
    /// that a viewer may paint: every form, pattern, soft-mask group and Type 3
    /// glyph procedure its entries name, whether or not a content names the
    /// entry. A content some name reached has been read, and is not read again.
    fn union_follow(&mut self, ctx: &Rc<Ctx>, poison: bool) {
        if self.glyphs.as_ref().map_or(true, |glyphs| glyphs.is_disabled()) {
            return;
        }
        let need = self.need(false, poison);
        for value in self.sub_values(ctx, XOBJECT) {
            let Object::Reference(r) = value else { continue };
            let Some(Object::Stream { dict, .. }) = self.read(r.id) else { continue };
            if !matches!(dict.get("Subtype").and_then(|s| s.as_name()), Some("Image" | "PS")) {
                self.paint(r.id, &dict, ctx, &None, need);
            }
        }
        for value in self.sub_values(ctx, PATTERN) {
            let Object::Reference(r) = value else { continue };
            if let Some(Object::Stream { dict, .. }) = self.read(r.id) {
                self.paint(r.id, &dict, ctx, &None, need);
            }
        }
        for value in self.sub_values(ctx, EXT_G_STATE) {
            let Some(state) = self.resolved(&value) else { continue };
            let Some(mask) = dict_of(&state).and_then(|d| d.get("SMask")) else { continue };
            let Some(mask) = self.resolved(mask) else { continue };
            let Some(Object::Reference(group)) = dict_of(&mask).and_then(|d| d.get("G")) else {
                continue;
            };
            if let Some(Object::Stream { dict, .. }) = self.read(group.id) {
                self.paint(group.id, &dict, ctx, &None, need);
            }
        }
        for value in self.sub_values(ctx, FONT) {
            let Object::Reference(r) = value else { continue };
            let Some(font) = self.object(r.id) else { continue };
            let Some(font) = dict_of(&font) else { continue };
            if font.get("Subtype").and_then(|s| s.as_name()) != Some("Type3") {
                continue;
            }
            let child = match font.get("Resources") {
                Some(value) if self.is_done(r.id, value, poison) => continue,
                Some(value) => self.context(r.id, value, false).unwrap_or_else(|| ctx.clone()),
                None => ctx.clone(),
            };
            let Some(procs) = font.get("CharProcs").and_then(|v| self.resolved(v)) else {
                continue;
            };
            let Object::Dictionary(procs) = &*procs else { continue };
            let mut ids: Vec<u32> =
                procs.values().filter_map(|v| v.as_reference()).map(|r| r.id).collect();
            ids.sort_unstable();
            if poison {
                self.poison_ctx(&child);
            }
            for id in ids {
                self.enter(id, Target::Ready(child.clone()), Some(ctx.clone()), &None, need);
            }
        }
    }

    /// Whether the save writes object `id` of a document that has no writers
    /// list: it is not dropped, and not a free entry.
    fn is_written(&self, id: u32) -> bool {
        id != 0
            && !self.excluded.contains(&id)
            && (self.replaced(id).is_some() || !self.doc.is_free_entry(id))
    }

    /// Keeps whole the subdictionaries some object the walk did not reach
    /// also points at: the object may resolve names against them.
    ///
    /// This is the one pass over every object of the document, so it is also
    /// where the collector is offered each object, once the walk is done:
    /// the objects it reads are in the source's cache for the references.
    fn pin_shared(&mut self, cancel: &CancelCheck) -> Result<()> {
        let scanning = self.glyphs.as_ref().is_some_and(|glyphs| glyphs.wants_scan());
        if self.seen.is_empty() && !scanning {
            return Ok(());
        }
        let ids = match &self.writers {
            Some(writers) => writers.clone(),
            None => {
                let mut ids: Vec<u32> = self.doc.all_object_ids();
                ids.extend(self.staged.keys().copied());
                ids.extend(self.overrides.keys().copied());
                ids.sort_unstable();
                ids.dedup();
                if !scanning {
                    ids.retain(|&id| self.is_written(id));
                }
                ids
            },
        };
        let mut stray: HashSet<u32> = HashSet::new();
        let mut refs: Vec<u32> = Vec::new();
        for id in ids {
            cancel.check()?;
            if scanning {
                if let Some(glyphs) = self.glyphs.as_mut() {
                    glyphs.scan_object(id);
                }
                if self.seen.is_empty() || (self.writers.is_none() && !self.is_written(id)) {
                    continue;
                }
            }
            refs.clear();
            self.refs_of(id, &mut refs);
            for target in &refs {
                if self.seen.get(target).is_some_and(|holders| !holders.contains(&id)) {
                    stray.insert(*target);
                }
            }
        }
        if scanning {
            if let Some(glyphs) = self.glyphs.as_mut() {
                glyphs.scanned();
            }
        }
        for (slot, sub) in &self.subs {
            let held = sub.holders.iter().any(|h| h.path.is_empty() && stray.contains(&h.owner));
            if held || (slot.path.is_empty() && stray.contains(&slot.owner)) {
                self.pinned.insert(slot.clone());
            }
        }
        Ok(())
    }

    /// Cuts down the subdictionaries the walk kept for the end: each one
    /// that is not pinned, without the entries nothing names.
    fn cut_shared(&mut self) {
        let subs = std::mem::take(&mut self.subs);
        let used = std::mem::take(&mut self.used);
        for (slot, sub) in &subs {
            if self.pinned.contains(slot) {
                continue;
            }
            let Some(used) = used.get(slot) else { continue };
            self.replace(slot, sub, used);
        }
    }

    /// Records the cut of subdictionary `sub` at `slot` down to the entries
    /// named in `used`.
    fn replace(&mut self, slot: &Slot, sub: &Sub, used: &HashSet<String>) {
        if !self.is_named_sub(sub) {
            return;
        }
        let mut keep: Vec<String> =
            sub.dict.keys().filter(|name| used.contains(*name)).cloned().collect();
        if keep.len() == sub.dict.len() {
            return;
        }
        if keep.is_empty() {
            for holder in &sub.holders {
                let path = holder.path.then(sub.kind);
                self.cuts.entry(holder.owner).or_default().push(Cut::Remove { path });
            }
        } else {
            keep.sort_unstable();
            let keep: Box<[Box<str>]> = keep.into_iter().map(String::into_boxed_str).collect();
            self.cuts.entry(slot.owner).or_default().push(Cut::Keep { path: slot.path, keep });
        }
    }
}

/// The slot of the resource dictionary `value`, the `/Resources` entry of
/// `owner`.
fn slot_of(owner: u32, value: &Object) -> Option<Slot> {
    match value {
        Object::Reference(r) => Some(Slot { owner: r.id, path: Path::EMPTY }),
        Object::Dictionary(_) => Some(Slot { owner, path: Path::resources() }),
        _ => None,
    }
}

/// Adds the names `op` used to `names`, by the kind of resource they name.
fn record(op: &Operator, consumed: &[&str], names: &mut Names) {
    let kind = match op {
        Operator::Tf { .. } => FONT,
        Operator::Do { .. } => XOBJECT,
        Operator::SetFillColorSpace { .. }
        | Operator::SetStrokeColorSpace { .. }
        | Operator::InlineImage { .. } => COLOR_SPACE,
        Operator::SetFillColorN { .. } | Operator::SetStrokeColorN { .. } => PATTERN,
        Operator::SetExtGState { .. } => EXT_G_STATE,
        Operator::PaintShading { .. } => SHADING,
        Operator::BeginMarkedContentDict { .. } => PROPERTIES,
        Operator::Other { name, .. } if name == "DP" => PROPERTIES,
        _ => return,
    };
    names.sets[kind].extend(consumed.iter().map(|name| (*name).to_string()));
}

fn dict_of(obj: &Object) -> Option<&Dict> {
    match obj {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

/// Every name in `obj` and the arrays inside it.
fn push_names(obj: &Object, out: &mut HashSet<String>) {
    match obj {
        Object::Name(name) => {
            out.insert(name.clone());
        },
        Object::Array(items) => items.iter().for_each(|item| push_names(item, out)),
        _ => {},
    }
}

fn push_refs(obj: &Object, out: &mut Vec<u32>) {
    match obj {
        Object::Reference(r) => out.push(r.id),
        Object::Array(items) => items.iter().for_each(|item| push_refs(item, out)),
        Object::Dictionary(dict) => dict.values().for_each(|item| push_refs(item, out)),
        Object::Stream { dict, .. } => dict.values().for_each(|item| push_refs(item, out)),
        _ => {},
    }
}
