//! LaneState — the engine state owned by ONE lane.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A lane is one isolated execution unit: a dedicated thread on
//! native, a Web Worker on web. Each lane owns exactly one LaneState
//! and is the ONLY code that ever touches it. There is no sharing,
//! therefore no locks: plain HashMaps, a plain counter. The borrow
//! checker enforces single ownership at compile time.
//!
//! Do NOT add Mutex/RwLock/Atomic fields here. If a field seems to
//! need one, state is escaping its lane — fix the escape, don't add
//! the lock.

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::host::dispatch;
use crate::writer::DocumentBuilder;
use std::collections::HashMap;

/// All engine state pinned to one lane. Handles created on this lane
/// (documents, editors, builders) live and die with it.
pub struct LaneState {
    /// Open read-only documents, by handle id.
    pub documents: HashMap<u32, PdfDocument>,
    /// Open editors, by handle id.
    pub editors: HashMap<u32, DocumentEditor>,
    /// In-progress builders, by handle id.
    pub builders: HashMap<u32, DocumentBuilder>,
    /// Staged page operations per editor handle.
    pub page_ops: HashMap<u32, Vec<dispatch::PageOp>>,
    /// Next handle id. Plain u32 — single-threaded owner, no atomics.
    next_handle: u32,
}

impl LaneState {
    /// Create an empty lane state. Handle ids start at 1 (0 means
    /// "no handle" on the wire).
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
            editors: HashMap::new(),
            builders: HashMap::new(),
            page_ops: HashMap::new(),
            next_handle: 1,
        }
    }

    /// Allocate the next handle id.
    pub fn next_handle_id(&mut self) -> u32 {
        let id = self.next_handle;
        self.next_handle += 1;
        id
    }
}

impl Default for LaneState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_ids_start_at_one_and_increment() {
        let mut state = LaneState::new();
        assert_eq!(state.next_handle_id(), 1);
        assert_eq!(state.next_handle_id(), 2);
        assert_eq!(state.next_handle_id(), 3);
    }

    #[test]
    fn maps_start_empty() {
        let state = LaneState::new();
        assert!(state.documents.is_empty());
        assert!(state.editors.is_empty());
        assert!(state.builders.is_empty());
        assert!(state.page_ops.is_empty());
    }
}
