//! Object streams and the cross-reference stream of a full rewrite.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! The rewrite serializes every non-stream object through one closure. When
//! packing is on, that closure hands the object's bytes to an
//! [`ObjectPacker`] instead of the writer; the save drains the packer into
//! `/Type /ObjStm` streams of [`MEMBERS`] objects at fixed points of the
//! write, so a batch is the only part of the document held in memory. The
//! final cross-reference section then names each packed object by its
//! stream and index ([`xref_stream`]).

use crate::object::Object;
use flate2::{write::ZlibEncoder, Compression};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write;

/// Objects per object stream.
pub(crate) const MEMBERS: usize = 128;

/// Queued bytes that close a batch before it holds [`MEMBERS`] objects, so a
/// run of large objects never sits whole in memory. Large enough that the
/// member count closes a batch of ordinary objects (a batch closed early
/// costs a stream wrapper and a fresh deflate window).
const BATCH_BYTES: usize = 256 * 1024;

/// The serialized non-stream objects waiting for an object stream.
pub(crate) struct ObjectPacker {
    enabled: bool,
    pending: RefCell<Vec<(u32, Vec<u8>)>>,
    pending_bytes: Cell<usize>,
}

impl ObjectPacker {
    pub(crate) fn new(enabled: bool) -> Self {
        Self { enabled, pending: RefCell::new(Vec::new()), pending_bytes: Cell::new(0) }
    }

    /// Whether `obj` is written inside an object stream: only an object that
    /// is not a stream can be (ISO 32000-1 7.5.7), and only when packing is on.
    pub(crate) fn takes(&self, obj: &Object) -> bool {
        self.enabled && !matches!(obj, Object::Stream { .. })
    }

    /// Queues the serialized body of object `id` (no `obj`/`endobj` frame).
    pub(crate) fn add(&self, id: u32, body: Vec<u8>) {
        self.pending_bytes.set(self.pending_bytes.get() + body.len());
        self.pending.borrow_mut().push((id, body));
    }

    /// A full batch is waiting: [`MEMBERS`] objects or [`BATCH_BYTES`] bytes.
    pub(crate) fn is_full(&self) -> bool {
        self.pending.borrow().len() >= MEMBERS || self.pending_bytes.get() >= BATCH_BYTES
    }

    /// The next batch: up to [`MEMBERS`] objects once a batch is full, or
    /// with `rest` whatever is left.
    pub(crate) fn take_batch(&self, rest: bool) -> Option<Vec<(u32, Vec<u8>)>> {
        if !rest && !self.is_full() {
            return None;
        }
        let mut pending = self.pending.borrow_mut();
        let n = pending.len().min(MEMBERS);
        if n == 0 {
            return None;
        }
        let batch: Vec<(u32, Vec<u8>)> = pending.drain(..n).collect();
        let taken: usize = batch.iter().map(|(_, body)| body.len()).sum();
        self.pending_bytes.set(self.pending_bytes.get() - taken);
        Some(batch)
    }
}

/// Set in the offset of an output xref entry whose object sits in an object
/// stream: the rest of the offset is the stream's number and the entry's
/// generation field holds the index. It keeps the placement in the entry the
/// save already holds, where a side table would cost memory per object.
pub(crate) const PACKED: u64 = 1 << 63;

/// Marks the entries of the objects `ids`, written as object stream `stream_id`, as
/// packed. The members were queued just before their entries were pushed, so
/// they sit at the tail of `entries`. `false` when one has no entry.
pub(crate) fn mark_packed(
    entries: &mut [(u32, u64, u16, bool)],
    ids: &[u32],
    stream_id: u32,
) -> bool {
    let mut index_of: HashMap<u32, u16> =
        ids.iter().enumerate().map(|(i, id)| (*id, i as u16)).collect();
    for entry in entries.iter_mut().rev() {
        if let Some(index) = index_of.remove(&entry.0) {
            entry.1 = PACKED | stream_id as u64;
            entry.2 = index;
            if index_of.is_empty() {
                return true;
            }
        }
    }
    false
}

/// The `/Type /ObjStm` stream object holding `batch`: the id/offset table,
/// then the objects, offsets counted from `/First`. The bodies are deflated
/// as they are read (at level `deflate`, when set), so the batch is held once
/// besides its output.
pub(crate) fn object_stream(batch: Vec<(u32, Vec<u8>)>, deflate: Option<u32>) -> Object {
    let count = batch.len();
    let mut table = String::new();
    let mut at = 0usize;
    for (id, body) in &batch {
        table.push_str(&format!("{id} {at} "));
        at += body.len() + 1;
    }
    let mut sink = match deflate {
        Some(level) => Sink::Zlib(ZlibEncoder::new(
            Vec::new(),
            Compression::new(crate::host::deflate::zlib_level(level)),
        )),
        None => Sink::Plain(Vec::new()),
    };
    sink.put(table.as_bytes());
    for (_, body) in batch {
        sink.put(&body);
        sink.put(b"\n");
    }

    let mut dict = HashMap::new();
    dict.insert("Type".to_string(), Object::Name("ObjStm".to_string()));
    dict.insert("N".to_string(), Object::Integer(count as i64));
    dict.insert("First".to_string(), Object::Integer(table.len() as i64));
    if deflate.is_some() {
        dict.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
    }
    Object::stream(dict, sink.finish())
}

/// One cross-reference entry.
pub(crate) enum Row {
    /// Type 0: a free object.
    Free { next: u32, gen: u16 },
    /// Type 1: an object at a byte offset.
    Offset { offset: u64, gen: u16 },
    /// Type 2: an object inside an object stream.
    Packed { stream: u32, index: u32 },
}

/// The rows of a cross-reference stream: one per object number from 0 to
/// `last`, read from the sorted output `entries` without building a map. Of
/// two entries for one id the later wins, as in the classic table.
#[derive(Clone)]
pub(crate) struct XrefRows<'a> {
    entries: &'a [(u32, u64, u16, bool)],
    next: u32,
    last: u32,
    own_offset: u64,
}

impl<'a> XrefRows<'a> {
    /// Rows `0..=last`, where `last` is the xref stream's own object number,
    /// written at `own_offset`.
    pub(crate) fn new(entries: &'a [(u32, u64, u16, bool)], last: u32, own_offset: u64) -> Self {
        Self { entries, next: 0, last, own_offset }
    }
}

impl Iterator for XrefRows<'_> {
    type Item = Row;

    fn next(&mut self) -> Option<Row> {
        if self.next > self.last {
            return None;
        }
        let id = self.next;
        self.next += 1;
        if id == self.last {
            return Some(Row::Offset { offset: self.own_offset, gen: 0 });
        }
        while self.entries.first().is_some_and(|e| e.0 < id) {
            self.entries = &self.entries[1..];
        }
        let mut found = None;
        while let Some(e) = self.entries.first().filter(|e| e.0 == id) {
            found = Some(*e);
            self.entries = &self.entries[1..];
        }
        Some(match found {
            Some((_, offset, index, true)) if offset & PACKED != 0 => {
                Row::Packed { stream: (offset & !PACKED) as u32, index: index as u32 }
            },
            Some((_, offset, gen, true)) => Row::Offset { offset, gen },
            Some((_, _, gen, false)) => Row::Free { next: 0, gen },
            None => Row::Free { next: 0, gen: 65535 },
        })
    }
}

/// The byte width of the second field: enough for `max`, at least one.
pub(crate) fn field_width(max: u64) -> usize {
    ((64 - max.leading_zeros() as usize) + 7) / 8
}

/// The `/Type /XRef` stream object for `rows` (object 0 first, one row per
/// object number), with `trailer` entries in its dictionary. Rows are
/// `[type, field2 (width bytes), field3 (2 bytes)]`. Compressed, they are
/// written twice over, as they are and through the PNG Up predictor (which
/// turns the steadily growing offsets of a large table into small
/// differences), and the smaller stream is kept: on a table of a few dozen
/// rows the predictor and its `/DecodeParms` cost more than they save.
pub(crate) fn xref_stream(
    rows: impl Iterator<Item = Row>,
    width: usize,
    deflate: Option<u32>,
    mut trailer: HashMap<String, Object>,
    cancel: &crate::cancel::CancelCheck,
) -> crate::error::Result<Object> {
    let width = width.max(1);
    let mut stride = crate::cancel::Stride::new(cancel);
    let row_len = 1 + width + 2;
    let new_sink = |level: u32| {
        Sink::Zlib(ZlibEncoder::new(
            Vec::new(),
            Compression::new(crate::host::deflate::zlib_level(level)),
        ))
    };
    let mut plain = match deflate {
        Some(level) => new_sink(level),
        None => Sink::Plain(Vec::new()),
    };
    let mut predicted = deflate.map(new_sink);
    let mut previous = vec![0u8; row_len];
    let mut row = vec![0u8; row_len];
    let mut filtered = vec![0u8; row_len + 1];
    for entry in rows {
        stride.tick()?;
        let (kind, second, third) = match entry {
            Row::Free { next, gen } => (0u8, next as u64, gen as u64),
            Row::Offset { offset, gen } => (1, offset, gen as u64),
            Row::Packed { stream, index } => (2, stream as u64, index as u64),
        };
        row[0] = kind;
        for i in 0..width {
            row[1 + i] = (second >> (8 * (width - 1 - i))) as u8;
        }
        row[1 + width] = (third >> 8) as u8;
        row[2 + width] = third as u8;
        plain.put(&row);
        if let Some(predicted) = predicted.as_mut() {
            filtered[0] = 2;
            for i in 0..row_len {
                filtered[1 + i] = row[i].wrapping_sub(previous[i]);
            }
            predicted.put(&filtered);
        }
        std::mem::swap(&mut previous, &mut row);
    }
    let mut data = plain.finish();

    trailer.insert("Type".to_string(), Object::Name("XRef".to_string()));
    trailer.insert(
        "W".to_string(),
        Object::Array(vec![
            Object::Integer(1),
            Object::Integer(width as i64),
            Object::Integer(2),
        ]),
    );
    if deflate.is_some() {
        trailer.insert("Filter".to_string(), Object::Name("FlateDecode".to_string()));
    }
    if let Some(predicted) = predicted {
        let predicted = predicted.finish();
        // The dictionary entry the predictor adds, as the compact writer spells it.
        let parms = format!("/DecodeParms<</Columns {row_len}/Predictor 12>>").len();
        if predicted.len() + parms < data.len() {
            data = predicted;
            let mut parms = HashMap::new();
            parms.insert("Predictor".to_string(), Object::Integer(12));
            parms.insert("Columns".to_string(), Object::Integer(row_len as i64));
            trailer.insert("DecodeParms".to_string(), Object::Dictionary(parms));
        }
    }
    Ok(Object::stream(trailer, data))
}

/// The bytes of an xref stream as they are produced: raw, or deflated on
/// the way so the rows are never held whole.
enum Sink {
    Plain(Vec<u8>),
    Zlib(ZlibEncoder<Vec<u8>>),
}

impl Sink {
    fn put(&mut self, bytes: &[u8]) {
        match self {
            Sink::Plain(v) => v.extend_from_slice(bytes),
            // Writing to a Vec cannot fail.
            Sink::Zlib(z) => z.write_all(bytes).expect("write to Vec"),
        }
    }

    fn finish(self) -> Vec<u8> {
        match self {
            Sink::Plain(v) => v,
            Sink::Zlib(z) => z.finish().expect("finish into Vec"),
        }
    }
}
