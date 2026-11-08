//! The document's one cache of decoded content, in a byte budget.

use crate::object::ObjectRef;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// The bytes the content cache may charge. The document's whole working set is
/// the 8 MiB object cache plus this: a form larger than the budget is never
/// cached and streams every time it is used.
pub const CONTENT_CACHE_BYTES: usize = 4 * 1024 * 1024;

/// Charged on top of an entry's bytes, so a flood of tiny forms is bounded too.
const ENTRY_OVERHEAD: usize = 64;

/// Decoded content by object, least recently used out first.
pub struct ContentCache {
    budget: usize,
    used: usize,
    tick: u64,
    entries: HashMap<ObjectRef, (Arc<Vec<u8>>, u64)>,
    /// Entries by the tick of their last use.
    order: BTreeMap<u64, ObjectRef>,
}

impl ContentCache {
    /// A cache with the document's budget, [`CONTENT_CACHE_BYTES`].
    pub fn new() -> Self {
        Self::with_budget(CONTENT_CACHE_BYTES)
    }

    /// A cache charging at most `budget` bytes.
    pub fn with_budget(budget: usize) -> Self {
        Self { budget, used: 0, tick: 0, entries: HashMap::new(), order: BTreeMap::new() }
    }

    /// What `len` bytes of content cost.
    fn charge(len: usize) -> usize {
        len + ENTRY_OVERHEAD
    }

    /// The most content bytes one entry may hold.
    pub fn room(&self) -> usize {
        self.budget.saturating_sub(ENTRY_OVERHEAD)
    }

    /// The bytes charged now; never more than the budget.
    pub fn used(&self) -> usize {
        self.used
    }

    /// The entries held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no entry is held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The content of `r`, marked as just used.
    pub fn get(&mut self, r: ObjectRef) -> Option<Arc<Vec<u8>>> {
        let (data, old) = self.entries.get_mut(&r)?;
        self.order.remove(old);
        self.tick += 1;
        *old = self.tick;
        self.order.insert(self.tick, r);
        Some(Arc::clone(data))
    }

    /// Admits `data` as the content of `r` when it fits the budget, evicting
    /// the least recently used until it does; `false` when it can never fit.
    pub fn insert(&mut self, r: ObjectRef, data: Arc<Vec<u8>>) -> bool {
        self.remove(r);
        let cost = Self::charge(data.len());
        if cost > self.budget {
            return false;
        }
        while self.used + cost > self.budget {
            let Some((_, oldest)) = self.order.pop_first() else { break };
            if let Some((evicted, _)) = self.entries.remove(&oldest) {
                self.used -= Self::charge(evicted.len());
            }
        }
        self.tick += 1;
        self.used += cost;
        self.order.insert(self.tick, r);
        self.entries.insert(r, (data, self.tick));
        true
    }

    fn remove(&mut self, r: ObjectRef) {
        if let Some((data, tick)) = self.entries.remove(&r) {
            self.order.remove(&tick);
            self.used -= Self::charge(data.len());
        }
    }
}

impl Default for ContentCache {
    fn default() -> Self {
        Self::new()
    }
}
