//! PDF object serialization.
//!
//! Serializes PDF objects to their byte representation according to
//! PDF specification ISO 32000-1:2008.

use crate::encryption::EncryptionWriteHandler;
use crate::object::{Object, ObjectRef};
// ── pdf_manipulator patch: stream bodies are StreamBody ──
use crate::object::StreamBody;
// ── end pdf_manipulator patch ──
use std::collections::HashMap;
// ── pdf_manipulator patch: a stream is its dictionary and an extent that reads itself ──
use std::io::{Read, Write};
// ── end pdf_manipulator patch ──

/// Serializer for PDF objects.
///
/// Converts PDF Object types to their byte representation following
/// the PDF specification syntax rules.
#[derive(Debug, Clone, Default)]
pub struct ObjectSerializer {
    /// Whether to use compact formatting (minimal whitespace)
    compact: bool,
    /// Current indentation level for pretty printing
    #[allow(dead_code)]
    indent_level: usize,
    // ── pdf_manipulator patch: references to unwritten objects as null ──
    /// Object ids a reference to is written as `null` (ISO 32000-1
    /// §7.3.10: a reference to a missing object is the null object).
    null_refs: std::collections::HashSet<u32>,
    /// Write every reference with generation 0: a full rewrite writes
    /// every object at generation 0, so `5 1 R` would name nothing.
    zero_generations: bool,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: the references a save wrote ──
    /// Every object id a written reference names, with the first indirect
    /// object that wrote it. `None` unless `recording_references` is set.
    reference_log: Option<std::sync::Arc<std::sync::Mutex<ReferenceLog>>>,
    // ── end pdf_manipulator patch ──
    // ── pdf_manipulator patch: references to a merged duplicate name its survivor ──
    /// Object id to the id every reference to it is written as. `None`
    /// unless `aliasing` is set.
    aliases: Option<std::sync::Arc<HashMap<u32, u32>>>,
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: the references a save wrote ──
/// What a recording serializer saw: the indirect object being written and
/// the first writer of each referenced id.
#[derive(Debug, Default)]
struct ReferenceLog {
    current: u32,
    first_writer: HashMap<u32, u32>,
}
// ── end pdf_manipulator patch ──

impl ObjectSerializer {
    /// Create a new object serializer with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a compact serializer (minimal whitespace).
    pub fn compact() -> Self {
        Self {
            compact: true,
            indent_level: 0,
            // ── pdf_manipulator patch: references to unwritten objects as null ──
            null_refs: Default::default(),
            zero_generations: false,
            reference_log: None,
            aliases: None,
            // ── end pdf_manipulator patch ──
        }
    }

    // ── pdf_manipulator patch: references to unwritten objects as null ──
    /// This serializer, writing every reference to one of `ids` as `null`:
    /// a save that leaves those objects out says so in every place that
    /// pointed at them, instead of leaving a dangling reference.
    pub(crate) fn nulling(mut self, ids: std::collections::HashSet<u32>) -> Self {
        self.null_refs = ids;
        self
    }

    /// This serializer, writing every reference with generation 0, for a
    /// save that writes every object at generation 0.
    pub(crate) fn zero_generations(mut self) -> Self {
        self.zero_generations = true;
        self
    }

    /// This serializer, writing every reference to a key of `aliases` as a
    /// reference to its value: a save that writes one copy of a set of
    /// identical objects names that copy everywhere. The alias applies
    /// before the null and recording checks, so the log names the copy.
    pub(crate) fn aliasing(mut self, aliases: std::sync::Arc<HashMap<u32, u32>>) -> Self {
        self.aliases = Some(aliases);
        self
    }

    /// This serializer, noting every reference it writes (a reference it
    /// writes as `null` is not one) so `written_references` can say which
    /// objects the output names. Clones share the log.
    pub(crate) fn recording_references(mut self) -> Self {
        self.reference_log = Some(Default::default());
        self
    }

    /// `(referenced id, first indirect object that wrote a reference to
    /// it)` for every reference written so far, ordered by referenced id.
    pub(crate) fn written_references(&self) -> Vec<(u32, u32)> {
        let Some(log) = &self.reference_log else {
            return Vec::new();
        };
        let log = log.lock().unwrap_or_else(|e| e.into_inner());
        let mut all: Vec<(u32, u32)> = log.first_writer.iter().map(|(&t, &w)| (t, w)).collect();
        all.sort_unstable();
        all
    }

    /// Serialize the body of object `id` for an object stream: the object
    /// alone, without the `obj`/`endobj` frame, its references noted as
    /// written by `id`.
    pub(crate) fn serialize_member(&self, id: u32, obj: &Object) -> Vec<u8> {
        self.begin_indirect(id);
        self.serialize(obj)
    }

    fn begin_indirect(&self, id: u32) {
        if let Some(log) = &self.reference_log {
            log.lock().unwrap_or_else(|e| e.into_inner()).current = id;
        }
    }

    fn write_reference<W: Write>(&self, w: &mut W, r: &ObjectRef) -> std::io::Result<()> {
        let id = self
            .aliases
            .as_ref()
            .and_then(|aliases| aliases.get(&r.id))
            .copied()
            .unwrap_or(r.id);
        if self.null_refs.contains(&id) {
            write!(w, "null")
        } else {
            if let Some(log) = &self.reference_log {
                let mut log = log.lock().unwrap_or_else(|e| e.into_inner());
                let current = log.current;
                log.first_writer.entry(id).or_insert(current);
            }
            let gen = if self.zero_generations { 0 } else { r.gen };
            write!(w, "{} {} R", id, gen)
        }
    }
    // ── end pdf_manipulator patch ──

    /// Serialize an object to bytes.
    pub fn serialize(&self, obj: &Object) -> Vec<u8> {
        let mut buf = Vec::new();
        self.write_object(&mut buf, obj).unwrap();
        buf
    }

    /// Serialize an object to a string (for debugging).
    pub fn serialize_to_string(&self, obj: &Object) -> String {
        String::from_utf8_lossy(&self.serialize(obj)).to_string()
    }

    /// Serialize an indirect object definition.
    ///
    /// Format: `{id} {gen} obj\n{object}\nendobj\n`
    pub fn serialize_indirect(&self, id: u32, gen: u16, obj: &Object) -> Vec<u8> {
        let mut buf = Vec::new();
        writeln!(buf, "{} {} obj", id, gen).unwrap();
        // ── pdf_manipulator patch: the references a save wrote ──
        self.begin_indirect(id);
        // ── end pdf_manipulator patch ──
        self.write_object(&mut buf, obj).unwrap();
        write!(buf, "\nendobj\n").unwrap();
        buf
    }

    /// Serialize an indirect object with encryption.
    ///
    /// Format: `{id} {gen} obj\n{encrypted_object}\nendobj\n`
    ///
    /// Strings and stream data within the object are encrypted using
    /// the provided encryption handler.
    pub fn serialize_indirect_encrypted(
        &self,
        id: u32,
        gen: u16,
        obj: &Object,
        handler: &EncryptionWriteHandler,
    ) -> Vec<u8> {
        let mut buf = Vec::new();
        writeln!(buf, "{} {} obj", id, gen).unwrap();
        // ── pdf_manipulator patch: the references a save wrote ──
        self.begin_indirect(id);
        // ── end pdf_manipulator patch ──
        self.write_object_encrypted(&mut buf, obj, id, gen, handler)
            .unwrap();
        write!(buf, "\nendobj\n").unwrap();
        buf
    }

    // ── pdf_manipulator patch: an indirect object is written to a sink, its stream data in pieces ──
    /// Like [`Self::serialize_indirect`], into `out`. A stream body that is
    /// an extent is read from its source in pieces as it is written, so a
    /// stream of any size passes through in constant memory; this is the
    /// call for an object loaded from a document.
    pub(crate) fn write_indirect(
        &self,
        mut out: &mut dyn Write,
        id: u32,
        gen: u16,
        obj: &Object,
    ) -> std::io::Result<()> {
        writeln!(out, "{} {} obj", id, gen)?;
        self.begin_indirect(id);
        self.write_object(&mut out, obj)?;
        write!(out, "\nendobj\n")
    }

    /// Like [`Self::serialize_indirect_encrypted`], into `out`; see
    /// [`Self::write_indirect`].
    pub(crate) fn write_indirect_encrypted(
        &self,
        mut out: &mut dyn Write,
        id: u32,
        gen: u16,
        obj: &Object,
        handler: &EncryptionWriteHandler,
    ) -> std::io::Result<()> {
        writeln!(out, "{} {} obj", id, gen)?;
        self.begin_indirect(id);
        self.write_object_encrypted(&mut out, obj, id, gen, handler)?;
        write!(out, "\nendobj\n")
    }

    /// Writes the head of stream object `id`: its header, `dict` as given
    /// (the caller states `/Length`) and the `stream` keyword. The caller
    /// writes the data and `\nendstream\nendobj\n`.
    pub(crate) fn write_stream_open(
        &self,
        mut out: &mut dyn Write,
        id: u32,
        gen: u16,
        dict: &HashMap<String, Object>,
    ) -> std::io::Result<()> {
        writeln!(out, "{} {} obj", id, gen)?;
        self.begin_indirect(id);
        self.write_dictionary(&mut out, dict)?;
        write!(out, "\nstream\n")
    }
    // ── end pdf_manipulator patch ──

    /// Write an encrypted object to a buffer.
    fn write_object_encrypted<W: Write>(
        &self,
        w: &mut W,
        obj: &Object,
        obj_num: u32,
        gen_num: u16,
        handler: &EncryptionWriteHandler,
    ) -> std::io::Result<()> {
        match obj {
            Object::Null => write!(w, "null"),
            Object::Boolean(b) => write!(w, "{}", if *b { "true" } else { "false" }),
            Object::Integer(i) => write!(w, "{}", i),
            Object::Real(r) => self.write_real(w, *r),
            Object::String(s) => {
                // Encrypt the string
                let encrypted = handler.encrypt_string(s, obj_num, gen_num);
                self.write_string(w, &encrypted)
            },
            Object::Name(n) => self.write_name(w, n),
            Object::Array(arr) => self.write_array_encrypted(w, arr, obj_num, gen_num, handler),
            Object::Dictionary(dict) => {
                self.write_dictionary_encrypted(w, dict, obj_num, gen_num, handler)
            },
            // ── pdf_manipulator patch: stream bodies are StreamBody ──
            Object::Stream { dict, body } => {
                self.write_stream_encrypted(w, dict, body, obj_num, gen_num, handler)
            },
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: references to unwritten objects as null ──
            Object::Reference(r) => self.write_reference(w, r),
            // ── end pdf_manipulator patch ──
        }
    }

    /// Write an encrypted array.
    fn write_array_encrypted<W: Write>(
        &self,
        w: &mut W,
        arr: &[Object],
        obj_num: u32,
        gen_num: u16,
        handler: &EncryptionWriteHandler,
    ) -> std::io::Result<()> {
        write!(w, "[")?;
        for (i, obj) in arr.iter().enumerate() {
            if i > 0 {
                write!(w, " ")?;
            }
            self.write_object_encrypted(w, obj, obj_num, gen_num, handler)?;
        }
        write!(w, "]")
    }

    /// Write an encrypted dictionary.
    fn write_dictionary_encrypted<W: Write>(
        &self,
        w: &mut W,
        dict: &HashMap<String, Object>,
        obj_num: u32,
        gen_num: u16,
        handler: &EncryptionWriteHandler,
    ) -> std::io::Result<()> {
        write!(w, "<<")?;

        // Sort keys for deterministic output
        let mut keys: Vec<_> = dict.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = dict.get(key) {
                if !self.compact {
                    write!(w, "\n  ")?;
                }
                self.write_name(w, key)?;
                write!(w, " ")?;
                self.write_object_encrypted(w, value, obj_num, gen_num, handler)?;
            }
        }

        if !self.compact && !dict.is_empty() {
            writeln!(w)?;
        }
        write!(w, ">>")
    }

    /// Write an encrypted stream.
    fn write_stream_encrypted<W: Write>(
        &self,
        w: &mut W,
        dict: &HashMap<String, Object>,
        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        body: &StreamBody,
        // ── end pdf_manipulator patch ──
        obj_num: u32,
        gen_num: u16,
        handler: &EncryptionWriteHandler,
    ) -> std::io::Result<()> {
        // ── pdf_manipulator patch: stream data is encrypted in pieces ──
        // The data is encrypted as it is written, so its encrypted length
        // is worked out first from the plain length.
        let cipher = handler.stream_cipher(obj_num, gen_num);
        let plain_len = body.decrypted_len().map_err(std::io::Error::other)?;

        // Update dictionary with encrypted length
        let mut dict_with_length = dict.clone();
        dict_with_length
            .insert("Length".to_string(), Object::Integer(cipher.encrypted_len(plain_len) as i64));

        // Write dictionary (with encrypted strings inside)
        self.write_dictionary_encrypted(w, &dict_with_length, obj_num, gen_num, handler)?;
        write!(w, "\nstream\n")?;
        let iv = EncryptionWriteHandler::generate_iv().map_err(std::io::Error::other)?;
        let mut encrypting = cipher.encrypting(&mut *w, iv).map_err(std::io::Error::other)?;
        copy_body(&mut encrypting, body)?;
        encrypting.finish()?;
        // ── end pdf_manipulator patch ──
        write!(w, "\nendstream")
    }

    /// Write an object to a buffer.
    fn write_object<W: Write>(&self, w: &mut W, obj: &Object) -> std::io::Result<()> {
        match obj {
            Object::Null => write!(w, "null"),
            Object::Boolean(b) => write!(w, "{}", if *b { "true" } else { "false" }),
            Object::Integer(i) => write!(w, "{}", i),
            Object::Real(r) => self.write_real(w, *r),
            Object::String(s) => self.write_string(w, s),
            Object::Name(n) => self.write_name(w, n),
            Object::Array(arr) => self.write_array(w, arr),
            Object::Dictionary(dict) => self.write_dictionary(w, dict),
            // ── pdf_manipulator patch: stream bodies are StreamBody ──
            Object::Stream { dict, body } => self.write_stream(w, dict, body),
            // ── end pdf_manipulator patch ──
            // ── pdf_manipulator patch: references to unwritten objects as null ──
            Object::Reference(r) => self.write_reference(w, r),
            // ── end pdf_manipulator patch ──
        }
    }

    /// Write a real number with appropriate precision.
    fn write_real<W: Write>(&self, w: &mut W, value: f64) -> std::io::Result<()> {
        // PDF spec allows up to 5 decimal places for coordinates
        // Remove trailing zeros for compact output
        if value.fract() == 0.0 {
            write!(w, "{}", value as i64)
        } else {
            // Format with enough precision, then trim trailing zeros
            let formatted = format!("{:.5}", value);
            let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
            write!(w, "{}", trimmed)
        }
    }

    /// Write a PDF string.
    ///
    /// Uses literal string syntax `(...)` with proper escaping,
    /// or hex string syntax `<...>` for binary data.
    fn write_string<W: Write>(&self, w: &mut W, data: &[u8]) -> std::io::Result<()> {
        // Check if data is printable ASCII
        let is_printable = data
            .iter()
            .all(|&b| b == b'\n' || b == b'\r' || b == b'\t' || (0x20..=0x7E).contains(&b));

        if is_printable {
            // Use literal string
            write!(w, "(")?;
            for &byte in data {
                match byte {
                    b'(' => write!(w, "\\(")?,
                    b')' => write!(w, "\\)")?,
                    b'\\' => write!(w, "\\\\")?,
                    b'\n' => write!(w, "\\n")?,
                    b'\r' => write!(w, "\\r")?,
                    b'\t' => write!(w, "\\t")?,
                    _ => w.write_all(&[byte])?,
                }
            }
            write!(w, ")")
        } else {
            // Use hex string
            write!(w, "<")?;
            for byte in data {
                write!(w, "{:02X}", byte)?;
            }
            write!(w, ">")
        }
    }

    /// Write a PDF name.
    ///
    /// Names start with `/` and escape special characters with `#xx`.
    fn write_name<W: Write>(&self, w: &mut W, name: &str) -> std::io::Result<()> {
        write!(w, "/")?;
        for byte in name.bytes() {
            // ── pdf_manipulator patch: a delimiter, `#` and anything outside printable ASCII are escaped ──
            let regular = (0x21..=0x7E).contains(&byte)
                && !matches!(
                    byte,
                    b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%' | b'#'
                );
            if regular {
                w.write_all(&[byte])?;
            } else {
                write!(w, "#{:02X}", byte)?;
            }
            // ── end pdf_manipulator patch ──
        }
        Ok(())
    }

    /// Write a PDF array.
    fn write_array<W: Write>(&self, w: &mut W, arr: &[Object]) -> std::io::Result<()> {
        write!(w, "[")?;
        for (i, obj) in arr.iter().enumerate() {
            if i > 0 {
                write!(w, " ")?;
            }
            self.write_object(w, obj)?;
        }
        write!(w, "]")
    }

    /// Write a PDF dictionary.
    fn write_dictionary<W: Write>(
        &self,
        w: &mut W,
        dict: &HashMap<String, Object>,
    ) -> std::io::Result<()> {
        write!(w, "<<")?;

        // Sort keys for deterministic output
        let mut keys: Vec<_> = dict.keys().collect();
        keys.sort();

        for key in keys {
            if let Some(value) = dict.get(key) {
                if !self.compact {
                    write!(w, "\n  ")?;
                }
                self.write_name(w, key)?;
                write!(w, " ")?;
                self.write_object(w, value)?;
            }
        }

        if !self.compact && !dict.is_empty() {
            writeln!(w)?;
        }
        write!(w, ">>")
    }

    /// Write a PDF stream.
    fn write_stream<W: Write>(
        &self,
        w: &mut W,
        dict: &HashMap<String, Object>,
        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        body: &StreamBody,
        // ── end pdf_manipulator patch ──
    ) -> std::io::Result<()> {
        // ── pdf_manipulator patch: an extent's /Length is the length it writes ──
        // Add Length to dictionary if not present; an extent states it
        // always: the dictionary's own may name a data length that was
        // repaired, or the ciphertext's.
        let mut dict_with_length = dict.clone();
        match body {
            StreamBody::Inline(data) => {
                if !dict_with_length.contains_key("Length") {
                    dict_with_length
                        .insert("Length".to_string(), Object::Integer(data.len() as i64));
                }
            },
            StreamBody::Extent(_) => {
                let len = body.decrypted_len().map_err(std::io::Error::other)?;
                dict_with_length.insert("Length".to_string(), Object::Integer(len as i64));
            },
        }

        self.write_dictionary(w, &dict_with_length)?;
        write!(w, "\nstream\n")?;
        copy_body(w, body)?;
        // ── end pdf_manipulator patch ──
        write!(w, "\nendstream")
    }
}

// ── pdf_manipulator patch: stream data is written in pieces ──
/// Writes the decrypted data of `body` to `w` in 64 KiB pieces.
fn copy_body(w: &mut dyn Write, body: &StreamBody) -> std::io::Result<()> {
    let mut data = body.raw_reader().map_err(std::io::Error::other)?;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = data.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        w.write_all(&buf[..n])?;
    }
}
// ── end pdf_manipulator patch ──

/// Helper functions for building PDF objects.
impl ObjectSerializer {
    /// Create a Name object.
    pub fn name(s: &str) -> Object {
        Object::Name(s.to_string())
    }

    /// Create a String object from a Rust string.
    ///
    /// Encodes using PDFDocEncoding (U+0000–U+00FF) or UTF-16BE with BOM
    /// (characters above U+00FF) per ISO 32000-2 §7.9.2.
    pub fn string(s: &str) -> Object {
        Object::text_string(s)
    }

    /// Create an Integer object.
    pub fn integer(i: i64) -> Object {
        Object::Integer(i)
    }

    /// Create a Real object.
    pub fn real(r: f64) -> Object {
        Object::Real(r)
    }

    /// Create a Boolean object.
    pub fn boolean(b: bool) -> Object {
        Object::Boolean(b)
    }

    /// Create an Array object.
    pub fn array(items: Vec<Object>) -> Object {
        Object::Array(items)
    }

    /// Create a Dictionary object.
    pub fn dict(entries: Vec<(&str, Object)>) -> Object {
        let map: HashMap<String, Object> = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        Object::Dictionary(map)
    }

    /// Create a Reference object.
    pub fn reference(id: u32, gen: u16) -> Object {
        Object::Reference(ObjectRef::new(id, gen))
    }

    /// Create a rectangle array [x, y, width, height] -> [llx, lly, urx, ury].
    pub fn rect(x: f64, y: f64, width: f64, height: f64) -> Object {
        Object::Array(vec![
            Object::Real(x),
            Object::Real(y),
            Object::Real(x + width),
            Object::Real(y + height),
        ])
    }
}

/// Hoist inline appearance streams out of an annotation dictionary into
/// indirect objects, as the PDF spec requires.
///
/// An annotation's `/AP` appearance can carry stream objects under `/N`,
/// `/D` and `/R` (each either a single stream or a sub-dictionary of named
/// appearance states). Some builders construct these as a direct
/// [`Object::Stream`] nested inside the dictionary — which serializes to an
/// illegal inline `<< … >> stream … endstream` *inside* the annotation dict.
/// A stream **must** be an indirect object (ISO 32000-1 §7.3.8), so this
/// replaces every nested stream with an [`Object::Reference`] to a freshly
/// allocated id (drawn from `next_id`, which is advanced) and returns the
/// `(id, stream)` pairs for the caller to write as separate indirect objects.
///
/// Returns an empty vector when there is nothing to hoist, so it is safe to
/// call on every annotation dictionary unconditionally.
pub(crate) fn hoist_appearance_streams(
    annot_dict: &mut HashMap<String, Object>,
    next_id: &mut u32,
) -> Vec<(u32, Object)> {
    let mut extracted = Vec::new();
    let mut hoist = |slot: &mut Object, out: &mut Vec<(u32, Object)>| {
        if matches!(slot, Object::Stream { .. }) {
            let id = *next_id;
            *next_id += 1;
            let stream = std::mem::replace(slot, Object::Reference(ObjectRef::new(id, 0)));
            out.push((id, stream));
        }
    };

    if let Some(Object::Dictionary(ap)) = annot_dict.get_mut("AP") {
        for key in ["N", "D", "R"] {
            match ap.get_mut(key) {
                Some(slot @ Object::Stream { .. }) => hoist(slot, &mut extracted),
                // A sub-dictionary of named appearance states (e.g. a
                // checkbox's `/On` and `/Off`); hoist each state's stream.
                Some(Object::Dictionary(states)) => {
                    for state in states.values_mut() {
                        hoist(state, &mut extracted);
                    }
                },
                _ => {},
            }
        }
    }

    extracted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_null() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::Null), "null");
    }

    fn stream(tag: &str) -> Object {
        let mut d = HashMap::new();
        d.insert("Subtype".to_string(), Object::Name("Form".to_string()));
        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        Object::stream(d, bytes::Bytes::from(tag.to_string()))
        // ── end pdf_manipulator patch ──
    }

    #[test]
    fn test_hoist_appearance_stream_replaces_inline_n_with_reference() {
        // An annotation whose `/AP /N` is an inline stream must come back with
        // `/N` rewritten to an indirect reference and the stream returned for
        // separate writing -- a stream is illegal as a direct dict value.
        let mut ap = HashMap::new();
        ap.insert("N".to_string(), stream("appearance"));
        let mut annot = HashMap::new();
        annot.insert("Subtype".to_string(), Object::Name("Watermark".to_string()));
        annot.insert("AP".to_string(), Object::Dictionary(ap));

        let mut next_id = 42;
        let hoisted = hoist_appearance_streams(&mut annot, &mut next_id);

        assert_eq!(next_id, 43, "one id consumed");
        assert_eq!(hoisted.len(), 1);
        assert_eq!(hoisted[0].0, 42);
        assert!(matches!(hoisted[0].1, Object::Stream { .. }));

        let Object::Dictionary(ap) = &annot["AP"] else {
            panic!("AP not a dict")
        };
        assert_eq!(ap["N"], Object::Reference(ObjectRef::new(42, 0)));

        // And it must serialize as an indirect ref, never as an inline stream.
        let rendered = ObjectSerializer::new().serialize_to_string(&annot["AP"]);
        assert!(rendered.contains("42 0 R"), "got: {rendered}");
        assert!(!rendered.contains("stream"), "inline stream leaked: {rendered}");
    }

    #[test]
    fn test_hoist_appearance_handles_named_state_substreams() {
        // `/N` may be a dict of named states (e.g. checkbox /On, /Off); each
        // state's stream must be hoisted independently.
        let mut states = HashMap::new();
        states.insert("On".to_string(), stream("on"));
        states.insert("Off".to_string(), stream("off"));
        let mut ap = HashMap::new();
        ap.insert("N".to_string(), Object::Dictionary(states));
        let mut annot = HashMap::new();
        annot.insert("AP".to_string(), Object::Dictionary(ap));

        let mut next_id = 100;
        let hoisted = hoist_appearance_streams(&mut annot, &mut next_id);

        assert_eq!(hoisted.len(), 2);
        assert_eq!(next_id, 102);
        let Object::Dictionary(ap) = &annot["AP"] else {
            panic!("AP not a dict")
        };
        let Object::Dictionary(states) = &ap["N"] else {
            panic!("N not a dict")
        };
        assert!(matches!(states["On"], Object::Reference(_)));
        assert!(matches!(states["Off"], Object::Reference(_)));
    }

    #[test]
    fn test_hoist_appearance_noop_without_streams() {
        // No `/AP`, or an `/AP` without streams, must consume no ids.
        let mut annot = HashMap::new();
        annot.insert("Subtype".to_string(), Object::Name("Link".to_string()));
        let mut next_id = 7;
        assert!(hoist_appearance_streams(&mut annot, &mut next_id).is_empty());
        assert_eq!(next_id, 7);
    }

    #[test]
    fn test_serialize_boolean() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::Boolean(true)), "true");
        assert_eq!(s.serialize_to_string(&Object::Boolean(false)), "false");
    }

    #[test]
    fn test_serialize_integer() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::Integer(42)), "42");
        assert_eq!(s.serialize_to_string(&Object::Integer(-123)), "-123");
    }

    #[test]
    fn test_serialize_real() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::Real(3.14258)), "3.14258");
        assert_eq!(s.serialize_to_string(&Object::Real(1.0)), "1");
        assert_eq!(s.serialize_to_string(&Object::Real(0.5)), "0.5");
    }

    #[test]
    fn test_serialize_string() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::String(b"Hello".to_vec())), "(Hello)");
        assert_eq!(
            s.serialize_to_string(&Object::String(b"Test (parens)".to_vec())),
            "(Test \\(parens\\))"
        );
    }

    #[test]
    fn test_serialize_hex_string() {
        let s = ObjectSerializer::new();
        // Binary data should use hex string
        assert_eq!(s.serialize_to_string(&Object::String(vec![0x00, 0xFF, 0x80])), "<00FF80>");
    }

    #[test]
    fn test_serialize_name() {
        let s = ObjectSerializer::new();
        assert_eq!(s.serialize_to_string(&Object::Name("Type".to_string())), "/Type");
        assert_eq!(s.serialize_to_string(&Object::Name("Font".to_string())), "/Font");
    }

    #[test]
    fn test_serialize_name_with_special_chars() {
        let s = ObjectSerializer::new();
        assert_eq!(
            s.serialize_to_string(&Object::Name("Name With Space".to_string())),
            "/Name#20With#20Space"
        );
    }

    #[test]
    fn test_serialize_array() {
        let s = ObjectSerializer::compact();
        let arr = Object::Array(vec![Object::Integer(1), Object::Integer(2), Object::Integer(3)]);
        assert_eq!(s.serialize_to_string(&arr), "[1 2 3]");
    }

    #[test]
    fn test_serialize_dictionary() {
        let s = ObjectSerializer::compact();
        let dict = ObjectSerializer::dict(vec![
            ("Type", ObjectSerializer::name("Page")),
            ("Count", ObjectSerializer::integer(1)),
        ]);
        let result = s.serialize_to_string(&dict);
        assert!(result.starts_with("<<"));
        assert!(result.ends_with(">>"));
        assert!(result.contains("/Type /Page"));
        assert!(result.contains("/Count 1"));
    }

    #[test]
    fn test_serialize_reference() {
        let s = ObjectSerializer::new();
        let r = Object::Reference(ObjectRef::new(10, 0));
        assert_eq!(s.serialize_to_string(&r), "10 0 R");
    }

    #[test]
    fn test_serialize_indirect() {
        let s = ObjectSerializer::new();
        let bytes = s.serialize_indirect(1, 0, &Object::Integer(42));
        let str = String::from_utf8_lossy(&bytes);
        assert!(str.contains("1 0 obj"));
        assert!(str.contains("42"));
        assert!(str.contains("endobj"));
    }

    #[test]
    fn test_serialize_stream() {
        let s = ObjectSerializer::compact();
        let mut dict = HashMap::new();
        dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));

        // ── pdf_manipulator patch: stream bodies are StreamBody ──
        let stream = Object::stream(dict, bytes::Bytes::from_static(b"stream data"));
        // ── end pdf_manipulator patch ──

        let result = s.serialize_to_string(&stream);
        assert!(result.contains("/Length 11"));
        assert!(result.contains("stream\n"));
        assert!(result.contains("stream data"));
        assert!(result.contains("\nendstream"));
    }

    #[test]
    fn test_rect_helper() {
        let rect = ObjectSerializer::rect(0.0, 0.0, 612.0, 792.0);
        let s = ObjectSerializer::compact();
        assert_eq!(s.serialize_to_string(&rect), "[0 0 612 792]");
    }
}
