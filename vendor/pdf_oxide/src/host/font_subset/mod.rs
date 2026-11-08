//! Embedded font programs cut down to the glyphs the document shows, with
//! every glyph id kept.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A full rewrite that prunes resources starts a [`Collector`] before the
//! unused-resource walk (`host::resource_prune`) and hands it to that walk.
//! The walk reads every content of the kept pages once (the page, the forms,
//! patterns, soft-mask groups and Type 3 glyph procedures its resources hold,
//! and the appearance streams of its annotations), and passes each operator to
//! [`Collector::op`], which collects for each font program the glyph ids its
//! fonts show. No content is opened for the font cut on its own. After the
//! save has settled its aliases, [`Collector::finish`] answers with the
//! replacement stream of each program it could shrink; the save writes those
//! in place of the source's.
//!
//! Kinds of font that are subset: a simple TrueType font (`/FontFile2`), a
//! CIDFontType2 under `Identity-H`/`Identity-V` (`/FontFile2`) and a
//! CIDFontType0 under the same (`/FontFile3`, `/CIDFontType0C`). A font
//! program is left whole when:
//! - any font that uses it is of another kind, has a CMap other than
//!   `Identity-H`/`Identity-V`, or cannot be read;
//! - a form field can use it: it is in an AcroForm or field `/DR`, a widget's
//!   appearance resources hold it, or a page the save edits holds it;
//! - an `ExtGState` `/Font` names it, or a resource dictionary no walk reached
//!   holds it;
//! - it has a `GSUB` table, since a shaper may then substitute a glyph no
//!   code names;
//! - the program is shared with a merged source's object;
//! - the subset, deflated, is not smaller than the program as the save would
//!   store it without the plan.
//!
//! The whole plan is dropped when a content shows text with no font set (the
//! font is then the painter's), names a font its resources do not hold, or
//! cannot be read to its end.
//!
//! The glyphs a code shows are every glyph any mapping of the code can
//! reach: the CID-to-GID of the font, the font's own `cmap` for the code, for
//! the code in the symbol range, and for each character the code decodes to,
//! and the glyph of a `/Differences` name. That is more than one renderer
//! paints and exactly what any of them may.
//!
//! The memory of a plan is the glyph sets, the fonts met, the ids of the
//! font maps a walk reached, and one font program at a time: a content is
//! never held.

pub(crate) mod cff;
pub(crate) mod truetype;

use crate::cancel::CancelCheck;
use crate::content::operators::{Operator, TextElement};
use crate::document::PdfDocument;
use crate::error::Result;
use crate::fonts::unicode_decode::TextCharIter;
use crate::fonts::{CIDToGIDMap, FontInfo};
use crate::host::content::parts::decoded_reader;
use crate::object::{Object, ObjectRef};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::Read;

pub(crate) type Dict = HashMap<String, Object>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    TrueTypeSimple,
    TrueTypeCid,
    CffCid,
}

struct Entry {
    info: FontInfo,
    file: u32,
    kind: Kind,
    memo: HashMap<u16, Vec<u16>>,
}

/// The font a content has selected.
#[derive(Clone, Copy)]
enum Current {
    Unset,
    Unresolved,
    /// A font no glyphs are collected for.
    Other,
    Font(u32),
}

/// What a content has selected so far: the current font and the ones its
/// `q` operators saved. One per content read.
pub(crate) struct Feed {
    current: Current,
    saved: Vec<Current>,
}

impl Feed {
    pub(crate) fn new() -> Feed {
        Feed { current: Current::Unset, saved: Vec::new() }
    }
}

/// The glyphs, fonts and poisoned programs the contents of one save show.
pub(crate) struct Collector<'a> {
    doc: &'a PdfDocument,
    staged: &'a HashMap<u32, Object>,
    overrides: &'a HashMap<u32, Object>,
    fonts: HashMap<u32, Option<Entry>>,
    kinds: HashMap<u32, Kind>,
    glyphs: HashMap<u32, BTreeSet<u16>>,
    poisoned: HashSet<u32>,
    walked: HashSet<u32>,
    scanned: bool,
    disabled: bool,
    cancel: CancelCheck,
}

fn dict_of(obj: &Object) -> Option<&Dict> {
    match obj {
        Object::Dictionary(dict) => Some(dict),
        Object::Stream { dict, .. } => Some(dict),
        _ => None,
    }
}

/// The programs `font`'s descriptors hold, whatever the kind of font.
fn descriptor_files(collector: &Collector<'_>, font: &Dict) -> Vec<u32> {
    let mut out = Vec::new();
    let mut descriptors = Vec::new();
    descriptors.extend(font.get("FontDescriptor").cloned());
    if let Some(list) = font.get("DescendantFonts").and_then(|v| collector.resolve(v)) {
        let items: Vec<Object> = match &list {
            Object::Array(items) => items.clone(),
            other => vec![other.clone()],
        };
        for item in items {
            if let Some(cid) = collector.resolve(&item) {
                descriptors.extend(dict_of(&cid).and_then(|d| d.get("FontDescriptor")).cloned());
            }
        }
    }
    for descriptor in descriptors {
        let Some(descriptor) = collector.resolve(&descriptor) else { continue };
        let Some(dict) = dict_of(&descriptor) else { continue };
        for key in ["FontFile", "FontFile2", "FontFile3"] {
            if let Some(Object::Reference(r)) = dict.get(key) {
                out.push(r.id);
            }
        }
    }
    out
}

impl<'a> Collector<'a> {
    /// A collector for a save that writes `doc` with `staged` objects over
    /// its own and `overrides` in place of them. It meets a font program
    /// only when a content shows text in a font that holds one, so a document
    /// with none costs no read of its own.
    pub(crate) fn new(
        doc: &'a PdfDocument,
        staged: &'a HashMap<u32, Object>,
        overrides: &'a HashMap<u32, Object>,
        cancel: &CancelCheck,
    ) -> Collector<'a> {
        Collector {
            doc,
            staged,
            overrides,
            fonts: HashMap::new(),
            kinds: HashMap::new(),
            glyphs: HashMap::new(),
            poisoned: HashSet::new(),
            walked: HashSet::new(),
            scanned: false,
            disabled: false,
            cancel: cancel.clone(),
        }
    }

    /// Whether the plan has been dropped: nothing more is to be offered.
    pub(crate) fn is_disabled(&self) -> bool {
        self.disabled
    }

    /// Drops the plan: a content cannot be trusted to show what it shows.
    pub(crate) fn disable(&mut self) {
        self.disabled = true;
    }

    /// Records that a walk reached the font map held by object `holder`
    /// (a font dictionary held by a resource dictionary is held by the
    /// object the resource dictionary is in, or is its own object).
    pub(crate) fn reached(&mut self, holder: u32) {
        self.walked.insert(holder);
    }

    fn read(&self, id: u32) -> Option<Object> {
        match self.overrides.get(&id).or_else(|| self.staged.get(&id)) {
            Some(obj) => Some(obj.clone()),
            None => self.doc.load_object(ObjectRef::new(id, 0)).ok(),
        }
    }

    /// `value` itself, or the object it references.
    fn resolve(&self, value: &Object) -> Option<Object> {
        match value {
            Object::Reference(r) => self.read(r.id),
            other => Some(other.clone()),
        }
    }

    fn dict_value(&self, value: &Object) -> Option<Dict> {
        dict_of(&self.resolve(value)?).cloned()
    }

    fn poison_files(&mut self, files: Vec<u32>) {
        self.poisoned.extend(files);
    }

    fn poison_font(&mut self, id: u32) {
        if let Some(font) = self.read(id) {
            if let Some(dict) = dict_of(&font) {
                let files = descriptor_files(self, dict);
                self.poison_files(files);
            }
        }
    }

    fn poison_font_value(&mut self, value: &Object) {
        match value {
            Object::Reference(r) => self.poison_font(r.id),
            Object::Dictionary(dict) => {
                let files = descriptor_files(self, dict);
                self.poison_files(files);
            },
            _ => {},
        }
    }

    /// Leaves whole the program of every font in `map`.
    pub(crate) fn poison_map(&mut self, map: &Dict) {
        for value in map.values() {
            self.poison_font_value(value);
        }
    }

    fn classify(&self, font: &Dict) -> Option<(Kind, u32)> {
        let file = |descriptor: &Object, key: &str| -> Option<u32> {
            let descriptor = self.resolve(descriptor)?;
            let dict = dict_of(&descriptor)?;
            let others = ["FontFile", "FontFile2", "FontFile3"].iter().filter(|k| **k != key);
            if others.into_iter().any(|k| dict.contains_key(*k)) {
                return None;
            }
            Some(dict.get(key)?.as_reference()?.id)
        };
        match font.get("Subtype")?.as_name()? {
            "TrueType" => {
                Some((Kind::TrueTypeSimple, file(font.get("FontDescriptor")?, "FontFile2")?))
            },
            "Type0" => {
                if !matches!(font.get("Encoding")?.as_name()?, "Identity-H" | "Identity-V") {
                    return None;
                }
                let descendants = self.resolve(font.get("DescendantFonts")?)?;
                let first = match &descendants {
                    Object::Array(items) => items.first()?.clone(),
                    other => other.clone(),
                };
                let cid = self.dict_value(&first)?;
                let descriptor = cid.get("FontDescriptor")?;
                match cid.get("Subtype")?.as_name()? {
                    "CIDFontType2" => Some((Kind::TrueTypeCid, file(descriptor, "FontFile2")?)),
                    "CIDFontType0" => {
                        let id = file(descriptor, "FontFile3")?;
                        let program = self.read(id)?;
                        let subtype = dict_of(&program)?.get("Subtype")?.as_name()?;
                        (subtype == "CIDFontType0C").then_some((Kind::CffCid, id))
                    },
                    _ => None,
                }
            },
            _ => None,
        }
    }

    /// Registers font `id` the first time it is met: a font that cannot be
    /// subset poisons its programs.
    fn font(&mut self, id: u32) {
        if self.fonts.contains_key(&id) {
            return;
        }
        let entry = (|| {
            let obj = self.read(id)?;
            let dict = dict_of(&obj)?;
            let (kind, file) = self.classify(dict)?;
            let info = FontInfo::from_dict(&Object::Dictionary(dict.clone()), self.doc).ok()?;
            info.embedded_font_data.as_ref()?;
            Some(Entry { info, file, kind, memo: HashMap::new() })
        })();
        match &entry {
            Some(entry) => {
                if *self.kinds.entry(entry.file).or_insert(entry.kind) != entry.kind {
                    self.poisoned.insert(entry.file);
                }
            },
            None => self.poison_font(id),
        }
        self.fonts.insert(id, entry);
    }

    fn show(&mut self, font: u32, bytes: &[u8]) {
        let Some(Some(entry)) = self.fonts.get_mut(&font) else { return };
        let set = self.glyphs.entry(entry.file).or_default();
        match entry.kind {
            Kind::TrueTypeSimple => {
                for &code in bytes {
                    set.extend(entry.reach(code as u16));
                }
            },
            Kind::TrueTypeCid | Kind::CffCid => {
                let codes: Vec<u16> =
                    TextCharIter::new(bytes, Some(&entry.info)).map(|(code, _)| code).collect();
                for code in codes {
                    set.extend(entry.reach(code));
                }
            },
        }
    }

    /// Takes the next operator of a content read against the font map
    /// `fonts`: collects what it shows. `poison` is whether the content
    /// belongs to a form field or a page the save edits, whose fonts stay
    /// whole.
    pub(crate) fn op(&mut self, feed: &mut Feed, op: &Operator, fonts: &Dict, poison: bool) {
        if self.disabled {
            return;
        }
        match op {
            Operator::SaveState => feed.saved.push(feed.current),
            Operator::RestoreState => {
                if let Some(previous) = feed.saved.pop() {
                    feed.current = previous;
                }
            },
            Operator::Tf { font, .. } => {
                feed.current = match fonts.get(font) {
                    Some(Object::Reference(r)) => {
                        if poison {
                            self.poison_font(r.id);
                        }
                        self.font(r.id);
                        Current::Font(r.id)
                    },
                    Some(direct) => {
                        self.poison_font_value(direct);
                        Current::Other
                    },
                    None => Current::Unresolved,
                };
            },
            Operator::Tj { text } | Operator::Quote { text } => self.shown(feed.current, text),
            Operator::DoubleQuote { text, .. } => self.shown(feed.current, text),
            Operator::TJ { array } => {
                for item in array {
                    if let TextElement::String(text) = item {
                        self.shown(feed.current, text);
                    }
                }
            },
            _ => {},
        }
    }

    fn shown(&mut self, current: Current, text: &[u8]) {
        match current {
            Current::Font(id) => self.show(id, text),
            Current::Other => {},
            Current::Unset | Current::Unresolved => self.disabled = true,
        }
    }

    /// Whether the objects of the document are still to be offered to
    /// [`Collector::scan_object`]: some font program is to be cut.
    pub(crate) fn wants_scan(&self) -> bool {
        !self.disabled && !self.glyphs.is_empty() && !self.scanned
    }

    /// Notes that every object of the document has been offered to
    /// [`Collector::scan_object`] after the last content was read.
    pub(crate) fn scanned(&mut self) {
        self.scanned = true;
    }

    /// Poisons the fonts a resource dictionary no walk reached holds in
    /// object `id`, the fonts an `ExtGState` names and those of a `/DR`.
    /// Every object of the document is offered once the walk is done: a
    /// walk that has not reached a font map cannot say that no field uses it.
    pub(crate) fn scan_object(&mut self, id: u32) {
        // A free entry holds nothing, and asking the document for one makes
        // it search the whole source for the object.
        let replaced = self.overrides.contains_key(&id) || self.staged.contains_key(&id);
        if id == 0 || (!replaced && self.doc.is_free_entry(id)) {
            return;
        }
        let Some(obj) = self.read(id) else { return };
        let Some(dict) = dict_of(&obj) else { return };
        self.scan_dict(dict, Some(id), true);
    }

    fn scan_all(&mut self) -> Result<()> {
        let mut ids = self.doc.all_object_ids();
        let source_ids: HashSet<u32> = ids.iter().copied().collect();
        ids.extend(self.staged.keys().copied().filter(|id| !source_ids.contains(id)));
        ids.extend(self.overrides.keys().copied().filter(|id| !source_ids.contains(id)));
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            self.cancel.check()?;
            self.scan_object(id);
        }
        self.scanned = true;
        Ok(())
    }

    fn scan_dict(&mut self, dict: &Dict, owner: Option<u32>, top: bool) {
        if let Some(value) = dict.get("Font") {
            self.scan_fonts(value, owner);
        }
        if let Some(value) = dict.get("DR") {
            if let Some(resources) = self.dict_value(value) {
                if let Some(map) = resources.get("Font").and_then(|v| self.dict_value(v)) {
                    self.poison_map(&map);
                }
            }
        }
        for (key, value) in dict {
            if key == "Font" {
                continue;
            }
            let inner = if top && key == "Resources" { owner } else { None };
            self.scan_value(value, inner);
        }
    }

    fn scan_value(&mut self, value: &Object, owner: Option<u32>) {
        match value {
            Object::Dictionary(dict) => self.scan_dict(dict, owner, false),
            Object::Array(items) => items.iter().for_each(|item| self.scan_value(item, None)),
            _ => {},
        }
    }

    fn scan_fonts(&mut self, value: &Object, owner: Option<u32>) {
        let (map, holder) = match value {
            Object::Dictionary(map) => (map.clone(), owner.unwrap_or(u32::MAX)),
            Object::Reference(r) => match self.dict_value(value) {
                Some(map) => (map, r.id),
                None => return,
            },
            Object::Array(items) => {
                // an ExtGState's [font size]
                if let Some(first) = items.first() {
                    self.poison_font_value(first);
                }
                return;
            },
            _ => return,
        };
        if self.walked.contains(&holder) {
            return;
        }
        self.poison_map(&map);
    }

    /// The replacement stream of each program the glyph sets let shrink,
    /// by object id, once every content has been offered. `aliases` is the
    /// duplicate-to-kept map of the save: a program written once for two
    /// streams holds the glyphs of both. `level` is the save's Flate level.
    pub(crate) fn finish(
        mut self,
        aliases: &HashMap<u32, u32>,
        level: u32,
    ) -> Result<HashMap<u32, Object>> {
        // No content showed a font that holds a program: nothing is to cut,
        // and no object has to be read to say which fonts stay whole.
        if self.disabled || self.glyphs.is_empty() {
            return Ok(HashMap::new());
        }
        if !self.scanned {
            self.scan_all()?;
        }
        let mut own_ids: HashSet<u32> = self.doc.all_object_ids().into_iter().collect();
        own_ids.extend(self.staged.keys().copied());
        Ok(self.cut(aliases, level, &own_ids))
    }

    fn cut(
        &mut self,
        aliases: &HashMap<u32, u32>,
        level: u32,
        own_ids: &HashSet<u32>,
    ) -> HashMap<u32, Object> {
        let mut sets: HashMap<u32, BTreeSet<u16>> = HashMap::new();
        let mut kinds: HashMap<u32, Kind> = HashMap::new();
        let mut poisoned: HashSet<u32> = HashSet::new();
        for (&file, set) in &self.glyphs {
            let target = aliases.get(&file).copied().unwrap_or(file);
            sets.entry(target).or_default().extend(set.iter().copied());
            if let Some(&kind) = self.kinds.get(&file) {
                if *kinds.entry(target).or_insert(kind) != kind {
                    poisoned.insert(target);
                }
            }
        }
        for &file in &self.poisoned {
            poisoned.insert(aliases.get(&file).copied().unwrap_or(file));
            poisoned.insert(file);
        }
        // A twin outside this document is a stream of a merged source.
        for (duplicate, kept) in aliases {
            if !own_ids.contains(duplicate) {
                poisoned.insert(*kept);
            }
        }

        let mut out = HashMap::new();
        let mut ids: Vec<u32> = sets.keys().copied().collect();
        ids.sort_unstable();
        for id in ids {
            if self.cancel.check().is_err() {
                break;
            }
            let (Some(set), Some(&kind)) = (sets.get(&id), kinds.get(&id)) else { continue };
            if poisoned.contains(&id)
                || !own_ids.contains(&id)
                || set.is_empty()
                || self.overrides.contains_key(&id)
                || self.staged.contains_key(&id)
            {
                continue;
            }
            if let Some(replacement) = self.subset(id, kind, set, level) {
                out.insert(id, replacement);
            }
        }
        out
    }

    fn subset(&self, id: u32, kind: Kind, set: &BTreeSet<u16>, level: u32) -> Option<Object> {
        let Object::Stream { dict, body } = self.read(id)? else { return None };
        let mut program = Vec::new();
        decoded_reader(&Object::Stream { dict: dict.clone(), body: body.clone() })
            .ok()?
            .read_to_end(&mut program)
            .ok()?;
        let cut = match kind {
            Kind::TrueTypeSimple | Kind::TrueTypeCid => truetype::subset(&program, set)?,
            Kind::CffCid => cff::subset(&program, set)?,
        };
        // What the save stores for the program without the plan: what the
        // source holds, or the program deflated, whichever is smaller.
        let deflated = |bytes: &[u8]| -> Option<Vec<u8>> {
            let mut out = Vec::new();
            let raw = crate::host::deflate::Encoding::Raw;
            crate::host::deflate::recode(std::io::Cursor::new(bytes), raw, level, &mut out).ok()?;
            Some(out)
        };
        let packed = deflated(&cut)?;
        let baseline = (body.len()).min(deflated(&program)?.len() as u64);
        if packed.len() as u64 >= baseline {
            return None;
        }
        let mut dict = dict;
        dict.remove("DecodeParms");
        dict.remove("DL");
        dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
        dict.insert("Length".to_string(), Object::Integer(packed.len() as i64));
        if dict.contains_key("Length1") {
            dict.insert("Length1".to_string(), Object::Integer(cut.len() as i64));
        }
        Some(Object::stream(dict, packed))
    }
}

impl Entry {
    /// Every glyph `code` can show in this font.
    fn reach(&mut self, code: u16) -> Vec<u16> {
        if let Some(hit) = self.memo.get(&code) {
            return hit.clone();
        }
        let mut out: BTreeSet<u16> = BTreeSet::new();
        let info = &self.info;
        if self.kind != Kind::TrueTypeSimple {
            let spec = match &info.cid_to_gid_map {
                Some(CIDToGIDMap::Identity) => code,
                Some(CIDToGIDMap::Explicit(map)) => *map.get(code as usize).unwrap_or(&code),
                None => info
                    .cff_cid_to_gid
                    .as_ref()
                    .and_then(|m| m.get(&code).copied())
                    .unwrap_or(code),
            };
            out.insert(spec);
            out.insert(code);
        }
        if self.kind != Kind::CffCid {
            if let Some(data) = &info.embedded_font_data {
                if let Ok(face) = ttf_parser::Face::parse(data, 0) {
                    let mut keys: Vec<u32> = Vec::new();
                    if self.kind == Kind::TrueTypeSimple {
                        keys.extend([code as u32, 0xF000 | code as u32]);
                    }
                    let text = info.char_to_unicode(code as u32).unwrap_or_default();
                    keys.extend(text.chars().map(|c| c as u32));
                    if let Some(extra) = u8::try_from(code).ok().and_then(|c| info.multi_char_map.get(&c)) {
                        keys.extend(extra.chars().map(|c| c as u32));
                    }
                    if let Some(cmap) = face.tables().cmap {
                        for key in &keys {
                            for sub in cmap.subtables {
                                if let Some(gid) = sub.glyph_index(*key) {
                                    out.insert(gid.0);
                                }
                            }
                        }
                    }
                    if let Some(name) = u8::try_from(code).ok().and_then(|c| info.diff_glyph_names.get(&c)) {
                        if let Some(gid) = face.glyph_index_by_name(name) {
                            out.insert(gid.0);
                        }
                    }
                    if let Some(map) = info.truetype_cmap() {
                        if let Some(gid) = map.code_to_gid(code) {
                            out.insert(gid);
                        }
                    }
                }
            }
        }
        let out: Vec<u16> = out.into_iter().collect();
        self.memo.insert(code, out.clone());
        out
    }
}
