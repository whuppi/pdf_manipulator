//! Bytes that stay in the caller's source until the save.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! An op that is handed a file to embed keeps the reader it was handed and
//! stages the file as a stream whose data is an extent over a [`Source`]
//! built from that reader. The save writes the stream through
//! `write_indirect`, which reads the extent in pieces, so the file passes
//! from its source to the output without being held.

use std::collections::HashMap;
use std::io::Read;

use crate::error::Result;
use crate::object::{Extent, ObjectRef, StreamBody};
use crate::source::{ReadSeek, Source};

/// Bytes moved per read when a staged file is summed.
const PIECE: usize = 64 * 1024;

/// The whole of `reader` as the data of a stream.
pub(crate) fn body_over(reader: Box<dyn ReadSeek>) -> Result<StreamBody> {
    let source = Source::over(reader);
    let len = source.len()?;
    Ok(StreamBody::Extent(Extent::new(source, 0, len, ObjectRef::new(0, 0), &HashMap::new())))
}

/// The MD5 sum of a staged body, read in pieces.
#[cfg(feature = "legacy-crypto")]
pub(crate) fn md5_of(body: &StreamBody) -> Result<Vec<u8>> {
    use md5::{Digest, Md5};

    let mut hasher = Md5::new();
    let mut reader = body.raw_reader()?;
    let mut piece = vec![0u8; PIECE];
    loop {
        crate::cancel::poll()?;
        let n = reader.read(&mut piece)?;
        if n == 0 {
            return Ok(hasher.finalize().to_vec());
        }
        hasher.update(&piece[..n]);
    }
}
