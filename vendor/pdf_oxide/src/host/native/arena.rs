//! Per-operation arena allocator with cooperative cancellation.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Each PDF operation gets a fresh `OperationArena`. The inner
//! `bumpalo::Bump` provides O(1) bulk deallocation: when the arena
//! drops, ALL memory for that operation is freed in one shot.
//!
//! Lifecycle:
//!   Task received → arena created → engine runs → arena dropped
//!   Success, error, cancel, timeout — all end the same way: drop.
//!
//! The arena also carries a cancellation flag shared with the host
//! so the engine can check early-exit without polling.

use bumpalo::Bump;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A scoped arena for one PDF operation.
///
/// Created at task start. Dropped at task end. All engine memory
/// for this operation should flow through `arena()`.
pub struct OperationArena {
    bump: Bump,
    cancelled: Arc<AtomicBool>,
}

impl OperationArena {
    /// Create a new arena with a shared cancellation flag.
    pub fn new(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            bump: Bump::new(),
            cancelled,
        }
    }

    /// Access the inner bumpalo arena.
    pub fn bump(&self) -> &Bump {
        &self.bump
    }

    /// Check if this operation has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Get a clone of the cancellation flag (for sharing with readers/writers).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }

    /// Reset the arena (free all memory, keep the allocation).
    pub fn reset(&mut self) {
        self.bump.reset();
    }
}

impl Drop for OperationArena {
    fn drop(&mut self) {
        // Bump::drop frees all chunks. Nothing else to do.
        // This comment exists to make the lifecycle explicit:
        // when this arena drops, every byte allocated from it is gone.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_allocate_and_drop() {
        let flag = Arc::new(AtomicBool::new(false));
        let arena = OperationArena::new(flag);
        let _v = arena.bump().alloc(42u32);
        assert!(!arena.is_cancelled());
        // arena drops here — all memory freed
    }

    #[test]
    fn cancel_flag_propagates() {
        let flag = Arc::new(AtomicBool::new(false));
        let arena = OperationArena::new(flag.clone());
        assert!(!arena.is_cancelled());

        flag.store(true, Ordering::Relaxed);
        assert!(arena.is_cancelled());
    }

    #[test]
    fn cancel_from_arena() {
        let flag = Arc::new(AtomicBool::new(false));
        let arena = OperationArena::new(flag.clone());
        arena.cancel();
        assert!(flag.load(Ordering::Relaxed));
    }

    #[test]
    fn reset_clears_allocations() {
        let flag = Arc::new(AtomicBool::new(false));
        let mut arena = OperationArena::new(flag);

        for _ in 0..1000 {
            arena.bump().alloc([0u8; 1024]);
        }
        // ~1MB allocated
        arena.reset();
        // Memory freed, arena reusable
        let _v = arena.bump().alloc(1u8);
    }
}
