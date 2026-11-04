//! The probe's measuring tape: the engine's one counting allocator, read
//! process-wide.
//!
//! The `#[global_allocator]` lives in `pdf_oxide::host::heap`, which counts every
//! allocation of the process, the ones the probes pay for included. The probe
//! reads the process-wide pair, not the per-thread one: its steps run engine work
//! on threads of their own.

use pdf_oxide::host::heap;

/// Live allocation size right now.
pub fn current() -> usize {
    heap::process_live()
}

/// Sets the peak to the current live count, so a step's growth starts
/// from where the last one ended.
pub fn reset_peak() {
    heap::process_reset_peak();
}

/// How far above `base` the peak reached since the last `reset_peak()`.
pub fn peak_over(base: usize) -> usize {
    heap::process_peak().saturating_sub(base)
}
