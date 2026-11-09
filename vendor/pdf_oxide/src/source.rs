//! The bytes of an open document and the handler that decrypts them.
//!
//! This file is fork-only code (NOT upstream).
//!
//! A [`Source`] is shared by the document and by every stream extent it
//! hands out ([`crate::object::Extent`]), so a stream reads itself from the
//! document's bytes, with the document's current encryption handler, without
//! the document being in scope.

use crate::cache::MutexExt;
use crate::encryption::EncryptionHandler;
use crate::error::Result;
use crate::extractors::warnings::WarningSink;
use crate::object::ObjectRef;
use std::io::{self, BufRead, BufReader, Cursor, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex, MutexGuard};

/// Trait alias for Read + Seek + Send — trait objects allow only one
/// non-auto trait, so the bound needs a single name.
pub(crate) trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// The two ways a document's bytes are reached: held in memory, or asked
/// for through a callback-backed `Read + Seek` (condvar on native,
/// SAB/Asyncify on web) so the source file is never buffered whole.
pub(crate) enum PdfReader {
    Memory(BufReader<Cursor<Vec<u8>>>),
    External(BufReader<Box<dyn ReadSeek>>),
}

impl Read for PdfReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            PdfReader::Memory(r) => r.read(buf),
            PdfReader::External(r) => r.read(buf),
        }
    }
}

impl Seek for PdfReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            PdfReader::Memory(r) => r.seek(pos),
            PdfReader::External(r) => r.seek(pos),
        }
    }
}

impl PdfReader {
    /// Moves to absolute `offset`. A target inside the bytes already
    /// buffered keeps them, so objects read one after another cost one
    /// read of the source, not one each.
    pub(crate) fn seek_keeping_buffer(&mut self, offset: u64) -> io::Result<()> {
        let here = match self {
            PdfReader::Memory(r) => r.stream_position()?,
            PdfReader::External(r) => r.stream_position()?,
        };
        let by = offset as i64 - here as i64;
        match self {
            PdfReader::Memory(r) => r.seek_relative(by),
            PdfReader::External(r) => r.seek_relative(by),
        }
    }
}

impl BufRead for PdfReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            PdfReader::Memory(r) => r.fill_buf(),
            PdfReader::External(r) => r.fill_buf(),
        }
    }

    fn consume(&mut self, amt: usize) {
        match self {
            PdfReader::Memory(r) => r.consume(amt),
            PdfReader::External(r) => r.consume(amt),
        }
    }
}

/// A document's bytes and its encryption state.
///
/// Every access to the bytes is one seek and one read under one guard of
/// `reader`; a caller that needs more than one read takes no lock between
/// them, so two threads never read each other's position.
pub(crate) struct Source {
    reader: Mutex<PdfReader>,
    /// The handler streams decrypt with; set when the document opens and
    /// replaced when a password authenticates. Read at the moment a reader
    /// opens, so an extent made before authentication decrypts after it.
    encryption: Mutex<Option<Arc<EncryptionHandler>>>,
    /// The `/Encrypt` dictionary's own reference: its strings are key
    /// material, never ciphertext.
    encrypt_dict_ref: Mutex<Option<ObjectRef>>,
    /// The document's structured warnings: a stream that reads itself
    /// reports what it recovered from here.
    warnings: Arc<WarningSink>,
}

impl Source {
    pub(crate) fn new(reader: PdfReader, warnings: Arc<WarningSink>) -> Self {
        Self {
            reader: Mutex::new(reader),
            encryption: Mutex::new(None),
            encrypt_dict_ref: Mutex::new(None),
            warnings,
        }
    }

    /// A source over a caller's reader, read where the reader stands and
    /// never decrypted: the bytes of a file that is not a document.
    pub(crate) fn over(reader: Box<dyn ReadSeek>) -> Arc<Self> {
        Arc::new(Self::new(
            PdfReader::External(BufReader::new(reader)),
            Arc::new(WarningSink::new()),
        ))
    }

    pub(crate) fn warnings(&self) -> &Arc<WarningSink> {
        &self.warnings
    }

    /// The reader, locked. Hold the guard for one seek-and-read only.
    pub(crate) fn lock_reader(&self) -> MutexGuard<'_, PdfReader> {
        self.reader.lock_or_recover()
    }

    /// Fills `buf` from the source at `offset`; returns the bytes read,
    /// fewer than `buf.len()` only at the end of the source.
    pub(crate) fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        let mut r = self.reader.lock_or_recover();
        r.seek_keeping_buffer(offset)?;
        let mut filled = 0;
        while filled < buf.len() {
            let n = r.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        Ok(filled)
    }

    /// The source's length in bytes.
    pub(crate) fn len(&self) -> Result<u64> {
        let mut r = self.reader.lock_or_recover();
        Ok(r.seek(SeekFrom::End(0))?)
    }

    /// The handler streams decrypt with, if the document is encrypted.
    pub(crate) fn handler(&self) -> Option<Arc<EncryptionHandler>> {
        self.encryption.lock_or_recover().clone()
    }

    pub(crate) fn set_handler(&self, handler: Option<Arc<EncryptionHandler>>) {
        *self.encryption.lock_or_recover() = handler;
    }

    pub(crate) fn encrypt_dict_ref(&self) -> Option<ObjectRef> {
        *self.encrypt_dict_ref.lock_or_recover()
    }

    pub(crate) fn set_encrypt_dict_ref(&self, r: Option<ObjectRef>) {
        *self.encrypt_dict_ref.lock_or_recover() = r;
    }
}

/// Reads `[at, end)` of a source in pieces.
pub(crate) struct SourceRead<'a> {
    source: &'a Source,
    at: u64,
    end: u64,
}

impl<'a> SourceRead<'a> {
    pub(crate) fn new(source: &'a Source, at: u64, len: u64) -> Self {
        Self { source, at, end: at + len }
    }
}

impl Read for SourceRead<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let want = ((self.end - self.at) as usize).min(buf.len());
        if want == 0 {
            return Ok(0);
        }
        let n = self
            .source
            .read_at(self.at, &mut buf[..want])
            .map_err(io::Error::other)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.at += n as u64;
        Ok(n)
    }
}

/// [`SourceRead`] that owns its source, so the reader outlives the borrow it
/// was made from: a content stream read operator by operator, from a function
/// that returns while the stream is still being read.
pub(crate) struct OwnedSourceRead {
    source: Arc<Source>,
    at: u64,
    end: u64,
}

impl OwnedSourceRead {
    pub(crate) fn new(source: Arc<Source>, at: u64, len: u64) -> Self {
        Self { source, at, end: at + len }
    }
}

impl Read for OwnedSourceRead {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        SourceRead::new(&self.source, self.at, self.end - self.at).read(buf).map(|n| {
            self.at += n as u64;
            n
        })
    }
}

/// A sink that only counts.
#[derive(Default)]
pub(crate) struct Counter(pub(crate) u64);

impl Write for Counter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A forward-growing read of a source from a fixed offset: the object
/// loader's view of "the bytes from here". Each growth is one `read_at`, so
/// no lock is held between two growths.
pub(crate) struct Window<'a> {
    source: &'a Source,
    base: u64,
    /// Everything read so far, from `base`.
    pub(crate) buf: Vec<u8>,
    /// The next read's size: doubles from 1 KiB to 64 KiB.
    want: usize,
    at_end: bool,
}

impl<'a> Window<'a> {
    pub(crate) fn new(source: &'a Source, base: u64) -> Self {
        Self { source, base, buf: Vec::new(), want: 1024, at_end: false }
    }

    /// Reads on; `false` when the source had nothing more.
    pub(crate) fn grow(&mut self) -> Result<bool> {
        if self.at_end {
            return Ok(false);
        }
        let have = self.buf.len();
        self.buf.resize(have + self.want, 0);
        let n = self.source.read_at(self.base + have as u64, &mut self.buf[have..])?;
        self.buf.truncate(have + n);
        if n < self.want {
            self.at_end = true;
        }
        self.want = (self.want * 2).min(64 * 1024);
        Ok(n > 0)
    }

    /// Grows until `buf` holds `len` bytes or the source ends.
    pub(crate) fn ensure(&mut self, len: usize) -> Result<()> {
        while self.buf.len() < len && self.grow()? {}
        Ok(())
    }

    /// The end of the line that starts at `from`, past its `\n`, or the end
    /// of the source when no `\n` follows.
    pub(crate) fn line_end(&mut self, from: usize) -> Result<usize> {
        let mut scanned = from;
        loop {
            if let Some(i) = self.buf[scanned.min(self.buf.len())..].iter().position(|&b| b == b'\n') {
                return Ok(scanned.min(self.buf.len()) + i + 1);
            }
            scanned = self.buf.len();
            if !self.grow()? {
                return Ok(self.buf.len());
            }
        }
    }

    /// The source offset of `buf[index]`.
    pub(crate) fn offset_of(&self, index: usize) -> u64 {
        self.base + index as u64
    }
}

impl Source {
    /// The offset of the first `needle` at or after `from`, read in 64 KiB
    /// windows that overlap by one byte less than the needle, so a match on
    /// a window's edge is still found. `None` when the source holds none.
    pub(crate) fn find(&self, from: u64, needle: &[u8]) -> Result<Option<u64>> {
        const WINDOW: usize = 64 * 1024;
        let keep = needle.len().saturating_sub(1);
        let mut buf: Vec<u8> = Vec::with_capacity(WINDOW + keep);
        let mut at = from;
        loop {
            // `buf` starts with the `keep` bytes carried from the last window.
            let carried = buf.len();
            buf.resize(carried + WINDOW, 0);
            let n = self.read_at(at, &mut buf[carried..])?;
            buf.truncate(carried + n);
            if let Some(i) = buf.windows(needle.len()).position(|w| w == needle) {
                return Ok(Some(at - carried as u64 + i as u64));
            }
            if n == 0 {
                return Ok(None);
            }
            at += n as u64;
            let tail = buf.len().saturating_sub(keep);
            buf.drain(..tail);
        }
    }
}

/// `Read + Seek` over a [`Source`], so a second reader of the same bytes
/// (another document, an editor) shares them instead of copying them.
pub(crate) struct SourceCursor {
    source: Arc<Source>,
    at: u64,
}

impl SourceCursor {
    pub(crate) fn new(source: Arc<Source>) -> Self {
        Self { source, at: 0 }
    }
}

impl Read for SourceCursor {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.source.read_at(self.at, buf).map_err(io::Error::other)?;
        self.at += n as u64;
        Ok(n)
    }
}

impl Seek for SourceCursor {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(n) => Some(n),
            SeekFrom::End(delta) => {
                let len = self.source.len().map_err(io::Error::other)?;
                len.checked_add_signed(delta)
            },
            SeekFrom::Current(delta) => self.at.checked_add_signed(delta),
        };
        let Some(target) = target else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"));
        };
        self.at = target;
        Ok(target)
    }
}
