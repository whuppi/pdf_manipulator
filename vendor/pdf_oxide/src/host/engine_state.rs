//! EngineState — the engine state owned by ONE executor.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! An executor is a dedicated thread on native, a Web Worker on web.
//! Each executor owns exactly one EngineState
//! and is the ONLY code that ever touches it. There is no sharing,
//! therefore no locks: plain HashMaps, a plain counter. The borrow
//! checker enforces single ownership at compile time.
//!
//! Do NOT add Mutex/RwLock/Atomic fields here. If a field seems to
//! need one, state is escaping its executor — fix the escape, don't add
//! the lock.

use crate::document::PdfDocument;
use crate::editor::DocumentEditor;
use crate::host::caches::{self, EngineCaches};
use crate::writer::DocumentBuilder;
use std::collections::HashMap;

/// All engine state pinned to one executor. Handles created on this executor
/// (documents, editors, builders) live and die with it.
pub struct EngineState {
    /// Open read-only documents, by handle id.
    pub documents: HashMap<u32, PdfDocument>,
    /// Open editors, by handle id.
    pub editors: HashMap<u32, DocumentEditor>,
    /// In-progress builders, by handle id.
    pub builders: HashMap<u32, DocumentBuilder>,
    /// Next handle id. Plain u32 — single-threaded owner, no atomics.
    next_handle: u32,
    /// The executor's font and CMap caches and fallback fonts, installed for the
    /// thread that made this state until it drops. Boxed so the installed
    /// address survives a move of the state; last, so the handles above drop
    /// before the caches they may have filled.
    caches: Box<EngineCaches>,
}

impl EngineState {
    /// Create an empty engine state. Handle ids start at 1 (0 means
    /// "no handle" on the wire).
    pub fn new() -> Self {
        let caches = Box::new(EngineCaches::new());
        caches::install(&caches);
        Self {
            documents: HashMap::new(),
            editors: HashMap::new(),
            builders: HashMap::new(),
            next_handle: 1,
            caches,
        }
    }

    /// The caches this state owns.
    pub fn caches(&self) -> &EngineCaches {
        &self.caches
    }

    /// Allocate the next handle id.
    pub fn next_handle_id(&mut self) -> u32 {
        let id = self.next_handle;
        self.next_handle += 1;
        id
    }
}

impl Drop for EngineState {
    fn drop(&mut self) {
        caches::uninstall(&self.caches);
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_ids_start_at_one_and_increment() {
        let mut state = EngineState::new();
        assert_eq!(state.next_handle_id(), 1);
        assert_eq!(state.next_handle_id(), 2);
        assert_eq!(state.next_handle_id(), 3);
    }

    #[test]
    fn maps_start_empty() {
        let state = EngineState::new();
        assert!(state.documents.is_empty());
        assert!(state.editors.is_empty());
        assert!(state.builders.is_empty());
    }
}
