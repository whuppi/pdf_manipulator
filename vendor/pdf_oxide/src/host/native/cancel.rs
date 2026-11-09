//! CancelFlags — the two-flag cancellation check for executor I/O.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Every job carries one CancelFlags combining the two ways it can die:
//!
//!   executor flag — `executor_terminate` (instance dispose). Ends every job on
//!                   the executor. Set once, never cleared.
//!   job flag      — `executor_job_cancel` (PdfTask.cancel()). Ends exactly
//!                   one job. Set once, never cleared.
//!
//! Readers and writers check the flags at every I/O boundary. The
//! flags are set-once and monotonic — a CancelFlags that reports cancelled
//! never reports uncancelled again, so a stale read is at worst one
//! I/O boundary late, never wrong.
//!
//! A CancelFlags is NOT optional on readers/writers. Production I/O is
//! always cancellable; tests use [`CancelFlags::unconnected`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Combined executor + job cancellation flags. Cheap to clone.
#[derive(Clone)]
pub struct CancelFlags {
    lane: Arc<AtomicBool>,
    job: Arc<AtomicBool>,
}

impl CancelFlags {
    /// Flags wired to an executor's terminate flag and one job's cancel flag.
    pub fn new(lane: Arc<AtomicBool>, job: Arc<AtomicBool>) -> Self {
        Self { lane, job }
    }

    /// Fresh, never-set flags. For tests and for contexts
    /// that have no terminate surface (e.g. pure unit fixtures).
    pub fn unconnected() -> Self {
        Self {
            lane: Arc::new(AtomicBool::new(false)),
            job: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The engine-loop probe over the same two flags.
    pub fn check(&self) -> crate::cancel::CancelCheck {
        crate::cancel::CancelCheck::from_flags(self.lane.clone(), self.job.clone())
    }

    /// True once either the executor or the job has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.lane.load(Ordering::Relaxed) || self.job.load(Ordering::Relaxed)
    }
}

/// The error every cancelled I/O operation returns.
///
/// Deliberately NOT `ErrorKind::Interrupted`: std's I/O combinators
/// (`read_exact`, `read_to_end`, `Write::write_all`, ...) silently
/// RETRY on Interrupted, which turns a cancelled read into an
/// infinite retry loop inside the engine. `ErrorKind::Other` is
/// retried by nothing — the op unwinds at the first I/O boundary.
/// Do not "correct" this to Interrupted.
pub fn cancelled() -> std::io::Error {
    std::io::Error::other("operation cancelled")
}

impl std::fmt::Debug for CancelFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelFlags")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconnected_is_not_cancelled() {
        assert!(!CancelFlags::unconnected().is_cancelled());
    }

    #[test]
    fn lane_flag_cancels() {
        let lane = Arc::new(AtomicBool::new(false));
        let token = CancelFlags::new(lane.clone(), Arc::new(AtomicBool::new(false)));
        assert!(!token.is_cancelled());
        lane.store(true, Ordering::SeqCst);
        assert!(token.is_cancelled());
    }

    #[test]
    fn job_flag_cancels() {
        let job = Arc::new(AtomicBool::new(false));
        let token = CancelFlags::new(Arc::new(AtomicBool::new(false)), job.clone());
        assert!(!token.is_cancelled());
        job.store(true, Ordering::SeqCst);
        assert!(token.is_cancelled());
    }

    #[test]
    fn each_stops_after_the_item_that_cancelled() {
        let job = Arc::new(AtomicBool::new(false));
        let check = CancelFlags::new(Arc::new(AtomicBool::new(false)), job.clone()).check();
        let mut calls = 0;
        let result = check.each(0..100, |i| {
            calls += 1;
            if i == 2 {
                job.store(true, Ordering::SeqCst);
            }
            Ok(())
        });
        assert_eq!(calls, 3);
        let err = result.expect_err("a cancelled loop returns the cancelled error");
        assert!(err.to_string().contains("operation cancelled"));
    }

    #[test]
    fn each_runs_every_item_without_a_cancel() {
        let check = CancelFlags::unconnected().check();
        let mut seen = Vec::new();
        check
            .each(0..5, |i| {
                seen.push(i);
                Ok(())
            })
            .unwrap();
        assert_eq!(seen, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn each_returns_the_bodys_own_error() {
        let check = CancelFlags::unconnected().check();
        let result = check.each(0..5, |i| {
            if i == 1 {
                Err(crate::error::Error::InvalidPdf("body".into()))
            } else {
                Ok(())
            }
        });
        assert!(result.unwrap_err().to_string().contains("body"));
    }

    #[test]
    fn clones_share_flags() {
        let job = Arc::new(AtomicBool::new(false));
        let token = CancelFlags::new(Arc::new(AtomicBool::new(false)), job.clone());
        let clone = token.clone();
        job.store(true, Ordering::SeqCst);
        assert!(clone.is_cancelled());
    }
}
