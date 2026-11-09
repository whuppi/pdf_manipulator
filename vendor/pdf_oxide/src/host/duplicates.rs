//! Identical images, ICC profiles and embedded font programs.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A full rewrite that merges duplicates asks [`find`] first. It reads the
//! objects the save will write, from the editor's document and from every
//! merged source, and answers with the alias map `duplicate id -> kept id`:
//! the lowest output id of each set of identical streams is kept.
//!
//! Two streams are one when they are of one kind (an image XObject, a
//! stream named by an `[/ICCBased <ref>]` array, a stream a FontDescriptor
//! names by `/FontFile`, `/FontFile2` or `/FontFile3`), neither carries
//! `/OC`, `/StructParent` or `/StructParents`, and their SHA-256 over the
//! canonical dictionary and the encoded payload bytes agree. The canonical
//! dictionary has its keys sorted, `/Length` removed and every reference
//! replaced by what it names; a stream it names counts with its own
//! dictionary and payload, so two images whose soft masks differ in data
//! stay two. Payloads are read in pieces and never held whole: a stream of
//! an encrypted source is decrypted through a reader into the hasher, and
//! two streams whose digests agree are compared the same way, piece by
//! piece, before one is replaced by the other.

use crate::cancel::CancelCheck;
use crate::document::{PdfDocument, StreamHead};
use crate::error::{Error, Result};
use crate::object::{Object, ObjectRef, StreamBody};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Read;

const CHUNK: usize = 64 * 1024;

type Digest32 = [u8; 32];

/// One document the save writes objects of.
pub(crate) struct Origin<'a> {
    pub(crate) doc: &'a PdfDocument,
    /// `(output id, id in doc)` of every object the save writes from it.
    pub(crate) objects: Vec<(u32, u32)>,
    /// The objects the editor stages over the document's own and beside
    /// them, by output id; only the editor's own document has any.
    pub(crate) staged: Option<&'a HashMap<u32, Object>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Image,
    IccProfile,
    FontProgram,
}

struct Candidate {
    origin: usize,
    out: u32,
    src: u32,
    kind: Kind,
    len: u64,
}

/// An object as the pre-pass reads it.
enum Fetched<'a> {
    Staged(&'a Object),
    /// A plain stream: its dictionary and where its data sits.
    Head(StreamHead),
    Loaded(Object),
}

impl Fetched<'_> {
    fn object(&self) -> Option<&Object> {
        match self {
            Fetched::Staged(obj) => Some(obj),
            Fetched::Loaded(obj) => Some(obj),
            Fetched::Head(_) => None,
        }
    }

    /// The dictionary and the data length of a stream.
    fn stream(&self) -> Option<(&HashMap<String, Object>, u64)> {
        match self {
            Fetched::Head(head) => Some((&head.dict, head.data_len())),
            other => match other.object()? {
                Object::Stream { dict, body } => Some((dict, body.len())),
                _ => None,
            },
        }
    }
}

fn fetch<'a>(origin: &Origin<'a>, id: u32) -> Result<Fetched<'a>> {
    if let Some(staged) = origin.staged.and_then(|staged| staged.get(&id)) {
        return Ok(Fetched::Staged(staged));
    }
    let r = ObjectRef::new(id, 0);
    if let Some(head) = origin.doc.stream_head(r)? {
        return Ok(Fetched::Head(head));
    }
    Ok(Fetched::Loaded(origin.doc.load_object(r)?))
}

/// Feeds the decrypted data of `body` to `hasher` in pieces.
fn hash_body(body: &StreamBody, hasher: &mut Sha256) -> Result<()> {
    let mut data = body.raw_reader()?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = data.read(&mut buf).map_err(crate::encryption::stream::from_io)?;
        if n == 0 {
            return Ok(());
        }
        hasher.update(&buf[..n]);
    }
}

/// A stream the merge never touches: it names a layer or a structure.
fn is_blocked(dict: &HashMap<String, Object>) -> bool {
    ["OC", "StructParent", "StructParents"].iter().any(|key| dict.contains_key(*key))
}

fn is_image(dict: &HashMap<String, Object>) -> bool {
    dict.get("Subtype").and_then(|s| s.as_name()) == Some("Image")
}

/// Notes the streams `obj` names as an ICC profile or a font program.
fn note_uses(obj: &Object, kinds: &mut HashMap<u32, Kind>) {
    match obj {
        Object::Array(items) => {
            if let [Object::Name(name), Object::Reference(r)] = items.as_slice() {
                if name == "ICCBased" {
                    kinds.entry(r.id).or_insert(Kind::IccProfile);
                }
            }
            items.iter().for_each(|item| note_uses(item, kinds));
        },
        Object::Dictionary(dict) => note_dict_uses(dict, kinds),
        Object::Stream { dict, .. } => note_dict_uses(dict, kinds),
        _ => {},
    }
}

fn note_dict_uses(dict: &HashMap<String, Object>, kinds: &mut HashMap<u32, Kind>) {
    for key in ["FontFile", "FontFile2", "FontFile3"] {
        if let Some(Object::Reference(r)) = dict.get(key) {
            kinds.insert(r.id, Kind::FontProgram);
        }
    }
    dict.values().for_each(|value| note_uses(value, kinds));
}

/// A stream that is neither an image nor blocked: it can only be merged as
/// an ICC profile or a font program, which a dictionary elsewhere names.
struct Plain {
    origin: usize,
    out: u32,
    src: u32,
    len: u64,
}

/// Whether the dictionary carries a key the specification requires of an
/// ICC profile (`/N`) or a font program (`/Length1` of a TrueType or Type 1
/// program, `/Subtype` of `/FontFile3`). A content stream has none, so a
/// document of thousands of equal-sized pages reads none of them.
fn names_a_profile_or_program(dict: &HashMap<String, Object>) -> bool {
    ["N", "Length1", "Subtype"].iter().any(|key| dict.contains_key(*key))
}

/// A container the save rewrites on its own.
fn is_container(dict: &HashMap<String, Object>) -> bool {
    matches!(dict.get("Type").and_then(|t| t.as_name()), Some("ObjStm") | Some("XRef"))
}

/// Reads the head of every stream of `origin`: its images are candidates,
/// its other streams wait in `plain` for [`shared_plain`]. A stream whose
/// head cannot be read without loading it whole is left alone.
fn scan_streams(
    index: usize,
    origin: &Origin<'_>,
    cancel: &CancelCheck,
    images: &mut Vec<Candidate>,
    plain: &mut Vec<Plain>,
) -> Result<()> {
    for &(out, src) in &origin.objects {
        cancel.check()?;
        let id = fetch_id(origin, out, src);
        let fetched = match origin.staged.and_then(|staged| staged.get(&id)) {
            Some(staged) => Fetched::Staged(staged),
            None => match origin.doc.stream_head(ObjectRef::new(id, 0))? {
                Some(head) => Fetched::Head(head),
                None => continue,
            },
        };
        let Some((dict, len)) = fetched.stream() else { continue };
        if is_blocked(dict) || is_container(dict) {
            continue;
        }
        if is_image(dict) {
            images.push(Candidate { origin: index, out, src, kind: Kind::Image, len });
        } else if names_a_profile_or_program(dict) {
            plain.push(Plain { origin: index, out, src, len });
        }
    }
    Ok(())
}

/// The streams of `plain` whose encoded data another one of them holds
/// too. A document with none of these has no profile or font program to
/// merge, so its other objects are never read.
fn shared_plain(origins: &[Origin<'_>], plain: Vec<Plain>, cancel: &CancelCheck) -> Result<Vec<Plain>> {
    let mut by_len: HashMap<u64, Vec<usize>> = HashMap::new();
    for (index, p) in plain.iter().enumerate() {
        by_len.entry(p.len).or_default().push(index);
    }
    let mut by_data: HashMap<(u64, Digest32), Vec<usize>> = HashMap::new();
    for group in by_len.into_values().filter(|group| group.len() > 1) {
        for index in group {
            cancel.check()?;
            let p = &plain[index];
            let origin = &origins[p.origin];
            let id = fetch_id(origin, p.out, p.src);
            let Ok(fetched) = fetch(origin, id) else { continue };
            let mut hasher = Sha256::new();
            if data_into(&fetched, &mut hasher)? {
                by_data.entry((p.len, hasher.finalize().into())).or_default().push(index);
            }
        }
    }
    let mut keep: Vec<usize> =
        by_data.into_values().filter(|group| group.len() > 1).flatten().collect();
    keep.sort_unstable();
    let mut plain: Vec<Option<Plain>> = plain.into_iter().map(Some).collect();
    Ok(keep.into_iter().filter_map(|index| plain[index].take()).collect())
}

/// Reads every object of `origin`: the ids named as an ICC profile or a
/// font program.
fn classify(origin: &Origin<'_>, cancel: &CancelCheck) -> Result<HashMap<u32, Kind>> {
    let mut kinds: HashMap<u32, Kind> = HashMap::new();
    for &(out, src) in &origin.objects {
        cancel.check()?;
        let fetched = fetch(origin, fetch_id(origin, out, src))?;
        match fetched.stream() {
            Some((dict, _)) => note_dict_uses(dict, &mut kinds),
            None => {
                if let Some(obj) = fetched.object() {
                    note_uses(obj, &mut kinds);
                }
            },
        }
    }
    Ok(kinds)
}

/// The id `fetch` reads: the editor's own document is read and staged by
/// output id, a merged source by its own.
fn fetch_id(origin: &Origin<'_>, out: u32, src: u32) -> u32 {
    if origin.staged.is_some() {
        out
    } else {
        src
    }
}

/// Feeds the encoded data of stream `id`, read in pieces and decrypted as it
/// is read; `false` when it cannot be read in full.
fn data_into(fetched: &Fetched<'_>, hasher: &mut Sha256) -> Result<bool> {
    match fetched {
        Fetched::Head(head) => {
            match hash_body(&StreamBody::Extent(head.extent.clone()), hasher) {
                Ok(()) => Ok(true),
                Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
                Err(e) => Err(e),
            }
        },
        other => match other.object() {
            Some(Object::Stream { body, .. }) => {
                hash_body(body, hasher)?;
                Ok(true)
            },
            _ => Ok(false),
        },
    }
}

/// Whether stream `left_id` of `left` and stream `right_id` of `right` hold
/// the same encoded data, decrypted and read in pieces side by side. The
/// cancel check is polled before each piece.
pub(crate) fn same_stream_data(
    left: &Origin<'_>,
    left_id: u32,
    right: &Origin<'_>,
    right_id: u32,
    cancel: &CancelCheck,
) -> Result<bool> {
    let Ok(left) = fetch(left, left_id) else { return Ok(false) };
    let Ok(right) = fetch(right, right_id) else { return Ok(false) };
    same_data(&left, &right, cancel)
}

/// Whether the encoded data of the two streams is the same, decrypted and
/// read in pieces side by side. A stream that cannot be read in full is
/// not the same as any other.
fn same_data(a: &Fetched<'_>, b: &Fetched<'_>, cancel: &CancelCheck) -> Result<bool> {
    let (Some(mut left), Some(mut right)) = (data_reader(a)?, data_reader(b)?) else {
        return Ok(false);
    };
    let mut left_buf = vec![0u8; CHUNK];
    let mut right_buf = vec![0u8; CHUNK];
    loop {
        cancel.check()?;
        let (n, m) = match (fill(&mut left, &mut left_buf), fill(&mut right, &mut right_buf)) {
            (Ok(n), Ok(m)) => (n, m),
            (Err(e), _) | (_, Err(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(false)
            },
            (Err(e), _) | (_, Err(e)) => return Err(crate::encryption::stream::from_io(e)),
        };
        if n != m || left_buf[..n] != right_buf[..m] {
            return Ok(false);
        }
        if n == 0 {
            return Ok(true);
        }
    }
}

/// The decrypted data of a stream as a reader; `None` for an object that is
/// not a stream.
fn data_reader<'a>(fetched: &'a Fetched<'_>) -> Result<Option<Box<dyn Read + 'a>>> {
    match fetched {
        Fetched::Head(head) => Ok(Some(StreamBody::Extent(head.extent.clone()).owned_reader()?)),
        other => match other.object() {
            Some(Object::Stream { body, .. }) => Ok(Some(body.raw_reader()?)),
            _ => Ok(None),
        },
    }
}

/// Reads until `buf` is full or the reader ends; the bytes read.
fn fill(reader: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

struct Hashing<'a, 'b> {
    origins: &'b [Origin<'a>],
    /// The digest of every stream a dictionary named, by origin and id.
    memo: Vec<HashMap<u32, Digest32>>,
    /// Set when a reference on the current path was met, so the digest it
    /// shaped is not reusable.
    met_cycle: bool,
}

impl Hashing<'_, '_> {
    /// The digest of candidate `c`, or `None` when its dictionary or data
    /// cannot be read in full.
    fn candidate(&mut self, c: &Candidate) -> Result<Option<Digest32>> {
        let origin = &self.origins[c.origin];
        let id = fetch_id(origin, c.out, c.src);
        let Ok(fetched) = fetch(origin, id) else { return Ok(None) };
        let mut hasher = Sha256::new();
        hasher.update([c.kind as u8]);
        let mut path = vec![c.src];
        self.met_cycle = false;
        if !self.stream_into(c.origin, &fetched, &mut path, &mut hasher)? {
            return Ok(None);
        }
        Ok(Some(hasher.finalize().into()))
    }

    /// Whether the encoded data of candidates `a` and `b` is the same.
    fn same_data(&self, a: &Candidate, b: &Candidate, cancel: &CancelCheck) -> Result<bool> {
        let (left, right) = (&self.origins[a.origin], &self.origins[b.origin]);
        same_stream_data(left, fetch_id(left, a.out, a.src), right, fetch_id(right, b.out, b.src), cancel)
    }

    /// Feeds the canonical dictionary and then the data of a stream.
    fn stream_into(
        &mut self,
        o: usize,
        fetched: &Fetched<'_>,
        path: &mut Vec<u32>,
        hasher: &mut Sha256,
    ) -> Result<bool> {
        let Some((dict, len)) = fetched.stream() else { return Ok(false) };
        if !self.dict_into(o, dict, true, path, hasher)? {
            return Ok(false);
        }
        hasher.update(len.to_le_bytes());
        data_into(fetched, hasher)
    }

    fn dict_into(
        &mut self,
        o: usize,
        dict: &HashMap<String, Object>,
        stream: bool,
        path: &mut Vec<u32>,
        hasher: &mut Sha256,
    ) -> Result<bool> {
        let mut keys: Vec<&String> =
            dict.keys().filter(|key| !(stream && key.as_str() == "Length")).collect();
        keys.sort_unstable();
        hasher.update([b'd']);
        hasher.update((keys.len() as u64).to_le_bytes());
        for key in keys {
            hasher.update((key.len() as u64).to_le_bytes());
            hasher.update(key.as_bytes());
            if !self.object_into(o, &dict[key], path, hasher)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn object_into(
        &mut self,
        o: usize,
        obj: &Object,
        path: &mut Vec<u32>,
        hasher: &mut Sha256,
    ) -> Result<bool> {
        match obj {
            Object::Null => hasher.update([b'n']),
            Object::Boolean(b) => hasher.update([b'b', *b as u8]),
            Object::Integer(i) => {
                hasher.update([b'i']);
                hasher.update(i.to_le_bytes());
            },
            Object::Real(f) => {
                hasher.update([b'r']);
                hasher.update(f.to_bits().to_le_bytes());
            },
            Object::String(bytes) => {
                hasher.update([b's']);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(bytes);
            },
            Object::Name(name) => {
                hasher.update([b'N']);
                hasher.update((name.len() as u64).to_le_bytes());
                hasher.update(name.as_bytes());
            },
            Object::Array(items) => {
                hasher.update([b'a']);
                hasher.update((items.len() as u64).to_le_bytes());
                for item in items {
                    if !self.object_into(o, item, path, hasher)? {
                        return Ok(false);
                    }
                }
            },
            Object::Dictionary(dict) => return self.dict_into(o, dict, false, path, hasher),
            Object::Stream { dict, .. } => return self.dict_into(o, dict, true, path, hasher),
            Object::Reference(r) => return self.reference_into(o, r.id, path, hasher),
        }
        Ok(true)
    }

    /// Feeds what reference `id` names. A reference already on the path
    /// feeds a marker; a stream feeds the digest of its dictionary and data.
    fn reference_into(
        &mut self,
        o: usize,
        id: u32,
        path: &mut Vec<u32>,
        hasher: &mut Sha256,
    ) -> Result<bool> {
        if path.contains(&id) {
            self.met_cycle = true;
            hasher.update([b'c']);
            return Ok(true);
        }
        if let Some(digest) = self.memo[o].get(&id) {
            hasher.update([b'S']);
            hasher.update(digest);
            return Ok(true);
        }
        let origin = &self.origins[o];
        let Ok(fetched) = fetch(origin, id) else { return Ok(false) };
        path.push(id);
        let was_cyclic = std::mem::replace(&mut self.met_cycle, false);
        let fed = match &fetched {
            Fetched::Head(_) => self.named_stream_into(o, id, &fetched, path, hasher)?,
            other => match other.object() {
                Some(Object::Stream { .. }) => self.named_stream_into(o, id, &fetched, path, hasher)?,
                Some(obj) => {
                    hasher.update([b'R']);
                    self.object_into(o, obj, path, hasher)?
                },
                None => false,
            },
        };
        path.pop();
        self.met_cycle |= was_cyclic;
        Ok(fed)
    }

    fn named_stream_into(
        &mut self,
        o: usize,
        id: u32,
        fetched: &Fetched<'_>,
        path: &mut Vec<u32>,
        hasher: &mut Sha256,
    ) -> Result<bool> {
        let mut inner = Sha256::new();
        if !self.stream_into(o, fetched, path, &mut inner)? {
            return Ok(false);
        }
        let digest: Digest32 = inner.finalize().into();
        if !self.met_cycle {
            self.memo[o].insert(id, digest);
        }
        hasher.update([b'S']);
        hasher.update(digest);
        Ok(true)
    }
}

/// The alias map of the objects `origins` write: every duplicate id to the
/// lowest id of its set. Empty when nothing repeats.
pub(crate) fn find(origins: &[Origin<'_>], cancel: &CancelCheck) -> Result<HashMap<u32, u32>> {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut plain: Vec<Plain> = Vec::new();
    for (index, origin) in origins.iter().enumerate() {
        scan_streams(index, origin, cancel, &mut candidates, &mut plain)?;
    }
    let shared = shared_plain(origins, plain, cancel)?;
    let mut kinds: HashMap<usize, HashMap<u32, Kind>> = HashMap::new();
    for p in shared {
        if !kinds.contains_key(&p.origin) {
            kinds.insert(p.origin, classify(&origins[p.origin], cancel)?);
        }
        if let Some(&kind) = kinds[&p.origin].get(&p.src) {
            candidates.push(Candidate { origin: p.origin, out: p.out, src: p.src, kind, len: p.len });
        }
    }

    // Equal streams have equal data lengths: only a group of two or more is
    // read.
    let mut groups: HashMap<(Kind, u64), Vec<usize>> = HashMap::new();
    for (index, c) in candidates.iter().enumerate() {
        groups.entry((c.kind, c.len)).or_default().push(index);
    }

    let mut hashing = Hashing {
        origins,
        memo: origins.iter().map(|_| HashMap::new()).collect(),
        met_cycle: false,
    };
    let mut aliases: HashMap<u32, u32> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    groups.sort_unstable();
    for group in groups {
        // The digest of each kept stream, with the candidate that holds it.
        let mut kept: HashMap<Digest32, (u32, usize)> = HashMap::new();
        let mut members: Vec<(u32, Digest32, usize)> = Vec::new();
        for index in group {
            cancel.check()?;
            let c = &candidates[index];
            if let Some(digest) = hashing.candidate(c)? {
                members.push((c.out, digest, index));
            }
        }
        members.sort_unstable();
        for (out, digest, index) in members {
            cancel.check()?;
            match kept.get(&digest) {
                Some(&(canonical, _)) if canonical == out => {},
                Some(&(canonical, kept_index)) => {
                    // Equal digests are not taken for equal data: the two are
                    // read side by side, decrypted, before one stands for both.
                    if hashing.same_data(&candidates[kept_index], &candidates[index], cancel)? {
                        aliases.insert(out, canonical);
                    }
                },
                None => {
                    kept.insert(digest, (out, index));
                },
            }
        }
    }
    Ok(aliases)
}
