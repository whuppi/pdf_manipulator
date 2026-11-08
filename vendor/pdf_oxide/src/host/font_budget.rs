//! Byte budgets for the fonts a document, a page and the process hold.
//!
//! This file is part of the pdf_manipulator host layer (NOT upstream).
//!
//! A font is as large as its embedded program, its ToUnicode CMap and its
//! width tables, so a cache of fonts is bounded by the bytes its fonts take
//! and never by how many there are: [`ByteCache`] evicts the oldest entries
//! until a new one fits, and an entry larger than the whole budget is not
//! kept. [`font_bytes`] is the charge of one font.

use crate::fonts::{Encoding, FontInfo};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;

/// What each of a document's font caches may hold.
pub(crate) const DOCUMENT_FONT_BYTES: usize = MIB;

/// What the process-wide font cache may hold by default.
pub(crate) const GLOBAL_FONT_BYTES: usize = 2 * MIB;

/// What the process-wide ToUnicode CMap cache may hold.
pub(crate) const CMAP_CACHE_BYTES: usize = MIB;

/// What the process-wide cache of the machine's font files may hold (the
/// renderer's `FONT_BYTES_CACHE`), and what its cache of faces parsed from
/// them may be charged. One render of any PDF committed in the tree needed at
/// most 6.9 MiB of them (three fonts of 3.7 MiB at most), so 8 MiB holds a
/// whole document's fonts and about two of the largest. A font above the
/// budget (a CJK face of 10 to 20 MiB) is read again for each use, except the
/// CJK fallback, which the renderer keeps for the process.
pub(crate) const MACHINE_FONT_BYTES: usize = 8 * MIB;

/// See [`MACHINE_FONT_BYTES`]; a face is charged its font program again,
/// though the two caches share one allocation.
pub(crate) const MACHINE_FACE_BYTES: usize = 8 * MIB;

/// The fonts a page's text extraction loads when it starts, and the fonts it
/// keeps resident beside them while it reads the rest by name.
pub(crate) const PAGE_FONT_BYTES: usize = 512 * KIB;

/// The tables a font fills on first use (byte to character, byte to width)
/// and the memo of a composite font's lookups, none of which exist when the
/// font is first charged.
const LAZY_TABLES: usize = 4 * KIB;

/// Slot, control byte and the allocator's rounding of one hash map entry.
const MAP_SLOT: usize = 16;

/// The bytes a parsed ToUnicode CMap takes for each byte of its stream: a
/// mapping written as `<20> <4E00>\n` is one map slot and one `String`.
pub(crate) const CMAP_GROWTH: usize = 8;

/// The bytes one font holds, as far as they can be known when it is loaded.
pub(crate) fn font_bytes(font: &FontInfo) -> usize {
    let encoding = match &font.encoding {
        Encoding::Standard(name) => name.capacity(),
        Encoding::Custom(map) => map.capacity() * (MAP_SLOT + 8),
        Encoding::Identity => 0,
    };
    let tables = font.cid_widths.as_ref().map_or(0, |m| m.capacity() * (MAP_SLOT + 8))
        + font.cff_gid_map.as_ref().map_or(0, |m| m.capacity() * (MAP_SLOT + 8))
        + font.cff_cid_to_gid.as_ref().map_or(0, |m| m.capacity() * (MAP_SLOT + 8))
        + font.cid_vertical_metrics.as_ref().map_or(0, |m| m.capacity() * (MAP_SLOT + 24))
        + font.multi_char_map.capacity() * (MAP_SLOT + 40)
        + font.diff_glyph_names.capacity() * (MAP_SLOT + 40)
        + font.widths.as_ref().map_or(0, |w| w.capacity() * 4)
        + match &font.cid_to_gid_map {
            Some(crate::fonts::CIDToGIDMap::Explicit(map)) => map.capacity() * 2,
            _ => 0,
        };
    std::mem::size_of::<FontInfo>()
        + font.base_font.capacity()
        + font.subtype.capacity()
        + encoding
        + tables
        + font.embedded_font_data.as_ref().map_or(0, |data| data.capacity())
        + font.to_unicode.as_ref().map_or(0, |cmap| cmap.heap_bytes())
        + font.truetype_cmap.get().and_then(Option::as_ref).map_or(0, |cmap| cmap.len() * (MAP_SLOT + 8))
        + LAZY_TABLES
}

/// What a document's cache of the spans of the pages last read may hold. A
/// page whose spans take more is handed to its caller and not kept.
pub(crate) const PAGE_SPANS_BYTES: usize = 512 * KIB;

/// What a document's cache of the characters of the pages last read through
/// `extract_chars` may hold, and each of its caches of the pages' dictionaries,
/// of the spans of Form XObjects and of their images.
pub(crate) const PAGE_CHARS_BYTES: usize = 256 * KIB;
pub(crate) const PAGE_OBJECT_BYTES: usize = 256 * KIB;
pub(crate) const FORM_SPANS_BYTES: usize = 256 * KIB;
pub(crate) const FORM_IMAGES_BYTES: usize = 256 * KIB;

/// What a document's search index may hold: the text of each page searched
/// and the box of each of its runs.
pub(crate) const SEARCH_INDEX_BYTES: usize = MIB;

/// The bytes a list of spans takes: each record and every buffer it owns.
pub(crate) fn span_bytes(spans: &[crate::layout::TextSpan]) -> usize {
    spans.len() * std::mem::size_of::<crate::layout::TextSpan>()
        + spans
            .iter()
            .map(|s| {
                s.text.capacity()
                    + 4 * (s.char_widths.capacity() + s.char_x_offsets.capacity())
            })
            .sum::<usize>()
}

impl Charge for Arc<Vec<crate::layout::TextChar>> {
    fn charge(&self) -> usize {
        self.len() * std::mem::size_of::<crate::layout::TextChar>()
            + self.iter().map(|c| c.font_name.capacity()).sum::<usize>()
    }
}

impl Charge for Option<Vec<crate::layout::TextSpan>> {
    fn charge(&self) -> usize {
        std::mem::size_of::<Self>() + self.as_deref().map_or(0, span_bytes)
    }
}

impl Charge for Vec<crate::extractors::PdfImage> {
    fn charge(&self) -> usize {
        self.iter()
            .map(|image| {
                std::mem::size_of::<crate::extractors::PdfImage>()
                    + match image.data() {
                        crate::extractors::ImageData::Jpeg(bytes) => bytes.capacity(),
                        crate::extractors::ImageData::Raw { pixels, .. } => pixels.capacity(),
                    }
            })
            .sum()
    }
}

impl Charge for Arc<crate::search::SearchPageIndex> {
    fn charge(&self) -> usize {
        self.bytes()
    }
}

impl Charge for Arc<Vec<crate::layout::TextSpan>> {
    fn charge(&self) -> usize {
        span_bytes(self)
    }
}

/// What a cached value takes, for the budget it is charged against.
pub(crate) trait Charge {
    fn charge(&self) -> usize;
}

impl Charge for Arc<FontInfo> {
    fn charge(&self) -> usize {
        font_bytes(self)
    }
}

fn set_bytes(set: &[(String, Arc<FontInfo>)]) -> usize {
    set.iter().map(|(name, font)| name.capacity() + MAP_SLOT + font_bytes(font)).sum()
}

/// Whether a set of fonts, charged as [`set_bytes`] charges it, takes at most
/// `budget` bytes; stops counting once it does not.
pub(crate) fn set_fits<'a>(
    fonts: impl Iterator<Item = (&'a String, &'a Arc<FontInfo>)>,
    budget: usize,
) -> bool {
    let mut bytes = 0usize;
    for (name, font) in fonts {
        bytes += name.len() + MAP_SLOT + font_bytes(font);
        if bytes > budget {
            return false;
        }
    }
    true
}

impl Charge for Vec<(String, Arc<FontInfo>)> {
    fn charge(&self) -> usize {
        set_bytes(self)
    }
}

impl Charge for (Arc<Vec<(String, Arc<FontInfo>)>>, u64) {
    fn charge(&self) -> usize {
        set_bytes(&self.0)
    }
}

/// A cache bounded by the bytes its entries are charged, evicting the
/// oldest first. With `lru`, a read makes its entry the newest.
#[derive(Debug)]
pub(crate) struct ByteCache<K: Eq + Hash + Copy, V> {
    entries: HashMap<K, (V, usize)>,
    order: VecDeque<K>,
    bytes: usize,
    budget: usize,
    lru: bool,
}

impl<K: Eq + Hash + Copy, V> ByteCache<K, V> {
    pub(crate) fn fifo(budget: usize) -> Self {
        Self { entries: HashMap::new(), order: VecDeque::new(), bytes: 0, budget, lru: false }
    }

    pub(crate) fn lru(budget: usize) -> Self {
        Self { lru: true, ..Self::fifo(budget) }
    }

    /// The entry for `key`, made the newest when the cache is an LRU.
    pub(crate) fn get(&mut self, key: &K) -> Option<&V> {
        if self.lru && self.entries.contains_key(key) {
            self.order.retain(|k| k != key);
            self.order.push_back(*key);
        }
        self.entries.get(key).map(|(value, _)| value)
    }

    /// The entry for `key`, with no effect on the order.
    pub(crate) fn peek(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|(value, _)| value)
    }

    /// Keeps `value`, charged `bytes`, in place of any entry under `key`,
    /// evicting the oldest entries until it fits. A value larger than the
    /// budget is not kept.
    pub(crate) fn insert_charged(&mut self, key: K, value: V, bytes: usize) {
        self.remove(&key);
        if bytes > self.budget {
            return;
        }
        self.evict_to(self.budget - bytes);
        self.bytes += bytes;
        self.order.push_back(key);
        self.entries.insert(key, (value, bytes));
    }

    fn remove(&mut self, key: &K) {
        if let Some((_, bytes)) = self.entries.remove(key) {
            self.bytes -= bytes;
            self.order.retain(|k| k != key);
        }
    }

    fn evict_to(&mut self, limit: usize) {
        while self.bytes > limit {
            let Some(oldest) = self.order.pop_front() else { break };
            if let Some((_, bytes)) = self.entries.remove(&oldest) {
                self.bytes -= bytes;
            }
        }
    }

    pub(crate) fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        self.evict_to(budget);
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.bytes = 0;
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn budget(&self) -> usize {
        self.budget
    }
}

impl<K: Eq + Hash + Copy, V: Charge> ByteCache<K, V> {
    /// [`Self::insert_charged`] with the value's own charge.
    pub(crate) fn put(&mut self, key: K, value: V) {
        let bytes = value.charge();
        self.insert_charged(key, value, bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::ByteCache;

    #[test]
    fn a_value_larger_than_the_whole_budget_is_not_kept() {
        let mut cache = ByteCache::fifo(100);
        cache.insert_charged(1, "small", 40);
        cache.insert_charged(2, "huge", 101);
        assert_eq!(cache.len(), 1, "the value above the budget is not kept");
        assert!(cache.peek(&2).is_none());
        assert!(cache.peek(&1).is_some(), "it evicted nothing to try");
    }

    #[test]
    fn the_oldest_entries_go_until_a_new_one_fits() {
        let mut cache = ByteCache::fifo(100);
        cache.insert_charged(1, "a", 40);
        cache.insert_charged(2, "b", 40);
        cache.insert_charged(3, "c", 40);
        assert!(cache.peek(&1).is_none(), "the oldest went to make room");
        assert!(cache.peek(&2).is_some() && cache.peek(&3).is_some());
    }
}
