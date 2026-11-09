//! This thread's CPU clock, for the proofs and guards that bound how long an
//! operation takes. The wall clock counts every other test the runner has in
//! flight, so a bound on it fails when the machine is busy; a thread's own CPU
//! time does not move with the load. The integration tests in `tests/` read it
//! as `pdf_oxide::host::cpu_clock`.
//!
//! The clock ticks in [`resolution`]: about a microsecond on Linux and macOS,
//! the scheduler's tick (about 15.6 ms) on Windows. A bound on a short span
//! allows one tick per reading.

use std::time::Duration;

/// The CPU time this thread has spent so far.
#[cfg(unix)]
pub fn thread_cpu() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer for the one call.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime(CLOCK_THREAD_CPUTIME_ID) failed");
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

/// The smallest step [`thread_cpu`] moves by.
#[cfg(unix)]
pub fn resolution() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer for the one call.
    let rc = unsafe { libc::clock_getres(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_getres(CLOCK_THREAD_CPUTIME_ID) failed");
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    pub(super) struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        /// The time in 100-nanosecond units.
        pub(super) fn ticks(&self) -> u64 {
            (u64::from(self.high) << 32) | u64::from(self.low)
        }
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub(super) fn GetCurrentThread() -> *mut c_void;
        pub(super) fn GetThreadTimes(
            thread: *mut c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        pub(super) fn GetSystemTimeAdjustment(adjustment: *mut u32, increment: *mut u32, disabled: *mut i32) -> i32;
    }
}

/// The CPU time this thread has spent so far: its kernel and user time.
#[cfg(windows)]
pub fn thread_cpu() -> Duration {
    use windows::{FileTime, GetCurrentThread, GetThreadTimes};
    let (mut creation, mut exit, mut kernel, mut user) =
        (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
    // SAFETY: the pseudo-handle names this thread; the four out-pointers are
    // valid for the one call.
    let ok = unsafe { GetThreadTimes(GetCurrentThread(), &mut creation, &mut exit, &mut kernel, &mut user) };
    assert_ne!(ok, 0, "GetThreadTimes failed");
    Duration::from_nanos((kernel.ticks() + user.ticks()) * 100)
}

/// The smallest step [`thread_cpu`] moves by: the clock interrupt interval,
/// at which Windows charges a thread its time.
#[cfg(windows)]
pub fn resolution() -> Duration {
    let (mut adjustment, mut increment, mut disabled) = (0u32, 0u32, 0i32);
    // SAFETY: the three out-pointers are valid for the one call.
    let ok = unsafe { windows::GetSystemTimeAdjustment(&mut adjustment, &mut increment, &mut disabled) };
    assert_ne!(ok, 0, "GetSystemTimeAdjustment failed");
    Duration::from_nanos(u64::from(increment) * 100)
}

/// [`thread_cpu`] in seconds.
pub fn thread_cpu_secs() -> f64 {
    thread_cpu().as_secs_f64()
}

/// [`resolution`] in seconds.
pub fn resolution_secs() -> f64 {
    resolution().as_secs_f64()
}

/// A moment on this thread's CPU clock, read back as a stopwatch.
pub struct CpuInstant(Duration);

impl CpuInstant {
    /// This moment on this thread's CPU clock.
    pub fn now() -> Self {
        CpuInstant(thread_cpu())
    }

    /// The CPU time this thread has spent since `self`.
    pub fn elapsed(&self) -> Duration {
        thread_cpu().saturating_sub(self.0)
    }
}
