//! CancelToken — the two-flag cancellation check for lane I/O.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Every job carries one token combining the two ways it can die:
//!
//!   lane flag — `lane_kill` (instance dispose). Kills every job on
//!               the lane. Set once, never cleared.
//!   job flag  — `lane_job_cancel` (PdfTask.cancel()). Kills exactly
//!               one job. Set once, never cleared.
//!
//! Readers and writers check the token at every I/O boundary. The
//! flags are set-once and monotonic — a token that reports cancelled
//! never reports uncancelled again, so a stale read is at worst one
//! I/O boundary late, never wrong.
//!
//! Tokens are NOT optional on readers/writers. Production I/O is
//! always cancellable; tests use [`CancelToken::unconnected`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Combined lane + job cancellation flags. Cheap to clone.
#[derive(Clone)]
pub struct CancelToken {
    lane: Arc<AtomicBool>,
    job: Arc<AtomicBool>,
}

impl CancelToken {
    /// Token wired to a lane's kill flag and one job's cancel flag.
    pub fn new(lane: Arc<AtomicBool>, job: Arc<AtomicBool>) -> Self {
        Self { lane, job }
    }

    /// Token with fresh, never-set flags. For tests and for contexts
    /// that have no kill surface (e.g. pure unit fixtures).
    pub fn unconnected() -> Self {
        Self {
            lane: Arc::new(AtomicBool::new(false)),
            job: Arc::new(AtomicBool::new(false)),
        }
    }

    /// True once either the lane or the job has been cancelled.
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

impl std::fmt::Debug for CancelToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unconnected_is_not_cancelled() {
        assert!(!CancelToken::unconnected().is_cancelled());
    }

    #[test]
    fn lane_flag_cancels() {
        let lane = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new(lane.clone(), Arc::new(AtomicBool::new(false)));
        assert!(!token.is_cancelled());
        lane.store(true, Ordering::SeqCst);
        assert!(token.is_cancelled());
    }

    #[test]
    fn job_flag_cancels() {
        let job = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new(Arc::new(AtomicBool::new(false)), job.clone());
        assert!(!token.is_cancelled());
        job.store(true, Ordering::SeqCst);
        assert!(token.is_cancelled());
    }

    #[test]
    fn clones_share_flags() {
        let job = Arc::new(AtomicBool::new(false));
        let token = CancelToken::new(Arc::new(AtomicBool::new(false)), job.clone());
        let clone = token.clone();
        job.store(true, Ordering::SeqCst);
        assert!(clone.is_cancelled());
    }
}
