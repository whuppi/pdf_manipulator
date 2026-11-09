//! The engine caches: the fonts, CMaps and fallback fonts one executor holds.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! Upstream keeps the parsed-font cache, the ToUnicode CMap cache and (here)
//! the runtime fallback fonts in process-wide statics, so every executor of a
//! process shared them and a reset could not empty them. An [`EngineCaches`] holds
//! the same three things for one executor. `EngineState` owns one and installs it
//! into a thread-local pointer for its whole life, so a reset, which drops the
//! `EngineState` and makes a new one, empties them, and no upstream function
//! signature changes: every call site reaches the state through [`with`].
//!
//! With nothing installed (the python, ffi and wasm.rs bindings, upstream
//! tests) [`with`] falls back to one process-wide default [`EngineCaches`], which is
//! what the statics were. On wasm32 a `thread_local!` is a plain static, so
//! the same source is per-executor on both platforms.
//!
//! **The rule:** a `RefCell` borrow lasts one get or one put, never across
//! engine code, and every accessor returns a clone. An accessor never calls
//! engine code, so [`with`] is never re-entered from inside its closure;
//! engine code that loads a font while it loads another (a Type 3 glyph
//! procedure naming a font) takes one short borrow per cache access and holds
//! none between them.

use crate::cache::MutexExt;
use crate::fonts::cmap::{CMap, CMapKey};
use crate::fonts::form_fallback::Fallback;
use crate::fonts::global_cache::FifoFontCache;
use crate::fonts::FontInfo;
use crate::host::font_budget::{ByteCache, CMAP_CACHE_BYTES, GLOBAL_FONT_BYTES};
use crate::host::heap;
use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;

/// One executor's caches and registrations.
pub struct EngineCaches {
    fonts: RefCell<FifoFontCache>,
    cmaps: RefCell<ByteCache<CMapKey, Arc<CMap>>>,
    fallback: RefCell<[Option<Arc<[u8]>>; 2]>,
    owner: ThreadId,
}

fn slot(kind: Fallback) -> usize {
    match kind {
        Fallback::Cjk => 0,
        Fallback::Emoji => 1,
    }
}

impl EngineCaches {
    /// Empty caches and no registrations, owned by the
    /// calling thread.
    pub fn new() -> Self {
        Self {
            fonts: RefCell::new(FifoFontCache::new(GLOBAL_FONT_BYTES)),
            cmaps: RefCell::new(ByteCache::lru(CMAP_CACHE_BYTES)),
            fallback: RefCell::new([None, None]),
            owner: std::thread::current().id(),
        }
    }

    /// The thread the caches were made on, which is the only thread that
    /// installs it.
    pub fn owner(&self) -> ThreadId {
        self.owner
    }

    /// The parsed font cached under `identity_hash`.
    pub(crate) fn font(&self, identity_hash: u64) -> Option<Arc<FontInfo>> {
        self.fonts.borrow().get(identity_hash)
    }

    /// Keeps `font` under `identity_hash`, evicting the oldest fonts until it
    /// fits; a font larger than the whole budget is not kept.
    pub(crate) fn put_font(&self, identity_hash: u64, font: Arc<FontInfo>) {
        self.fonts.borrow_mut().insert(identity_hash, font);
    }

    pub(crate) fn clear_fonts(&self) {
        self.fonts.borrow_mut().clear();
    }

    /// The fonts held and the font budget, in bytes.
    pub(crate) fn font_stats(&self) -> (usize, usize) {
        let fonts = self.fonts.borrow();
        (fonts.len(), fonts.capacity())
    }

    pub(crate) fn set_font_budget(&self, max_bytes: usize) {
        self.fonts.borrow_mut().set_capacity(max_bytes);
    }

    /// The parsed CMap cached under `key`, made the newest.
    pub(crate) fn cmap(&self, key: &CMapKey) -> Option<Arc<CMap>> {
        self.cmaps.borrow_mut().get(key).map(Arc::clone)
    }

    /// Keeps `cmap` under `key`, charged `bytes`.
    pub(crate) fn put_cmap(&self, key: CMapKey, cmap: Arc<CMap>, bytes: usize) {
        self.cmaps.borrow_mut().insert_charged(key, cmap, bytes);
    }

    pub(crate) fn clear_cmaps(&self) {
        self.cmaps.borrow_mut().clear();
    }

    pub(crate) fn cmap_count(&self) -> usize {
        self.cmaps.borrow().len()
    }

    /// Stores the font for `kind`, replacing any earlier one.
    pub fn register(&self, kind: Fallback, bytes: Vec<u8>) {
        self.fallback.borrow_mut()[slot(kind)] = Some(Arc::from(bytes));
    }

    /// The registered font for `kind`, if any.
    pub fn registered(&self, kind: Fallback) -> Option<Arc<[u8]>> {
        self.fallback.borrow()[slot(kind)].clone()
    }
}

impl Default for EngineCaches {
    fn default() -> Self {
        Self::new()
    }
}

std::thread_local! {
    static INSTALLED: Cell<Option<NonNull<EngineCaches>>> = const { Cell::new(None) };
}

fn process_default() -> &'static Mutex<EngineCaches> {
    static DEFAULT: OnceLock<Mutex<EngineCaches>> = OnceLock::new();
    DEFAULT.get_or_init(|| Mutex::new(EngineCaches::new()))
}

/// Makes `e` the caches of the calling thread and clears what the thread's
/// previous owner left in the thread-locals the engine keeps: the warning
/// sink, the object-resolving stack, the office conversion context and the
/// rasteriser's fill count. `e` must stay where it is until [`uninstall`].
pub fn install(e: &EngineCaches) {
    INSTALLED.with(|installed| installed.set(Some(NonNull::from(e))));
    drop(crate::extractors::warnings::drain_global_warnings());
    crate::document::reset_resolving();
    #[cfg(feature = "office")]
    crate::converters::office::reset_conversion_context();
    #[cfg(feature = "rendering")]
    crate::rendering::band::reset_fills();
    heap::reset_peak();
}

/// Removes `e` from the calling thread, if it is the one installed there.
pub fn uninstall(e: &EngineCaches) {
    INSTALLED.with(|installed| {
        if installed.get() == Some(NonNull::from(e)) {
            installed.set(None);
        }
    });
}

/// Runs `f` on the calling thread's installed caches, or on the process-wide
/// default when none is installed. `f` is one get or one put: it must not
/// call engine code or `with` again.
pub fn with<R>(f: impl FnOnce(&EngineCaches) -> R) -> R {
    match INSTALLED.with(Cell::get) {
        // SAFETY: `install` stored the address of the caches its owner keeps
        // alive until `uninstall` runs on this same thread, and a thread-local
        // is read only on its own thread.
        Some(installed) => f(unsafe { installed.as_ref() }),
        None => f(&process_default().lock_or_recover()),
    }
}
