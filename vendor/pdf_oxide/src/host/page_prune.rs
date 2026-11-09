//! What a full rewrite drops with the pages it drops (#261).
//!
//! `write_full_to_writer` writes the kept pages as one flat `/Kids` list
//! under the root `/Pages` node, then sweeps every object it can reach
//! from the catalog. Anything that still points at a dropped page keeps
//! that page, its content streams and its images in the output: a kept
//! page's `/Parent` (the old intermediate `/Pages` node lists every
//! sibling), named destinations, outline items, link annotations, the
//! `/OpenAction` and the AcroForm's widgets.
//!
//! [`plan`] answers four things for one save:
//! - `kept_pages`: the kept source pages, the kids of the output's root
//!   `/Pages` node. The reachability walk starts from them, because the
//!   source root points at nodes the save excludes.
//! - `excluded`: object ids the save never writes and the reachability
//!   walk never enters: dropped page leaves, every intermediate `/Pages`
//!   node, and annotations only dropped pages list. A reference that is
//!   left pointing at one of them names a missing object, which a reader
//!   takes as null (ISO 32000-1 §7.3.10). A save without garbage
//!   collection also skips everything only these objects reach.
//! - `overrides`: replacement objects the save writes instead of the
//!   staged or source object: each kept page re-parented to the root with
//!   the attributes it inherited from a dropped node copied onto it, and
//!   the destination, outline, link and form objects with their entries
//!   for dropped pages removed.
//! - `catalog_edits`: the same removals for entries held directly in the
//!   catalog, applied by [`PagePrune::tidy_catalog`].

use std::collections::{HashMap, HashSet};

use crate::document::PdfDocument;
use crate::editor::{PageKey, PageSlot};
use crate::host::resource_prune::Cuts;
use crate::object::{Object, ObjectRef};

type Dict = HashMap<String, Object>;

/// Page attributes a page inherits from its ancestors (ISO 32000-1 §7.7.3.4).
const INHERITABLE: [&str; 4] = ["Resources", "MediaBox", "CropBox", "Rotate"];

/// The plan for one full rewrite. See the module docs.
#[derive(Default)]
pub(crate) struct PagePrune {
    pub excluded: HashSet<u32>,
    // ── pdf_manipulator patch: the image sources a redaction replaced ──
    /// The source image XObjects destructive redaction overwrote or dropped:
    /// an original is written only when something else still references it,
    /// with garbage collection on and off (the reachability walk's answer,
    /// which garbage collection on already applies and garbage collection
    /// off applies by name here).
    pub redacted_image_originals: HashSet<u32>,
    // ── end pdf_manipulator patch ──
    /// The kept source pages: the kids of the root `/Pages` node in the
    /// output. The reachability walk enters them directly, since the
    /// source root lists the intermediate nodes the save excludes.
    pub kept_pages: Vec<u32>,
    pub overrides: HashMap<u32, Object>,
    /// What the resource prune cuts out of objects, by id: applied to the
    /// object the save writes (the override, else the staged object, else
    /// the source's) with [`PagePrune::cut`]. No object body is held.
    pub cuts: HashMap<u32, Cuts>,
    catalog_edits: Vec<(String, Object, Option<Object>)>,
}

impl PagePrune {
    /// `object`, the object the save writes for `id`, with its cuts applied.
    pub fn cut(&self, id: u32, object: Object) -> Object {
        match self.cuts.get(&id) {
            Some(cuts) => crate::host::resource_prune::apply(object, cuts),
            None => object,
        }
    }

    /// Applies the edits to catalog entries held directly in the catalog.
    /// An entry the save has already rebuilt (it no longer equals the
    /// value the plan read) is left to its rebuilder.
    pub fn tidy_catalog(&self, catalog: &mut Dict) {
        for (key, before, after) in &self.catalog_edits {
            if catalog.get(key) != Some(before) {
                continue;
            }
            match after {
                Some(v) => {
                    catalog.insert(key.clone(), v.clone());
                },
                None => {
                    catalog.remove(key);
                },
            }
        }
    }
}

/// Plans the save of `doc` with the edits in `staged`, the pages in
/// `page_order` and the annotations a redaction drops. A source page missing
/// from `page_order` is dropped; a merged page is never part of the plan,
/// since it is not a page of `doc`. A dropped annotation that is a widget
/// leaves the field tree with its field, as a widget of a dropped page does.
pub(crate) fn plan(
    doc: &PdfDocument,
    staged: &HashMap<u32, Object>,
    page_order: &[PageSlot],
    dropped_annots: &HashSet<u32>,
    // ── pdf_manipulator patch: the image sources destructive redaction replaced ──
    redacted_image_originals: &HashSet<u32>,
    // ── end pdf_manipulator patch ──
) -> PagePrune {
    plan_cancellable(
        doc,
        staged,
        page_order,
        dropped_annots,
        redacted_image_originals,
        &crate::cancel::CancelCheck::none(),
    )
    .expect("a plan that cannot be cancelled succeeds")
}

/// [`plan`] that stops with the cancelled error once `cancel` reports
/// cancelled, polled while the page tree is walked and before each kept page
/// is reparented. The plan reads and changes nothing but itself.
pub(crate) fn plan_cancellable(
    doc: &PdfDocument,
    staged: &HashMap<u32, Object>,
    page_order: &[PageSlot],
    dropped_annots: &HashSet<u32>,
    redacted_image_originals: &HashSet<u32>,
    cancel: &crate::cancel::CancelCheck,
) -> crate::error::Result<PagePrune> {
    let mut p = Planner {
        cancel,
        doc,
        staged,
        dropped_annots,
        out: PagePrune::default(),
        dropped_pages: HashSet::new(),
        removed_names: HashSet::new(),
    };
    // ── pdf_manipulator patch: the image sources destructive redaction replaced ──
    p.out.redacted_image_originals = redacted_image_originals.clone();
    // ── end pdf_manipulator patch ──
    p.run(page_order);
    cancel.check()?;
    Ok(p.out)
}

struct Planner<'a> {
    cancel: &'a crate::cancel::CancelCheck,
    doc: &'a PdfDocument,
    staged: &'a HashMap<u32, Object>,
    dropped_annots: &'a HashSet<u32>,
    out: PagePrune,
    dropped_pages: HashSet<u32>,
    /// Named destinations removed because they targeted a dropped page.
    removed_names: HashSet<Vec<u8>>,
}

impl Planner<'_> {
    /// The object the save would write for `r` before this plan: an
    /// override, then a staged edit, then the source.
    fn load(&self, r: ObjectRef) -> Option<Object> {
        if let Some(o) = self.out.overrides.get(&r.id) {
            return Some(o.clone());
        }
        if let Some(o) = self.staged.get(&r.id) {
            return Some(o.clone());
        }
        self.doc.load_object(r).ok()
    }

    fn resolve(&self, o: &Object) -> Option<Object> {
        match o {
            Object::Reference(r) => self.load(*r),
            other => Some(other.clone()),
        }
    }

    fn run(&mut self, page_order: &[PageSlot]) {
        let Some(catalog_ref) = self
            .doc
            .trailer()
            .as_dict()
            .and_then(|d| d.get("Root"))
            .and_then(|r| r.as_reference())
        else {
            return;
        };
        let Some(catalog) = self.load(catalog_ref).and_then(|c| c.as_dict().cloned()) else {
            return;
        };
        let Some(root) = catalog.get("Pages").and_then(|p| p.as_reference()) else {
            return;
        };
        let leaves = self.doc.all_page_refs_cancellable(self.cancel).unwrap_or_default();
        if self.cancel.is_cancelled() {
            return;
        }
        let kept_idx: Vec<usize> = page_order
            .iter()
            .filter_map(|slot| match slot.key() {
                PageKey::Source(i) if i < leaves.len() => Some(i),
                _ => None,
            })
            .collect();
        let kept: Vec<ObjectRef> = kept_idx.iter().map(|&i| leaves[i]).collect();
        let kept_ids: HashSet<u32> = kept.iter().map(|r| r.id).collect();
        self.out.kept_pages = kept.iter().map(|r| r.id).collect();
        self.dropped_pages = leaves
            .iter()
            .map(|r| r.id)
            .filter(|id| !kept_ids.contains(id))
            .collect();

        let inner = self.inner_nodes(root);
        self.out.excluded.extend(inner.iter().copied());
        self.out.excluded.extend(self.dropped_pages.iter().copied());
        self.out.excluded.extend(self.dropped_annots.iter().copied());

        let mut seen = HashSet::new();
        for r in &kept {
            if self.cancel.is_cancelled() {
                return;
            }
            if seen.insert(r.id) {
                self.reparent(*r, root, &inner);
            }
        }

        self.renumber_page_labels(&catalog, page_order);

        // Nothing to prune: the tree was flat, every page is kept and no
        // annotation is dropped.
        if self.dropped_pages.is_empty() && self.dropped_annots.is_empty() {
            return;
        }
        self.exclude_dropped_annotations(&kept, &leaves);
        // A named destination may name another one, so removing one can
        // make another dead: prune until a pass removes no name.
        let mut view = catalog.clone();
        loop {
            let removed = self.removed_names.len();
            self.prune_named_dests(&view);
            for (key, _, after) in &self.out.catalog_edits {
                match after {
                    Some(v) => {
                        view.insert(key.clone(), v.clone());
                    },
                    None => {
                        view.remove(key);
                    },
                }
            }
            if self.removed_names.len() == removed {
                break;
            }
        }
        self.prune_outlines(&catalog);
        self.prune_links(&kept);
        self.prune_open_action(&catalog);
        self.prune_acroform(&catalog);
    }

    /// Rebuilds `/PageLabels` for the kept pages in their new order
    /// (ISO 32000-1 §12.4.2). The number tree is keyed by page index, so
    /// after a drop or a reorder every range after the change labels the
    /// wrong pages. Each kept page keeps the label it had: a new range
    /// starts wherever the old range or the run of consecutive pages
    /// breaks, with `/St` moved on by the pages skipped.
    ///
    /// A merged page has no entry of its own: it takes the label that runs
    /// on from the page before it, and the source page after it starts a
    /// new range.
    fn renumber_page_labels(&mut self, catalog: &Dict, page_order: &[PageSlot]) {
        let Some(labels) = catalog.get("PageLabels").cloned() else {
            return;
        };
        let in_place = |(j, slot): (usize, &PageSlot)| match slot.key() {
            PageKey::Source(i) => i == j,
            PageKey::Merged(_) => true,
        };
        if page_order.iter().enumerate().all(in_place) {
            return;
        }
        let mut ranges: Vec<(usize, Dict)> = Vec::new();
        self.collect_number_tree(&labels, &mut ranges, &mut HashSet::new());
        ranges.sort_by_key(|(start, _)| *start);

        let mut nums = Vec::new();
        let mut prev: Option<(Option<usize>, usize)> = None;
        for (j, slot) in page_order.iter().enumerate() {
            if self.cancel.is_cancelled() {
                return;
            }
            let PageKey::Source(i) = slot.key() else {
                prev = None;
                continue;
            };
            let range = ranges.iter().rposition(|(start, _)| *start <= i);
            if prev == Some((range, i.wrapping_sub(1))) {
                prev = Some((range, i));
                continue;
            }
            prev = Some((range, i));
            let dict = match range {
                Some(k) => {
                    let (start, ref d) = ranges[k];
                    let mut d = d.clone();
                    let st = d.get("St").and_then(|s| s.as_integer()).unwrap_or(1);
                    d.insert("St".into(), Object::Integer(st + (i - start) as i64));
                    d
                },
                // A page before the first range: decimal, its own number.
                None => HashMap::from([
                    ("S".to_string(), Object::Name("D".into())),
                    ("St".to_string(), Object::Integer(i as i64 + 1)),
                ]),
            };
            nums.push(Object::Integer(j as i64));
            nums.push(Object::Dictionary(dict));
        }
        let tree = Object::Dictionary(HashMap::from([("Nums".to_string(), Object::Array(nums))]));
        let new = self.edit(&labels, |_, _| Some(tree));
        if new.is_some() {
            self.catalog_edit(catalog, "PageLabels", new);
        }
    }

    /// The `(page index, label dictionary)` pairs of a number tree.
    fn collect_number_tree(
        &self,
        node: &Object,
        out: &mut Vec<(usize, Dict)>,
        visited: &mut HashSet<u32>,
    ) {
        if let Object::Reference(r) = node {
            if !visited.insert(r.id) {
                return;
            }
        }
        let Some(Object::Dictionary(d)) = self.resolve(node) else {
            return;
        };
        if let Some(Object::Array(pairs)) = d.get("Nums").and_then(|n| self.resolve(n)) {
            for pair in pairs.chunks(2) {
                if let [key, value] = pair {
                    if let (Some(k), Some(Object::Dictionary(v))) =
                        (key.as_integer(), self.resolve(value))
                    {
                        if k >= 0 {
                            out.push((k as usize, v));
                        }
                    }
                }
            }
        }
        if let Some(Object::Array(kids)) = d.get("Kids").and_then(|k| self.resolve(k)) {
            for kid in &kids {
                self.collect_number_tree(kid, out, visited);
            }
        }
    }

    /// Every `/Pages` node under `root`, not `root` itself.
    fn inner_nodes(&self, root: ObjectRef) -> HashSet<u32> {
        let mut inner = HashSet::new();
        let mut visited = HashSet::new();
        let mut stack = vec![root];
        while let Some(r) = stack.pop() {
            if !visited.insert(r.id) {
                continue;
            }
            let Some(node) = self.load(r) else { continue };
            let Some(kids) = node
                .as_dict()
                .and_then(|d| d.get("Kids"))
                .and_then(|k| self.resolve(k))
            else {
                continue;
            };
            if r.id != root.id {
                inner.insert(r.id);
            }
            if let Some(kids) = kids.as_array() {
                stack.extend(kids.iter().filter_map(|k| k.as_reference()));
            }
        }
        inner
    }

    /// Points a kept page's `/Parent` at the root, first copying onto it
    /// each inheritable attribute it only had through an inner node.
    fn reparent(&mut self, page: ObjectRef, root: ObjectRef, inner: &HashSet<u32>) {
        // A malformed tree whose root is itself the only page has no parent to point at.
        if page.id == root.id {
            return;
        }
        let Some(Object::Dictionary(mut dict)) = self.load(page) else {
            return;
        };
        let parent = dict.get("Parent").and_then(|p| p.as_reference());
        if parent.map(|p| p.id) == Some(root.id) {
            return;
        }
        let mut node = parent;
        let mut guard = 0;
        while let Some(r) = node {
            guard += 1;
            if guard > 64 || !inner.contains(&r.id) {
                break;
            }
            let Some(Object::Dictionary(anc)) = self.load(r) else {
                break;
            };
            for key in INHERITABLE {
                if !dict.contains_key(key) {
                    if let Some(v) = anc.get(key) {
                        dict.insert(key.to_string(), v.clone());
                    }
                }
            }
            node = anc.get("Parent").and_then(|p| p.as_reference());
        }
        dict.insert("Parent".to_string(), Object::Reference(root));
        self.out.overrides.insert(page.id, Object::Dictionary(dict));
    }

    fn annot_refs(&self, page: ObjectRef) -> Vec<ObjectRef> {
        self.load(page)
            .and_then(|p| {
                p.as_dict()
                    .and_then(|d| d.get("Annots"))
                    .and_then(|a| self.resolve(a))
            })
            .and_then(|a| {
                a.as_array()
                    .map(|a| a.iter().filter_map(|x| x.as_reference()).collect())
            })
            .unwrap_or_default()
    }

    /// Excludes the annotations only dropped pages list; one a kept page
    /// also lists stays.
    fn exclude_dropped_annotations(&mut self, kept: &[ObjectRef], leaves: &[ObjectRef]) {
        let on_kept: HashSet<u32> = kept
            .iter()
            .flat_map(|r| self.annot_refs(*r))
            .map(|r| r.id)
            .collect();
        let dropped: Vec<ObjectRef> = leaves
            .iter()
            .filter(|r| self.dropped_pages.contains(&r.id))
            .copied()
            .collect();
        for page in dropped {
            for a in self.annot_refs(page) {
                if self.cancel.is_cancelled() {
                    return;
                }
                if !on_kept.contains(&a.id) {
                    self.out.excluded.insert(a.id);
                }
            }
        }
    }

    /// True when `dest` (an explicit destination, a `/D` dictionary or a
    /// name) goes to a dropped page.
    fn dest_is_dead(&self, dest: &Object) -> bool {
        match self.resolve(dest) {
            Some(Object::Array(a)) => a
                .first()
                .and_then(|p| p.as_reference())
                .is_some_and(|p| self.dropped_pages.contains(&p.id)),
            Some(Object::Dictionary(d)) => d.get("D").is_some_and(|x| self.dest_is_dead(x)),
            Some(Object::Name(n)) => self.removed_names.contains(n.as_bytes()),
            Some(Object::String(s)) => self.removed_names.contains(&s),
            _ => false,
        }
    }

    /// True when `action` is a GoTo to a dropped page.
    fn action_is_dead(&self, action: &Object) -> bool {
        let Some(Object::Dictionary(a)) = self.resolve(action) else {
            return false;
        };
        a.get("S").and_then(|s| s.as_name()) == Some("GoTo")
            && a.get("D").is_some_and(|d| self.dest_is_dead(d))
    }

    /// Replaces `value` by `f` of it: an indirect one through an override,
    /// a direct one in the returned value. `None` means no change.
    fn edit(
        &mut self,
        value: &Object,
        f: impl FnOnce(&mut Self, Object) -> Option<Object>,
    ) -> Option<Object> {
        match value {
            Object::Reference(r) => {
                let current = self.load(*r)?;
                if let Some(new) = f(self, current) {
                    self.out.overrides.insert(r.id, new);
                }
                None
            },
            direct => f(self, direct.clone()),
        }
    }

    /// Records an edit of a catalog entry. A later edit of the same entry
    /// replaces the value to write and keeps the value first read.
    fn catalog_edit(&mut self, catalog: &Dict, key: &str, after: Option<Object>) {
        if let Some(edit) = self.out.catalog_edits.iter_mut().find(|(k, _, _)| k == key) {
            edit.2 = after;
            return;
        }
        if let Some(before) = catalog.get(key) {
            self.out
                .catalog_edits
                .push((key.to_string(), before.clone(), after));
        }
    }

    /// Removes the named destinations that go to a dropped page, from the
    /// catalog's `/Dests` dictionary and the `/Names /Dests` name tree.
    fn prune_named_dests(&mut self, catalog: &Dict) {
        if let Some(dests) = catalog.get("Dests") {
            // Some writers put a name tree here instead of the PDF 1.1
            // name-to-destination dictionary; prune it as the tree it is.
            let tree_shaped = self
                .resolve(dests)
                .and_then(|d| {
                    d.as_dict()
                        .map(|d| d.contains_key("Names") || d.contains_key("Kids"))
                })
                .unwrap_or(false);
            let new = if tree_shaped {
                self.prune_name_tree(dests, &mut HashSet::new())
            } else {
                self.edit(dests, |p, obj| {
                    let Object::Dictionary(mut d) = obj else {
                        return None;
                    };
                    let dead: Vec<String> = d
                        .iter()
                        .filter(|(_, v)| p.dest_is_dead(v))
                        .map(|(k, _)| k.clone())
                        .collect();
                    if dead.is_empty() {
                        return None;
                    }
                    for k in dead {
                        d.remove(&k);
                        p.removed_names.insert(k.into_bytes());
                    }
                    Some(Object::Dictionary(d))
                })
            };
            if new.is_some() {
                self.catalog_edit(catalog, "Dests", new);
            }
        }
        let Some(names) = catalog.get("Names").cloned() else {
            return;
        };
        let new_names = self.edit(&names, |p, obj| {
            let Object::Dictionary(mut d) = obj else {
                return None;
            };
            let tree = d.get("Dests")?.clone();
            let new_tree = p.prune_name_tree(&tree, &mut HashSet::new())?;
            d.insert("Dests".to_string(), new_tree);
            Some(Object::Dictionary(d))
        });
        if new_names.is_some() {
            self.catalog_edit(catalog, "Names", new_names);
        }
    }

    /// Prunes one name-tree node and its kids. Returns the direct node's
    /// replacement; an indirect node is replaced through an override.
    fn prune_name_tree(&mut self, node: &Object, visited: &mut HashSet<u32>) -> Option<Object> {
        if let Object::Reference(r) = node {
            if !visited.insert(r.id) {
                return None;
            }
        }
        self.edit(node, |p, obj| {
            let Object::Dictionary(mut d) = obj else {
                return None;
            };
            let mut changed = false;
            if let Some(kids) = d
                .get("Kids")
                .and_then(|k| p.resolve(k))
                .and_then(|k| k.as_array().cloned())
            {
                for kid in &kids {
                    // Kids are indirect (ISO 32000-1 §7.9.6), so each prunes
                    // through its own override.
                    p.prune_name_tree(kid, visited);
                }
            }
            if let Some(Object::Array(pairs)) = d.get("Names").and_then(|n| p.resolve(n)) {
                let mut kept = Vec::with_capacity(pairs.len());
                for pair in pairs.chunks(2) {
                    let [key, value] = pair else { continue };
                    if p.dest_is_dead(value) {
                        if let Some(k) = key.as_string() {
                            p.removed_names.insert(k.to_vec());
                        }
                        changed = true;
                    } else {
                        kept.extend_from_slice(pair);
                    }
                }
                if changed {
                    match (kept.first().cloned(), kept.get(kept.len().saturating_sub(2)).cloned()) {
                        (Some(first), Some(last)) if d.contains_key("Limits") => {
                            d.insert("Limits".to_string(), Object::Array(vec![first, last]));
                        },
                        _ => {
                            d.remove("Limits");
                        },
                    }
                    d.insert("Names".to_string(), Object::Array(kept));
                }
            }
            changed.then_some(Object::Dictionary(d))
        })
    }

    /// Strips the destination and the GoTo action from every outline
    /// item that goes to a dropped page. The item and its children stay.
    fn prune_outlines(&mut self, catalog: &Dict) {
        let Some(outlines) = catalog.get("Outlines").and_then(|o| self.resolve(o)) else {
            return;
        };
        let mut stack: Vec<ObjectRef> = outlines
            .as_dict()
            .and_then(|d| d.get("First"))
            .and_then(|f| f.as_reference())
            .into_iter()
            .collect();
        let mut visited = HashSet::new();
        while let Some(r) = stack.pop() {
            if !visited.insert(r.id) {
                continue;
            }
            let Some(Object::Dictionary(mut item)) = self.load(r) else {
                continue;
            };
            for key in ["First", "Next"] {
                if let Some(n) = item.get(key).and_then(|n| n.as_reference()) {
                    stack.push(n);
                }
            }
            if self.strip_dead_links(&mut item) {
                self.out.overrides.insert(r.id, Object::Dictionary(item));
            }
        }
    }

    /// Removes `/Dest` and a GoTo `/A` that go to a dropped page.
    fn strip_dead_links(&self, d: &mut Dict) -> bool {
        let dest = d.get("Dest").is_some_and(|x| self.dest_is_dead(x));
        let action = d.get("A").is_some_and(|x| self.action_is_dead(x));
        if dest {
            d.remove("Dest");
        }
        if action {
            d.remove("A");
        }
        dest || action
    }

    /// Strips the dead destination from every link annotation on a kept page.
    fn prune_links(&mut self, kept: &[ObjectRef]) {
        let annots: HashSet<ObjectRef> = kept.iter().flat_map(|r| self.annot_refs(*r)).collect();
        for a in annots {
            if self.cancel.is_cancelled() {
                return;
            }
            let Some(Object::Dictionary(mut d)) = self.load(a) else {
                continue;
            };
            if d.get("Subtype").and_then(|s| s.as_name()) == Some("Link")
                && self.strip_dead_links(&mut d)
            {
                self.out.overrides.insert(a.id, Object::Dictionary(d));
            }
        }
    }

    fn prune_open_action(&mut self, catalog: &Dict) {
        let Some(open) = catalog.get("OpenAction") else {
            return;
        };
        let dead = match self.resolve(open) {
            Some(Object::Array(_)) => self.dest_is_dead(open),
            Some(Object::Dictionary(_)) => self.action_is_dead(open),
            _ => false,
        };
        if dead {
            self.catalog_edit(catalog, "OpenAction", None);
        }
    }

    /// True when a field-tree entry is a widget of a dropped page.
    fn is_dropped_widget(&self, r: ObjectRef) -> bool {
        if self.out.excluded.contains(&r.id) {
            return true;
        }
        self.load(r)
            .and_then(|o| {
                o.as_dict()
                    .and_then(|d| d.get("P"))
                    .and_then(|p| p.as_reference())
            })
            .is_some_and(|p| self.dropped_pages.contains(&p.id))
    }

    /// Removes the dropped pages' widgets from the AcroForm's field tree,
    /// and every field left with no widget at all.
    fn prune_acroform(&mut self, catalog: &Dict) {
        let Some(acroform) = catalog.get("AcroForm").cloned() else {
            return;
        };
        let new = self.edit(&acroform, |p, obj| {
            let Object::Dictionary(mut form) = obj else {
                return None;
            };
            let fields = form.get("Fields")?.clone();
            let mut visited = HashSet::new();
            let new_fields = p.edit(&fields, |p, arr| {
                let Object::Array(list) = arr else {
                    return None;
                };
                let (kept, changed) = p.prune_field_list(&list, &mut visited);
                changed.then_some(Object::Array(kept))
            });
            let new_fields = new_fields?;
            form.insert("Fields".to_string(), new_fields);
            Some(Object::Dictionary(form))
        });
        if new.is_some() {
            self.catalog_edit(catalog, "AcroForm", new);
        }
    }

    /// Returns the list without the dropped widgets and the fields left
    /// empty, and whether anything was removed.
    fn prune_field_list(
        &mut self,
        list: &[Object],
        visited: &mut HashSet<u32>,
    ) -> (Vec<Object>, bool) {
        let mut kept = Vec::with_capacity(list.len());
        let mut changed = false;
        for entry in list {
            let Some(r) = entry.as_reference() else {
                kept.push(entry.clone());
                continue;
            };
            if !visited.insert(r.id) {
                kept.push(entry.clone());
                continue;
            }
            if self.is_dropped_widget(r) {
                changed = true;
                continue;
            }
            let Some(Object::Dictionary(mut field)) = self.load(r) else {
                kept.push(entry.clone());
                continue;
            };
            let Some(Object::Array(kids)) = field.get("Kids").and_then(|k| self.resolve(k)) else {
                kept.push(entry.clone());
                continue;
            };
            let (new_kids, kids_changed) = self.prune_field_list(&kids, visited);
            if kids_changed {
                changed = true;
                if new_kids.is_empty() {
                    continue;
                }
                field.insert("Kids".to_string(), Object::Array(new_kids));
                self.out.overrides.insert(r.id, Object::Dictionary(field));
            }
            kept.push(entry.clone());
        }
        (kept, changed)
    }
}
