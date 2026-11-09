//! A decoded stream read forward, positioned, reopened to go back.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A stream's filters decode forward only. [`Forward`] keeps one decoding
//! reader and the offset it has reached: a read at a later offset decodes on
//! from there, a read at an earlier one opens the stream again. It holds the
//! reader, never the bytes decoded so far.

use crate::error::Result;
use std::io::{self, Read};
use std::sync::Mutex;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

/// A decoding reader that may cross threads.
struct Decoded(Box<dyn Read>);

// SAFETY: the filter readers share state through `Rc`, which is not `Send`.
// Every `Rc` clone of one reader lives inside that reader's own tree, and the
// tree is moved as a whole, behind the `Mutex` of `Forward`, so no `Rc`
// count is touched from two threads at once and none is left behind.
unsafe impl Send for Decoded {}

type Open = Box<dyn Fn() -> Result<Box<dyn Read>> + Send + Sync>;

/// One stream's decoded bytes, read at offsets.
pub(crate) struct Forward {
    open: Open,
    /// The reader and the decoded offset it stands at.
    state: Mutex<Option<(Decoded, u64)>>,
    /// Times a read went back and opened the stream again.
    #[cfg(test)]
    reopens: AtomicUsize,
}

impl Forward {
    /// Reads decoded bytes of whatever `open` opens. `started` is a reader
    /// that `open` already made and has read to `at`, if there is one.
    pub(crate) fn new(open: Open, started: Option<(Box<dyn Read>, u64)>) -> Self {
        Self {
            open,
            state: Mutex::new(started.map(|(r, at)| (Decoded(r), at))),
            #[cfg(test)]
            reopens: AtomicUsize::new(0),
        }
    }

    /// Times a read has opened the stream again.
    #[cfg(test)]
    pub(crate) fn reopens(&self) -> usize {
        self.reopens.load(Ordering::Relaxed)
    }

    /// Up to `len` decoded bytes from decoded offset `at`; fewer when the
    /// stream ends, none when it ends before `at`.
    pub(crate) fn read_at(&self, at: u64, len: u64) -> Result<Vec<u8>> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let reusable = matches!(state.as_ref(), Some((_, here)) if *here <= at);
        if !reusable {
            #[cfg(test)]
            self.reopens.fetch_add(1, Ordering::Relaxed);
            *state = Some((Decoded((self.open)()?), 0));
        }
        let (reader, here) = state.as_mut().expect("a reader was just opened");
        let outcome = (|| -> io::Result<Vec<u8>> {
            let skip = at - *here;
            let skipped = io::copy(&mut reader.0.by_ref().take(skip), &mut io::sink())?;
            *here += skipped;
            if skipped < skip {
                return Ok(Vec::new());
            }
            let mut out = Vec::new();
            reader.0.by_ref().take(len).read_to_end(&mut out)?;
            *here += out.len() as u64;
            Ok(out)
        })();
        match outcome {
            Ok(bytes) => Ok(bytes),
            Err(e) => {
                // A reader that failed mid-way stands nowhere known.
                *state = None;
                Err(crate::encryption::stream::from_io(e))
            },
        }
    }
}
