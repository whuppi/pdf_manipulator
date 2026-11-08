//! Readers of the decoded content of a page and of a form.

use super::cache::ContentCache;
use crate::decoders::stream::reader_in;
use crate::error::{Error, Result};
use crate::object::{Object, ObjectRef};
use std::collections::VecDeque;
use std::io::{self, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The decoded content of a stream object, read as the stream is decoded.
///
/// The first chunk is decoded before this returns, so a stream that cannot be
/// decoded at all is an error here, as it is for `decode_stream_data`; damage
/// further in ends the data where it is.
pub(crate) fn decoded_reader(obj: &Object) -> Result<Box<dyn Read>> {
    let rest: Box<dyn Read> = match obj {
        Object::Stream { dict, body } => {
            let filters = dict
                .get("Filter")
                .map(crate::object::extract_filter_names)
                .unwrap_or_default();
            if filters.is_empty() {
                body.owned_reader()?
            } else {
                let owned = body.clone();
                reader_in(dict, body.origin(), move || owned.owned_reader())?
            }
        },
        // A dictionary where a stream is expected is an empty stream, as for
        // `decode_stream_data`.
        Object::Dictionary(_) | Object::Null => Box::new(io::empty()),
        other => {
            return Err(Error::InvalidObjectType {
                expected: "Stream".to_string(),
                found: other.type_name().to_string(),
            })
        },
    };
    Primed::open(rest).map(|r| Box::new(r) as Box<dyn Read>)
}

/// A reader whose first chunk has been read.
struct Primed {
    head: Vec<u8>,
    at: usize,
    rest: Box<dyn Read>,
}

impl Primed {
    fn open(mut rest: Box<dyn Read>) -> Result<Self> {
        let mut head = vec![0u8; 8 * 1024];
        let n = loop {
            match rest.read(&mut head) {
                Ok(n) => break n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {},
                Err(e) => return Err(crate::encryption::stream::from_io(e)),
            }
        };
        head.truncate(n);
        Ok(Self { head, at: 0, rest })
    }
}

impl Read for Primed {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.at < self.head.len() {
            let n = out.len().min(self.head.len() - self.at);
            out[..n].copy_from_slice(&self.head[self.at..self.at + n]);
            self.at += n;
            return Ok(n);
        }
        self.rest.read(out)
    }
}

/// The decoded parts of a page's `/Contents`, one after another.
///
/// An array's parts each end with a newline, as `cached_page_content` joins
/// them, so a token never runs across two parts. A part that cannot be
/// decoded is skipped and counted, as `cached_page_content` skips it.
pub struct PartsReader {
    parts: VecDeque<Object>,
    current: Option<Box<dyn Read>>,
    /// Each part is followed by a newline (an array of parts).
    separate: bool,
    newline: bool,
    unreadable: Arc<AtomicU64>,
}

impl PartsReader {
    /// An array's parts, read in order.
    pub(crate) fn of_array(parts: Vec<Object>, unreadable: Arc<AtomicU64>) -> Self {
        Self { parts: parts.into(), current: None, separate: true, newline: false, unreadable }
    }

    /// One stream, already opened.
    pub(crate) fn of_one(only: Box<dyn Read>, unreadable: Arc<AtomicU64>) -> Self {
        Self {
            parts: VecDeque::new(),
            current: Some(only),
            separate: false,
            newline: false,
            unreadable,
        }
    }
}

impl Read for PartsReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        loop {
            if let Some(current) = &mut self.current {
                let n = current.read(out)?;
                if n > 0 {
                    return Ok(n);
                }
                self.current = None;
                self.newline = self.separate;
            }
            if self.newline {
                self.newline = false;
                out[0] = b'\n';
                return Ok(1);
            }
            let Some(part) = self.parts.pop_front() else { return Ok(0) };
            match decoded_reader(&part) {
                Ok(reader) => self.current = Some(reader),
                Err(e) => {
                    log::warn!("Failed to decode content stream element: {}, skipping", e);
                    self.unreadable.fetch_add(1, Ordering::Relaxed);
                },
            }
        }
    }
}

#[cfg(test)]
thread_local! {
    /// The most bytes any [`Tee`] on this thread has held.
    pub(crate) static TEE_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Passes content through and collects it for the cache while it fits the
/// budget; when the data ends in time the cache admits it. A form that
/// outgrows the budget stops being collected at that byte, so a large form
/// never costs more than the budget.
pub(crate) struct Tee {
    inner: Box<dyn Read>,
    collected: Option<Vec<u8>>,
    room: usize,
    cache: Arc<Mutex<ContentCache>>,
    key: ObjectRef,
}

impl Tee {
    pub(crate) fn new(inner: Box<dyn Read>, cache: Arc<Mutex<ContentCache>>, key: ObjectRef) -> Self {
        let room = crate::cache::MutexExt::lock_or_recover(&*cache).room();
        Self { inner, collected: Some(Vec::new()), room, cache, key }
    }
}

impl Read for Tee {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let n = match self.inner.read(out) {
            Ok(n) => n,
            Err(e) => {
                self.collected = None;
                return Err(e);
            },
        };
        if n > 0 {
            if let Some(buf) = &mut self.collected {
                if buf.len() + n > self.room {
                    self.collected = None;
                } else {
                    buf.extend_from_slice(&out[..n]);
                    #[cfg(test)]
                    TEE_HIGH_WATER.with(|h| h.set(h.get().max(buf.len())));
                }
            }
        } else if !out.is_empty() {
            if let Some(buf) = self.collected.take() {
                crate::cache::MutexExt::lock_or_recover(&*self.cache)
                    .insert(self.key, Arc::new(buf));
            }
        }
        Ok(n)
    }
}

/// Cached content, read as bytes.
pub(crate) struct SharedBytes(pub(crate) Arc<Vec<u8>>);

impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
