//! CancelCheck — the cancellation probe engine loops poll between pages.
//!
//! This file is fork-only code (NOT upstream).
//!
//! A loop that does CPU work between I/O calls (all-pages extract, search,
//! image reduction, validation, conversion) calls [`CancelCheck::check`]
//! at the top of each iteration, so a cancel stops it within one page,
//! one image or one rule batch instead of at the next I/O boundary.
//!
//!   native — wraps the lane's kill flag and the job's cancel flag.
//!   wasm32 — reads the job's cancel word through the `host_cancelled`
//!            import (a SharedArrayBuffer word the main thread stores to).
//!   none   — never cancelled: every caller outside a lane, and tests.
//!
//! Upstream code has no `CancelCheck` in scope, so the running job's check is
//! installed in a thread-local for the job's life ([`enter`]) and a loop in
//! such code polls it with [`poll`].

use crate::error::{Error, Result};
use std::cell::RefCell;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

// ── pdf_manipulator patch: the import tool/fuelprobe's host provides ──
#[cfg(all(feature = "fuel-probe", target_arch = "wasm32"))]
#[link(wasm_import_module = "fuelprobe")]
extern "C" {
    #[link_name = "poll"]
    fn fuelprobe_poll();
}
// ── end pdf_manipulator patch ──

/// Cheap-to-clone cancellation probe.
#[derive(Clone, Default, Debug)]
pub struct CancelCheck {
    #[cfg(not(target_arch = "wasm32"))]
    flags: Option<(Arc<AtomicBool>, Arc<AtomicBool>)>,
    #[cfg(target_arch = "wasm32")]
    host: bool,
    /// Tests only: counts every poll and cancels from the `n`th on.
    #[cfg(test)]
    trip: Option<Arc<(std::sync::atomic::AtomicUsize, usize)>>,
}

impl CancelCheck {
    /// A check that is never cancelled.
    pub fn none() -> Self {
        Self::default()
    }

    /// A check wired to a lane's kill flag and one job's cancel flag.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_flags(lane: Arc<AtomicBool>, job: Arc<AtomicBool>) -> Self {
        Self { flags: Some((lane, job)), ..Self::default() }
    }

    /// A check that reads the current job's cancel word from the host.
    #[cfg(target_arch = "wasm32")]
    pub fn from_host() -> Self {
        Self { host: true, ..Self::default() }
    }

    /// A check that counts its polls and reports a cancel from the `n`th
    /// poll on (`usize::MAX` never cancels): the cancel that lands at an
    /// exact point of an operation. Clones share the count.
    #[cfg(test)]
    pub(crate) fn tripping_at(n: usize) -> Self {
        Self { trip: Some(Arc::new((std::sync::atomic::AtomicUsize::new(0), n))), ..Self::default() }
    }

    /// The polls made so far on a check from [`Self::tripping_at`].
    #[cfg(test)]
    pub(crate) fn polls(&self) -> usize {
        self.trip.as_ref().map_or(0, |trip| trip.0.load(Ordering::Relaxed))
    }

    // ── pdf_manipulator patch: the check tool/fuelprobe runs every op under; it never cancels ──
    /// A check that is never cancelled and, with `fuel-probe` on wasm32,
    /// reports every poll to the host.
    #[cfg(feature = "fuel-probe")]
    pub fn probe() -> Self {
        Self::default()
    }
    // ── end pdf_manipulator patch ──

    /// True once the lane or the job has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        // ── pdf_manipulator patch: tell the fuel probe a poll happened ──
        #[cfg(all(feature = "fuel-probe", target_arch = "wasm32"))]
        // SAFETY: `fuelprobe.poll` takes nothing and returns nothing.
        unsafe {
            fuelprobe_poll()
        }
        // ── end pdf_manipulator patch ──
        #[cfg(test)]
        if let Some(trip) = &self.trip {
            return trip.0.fetch_add(1, Ordering::Relaxed) + 1 >= trip.1;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            match &self.flags {
                Some((lane, job)) => {
                    lane.load(Ordering::Relaxed) || job.load(Ordering::Relaxed)
                }
                None => false,
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.host && crate::host::wasm::host_cancelled() != 0
        }
    }

    /// `Err` with the cancelled I/O error once cancelled.
    ///
    /// The error is the one the readers and writers return, so a loop
    /// check and an I/O check are indistinguishable to the caller.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Io(std::io::Error::other("operation cancelled")))
        } else {
            Ok(())
        }
    }

    /// Runs `f` on each item, checking before every one: a cancel stops
    /// the loop within one item with the cancelled error. A loop whose
    /// body must `break`, `continue` or return early keeps a plain
    /// [`check`](Self::check) instead.
    pub fn each<T>(
        &self,
        items: impl IntoIterator<Item = T>,
        mut f: impl FnMut(T) -> Result<()>,
    ) -> Result<()> {
        for item in items {
            self.check()?;
            f(item)?;
        }
        Ok(())
    }
}

/// Iterations of an inner loop between two polls of a [`CancelCheck`]: a
/// poll is a relaxed atomic load, cheap, but not free in a loop whose body is
/// a few instructions.
pub(crate) const STRIDE: u32 = 4096;

/// Polls a [`CancelCheck`] once per [`STRIDE`] iterations of an inner loop.
pub(crate) struct Stride<'a> {
    cancel: Option<&'a CancelCheck>,
    count: u32,
}

impl<'a> Stride<'a> {
    pub(crate) fn new(cancel: &'a CancelCheck) -> Self {
        Self { cancel: Some(cancel), count: 0 }
    }

    /// A stride over the check installed by [`enter`], for a loop with no
    /// `CancelCheck` in scope.
    pub(crate) fn job() -> Self {
        Self { cancel: None, count: 0 }
    }

    /// Counts one iteration; every [`STRIDE`]th one checks for a cancel.
    pub(crate) fn tick(&mut self) -> Result<()> {
        self.count = self.count.wrapping_add(1);
        if self.count % STRIDE != 0 {
            return Ok(());
        }
        match self.cancel {
            Some(cancel) => cancel.check(),
            None => poll(),
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<CancelCheck>> = const { RefCell::new(None) };
}

/// Keeps a job's check installed for [`poll`]; dropping it restores the check
/// that was installed before.
#[must_use = "the check is installed only while the guard lives"]
pub struct JobGuard {
    previous: Option<CancelCheck>,
}

/// Installs `check` as the running job's check on this thread until the
/// returned guard drops.
pub fn enter(check: &CancelCheck) -> JobGuard {
    let previous = CURRENT.with(|current| current.borrow_mut().replace(check.clone()));
    JobGuard { previous }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        let previous = self.previous.take();
        CURRENT.with(|current| *current.borrow_mut() = previous);
    }
}

/// Polls the check installed by [`enter`]: the cancelled error once the job
/// is cancelled, `Ok` when nothing is installed.
pub fn poll() -> Result<()> {
    CURRENT.with(|current| match current.borrow().as_ref() {
        Some(check) => check.check(),
        None => Ok(()),
    })
}

/// [`poll`] for a loop that returns `io::Result`: the cancelled error is the
/// one the readers and writers return.
pub fn poll_io() -> std::io::Result<()> {
    poll().map_err(|error| match error {
        Error::Io(error) => error,
        other => std::io::Error::other(other),
    })
}
