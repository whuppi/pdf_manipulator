//! Process-level cross-document font cache.
//!
//! When processing multiple PDFs in sequence or parallel, the same font programs
//! (e.g., Helvetica, Times-Roman, Arial) appear repeatedly across documents.
// ── pdf_manipulator patch: the cache is bounded by bytes, not an LRU of entries ──
//! This module provides a global cache keyed by font identity hash so that
//! already-parsed `FontInfo` instances can be reused across documents without
//! re-parsing.
//!
//! The cache is safe for concurrent access from multiple threads and bounds
//! memory by bytes: the fonts it holds are charged what they take, and the
//! oldest are evicted when a new one would exceed the budget.
// ── end pdf_manipulator patch ──
//!
//! # Usage
//!
//! The cache is integrated automatically into `PdfDocument::load_fonts()`.
//! For explicit memory management, call `clear_global_font_cache` between
//! batches or when reclaiming memory.
//!
//! ```no_run
//! use pdf_oxide::fonts::global_cache::{clear_global_font_cache, global_font_cache_stats};
//!
//! // Process many PDFs... fonts are cached automatically
//!
//! // Check cache statistics
// ── pdf_manipulator patch: the capacity is a number of bytes ──
//! let (entries, capacity_bytes) = global_font_cache_stats();
//! println!("Global font cache: {} fonts, budget {} bytes", entries, capacity_bytes);
// ── end pdf_manipulator patch ──
//!
//! // Clear when done to free memory
//! clear_global_font_cache();
//! ```

use super::FontInfo;
// ── pdf_manipulator patch: the byte-bounded cache and its charge ──
use crate::host::caches;
use crate::host::font_budget::ByteCache;
// ── end pdf_manipulator patch ──
use std::sync::Arc;

// ── pdf_manipulator patch: the cache is bounded by the bytes its fonts take, not by their number ──
/// A bounded font cache with FIFO eviction, charged by the bytes each font
/// takes ([`font_bytes`]).
///
/// Lookups are read-only (`&self`). Eviction is first-in-first-out
/// rather than LRU; the cache is correctness-neutral (a miss just re-parses the
/// same font to the same result), so the policy only affects memory/perf.
pub(crate) struct FifoFontCache {
    fonts: ByteCache<u64, Arc<FontInfo>>,
}

impl FifoFontCache {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self { fonts: ByteCache::fifo(max_bytes) }
    }

    pub(crate) fn get(&self, key: u64) -> Option<Arc<FontInfo>> {
        self.fonts.peek(&key).map(Arc::clone)
    }

    /// Keeps `font` under `key`, evicting the oldest fonts until it fits; a
    /// font larger than the whole budget is not kept.
    pub(crate) fn insert(&mut self, key: u64, font: Arc<FontInfo>) {
        self.fonts.put(key, font);
    }

    pub(crate) fn len(&self) -> usize {
        self.fonts.len()
    }

    /// The budget, in bytes.
    pub(crate) fn capacity(&self) -> usize {
        self.fonts.budget()
    }

    pub(crate) fn clear(&mut self) {
        self.fonts.clear();
    }

    pub(crate) fn set_capacity(&mut self, max_bytes: usize) {
        self.fonts.set_budget(max_bytes);
    }
}
// ── end pdf_manipulator patch ──

// ── pdf_manipulator patch: the cache belongs to the executor, not to the process ──
// The cache lives on the lane's executor (the process-wide default when no
// executor is installed); these functions are one get or one put on it.
// ── end pdf_manipulator patch ──

/// Look up a font in the global cross-document cache.
///
/// Returns `Some(Arc<FontInfo>)` if a font with the given identity hash was
/// previously cached (from any document), or `None` on cache miss.
///
/// This is called from `PdfDocument::load_fonts()` before attempting to parse
/// a font from its dictionary.
pub fn global_font_cache_get(identity_hash: u64) -> Option<Arc<FontInfo>> {
    // Concurrent read lock; on poison (another thread panicked) skip the cache
    // rather than propagating the panic — font loading should still work.
    // ── pdf_manipulator patch: the executor's cache ──
    caches::with(|e| e.font(identity_hash))
    // ── end pdf_manipulator patch ──
}

/// Insert a parsed font into the global cross-document cache.
///
/// Called after successfully parsing a `FontInfo` from a font dictionary.
/// The font is keyed by its identity hash so that structurally identical fonts
/// in other documents will get a cache hit.
pub fn global_font_cache_insert(identity_hash: u64, font: Arc<FontInfo>) {
    // ── pdf_manipulator patch: the executor's cache ──
    caches::with(|e| e.put_font(identity_hash, font));
    // ── end pdf_manipulator patch ──
}

/// Clear all entries from the global font cache.
///
/// Call this between batch processing runs or when you need to reclaim memory.
/// Subsequent `PdfDocument` operations will rebuild the cache as fonts are
/// encountered.
///
/// # Example
///
/// ```no_run
/// use pdf_oxide::fonts::global_cache::clear_global_font_cache;
///
/// // After processing a batch of PDFs
/// clear_global_font_cache();
/// ```
pub fn clear_global_font_cache() {
    // ── pdf_manipulator patch: the executor's cache ──
    caches::with(|e| e.clear_fonts());
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: the capacity is a number of bytes ──
/// Return the number of fonts in the global font cache and its budget.
///
/// Returns `(current_entries, max_capacity_bytes)`.
///
/// # Example
///
/// ```no_run
/// use pdf_oxide::fonts::global_cache::global_font_cache_stats;
///
/// let (entries, capacity_bytes) = global_font_cache_stats();
/// println!("Global font cache: {} fonts, budget {} bytes", entries, capacity_bytes);
/// ```
// ── end pdf_manipulator patch ──
pub fn global_font_cache_stats() -> (usize, usize) {
    // ── pdf_manipulator patch: the executor's cache ──
    caches::with(|e| e.font_stats())
    // ── end pdf_manipulator patch ──
}

// ── pdf_manipulator patch: the capacity is a number of bytes ──
/// Set the most bytes of fonts the global font cache holds.
///
/// A font is charged what it takes (its embedded program, its ToUnicode CMap
/// and its tables). If `max_bytes` is smaller than what the cache holds, the
/// oldest fonts are evicted immediately. Setting this to 0 disables caching
/// (every insert is refused), and so does a budget smaller than one font.
///
/// # Example
///
/// ```no_run
/// use pdf_oxide::fonts::global_cache::set_global_font_cache_capacity;
///
/// // Keep up to 64 MiB of fonts for large batch processing
/// set_global_font_cache_capacity(64 * 1024 * 1024);
/// ```
pub fn set_global_font_cache_capacity(max_bytes: usize) {
    caches::with(|e| e.set_font_budget(max_bytes));
}
// ── end pdf_manipulator patch ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::font_dict::Encoding;
    use std::collections::HashMap;

    /// Create a minimal FontInfo for testing.
    fn make_test_font(name: &str) -> FontInfo {
        FontInfo {
            base_font: name.to_string(),
            subtype: "Type1".to_string(),
            encoding: Encoding::Standard("WinAnsiEncoding".to_string()),
            to_unicode: None,
            font_weight: None,
            flags: None,
            stem_v: None,
            ascent: 0.95,
            descent: -0.35,
            embedded_font_data: None,
            truetype_cmap: std::sync::OnceLock::new(),
            embedded_glyph_names: std::sync::OnceLock::new(),
            is_truetype_font: false,
            cid_to_gid_map: None,
            cid_system_info: None,
            cid_font_type: None,
            widths: None,
            first_char: None,
            last_char: None,
            font_matrix_a: 0.001,
            default_width: 600.0,
            cid_widths: None,
            cid_default_width: 1000.0,
            has_explicit_dw: false,
            cff_gid_map: None,
            cff_cid_to_gid: None,
            multi_char_map: HashMap::new(),
            byte_to_char_table: std::sync::OnceLock::new(),
            type0_unicode_memo: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            byte_to_width_table: std::sync::OnceLock::new(),
            weight_memo: std::sync::OnceLock::new(),
            italic_memo: std::sync::OnceLock::new(),
            std14_memo: std::sync::OnceLock::new(),
            diff_glyph_names: std::collections::HashMap::new(),
            wmode: 0,
            cid_vertical_metrics: None,
            cid_default_vertical_metrics: crate::fonts::VerticalMetrics::SPEC_DEFAULT,
            cjk_substitution: None,
        }
    }

    // ---- Unit tests for FifoFontCache (no global state, safe for parallel execution) ----

    #[test]
    fn test_fifo_cache_insert_and_get() {
        // ── pdf_manipulator patch: the budget is in bytes, and this one is room enough ──
        let mut cache = FifoFontCache::new(ROOMY);
        // ── end pdf_manipulator patch ──
        let font = Arc::new(make_test_font("Helvetica"));

        assert!(cache.get(100).is_none());

        cache.insert(100, Arc::clone(&font));
        let cached = cache.get(100);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().base_font, "Helvetica");
    }

    // ── pdf_manipulator patch: the cache is bounded by bytes, so its tests count bytes ──
    /// The bytes one test font is charged; every test font is named alike in length.
    fn unit() -> usize {
        crate::host::font_budget::font_bytes(&make_test_font("F1"))
    }

    /// A budget no test fills.
    const ROOMY: usize = 1 << 30;

    #[test]
    fn test_fifo_cache_eviction() {
        let mut cache = FifoFontCache::new(3 * unit());

        cache.insert(10, Arc::new(make_test_font("F1")));
        cache.insert(20, Arc::new(make_test_font("F2")));
        cache.insert(30, Arc::new(make_test_font("F3")));

        // Reads do not affect eviction order (FIFO, not LRU).
        cache.get(10);

        // Insert F4 — the byte budget is exceeded, so F1 (first in) goes.
        cache.insert(40, Arc::new(make_test_font("F4")));

        assert!(cache.get(10).is_none(), "F1 (oldest) should have been evicted");
        assert!(cache.get(20).is_some(), "F2 should still be cached");
        assert!(cache.get(30).is_some(), "F3 should still be cached");
        assert!(cache.get(40).is_some(), "F4 should be cached");
    }

    #[test]
    fn test_fifo_cache_clear() {
        let mut cache = FifoFontCache::new(ROOMY);
        cache.insert(1, Arc::new(make_test_font("A")));
        cache.insert(2, Arc::new(make_test_font("B")));
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn test_fifo_cache_set_capacity() {
        let mut cache = FifoFontCache::new(ROOMY);

        for i in 0..5 {
            cache.insert(i, Arc::new(make_test_font(&format!("Font{}", i))));
        }
        assert_eq!(cache.len(), 5);

        // Shrink to two fonts' bytes — evicts the 3 oldest entries (0,1,2); 3,4 survive.
        cache.set_capacity(2 * unit_of("Font0"));
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.capacity(), 2 * unit_of("Font0"));
        assert!(cache.get(3).is_some());
        assert!(cache.get(4).is_some());
    }

    /// The bytes the test font called `name` is charged.
    fn unit_of(name: &str) -> usize {
        crate::host::font_budget::font_bytes(&make_test_font(name))
    }

    #[test]
    fn test_fifo_cache_duplicate_key_update() {
        let mut cache = FifoFontCache::new(ROOMY);

        cache.insert(50, Arc::new(make_test_font("OldFont")));
        assert_eq!(cache.get(50).unwrap().base_font, "OldFont");

        cache.insert(50, Arc::new(make_test_font("NewFont")));
        assert_eq!(cache.get(50).unwrap().base_font, "NewFont");
        assert_eq!(cache.len(), 1, "Duplicate key should not increase size");
    }

    #[test]
    fn test_fifo_cache_eviction_order_is_insertion_order() {
        let mut cache = FifoFontCache::new(3 * unit());
        cache.insert(1, Arc::new(make_test_font("A")));
        cache.insert(2, Arc::new(make_test_font("B")));
        cache.insert(3, Arc::new(make_test_font("C")));

        // Reads don't change order; inserting evicts the first-inserted key.
        cache.get(1);
        cache.get(3);
        cache.insert(4, Arc::new(make_test_font("D")));
        assert!(cache.get(1).is_none(), "Key 1 (first in) should be evicted");
        assert!(cache.get(2).is_some());
        assert!(cache.get(3).is_some());
        assert!(cache.get(4).is_some());
    }
    // ── end pdf_manipulator patch ──

    // ---- Integration test for global API (single test to avoid parallel interference) ----

    #[test]
    fn test_global_api_insert_get_clear_stats() {
        // Use very high unique keys to avoid collisions with other tests
        let key_base = 9_000_000u64;

        // Insert
        let font = Arc::new(make_test_font("GlobalTestFont"));
        global_font_cache_insert(key_base, Arc::clone(&font));

        // Get (hit)
        let cached = global_font_cache_get(key_base);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().base_font, "GlobalTestFont");

        // Get (miss)
        assert!(global_font_cache_get(key_base + 999).is_none());

        // Stats: size should be at least 1
        let (size, cap) = global_font_cache_stats();
        assert!(size >= 1);
        assert!(cap > 0);

        // Clear and verify
        clear_global_font_cache();
        assert!(global_font_cache_get(key_base).is_none());

        let (size_after, _) = global_font_cache_stats();
        assert_eq!(size_after, 0);

        // Restore capacity in case another test changed it
        // ── pdf_manipulator patch: the default budget is in bytes ──
        set_global_font_cache_capacity(crate::host::font_budget::GLOBAL_FONT_BYTES);
        // ── end pdf_manipulator patch ──
    }
}
